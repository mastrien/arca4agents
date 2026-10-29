//! Regras do que um agente pode fazer com as tarefas (seção 3.3 do spec).
//! Aplicadas no servidor MCP, não só nas instruções do prompt.

use crate::persistence::{Policy, Task};

/// Política efetiva de uma tarefa ou projeto: sem projeto vale `OWN`.
pub fn effective(project_policy: Option<Policy>) -> Policy {
  project_policy.unwrap_or(Policy::Own)
}

/// O agente pode criar uma tarefa num projeto com esta política?
pub fn can_create(policy: Policy, project_name: Option<&str>) -> Result<(), String> {
  match policy {
    Policy::None => Err(format!(
      "{}: política NONE, agentes só leem as tarefas deste projeto",
      describe_project(project_name),
    )),
    Policy::Own | Policy::All => Ok(()),
  }
}

/// O agente `agent_author` (ex.: `agent:claude-code`) pode alterar, mover, concluir ou apagar `task`?
pub fn can_modify(policy: Policy, agent_author: &str, task: &Task) -> Result<(), String> {
  match policy {
    Policy::All => Ok(()),
    Policy::None => Err(format!(
      "Tarefa #{} ({}): política NONE, agentes só leem",
      task.id, describe_project(task.project_name.as_deref()),
    )),
    Policy::Own if task.created_by == agent_author => Ok(()),
    Policy::Own => Err(format!(
      "Tarefa #{} criada {}; política OWN ({}): o agente só altera as tarefas que ele mesmo criou",
      task.id, describe_author(&task.created_by), describe_project(task.project_name.as_deref()),
    )),
  }
}

fn describe_project(name: Option<&str>) -> String {
  match name {
    Some(name) => format!("projeto '{name}'"),
    None => "sem projeto".to_string(),
  }
}

fn describe_author(created_by: &str) -> String {
  if created_by == crate::persistence::USER { "pelo usuário".to_string() } else { format!("por {created_by}") }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::persistence::{Status, USER};

  fn task(created_by: &str) -> Task {
    Task {
      id: 7,
      title: "x".into(),
      daily: false,
      status: Status::Shelf,
      created_at: String::new(),
      due_date: None,
      done_at: None,
      project_id: Some(1),
      project_name: Some("arca".into()),
      project_policy: Some(Policy::Own),
      created_by: created_by.into(),
    }
  }

  const ME: &str = "agent:claude-code";

  #[test]
  fn own_only_touches_own_tasks() {
    assert!(can_modify(Policy::Own, ME, &task(ME)).is_ok());
    let err = can_modify(Policy::Own, ME, &task(USER)).unwrap_err();
    assert!(err.starts_with("Tarefa #7 criada pelo usuário; política OWN"), "{err}");
    assert!(err.contains("OWN"));
    assert!(can_modify(Policy::Own, ME, &task("agent:antigravity")).is_err());
  }

  #[test]
  fn none_is_read_only() {
    assert!(can_modify(Policy::None, ME, &task(ME)).is_err());
    assert!(can_create(Policy::None, Some("arca")).is_err());
  }

  #[test]
  fn all_allows_everything() {
    assert!(can_modify(Policy::All, ME, &task(USER)).is_ok());
    assert!(can_create(Policy::All, None).is_ok());
  }

  #[test]
  fn no_project_means_own() {
    assert_eq!(effective(None), Policy::Own);
    assert_eq!(effective(Some(Policy::All)), Policy::All);
  }
}
