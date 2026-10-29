use crate::dates::{self, humanize_ago, humanize_due};
use crate::persistence::{HistoryEntry, Policy, Project, Session, SessionState, Status, Task};
use chrono::{NaiveDate, NaiveDateTime};
use colored::{ColoredString, Colorize};
use std::io::{self, Write};

/// O que mostrar em cada linha de uma listagem de tarefas.
pub struct ListStyle {
  /// `#id` em vez de `[posição]`.
  pub show_id: bool,
  /// Nome do projeto (desnecessário quando a listagem já é de um projeto).
  pub show_project: bool,
  /// Autor da tarefa (`--agents` / `--only-agents`).
  pub show_author: bool,
  pub today: NaiveDate,
}

fn separator() -> ColoredString {
  "·".dimmed()
}

fn join_tags(tags: Vec<ColoredString>) -> String {
  let sep = format!(" {} ", separator());
  tags.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(&sep)
}

fn due_tag(due: NaiveDate, today: NaiveDate) -> ColoredString {
  let text = humanize_due(due, today);
  if due < today {
    text.red()
  } else if due == today {
    text.yellow()
  } else {
    text.blue()
  }
}

fn render_list(tasks: &[Task], style: &ListStyle) {
  if tasks.is_empty() {
    println!("{}", "Empty".red());
    return;
  }

  let stdout = io::stdout();
  let mut handle = io::BufWriter::new(stdout.lock());

  for (index, task) in tasks.iter().enumerate() {
    let marker = if style.show_id { format!("#{}", task.id) } else { format!("[{index}]") };

    let mut tags = Vec::new();
    if task.daily {
      tags.push("daily".cyan());
    }
    if task.status == Status::Done {
      tags.push("done".green());
    }
    if let Some(due) = task.due_date {
      tags.push(due_tag(due, style.today));
    }
    if style.show_project
      && let Some(project) = &task.project_name {
        tags.push(format!("@{project}").magenta());
      }
    if style.show_author && task.is_from_agent() {
      tags.push(task.created_by.as_str().yellow());
    }

    let line = format!("{} {}  {}", marker.purple(), task.title, join_tags(tags));
    writeln!(handle, "{}", line.trim_end()).unwrap();
  }

  handle.flush().unwrap();
}

pub fn render_cart(tasks: &[Task], style: &ListStyle) {
  println!("Cart");
  render_list(tasks, style);
}

pub fn render_shelf(tasks: &[Task], style: &ListStyle) {
  println!("Shelf");
  render_list(tasks, style);
}

pub fn render_all(cart: &[Task], shelf: &[Task], style: &ListStyle) {
  render_cart(cart, style);
  println!();
  render_shelf(shelf, style);
}

pub fn render_history(entries: &[HistoryEntry]) {
  println!("History");
  if entries.is_empty() {
    println!("{}", "Empty".red());
    return;
  }

  for entry in entries {
    let mut tags = vec!["done".green()];
    if entry.daily {
      tags.insert(0, "daily".cyan());
    }
    if let Some(project) = &entry.project_name {
      tags.push(format!("@{project}").magenta());
    }
    if let Some(due) = entry.due_date {
      tags.push(format!("prazo {due}").blue());
    }
    if entry.created_by != crate::persistence::USER {
      tags.push(entry.created_by.as_str().yellow());
    }

    println!("{} {} {}  {}",
      format!("#{}", entry.id).purple(),
      format!("{}:", entry.date).yellow(),
      entry.title,
      join_tags(tags),
    );
  }
}

fn policy_tag(policy: Policy) -> ColoredString {
  format!("[{}]", policy.as_str()).dimmed()
}

pub fn project_summary(project: &Project) -> String {
  format!("{} {}  estante {} {} carrinho {} {} feitas {}",
    project.name.magenta().bold(),
    policy_tag(project.policy),
    project.shelf, separator(), project.cart, separator(), project.done,
  )
}

pub fn render_projects(projects: &[Project]) {
  if projects.is_empty() {
    println!("{}", "Nenhum projeto. Crie um com `arca project add <nome>`.".dimmed());
    return;
  }
  for project in projects {
    println!("{}", project_summary(project));
    for folder in &project.folders {
      println!("  {}", folder.dimmed());
    }
  }
}

/// Como uma sessão aparece no painel de atividade.
pub fn session_label(session: &Session, now: NaiveDateTime, window_minutes: i64) -> ColoredString {
  let last_seen = dates::parse_datetime(&session.last_seen_at);
  match session.state {
    SessionState::Working => {
      let fresh = last_seen.is_some_and(|t| (now - t).num_minutes() < window_minutes);
      if fresh { "trabalhando".green().bold() } else { "possivelmente parada".yellow() }
    }
    SessionState::Waiting => "esperando você".cyan().bold(),
    SessionState::Ended => "encerrada".dimmed(),
  }
}

pub fn render_sessions(sessions: &[Session], now: NaiveDateTime, window_minutes: i64) {
  if sessions.is_empty() {
    println!("{}", "Nenhuma sessão de agente registrada.".dimmed());
    return;
  }

  // Agrupa por projeto mantendo a ordem (mais recente primeiro).
  let mut groups: Vec<(Option<String>, Vec<&Session>)> = Vec::new();
  for session in sessions {
    match groups.iter_mut().find(|(name, _)| *name == session.project_name) {
      Some((_, list)) => list.push(session),
      None => groups.push((session.project_name.clone(), vec![session])),
    }
  }

  for (name, list) in groups {
    match name {
      Some(name) => println!("{}", name.magenta().bold()),
      None => println!("{}", "(sem projeto)".dimmed()),
    }
    for session in list {
      let ago = dates::parse_datetime(&session.last_seen_at)
        .map(|t| humanize_ago(t, now))
        .unwrap_or_else(|| session.last_seen_at.clone());
      let short_id: String = session.session_id.chars().take(8).collect();
      let mut line = format!("  {} {}  {} {} última atividade {}",
        session.agent.yellow(),
        short_id.dimmed(),
        session_label(session, now, window_minutes),
        separator(),
        ago,
      );
      if session.project_name.is_none() {
        line.push_str(&format!(" {} {}", separator(), session.cwd.dimmed()));
      }
      println!("{line}");
    }
  }
}
