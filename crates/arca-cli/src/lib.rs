pub mod render;
pub mod mcp;

// O núcleo mora em `arca-core`; reexportado para manter os caminhos `crate::persistence` etc.
pub use arca_core::{agents, db_path_solver, dates, paths, persistence, policy, suggestions};

use anyhow::{bail, Context, Result};
use chrono::Duration;
use colored::Colorize;
use std::io::{IsTerminal, Read};

use crate::agents::ActiveWindow;
use crate::persistence::{Authors, Database, NewTask, Policy, Status, Task, TaskEdit, TaskFilter, USER};
use crate::render::ListStyle;


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
  print_pending_hint(db)?;
  Ok(())
}

/// Lembrete discreto quando há sugestões de agentes esperando o usuário.
fn print_pending_hint(db: &Database) -> Result<()> {
  let pending = db.count_pending_suggestions()?;
  if pending > 0 {
    let text = if pending == 1 { "1 sugestão de agente pendente".to_string() } else { format!("{pending} sugestões de agentes pendentes") };
    println!("\n{}", format!("{text}: arca suggestions").yellow());
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

/// Mostra ou muda a janela de "ativo agora" do projeto (`none` volta a usar a global).
pub fn project_window(db: &Database, name: &str, minutes: Option<&str>) -> Result<()> {
  let id = db.project_id(name)?;
  match clearable(minutes, parse_minutes)? {
    None => match db.get_project(id)?.active_window {
      Some(m) => println!("{name}: janela {m} min"),
      None => println!("{name}: usa a janela global ({} min)", agents::global_window(db)?),
    },
    Some(value) => {
      db.set_project_window(id, value)?;
      match value {
        Some(m) => println!("{name}: janela {m} min"),
        None => println!("{name}: voltou a usar a janela global"),
      }
    }
  }
  Ok(())
}

fn parse_minutes(value: &str) -> Result<i64> {
  match value.parse::<i64>() {
    Ok(m) if m > 0 => Ok(m),
    _ => bail!("Janela inválida: '{value}'. Use um número de minutos maior que zero ou none"),
  }
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

pub use arca_core::agents::{agent_author, agent_name, Ping};

/// Registra um sinal de vida de uma sessão de agente. Pensado para hooks:
/// lê o JSON do hook no stdin (`session_id`, `cwd`, `hook_event_name`) e não escreve nada no stdout.
pub fn ping(db: &Database, ping: Ping) -> Result<()> {
  agents::record_ping(db, ping, &read_hook_input()?)
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

/// Painel de atividade dos agentes por projeto.
pub fn agents(db: &Database, all: bool, window: Option<i64>) -> Result<()> {
  let window = ActiveWindow::load(db, window)?;
  let now = dates::now();
  render::render_sessions(&agents::recent_sessions(db, all, now)?, now, window);
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

  let window = ActiveWindow::load(db, options.window)?;
  let now = dates::now();
  let sessions: Vec<_> = agents::recent_sessions(db, false, now)?.into_iter()
    .filter(|s| project.as_ref().is_none_or(|p| s.project_name.as_deref() == Some(p.name.as_str())))
    .collect();
  if !sessions.is_empty() {
    println!("\n{}", "Agentes".bold());
    render::render_sessions(&sessions, now, window);
  }

  let pending = db.get_suggestions(true)?;
  if !pending.is_empty() {
    println!("\n{}", format!("Sugestões pendentes ({})", pending.len()).bold());
    render::render_suggestions(&pending, now);
    println!("{}", "arca approve <n> / arca reject <n>".dimmed());
  }
  Ok(())
}

// ---------- Sugestões ----------

pub fn list_suggestions(db: &Database, all: bool) -> Result<()> {
  render::render_suggestions(&db.get_suggestions(!all)?, dates::now());
  Ok(())
}

pub fn approve(db: &Database, id: i64) -> Result<()> {
  let suggestion = suggestions::approve(db, id)?;
  println!("{} {} #{} {}", "Aprovada:".green(), suggestions::describe(&suggestion), suggestion.task_id, suggestion.task_title);
  Ok(())
}

pub fn reject(db: &Database, id: i64) -> Result<()> {
  let suggestion = suggestions::reject(db, id)?;
  println!("{} {} #{} {}", "Rejeitada:".red(), suggestions::describe(&suggestion), suggestion.task_id, suggestion.task_title);
  Ok(())
}

pub fn config_active_window(db: &Database, minutes: Option<i64>) -> Result<()> {
  if let Some(m) = minutes {
    agents::set_global_window(db, m)?;
  }
  println!("active-window = {} min", agents::global_window(db)?);
  Ok(())
}

// ---------- Sincronização ----------

fn print_qr(text: &str) -> Result<()> {
  use qrcode::render::unicode::Dense1x2;
  let code = qrcode::QrCode::new(text.as_bytes())?;
  // Cores invertidas: em terminal escuro, o "claro" precisa ser o fundo do QR.
  let image = code.render::<Dense1x2>()
    .dark_color(Dense1x2::Light)
    .light_color(Dense1x2::Dark)
    .quiet_zone(true)
    .build();
  println!("{image}");
  Ok(())
}

/// Servidor de sincronização na rede de casa. Mostra o QR code de pareamento e fica no ar até Ctrl+C.
pub fn serve(db: Database, bind: &str, port: u16, show_qr: bool) -> Result<()> {
  use arca_core::net::server::{self, Event};
  use std::sync::atomic::AtomicBool;
  use std::sync::Arc;

  let pairing = server::pairing(&db, port)?;
  if pairing.urls.is_empty() {
    eprintln!("{}", "Nenhum endereço de rede local encontrado; o celular não vai achar este PC.".yellow());
  }
  println!("{} {} {}", "Arca sync".bold(), pairing.name.magenta(), format!("({})", pairing.device).dimmed());
  for url in &pairing.urls {
    println!("  {url}");
  }
  if show_qr {
    println!("\nLeia este QR code no app do celular para parear:");
    print_qr(&pairing.to_json())?;
  }
  println!("Código de pareamento (para `arca sync pair` em outro PC):\n{}\n", pairing.to_json().dimmed());
  println!("{}", "Ctrl+C para parar. No primeiro uso, o Windows pode pedir para liberar o firewall.".dimmed());

  let stop = Arc::new(AtomicBool::new(false));
  server::serve(db, bind, port, stop, |event| {
    let time = dates::now().format("%H:%M:%S").to_string().dimmed();
    match event {
      Event::Pushed { client, name, report } => println!("{time} {} recebidas de {} ({} aplicadas, {} ignoradas, {} apagadas)",
        "↓".green(), name.unwrap_or_else(|| short_device(&client)), report.applied, report.skipped, report.deleted),
      Event::Pulled { client, name, rows } => println!("{time} {} {rows} linha(s) enviadas para {}", "↑".cyan(), name.unwrap_or_else(|| short_device(&client))),
      Event::Rejected { reason } => println!("{time} {} {reason}", "✗".red()),
    }
  })
}

fn short_device(device: &str) -> String {
  device.chars().take(8).collect()
}

/// Pareia com um servidor a partir do código (o JSON do QR), ou de `@arquivo`.
pub fn sync_pair(db: &Database, code: &str) -> Result<()> {
  let text = match code.strip_prefix('@') {
    Some(path) => std::fs::read_to_string(path).with_context(|| format!("Não consegui ler {path}"))?,
    None => code.to_string(),
  };
  let pairing = arca_core::net::Pairing::from_json(&text)?;
  let peer = arca_core::net::client::pair(db, &pairing)?;
  println!("Pareado com {} ({}).", peer.name.magenta(), peer.url.split_whitespace().next().unwrap_or(""));
  sync_now(db)
}

/// Sincroniza com todos os servidores pareados.
pub fn sync_now(db: &Database) -> Result<()> {
  let peers = arca_core::sync::peers(db)?;
  if peers.is_empty() {
    bail!("Nenhum servidor pareado. Rode `arca serve` no PC e use `arca sync pair <código>`.");
  }
  let mut failed = false;
  for peer in peers {
    match arca_core::net::client::sync_with(db, &peer) {
      Ok(report) => println!("{} {}: enviadas {} (aplicadas {}), recebidas {} (aplicadas {}, apagadas {}, arquivadas {})",
        "✓".green(), peer.name.magenta(),
        report.pushed.applied + report.pushed.skipped, report.pushed.applied,
        report.pulled.applied + report.pulled.skipped, report.pulled.applied, report.pulled.deleted, report.pulled.archived),
      Err(error) => {
        failed = true;
        eprintln!("{} {}: {error:#}", "✗".red(), peer.name.magenta());
      }
    }
  }
  if failed {
    bail!("Alguma sincronização falhou");
  }
  Ok(())
}

pub fn sync_peers(db: &Database) -> Result<()> {
  let peers = arca_core::sync::peers(db)?;
  if peers.is_empty() {
    println!("{}", "Nenhum servidor pareado.".dimmed());
  }
  for peer in peers {
    println!("{} {}  {}  última sincronização: {}",
      peer.name.magenta(), short_device(&peer.device).dimmed(),
      peer.url.split_whitespace().next().unwrap_or(""),
      peer.last_synced_at.as_deref().unwrap_or("nunca"));
  }
  Ok(())
}

pub fn sync_unpair(db: &Database, who: &str) -> Result<()> {
  let peer = arca_core::sync::peers(db)?.into_iter()
    .find(|p| p.device == who || p.device.starts_with(who) || p.name.eq_ignore_ascii_case(who))
    .with_context(|| format!("Nenhum servidor pareado chamado '{who}' (veja `arca sync peers`)"))?;
  arca_core::sync::remove_peer(db, &peer.device)?;
  println!("Pareamento com {} removido.", peer.name);
  Ok(())
}

pub fn sync_rotate_token(db: &Database) -> Result<()> {
  arca_core::sync::rotate_server_token(db)?;
  println!("Token trocado: os aparelhos pareados com este PC precisam parear de novo.");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

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
