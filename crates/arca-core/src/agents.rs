//! Identidade e atividade dos agentes: nomes, eventos de hook, janela de "ativo agora".

use anyhow::{bail, Result};
use chrono::NaiveDateTime;
use serde::Serialize;

use crate::dates;
use crate::paths;
use crate::persistence::{Database, Session, SessionState, AGENT_PREFIX, META_ACTIVE_WINDOW};

pub const DEFAULT_ACTIVE_WINDOW_MINUTES: i64 = 10;

/// Nome do agente: argumento explícito, senão `ARCA_AGENT`. Aceita com ou sem o prefixo `agent:`.
pub fn agent_name(explicit: Option<&str>) -> Option<String> {
  let raw = explicit.map(str::to_string).or_else(|| std::env::var("ARCA_AGENT").ok())?;
  let name = raw.trim();
  let name = name.strip_prefix(AGENT_PREFIX).unwrap_or(name).trim();
  if name.is_empty() { None } else { Some(name.to_string()) }
}

/// Valor de `created_by` para tarefas criadas por um agente.
pub fn agent_author(name: &str) -> String {
  format!("{AGENT_PREFIX}{name}")
}

/// Estado correspondente a cada evento de hook do Claude Code.
pub fn state_for_hook_event(event: &str) -> Option<SessionState> {
  match event {
    // PostToolUse cobre a volta ao trabalho depois de uma pergunta ou pedido de permissão,
    // que disparam Notification (WAITING) mas não UserPromptSubmit.
    "SessionStart" | "UserPromptSubmit" | "PostToolUse" => Some(SessionState::Working),
    "Stop" | "Notification" => Some(SessionState::Waiting),
    "SessionEnd" => Some(SessionState::Ended),
    _ => None,
  }
}

/// Um sinal de vida, vindo de um hook (JSON no stdin) e/ou de argumentos explícitos.
#[derive(Default)]
pub struct Ping<'a> {
  pub state: Option<SessionState>,
  pub agent: Option<&'a str>,
  pub session: Option<&'a str>,
  pub cwd: Option<&'a str>,
  /// Liga a sessão a uma tarefa (para agentes que registram atividade por script).
  pub task: Option<i64>,
}

/// Registra o sinal de vida. `hook_input` é o JSON do hook (`session_id`, `cwd`,
/// `hook_event_name`), ou `Null`. Eventos que não mudam o estado são ignorados.
pub fn record_ping(db: &Database, ping: Ping, hook_input: &serde_json::Value) -> Result<()> {
  let field = |name: &str| hook_input.get(name).and_then(|v| v.as_str()).map(str::to_string);

  let state = match ping.state {
    Some(state) => state,
    None => match field("hook_event_name") {
      Some(event) => match state_for_hook_event(&event) {
        Some(state) => state,
        None => return Ok(()),
      },
      None => bail!("ping sem --state e sem hook_event_name no stdin"),
    },
  };

  let agent = agent_name(ping.agent).unwrap_or_else(|| "unknown".to_string());
  let cwd = match ping.cwd.map(str::to_string).or_else(|| field("cwd")) {
    Some(cwd) => paths::normalize(&cwd)?,
    None => paths::normalize(&std::env::current_dir()?.to_string_lossy())?,
  };
  let session_id = ping.session.map(str::to_string)
    .or_else(|| field("session_id"))
    .unwrap_or_else(|| format!("manual:{agent}:{cwd}"));

  let project_id = db.project_for_path(&cwd)?;
  db.upsert_session(&session_id, &agent, project_id, &cwd, state)?;
  if let Some(task) = ping.task {
    db.get_task(task)?;
    db.set_session_task(&session_id, Some(task))?;
  }
  db.prune_sessions(30)?;
  Ok(())
}

/// Janela global configurada (ou a padrão de 10 min).
pub fn global_window(db: &Database) -> Result<i64> {
  Ok(db.get_meta(META_ACTIVE_WINDOW)?
    .and_then(|v| v.parse().ok())
    .unwrap_or(DEFAULT_ACTIVE_WINDOW_MINUTES))
}

pub fn set_global_window(db: &Database, minutes: i64) -> Result<()> {
  if minutes <= 0 {
    bail!("A janela precisa ser maior que zero");
  }
  db.set_meta(META_ACTIVE_WINDOW, &minutes.to_string())
}

