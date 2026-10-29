pub mod persistence;
pub mod render;
pub mod db_path_solver;
pub mod dates;
pub mod paths;
pub mod policy;
pub mod mcp;

use anyhow::{bail, Context, Result};
use chrono::{Duration, NaiveDateTime};
use colored::Colorize;
use std::io::{IsTerminal, Read};

use crate::persistence::{
  Authors, Database, NewTask, Policy, Session, SessionState, Status, Task, TaskEdit, TaskFilter,
  AGENT_PREFIX, META_ACTIVE_WINDOW, USER,
};
use crate::render::ListStyle;

pub const DEFAULT_ACTIVE_WINDOW_MINUTES: i64 = 10;

/// Como o usuário aponta para uma tarefa: posição na listagem ou id.
#[derive(Clone, Copy)]
pub enum Target {
  Position(usize),
  Id(i64),
}

/// Em qual lista a posição é contada.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
  /// Carrinho (inclui as concluídas, como no `list`).
  Cart,
  Shelf,
}

fn in_place(task: &Task, place: Place) -> bool {
  match place {
    Place::Shelf => task.status == Status::Shelf,
    Place::Cart => task.status != Status::Shelf,
  }
}

fn optional_project_id(db: &Database, project: Option<&str>) -> Result<Option<i64>> {
  project.map(|name| db.project_id(name)).transpose()
}

/// Resolve a tarefa apontada. Posições seguem a mesma ordem e o mesmo filtro do `list`.
fn resolve(db: &Database, target: Target, place: Place, project: Option<&str>) -> Result<Task> {
  match target {
    Target::Id(id) => db.get_task(id),
    Target::Position(position) => {
      let filter = TaskFilter { project_id: optional_project_id(db, project)?, authors: Authors::User };
      let tasks: Vec<Task> = db.get_tasks(filter)?.into_iter().filter(|t| in_place(t, place)).collect();
      let name = if place == Place::Shelf { "da estante" } else { "do carrinho" };
      tasks.get(position).cloned().with_context(|| {
        format!("Não há tarefa na posição [{position}] {name} ({} tarefa(s)). Veja `arca list`.", tasks.len())
      })
    }
  }
}

/// `"none"` limpa o valor; qualquer outra coisa é interpretada por `parse`.
fn clearable<T>(value: Option<&str>, parse: impl FnOnce(&str) -> Result<T>) -> Result<Option<Option<T>>> {
  match value {
    None => Ok(None),
    Some(v) if v.eq_ignore_ascii_case("none") => Ok(Some(None)),
    Some(v) => Ok(Some(Some(parse(v)?))),
  }
}

pub struct ListOptions<'a> {
  pub shelf: bool,
  pub cart: bool,
  pub id: bool,
  pub project: Option<&'a str>,
  pub authors: Authors,
}

pub fn list(db: &Database, options: ListOptions) -> Result<()> {
  let project_id = optional_project_id(db, options.project)?;
  if let Some(id) = project_id {
    println!("{}\n", render::project_summary(&db.get_project(id)?));
  }

  let tasks = db.get_tasks(TaskFilter { project_id, authors: options.authors })?;
  let (shelf, cart): (Vec<Task>, Vec<Task>) = tasks.into_iter().partition(|t| t.status == Status::Shelf);

  let style = ListStyle {
    // Posições só valem para a visão do usuário; nas visões com agentes mostramos ids.
    show_id: options.id || options.authors != Authors::User,
    show_project: project_id.is_none(),
    show_author: options.authors != Authors::User,
    today: dates::today(),
  };

  // Sem flags (ou com as duas) mostra tudo, como no Arca original.
  match (options.cart, options.shelf) {
    (true, false) => render::render_cart(&cart, &style),
    (false, true) => render::render_shelf(&shelf, &style),
    _ => render::render_all(&cart, &shelf, &style),
  }
  Ok(())
}

pub struct AddOptions<'a> {
  pub title: &'a str,
  pub daily: bool,
  pub shelf: bool,
  pub due: Option<&'a str>,
  pub project: Option<&'a str>,
}

