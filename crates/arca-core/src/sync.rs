//! Sincronização entre dispositivos (spec-apps.md, seção 3), independente do transporte.
//!
//! Cada linha sincronizável tem `uuid` e `sync_clock` (HLC, ver `clock.rs`), carimbados pelos
//! triggers da migration 05. A regra é "a última escrita vence, por linha": uma mudança remota só
//! sobrescreve a local se o relógio dela for maior. Exclusões viajam como tombstones.
//!
//! Protocolo (cliente = celular, servidor = PC):
//! 1. push: o cliente envia `changes_since(cliente, last_pushed, Some(dispositivo do servidor))`
//!    e o servidor aplica com `apply`.
//! 2. pull: o servidor devolve `changes_since(servidor, last_pulled, Some(dispositivo do cliente))`
//!    com as sessões de agentes, e o cliente aplica.
//!
//! Os ids inteiros são locais: referências viajam como uuid e são traduzidas ao aplicar.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::clock;
use crate::dates;
use crate::persistence::Database;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRow {
  pub uuid: String,
  pub name: String,
  pub agent_policy: String,
  pub active_window_minutes: Option<i64>,
  pub created_at: Option<String>,
  pub sync_clock: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRow {
  pub uuid: String,
  pub title: String,
  pub status: String,
  pub daily: bool,
  pub created_at: Option<String>,
  pub updated_at: Option<String>,
  pub due_date: Option<String>,
  pub done_at: Option<String>,
  pub project_uuid: Option<String>,
  pub created_by: String,
  pub sync_clock: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRow {
  pub uuid: String,
  pub title: String,
  pub daily: bool,
  pub date: Option<String>,
  pub due_date: Option<String>,
  pub project_uuid: Option<String>,
  pub created_by: String,
  pub sync_clock: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestionRow {
  pub uuid: String,
  pub task_uuid: String,
  pub created_by: String,
  pub action: String,
  pub payload: String,
  pub reason: Option<String>,
  pub state: String,
  pub created_at: Option<String>,
  pub resolved_at: Option<String>,
  pub sync_clock: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tombstone {
  pub entity: String,
  pub uuid: String,
  pub sync_clock: String,
}

/// Sessões de agentes viajam só do PC para o celular, para leitura.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRow {
  pub session_id: String,
  pub agent: String,
  pub project_uuid: Option<String>,
  pub cwd: String,
  pub state: String,
  pub started_at: Option<String>,
  pub last_seen_at: Option<String>,
  pub task_uuid: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChangeSet {
  /// Dispositivo que gerou o conjunto.
  pub device: String,
  /// Maior relógio considerado; o próximo pedido usa este valor como `since`.
  pub cursor: String,
  pub projects: Vec<ProjectRow>,
  pub tasks: Vec<TaskRow>,
  pub history: Vec<HistoryRow>,
  pub suggestions: Vec<SuggestionRow>,
  pub tombstones: Vec<Tombstone>,
  pub sessions: Vec<SessionRow>,
  /// Leituras de cota dos agentes, só do PC para o celular (ver `usage::samples_for_sync`).
  /// `default`: versões anteriores não mandam o campo.
  #[serde(default)]
  pub usage: Vec<crate::usage::UsageSample>,
}

impl ChangeSet {
  pub fn is_empty(&self) -> bool {
    self.projects.is_empty() && self.tasks.is_empty() && self.history.is_empty()
      && self.suggestions.is_empty() && self.tombstones.is_empty() && self.sessions.is_empty()
      && self.usage.is_empty()
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
  /// Linhas criadas ou atualizadas.
  pub applied: usize,
  /// Linhas ignoradas porque a versão local era mais nova (ou a referência não existia).
  pub skipped: usize,
  pub deleted: usize,
  /// Tarefas arquivadas pela virada do dia depois de aplicar.
  pub archived: usize,
}

/// Linhas com relógio maior que `since`, sem as escritas feitas por `exclude_device`
/// (que já as tem). `include_sessions` acrescenta as sessões de agentes recentes.
pub fn changes_since(db: &Database, since: &str, exclude_device: Option<&str>, include_sessions: bool) -> Result<ChangeSet> {
  let conn = db.conn();
  let mut cursor = since.to_string();
  let mut keep = |clock: &str| {
    if clock > cursor.as_str() {
      cursor = clock.to_string();
    }
    exclude_device.is_none_or(|d| clock::device_of(clock) != Some(d))
  };

  let mut set = ChangeSet { device: db.device_id(), ..Default::default() };

  let mut stmt = conn.prepare(
    "SELECT uuid, project_name, agent_policy, active_window_minutes, created_at, sync_clock
      FROM projects WHERE sync_clock > ?1 ORDER BY sync_clock",
  )?;
  for row in stmt.query_map(params![since], |r| Ok(ProjectRow {
    uuid: r.get(0)?, name: r.get(1)?, agent_policy: r.get(2)?, active_window_minutes: r.get(3)?,
    created_at: r.get(4)?, sync_clock: r.get(5)?,
  }))? {
    let row = row?;
    if keep(&row.sync_clock) { set.projects.push(row); }
  }

  let mut stmt = conn.prepare(
    "SELECT t.uuid, t.task_title, t.task_status, t.task_is_daily, t.created_at, t.updated_at, t.due_date,
        t.done_at, p.uuid, t.created_by, t.sync_clock
      FROM tasks t LEFT JOIN projects p ON p.project_id = t.project_id
      WHERE t.sync_clock > ?1 ORDER BY t.sync_clock",
  )?;
  for row in stmt.query_map(params![since], |r| Ok(TaskRow {
    uuid: r.get(0)?, title: r.get(1)?, status: r.get(2)?, daily: r.get(3)?, created_at: r.get(4)?,
    updated_at: r.get(5)?, due_date: r.get(6)?, done_at: r.get(7)?, project_uuid: r.get(8)?,
    created_by: r.get(9)?, sync_clock: r.get(10)?,
  }))? {
    let row = row?;
    if keep(&row.sync_clock) { set.tasks.push(row); }
  }

  let mut stmt = conn.prepare(
    "SELECT h.uuid, h.task_history_title, h.task_history_is_daily, h.task_history_date, h.task_history_due_date,
        p.uuid, h.task_history_created_by, h.sync_clock
      FROM task_history h LEFT JOIN projects p ON p.project_id = h.task_history_project_id
      WHERE h.sync_clock > ?1 ORDER BY h.sync_clock",
  )?;
  for row in stmt.query_map(params![since], |r| Ok(HistoryRow {
    uuid: r.get(0)?, title: r.get(1)?, daily: r.get(2)?, date: r.get(3)?, due_date: r.get(4)?,
    project_uuid: r.get(5)?, created_by: r.get(6)?, sync_clock: r.get(7)?,
  }))? {
    let row = row?;
    if keep(&row.sync_clock) { set.history.push(row); }
  }

  let mut stmt = conn.prepare(
    "SELECT s.uuid, t.uuid, s.created_by, s.action, s.payload, s.reason, s.state, s.created_at, s.resolved_at, s.sync_clock
      FROM suggestions s JOIN tasks t ON t.task_id = s.task_id
      WHERE s.sync_clock > ?1 ORDER BY s.sync_clock",
  )?;
  for row in stmt.query_map(params![since], |r| Ok(SuggestionRow {
    uuid: r.get(0)?, task_uuid: r.get(1)?, created_by: r.get(2)?, action: r.get(3)?, payload: r.get(4)?,
    reason: r.get(5)?, state: r.get(6)?, created_at: r.get(7)?, resolved_at: r.get(8)?, sync_clock: r.get(9)?,
  }))? {
    let row = row?;
    if keep(&row.sync_clock) { set.suggestions.push(row); }
  }

  let mut stmt = conn.prepare(
    "SELECT entity, uuid, sync_clock FROM sync_tombstones WHERE sync_clock > ?1 ORDER BY sync_clock",
  )?;
  for row in stmt.query_map(params![since], |r| Ok(Tombstone { entity: r.get(0)?, uuid: r.get(1)?, sync_clock: r.get(2)? }))? {
    let row = row?;
    if keep(&row.sync_clock) { set.tombstones.push(row); }
  }

  if include_sessions {
    let mut stmt = conn.prepare(
      "SELECT s.session_id, s.agent, p.uuid, s.cwd, s.state, s.started_at, s.last_seen_at, t.uuid
        FROM agent_sessions s
          LEFT JOIN projects p ON p.project_id = s.project_id
          LEFT JOIN tasks t ON t.task_id = s.task_id
        WHERE s.state <> 'ENDED' OR s.last_seen_at > DATETIME('now', 'localtime', '-1 day')",
    )?;
    set.sessions = stmt.query_map([], |r| Ok(SessionRow {
      session_id: r.get(0)?, agent: r.get(1)?, project_uuid: r.get(2)?, cwd: r.get(3)?, state: r.get(4)?,
      started_at: r.get(5)?, last_seen_at: r.get(6)?, task_uuid: r.get(7)?,
    }))?.collect::<rusqlite::Result<Vec<_>>>()?;
  }

  set.cursor = cursor;
  Ok(set)
}

fn local_clock(conn: &Connection, table: &str, uuid: &str) -> Result<Option<(i64, String)>> {
  let pk = match table {
    "projects" => "project_id",
    "tasks" => "task_id",
    "task_history" => "task_history_id",
    "suggestions" => "suggestion_id",
    other => anyhow::bail!("tabela desconhecida: {other}"),
  };
  Ok(conn.query_row(
    &format!("SELECT {pk}, sync_clock FROM {table} WHERE uuid = ?1"),
    params![uuid], |r| Ok((r.get(0)?, r.get(1)?)),
  ).optional()?)
}

/// A linha foi apagada (em algum dispositivo) depois do relógio `clock`?
fn deleted_after(conn: &Connection, entity: &str, uuid: &str, clock: &str) -> Result<bool> {
  let tombstone: Option<String> = conn.query_row(
    "SELECT sync_clock FROM sync_tombstones WHERE entity = ?1 AND uuid = ?2",
    params![entity, uuid], |r| r.get(0),
  ).optional()?;
  Ok(tombstone.is_some_and(|t| t.as_str() >= clock))
}

fn local_id(conn: &Connection, table: &str, pk: &str, uuid: Option<&str>) -> Result<Option<i64>> {
  let Some(uuid) = uuid else { return Ok(None) };
  Ok(conn.query_row(&format!("SELECT {pk} FROM {table} WHERE uuid = ?1"), params![uuid], |r| r.get(0)).optional()?)
}

/// Nome de projeto livre: dois aparelhos podem ter criado projetos diferentes com o mesmo nome.
fn free_project_name(conn: &Connection, name: &str, except_id: Option<i64>) -> Result<String> {
  let mut candidate = name.to_string();
  let mut n = 2;
  loop {
    let taken: Option<i64> = conn.query_row(
      "SELECT project_id FROM projects WHERE project_name = ?1", params![candidate], |r| r.get(0),
    ).optional()?;
    match taken {
      Some(id) if Some(id) != except_id => {
        candidate = format!("{name} ({n})");
        n += 1;
      }
      _ => return Ok(candidate),
    }
  }
}

/// Aplica um conjunto de mudanças vindo de outro dispositivo. Sessões só são aceitas com
/// `accept_sessions` (no celular). Depois arquiva o que estiver concluído antes de hoje.
pub fn apply(db: &Database, set: &ChangeSet, accept_sessions: bool) -> Result<ApplyReport> {
  let mut report = ApplyReport::default();
  if !set.cursor.is_empty() {
    db.observe_clock(&set.cursor);
  }

  let tx = Transaction::new_unchecked(db.conn(), TransactionBehavior::Immediate)?;
  let mut count = |applied: bool| if applied { report.applied += 1 } else { report.skipped += 1 };

  for row in &set.projects {
    let applied = match local_clock(&tx, "projects", &row.uuid)? {
      Some((_, clock)) if clock >= row.sync_clock => false,
      Some((id, _)) => {
        let name = free_project_name(&tx, &row.name, Some(id))?;
        tx.execute(
          "UPDATE projects SET project_name = ?1, agent_policy = ?2, active_window_minutes = ?3, sync_clock = ?4
            WHERE project_id = ?5",
          params![name, row.agent_policy, row.active_window_minutes, row.sync_clock, id],
        )?;
        true
      }
      None if deleted_after(&tx, "project", &row.uuid, &row.sync_clock)? => false,
      None => {
        let name = free_project_name(&tx, &row.name, None)?;
        tx.execute(
          "INSERT INTO projects (project_name, agent_policy, active_window_minutes, created_at, uuid, sync_clock)
            VALUES (?1, ?2, ?3, COALESCE(?4, DATETIME('now', 'localtime')), ?5, ?6)",
          params![name, row.agent_policy, row.active_window_minutes, row.created_at, row.uuid, row.sync_clock],
        )?;
        true
      }
    };
    count(applied);
  }

  for row in &set.tasks {
    let project_id = local_id(&tx, "projects", "project_id", row.project_uuid.as_deref())?;
    let applied = match local_clock(&tx, "tasks", &row.uuid)? {
      Some((_, clock)) if clock >= row.sync_clock => false,
      Some((id, _)) => {
        tx.execute(
          "UPDATE tasks SET task_title = ?1, task_status = ?2, task_is_daily = ?3, updated_at = ?4, due_date = ?5,
              done_at = ?6, project_id = ?7, created_by = ?8, sync_clock = ?9
            WHERE task_id = ?10",
          params![row.title, row.status, row.daily, row.updated_at, row.due_date, row.done_at, project_id,
            row.created_by, row.sync_clock, id],
        )?;
        true
      }
      None if deleted_after(&tx, "task", &row.uuid, &row.sync_clock)? => false,
      None => {
        tx.execute(
          "INSERT INTO tasks (task_title, task_status, task_is_daily, created_at, updated_at, due_date, done_at,
              project_id, created_by, uuid, sync_clock)
            VALUES (?1, ?2, ?3, COALESCE(?4, DATETIME('now', 'localtime')), COALESCE(?5, DATETIME('now', 'localtime')),
              ?6, ?7, ?8, ?9, ?10, ?11)",
          params![row.title, row.status, row.daily, row.created_at, row.updated_at, row.due_date, row.done_at,
            project_id, row.created_by, row.uuid, row.sync_clock],
        )?;
        true
      }
    };
    count(applied);
  }

  // O histórico só cresce: uma linha que já existe não muda.
  for row in &set.history {
    let applied = if local_clock(&tx, "task_history", &row.uuid)?.is_some()
      || deleted_after(&tx, "history", &row.uuid, &row.sync_clock)? {
      false
    } else {
      let project_id = local_id(&tx, "projects", "project_id", row.project_uuid.as_deref())?;
      tx.execute(
        "INSERT INTO task_history (task_history_title, task_history_is_daily, task_history_date, task_history_due_date,
            task_history_project_id, task_history_created_by, uuid, sync_clock)
          VALUES (?1, ?2, COALESCE(?3, DATETIME('now', 'localtime')), ?4, ?5, ?6, ?7, ?8)",
        params![row.title, row.daily, row.date, row.due_date, project_id, row.created_by, row.uuid, row.sync_clock],
      )?;
      true
    };
    count(applied);
  }

  for row in &set.suggestions {
    let Some(task_id) = local_id(&tx, "tasks", "task_id", Some(&row.task_uuid))? else {
      count(false);
      continue;
    };
    let applied = match local_clock(&tx, "suggestions", &row.uuid)? {
      Some((_, clock)) if clock >= row.sync_clock => false,
      Some((id, _)) => {
        tx.execute(
          "UPDATE suggestions SET task_id = ?1, created_by = ?2, action = ?3, payload = ?4, reason = ?5, state = ?6,
              resolved_at = ?7, sync_clock = ?8
            WHERE suggestion_id = ?9",
          params![task_id, row.created_by, row.action, row.payload, row.reason, row.state, row.resolved_at,
            row.sync_clock, id],
        )?;
        true
      }
      None if deleted_after(&tx, "suggestion", &row.uuid, &row.sync_clock)? => false,
      None => {
        tx.execute(
          "INSERT INTO suggestions (task_id, created_by, action, payload, reason, state, created_at, resolved_at, uuid, sync_clock)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE(?7, DATETIME('now', 'localtime')), ?8, ?9, ?10)",
          params![task_id, row.created_by, row.action, row.payload, row.reason, row.state, row.created_at,
            row.resolved_at, row.uuid, row.sync_clock],
        )?;
        true
      }
    };
    count(applied);
  }

  for tombstone in &set.tombstones {
    let table = match tombstone.entity.as_str() {
      "project" => "projects",
      "task" => "tasks",
      "history" => "task_history",
      "suggestion" => "suggestions",
      _ => continue,
    };
    // Uma edição local depois da exclusão remota vence: a linha fica (e volta para o outro lado).
    if let Some((id, clock)) = local_clock(&tx, table, &tombstone.uuid)?
      && clock < tombstone.sync_clock {
        let pk = match table {
          "projects" => "project_id",
          "tasks" => "task_id",
          "task_history" => "task_history_id",
          _ => "suggestion_id",
        };
        tx.execute(&format!("DELETE FROM {table} WHERE {pk} = ?1"), params![id])?;
        report.deleted += 1;
      }
    tx.execute(
      "INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES (?1, ?2, ?3)
        ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = MAX(sync_clock, excluded.sync_clock)",
      params![tombstone.entity, tombstone.uuid, tombstone.sync_clock],
    )?;
  }

  if accept_sessions {
    for row in &set.sessions {
      let project_id = local_id(&tx, "projects", "project_id", row.project_uuid.as_deref())?;
      let task_id = local_id(&tx, "tasks", "task_id", row.task_uuid.as_deref())?;
      tx.execute(
        "INSERT INTO agent_sessions (session_id, agent, project_id, cwd, state, started_at, last_seen_at, task_id)
          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
          ON CONFLICT(session_id) DO UPDATE SET agent = excluded.agent, project_id = excluded.project_id,
            cwd = excluded.cwd, state = excluded.state, last_seen_at = excluded.last_seen_at, task_id = excluded.task_id
          WHERE excluded.last_seen_at >= agent_sessions.last_seen_at",
        params![row.session_id, row.agent, project_id, row.cwd, row.state, row.started_at, row.last_seen_at, task_id],
      )?;
    }
  }

  if accept_sessions {
    crate::usage::import(&tx, &set.usage, dates::now())?;
  }

  tx.commit()?;

  // Uma tarefa concluída no outro aparelho num dia anterior entra no histórico agora.
  report.archived = db.archive_done_before(dates::today())?;
  Ok(report)
}

// ---------- Pares (lado cliente) e token (lado servidor) ----------

pub const META_SYNC_TOKEN: &str = "sync_token";

/// Token que os clientes pareados precisam enviar. Criado na primeira vez.
pub fn server_token(db: &Database) -> Result<String> {
  if let Some(token) = db.get_meta(META_SYNC_TOKEN)? {
    return Ok(token);
  }
  let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
  db.set_meta(META_SYNC_TOKEN, &token)?;
  Ok(token)
}

/// Troca o token, desfazendo todos os pareamentos.
pub fn rotate_server_token(db: &Database) -> Result<String> {
  db.conn().execute("DELETE FROM app_meta WHERE key = ?1", params![META_SYNC_TOKEN])?;
  server_token(db)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Peer {
  pub device: String,
  pub name: String,
  pub url: String,
  pub token: String,
  pub last_pulled: String,
  pub last_pushed: String,
  pub last_synced_at: Option<String>,
}

pub fn save_peer(db: &Database, device: &str, name: &str, url: &str, token: &str) -> Result<()> {
  db.conn().execute(
    "INSERT INTO sync_peers (peer_device, name, url, token) VALUES (?1, ?2, ?3, ?4)
      ON CONFLICT(peer_device) DO UPDATE SET name = excluded.name, url = excluded.url, token = excluded.token",
    params![device, name, url, token],
  )?;
  Ok(())
}

pub fn peers(db: &Database) -> Result<Vec<Peer>> {
  let mut stmt = db.conn().prepare(
    "SELECT peer_device, name, url, token, last_pulled, last_pushed, last_synced_at FROM sync_peers ORDER BY name",
  )?;
  let peers = stmt.query_map([], |r| Ok(Peer {
    device: r.get(0)?, name: r.get(1)?, url: r.get(2)?, token: r.get(3)?, last_pulled: r.get(4)?,
    last_pushed: r.get(5)?, last_synced_at: r.get(6)?,
  }))?.collect::<rusqlite::Result<Vec<_>>>()?;
  Ok(peers)
}

pub fn remove_peer(db: &Database, device: &str) -> Result<()> {
  db.conn().execute("DELETE FROM sync_peers WHERE peer_device = ?1", params![device])?;
  Ok(())
}

pub fn update_peer_cursors(db: &Database, device: &str, last_pulled: &str, last_pushed: &str) -> Result<()> {
  let rows = db.conn().execute(
    "UPDATE sync_peers SET last_pulled = ?1, last_pushed = ?2, last_synced_at = DATETIME('now', 'localtime')
      WHERE peer_device = ?3",
    params![last_pulled, last_pushed, device],
  ).context("Fail to update peer")?;
  if rows == 0 {
    anyhow::bail!("Servidor pareado '{device}' não encontrado");
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::persistence::{NewTask, Policy, SessionState, Status, TaskEdit, USER};

  /// Cursores de um cliente em relação a um servidor, como o `sync_peers` guardaria.
  #[derive(Default)]
  struct Cursors {
    pulled: String,
    pushed: String,
  }

  fn sync(client: &Database, server: &Database, cursors: &mut Cursors) -> (ApplyReport, ApplyReport) {
    let push = changes_since(client, &cursors.pushed, Some(&server.device_id()), false).unwrap();
    let on_server = apply(server, &push, false).unwrap();
    cursors.pushed = push.cursor;

    let pull = changes_since(server, &cursors.pulled, Some(&client.device_id()), true).unwrap();
    let on_client = apply(client, &pull, true).unwrap();
    cursors.pulled = pull.cursor;
    (on_server, on_client)
  }

  fn add(db: &Database, title: &str, project_id: Option<i64>) -> i64 {
    db.add_task(NewTask { title, daily: false, status: Status::Shelf, due_date: None, project_id, created_by: USER }).unwrap()
  }

  fn titles(db: &Database) -> Vec<(String, String, Option<String>)> {
    let mut tasks: Vec<_> = db.get_tasks(crate::persistence::TaskFilter { project_id: None, authors: crate::persistence::Authors::All })
      .unwrap().into_iter().map(|t| (t.title, t.status.as_str().to_string(), t.project_name)).collect();
    tasks.sort();
    tasks
  }

  fn uuid_of(db: &Database, id: i64) -> String {
    db.conn().query_row("SELECT uuid FROM tasks WHERE task_id = ?1", params![id], |r| r.get(0)).unwrap()
  }

  fn id_of(db: &Database, uuid: &str) -> i64 {
    db.conn().query_row("SELECT task_id FROM tasks WHERE uuid = ?1", params![uuid], |r| r.get(0)).unwrap()
  }

  #[test]
  fn writes_are_stamped_by_triggers() {
    let db = Database::open_in_memory().unwrap();
    let id = add(&db, "x", None);
    let read = |db: &Database| -> (String, String) {
      db.conn().query_row("SELECT uuid, sync_clock FROM tasks WHERE task_id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
    };
    let (uuid, first) = read(&db);
    assert_eq!(uuid.len(), 36);
    assert_eq!(clock::device_of(&first), Some(db.device_id().as_str()));

    db.change_task_status(id, Status::Cart).unwrap();
    let (_, second) = read(&db);
    assert!(second > first);

    db.delete_task(id).unwrap();
    let tomb: String = db.conn().query_row("SELECT sync_clock FROM sync_tombstones WHERE uuid = ?1", params![uuid], |r| r.get(0)).unwrap();
    assert!(tomb > second);
  }

  #[test]
  fn two_devices_converge() {
    let pc = Database::open_in_memory().unwrap();
    let phone = Database::open_in_memory().unwrap();
    let mut cursors = Cursors::default();

    let project = pc.add_project("arca", Policy::Own).unwrap();
    let readme = add(&pc, "README", Some(project));
    pc.upsert_session("s1", "claude-code", Some(project), "c:/x", SessionState::Working).unwrap();
    pc.set_session_task("s1", Some(readme)).unwrap();

    sync(&phone, &pc, &mut cursors);
    assert_eq!(titles(&phone), titles(&pc));
    let phone_session = phone.get_sessions().unwrap().remove(0);
    assert_eq!(phone_session.task_title.as_deref(), Some("README"));
    assert_eq!(phone_session.project_name.as_deref(), Some("arca"));

    // Celular fora de casa: edita e cria; PC também cria.
    let on_phone = id_of(&phone, &uuid_of(&pc, readme));
    phone.change_task_status(on_phone, Status::Cart).unwrap();
    add(&phone, "Ideia no ônibus", None);
    add(&pc, "Do PC", Some(project));

    let (on_server, _) = sync(&phone, &pc, &mut cursors);
    assert_eq!(on_server.applied, 2);
    assert_eq!(titles(&phone), titles(&pc));
    assert!(titles(&pc).contains(&("README".into(), "CART".into(), Some("arca".into()))));

    // Sem mudanças, nada viaja (as linhas recebidas não voltam).
    let (again_server, again_client) = sync(&phone, &pc, &mut cursors);
    assert_eq!((again_server.applied, again_client.applied), (0, 0));
  }

  #[test]
  fn last_write_wins_and_deletes_propagate() {
    let pc = Database::open_in_memory().unwrap();
    let phone = Database::open_in_memory().unwrap();
    let mut cursors = Cursors::default();
    let id = add(&pc, "Original", None);
    let gone = add(&pc, "Vai sumir", None);
    sync(&phone, &pc, &mut cursors);

    let phone_id = id_of(&phone, &uuid_of(&pc, id));
    pc.edit_task(id, TaskEdit { title: Some("Do PC"), ..Default::default() }).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    phone.edit_task(phone_id, TaskEdit { title: Some("Do celular, depois"), ..Default::default() }).unwrap();
    phone.delete_task(id_of(&phone, &uuid_of(&pc, gone))).unwrap();

    sync(&phone, &pc, &mut cursors);
    assert_eq!(titles(&pc), vec![("Do celular, depois".into(), "SHELF".into(), None)]);
    assert_eq!(titles(&phone), titles(&pc));
  }

  #[test]
  fn both_rollovers_do_not_duplicate_history() {
    let pc = Database::open_in_memory().unwrap();
    let phone = Database::open_in_memory().unwrap();
    let mut cursors = Cursors::default();
    let id = add(&pc, "Feita ontem", None);
    pc.conn().execute("UPDATE tasks SET task_status = 'DONE', done_at = '2026-10-28 10:00:00' WHERE task_id = ?1", params![id]).unwrap();

    // O celular recebe a tarefa concluída e, ao aplicar, já faz a virada.
    let pull = changes_since(&pc, "", Some(&phone.device_id()), true).unwrap();
    assert_eq!(apply(&phone, &pull, true).unwrap().archived, 1);
    // O PC faz a mesma virada por conta própria, antes de saber do celular.
    assert_eq!(pc.archive_done_before(dates::today()).unwrap(), 1);

    sync(&phone, &pc, &mut cursors);
    assert_eq!(pc.get_history().unwrap().len(), 1);
    assert_eq!(phone.get_history().unwrap().len(), 1);
    assert!(titles(&pc).is_empty() && titles(&phone).is_empty());
  }

  #[test]
  fn project_name_clash_is_renamed() {
    let pc = Database::open_in_memory().unwrap();
    let phone = Database::open_in_memory().unwrap();
    pc.add_project("mestrado", Policy::Own).unwrap();
    phone.add_project("mestrado", Policy::All).unwrap();
    sync(&phone, &pc, &mut Cursors::default());
    let mut names: Vec<_> = pc.get_projects().unwrap().into_iter().map(|p| p.name).collect();
    names.sort();
    assert_eq!(names, vec!["mestrado", "mestrado (2)"]);
  }

  #[test]
  fn suggestions_sync_both_ways() {
    let pc = Database::open_in_memory().unwrap();
    let phone = Database::open_in_memory().unwrap();
    let mut cursors = Cursors::default();
    let id = add(&pc, "Tarefa", None);
    let s = pc.add_suggestion(id, "agent:claude-code", crate::persistence::SuggestionAction::Done, &serde_json::json!({}), Some("pronto")).unwrap();
    sync(&phone, &pc, &mut cursors);

    let on_phone = phone.get_suggestions(true).unwrap().remove(0);
    assert_eq!(on_phone.reason.as_deref(), Some("pronto"));
    crate::suggestions::approve(&phone, on_phone.id).unwrap();

    sync(&phone, &pc, &mut cursors);
    assert_eq!(pc.get_suggestion(s).unwrap().state, crate::persistence::SuggestionState::Approved);
    assert_eq!(pc.get_task(id).unwrap().status, Status::Done);
  }

  #[test]
  fn peers_and_token() {
    let db = Database::open_in_memory().unwrap();
    let token = server_token(&db).unwrap();
    assert_eq!(token.len(), 64);
    assert_eq!(server_token(&db).unwrap(), token);
    assert_ne!(rotate_server_token(&db).unwrap(), token);

    save_peer(&db, "pc-1", "PC", "http://192.168.0.10:7377", "t").unwrap();
    update_peer_cursors(&db, "pc-1", "a", "b").unwrap();
    let peer = peers(&db).unwrap().remove(0);
    assert_eq!((peer.last_pulled.as_str(), peer.last_pushed.as_str()), ("a", "b"));
    assert!(update_peer_cursors(&db, "outro", "a", "b").is_err());
  }
}
