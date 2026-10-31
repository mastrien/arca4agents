//! Cota de uso dos agentes: leitura das fontes (statusline do Claude Code, app-server do Codex),
//! gravação das leituras e consulta para os gráficos.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, NaiveDateTime};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::dates::{self, DATETIME_FORMAT};
use crate::persistence::Database;

pub const CLAUDE_CODE: &str = "claude-code";
pub const CODEX: &str = "codex";

/// Uma leitura que repete a anterior só é gravada depois deste intervalo, para o gráfico saber
/// que o valor continuou o mesmo (e não que a fonte parou de mandar).
const REPEAT_AFTER_MINUTES: i64 = 30;
/// Leituras mais antigas que isto são apagadas.
const KEEP_DAYS: i64 = 180;
/// Tempo máximo esperando o `codex app-server` responder.
const CODEX_TIMEOUT: Duration = Duration::from_secs(30);

/// Leitura de uma janela de limite, ainda não gravada.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
  pub window_key: String,
  pub used_percent: f64,
  pub window_minutes: Option<i64>,
  pub resets_at: Option<NaiveDateTime>,
}

/// Leitura gravada. Também é o formato em que as leituras viajam do PC para o celular.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSample {
  pub agent: String,
  pub window_key: String,
  pub used_percent: f64,
  pub window_minutes: Option<i64>,
  pub resets_at: Option<String>,
  pub sampled_at: String,
}

/// Segundos Unix (número) ou data ISO 8601 (texto) → horário local.
fn parse_instant(value: &Value) -> Option<NaiveDateTime> {
  let utc = match value {
    Value::Number(n) => DateTime::from_timestamp(n.as_f64()? as i64, 0)?,
    Value::String(s) => DateTime::parse_from_rfc3339(s).ok()?.to_utc(),
    _ => return None,
  };
  Some(utc.with_timezone(&Local).naive_local())
}

fn claude_window_minutes(key: &str) -> Option<i64> {
  match key {
    "five_hour" => Some(5 * 60),
    k if k.starts_with("seven_day") => Some(7 * 24 * 60),
    _ => None,
  }
}

/// Janelas do campo `rate_limits` do JSON que o Claude Code manda para a statusline
/// (`{"five_hour": {"used_percentage": 23, "resets_at": 1761...}, "seven_day": {...}}`).
/// O campo só existe nos planos de assinatura; sem ele, nenhuma leitura.
pub fn parse_claude_statusline(input: &Value) -> Vec<Reading> {
  let Some(limits) = input.get("rate_limits").and_then(Value::as_object) else { return Vec::new() };
  let mut readings: Vec<Reading> = limits.iter()
    .filter_map(|(key, window)| {
      let used = window.get("used_percentage").or_else(|| window.get("utilization"))?.as_f64()?;
      let resets = window.get("resets_at").or_else(|| window.get("reset_at"));
      Some(Reading {
        window_key: key.clone(),
        used_percent: used,
        window_minutes: claude_window_minutes(key),
        resets_at: resets.and_then(parse_instant),
      })
    })
    .collect();
  readings.sort_by_key(|r| (r.window_minutes.unwrap_or(i64::MAX), r.window_key.clone()));
  readings
}

/// Janelas da resposta de `account/rateLimits/read` do app-server do Codex. Cada limite tem uma
/// janela `primary` e talvez uma `secondary`; limites além do padrão (`codex`) ganham o id como prefixo.
pub fn parse_codex_rate_limits(result: &Value) -> Vec<Reading> {
  let limits: Vec<&Value> = match result.get("rateLimitsByLimitId").and_then(Value::as_object) {
    Some(map) if !map.is_empty() => map.values().collect(),
    _ => result.get("rateLimits").into_iter().collect(),
  };
  let mut readings = Vec::new();
  for limit in limits {
    let prefix = match limit.get("limitId").and_then(Value::as_str) {
      None | Some("codex") => String::new(),
      Some(id) => format!("{id}:"),
    };
    for slot in ["primary", "secondary"] {
      let Some(window) = limit.get(slot).filter(|w| w.is_object()) else { continue };
      let Some(used) = window.get("usedPercent").and_then(Value::as_f64) else { continue };
      readings.push(Reading {
        window_key: format!("{prefix}{slot}"),
        used_percent: used,
        window_minutes: window.get("windowDurationMins").and_then(Value::as_i64),
        resets_at: window.get("resetsAt").and_then(parse_instant),
      });
    }
  }
  readings
}