pub fn add(db: &Database, options: AddOptions) -> Result<()> {
  let due_date = options.due.map(|d| dates::parse_due(d, dates::today())).transpose()?;
  let project_id = optional_project_id(db, options.project)?;
  let id = db.add_task(NewTask {
    title: options.title,
    daily: options.daily,
    status: if options.shelf { Status::Shelf } else { Status::Cart },
    due_date,
    project_id,
    created_by: USER,
  })?;
  println!("Tarefa #{id} adicionada {}.", if options.shelf { "à estante" } else { "ao carrinho" });
  Ok(())
}

pub fn done(db: &Database, target: Target, project: Option<&str>) -> Result<()> {
  let task = resolve(db, target, Place::Cart, project)?;
  db.change_task_status(task.id, Status::Done)?;
  println!("{} {}", "Feita:".green(), task.title);
  Ok(())
}

pub struct EditOptions<'a> {
  pub title: Option<&'a str>,
  pub daily: Option<bool>,
  pub due: Option<&'a str>,
  pub project: Option<&'a str>,
}

pub fn edit(db: &Database, id: i64, options: EditOptions) -> Result<()> {
  let today = dates::today();
  let edit = TaskEdit {
    title: options.title,
    daily: options.daily,
    due_date: clearable(options.due, |d| dates::parse_due(d, today))?,
    project_id: clearable(options.project, |p| db.project_id(p))?,
  };
  db.edit_task(id, edit)?;
  println!("Tarefa #{id} atualizada.");
  Ok(())
}

pub fn delete(db: &Database, id: i64) -> Result<()> {
  let task = db.get_task(id)?;
  db.delete_task(id)?;
  println!("Apagada: {}", task.title);
  Ok(())
}

pub fn take(db: &Database, target: Target, project: Option<&str>) -> Result<()> {
  let task = resolve(db, target, Place::Shelf, project)?;
  db.change_task_status(task.id, Status::Cart)?;
  println!("No carrinho: {}", task.title);
  Ok(())
}

pub fn putback(db: &Database, target: Target, project: Option<&str>) -> Result<()> {
  let task = resolve(db, target, Place::Cart, project)?;
  db.change_task_status(task.id, Status::Shelf)?;
  println!("De volta à estante: {}", task.title);
  Ok(())
}

/// Legado: a virada do dia agora é automática. `check` força o arquivamento imediato
/// de tudo que está concluído, inclusive o que foi concluído hoje (comportamento original).
pub fn check(db: &Database) -> Result<()> {
  let archived = db.archive_done_before(dates::today() + Duration::days(1))?;
  println!("Cart checked ({archived} archived).");
  eprintln!("{}", "`arca check` é legado: a virada do dia agora acontece sozinha no primeiro comando do dia.".dimmed());
  Ok(())
}

pub fn history(db: &Database) -> Result<()> {
  render::render_history(&db.get_history()?);
  Ok(())
}

// ---------- Projetos ----------

pub fn project_add(db: &Database, name: &str, policy: Policy) -> Result<()> {
  if name.trim().is_empty() || name.eq_ignore_ascii_case("none") {
    bail!("Nome de projeto inválido: '{name}'");
  }
  db.add_project(name, policy)?;
  println!("Projeto '{name}' criado (política {}).", policy.as_str());
  Ok(())
}

pub fn project_list(db: &Database) -> Result<()> {
  render::render_projects(&db.get_projects()?);
  Ok(())
}

pub fn project_rename(db: &Database, name: &str, new_name: &str) -> Result<()> {
  if new_name.trim().is_empty() || new_name.eq_ignore_ascii_case("none") {
    bail!("Nome de projeto inválido: '{new_name}'");
  }
  let id = db.project_id(name)?;
  db.rename_project(id, new_name)?;
  println!("Projeto '{name}' renomeado para '{new_name}'.");
  Ok(())
}

pub fn project_delete(db: &Database, name: &str) -> Result<()> {
  let id = db.project_id(name)?;
  db.delete_project(id)?;
  println!("Projeto '{name}' apagado. As tarefas dele ficaram sem projeto.");
  Ok(())
}

pub fn project_policy(db: &Database, name: &str, policy: Policy) -> Result<()> {
  let id = db.project_id(name)?;
  db.set_project_policy(id, policy)?;
  println!("Política de agentes de '{name}': {}.", policy.as_str());
  Ok(())
}

