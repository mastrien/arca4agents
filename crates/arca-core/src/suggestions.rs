//! Sugestões de agentes (decisão 5): o agente propõe, só o usuário aplica.

use anyhow::{bail, Result};

use crate::dates;
use crate::persistence::{Database, Status, Suggestion, SuggestionAction, SuggestionState, TaskEdit};

fn pending(db: &Database, id: i64) -> Result<Suggestion> {
  let suggestion = db.get_suggestion(id)?;
  if suggestion.state != SuggestionState::Pending {
    bail!("A sugestão [{id}] já foi resolvida");
  }
  Ok(suggestion)
}

/// Aplica a mudança proposta, com as permissões do usuário.
fn apply(db: &Database, suggestion: &Suggestion) -> Result<()> {
  let payload = &suggestion.payload;
  let id = suggestion.task_id;
  match suggestion.action {
    SuggestionAction::Done => db.change_task_status(id, Status::Done),
    SuggestionAction::Delete => db.delete_task(id),
    SuggestionAction::Move => {
      let to = match payload.get("to").and_then(|v| v.as_str()) {
        Some("CART") => Status::Cart,
        Some("SHELF") => Status::Shelf,
        other => bail!("Destino inválido na sugestão: {other:?}"),
      };
      db.change_task_status(id, to)
    }
    SuggestionAction::Edit => {
      let due_date = match payload.get("due") {
        None => None,
        Some(serde_json::Value::Null) => Some(None),
        Some(v) => Some(Some(dates::parse_due(v.as_str().unwrap_or_default(), dates::today())?)),
      };
      let project_id = match payload.get("project") {
        None => None,
        Some(serde_json::Value::Null) => Some(None),
        Some(v) => Some(Some(db.project_id(v.as_str().unwrap_or_default())?)),
      };
      db.edit_task(id, TaskEdit {
        title: payload.get("title").and_then(|v| v.as_str()),
        daily: payload.get("daily").and_then(|v| v.as_bool()),
        due_date,
        project_id,
      })
    }
  }
}

/// Aplica e marca como aprovada. Devolve a sugestão como estava antes de aplicar.
pub fn approve(db: &Database, id: i64) -> Result<Suggestion> {
  let suggestion = pending(db, id)?;
  apply(db, &suggestion)?;
  // Se a sugestão era apagar, a linha dela já saiu junto com a tarefa.
  db.resolve_suggestion(id, SuggestionState::Approved)?;
  Ok(suggestion)
}

pub fn reject(db: &Database, id: i64) -> Result<Suggestion> {
  let suggestion = pending(db, id)?;
  db.resolve_suggestion(id, SuggestionState::Rejected)?;
  Ok(suggestion)
}

/// "concluir", "mover para o carrinho", "editar (título → 'x', prazo → 2026-11-05)", "apagar".
pub fn describe(suggestion: &Suggestion) -> String {
  let payload = &suggestion.payload;
  match suggestion.action {
    SuggestionAction::Done => "concluir".to_string(),
    SuggestionAction::Delete => "apagar".to_string(),
    SuggestionAction::Move => match payload.get("to").and_then(|v| v.as_str()) {
      Some("CART") => "mover para o carrinho".to_string(),
      Some("SHELF") => "mover para a estante".to_string(),
      _ => "mover".to_string(),
    },
    SuggestionAction::Edit => {
      let mut changes = Vec::new();
      if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
        changes.push(format!("título → '{title}'"));
      }
      match payload.get("due") {
        Some(serde_json::Value::Null) => changes.push("sem prazo".to_string()),
        Some(due) => changes.push(format!("prazo → {}", due.as_str().unwrap_or("?"))),
        None => {}
      }
      if let Some(daily) = payload.get("daily").and_then(|v| v.as_bool()) {
        changes.push(if daily { "tornar diária" } else { "deixar de ser diária" }.to_string());
      }
      match payload.get("project") {
        Some(serde_json::Value::Null) => changes.push("sem projeto".to_string()),
        Some(project) => changes.push(format!("projeto → {}", project.as_str().unwrap_or("?"))),
        None => {}
      }
      format!("editar ({})", changes.join(", "))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::persistence::{NewTask, Policy, USER};

  #[test]
  fn approving_applies_the_suggested_change() {
    let db = Database::open_in_memory().unwrap();
    let project = db.add_project("arca", Policy::Own).unwrap();
    let id = db.add_task(NewTask { title: "x", daily: false, status: Status::Cart, due_date: None, project_id: Some(project), created_by: USER }).unwrap();

    let edit = db.add_suggestion(id, "agent:claude-code", SuggestionAction::Edit,
      &serde_json::json!({"title": "novo", "due": "2026-11-05", "project": null}), None).unwrap();
    assert_eq!(describe(&db.get_suggestion(edit).unwrap()), "editar (título → 'novo', prazo → 2026-11-05, sem projeto)");
    approve(&db, edit).unwrap();
    let task = db.get_task(id).unwrap();
    assert_eq!(task.title, "novo");
    assert_eq!(task.due_date.map(|d| d.to_string()).as_deref(), Some("2026-11-05"));
    assert_eq!(task.project_id, None);
    assert!(approve(&db, edit).is_err(), "não aprova duas vezes");

    let move_ = db.add_suggestion(id, "agent:claude-code", SuggestionAction::Move, &serde_json::json!({"to": "SHELF"}), None).unwrap();
    reject(&db, move_).unwrap();
    assert_eq!(db.get_task(id).unwrap().status, Status::Cart);

    let delete = db.add_suggestion(id, "agent:claude-code", SuggestionAction::Delete, &serde_json::json!({}), None).unwrap();
    approve(&db, delete).unwrap();
    assert!(db.get_task(id).is_err());
  }
}
