//! API de dados para os apps (Tauri e Flutter). Cada função recebe tipos simples e devolve
//! estruturas serializáveis, já com os textos humanizados que a interface mostra.
//! As ações são sempre as do usuário (sem política de agentes).

use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::agents::{self, Activity, ActiveWindow};
use crate::dates;
use crate::persistence::{
  Authors, Birthday, BirthdayEdit, Database, HistoryEntry, NewBirthday, NewTask, NewTrackedDate, Policy, Project,
  Session, Status, Suggestion, Task, TaskEdit, TaskFilter, TrackedDateEdit, AGENT_PREFIX, DEFAULT_BIRTHDAY_GROUP,
  USER,
};
use crate::suggestions;

pub const UPCOMING_SOON_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DueState {
  Overdue,
  Today,
  Upcoming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpcomingKind {
  Event,
  Task,
  Birthday,
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
  /// Dica de onde a sessão veio, só quando ela não tem projeto ("app Claude · sessão lateral").
  pub origin: Option<String>,
  /// Link da sessão no controle remoto nativo do agente (Remote Control do Claude Code).
  pub remote_url: Option<String>,
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
pub struct UpcomingItemView {
  pub id: i64,
  pub kind: UpcomingKind,
  pub title: String,
  /// AAAA-MM-DD (próxima ocorrência para aniversários; data alvo nos demais).
  pub date: String,
  /// DD/MM ou DD/MM/AAAA quando em outro ano.
  pub date_label: String,
  pub days_until: i64,
  pub due_label: String,
  pub due_state: DueState,
  pub project: Option<String>,
  pub group: Option<String>,
  pub note: Option<String>,
  pub task_status: Option<String>,
  pub turning_age: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatesOverview {
  pub today: String,
  pub upcoming: Vec<UpcomingItemView>,
  pub birthdays: Vec<BirthdayView>,
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
  pub upcoming_soon: Vec<UpcomingItemView>,
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

fn due_state_of(date: NaiveDate, today: NaiveDate) -> DueState {
  match date.cmp(&today) {
    std::cmp::Ordering::Less => DueState::Overdue,
    std::cmp::Ordering::Equal => DueState::Today,
    std::cmp::Ordering::Greater => DueState::Upcoming,
  }
}

fn format_short_date(date: NaiveDate, today: NaiveDate) -> String {
  if date.year() == today.year() {
    format!("{:02}/{:02}", date.day(), date.month())
  } else {
    format!("{:02}/{:02}/{:04}", date.day(), date.month(), date.year())
  }
}

fn task_view(task: &Task, today: NaiveDate, sessions: &[Session]) -> TaskView {
  let due_state = task.due_date.map(|d| due_state_of(d, today));
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
    origin: session.project_name.is_none().then(|| agents::origin(&session.session_id, &session.cwd)),
    remote_url: session.remote_url.clone(),
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
    upcoming_soon: collect_upcoming(db, today, project, Some(UPCOMING_SOON_DAYS))?,
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

fn parse_project_edit(db: &Database, value: Option<&str>) -> Result<Option<Option<i64>>> {
  match value.map(str::trim) {
    None => Ok(None),
    Some("") => Ok(Some(None)),
    Some(name) => Ok(Some(Some(db.project_id(name)?))),
  }
}

fn parse_note_edit(value: Option<&str>) -> Option<Option<&str>> {
  match value.map(str::trim) {
    None => None,
    Some("") => Some(None),
    Some(note) => Some(Some(note)),
  }
}

pub fn edit_task(db: &Database, id: i64, changes: &TaskChanges) -> Result<()> {
  let today = dates::today();
  let due_date = match changes.due.as_deref().map(str::trim) {
    None => None,
    Some("") => Some(None),
    Some(d) => Some(Some(dates::parse_due(d, today)?)),
  };
  let project_id = parse_project_edit(db, changes.project.as_deref())?;
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

#[derive(Debug, Clone, Serialize)]
pub struct BirthdayView {
  pub id: i64,
  pub name: String,
  pub group: String,
  pub month: u32,
  pub day: u32,
  pub year: Option<i32>,
  pub note: Option<String>,
  /// "15/03" ou "15/03/1995".
  pub date_label: String,
  /// Próxima ocorrência (AAAA-MM-DD).
  pub next_date: String,
  pub days_until: i64,
  /// "hoje", "amanhã", "em 12 dias".
  pub due_label: String,
  pub due_state: DueState,
  /// Idade que completa em `next_date`, quando `year` está preenchido.
  pub turning_age: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BirthdayInput {
  pub name: String,
  pub group: Option<String>,
  pub month: u32,
  pub day: u32,
  pub year: Option<i32>,
  pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BirthdayChanges {
  pub name: Option<String>,
  pub group: Option<String>,
  pub month: Option<u32>,
  pub day: Option<u32>,
  /// `0` limpa o ano de nascimento.
  pub year: Option<i32>,
  /// `""` limpa a nota.
  pub note: Option<String>,
}

fn birthday_view(b: &Birthday, today: NaiveDate) -> BirthdayView {
  let next = dates::next_birthday(b.month, b.day, today);
  let days_until = (next - today).num_days();
  BirthdayView {
    id: b.id,
    name: b.name.clone(),
    group: b.group.clone(),
    month: b.month,
    day: b.day,
    year: b.year,
    note: b.note.clone(),
    date_label: match b.year {
      Some(y) => format!("{:02}/{:02}/{y:04}", b.day, b.month),
      None => format!("{:02}/{:02}", b.day, b.month),
    },
    next_date: next.to_string(),
    days_until,
    due_label: dates::humanize_due(next, today),
    due_state: if days_until == 0 { DueState::Today } else { DueState::Upcoming },
    turning_age: dates::turning_age(b.year, next),
  }
}

fn birthdays_for(db: &Database, today: NaiveDate) -> Result<Vec<BirthdayView>> {
  let mut views: Vec<BirthdayView> = db.get_birthdays()?.iter().map(|b| birthday_view(b, today)).collect();
  views.sort_by(|a, b| {
    a.days_until
      .cmp(&b.days_until)
      .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
      .then_with(|| a.id.cmp(&b.id))
  });
  Ok(views)
}

pub fn birthdays(db: &Database) -> Result<Vec<BirthdayView>> {
  birthdays_for(db, dates::today())
}

pub fn add_birthday(db: &Database, input: &BirthdayInput) -> Result<i64> {
  let name = input.name.trim();
  if name.is_empty() {
    anyhow::bail!("A pessoa precisa de um nome");
  }
  dates::validate_birthday(input.month, input.day, input.year, dates::today())?;
  let group = input.group.as_deref().map(str::trim).filter(|g| !g.is_empty()).unwrap_or(DEFAULT_BIRTHDAY_GROUP);
  let note = input.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
  db.add_birthday(NewBirthday {
    name,
    group,
    month: input.month,
    day: input.day,
    year: input.year,
    note,
  })
}

pub fn edit_birthday(db: &Database, id: i64, changes: &BirthdayChanges) -> Result<()> {
  let current = db.get_birthday(id)?;
  let name = match changes.name.as_deref().map(str::trim) {
    Some("") => anyhow::bail!("A pessoa precisa de um nome"),
    Some(n) => Some(n),
    None => None,
  };
  let group = changes.group.as_deref().map(str::trim).map(|g| if g.is_empty() { DEFAULT_BIRTHDAY_GROUP } else { g });
  let month = changes.month.unwrap_or(current.month);
  let day = changes.day.unwrap_or(current.day);
  let resolved_year = match changes.year {
    None => current.year,
    Some(y) if y <= 0 => None,
    Some(y) => Some(y),
  };
  dates::validate_birthday(month, day, resolved_year, dates::today())?;

  let year_edit = changes.year.map(|y| (y > 0).then_some(y));
  let note_edit = parse_note_edit(changes.note.as_deref());

  db.edit_birthday(id, BirthdayEdit {
    name,
    group,
    month: changes.month,
    day: changes.day,
    year: year_edit,
    note: note_edit,
  })
}

pub fn delete_birthday(db: &Database, id: i64) -> Result<()> {
  db.delete_birthday(id)
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrackedDateInput {
  pub title: String,
  /// AAAA-MM-DD, DD/MM/AAAA, hoje, amanhã, +3d.
  pub date: String,
  pub project: Option<String>,
  pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrackedDateChanges {
  pub title: Option<String>,
  pub date: Option<String>,
  /// `""` remove o projeto.
  pub project: Option<String>,
  /// `""` limpa a nota.
  pub note: Option<String>,
}

fn collect_upcoming(
  db: &Database,
  today: NaiveDate,
  project_id: Option<i64>,
  max_days: Option<i64>,
) -> Result<Vec<UpcomingItemView>> {
  let within_limit = |days_until: i64| max_days.is_none_or(|limit| days_until <= limit);
  let mut items = Vec::new();

  for item in db.get_tracked_dates(project_id)? {
    let days_until = (item.target_date - today).num_days();
    if !within_limit(days_until) {
      continue;
    }
    items.push(UpcomingItemView {
      id: item.id,
      kind: UpcomingKind::Event,
      title: item.title,
      date: item.target_date.to_string(),
      date_label: format_short_date(item.target_date, today),
      days_until,
      due_label: dates::humanize_due(item.target_date, today),
      due_state: due_state_of(item.target_date, today),
      project: item.project_name,
      group: None,
      note: item.note,
      task_status: None,
      turning_age: None,
    });
  }

  for task in db.get_tasks(TaskFilter { project_id, authors: Authors::All })? {
    if task.status == Status::Done {
      continue;
    }
    let Some(due) = task.due_date else { continue };
    let days_until = (due - today).num_days();
    if !within_limit(days_until) {
      continue;
    }
    items.push(UpcomingItemView {
      id: task.id,
      kind: UpcomingKind::Task,
      title: task.title,
      date: due.to_string(),
      date_label: format_short_date(due, today),
      days_until,
      due_label: dates::humanize_due(due, today),
      due_state: due_state_of(due, today),
      project: task.project_name,
      group: None,
      note: None,
      task_status: Some(task.status.as_str().to_string()),
      turning_age: None,
    });
  }

  if project_id.is_none() {
    for b in db.get_birthdays()? {
      let next = dates::next_birthday(b.month, b.day, today);
      let days_until = (next - today).num_days();
      if !within_limit(days_until) {
        continue;
      }
      let view = birthday_view(&b, today);
      items.push(UpcomingItemView {
        id: view.id,
        kind: UpcomingKind::Birthday,
        title: view.name,
        date: view.next_date,
        date_label: format_short_date(next, today),
        days_until,
        due_label: view.due_label,
        due_state: view.due_state,
        project: None,
        group: Some(view.group),
        note: view.note,
        task_status: None,
        turning_age: view.turning_age,
      });
    }
  }

  items.sort_by(|a, b| {
    a.days_until
      .cmp(&b.days_until)
      .then_with(|| a.kind.cmp(&b.kind))
      .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
      .then_with(|| a.id.cmp(&b.id))
  });
  Ok(items)
}

pub fn dates_overview(db: &Database, max_days: Option<i64>) -> Result<DatesOverview> {
  let today = dates::today();
  Ok(DatesOverview {
    today: today.to_string(),
    upcoming: collect_upcoming(db, today, None, max_days)?,
    birthdays: birthdays_for(db, today)?,
  })
}

pub fn add_tracked_date(db: &Database, input: &TrackedDateInput) -> Result<i64> {
  let title = input.title.trim();
  if title.is_empty() {
    anyhow::bail!("A data precisa de um título");
  }
  let raw_date = input.date.trim();
  if raw_date.is_empty() {
    anyhow::bail!("Informe uma data");
  }
  let target_date = dates::parse_due(raw_date, dates::today())?;
  let note = input.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
  db.add_tracked_date(NewTrackedDate {
    title,
    target_date,
    project_id: optional_project(db, input.project.as_deref())?,
    note,
  })
}

pub fn edit_tracked_date(db: &Database, id: i64, changes: &TrackedDateChanges) -> Result<()> {
  let title = match changes.title.as_deref().map(str::trim) {
    Some("") => anyhow::bail!("A data precisa de um título"),
    Some(t) => Some(t),
    None => None,
  };
  let target_date = match changes.date.as_deref().map(str::trim) {
    None => None,
    Some("") => anyhow::bail!("Informe uma data"),
    Some(d) => Some(dates::parse_due(d, dates::today())?),
  };
  let project_id = parse_project_edit(db, changes.project.as_deref())?;
  let note = parse_note_edit(changes.note.as_deref());
  db.edit_tracked_date(id, TrackedDateEdit { title, target_date, project_id, note })
}

pub fn delete_tracked_date(db: &Database, id: i64) -> Result<()> {
  db.delete_tracked_date(id)
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

  #[test]
  fn dates_overview_and_board_upcoming_combine_sources() {
    let db = Database::open_in_memory().unwrap();
    add_project(&db, "arca", "OWN").unwrap();
    let today = dates::today();

    let overdue_date = (today - chrono::Duration::days(2)).format(dates::DATE_FORMAT).to_string();
    let overdue_id = add_tracked_date(&db, &TrackedDateInput {
      title: "Entrega atrasada".into(),
      date: overdue_date,
      project: Some("arca".into()),
      note: Some("falar com cliente".into()),
    }).unwrap();
    add_tracked_date(&db, &TrackedDateInput {
      title: "Dentista".into(),
      date: "+5d".into(),
      ..Default::default()
    }).unwrap();
    let distant_id = add_tracked_date(&db, &TrackedDateInput {
      title: "Viagem".into(),
      date: "+20d".into(),
      ..Default::default()
    }).unwrap();

    add_task(&db, &TaskInput { title: "Subir build".into(), due: Some("+2d".into()), ..Default::default() }).unwrap();
    let done_task = add_task(&db, &TaskInput { title: "Já foi".into(), due: Some("+1d".into()), ..Default::default() }).unwrap();
    move_task(&db, done_task, "DONE").unwrap();

    let bday_date = today + chrono::Duration::days(3);
    let birth_year = if bday_date.month() == 2 && bday_date.day() == 29 { 1996 } else { bday_date.year() - 30 };
    let bday_id = add_birthday(&db, &BirthdayInput {
      name: "Ana".into(),
      group: None,
      month: bday_date.month(),
      day: bday_date.day(),
      year: Some(birth_year),
      note: Some("chocolate".into()),
    }).unwrap();
    assert_eq!(birthdays(&db).unwrap()[0].group, DEFAULT_BIRTHDAY_GROUP);
    edit_birthday(&db, bday_id, &BirthdayChanges { group: Some("Família".into()), ..Default::default() }).unwrap();
    let bdays = birthdays(&db).unwrap();
    assert_eq!(bdays.len(), 1);
    assert_eq!(bdays[0].group, "Família");
    assert_eq!(bdays[0].turning_age, Some((bday_date.year() - birth_year) as u32));

    let overview = dates_overview(&db, None).unwrap();
    let kinds: Vec<(UpcomingKind, &str)> = overview.upcoming.iter().map(|i| (i.kind, i.title.as_str())).collect();
    assert_eq!(kinds, vec![
      (UpcomingKind::Event, "Entrega atrasada"),
      (UpcomingKind::Task, "Subir build"),
      (UpcomingKind::Birthday, "Ana"),
      (UpcomingKind::Event, "Dentista"),
      (UpcomingKind::Event, "Viagem"),
    ]);
    assert_eq!(overview.upcoming[0].due_state, DueState::Overdue);
    assert_eq!(overview.upcoming[0].project.as_deref(), Some("arca"));
    assert_eq!(overview.upcoming[2].group.as_deref(), Some("Família"));

    let b = board(&db, &BoardFilter::default()).unwrap();
    let soon_titles: Vec<&str> = b.upcoming_soon.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(soon_titles, vec!["Entrega atrasada", "Subir build", "Ana", "Dentista"]);
    assert_eq!(b.upcoming_soon[0].due_state, DueState::Overdue);

    edit_tracked_date(&db, overdue_id, &TrackedDateChanges {
      title: Some("Entrega remarcada".into()),
      project: Some(String::new()),
      note: Some(String::new()),
      ..Default::default()
    }).unwrap();
    let edited = db.get_tracked_date(overdue_id).unwrap();
    assert_eq!((edited.title.as_str(), edited.project_id, edited.note), ("Entrega remarcada", None, None));

    delete_tracked_date(&db, overdue_id).unwrap();
    delete_tracked_date(&db, distant_id).unwrap();
    let b = board(&db, &BoardFilter::default()).unwrap();
    let soon_titles: Vec<&str> = b.upcoming_soon.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(soon_titles, vec!["Subir build", "Ana", "Dentista"]);
  }
}
