//! `arca mcp`: servidor MCP (stdio) para agentes de IA lerem e gerenciarem tarefas.
//! Todas as ferramentas operam por id e passam pela política do projeto (`policy.rs`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::persistence::{
  Authors, Database, NewTask, Policy, Project, SessionState, Status, SuggestionAction, Task,
  TaskEdit, TaskFilter,
};
use crate::{dates, paths, policy};

const INSTRUCTIONS: &str = "\
Arca é o gerenciador de tarefas do usuário. Estante (SHELF) é o backlog que ele decidiu não priorizar agora; \
carrinho (CART) é o que ele quer manter por perto (não significa \"em andamento\"); DONE é concluída.
Quando o usuário pedir para anotar algo para fazer depois, use add_task em vez de criar um arquivo Markdown.
Chame project_for_cwd com o diretório de trabalho da sessão para descobrir o projeto e a política antes de criar tarefas.
Políticas: NONE = agente só lê; OWN (padrão, e também para tarefas sem projeto) = agente só altera as tarefas que ele criou; \
ALL = agente gerencia todas as tarefas do projeto. O campo can_modify de cada tarefa diz se você pode alterá-la.
Quando a política não deixar você alterar uma tarefa (can_modify = false), use suggest_change para propor a mudança: \
o usuário aprova ou rejeita.
Ao começar a trabalhar numa tarefa, chame set_working_task com o id dela; isso aparece no painel do usuário.
Prazos (due) são só referência: AAAA-MM-DD, hoje, amanhã ou +Nd.";

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum StatusParam {
  Cart,
  Shelf,
  Done,
}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlaceParam {
  Cart,
  Shelf,
}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum StateParam {
  Working,
  Waiting,
  Ended,
}

impl From<StatusParam> for Status {
  fn from(value: StatusParam) -> Self {
    match value {
      StatusParam::Cart => Status::Cart,
      StatusParam::Shelf => Status::Shelf,
      StatusParam::Done => Status::Done,
    }
  }
}

impl From<PlaceParam> for Status {
  fn from(value: PlaceParam) -> Self {
    match value {
      PlaceParam::Cart => Status::Cart,
      PlaceParam::Shelf => Status::Shelf,
    }
  }
}