/// Grava as leituras de um agente. Uma leitura igual à última da mesma janela (mesmo percentual e
/// mesma virada) só entra se a última tiver mais de 30 min. Devolve quantas foram gravadas.
pub fn record(db: &Database, agent: &str, readings: &[Reading], now: NaiveDateTime) -> Result<usize> {
  let conn = db.conn();
  let now_text = now.format(DATETIME_FORMAT).to_string();
  let mut written = 0;
  for reading in readings {
    let resets = reading.resets_at.map(|t| t.format(DATETIME_FORMAT).to_string());
    let last: Option<(f64, Option<String>, String)> = conn.query_row(
      "SELECT used_percent, resets_at, sampled_at FROM usage_samples
       WHERE agent = ?1 AND window_key = ?2 ORDER BY sampled_at DESC, sample_id DESC LIMIT 1",
      params![agent, reading.window_key],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).optional()?;
    let repeated = last.is_some_and(|(used, last_resets, sampled_at)| {
      (used - reading.used_percent).abs() < 0.01
        && last_resets == resets
        && dates::parse_datetime(&sampled_at).is_some_and(|t| (now - t).num_minutes() < REPEAT_AFTER_MINUTES)
    });
    if repeated {
      continue;
    }
    upsert(conn, &UsageSample {
      agent: agent.to_string(),
      window_key: reading.window_key.clone(),
      used_percent: reading.used_percent,
      window_minutes: reading.window_minutes,
      resets_at: resets,
      sampled_at: now_text.clone(),
    })?;
    written += 1;
  }
  if written > 0 {
    prune(conn, now)?;
  }
  Ok(written)
}

/// Grava uma leitura; outra leitura da mesma janela no mesmo segundo é substituída.
fn upsert(conn: &rusqlite::Connection, sample: &UsageSample) -> rusqlite::Result<usize> {
  conn.execute(
    "INSERT INTO usage_samples (agent, window_key, used_percent, window_minutes, resets_at, sampled_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
     ON CONFLICT(agent, window_key, sampled_at) DO UPDATE SET
       used_percent = excluded.used_percent, window_minutes = excluded.window_minutes, resets_at = excluded.resets_at",
    params![sample.agent, sample.window_key, sample.used_percent, sample.window_minutes, sample.resets_at, sample.sampled_at],
  )
}

fn prune(conn: &rusqlite::Connection, now: NaiveDateTime) -> rusqlite::Result<usize> {
  let cutoff = (now - chrono::Duration::days(KEEP_DAYS)).format(DATETIME_FORMAT).to_string();
  conn.execute("DELETE FROM usage_samples WHERE sampled_at < ?1", params![cutoff])
}

// ---------- Sincronização (só do PC para o celular) ----------

/// Na primeira sincronização o celular recebe só este tanto de histórico.
const SYNC_FIRST_DAYS: i64 = 30;
/// Limite de leituras por resposta; o resto vem nas próximas sincronizações.
const SYNC_MAX_ROWS: i64 = 5000;

/// Instante da leitura mais recente deste aparelho; o celular manda para o PC pedir só as novas.
pub fn latest_sampled_at(db: &Database) -> Result<Option<String>> {
  Ok(db.conn().query_row("SELECT MAX(sampled_at) FROM usage_samples", [], |r| r.get(0))?)
}