/// Janela de "ativo agora": `forced` (ex.: `--window`) vale para tudo; senão a do projeto; senão a global.
#[derive(Clone, Copy, Debug)]
pub struct ActiveWindow {
  pub forced: Option<i64>,
  pub default: i64,
}

impl ActiveWindow {
  pub fn load(db: &Database, forced: Option<i64>) -> Result<Self> {
    Ok(Self { forced, default: global_window(db)? })
  }

  pub fn minutes_for(&self, session: &Session) -> i64 {
    self.forced.or(session.project_window).unwrap_or(self.default)
  }
}

/// Como uma sessão deve ser apresentada agora.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
  /// `WORKING` com sinal dentro da janela.
  Working,
  /// `WORKING` sem sinal além da janela.
  PossiblyStopped,
  /// Terminou e espera o usuário.
  Waiting,
  Ended,
}

pub fn activity(session: &Session, now: NaiveDateTime, window: ActiveWindow) -> Activity {
  match session.state {
    SessionState::Working => {
      let fresh = dates::parse_datetime(&session.last_seen_at)
        .is_some_and(|t| (now - t).num_minutes() < window.minutes_for(session));
      if fresh { Activity::Working } else { Activity::PossiblyStopped }
    }
    SessionState::Waiting => Activity::Waiting,
    SessionState::Ended => Activity::Ended,
  }
}

/// Sessões abertas e as encerradas nas últimas 24h (ou todas com `all`), da mais recente à mais antiga.
pub fn recent_sessions(db: &Database, all: bool, now: NaiveDateTime) -> Result<Vec<Session>> {
  Ok(db.get_sessions()?.into_iter()
    .filter(|s| {
      all || s.state != SessionState::Ended
        || dates::parse_datetime(&s.last_seen_at).is_some_and(|t| (now - t).num_hours() < 24)
    })
    .collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn maps_hook_events() {
    assert_eq!(state_for_hook_event("SessionStart"), Some(SessionState::Working));
    assert_eq!(state_for_hook_event("UserPromptSubmit"), Some(SessionState::Working));
    assert_eq!(state_for_hook_event("PostToolUse"), Some(SessionState::Working));
    assert_eq!(state_for_hook_event("Stop"), Some(SessionState::Waiting));
    assert_eq!(state_for_hook_event("Notification"), Some(SessionState::Waiting));
    assert_eq!(state_for_hook_event("SessionEnd"), Some(SessionState::Ended));
    assert_eq!(state_for_hook_event("PreToolUse"), None);
  }

  #[test]
  fn normalizes_agent_name() {
    assert_eq!(agent_name(Some("claude-code")).as_deref(), Some("claude-code"));
    assert_eq!(agent_name(Some("agent:claude-code")).as_deref(), Some("claude-code"));
    assert_eq!(agent_name(Some("  ")), None);
    assert_eq!(agent_author("claude-code"), "agent:claude-code");
  }

  #[test]
  fn records_hook_ping() {
    let db = Database::open_in_memory().unwrap();
    let input = serde_json::json!({"session_id": "s1", "cwd": "/tmp/x", "hook_event_name": "Stop"});
    record_ping(&db, Ping { agent: Some("claude-code"), ..Default::default() }, &input).unwrap();
    let sessions = db.get_sessions().unwrap();
    assert_eq!(sessions[0].state, SessionState::Waiting);

    // Evento sem estado: nada muda.
    let ignored = serde_json::json!({"session_id": "s2", "cwd": "/tmp/x", "hook_event_name": "PreToolUse"});
    record_ping(&db, Ping::default(), &ignored).unwrap();
    assert_eq!(db.get_sessions().unwrap().len(), 1);
  }

  #[test]
  fn classifies_activity_with_project_window() {
    let db = Database::open_in_memory().unwrap();
    db.upsert_session("s1", "a", None, "/x", SessionState::Working).unwrap();
    let mut session = db.get_sessions().unwrap().remove(0);
    let seen = dates::parse_datetime(&session.last_seen_at).unwrap();
    let later = seen + chrono::Duration::minutes(15);

    let window = ActiveWindow { forced: None, default: 10 };
    assert_eq!(activity(&session, seen, window), Activity::Working);
    assert_eq!(activity(&session, later, window), Activity::PossiblyStopped);

    session.project_window = Some(30);
    assert_eq!(activity(&session, later, window), Activity::Working);
    assert_eq!(activity(&session, later, ActiveWindow { forced: Some(5), default: 10 }), Activity::PossiblyStopped);
  }
}
