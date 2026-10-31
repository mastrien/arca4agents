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
  // O link do Remote Control só muda quando o usuário liga ou desliga; basta olhar nos eventos
  // principais (o PostToolUse dispara a cada ferramenta).
  let main_event = matches!(field("hook_event_name").as_deref(), Some("SessionStart" | "UserPromptSubmit" | "Stop" | "Notification"));
  if let (true, Some(transcript)) = (main_event, field("transcript_path"))
    && let Some(url) = remote_url_from_transcript(std::path::Path::new(&transcript))
  {
    db.set_session_remote_url(&session_id, Some(&url))?;
  }
  if let Some(task) = ping.task {
    db.get_task(task)?;
    db.set_session_task(&session_id, Some(task))?;
  }
  db.prune_sessions(30)?;
  Ok(())
}

/// Quanto do fim do transcript é lido atrás do link do Remote Control.
const TRANSCRIPT_TAIL: u64 = 1024 * 1024;

/// Link da sessão no Remote Control do Claude Code, lido do fim do transcript. O Claude Code grava
/// registros `bridge-session` (`"bridgeSessionId":"cse_<id>"`) e `bridge_status`
/// (`"url":"https://claude.ai/code/session_<id>"`); vale o mais recente. O formato não é
/// documentado: se mudar, a sessão só fica sem link.
pub fn remote_url_from_transcript(path: &std::path::Path) -> Option<String> {
  use std::io::{Read, Seek, SeekFrom};
  let mut file = std::fs::File::open(path).ok()?;
  let len = file.metadata().ok()?.len();
  file.seek(SeekFrom::Start(len.saturating_sub(TRANSCRIPT_TAIL))).ok()?;
  let mut bytes = Vec::new();
  file.read_to_end(&mut bytes).ok()?;
  remote_url_from_text(&String::from_utf8_lossy(&bytes))
}

fn remote_url_from_text(text: &str) -> Option<String> {
  const URL: &str = "\"url\":\"https://claude.ai/code/session_";
  const BRIDGE: &str = "\"bridgeSessionId\":\"cse_";
  let last = |marker: &str| text.rfind(marker).map(|i| (i, &text[i + marker.len()..]));
  let (_, rest) = [last(URL), last(BRIDGE)].into_iter().flatten().max_by_key(|(i, _)| *i)?;
  let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
  (!id.is_empty()).then(|| format!("https://claude.ai/code/session_{id}"))
}

/// Dica de onde veio uma sessão sem projeto, a partir do id e da pasta (já normalizada):
/// `app Claude · sessão lateral`, `~/desktop/notas`, `provavelmente nuvem · /home/user/repo`...
pub fn origin(session_id: &str, cwd: &str) -> String {
  let place = place_of(cwd);
  if session_id.starts_with("mcp:") {
    format!("{place} · pelo MCP, sem hooks")
  } else if session_id.starts_with("manual:") {
    format!("{place} · registro manual")
  } else {
    place
  }
}

fn place_of(cwd: &str) -> String {
  let cwd = cwd.trim_end_matches('/');
  if cwd.contains("/claude-data/") || cwd.contains("/appdata/roaming/claude/") {
    return if cwd.contains("side-session") { "app Claude · sessão lateral".into() } else { "app Claude".into() };
  }
  if let Some((parent, _)) = cwd.split_once("/.claude/worktrees/") {
    return format!("worktree de {}", short_path(parent));
  }
  if cwd.contains("/.codex/") {
    return "pasta interna do Codex".into();
  }
  if let Some(rest) = cwd.strip_prefix("/mnt/").filter(|r| r.chars().nth(1).is_none_or(|c| c == '/')) {
    // /mnt/c/x → WSL, pasta c:/x do Windows
    let (drive, path) = rest.split_at(1);
    return format!("WSL · {}", short_path(&format!("{drive}:{path}")));
  }
  if cwd == "/home/user" || cwd.starts_with("/home/user/") {
    // Pasta padrão das sessões do Claude Code na web.
    return format!("provavelmente nuvem · {}", short_path(cwd));
  }
  if cwd.starts_with("/tmp") || cwd.contains("/appdata/local/temp/") {
    return "pasta temporária".into();
  }
  short_path(cwd)
}