/// Leituras para um celular que já tem até `since` (inclusive: o celular ignora as repetidas).
/// Sem `since`, os últimos 30 dias. Em ordem de tempo, no máximo 5000.
pub fn samples_for_sync(db: &Database, since: Option<&str>, now: NaiveDateTime) -> Result<Vec<UsageSample>> {
  let first = (now - chrono::Duration::days(SYNC_FIRST_DAYS)).format(DATETIME_FORMAT).to_string();
  let since = since.filter(|s| !s.is_empty()).map(str::to_string).unwrap_or(first);
  let mut stmt = db.conn().prepare(
    "SELECT agent, window_key, used_percent, window_minutes, resets_at, sampled_at FROM usage_samples
     WHERE sampled_at >= ?1 ORDER BY sampled_at, sample_id LIMIT ?2",
  )?;
  let rows = stmt.query_map(params![since, SYNC_MAX_ROWS], |r| Ok(UsageSample {
    agent: r.get(0)?,
    window_key: r.get(1)?,
    used_percent: r.get(2)?,
    window_minutes: r.get(3)?,
    resets_at: r.get(4)?,
    sampled_at: r.get(5)?,
  }))?.collect::<rusqlite::Result<Vec<_>>>()?;
  Ok(rows)
}

/// Grava as leituras recebidas do PC. Devolve quantas chegaram.
pub fn import(conn: &rusqlite::Connection, samples: &[UsageSample], now: NaiveDateTime) -> Result<usize> {
  for sample in samples {
    upsert(conn, sample)?;
  }
  if !samples.is_empty() {
    prune(conn, now)?;
  }
  Ok(samples.len())
}

/// Leituras desde `since` (todas, se `None`), em ordem de agente, janela e tempo.
pub fn samples(db: &Database, since: Option<NaiveDateTime>) -> Result<Vec<UsageSample>> {
  let since = since.map(|t| t.format(DATETIME_FORMAT).to_string()).unwrap_or_default();
  let mut stmt = db.conn().prepare(
    "SELECT agent, window_key, used_percent, window_minutes, resets_at, sampled_at FROM usage_samples
     WHERE sampled_at >= ?1 ORDER BY agent, window_key, sampled_at, sample_id",
  )?;
  let rows = stmt.query_map(params![since], |r| Ok(UsageSample {
    agent: r.get(0)?,
    window_key: r.get(1)?,
    used_percent: r.get(2)?,
    window_minutes: r.get(3)?,
    resets_at: r.get(4)?,
    sampled_at: r.get(5)?,
  }))?.collect::<rusqlite::Result<Vec<_>>>()?;
  Ok(rows)
}

/// Última leitura de cada janela de cada agente.
pub fn latest(db: &Database) -> Result<Vec<UsageSample>> {
  let mut stmt = db.conn().prepare(
    "SELECT agent, window_key, used_percent, window_minutes, resets_at, sampled_at FROM usage_samples s
     WHERE sample_id = (SELECT sample_id FROM usage_samples t WHERE t.agent = s.agent AND t.window_key = s.window_key
                        ORDER BY sampled_at DESC, sample_id DESC LIMIT 1)
     ORDER BY agent, COALESCE(window_minutes, 1e18), window_key",
  )?;
  let rows = stmt.query_map([], |r| Ok(UsageSample {
    agent: r.get(0)?,
    window_key: r.get(1)?,
    used_percent: r.get(2)?,
    window_minutes: r.get(3)?,
    resets_at: r.get(4)?,
    sampled_at: r.get(5)?,
  }))?.collect::<rusqlite::Result<Vec<_>>>()?;
  Ok(rows)
}

/// Nome curto de uma janela pelo tamanho: `5h`, `7d`, `30d`; sem tamanho, a própria chave.
pub fn window_label(window_key: &str, window_minutes: Option<i64>) -> String {
  let base = match window_minutes {
    Some(m) if m % (24 * 60) == 0 => format!("{}d", m / (24 * 60)),
    Some(m) if m % 60 == 0 => format!("{}h", m / 60),
    Some(m) => format!("{m}min"),
    None => return window_key.to_string(),
  };
  // Janelas por modelo do Claude (`seven_day_opus`) ou limites extras do Codex (`x:primary`).
  match window_key.split_once(':').map(|(id, _)| id).or_else(|| window_key.strip_prefix("seven_day_")) {
    Some(extra) => format!("{base} {extra}"),
    None => base,
  }
}

/// Texto da statusline: `5h 23% · 7d 41%`.
pub fn statusline_text(readings: &[Reading]) -> String {
  readings.iter()
    .map(|r| format!("{} {}%", window_label(&r.window_key, r.window_minutes), r.used_percent.round()))
    .collect::<Vec<_>>()
    .join(" · ")
}

