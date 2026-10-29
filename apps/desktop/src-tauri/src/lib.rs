//! Backend do app desktop: comandos finos sobre `arca_core::api` e o servidor de sincronização,
//! que roda numa thread com conexão própria enquanto o app está aberto.
//! O banco é o mesmo da CLI, então hooks, MCP e app enxergam as mesmas tarefas.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use arca_core::api::{self, Board, BoardFilter, HistoryView, ProjectChanges, TaskChanges, TaskInput};
use arca_core::net::{self, server};
use arca_core::persistence::Database;
use serde::Serialize;
use tauri::State;

const META_SYNC_ENABLED: &str = "desktop_sync_enabled";
const MAX_LOG: usize = 50;

struct SyncServer {
  stop: Arc<AtomicBool>,
  port: u16,
}

struct AppState {
  db_path: PathBuf,
  db: Mutex<Database>,
  server: Mutex<Option<SyncServer>>,
  log: Arc<Mutex<Vec<String>>>,
  last_error: Arc<Mutex<Option<String>>>,
}

type CmdResult<T> = Result<T, String>;

fn err(error: impl std::fmt::Display) -> String {
  format!("{error:#}")
}

impl AppState {
  fn with_db<T>(&self, f: impl FnOnce(&Database) -> anyhow::Result<T>) -> CmdResult<T> {
    let db = self.db.lock().map_err(|_| "banco indisponível".to_string())?;
    f(&db).map_err(|e| format!("{e:#}"))
  }

  fn push_log(log: &Mutex<Vec<String>>, line: String) {
    if let Ok(mut log) = log.lock() {
      log.push(line);
      let overflow = log.len().saturating_sub(MAX_LOG);
      log.drain(..overflow);
    }
  }

  fn start_server(&self, port: u16) -> CmdResult<()> {
    let mut slot = self.server.lock().map_err(err)?;
    if slot.is_some() {
      return Ok(());
    }
    let db = Database::init(&self.db_path).map_err(err)?;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let log = Arc::clone(&self.log);
    let last_error = Arc::clone(&self.last_error);
    if let Ok(mut e) = last_error.lock() {
      *e = None;
    }

    std::thread::spawn(move || {
      let now = || arca_core::dates::now().format("%H:%M:%S").to_string();
      let result = server::serve(db, "0.0.0.0", port, thread_stop, |event| {
        let line = match event {
          server::Event::Pushed { client, report } => format!("{} recebidas de {}: {} aplicadas", now(), &client[..client.len().min(8)], report.applied),
          server::Event::Pulled { client, rows } => format!("{} {rows} enviadas para {}", now(), &client[..client.len().min(8)]),
          server::Event::Rejected { reason } => format!("{} recusada: {reason}", now()),
        };
        AppState::push_log(&log, line);
      });
      if let Err(error) = result
        && let Ok(mut e) = last_error.lock() {
          *e = Some(format!("{error:#}"));
        }
    });

    *slot = Some(SyncServer { stop, port });
    Ok(())
  }

  fn stop_server(&self) -> CmdResult<()> {
    if let Some(server) = self.server.lock().map_err(err)?.take() {
      server.stop.store(true, Ordering::Relaxed);
    }
    Ok(())
  }
}

// ---------- Board e tarefas ----------

#[tauri::command]
fn board(state: State<AppState>, filter: BoardFilter) -> CmdResult<Board> {
  state.with_db(|db| api::board(db, &filter))
}

#[tauri::command]
fn add_task(state: State<AppState>, input: TaskInput) -> CmdResult<i64> {
  state.with_db(|db| api::add_task(db, &input))
}

#[tauri::command]
fn edit_task(state: State<AppState>, id: i64, changes: TaskChanges) -> CmdResult<()> {
  state.with_db(|db| api::edit_task(db, id, &changes))
}

#[tauri::command]
fn move_task(state: State<AppState>, id: i64, status: String) -> CmdResult<()> {
  state.with_db(|db| api::move_task(db, id, &status))
}

#[tauri::command]
fn delete_task(state: State<AppState>, id: i64) -> CmdResult<()> {
  state.with_db(|db| api::delete_task(db, id))
}

#[tauri::command]
fn approve_suggestion(state: State<AppState>, id: i64) -> CmdResult<()> {
  state.with_db(|db| api::approve_suggestion(db, id))
}