/// `c:/users/fulano` → `~`; `c:/users/fulano/desktop/notas` → `~/desktop/notas`; caminhos longos
/// ficam com o começo e as duas últimas pastas (`~/…/c/d`, `d:/…/b/c`).
fn short_path(path: &str) -> String {
  let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
  if parts.len() >= 3 && parts[0].ends_with(':') && parts[1] == "users" {
    let rest = &parts[3..];
    return match rest.len() {
      0 => "pasta do usuário (~)".into(),
      1 | 2 => format!("~/{}", rest.join("/")),
      n => format!("~/…/{}", rest[n - 2..].join("/")),
    };
  }
  let root = if path.starts_with('/') { "/" } else { "" };
  match parts.len() {
    0..=3 => format!("{root}{}", parts.join("/")),
    n => format!("{root}{}/…/{}", parts[0], parts[n - 2..].join("/")),
  }
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
  fn ping_records_remote_url_and_late_folder_link() {
    let db = Database::open_in_memory().unwrap();
    let dir = std::env::temp_dir().join(format!("arca-transcript-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let transcript = dir.join("t.jsonl");
    std::fs::write(&transcript, "{\"type\":\"bridge-session\",\"bridgeSessionId\":\"cse_01XYZ\"}
").unwrap();
    let input = serde_json::json!({
      "session_id": "s1", "cwd": "/tmp/proj/sub", "hook_event_name": "Stop",
      "transcript_path": transcript.to_string_lossy(),
    });
    record_ping(&db, Ping { agent: Some("claude-code"), ..Default::default() }, &input).unwrap();
    let session = db.get_sessions().unwrap().remove(0);
    assert_eq!(session.remote_url.as_deref(), Some("https://claude.ai/code/session_01XYZ"));
    assert_eq!(session.project_name, None);

    // Ligar a pasta depois traz a sessão para o projeto.
    let project = db.add_project("proj", crate::persistence::Policy::Own).unwrap();
    let folder = session.cwd.strip_suffix("/sub").unwrap().to_string(); // já normalizado (c:/tmp/proj no Windows)
    db.link_folder(project, &folder).unwrap();
    assert_eq!(db.get_sessions().unwrap()[0].project_name.as_deref(), Some("proj"));
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn reads_remote_url_from_transcript_tail() {
    let text = r#"{"type":"bridge-session","bridgeSessionId":"cse_01AAA"}
{"type":"system","subtype":"bridge_status","url":"https://claude.ai/code/session_01BBB"}
{"type":"bridge-session","bridgeSessionId":"cse_01CCC","lastSequenceNum":3}"#;
    assert_eq!(remote_url_from_text(text).as_deref(), Some("https://claude.ai/code/session_01CCC"));
    assert_eq!(remote_url_from_text(r#"{"url":"https://claude.ai/code/session_01BBB?x=1"}"#).as_deref(), Some("https://claude.ai/code/session_01BBB"));
    assert_eq!(remote_url_from_text("nada aqui"), None);
  }

  #[test]
  fn describes_session_origin() {
    let side = "c:/users/g51 informática/appdata/local/claude-data/claude_x/side-session-notes/local_1";
    assert_eq!(origin("uuid", side), "app Claude · sessão lateral");
    assert_eq!(origin("uuid", "c:/users/fulano"), "pasta do usuário (~)");
    assert_eq!(origin("uuid", "c:/users/fulano/desktop/notas"), "~/desktop/notas");
    assert_eq!(origin("uuid", "c:/users/fulano/a/b/c/d"), "~/…/c/d");
    assert_eq!(origin("uuid", "c:/root_lab/x"), "c:/root_lab/x");
    assert_eq!(origin("uuid", "d:/a/b/c"), "d:/…/b/c");
    assert_eq!(origin("uuid", "c:/root_lab/app/.claude/worktrees/feat-x"), "worktree de c:/root_lab/app");
    assert_eq!(origin("uuid", "/home/user/meu-repo"), "provavelmente nuvem · /home/user/meu-repo");
    assert_eq!(origin("uuid", "/srv/a/b/c"), "/srv/…/b/c");
    assert_eq!(origin("uuid", "/mnt/c/root_lab/x"), "WSL · c:/root_lab/x");
    assert_eq!(origin("uuid", "c:/users/fulano/appdata/local/temp/abc"), "pasta temporária");
    assert_eq!(origin("mcp:claude-code:12", "c:/root_lab/x"), "c:/root_lab/x · pelo MCP, sem hooks");
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