impl From<StateParam> for SessionState {
  fn from(value: StateParam) -> Self {
    match value {
      StateParam::Working => SessionState::Working,
      StateParam::Waiting => SessionState::Waiting,
      StateParam::Ended => SessionState::Ended,
    }
  }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectForCwdParams {
  #[schemars(description = "Diretório de trabalho da sessão (caminho absoluto)")]
  pub cwd: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListTasksParams {
  #[schemars(description = "Nome do projeto; omita para todos")]
  pub project: Option<String>,
  #[schemars(description = "Filtra por status")]
  pub status: Option<StatusParam>,
  #[schemars(description = "Incluir as tarefas criadas pelo usuário (padrão: true). false mostra só as de agentes")]
  pub include_user_tasks: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddTaskParams {
  pub title: String,
  #[schemars(description = "Nome do projeto (veja project_for_cwd); omita para tarefa sem projeto")]
  pub project: Option<String>,
  #[schemars(description = "Prazo de referência: AAAA-MM-DD, hoje, amanhã ou +Nd")]
  pub due: Option<String>,
  #[schemars(description = "SHELF (padrão: anotar para depois) ou CART")]
  pub status: Option<PlaceParam>,
  #[schemars(description = "Tarefa diária (volta a pendente a cada dia)")]
  pub daily: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditTaskParams {
  pub id: i64,
  pub title: Option<String>,
  #[schemars(description = "Novo prazo (AAAA-MM-DD, hoje, amanhã, +Nd) ou \"none\" para remover")]
  pub due: Option<String>,
  pub daily: Option<bool>,
  #[schemars(description = "Novo projeto ou \"none\" para deixar sem projeto")]
  pub project: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveTaskParams {
  pub id: i64,
  #[schemars(description = "CART ou SHELF")]
  pub to: PlaceParam,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TaskIdParams {
  pub id: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum SuggestActionParam {
  Done,
  Move,
  Edit,
  Delete,
}

impl From<SuggestActionParam> for SuggestionAction {
  fn from(value: SuggestActionParam) -> Self {
    match value {
      SuggestActionParam::Done => SuggestionAction::Done,
      SuggestActionParam::Move => SuggestionAction::Move,
      SuggestActionParam::Edit => SuggestionAction::Edit,
      SuggestActionParam::Delete => SuggestionAction::Delete,
    }
  }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SuggestChangeParams {
  pub id: i64,
  #[schemars(description = "DONE (concluir), MOVE (mover, exige to), EDIT (editar campos) ou DELETE (apagar)")]
  pub action: SuggestActionParam,
  #[schemars(description = "Para MOVE: CART ou SHELF")]
  pub to: Option<PlaceParam>,
  #[schemars(description = "Para EDIT: novo título")]
  pub title: Option<String>,
  #[schemars(description = "Para EDIT: novo prazo (AAAA-MM-DD, hoje, amanhã, +Nd) ou \"none\" para remover")]
  pub due: Option<String>,
  #[schemars(description = "Para EDIT: tornar diária ou não")]
  pub daily: Option<bool>,
  #[schemars(description = "Para EDIT: novo projeto ou \"none\" para deixar sem projeto")]
  pub project: Option<String>,
  #[schemars(description = "Por que você sugere isso (mostrado ao usuário)")]
  pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSuggestionsParams {
  #[schemars(description = "Só as pendentes (padrão: false, mostra também as aprovadas e rejeitadas)")]
  pub only_pending: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetWorkingTaskParams {
  #[schemars(description = "Id da tarefa em que esta sessão está trabalhando; omita para limpar")]
  pub id: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportActivityParams {
  #[schemars(description = "WORKING enquanto trabalha, WAITING ao terminar e esperar o usuário, ENDED ao encerrar")]
  pub state: StateParam,
  #[schemars(description = "Diretório de trabalho da sessão; padrão: o do servidor")]
  pub cwd: Option<String>,
}

type ToolResult = Result<String, String>;

#[derive(Clone)]
pub struct ArcaServer {
  db: Arc<Mutex<Database>>,
  /// Nome do agente (ex.: `claude-code`).
  agent: String,
  /// `created_by` das tarefas deste agente (ex.: `agent:claude-code`).
  author: String,
  /// Sessão própria do servidor (uma por processo), usada quando não há sessão de hooks.
  session_id: String,
  /// Se a sessão própria chegou a ser registrada; nesse caso ela é encerrada quando o servidor sai.
  reported: Arc<AtomicBool>,
  /// Último diretório de trabalho conhecido da sessão (o do servidor, até o agente informar outro).
  cwd: Arc<Mutex<String>>,
  #[expect(dead_code, reason = "lido pelo código gerado por #[tool_handler]")]
  tool_router: ToolRouter<Self>,
}

fn to_text(value: Value) -> ToolResult {
  serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

fn project_json(project: &Project) -> Value {
  json!({
    "name": project.name,
    "policy": project.policy.as_str(),
    "folders": project.folders,
    "shelf": project.shelf,
    "cart": project.cart,
    "done": project.done,
    "active_window_minutes": project.active_window,
  })
}

impl ArcaServer {
  pub fn new(db: Database, agent: String) -> Self {
    Self {
      db: Arc::new(Mutex::new(db)),
      author: crate::agent_author(&agent),
      session_id: format!("mcp:{agent}:{}", std::process::id()),
      agent,
      reported: Arc::new(AtomicBool::new(false)),
      cwd: Arc::new(Mutex::new(
        std::env::current_dir().ok()
          .and_then(|d| paths::normalize(&d.to_string_lossy()).ok())
          .unwrap_or_default(),
      )),
      tool_router: Self::tool_router(),
    }
  }

  /// Abre o banco para uma ferramenta, fazendo antes a virada do dia se for preciso
  /// (o servidor pode ficar aberto de um dia para o outro). Toda chamada conta como
  /// sinal de vida da sessão, o que cobre agentes sem hooks.
  fn with_db<T>(&self, f: impl FnOnce(&Database) -> anyhow::Result<T>) -> Result<T, String> {
    let db = self.db.lock().map_err(|_| "banco indisponível (lock envenenado)".to_string())?;
    db.rollover_if_needed(dates::today()).map_err(|e| format!("{e:#}"))?;
    if let Ok(session) = self.activity_session(&db) {
      let _ = db.touch_session(&session);
    }
    f(&db).map_err(|e| format!("{e:#}"))
  }

  fn current_cwd(&self) -> String {
    self.cwd.lock().map(|c| c.clone()).unwrap_or_default()
  }

  /// Guarda o diretório informado pelo agente, que é mais confiável que o do servidor.
  fn remember_cwd(&self, cwd: &str) {
    if let (Ok(normalized), Ok(mut current)) = (paths::normalize(cwd), self.cwd.lock()) {
      *current = normalized;
    }
  }

  /// Sessão à qual a atividade deste servidor pertence: a sessão aberta dos hooks do mesmo
  /// agente na mesma pasta (ou, senão, no mesmo projeto); sem ela, a sessão própria do servidor.
  fn activity_session(&self, db: &Database) -> anyhow::Result<String> {
    let cwd = self.current_cwd();
    if let Some(id) = db.find_open_session(&self.agent, &cwd)? {
      return Ok(id);
    }

    let project_id = db.project_for_path(&cwd)?;
    if let Some(project_id) = project_id {
      let project = db.get_project(project_id)?.name;
      let same_project = db.get_sessions()?.into_iter().find(|s| {
        s.agent == self.agent
          && s.state != SessionState::Ended
          && !s.session_id.starts_with("mcp:")
          && s.project_name.as_deref() == Some(project.as_str())
      });
      if let Some(session) = same_project {
        return Ok(session.session_id);
      }
    }

    if !self.reported.load(Ordering::Relaxed) {
      db.upsert_session(&self.session_id, &self.agent, project_id, &cwd, SessionState::Working)?;
      self.reported.store(true, Ordering::Relaxed);
    }
    Ok(self.session_id.clone())
  }

  fn task_json(&self, task: &Task, today: chrono::NaiveDate) -> Value {
    let can_modify = policy::can_modify(policy::effective(task.project_policy), &self.author, task).is_ok();
    json!({
      "id": task.id,
      "title": task.title,
      "status": task.status.as_str(),
      "daily": task.daily,
      "due_date": task.due_date.map(|d| d.to_string()),
      "due": task.due_date.map(|d| dates::humanize_due(d, today)),
      "project": task.project_name,
      "created_by": task.created_by,
      "can_modify": can_modify,
    })
  }

  /// Carrega a tarefa e confere se este agente pode alterá-la.
  fn modifiable_task(&self, db: &Database, id: i64) -> anyhow::Result<Task> {
    let task = db.get_task(id)?;
    let policy = policy::effective(task.project_policy);
    policy::can_modify(policy, &self.author, &task).map_err(|reason| {
      if policy == Policy::Own {
        anyhow::anyhow!("{reason}. Use suggest_change para propor a mudança ao usuário.")
      } else {
        anyhow::Error::msg(reason)
      }
    })?;
    Ok(task)
  }

  /// Resolve um projeto de destino e confere se o agente pode criar tarefas nele.
  fn writable_project(db: &Database, name: &str) -> anyhow::Result<i64> {
    let id = db.project_id(name)?;
    let project = db.get_project(id)?;
    policy::can_create(project.policy, Some(&project.name)).map_err(anyhow::Error::msg)?;
    Ok(id)
  }
}

#[tool_router]
impl ArcaServer {
  #[tool(description = "Descobre o projeto de um diretório (pasta vinculada mais específica), a política de agentes e o resumo de tarefas.")]
  fn project_for_cwd(&self, Parameters(p): Parameters<ProjectForCwdParams>) -> ToolResult {
    self.remember_cwd(&p.cwd);
    let value = self.with_db(|db| {
      let cwd = paths::normalize(&p.cwd)?;
      Ok(match db.project_for_path(&cwd)? {
        Some(id) => json!({ "cwd": cwd, "project": project_json(&db.get_project(id)?) }),
        None => json!({
          "cwd": cwd,
          "project": null,
          "policy": Policy::Own.as_str(),
          "note": "Nenhum projeto vinculado a esta pasta; tarefas sem projeto seguem a política OWN.",
        }),
      })
    })?;
    to_text(value)
  }

  #[tool(description = "Lista os projetos com política, pastas vinculadas e contagem de tarefas.")]
  fn list_projects(&self) -> ToolResult {
    let projects = self.with_db(|db| db.get_projects())?;
    to_text(Value::Array(projects.iter().map(project_json).collect()))
  }

  #[tool(description = "Lista tarefas (de agentes e, por padrão, também do usuário). Cada tarefa traz can_modify.")]
  fn list_tasks(&self, Parameters(p): Parameters<ListTasksParams>) -> ToolResult {
    let today = dates::today();
    let tasks = self.with_db(|db| {
      let project_id = p.project.as_deref().map(|name| db.project_id(name)).transpose()?;
      let authors = if p.include_user_tasks.unwrap_or(true) { Authors::All } else { Authors::Agents };
      let status = p.status.map(Status::from);
      Ok(db.get_tasks(TaskFilter { project_id, authors })?
        .into_iter()
        .filter(|t| status.is_none_or(|s| t.status == s))
        .collect::<Vec<_>>())
    })?;
    to_text(Value::Array(tasks.iter().map(|t| self.task_json(t, today)).collect()))
  }

  #[tool(description = "Cria uma tarefa em nome deste agente. Use para anotar algo para fazer depois (vai para a estante por padrão).")]
  fn add_task(&self, Parameters(p): Parameters<AddTaskParams>) -> ToolResult {
    let today = dates::today();
    let task = self.with_db(|db| {
      // Sem projeto vale OWN, que sempre permite criar.
      let project_id = p.project.as_deref().map(|name| Self::writable_project(db, name)).transpose()?;
      let due_date = p.due.as_deref().map(|d| dates::parse_due(d, today)).transpose()?;
      let id = db.add_task(NewTask {
        title: &p.title,
        daily: p.daily.unwrap_or(false),
        status: p.status.map(Status::from).unwrap_or(Status::Shelf),
        due_date,
        project_id,
        created_by: &self.author,
      })?;
      db.get_task(id)
    })?;
    to_text(self.task_json(&task, today))
  }

  #[tool(description = "Edita título, prazo, diária ou projeto de uma tarefa (respeitando a política).")]
  fn edit_task(&self, Parameters(p): Parameters<EditTaskParams>) -> ToolResult {
    let today = dates::today();
    let task = self.with_db(|db| {
      self.modifiable_task(db, p.id)?;
      let project_id = match p.project.as_deref() {
        None => None,
        Some(v) if v.eq_ignore_ascii_case("none") => Some(None),
        Some(name) => Some(Some(Self::writable_project(db, name)?)),
      };
      let due_date = match p.due.as_deref() {
        None => None,
        Some(v) if v.eq_ignore_ascii_case("none") => Some(None),
        Some(v) => Some(Some(dates::parse_due(v, today)?)),
      };
      db.edit_task(p.id, TaskEdit { title: p.title.as_deref(), daily: p.daily, due_date, project_id })?;
      db.get_task(p.id)
    })?;
    to_text(self.task_json(&task, today))
  }

  #[tool(description = "Move uma tarefa para o carrinho (CART) ou para a estante (SHELF). Também reabre uma tarefa concluída.")]
  fn move_task(&self, Parameters(p): Parameters<MoveTaskParams>) -> ToolResult {
    let today = dates::today();
    let task = self.with_db(|db| {
      self.modifiable_task(db, p.id)?;
      db.change_task_status(p.id, p.to.into())?;
      db.get_task(p.id)
    })?;
    to_text(self.task_json(&task, today))
  }

  #[tool(description = "Marca uma tarefa como concluída.")]
  fn done_task(&self, Parameters(p): Parameters<TaskIdParams>) -> ToolResult {
    let today = dates::today();
    let task = self.with_db(|db| {
      self.modifiable_task(db, p.id)?;
      db.change_task_status(p.id, Status::Done)?;
      db.get_task(p.id)
    })?;
    to_text(self.task_json(&task, today))
  }

  #[tool(description = "Apaga uma tarefa (respeitando a política).")]
  fn delete_task(&self, Parameters(p): Parameters<TaskIdParams>) -> ToolResult {
    let task = self.with_db(|db| {
      let task = self.modifiable_task(db, p.id)?;
      db.delete_task(p.id)?;
      Ok(task)
    })?;
    to_text(json!({ "deleted": task.id, "title": task.title }))
  }

  #[tool(description = "Registra o estado da sessão deste agente no painel de atividade. Só é necessário para agentes sem hooks.")]
  fn report_activity(&self, Parameters(p): Parameters<ReportActivityParams>) -> ToolResult {
    let state: SessionState = p.state.into();
    if let Some(cwd) = p.cwd.as_deref() {
      self.remember_cwd(cwd);
    }
    let value = self.with_db(|db| {
      let session = self.activity_session(db)?;
      let cwd = self.current_cwd();
      let project_id = db.project_for_path(&cwd)?;
      db.upsert_session(&session, &self.agent, project_id, &cwd, state)?;
      let project = project_id.map(|id| db.get_project(id)).transpose()?.map(|p| p.name);
      Ok(json!({ "session_id": session, "state": state.as_str(), "project": project, "cwd": cwd }))
    })?;
    to_text(value)
  }

  #[tool(description = "Indica em qual tarefa esta sessão está trabalhando (aparece no painel do usuário). Omita id para limpar.")]
  fn set_working_task(&self, Parameters(p): Parameters<SetWorkingTaskParams>) -> ToolResult {
    let today = dates::today();
    let value = self.with_db(|db| {
      let task = p.id.map(|id| db.get_task(id)).transpose()?;
      let session = self.activity_session(db)?;
      db.set_session_task(&session, p.id)?;
      Ok(json!({ "session_id": session, "task": task.map(|t| self.task_json(&t, today)) }))
    })?;
    to_text(value)
  }

  #[tool(description = "Propõe ao usuário uma mudança numa tarefa que você não pode alterar (concluir, mover, editar, apagar). Ele aprova ou rejeita.")]
  fn suggest_change(&self, Parameters(p): Parameters<SuggestChangeParams>) -> ToolResult {
    let today = dates::today();
    let value = self.with_db(|db| {
      let task = db.get_task(p.id)?;
      if policy::effective(task.project_policy) == Policy::None {
        anyhow::bail!("Tarefa #{}: política NONE, agentes só leem (nem sugestões)", task.id);
      }

      let action: SuggestionAction = p.action.into();
      let payload = match action {
        SuggestionAction::Done | SuggestionAction::Delete => json!({}),
        SuggestionAction::Move => match p.to {
          Some(to) => json!({ "to": Status::from(to).as_str() }),
          None => anyhow::bail!("MOVE precisa de to (CART ou SHELF)"),
        },
        SuggestionAction::Edit => {
          let mut changes = serde_json::Map::new();
          if let Some(title) = &p.title {
            changes.insert("title".into(), json!(title));
          }
          if let Some(due) = p.due.as_deref() {
            // Prazos relativos (+3d) são fixados agora, para não mudarem até a aprovação.
            let value = if due.eq_ignore_ascii_case("none") { Value::Null } else { json!(dates::parse_due(due, today)?.to_string()) };
            changes.insert("due".into(), value);
          }
          if let Some(daily) = p.daily {
            changes.insert("daily".into(), json!(daily));
          }
          if let Some(project) = p.project.as_deref() {
            let value = if project.eq_ignore_ascii_case("none") { Value::Null } else { json!(db.get_project(db.project_id(project)?)?.name) };
            changes.insert("project".into(), value);
          }
          if changes.is_empty() {
            anyhow::bail!("EDIT precisa de pelo menos um campo (title, due, daily ou project)");
          }
          Value::Object(changes)
        }
      };

      let id = db.add_suggestion(task.id, &self.author, action, &payload, p.reason.as_deref())?;
      let suggestion = db.get_suggestion(id)?;
      Ok(json!({
        "suggestion_id": id,
        "task_id": task.id,
        "summary": crate::render::describe_suggestion(&suggestion),
        "state": suggestion.state.as_str(),
        "note": "Aguardando o usuário aprovar ou rejeitar (arca suggestions).",
      }))
    })?;
    to_text(value)
  }

  #[tool(description = "Lista as sugestões feitas por este agente e se o usuário já aprovou ou rejeitou.")]
  fn list_suggestions(&self, Parameters(p): Parameters<ListSuggestionsParams>) -> ToolResult {
    let suggestions = self.with_db(|db| db.get_suggestions(p.only_pending.unwrap_or(false)))?;
    let mine: Vec<Value> = suggestions.iter()
      .filter(|s| s.created_by == self.author)
      .map(|s| json!({
        "suggestion_id": s.id,
        "task_id": s.task_id,
        "task_title": s.task_title,
        "summary": crate::render::describe_suggestion(s),
        "reason": s.reason,
        "state": s.state.as_str(),
        "created_at": s.created_at,
      }))
      .collect();
    to_text(Value::Array(mine))
  }
}

#[tool_handler]
impl ServerHandler for ArcaServer {
  fn get_info(&self) -> ServerConfig {
    ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
      .with_server_info(Implementation::new("arca", env!("CARGO_PKG_VERSION")))
      .with_instructions(INSTRUCTIONS)
  }
}

/// Atende o protocolo MCP no stdin/stdout até o cliente fechar a conexão.
pub fn serve(db: Database, agent: String) -> anyhow::Result<()> {
  let server = ArcaServer::new(db, agent);
  let handle = server.clone();

  let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
  let result = runtime.block_on(async move {
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    anyhow::Ok(())
  });

  handle.end_reported_session();
  result
}

impl ArcaServer {
  /// Se o agente reportou atividade, a sessão termina junto com o servidor
  /// (senão ficaria "trabalhando" no painel até expirar a janela).
  fn end_reported_session(&self) {
    if !self.reported.load(Ordering::Relaxed) {
      return;
    }
    if let Ok(db) = self.db.lock() {
      let _ = db.end_session(&self.session_id);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::persistence::{SuggestionState, USER};

  fn server_with(setup: impl FnOnce(&Database)) -> ArcaServer {
    let db = Database::open_in_memory().unwrap();
    setup(&db);
    ArcaServer::new(db, "claude-code".to_string())
  }

  fn user_task(db: &Database, title: &str, project_id: Option<i64>) -> i64 {
    db.add_task(NewTask { title, daily: false, status: Status::Cart, due_date: None, project_id, created_by: USER }).unwrap()
  }

  #[test]
  fn own_policy_protects_user_tasks() {
    let server = server_with(|db| {
      let p = db.add_project("arca", Policy::Own).unwrap();
      user_task(db, "do usuário", Some(p));
    });

    let err = server.done_task(Parameters(TaskIdParams { id: 1 })).unwrap_err();
    assert!(err.contains("criada pelo usuário; política OWN"), "{err}");
    assert!(server.delete_task(Parameters(TaskIdParams { id: 1 })).is_err());

    let added: Value = serde_json::from_str(&server.add_task(Parameters(AddTaskParams {
      title: "anotar para depois".into(), project: Some("arca".into()), due: Some("+3d".into()), status: None, daily: None,
    })).unwrap()).unwrap();
    assert_eq!(added["status"], "SHELF");
    assert_eq!(added["created_by"], "agent:claude-code");
    assert_eq!(added["due"], "em 3 dias");
    assert_eq!(added["can_modify"], true);

    let id = added["id"].as_i64().unwrap();
    assert!(server.move_task(Parameters(MoveTaskParams { id, to: PlaceParam::Cart })).is_ok());
    assert!(server.done_task(Parameters(TaskIdParams { id })).is_ok());
  }

  #[test]
  fn suggestions_go_to_the_user() {
    let server = server_with(|db| {
      let p = db.add_project("arca", Policy::Own).unwrap();
      user_task(db, "do usuário", Some(p));
    });

    let err = server.done_task(Parameters(TaskIdParams { id: 1 })).unwrap_err();
    assert!(err.contains("suggest_change"), "{err}");

    let created: Value = serde_json::from_str(&server.suggest_change(Parameters(SuggestChangeParams {
      id: 1, action: SuggestActionParam::Edit, to: None, title: None, due: Some("+3d".into()),
      daily: None, project: Some("none".into()), reason: Some("prazo da banca".into()),
    })).unwrap()).unwrap();
    assert_eq!(created["state"], SuggestionState::Pending.as_str());
    assert!(created["summary"].as_str().unwrap().contains("sem projeto"));

    assert!(server.suggest_change(Parameters(SuggestChangeParams {
      id: 1, action: SuggestActionParam::Move, to: None, title: None, due: None, daily: None, project: None, reason: None,
    })).is_err());

    let mine: Value = serde_json::from_str(&server.list_suggestions(Parameters(ListSuggestionsParams { only_pending: None })).unwrap()).unwrap();
    assert_eq!(mine.as_array().unwrap().len(), 1);
  }

  #[test]
  fn calls_are_activity_and_link_tasks() {
    let server = server_with(|db| {
      user_task(db, "tarefa", None);
    });
    *server.cwd.lock().unwrap() = "c:/projeto".to_string();

    // Sem sessão de hooks, o servidor registra a própria sessão na primeira chamada.
    server.list_projects().unwrap();
    server.set_working_task(Parameters(SetWorkingTaskParams { id: Some(1) })).unwrap();
    {
      let db = server.db.lock().unwrap();
      let sessions = db.get_sessions().unwrap();
      assert_eq!(sessions.len(), 1);
      assert!(sessions[0].session_id.starts_with("mcp:claude-code:"));
      assert_eq!(sessions[0].state, SessionState::Working);
      assert_eq!(sessions[0].task_title.as_deref(), Some("tarefa"));
    }

    // Com uma sessão de hooks na mesma pasta, a atividade vai para ela.
    server.db.lock().unwrap().upsert_session("hook-1", "claude-code", None, "c:/projeto", SessionState::Waiting).unwrap();
    server.set_working_task(Parameters(SetWorkingTaskParams { id: Some(1) })).unwrap();
    let db = server.db.lock().unwrap();
    let hook = db.get_sessions().unwrap().into_iter().find(|s| s.session_id == "hook-1").unwrap();
    assert_eq!(hook.state, SessionState::Working);
    assert_eq!(hook.task_id, Some(1));
  }

  #[test]
  fn none_policy_is_read_only() {
    let server = server_with(|db| {
      db.add_project("leitura", Policy::None).unwrap();
    });
    let err = server.add_task(Parameters(AddTaskParams {
      title: "x".into(), project: Some("leitura".into()), due: None, status: None, daily: None,
    })).unwrap_err();
    assert!(err.contains("NONE"), "{err}");
    assert!(server.list_tasks(Parameters(ListTasksParams { project: Some("leitura".into()), status: None, include_user_tasks: None })).is_ok());
  }

  #[test]
  fn all_policy_allows_user_tasks_but_not_moving_them_out() {
    let server = server_with(|db| {
      let all = db.add_project("tudo", Policy::All).unwrap();
      db.add_project("protegido", Policy::None).unwrap();
      user_task(db, "do usuário", Some(all));
    });
    assert!(server.done_task(Parameters(TaskIdParams { id: 1 })).is_ok());
    let err = server.edit_task(Parameters(EditTaskParams {
      id: 1, title: None, due: None, daily: None, project: Some("protegido".into()),
    })).unwrap_err();
    assert!(err.contains("NONE"), "{err}");
  }

  #[test]
  fn list_marks_what_the_agent_can_modify() {
    let server = server_with(|db| {
      user_task(db, "do usuário", None);
    });
    server.add_task(Parameters(AddTaskParams { title: "minha".into(), project: None, due: None, status: None, daily: None })).unwrap();

    let all: Value = serde_json::from_str(&server.list_tasks(Parameters(ListTasksParams {
      project: None, status: None, include_user_tasks: None,
    })).unwrap()).unwrap();
    let flags: Vec<(String, bool)> = all.as_array().unwrap().iter()
      .map(|t| (t["title"].as_str().unwrap().to_string(), t["can_modify"].as_bool().unwrap()))
      .collect();
    assert_eq!(flags, vec![("do usuário".into(), false), ("minha".into(), true)]);

    let only_agents: Value = serde_json::from_str(&server.list_tasks(Parameters(ListTasksParams {
      project: None, status: Some(StatusParam::Shelf), include_user_tasks: Some(false),
    })).unwrap()).unwrap();
    assert_eq!(only_agents.as_array().unwrap().len(), 1);
  }
}
