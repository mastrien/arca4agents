use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{bail, Context};
use chrono::NaiveDate;
use include_dir::{include_dir, Dir};
use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use rusqlite_migration::Migrations;

use crate::clock::Hlc;
use crate::dates::DATE_FORMAT;

// Embed the entire migrations directory into the binary at compile-time
static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

static MIGRATIONS: LazyLock<Migrations<'static>> =
  LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

pub const META_LAST_ROLLOVER: &str = "last_rollover_date";
pub const META_ACTIVE_WINDOW: &str = "active_window_minutes";
pub const META_DEVICE_ID: &str = "device_id";

pub const USER: &str = "user";
pub const AGENT_PREFIX: &str = "agent:";

pub struct Database {
  conn: Connection,
  clock: Arc<Mutex<Hlc>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
  Done,
  Cart,
  Shelf,
}

impl Status {
  pub fn as_str(self) -> &'static str {
    match self {
      Status::Done => "DONE",
      Status::Cart => "CART",
      Status::Shelf => "SHELF",
    }
  }

  fn parse(s: &str) -> rusqlite::Result<Self> {
    match s {
      "DONE" => Ok(Status::Done),
      "CART" => Ok(Status::Cart),
      "SHELF" => Ok(Status::Shelf),
      other => Err(invalid_text(other)),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Policy {
  None,
  Own,
  All,
}

impl Policy {
  pub fn as_str(self) -> &'static str {
    match self {
      Policy::None => "NONE",
      Policy::Own => "OWN",
      Policy::All => "ALL",
    }
  }

  fn parse(s: &str) -> rusqlite::Result<Self> {
    match s {
      "NONE" => Ok(Policy::None),
      "OWN" => Ok(Policy::Own),
      "ALL" => Ok(Policy::All),
      other => Err(invalid_text(other)),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionState {
  Working,
  Waiting,
  Ended,
}

impl SessionState {
  pub fn as_str(self) -> &'static str {
    match self {
      SessionState::Working => "WORKING",
      SessionState::Waiting => "WAITING",
      SessionState::Ended => "ENDED",
    }
  }

  fn parse(s: &str) -> rusqlite::Result<Self> {
    match s {
      "WORKING" => Ok(SessionState::Working),
      "WAITING" => Ok(SessionState::Waiting),
      "ENDED" => Ok(SessionState::Ended),
      other => Err(invalid_text(other)),
    }
  }
}

fn invalid_text(value: &str) -> rusqlite::Error {
  rusqlite::Error::FromSqlConversionFailure(
    0,
    rusqlite::types::Type::Text,
    format!("valor inesperado no banco: {value}").into(),
  )
}

#[derive(Clone, Debug)]
pub struct Task {
  pub id: i64,
  pub title: String,
  pub daily: bool,
  pub status: Status,
  pub created_at: String,
  pub due_date: Option<NaiveDate>,
  pub done_at: Option<String>,
  pub project_id: Option<i64>,
  pub project_name: Option<String>,
  /// Política de agentes do projeto da tarefa (`None` quando não tem projeto).
  pub project_policy: Option<Policy>,
  pub created_by: String,
}

impl Task {
  pub fn is_from_agent(&self) -> bool {
    self.created_by != USER
  }
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
  pub id: i64,
  pub title: String,
  pub daily: bool,
  pub date: String,
  pub due_date: Option<NaiveDate>,
  pub project_name: Option<String>,
  pub created_by: String,
}

#[derive(Clone, Debug)]
pub struct Project {
  pub id: i64,
  pub name: String,
  pub policy: Policy,
  pub folders: Vec<String>,
  pub shelf: u32,
  pub cart: u32,
  pub done: u32,
  /// Janela de "ativo agora" do projeto; `None` usa a global.
  pub active_window: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Session {
  pub session_id: String,
  pub agent: String,
  pub project_name: Option<String>,
  pub cwd: String,
  pub state: SessionState,
  pub started_at: String,
  pub last_seen_at: String,
  /// Tarefa em que a sessão está trabalhando.
  pub task_id: Option<i64>,
  pub task_title: Option<String>,
  /// Janela de "ativo agora" do projeto da sessão, se houver.
  pub project_window: Option<i64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SuggestionAction {
  Done,
  Move,
  Edit,
  Delete,
}

impl SuggestionAction {
  pub fn as_str(self) -> &'static str {
    match self {
      SuggestionAction::Done => "DONE",
      SuggestionAction::Move => "MOVE",
      SuggestionAction::Edit => "EDIT",
      SuggestionAction::Delete => "DELETE",
    }
  }

  fn parse(s: &str) -> rusqlite::Result<Self> {
    match s {
      "DONE" => Ok(SuggestionAction::Done),
      "MOVE" => Ok(SuggestionAction::Move),
      "EDIT" => Ok(SuggestionAction::Edit),
      "DELETE" => Ok(SuggestionAction::Delete),
      other => Err(invalid_text(other)),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SuggestionState {
  Pending,
  Approved,
  Rejected,
}

impl SuggestionState {
  pub fn as_str(self) -> &'static str {
    match self {
      SuggestionState::Pending => "PENDING",
      SuggestionState::Approved => "APPROVED",
      SuggestionState::Rejected => "REJECTED",
    }
  }

  fn parse(s: &str) -> rusqlite::Result<Self> {
    match s {
      "PENDING" => Ok(SuggestionState::Pending),
      "APPROVED" => Ok(SuggestionState::Approved),
      "REJECTED" => Ok(SuggestionState::Rejected),
      other => Err(invalid_text(other)),
    }
  }
}

/// Mudança proposta por um agente numa tarefa que ele não pode alterar.
/// `payload` é JSON: `{"to": "CART"}` para MOVE; para EDIT, chaves ausentes não mudam
/// e `null` limpa (`{"due": null}`).
#[derive(Clone, Debug)]
pub struct Suggestion {
  pub id: i64,
  pub task_id: i64,
  pub task_title: String,
  pub created_by: String,
  pub action: SuggestionAction,
  pub payload: serde_json::Value,
  pub reason: Option<String>,
  pub state: SuggestionState,
  pub created_at: String,
}

/// Quais autores aparecem numa listagem.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Authors {
  /// Visão padrão: só as tarefas do usuário.
  User,
  /// `--agents`: usuário e agentes.
  All,
  /// `--only-agents`.
  Agents,
}

#[derive(Clone, Copy, Debug)]
pub struct TaskFilter {
  pub project_id: Option<i64>,
  pub authors: Authors,
}

impl Default for TaskFilter {
  fn default() -> Self {
    Self { project_id: None, authors: Authors::User }
  }
}

pub struct NewTask<'a> {
  pub title: &'a str,
  pub daily: bool,
  pub status: Status,
  pub due_date: Option<NaiveDate>,
  pub project_id: Option<i64>,
  pub created_by: &'a str,
}

/// Campos editáveis. `Some(None)` limpa o valor (ex.: `--due none`).
#[derive(Default)]
pub struct TaskEdit<'a> {
  pub title: Option<&'a str>,
  pub daily: Option<bool>,
  pub due_date: Option<Option<NaiveDate>>,
  pub project_id: Option<Option<i64>>,
}

const TASK_SELECT: &str = "SELECT t.task_id, t.task_title, t.task_status, t.task_is_daily, t.created_at,
    t.due_date, t.done_at, t.project_id, p.project_name, t.created_by, p.agent_policy
  FROM tasks t LEFT JOIN projects p ON p.project_id = t.project_id";

fn task_from_row(row: &Row) -> rusqlite::Result<Task> {
  Ok(Task {
    id: row.get(0)?,
    title: row.get(1)?,
    status: Status::parse(&row.get::<_, String>(2)?)?,
    daily: row.get(3)?,
    created_at: row.get(4)?,
    due_date: parse_date_column(row.get(5)?),
    done_at: row.get(6)?,
    project_id: row.get(7)?,
    project_name: row.get(8)?,
    created_by: row.get(9)?,
    project_policy: row.get::<_, Option<String>>(10)?.map(|p| Policy::parse(&p)).transpose()?,
  })
}

fn parse_date_column(value: Option<String>) -> Option<NaiveDate> {
  value.and_then(|s| NaiveDate::parse_from_str(&s, DATE_FORMAT).ok())
}

fn date_to_sql(date: Option<NaiveDate>) -> Option<String> {
  date.map(|d| d.format(DATE_FORMAT).to_string())
}

pub fn new_uuid() -> String {
  uuid::Uuid::new_v4().to_string()
}

/// `arca_uuid()` e `arca_clock()`, usadas pelos triggers de sincronização.
fn register_sync_functions(conn: &Connection, clock: &Arc<Mutex<Hlc>>) -> rusqlite::Result<()> {
  conn.create_scalar_function("arca_uuid", 0, FunctionFlags::SQLITE_UTF8, |_| Ok(new_uuid()))?;
  let clock = Arc::clone(clock);
  conn.create_scalar_function("arca_clock", 0, FunctionFlags::SQLITE_UTF8, move |_| {
    clock.lock()
      .map(|mut c| c.tick())
      .map_err(|_| rusqlite::Error::UserFunctionError("relógio indisponível".into()))
  })
}

impl Database {
  pub fn init(db_path: impl AsRef<Path>) -> anyhow::Result<Self> {
    let conn = Connection::open(db_path)
      .context("Failed to open SQLite database")?;
    Self::setup(conn)
  }

  pub fn open_in_memory() -> anyhow::Result<Self> {
    Self::setup(Connection::open_in_memory()?)
  }

  fn setup(mut conn: Connection) -> anyhow::Result<Self> {
    // CLI, hooks e MCP escrevem no mesmo banco em processos diferentes.
    conn.busy_timeout(Duration::from_millis(5000))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    // O id do dispositivo só é conhecido depois das migrations; até lá vale um id novo, que
    // vira o definitivo se o banco ainda não tinha um (a migration de sync carimba com ele).
    let clock = Arc::new(Mutex::new(Hlc::new(new_uuid())));
    register_sync_functions(&conn, &clock)?;

    MIGRATIONS.to_latest(&mut conn)?;

    let db = Self { conn, clock };
    db.init_clock()?;
    Ok(db)
  }

  fn init_clock(&self) -> anyhow::Result<()> {
    let mut clock = self.clock.lock().map_err(|_| anyhow::anyhow!("relógio indisponível"))?;
    match self.get_meta(META_DEVICE_ID)? {
      Some(device) => clock.set_device(device),
      None => self.set_meta(META_DEVICE_ID, clock.device())?,
    }
    // Continua de onde o banco parou, mesmo que o relógio do sistema tenha voltado.
    let latest: Option<String> = self.conn.query_row(
      "SELECT MAX(c) FROM (
        SELECT MAX(sync_clock) AS c FROM projects UNION ALL SELECT MAX(sync_clock) FROM tasks
        UNION ALL SELECT MAX(sync_clock) FROM task_history UNION ALL SELECT MAX(sync_clock) FROM suggestions
        UNION ALL SELECT MAX(sync_clock) FROM sync_tombstones)",
      [], |r| r.get(0),
    )?;
    if let Some(latest) = latest {
      clock.observe(&latest);
    }
    Ok(())
  }

  /// Id deste dispositivo na sincronização.
  pub fn device_id(&self) -> String {
    self.clock.lock().map(|c| c.device().to_string()).unwrap_or_default()
  }

  /// Avança o relógio local para depois de um relógio remoto.
  pub fn observe_clock(&self, clock: &str) {
    if let Ok(mut hlc) = self.clock.lock() {
      hlc.observe(clock);
    }
  }

  pub fn conn(&self) -> &Connection {
    &self.conn
  }

  // ---------- Virada do dia ----------

  /// Executa o rollover se ainda não rodou hoje. Retorna se rodou.
  pub fn rollover_if_needed(&self, today: NaiveDate) -> anyhow::Result<bool> {
    let today_str = today.format(DATE_FORMAT).to_string();
    if self.get_meta(META_LAST_ROLLOVER)?.as_deref() == Some(today_str.as_str()) {
      return Ok(false);
    }

    let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
    // Outro processo pode ter feito a virada enquanto esperávamos o lock.
    let current: Option<String> = tx.query_row(
      "SELECT value FROM app_meta WHERE key = ?1", params![META_LAST_ROLLOVER], |r| r.get(0),
    ).optional()?;
    if current.as_deref() == Some(today_str.as_str()) {
      return Ok(false);
    }

    Self::archive_done(&tx, today)?;
    tx.execute(
      "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value",
      params![META_LAST_ROLLOVER, today_str],
    )?;
    tx.commit()?;
    Ok(true)
  }

  /// Arquiva as tarefas concluídas antes de `cutoff`: todas vão para o histórico
  /// (com a data em que foram concluídas), as diárias voltam ao carrinho e as demais saem.
  /// Retorna quantas foram arquivadas.
  pub fn archive_done_before(&self, cutoff: NaiveDate) -> anyhow::Result<usize> {
    let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
    let archived = Self::archive_done(&tx, cutoff)?;
    tx.commit()?;
    Ok(archived)
  }

  fn archive_done(tx: &Transaction, cutoff: NaiveDate) -> anyhow::Result<usize> {
    let cutoff = cutoff.format(DATE_FORMAT).to_string();
    let finished = "task_status = 'DONE' AND DATE(COALESCE(done_at, updated_at)) < ?1";

    let archived = tx.execute(
      // uuid determinístico: dois aparelhos que fazem a mesma virada geram a mesma linha.
      &format!("INSERT OR IGNORE INTO task_history (task_history_title, task_history_is_daily, task_history_date,
          task_history_due_date, task_history_project_id, task_history_created_by, uuid)
        SELECT task_title, task_is_daily, COALESCE(done_at, updated_at), due_date, project_id, created_by,
          'h:' || uuid || ':' || COALESCE(done_at, updated_at)
        FROM tasks WHERE {finished} ORDER BY COALESCE(done_at, updated_at), task_id"),
      params![cutoff],
    )?;
    tx.execute(&format!("DELETE FROM tasks WHERE task_is_daily = 0 AND {finished}"), params![cutoff])?;
    tx.execute(
      &format!("UPDATE tasks SET task_status = 'CART', done_at = NULL WHERE task_is_daily = 1 AND {finished}"),
      params![cutoff],
    )?;
    Ok(archived)
  }

  // ---------- Tarefas ----------

  /// Tarefas do filtro, na ordem estável usada pelas posições (`done`, `take`, `put-back`).
  pub fn get_tasks(&self, filter: TaskFilter) -> anyhow::Result<Vec<Task>> {
    let authors = match filter.authors {
      Authors::User => "t.created_by = 'user'",
      Authors::All => "1",
      Authors::Agents => "t.created_by <> 'user'",
    };
    let sql = format!(
      "{TASK_SELECT} WHERE {authors} AND (?1 IS NULL OR t.project_id = ?1) ORDER BY t.task_id ASC"
    );
    let mut stmt = self.conn.prepare(&sql).context("Fail while preparing SQL query")?;
    let tasks = stmt.query_map(params![filter.project_id], task_from_row)?
      .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tasks)
  }

  pub fn get_task(&self, id: i64) -> anyhow::Result<Task> {
    self.conn.query_row(&format!("{TASK_SELECT} WHERE t.task_id = ?1"), params![id], task_from_row)
      .optional()?
      .with_context(|| format!("Tarefa #{id} não encontrada"))
  }

  pub fn add_task(&self, task: NewTask) -> anyhow::Result<i64> {
    self.conn.execute(
      "INSERT INTO tasks (task_title, task_is_daily, task_status, due_date, project_id, created_by, done_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, CASE WHEN ?3 = 'DONE' THEN DATETIME('now', 'localtime') END)",
      params![task.title, task.daily, task.status.as_str(), date_to_sql(task.due_date), task.project_id, task.created_by],
    ).context("Fail to insert task")?;

    Ok(self.conn.last_insert_rowid())
  }

  pub fn edit_task(&self, id: i64, edit: TaskEdit) -> anyhow::Result<()> {
    let (set_due, due) = match edit.due_date {
      Some(value) => (true, date_to_sql(value)),
      None => (false, None),
    };
    let (set_project, project) = match edit.project_id {
      Some(value) => (true, value),
      None => (false, None),
    };

    let rows = self.conn.execute(
      "UPDATE tasks SET
        task_title = COALESCE(?1, task_title),
        task_is_daily = COALESCE(?2, task_is_daily),
        due_date = CASE WHEN ?3 THEN ?4 ELSE due_date END,
        project_id = CASE WHEN ?5 THEN ?6 ELSE project_id END
      WHERE task_id = ?7",
      params![edit.title, edit.daily, set_due, due, set_project, project, id],
    ).context("Fail to update task")?;

    if rows == 0 {
      bail!("Tarefa #{id} não encontrada");
    }
    Ok(())
  }

  /// Muda o status mantendo `done_at` coerente: preenchido ao concluir, limpo ao reabrir.
  pub fn change_task_status(&self, id: i64, status: Status) -> anyhow::Result<()> {
    let rows = self.conn.execute(
      "UPDATE tasks SET
        task_status = ?1,
        done_at = CASE
          WHEN ?1 <> 'DONE' THEN NULL
          WHEN task_status = 'DONE' THEN done_at
          ELSE DATETIME('now', 'localtime')
        END
      WHERE task_id = ?2",
      params![status.as_str(), id],
    ).context("Fail to change task status")?;

    if rows == 0 {
      bail!("Tarefa #{id} não encontrada");
    }
    Ok(())
  }

  pub fn delete_task(&self, id: i64) -> anyhow::Result<()> {
    let rows = self.conn.execute("DELETE FROM tasks WHERE task_id = ?1", params![id])
      .context("Fail to delete task")?;
    if rows == 0 {
      bail!("Tarefa #{id} não encontrada");
    }
    Ok(())
  }

  pub fn get_history(&self) -> anyhow::Result<Vec<HistoryEntry>> {
    let mut stmt = self.conn.prepare(
      "SELECT h.task_history_id, h.task_history_title, h.task_history_is_daily, h.task_history_date,
          h.task_history_due_date, p.project_name, h.task_history_created_by
        FROM task_history h LEFT JOIN projects p ON p.project_id = h.task_history_project_id
        ORDER BY h.task_history_date ASC, h.task_history_id ASC"
    ).context("Fail while preparing SQL query")?;

    let entries = stmt.query_map([], |row| {
      Ok(HistoryEntry {
        id: row.get(0)?,
        title: row.get(1)?,
        daily: row.get(2)?,
        date: row.get(3)?,
        due_date: parse_date_column(row.get(4)?),
        project_name: row.get(5)?,
        created_by: row.get(6)?,
      })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(entries)
  }

  /// Fotos diárias de (data, estante, carrinho), em ordem de data. Gravadas pelos triggers da
  /// migration 06; um dia sem linha repete o último dia registrado antes dele.
  pub fn get_status_snapshots(&self) -> anyhow::Result<Vec<(String, u32, u32)>> {
    let mut stmt = self.conn.prepare("SELECT snapshot_date, shelf, cart FROM status_snapshots ORDER BY snapshot_date")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
      .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
  }

  // ---------- Projetos ----------

  pub fn add_project(&self, name: &str, policy: Policy) -> anyhow::Result<i64> {
    if self.find_project_id(name)?.is_some() {
      bail!("Já existe um projeto chamado '{name}'");
    }
    self.conn.execute(
      "INSERT INTO projects (project_name, agent_policy) VALUES (?1, ?2)",
      params![name, policy.as_str()],
    ).context("Fail to insert project")?;
    Ok(self.conn.last_insert_rowid())
  }

  pub fn find_project_id(&self, name: &str) -> anyhow::Result<Option<i64>> {
    Ok(self.conn.query_row(
      "SELECT project_id FROM projects WHERE project_name = ?1 COLLATE NOCASE",
      params![name], |r| r.get(0),
    ).optional()?)
  }

  pub fn project_id(&self, name: &str) -> anyhow::Result<i64> {
    self.find_project_id(name)?
      .with_context(|| format!("Projeto '{name}' não encontrado (veja `arca project list`)"))
  }

  pub fn get_projects(&self) -> anyhow::Result<Vec<Project>> {
    let mut stmt = self.conn.prepare(
      "SELECT p.project_id, p.project_name, p.agent_policy,
          (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.project_id AND t.task_status = 'SHELF'),
          (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.project_id AND t.task_status = 'CART'),
          (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.project_id AND t.task_status = 'DONE')
            + (SELECT COUNT(*) FROM task_history h WHERE h.task_history_project_id = p.project_id),
          p.active_window_minutes
        FROM projects p ORDER BY p.project_name COLLATE NOCASE"
    )?;
    let mut projects = stmt.query_map([], |row| {
      Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        policy: Policy::parse(&row.get::<_, String>(2)?)?,
        folders: Vec::new(),
        shelf: row.get(3)?,
        cart: row.get(4)?,
        done: row.get(5)?,
        active_window: row.get(6)?,
      })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    let folders = self.get_folders()?;
    for project in &mut projects {
      project.folders = folders.iter()
        .filter(|(_, id)| *id == project.id)
        .map(|(path, _)| path.clone())
        .collect();
    }
    Ok(projects)
  }

  pub fn get_project(&self, id: i64) -> anyhow::Result<Project> {
    self.get_projects()?.into_iter().find(|p| p.id == id)
      .with_context(|| format!("Projeto #{id} não encontrado"))
  }

  pub fn rename_project(&self, id: i64, new_name: &str) -> anyhow::Result<()> {
    if let Some(other) = self.find_project_id(new_name)?
      && other != id {
        bail!("Já existe um projeto chamado '{new_name}'");
      }
    self.conn.execute("UPDATE projects SET project_name = ?1 WHERE project_id = ?2", params![new_name, id])?;
    Ok(())
  }

  pub fn set_project_policy(&self, id: i64, policy: Policy) -> anyhow::Result<()> {
    self.conn.execute("UPDATE projects SET agent_policy = ?1 WHERE project_id = ?2", params![policy.as_str(), id])?;
    Ok(())
  }

  pub fn set_project_window(&self, id: i64, minutes: Option<i64>) -> anyhow::Result<()> {
    self.conn.execute("UPDATE projects SET active_window_minutes = ?1 WHERE project_id = ?2", params![minutes, id])?;
    Ok(())
  }

  /// Apaga o projeto; as tarefas ficam sem projeto (ON DELETE SET NULL) e as pastas são desvinculadas.
  pub fn delete_project(&self, id: i64) -> anyhow::Result<()> {
    self.conn.execute("DELETE FROM projects WHERE project_id = ?1", params![id])?;
    Ok(())
  }

  /// Vincula uma pasta (já normalizada). Retorna `false` se ela já era deste projeto.
  pub fn link_folder(&self, project_id: i64, folder: &str) -> anyhow::Result<bool> {
    let owner: Option<(i64, String)> = self.conn.query_row(
      "SELECT f.project_id, p.project_name FROM project_folders f
        JOIN projects p ON p.project_id = f.project_id WHERE f.folder_path = ?1",
      params![folder], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?;

    match owner {
      Some((id, _)) if id == project_id => Ok(false),
      Some((_, name)) => bail!("A pasta '{folder}' já está vinculada ao projeto '{name}'"),
      None => {
        self.conn.execute(
          "INSERT INTO project_folders (project_id, folder_path) VALUES (?1, ?2)",
          params![project_id, folder],
        )?;
        Ok(true)
      }
    }
  }

  pub fn unlink_folder(&self, folder: &str) -> anyhow::Result<()> {
    let rows = self.conn.execute("DELETE FROM project_folders WHERE folder_path = ?1", params![folder])?;
    if rows == 0 {
      bail!("A pasta '{folder}' não está vinculada a nenhum projeto");
    }
    Ok(())
  }

  /// Todas as pastas vinculadas: (caminho normalizado, project_id).
  pub fn get_folders(&self) -> anyhow::Result<Vec<(String, i64)>> {
    let mut stmt = self.conn.prepare("SELECT folder_path, project_id FROM project_folders ORDER BY folder_path")?;
    let folders = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
      .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(folders)
  }

  /// Projeto da pasta vinculada mais específica que contém `cwd` (já normalizado).
  pub fn project_for_path(&self, cwd: &str) -> anyhow::Result<Option<i64>> {
    Ok(crate::paths::most_specific(cwd, &self.get_folders()?).copied())
  }

  // ---------- Metadados ----------

  pub fn get_meta(&self, key: &str) -> anyhow::Result<Option<String>> {
    Ok(self.conn.query_row("SELECT value FROM app_meta WHERE key = ?1", params![key], |r| r.get(0)).optional()?)
  }

  pub fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
    self.conn.execute(
      "INSERT INTO app_meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
      params![key, value],
    )?;
    Ok(())
  }

  // ---------- Sessões de agentes ----------

  pub fn upsert_session(
    &self,
    session_id: &str,
    agent: &str,
    project_id: Option<i64>,
    cwd: &str,
    state: SessionState,
  ) -> anyhow::Result<()> {
    self.conn.execute(
      "INSERT INTO agent_sessions (session_id, agent, project_id, cwd, state) VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(session_id) DO UPDATE SET
          agent = excluded.agent,
          project_id = excluded.project_id,
          cwd = excluded.cwd,
          state = excluded.state,
          last_seen_at = DATETIME('now', 'localtime')",
      params![session_id, agent, project_id, cwd, state.as_str()],
    )?;
    Ok(())
  }

  pub fn end_session(&self, session_id: &str) -> anyhow::Result<()> {
    self.conn.execute(
      "UPDATE agent_sessions SET state = 'ENDED', last_seen_at = DATETIME('now', 'localtime') WHERE session_id = ?1",
      params![session_id],
    )?;
    Ok(())
  }

  /// Remove sessões encerradas há mais de `days` dias, para a tabela não crescer sem limite.
  pub fn prune_sessions(&self, days: u32) -> anyhow::Result<()> {
    self.conn.execute(
      "DELETE FROM agent_sessions WHERE state = 'ENDED'
        AND last_seen_at < DATETIME('now', 'localtime', ?1)",
      params![format!("-{days} days")],
    )?;
    Ok(())
  }

  /// Sessões da mais recente para a mais antiga.
  pub fn get_sessions(&self) -> anyhow::Result<Vec<Session>> {
    let mut stmt = self.conn.prepare(
      "SELECT s.session_id, s.agent, p.project_name, s.cwd, s.state, s.started_at, s.last_seen_at,
          s.task_id, t.task_title, p.active_window_minutes
        FROM agent_sessions s
          LEFT JOIN projects p ON p.project_id = s.project_id
          LEFT JOIN tasks t ON t.task_id = s.task_id
        ORDER BY s.last_seen_at DESC"
    )?;
    let sessions = stmt.query_map([], |row| {
      Ok(Session {
        session_id: row.get(0)?,
        agent: row.get(1)?,
        project_name: row.get(2)?,
        cwd: row.get(3)?,
        state: SessionState::parse(&row.get::<_, String>(4)?)?,
        started_at: row.get(5)?,
        last_seen_at: row.get(6)?,
        task_id: row.get(7)?,
        task_title: row.get(8)?,
        project_window: row.get(9)?,
      })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(sessions)
  }

  /// Sessão aberta (não encerrada) mais recente de um agente numa pasta, ignorando as
  /// sessões próprias dos servidores MCP. Usada para o MCP achar a sessão registrada pelos hooks.
  pub fn find_open_session(&self, agent: &str, cwd: &str) -> anyhow::Result<Option<String>> {
    Ok(self.conn.query_row(
      "SELECT session_id FROM agent_sessions
        WHERE agent = ?1 AND cwd = ?2 AND state <> 'ENDED' AND session_id NOT LIKE 'mcp:%'
        ORDER BY last_seen_at DESC LIMIT 1",
      params![agent, cwd], |r| r.get(0),
    ).optional()?)
  }

  /// Sinal de vida implícito (ex.: uma chamada MCP): marca a sessão como trabalhando agora.
  pub fn touch_session(&self, session_id: &str) -> anyhow::Result<()> {
    self.conn.execute(
      "UPDATE agent_sessions SET state = 'WORKING', last_seen_at = DATETIME('now', 'localtime') WHERE session_id = ?1",
      params![session_id],
    )?;
    Ok(())
  }

  /// Liga (ou desliga, com `None`) a sessão à tarefa em que ela está trabalhando.
  pub fn set_session_task(&self, session_id: &str, task_id: Option<i64>) -> anyhow::Result<()> {
    let rows = self.conn.execute(
      "UPDATE agent_sessions SET task_id = ?1 WHERE session_id = ?2",
      params![task_id, session_id],
    )?;
    if rows == 0 {
      bail!("Sessão '{session_id}' não encontrada");
    }
    Ok(())
  }

  // ---------- Sugestões ----------

  pub fn add_suggestion(
    &self,
    task_id: i64,
    created_by: &str,
    action: SuggestionAction,
    payload: &serde_json::Value,
    reason: Option<&str>,
  ) -> anyhow::Result<i64> {
    self.conn.execute(
      "INSERT INTO suggestions (task_id, created_by, action, payload, reason) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![task_id, created_by, action.as_str(), payload.to_string(), reason],
    ).context("Fail to insert suggestion")?;
    Ok(self.conn.last_insert_rowid())
  }

  /// Sugestões, das mais antigas para as mais novas; `only_pending` esconde as já resolvidas.
  pub fn get_suggestions(&self, only_pending: bool) -> anyhow::Result<Vec<Suggestion>> {
    let mut stmt = self.conn.prepare(
      "SELECT s.suggestion_id, s.task_id, t.task_title, s.created_by, s.action, s.payload, s.reason,
          s.state, s.created_at
        FROM suggestions s JOIN tasks t ON t.task_id = s.task_id
        WHERE (?1 = 0 OR s.state = 'PENDING')
        ORDER BY s.suggestion_id ASC"
    )?;
    let suggestions = stmt.query_map(params![only_pending], |row| {
      Ok(Suggestion {
        id: row.get(0)?,
        task_id: row.get(1)?,
        task_title: row.get(2)?,
        created_by: row.get(3)?,
        action: SuggestionAction::parse(&row.get::<_, String>(4)?)?,
        payload: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or(serde_json::Value::Null),
        reason: row.get(6)?,
        state: SuggestionState::parse(&row.get::<_, String>(7)?)?,
        created_at: row.get(8)?,
      })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(suggestions)
  }

  pub fn get_suggestion(&self, id: i64) -> anyhow::Result<Suggestion> {
    self.get_suggestions(false)?.into_iter().find(|s| s.id == id)
      .with_context(|| format!("Sugestão #{id} não encontrada (veja `arca suggestions`)"))
  }

  pub fn resolve_suggestion(&self, id: i64, state: SuggestionState) -> anyhow::Result<()> {
    self.conn.execute(
      "UPDATE suggestions SET state = ?1, resolved_at = DATETIME('now', 'localtime') WHERE suggestion_id = ?2",
      params![state.as_str(), id],
    )?;
    Ok(())
  }

  pub fn count_pending_suggestions(&self) -> anyhow::Result<u32> {
    Ok(self.conn.query_row("SELECT COUNT(*) FROM suggestions WHERE state = 'PENDING'", [], |r| r.get(0))?)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, DATE_FORMAT).unwrap()
  }

  fn add(db: &Database, title: &str, daily: bool) -> i64 {
    db.add_task(NewTask {
      title, daily, status: Status::Cart, due_date: None, project_id: None, created_by: USER,
    }).unwrap()
  }

  fn set_done_at(db: &Database, id: i64, done_at: &str) {
    db.conn().execute("UPDATE tasks SET task_status = 'DONE', done_at = ?1 WHERE task_id = ?2", params![done_at, id]).unwrap();
  }

  #[test]
  fn rollover_archives_and_resets_dailies() {
    let db = Database::open_in_memory().unwrap();
    let daily = add(&db, "Ler 10 páginas", true);
    let once = add(&db, "Comprar café", false);
    let today_done = add(&db, "Feita hoje", false);
    let pending = add(&db, "Pendente", false);

    set_done_at(&db, daily, "2026-10-26 09:00:00");
    set_done_at(&db, once, "2026-10-27 18:30:00");
    set_done_at(&db, today_done, "2026-10-29 08:00:00");

    // Fim de semana sem usar: a virada só roda hoje, mas o histórico guarda o dia da conclusão.
    assert!(db.rollover_if_needed(d("2026-10-29")).unwrap());
    assert!(!db.rollover_if_needed(d("2026-10-29")).unwrap());

    let daily_task = db.get_task(daily).unwrap();
    assert_eq!(daily_task.status, Status::Cart);
    assert!(daily_task.done_at.is_none());
    assert!(db.get_task(once).is_err());
    assert_eq!(db.get_task(today_done).unwrap().status, Status::Done);
    assert_eq!(db.get_task(pending).unwrap().status, Status::Cart);

    let history = db.get_history().unwrap();
    let rows: Vec<(&str, &str)> = history.iter().map(|h| (h.title.as_str(), h.date.as_str())).collect();
    assert_eq!(rows, vec![("Ler 10 páginas", "2026-10-26 09:00:00"), ("Comprar café", "2026-10-27 18:30:00")]);
    assert_eq!(db.get_meta(META_LAST_ROLLOVER).unwrap().as_deref(), Some("2026-10-29"));
  }

  #[test]
  fn status_snapshot_follows_every_task_write() {
    let db = Database::open_in_memory().unwrap();
    let today: String = db.conn().query_row("SELECT DATE('now', 'localtime')", [], |r| r.get(0)).unwrap();
    let snapshot = |db: &Database| db.get_status_snapshots().unwrap();

    // A migration já grava a foto de hoje, mesmo com o banco vazio.
    assert_eq!(snapshot(&db), vec![(today.clone(), 0, 0)]);

    let a = add(&db, "No carrinho", false);
    let b = add(&db, "Também", false);
    assert_eq!(snapshot(&db), vec![(today.clone(), 0, 2)]);

    db.change_task_status(a, Status::Shelf).unwrap();
    assert_eq!(snapshot(&db), vec![(today.clone(), 1, 1)]);

    db.change_task_status(b, Status::Done).unwrap();
    assert_eq!(snapshot(&db), vec![(today.clone(), 1, 0)]);

    db.delete_task(a).unwrap();
    assert_eq!(snapshot(&db), vec![(today.clone(), 0, 0)]);
  }

  #[test]
  fn status_changes_keep_done_at_consistent() {
    let db = Database::open_in_memory().unwrap();
    let id = add(&db, "Tarefa", false);

    db.change_task_status(id, Status::Done).unwrap();
    let first = db.get_task(id).unwrap().done_at;
    assert!(first.is_some());

    db.conn().execute("UPDATE tasks SET done_at = '2026-01-01 00:00:00' WHERE task_id = ?1", params![id]).unwrap();
    db.change_task_status(id, Status::Done).unwrap();
    assert_eq!(db.get_task(id).unwrap().done_at.as_deref(), Some("2026-01-01 00:00:00"));

    db.change_task_status(id, Status::Shelf).unwrap();
    assert!(db.get_task(id).unwrap().done_at.is_none());

    assert!(db.change_task_status(999, Status::Done).is_err());
  }

  #[test]
  fn edit_sets_and_clears_due_and_project() {
    let db = Database::open_in_memory().unwrap();
    let project = db.add_project("arca", Policy::Own).unwrap();
    let id = add(&db, "Tarefa", false);

    db.edit_task(id, TaskEdit { due_date: Some(Some(d("2026-11-05"))), project_id: Some(Some(project)), ..Default::default() }).unwrap();
    let task = db.get_task(id).unwrap();
    assert_eq!(task.due_date, Some(d("2026-11-05")));
    assert_eq!(task.project_name.as_deref(), Some("arca"));

    db.edit_task(id, TaskEdit { title: Some("Novo título"), ..Default::default() }).unwrap();
    let task = db.get_task(id).unwrap();
    assert_eq!(task.title, "Novo título");
    assert_eq!(task.due_date, Some(d("2026-11-05")));

    db.edit_task(id, TaskEdit { due_date: Some(None), ..Default::default() }).unwrap();
    assert_eq!(db.get_task(id).unwrap().due_date, None);
  }

  #[test]
  fn deleting_project_keeps_tasks() {
    let db = Database::open_in_memory().unwrap();
    let project = db.add_project("arca", Policy::Own).unwrap();
    db.link_folder(project, "c:/root_lab/arca").unwrap();
    let id = db.add_task(NewTask {
      title: "x", daily: false, status: Status::Shelf, due_date: None, project_id: Some(project), created_by: USER,
    }).unwrap();

    db.delete_project(project).unwrap();
    assert_eq!(db.get_task(id).unwrap().project_id, None);
    assert!(db.get_folders().unwrap().is_empty());
  }

  #[test]
  fn resolves_project_by_most_specific_folder() {
    let db = Database::open_in_memory().unwrap();
    let lab = db.add_project("lab", Policy::Own).unwrap();
    let arca = db.add_project("arca", Policy::All).unwrap();
    db.link_folder(lab, "c:/root_lab").unwrap();
    db.link_folder(arca, "c:/root_lab/arca4agents").unwrap();

    assert_eq!(db.project_for_path("c:/root_lab/arca4agents/src").unwrap(), Some(arca));
    assert_eq!(db.project_for_path("c:/root_lab/outro").unwrap(), Some(lab));
    assert_eq!(db.project_for_path("d:/x").unwrap(), None);

    assert!(!db.link_folder(arca, "c:/root_lab/arca4agents").unwrap());
    assert!(db.link_folder(lab, "c:/root_lab/arca4agents").is_err());
  }

  #[test]
  fn filters_by_author() {
    let db = Database::open_in_memory().unwrap();
    add(&db, "do usuário", false);
    db.add_task(NewTask {
      title: "do agente", daily: false, status: Status::Shelf, due_date: None, project_id: None, created_by: "agent:claude-code",
    }).unwrap();

    let titles = |authors| -> Vec<String> {
      db.get_tasks(TaskFilter { project_id: None, authors }).unwrap().into_iter().map(|t| t.title).collect()
    };
    assert_eq!(titles(Authors::User), vec!["do usuário"]);
    assert_eq!(titles(Authors::Agents), vec!["do agente"]);
    assert_eq!(titles(Authors::All), vec!["do usuário", "do agente"]);
  }

  #[test]
  fn hooks_keep_the_session_task_link() {
    let db = Database::open_in_memory().unwrap();
    let id = add(&db, "Tarefa", false);
    db.upsert_session("s1", "claude-code", None, "c:/x", SessionState::Working).unwrap();
    db.upsert_session("mcp:claude-code:1", "claude-code", None, "c:/x", SessionState::Working).unwrap();

    assert_eq!(db.find_open_session("claude-code", "c:/x").unwrap().as_deref(), Some("s1"));
    assert_eq!(db.find_open_session("claude-code", "c:/y").unwrap(), None);

    db.set_session_task("s1", Some(id)).unwrap();
    db.upsert_session("s1", "claude-code", None, "c:/x", SessionState::Waiting).unwrap();
    let session = db.get_sessions().unwrap().into_iter().find(|s| s.session_id == "s1").unwrap();
    assert_eq!(session.task_title.as_deref(), Some("Tarefa"));

    db.upsert_session("s1", "claude-code", None, "c:/x", SessionState::Ended).unwrap();
    assert_eq!(db.find_open_session("claude-code", "c:/x").unwrap(), None);
    assert!(db.set_session_task("nao-existe", None).is_err());
  }

  #[test]
  fn suggestions_are_listed_and_resolved() {
    let db = Database::open_in_memory().unwrap();
    let id = add(&db, "Tarefa", false);
    let s = db.add_suggestion(id, "agent:claude-code", SuggestionAction::Move, &serde_json::json!({"to": "SHELF"}), Some("parada")).unwrap();
    assert_eq!(db.count_pending_suggestions().unwrap(), 1);
    assert_eq!(db.get_suggestion(s).unwrap().payload["to"], "SHELF");

    db.resolve_suggestion(s, SuggestionState::Rejected).unwrap();
    assert_eq!(db.count_pending_suggestions().unwrap(), 0);
    assert!(db.get_suggestions(true).unwrap().is_empty());
    assert_eq!(db.get_suggestions(false).unwrap()[0].state, SuggestionState::Rejected);

    // Apagar a tarefa leva junto as sugestões dela.
    db.delete_task(id).unwrap();
    assert!(db.get_suggestions(false).unwrap().is_empty());
  }

  #[test]
  fn session_upsert_updates_state() {
    let db = Database::open_in_memory().unwrap();
    db.upsert_session("s1", "claude-code", None, "c:/x", SessionState::Working).unwrap();
    db.upsert_session("s1", "claude-code", None, "c:/x", SessionState::Waiting).unwrap();
    let sessions = db.get_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].state, SessionState::Waiting);
  }
}