#[tauri::command]
fn reject_suggestion(state: State<AppState>, id: i64) -> CmdResult<()> {
  state.with_db(|db| api::reject_suggestion(db, id))
}

#[tauri::command]
fn history(state: State<AppState>) -> CmdResult<Vec<HistoryView>> {
  state.with_db(api::history)
}

// ---------- Projetos ----------

#[tauri::command]
fn add_project(state: State<AppState>, name: String, policy: String) -> CmdResult<i64> {
  state.with_db(|db| api::add_project(db, &name, &policy))
}

#[tauri::command]
fn edit_project(state: State<AppState>, id: i64, changes: ProjectChanges) -> CmdResult<()> {
  state.with_db(|db| api::edit_project(db, id, &changes))
}

#[tauri::command]
fn delete_project(state: State<AppState>, id: i64) -> CmdResult<()> {
  state.with_db(|db| api::delete_project(db, id))
}

#[tauri::command]
fn link_folder(state: State<AppState>, id: i64, folder: String) -> CmdResult<()> {
  state.with_db(|db| api::link_folder(db, id, &folder))
}

#[tauri::command]
fn unlink_folder(state: State<AppState>, folder: String) -> CmdResult<()> {
  state.with_db(|db| api::unlink_folder(db, &folder))
}

// ---------- Sincronização ----------

#[derive(Serialize)]
struct SyncStatus {
  running: bool,
  port: u16,
  name: String,
  device: String,
  urls: Vec<String>,
  log: Vec<String>,
  error: Option<String>,
}

#[tauri::command]
fn sync_status(state: State<AppState>) -> CmdResult<SyncStatus> {
  let server = state.server.lock().map_err(err)?;
  let port = server.as_ref().map(|s| s.port).unwrap_or(net::DEFAULT_PORT);
  Ok(SyncStatus {
    running: server.is_some(),
    port,
    name: net::host_name(),
    device: state.with_db(|db| Ok(db.device_id()))?,
    urls: server::lan_urls(port),
    log: state.log.lock().map(|l| l.clone()).unwrap_or_default(),
    error: state.last_error.lock().ok().and_then(|e| e.clone()),
  })
}

#[tauri::command]
fn set_sync_enabled(state: State<AppState>, enabled: bool) -> CmdResult<()> {
  state.with_db(|db| db.set_meta(META_SYNC_ENABLED, if enabled { "1" } else { "0" }))?;
  if enabled { state.start_server(net::DEFAULT_PORT) } else { state.stop_server() }
}

#[derive(Serialize)]
struct PairingView {
  /// QR code em SVG, pronto para `innerHTML`.
  svg: String,
  /// O mesmo conteúdo em texto, para `arca sync pair`.
  code: String,
}

#[tauri::command]
fn pairing(state: State<AppState>) -> CmdResult<PairingView> {
  let port = state.server.lock().map_err(err)?.as_ref().map(|s| s.port).unwrap_or(net::DEFAULT_PORT);
  let code = state.with_db(|db| server::pairing(db, port))?.to_json();
  let svg = qrcode::QrCode::new(code.as_bytes()).map_err(err)?
    .render::<qrcode::render::svg::Color>()
    .min_dimensions(240, 240)
    .quiet_zone(true)
    .build();
  Ok(PairingView { svg, code })
}

#[tauri::command]
fn rotate_token(state: State<AppState>) -> CmdResult<()> {
  state.with_db(|db| arca_core::sync::rotate_server_token(db).map(|_| ()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let db_path = arca_core::db_path_solver::get_db_path().expect("diretório de dados indisponível");
  let db = Database::init(&db_path).expect("não foi possível abrir o banco do Arca");
  let sync_enabled = db.get_meta(META_SYNC_ENABLED).ok().flatten().as_deref() != Some("0");

  let state = AppState {
    db_path,
    db: Mutex::new(db),
    server: Mutex::new(None),
    log: Arc::new(Mutex::new(Vec::new())),
    last_error: Arc::new(Mutex::new(None)),
  };
  if sync_enabled {
    let _ = state.start_server(net::DEFAULT_PORT);
  }

  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .manage(state)
    .invoke_handler(tauri::generate_handler![
      board, add_task, edit_task, move_task, delete_task, approve_suggestion, reject_suggestion, history,
      add_project, edit_project, delete_project, link_folder, unlink_folder,
      sync_status, set_sync_enabled, pairing, rotate_token,
    ])
    .run(tauri::generate_context!())
    .expect("erro ao iniciar o app");
}