/// Pergunta a cota ao Codex: sobe `codex app-server`, faz o `initialize` e chama
/// `account/rateLimits/read`. Devolve o `result` da resposta.
pub fn fetch_codex() -> Result<Value> {
  let mut command = codex_command();
  command.args(["app-server"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
  let mut child = command.spawn().context("Não consegui rodar o `codex` (ele está no PATH?)")?;

  let mut stdin = child.stdin.take().context("sem stdin do codex")?;
  let stdout = child.stdout.take().context("sem stdout do codex")?;
  let (tx, rx) = mpsc::channel::<String>();
  std::thread::spawn(move || {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
      if tx.send(line).is_err() {
        break;
      }
    }
  });

  let send = |stdin: &mut std::process::ChildStdin, message: Value| -> Result<()> {
    writeln!(stdin, "{message}")?;
    stdin.flush()?;
    Ok(())
  };
  let wait_for = |id: i64| -> Result<Value> {
    let deadline = std::time::Instant::now() + CODEX_TIMEOUT;
    loop {
      let left = deadline.saturating_duration_since(std::time::Instant::now());
      let line = rx.recv_timeout(left).map_err(|_| anyhow::anyhow!("o codex não respondeu a tempo"))?;
      let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
      if message.get("id").and_then(Value::as_i64) != Some(id) {
        continue; // notificações
      }
      if let Some(error) = message.get("error") {
        bail!("o codex recusou o pedido: {error}");
      }
      return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
  };

  let result = (|| {
    send(&mut stdin, json!({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "arca", "title": "Arca", "version": env!("CARGO_PKG_VERSION")}}}))?;
    wait_for(1)?;
    send(&mut stdin, json!({"method": "initialized"}))?;
    send(&mut stdin, json!({"id": 2, "method": "account/rateLimits/read"}))?;
    wait_for(2)
  })();

  // Sem stdin o app-server termina sozinho; se não terminar, é encerrado.
  drop(stdin);
  std::thread::sleep(Duration::from_millis(200));
  if child.try_wait().ok().flatten().is_none() {
    let _ = child.kill();
  }
  let _ = child.wait();
  result
}

/// Lê a cota do Codex e grava. Devolve as leituras.
pub fn collect_codex(db: &Database) -> Result<Vec<Reading>> {
  let readings = parse_codex_rate_limits(&fetch_codex()?);
  record(db, CODEX, &readings, dates::now())?;
  Ok(readings)
}

#[cfg(windows)]
fn codex_command() -> Command {
  use std::os::windows::process::CommandExt;
  const CREATE_NO_WINDOW: u32 = 0x0800_0000;
  // O `codex` do npm é um `.cmd`, que só roda pelo cmd.
  let mut command = Command::new("cmd");
  command.args(["/D", "/C", "codex"]).creation_flags(CREATE_NO_WINDOW);
  command
}

