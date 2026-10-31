//! API de dados para os apps (Tauri e Flutter). Cada função recebe tipos simples e devolve
//! estruturas serializáveis, já com os textos humanizados que a interface mostra.
//! As ações são sempre as do usuário (sem política de agentes).

use anyhow::Result;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::agents::{self, Activity, ActiveWindow};
use crate::dates;
use crate::persistence::{
  Authors, Database, HistoryEntry, NewTask, Policy, Project, Session, Status, Suggestion, Task, TaskEdit,
  TaskFilter, AGENT_PREFIX, USER,
};
use crate::suggestions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DueState {
  Overdue,
  Today,
  Upcoming,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
  pub id: i64,
  pub title: String,
  /// `SHELF`, `CART` ou `DONE`.
  pub status: String,
  pub daily: bool,
  pub due_date: Option<String>,
  /// "em 3 dias", "hoje", "passou há 2 dias".
  pub due_label: Option<String>,
  pub due_state: Option<DueState>,
  pub project: Option<String>,
  pub created_by: String,
  /// Nome do agente que criou a tarefa, se não foi o usuário.
  pub agent: Option<String>,
  /// Agentes com sessão aberta trabalhando nesta tarefa agora.
  pub working_agents: Vec<String>,
  /// Hora da conclusão ("21:40"), só nas concluídas.
  pub done_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
  pub session_id: String,
  pub agent: String,
  pub project: Option<String>,
  pub cwd: String,
  pub activity: Activity,
  /// "trabalhando", "possivelmente parada", "esperando você", "encerrada".
  pub activity_label: String,
  /// "há 5 min".
  pub last_seen_label: String,
  pub task_id: Option<i64>,
  pub task_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestionView {
  pub id: i64,
  pub task_id: i64,
  pub task_title: String,
  pub agent: String,
  /// "concluir", "mover para a estante", "editar (...)".
  pub summary: String,
  pub reason: Option<String>,
  pub created_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
  pub id: i64,
  pub name: String,
  /// `NONE`, `OWN` ou `ALL`.
  pub policy: String,
  pub active_window: Option<i64>,
  pub folders: Vec<String>,
  pub shelf: u32,
  pub cart: u32,
  pub done: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryView {
  pub id: i64,
  pub title: String,
  pub daily: bool,
  pub date: String,
  pub due_date: Option<String>,
  pub project: Option<String>,
  pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Board {
  pub today: String,
  pub shelf: Vec<TaskView>,
  pub cart: Vec<TaskView>,
  pub done: Vec<TaskView>,
  pub sessions: Vec<SessionView>,
  pub suggestions: Vec<SuggestionView>,
  pub projects: Vec<ProjectView>,
  /// Minutos até "trabalhando" virar "possivelmente parada" (janela global).
  pub window_minutes: i64,
}

/// Filtro do board: projeto (por nome) e quais autores mostrar.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BoardFilter {
  pub project: Option<String>,
  /// `user` (padrão), `all` ou `agents`.
  pub authors: Option<String>,
}

fn agent_of(created_by: &str) -> Option<String> {
  (created_by != USER).then(|| created_by.strip_prefix(AGENT_PREFIX).unwrap_or(created_by).to_string())
}

fn task_view(task: &Task, today: NaiveDate, sessions: &[Session]) -> TaskView {
  let due_state = task.due_date.map(|d| match d.cmp(&today) {
    std::cmp::Ordering::Less => DueState::Overdue,
    std::cmp::Ordering::Equal => DueState::Today,
    std::cmp::Ordering::Greater => DueState::Upcoming,
  });
  TaskView {
    id: task.id,
    title: task.title.clone(),
    status: task.status.as_str().to_string(),
    daily: task.daily,
    due_date: task.due_date.map(|d| d.to_string()),
    due_label: task.due_date.map(|d| dates::humanize_due(d, today)),
    due_state,
    project: task.project_name.clone(),
    created_by: task.created_by.clone(),
    agent: agent_of(&task.created_by),
    working_agents: sessions.iter()
      .filter(|s| s.task_id == Some(task.id) && s.state != crate::persistence::SessionState::Ended)
      .map(|s| s.agent.clone())
      .collect(),
    done_time: task.done_at.as_deref().and_then(dates::parse_datetime).map(|t| t.format("%H:%M").to_string()),
  }
}

fn activity_label(activity: Activity) -> &'static str {
  match activity {
    Activity::Working => "trabalhando",
    Activity::PossiblyStopped => "possivelmente parada",
    Activity::Waiting => "esperando você",
    Activity::Ended => "encerrada",
  }
}

fn ago(datetime: &str, now: chrono::NaiveDateTime) -> String {
  dates::parse_datetime(datetime).map(|t| dates::humanize_ago(t, now)).unwrap_or_else(|| datetime.to_string())
}

fn session_view(session: &Session, now: chrono::NaiveDateTime, window: ActiveWindow) -> SessionView {
  let activity = agents::activity(session, now, window);
  SessionView {
    session_id: session.session_id.clone(),
    agent: session.agent.clone(),
    project: session.project_name.clone(),
    cwd: session.cwd.clone(),
    activity,
    activity_label: activity_label(activity).to_string(),
    last_seen_label: ago(&session.last_seen_at, now),
    task_id: session.task_id,
    task_title: session.task_title.clone(),
  }
}

fn suggestion_view(suggestion: &Suggestion, now: chrono::NaiveDateTime) -> SuggestionView {
  SuggestionView {
    id: suggestion.id,
    task_id: suggestion.task_id,
    task_title: suggestion.task_title.clone(),
    agent: agent_of(&suggestion.created_by).unwrap_or_else(|| suggestion.created_by.clone()),
    summary: suggestions::describe(suggestion),
    reason: suggestion.reason.clone(),
    created_label: ago(&suggestion.created_at, now),
  }
}

fn project_view(project: &Project) -> ProjectView {
  ProjectView {
    id: project.id,
    name: project.name.clone(),
    policy: project.policy.as_str().to_string(),
    active_window: project.active_window,
    folders: project.folders.clone(),
    shelf: project.shelf,
    cart: project.cart,
    done: project.done,
  }
}

fn parse_authors(value: Option<&str>) -> Authors {
  match value {
    Some("all") => Authors::All,
    Some("agents") => Authors::Agents,
    _ => Authors::User,
  }
}

/// Tudo o que a tela principal mostra. Faz a virada do dia antes, se for preciso.
pub fn board(db: &Database, filter: &BoardFilter) -> Result<Board> {
  let today = dates::today();
  db.rollover_if_needed(today)?;
  let now = dates::now();

  let project = filter.project.as_deref().filter(|p| !p.is_empty()).map(|name| db.project_id(name)).transpose()?;
  let window = ActiveWindow::load(db, None)?;
  let all_sessions = agents::recent_sessions(db, false, now)?;

  let (mut shelf, mut cart, mut done) = (Vec::new(), Vec::new(), Vec::new());
  for task in db.get_tasks(TaskFilter { project_id: project, authors: parse_authors(filter.authors.as_deref()) })? {
    let view = task_view(&task, today, &all_sessions);
    match task.status {
      Status::Shelf => shelf.push(view),
      Status::Cart => cart.push(view),
      Status::Done => done.push(view),
    }
  }

  let project_name = project.map(|id| db.get_project(id)).transpose()?.map(|p| p.name);
  let sessions = all_sessions.iter()
    .filter(|s| project_name.is_none() || s.project_name == project_name)
    .map(|s| session_view(s, now, window))
    .collect();

  Ok(Board {
    today: today.to_string(),
    shelf,
    cart,
    done,
    sessions,
    suggestions: db.get_suggestions(true)?.iter().map(|s| suggestion_view(s, now)).collect(),
    projects: db.get_projects()?.iter().map(project_view).collect(),
    window_minutes: window.default,
  })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TaskInput {
  pub title: String,
  /// AAAA-MM-DD, hoje, amanhã, +3d.
  pub due: Option<String>,
  pub project: Option<String>,
  pub daily: Option<bool>,
  /// `SHELF` ou `CART` (padrão).
  pub status: Option<String>,
}

fn optional_project(db: &Database, name: Option<&str>) -> Result<Option<i64>> {
  name.filter(|n| !n.trim().is_empty()).map(|n| db.project_id(n)).transpose()
}

fn parse_status(value: &str) -> Result<Status> {
  match value {
    "CART" => Ok(Status::Cart),
    "SHELF" => Ok(Status::Shelf),
    "DONE" => Ok(Status::Done),
    other => anyhow::bail!("Status inválido: {other}"),
  }
}

pub fn add_task(db: &Database, input: &TaskInput) -> Result<i64> {
  let title = input.title.trim();
  if title.is_empty() {
    anyhow::bail!("A tarefa precisa de um título");
  }
  let due_date = input.due.as_deref().filter(|d| !d.trim().is_empty())
    .map(|d| dates::parse_due(d, dates::today())).transpose()?;
  db.add_task(NewTask {
    title,
    daily: input.daily.unwrap_or(false),
    status: parse_status(input.status.as_deref().unwrap_or("CART"))?,
    due_date,
    project_id: optional_project(db, input.project.as_deref())?,
    created_by: USER,
  })
}

/// Edição: campos ausentes não mudam; `due`/`project` vazios removem.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TaskChanges {
  pub title: Option<String>,
  pub due: Option<String>,
  pub project: Option<String>,
  pub daily: Option<bool>,
}

pub fn edit_task(db: &Database, id: i64, changes: &TaskChanges) -> Result<()> {
  let today = dates::today();
  let due_date = match changes.due.as_deref().map(str::trim) {
    None => None,
    Some("") => Some(None),
    Some(d) => Some(Some(dates::parse_due(d, today)?)),
  };
  let project_id = match changes.project.as_deref().map(str::trim) {
    None => None,
    Some("") => Some(None),
    Some(name) => Some(Some(db.project_id(name)?)),
  };
  db.edit_task(id, TaskEdit {
    title: changes.title.as_deref().map(str::trim).filter(|t| !t.is_empty()),
    daily: changes.daily,
    due_date,
    project_id,
  })
}

pub fn move_task(db: &Database, id: i64, status: &str) -> Result<()> {
  db.change_task_status(id, parse_status(status)?)
}

pub fn delete_task(db: &Database, id: i64) -> Result<()> {
  db.delete_task(id)
}

pub fn approve_suggestion(db: &Database, id: i64) -> Result<()> {
  suggestions::approve(db, id).map(|_| ())
}

pub fn reject_suggestion(db: &Database, id: i64) -> Result<()> {
  suggestions::reject(db, id).map(|_| ())
}

pub fn history(db: &Database) -> Result<Vec<HistoryView>> {
  let entries: Vec<HistoryEntry> = db.get_history()?;
  Ok(entries.into_iter().rev().map(|h| HistoryView {
    id: h.id,
    title: h.title,
    daily: h.daily,
    date: h.date,
    due_date: h.due_date.map(|d| d.to_string()),
    project: h.project_name,
    agent: agent_of(&h.created_by),
  }).collect())
}

/// Tamanho da estante e do carrinho no fim de um dia (só existem a partir da migration 06).
#[derive(Debug, Clone, Serialize)]
pub struct StatusSnapshotView {
  pub date: String,
  pub shelf: u32,
  pub cart: u32,
}

/// Fotos diárias em ordem de data. Um dia ausente repete o último dia anterior a ele.
pub fn status_snapshots(db: &Database) -> Result<Vec<StatusSnapshotView>> {
  Ok(db.get_status_snapshots()?.into_iter().map(|(date, shelf, cart)| StatusSnapshotView { date, shelf, cart }).collect())
}

fn parse_policy(value: &str) -> Result<Policy> {
  match value {
    "NONE" => Ok(Policy::None),
    "OWN" => Ok(Policy::Own),
    "ALL" => Ok(Policy::All),
    other => anyhow::bail!("Política inválida: {other}"),
  }
}

pub fn add_project(db: &Database, name: &str, policy: &str) -> Result<i64> {
  let name = name.trim();
  if name.is_empty() {
    anyhow::bail!("O projeto precisa de um nome");
  }
  db.add_project(name, parse_policy(policy)?)
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectChanges {
  pub name: Option<String>,
  pub policy: Option<String>,
  /// Minutos; 0 volta a usar a janela global.
  pub active_window: Option<i64>,
}

pub fn edit_project(db: &Database, id: i64, changes: &ProjectChanges) -> Result<()> {
  if let Some(name) = changes.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
    db.rename_project(id, name)?;
  }
  if let Some(policy) = changes.policy.as_deref() {
    db.set_project_policy(id, parse_policy(policy)?)?;
  }
  if let Some(minutes) = changes.active_window {
    db.set_project_window(id, (minutes > 0).then_some(minutes))?;
  }
  Ok(())
}

pub fn delete_project(db: &Database, id: i64) -> Result<()> {
  db.delete_project(id)
}

pub fn link_folder(db: &Database, project_id: i64, folder: &str) -> Result<()> {
  db.link_folder(project_id, &crate::paths::normalize(folder)?).map(|_| ())
}

pub fn unlink_folder(db: &Database, folder: &str) -> Result<()> {
  db.unlink_folder(folder)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn board_groups_and_humanizes() {
    let db = Database::open_in_memory().unwrap();
    add_project(&db, "arca", "OWN").unwrap();
    add_task(&db, &TaskInput { title: "Hoje".into(), due: Some("hoje".into()), project: Some("arca".into()), ..Default::default() }).unwrap();
    let shelf = add_task(&db, &TaskInput { title: "Depois".into(), status: Some("SHELF".into()), ..Default::default() }).unwrap();
    assert!(add_task(&db, &TaskInput { title: "  ".into(), ..Default::default() }).is_err());

    move_task(&db, shelf, "DONE").unwrap();
    let board = board(&db, &BoardFilter::default()).unwrap();
    assert_eq!(board.cart[0].due_label.as_deref(), Some("hoje"));
    assert_eq!(board.cart[0].due_state, Some(DueState::Today));
    assert_eq!(board.done[0].title, "Depois");
    assert_eq!(board.projects[0].cart, 1);

    let only_arca = super::board(&db, &BoardFilter { project: Some("arca".into()), authors: None }).unwrap();
    assert!(only_arca.done.is_empty());
  }

  #[test]
  fn edits_with_empty_strings_clear() {
    let db = Database::open_in_memory().unwrap();
    add_project(&db, "arca", "ALL").unwrap();
    let id = add_task(&db, &TaskInput { title: "x".into(), due: Some("+3d".into()), project: Some("arca".into()), ..Default::default() }).unwrap();
    edit_task(&db, id, &TaskChanges { due: Some(String::new()), project: Some(String::new()), ..Default::default() }).unwrap();
    let task = db.get_task(id).unwrap();
    assert_eq!((task.due_date, task.project_id), (None, None));

    let project = db.project_id("arca").unwrap();
    edit_project(&db, project, &ProjectChanges { active_window: Some(15), policy: Some("NONE".into()), ..Default::default() }).unwrap();
    assert_eq!(db.get_project(project).unwrap().active_window, Some(15));
    edit_project(&db, project, &ProjectChanges { active_window: Some(0), ..Default::default() }).unwrap();
    assert_eq!(db.get_project(project).unwrap().active_window, None);
  }
}