fn folder_or_cwd(folder: Option<&str>) -> Result<String> {
  match folder {
    Some(f) => paths::normalize(f),
    None => paths::normalize(&std::env::current_dir()?.to_string_lossy()),
  }
}

pub fn project_link(db: &Database, name: &str, folder: Option<&str>) -> Result<()> {
  let id = db.project_id(name)?;
  let folder = folder_or_cwd(folder)?;
  if db.link_folder(id, &folder)? {
    println!("{folder} vinculada a '{name}'.");
  } else {
    println!("{folder} já estava vinculada a '{name}'.");
  }
  Ok(())
}

pub fn project_unlink(db: &Database, folder: Option<&str>) -> Result<()> {
  let folder = folder_or_cwd(folder)?;
  db.unlink_folder(&folder)?;
  println!("{folder} desvinculada.");
  Ok(())
}

/// Mostra o projeto de uma pasta (padrão: a atual), pela pasta vinculada mais específica.
pub fn project_which(db: &Database, folder: Option<&str>) -> Result<()> {
  let folder = folder_or_cwd(folder)?;
  match db.project_for_path(&folder)? {
    Some(id) => println!("{}", render::project_summary(&db.get_project(id)?)),
    None => println!("{}", format!("Nenhum projeto vinculado a {folder}").dimmed()),
  }
  Ok(())
}

// ---------- Agentes ----------

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
    "SessionStart" | "UserPromptSubmit" => Some(SessionState::Working),
    "Stop" | "Notification" => Some(SessionState::Waiting),
    "SessionEnd" => Some(SessionState::Ended),
    _ => None,
  }
}

pub struct PingOptions<'a> {
  pub state: Option<SessionState>,
  pub agent: Option<&'a str>,
  pub session: Option<&'a str>,
  pub cwd: Option<&'a str>,
}

/// Registra um sinal de vida de uma sessão de agente. Pensado para hooks:
/// lê o JSON do hook no stdin (`session_id`, `cwd`, `hook_event_name`) e não escreve nada no stdout.
pub fn ping(db: &Database, options: PingOptions) -> Result<()> {
  let input = read_hook_input()?;
  let field = |name: &str| input.get(name).and_then(|v| v.as_str()).map(str::to_string);

  let state = match options.state {
    Some(state) => state,
    None => match field("hook_event_name") {
      Some(event) => match state_for_hook_event(&event) {
        Some(state) => state,
        // Evento que não muda o estado (ex.: PreToolUse): ignora em silêncio.
        None => return Ok(()),
      },
      None => bail!("ping sem --state e sem hook_event_name no stdin"),
    },
  };

  let agent = agent_name(options.agent).unwrap_or_else(|| "unknown".to_string());
  let cwd = match options.cwd.map(str::to_string).or_else(|| field("cwd")) {
    Some(cwd) => paths::normalize(&cwd)?,
    None => paths::normalize(&std::env::current_dir()?.to_string_lossy())?,
  };
  let session_id = options.session.map(str::to_string)
    .or_else(|| field("session_id"))
    .unwrap_or_else(|| format!("manual:{agent}:{cwd}"));

  let project_id = db.project_for_path(&cwd)?;
  db.upsert_session(&session_id, &agent, project_id, &cwd, state)?;
  db.prune_sessions(30)?;
  Ok(())
}

fn read_hook_input() -> Result<serde_json::Value> {
  let stdin = std::io::stdin();
  if stdin.is_terminal() {
    return Ok(serde_json::Value::Null);
  }
  let mut raw = String::new();
  stdin.lock().take(1024 * 1024).read_to_string(&mut raw)?;
  if raw.trim().is_empty() {
    return Ok(serde_json::Value::Null);
  }
  serde_json::from_str(&raw).context("stdin do hook não é JSON válido")
}

pub fn active_window(db: &Database, override_minutes: Option<i64>) -> Result<i64> {
  if let Some(minutes) = override_minutes {
    return Ok(minutes);
  }
  Ok(db.get_meta(META_ACTIVE_WINDOW)?
    .and_then(|v| v.parse().ok())
    .unwrap_or(DEFAULT_ACTIVE_WINDOW_MINUTES))
}