#[cfg(not(windows))]
fn codex_command() -> Command {
  Command::new("codex")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn at(s: &str) -> NaiveDateTime {
    dates::parse_datetime(s).unwrap()
  }

  #[test]
  fn parses_claude_statusline() {
    let input = json!({
      "model": {"display_name": "Opus"},
      "rate_limits": {
        "seven_day": {"used_percentage": 41.2, "resets_at": 1_800_000_000},
        "five_hour": {"used_percentage": 23, "resets_at": 1_800_000_000},
        "broken": {"resets_at": 1}
      }
    });
    let readings = parse_claude_statusline(&input);
    let keys: Vec<_> = readings.iter().map(|r| r.window_key.as_str()).collect();
    assert_eq!(keys, ["five_hour", "seven_day"]);
    assert_eq!(readings[0].window_minutes, Some(300));
    assert!(readings[0].resets_at.is_some());
    assert_eq!(statusline_text(&readings), "5h 23% · 7d 41%");
    assert!(parse_claude_statusline(&json!({"model": {}})).is_empty());
  }

  #[test]
  fn parses_codex_rate_limits() {
    let result = json!({
      "rateLimits": {"limitId": "codex", "primary": {"usedPercent": 9, "windowDurationMins": 43200, "resetsAt": 1_796_064_864}, "secondary": null},
      "rateLimitsByLimitId": {
        "codex": {"limitId": "codex", "primary": {"usedPercent": 9, "windowDurationMins": 43200, "resetsAt": 1_796_064_864}, "secondary": null},
        "extra": {"limitId": "extra", "primary": {"usedPercent": 50, "windowDurationMins": 300, "resetsAt": null}}
      }
    });
    let mut readings = parse_codex_rate_limits(&result);
    readings.sort_by(|a, b| a.window_key.cmp(&b.window_key));
    assert_eq!(readings.len(), 2);
    assert_eq!(readings[0].window_key, "extra:primary");
    assert_eq!(window_label(&readings[0].window_key, readings[0].window_minutes), "5h extra");
    assert_eq!(readings[1].window_key, "primary");
    assert_eq!(window_label(&readings[1].window_key, readings[1].window_minutes), "30d");
  }

  #[test]
  fn records_only_changes_or_stale_repeats() {
    let db = Database::open_in_memory().unwrap();
    let reading = |used: f64| Reading { window_key: "five_hour".into(), used_percent: used, window_minutes: Some(300), resets_at: None };

    assert_eq!(record(&db, CLAUDE_CODE, &[reading(10.0)], at("2026-10-31 10:00:00")).unwrap(), 1);
    assert_eq!(record(&db, CLAUDE_CODE, &[reading(10.0)], at("2026-10-31 10:05:00")).unwrap(), 0);
    assert_eq!(record(&db, CLAUDE_CODE, &[reading(12.0)], at("2026-10-31 10:06:00")).unwrap(), 1);
    assert_eq!(record(&db, CLAUDE_CODE, &[reading(12.0)], at("2026-10-31 10:40:00")).unwrap(), 1);
    // Mesmo valor, outro agente: entra.
    assert_eq!(record(&db, CODEX, &[reading(12.0)], at("2026-10-31 10:41:00")).unwrap(), 1);

    assert_eq!(samples(&db, None).unwrap().len(), 4);
    assert_eq!(samples(&db, Some(at("2026-10-31 10:30:00"))).unwrap().len(), 2);
    let latest = latest(&db).unwrap();
    assert_eq!(latest.len(), 2);
    assert_eq!(latest.iter().find(|s| s.agent == CLAUDE_CODE).unwrap().sampled_at, "2026-10-31 10:40:00");
  }

  #[test]
  fn sync_sends_new_samples_and_import_ignores_repeats() {
    let pc = Database::open_in_memory().unwrap();
    let phone = Database::open_in_memory().unwrap();
    let reading = |used: f64| Reading { window_key: "five_hour".into(), used_percent: used, window_minutes: Some(300), resets_at: None };
    let now = at("2026-10-31 12:00:00");
    record(&pc, CLAUDE_CODE, &[reading(10.0)], at("2026-09-01 10:00:00")).unwrap(); // antes dos 30 dias
    record(&pc, CLAUDE_CODE, &[reading(20.0)], at("2026-10-30 10:00:00")).unwrap();

    let first = samples_for_sync(&pc, None, now).unwrap();
    assert_eq!(first.len(), 1);
    import(phone.conn(), &first, now).unwrap();
    let since = latest_sampled_at(&phone).unwrap();
    assert_eq!(since.as_deref(), Some("2026-10-30 10:00:00"));

    record(&pc, CLAUDE_CODE, &[reading(30.0)], at("2026-10-31 11:00:00")).unwrap();
    let next = samples_for_sync(&pc, since.as_deref(), now).unwrap();
    assert_eq!(next.len(), 2); // a repetida (inclusive) e a nova
    import(phone.conn(), &next, now).unwrap();
    assert_eq!(samples(&phone, None).unwrap().len(), 2);
  }

  #[test]
  fn prunes_old_samples() {
    let db = Database::open_in_memory().unwrap();
    let reading = |used: f64| Reading { window_key: "seven_day".into(), used_percent: used, window_minutes: None, resets_at: None };
    record(&db, CLAUDE_CODE, &[reading(1.0)], at("2026-01-01 00:00:00")).unwrap();
    record(&db, CLAUDE_CODE, &[reading(2.0)], at("2026-10-31 00:00:00")).unwrap();
    assert_eq!(samples(&db, None).unwrap().len(), 1);
  }
}
