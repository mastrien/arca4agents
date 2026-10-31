//! API exposta ao Dart via flutter_rust_bridge: uma camada fina sobre `arca_core::api`.
//! Estruturas compostas viajam como JSON (o Dart tem os modelos em `lib/models.dart`),
//! para não espelhar cada tipo do núcleo no bridge.

use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use arca_core::api::{self, BoardFilter, ProjectChanges, TaskChanges, TaskInput};
use arca_core::net::{client, Pairing};
use arca_core::persistence::Database;
use arca_core::sync;

static DB: OnceLock<Mutex<Database>> = OnceLock::new();

fn with_db<T>(f: impl FnOnce(&Database) -> Result<T>) -> Result<T> {
  let db = DB.get().context("banco não aberto: chame open_database primeiro")?;
  let db = db.lock().map_err(|_| anyhow::anyhow!("banco indisponível"))?;
  f(&db)
}

fn json<T: serde::Serialize>(value: &T) -> Result<String> {
  Ok(serde_json::to_string(value)?)
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
  flutter_rust_bridge::setup_default_user_utils();
}

/// Abre (ou cria) o banco no diretório de documentos do app. Chamar uma vez, no início.
pub fn open_database(path: String) -> Result<String> {
  if DB.get().is_none() {
    let db = Database::init(&path)?;
    let _ = DB.set(Mutex::new(db));
  }
  with_db(|db| Ok(db.device_id()))
}

/// `Board` em JSON. `authors`: `user`, `all` ou `agents`.
pub fn board(project: Option<String>, authors: Option<String>) -> Result<String> {
  with_db(|db| json(&api::board(db, &BoardFilter { project, authors })?))
}

/// `input`: JSON de `TaskInput` (`title`, `due`, `project`, `daily`, `status`).
pub fn add_task(input: String) -> Result<i64> {
  let input: TaskInput = serde_json::from_str(&input)?;
  with_db(|db| api::add_task(db, &input))
}

/// `changes`: JSON de `TaskChanges`; campos ausentes não mudam, `""` remove prazo/projeto.
pub fn edit_task(id: i64, changes: String) -> Result<()> {
  let changes: TaskChanges = serde_json::from_str(&changes)?;
  with_db(|db| api::edit_task(db, id, &changes))
}

pub fn move_task(id: i64, status: String) -> Result<()> {
  with_db(|db| api::move_task(db, id, &status))
}

pub fn delete_task(id: i64) -> Result<()> {
  with_db(|db| api::delete_task(db, id))
}

pub fn approve_suggestion(id: i64) -> Result<()> {
  with_db(|db| api::approve_suggestion(db, id))
}

pub fn reject_suggestion(id: i64) -> Result<()> {
  with_db(|db| api::reject_suggestion(db, id))
}

pub fn history() -> Result<String> {
  with_db(|db| json(&api::history(db)?))
}

/// Fotos diárias de estante/carrinho (`[{date, shelf, cart}]`), para os gráficos.
pub fn status_snapshots() -> Result<String> {
  with_db(|db| json(&api::status_snapshots(db)?))
}

pub fn add_project(name: String, policy: String) -> Result<i64> {
  with_db(|db| api::add_project(db, &name, &policy))
}

pub fn edit_project(id: i64, changes: String) -> Result<()> {
  let changes: ProjectChanges = serde_json::from_str(&changes)?;
  with_db(|db| api::edit_project(db, id, &changes))
}

pub fn delete_project(id: i64) -> Result<()> {
  with_db(|db| api::delete_project(db, id))
}

// ---------- Sincronização ----------

/// Nome com que o celular aparece no PC ("Pixel 8").
pub fn set_device_name(name: String) -> Result<()> {
  with_db(|db| arca_core::net::set_device_name(db, &name))
}

/// Pareia com o PC a partir do conteúdo do QR code. Devolve o `Peer` em JSON.
pub fn pair(code: String) -> Result<String> {
  let pairing = Pairing::from_json(&code)?;
  with_db(|db| json(&client::pair(db, &pairing)?))
}

/// Servidores pareados (`Vec<Peer>` em JSON).
pub fn peers() -> Result<String> {
  with_db(|db| json(&sync::peers(db)?))
}

pub fn unpair(device: String) -> Result<()> {
  with_db(|db| sync::remove_peer(db, &device))
}

/// Sincroniza com todos os servidores pareados. Devolve, em JSON, uma lista de
/// `{ "peer": nome, "ok": bool, "report": SyncReport | null, "error": texto | null }`.
pub fn sync_now() -> Result<String> {
  // Segura o banco durante toda a troca (push, pull e aplicação). As chamadas do Dart rodam em
  // outra thread, então a interface não trava; outras chamadas só esperam a sincronização acabar.
  with_db(|db| {
    let results: Vec<serde_json::Value> = sync::peers(db)?.iter().map(|peer| match client::sync_with(db, peer) {
      Ok(report) => serde_json::json!({ "peer": peer.name, "ok": true, "report": report, "error": null }),
      Err(error) => serde_json::json!({ "peer": peer.name, "ok": false, "report": null, "error": format!("{error:#}") }),
    }).collect();
    json(&results)
  })
}