/// Sessões abertas e as encerradas nas últimas 24h (ou todas com `all`).
fn recent_sessions(db: &Database, all: bool, now: NaiveDateTime) -> Result<Vec<Session>> {
  Ok(db.get_sessions()?.into_iter()
    .filter(|s| {
      all || s.state != SessionState::Ended
        || dates::parse_datetime(&s.last_seen_at).is_some_and(|t| (now - t).num_hours() < 24)
    })
    .collect())
}

/// Painel de atividade dos agentes por projeto.
pub fn agents(db: &Database, all: bool, window: Option<i64>) -> Result<()> {
  let window = active_window(db, window)?;
  let now = dates::now();
  render::render_sessions(&recent_sessions(db, all, now)?, now, window);
  Ok(())
}

/// Largura do terminal: `--width`, senão a detectada, senão `COLUMNS`, senão 100.
fn terminal_width(explicit: Option<usize>) -> usize {
  explicit
    .or_else(|| terminal_size::terminal_size().map(|(w, _)| w.0 as usize))
    .or_else(|| std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()))
    .unwrap_or(100)
}

pub struct BoardOptions<'a> {
  pub project: Option<&'a str>,
  pub authors: Authors,
  pub width: Option<usize>,
  pub window: Option<i64>,
}

/// Visão geral: Estante | Carrinho | Feito (hoje) e, abaixo, o painel de agentes.
pub fn board(db: &Database, options: BoardOptions) -> Result<()> {
  let project = optional_project_id(db, options.project)?.map(|id| db.get_project(id)).transpose()?;
  if let Some(project) = &project {
    println!("{}\n", render::project_summary(project));
  }

  let (mut shelf, mut cart, mut done) = (Vec::new(), Vec::new(), Vec::new());
  let filter = TaskFilter { project_id: project.as_ref().map(|p| p.id), authors: options.authors };
  for task in db.get_tasks(filter)? {
    match task.status {
      Status::Shelf => shelf.push(task),
      Status::Cart => cart.push(task),
      // Concluídas de dias anteriores já foram arquivadas pela virada do dia.
      Status::Done => done.push(task),
    }
  }

  let style = ListStyle {
    show_id: true,
    show_project: project.is_none(),
    show_author: options.authors != Authors::User,
    today: dates::today(),
  };
  render::render_board(&shelf, &cart, &done, &style, terminal_width(options.width));

  let window = active_window(db, options.window)?;
  let now = dates::now();
  let sessions: Vec<_> = recent_sessions(db, false, now)?.into_iter()
    .filter(|s| project.as_ref().is_none_or(|p| s.project_name.as_deref() == Some(p.name.as_str())))
    .collect();
  if !sessions.is_empty() {
    println!("\n{}", "Agentes".bold());
    render::render_sessions(&sessions, now, window);
  }
  Ok(())
}

pub fn config_active_window(db: &Database, minutes: Option<i64>) -> Result<()> {
  match minutes {
    Some(m) if m <= 0 => bail!("A janela precisa ser maior que zero"),
    Some(m) => {
      db.set_meta(META_ACTIVE_WINDOW, &m.to_string())?;
      println!("active-window = {m} min");
    }
    None => println!("active-window = {} min", active_window(db, None)?),
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn maps_hook_events() {
    assert_eq!(state_for_hook_event("SessionStart"), Some(SessionState::Working));
    assert_eq!(state_for_hook_event("UserPromptSubmit"), Some(SessionState::Working));
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
  fn positions_follow_the_user_view() {
    let db = Database::open_in_memory().unwrap();
    for (title, status, by) in [
      ("a", Status::Cart, USER), ("agente", Status::Cart, "agent:x"), ("b", Status::Shelf, USER), ("c", Status::Cart, USER),
    ] {
      db.add_task(NewTask { title, daily: false, status, due_date: None, project_id: None, created_by: by }).unwrap();
    }
    assert_eq!(resolve(&db, Target::Position(1), Place::Cart, None).unwrap().title, "c");
    assert_eq!(resolve(&db, Target::Position(0), Place::Shelf, None).unwrap().title, "b");
    assert!(resolve(&db, Target::Position(2), Place::Cart, None).is_err());
  }
}
