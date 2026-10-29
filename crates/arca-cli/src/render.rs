use crate::agents::{self, Activity, ActiveWindow};
use crate::dates::{self, humanize_ago, humanize_due};
use crate::persistence::{HistoryEntry, Policy, Project, Session, Status, Suggestion, Task, AGENT_PREFIX};
use crate::suggestions;
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

/// Etiquetas de uma tarefa: daily, done, prazo, projeto e autor (conforme o estilo).
fn task_tags(task: &Task, style: &ListStyle, show_done: bool) -> Vec<ColoredString> {
  let mut tags = Vec::new();
  if task.daily {
    tags.push("daily".cyan());
  }
  if show_done && task.status == Status::Done {
    tags.push("done".green());
  }
  if let Some(due) = task.due_date {
    tags.push(due_tag(due, style.today));
  }
  if style.show_project && let Some(project) = &task.project_name {
    tags.push(format!("@{project}").magenta());
  }
  if style.show_author && task.is_from_agent() {
    tags.push(task.created_by.as_str().yellow());
  }
  tags
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

    let tags = task_tags(task, style, true);

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
  let window = project.active_window
    .map(|m| format!(" {}", format!("[janela {m} min]").dimmed()))
    .unwrap_or_default();
  format!("{} {}{}  estante {} {} carrinho {} {} feitas {}",
    project.name.magenta().bold(),
    policy_tag(project.policy),
    window,
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
pub fn session_label(session: &Session, now: NaiveDateTime, window: ActiveWindow) -> ColoredString {
  match agents::activity(session, now, window) {
    Activity::Working => "trabalhando".green().bold(),
    Activity::PossiblyStopped => "possivelmente parada".yellow(),
    Activity::Waiting => "esperando você".cyan().bold(),
    Activity::Ended => "encerrada".dimmed(),
  }
}

/// `mcp:claude-code:1234` → `mcp 1234`; `manual:...` → `manual`; UUIDs → primeiros 8 caracteres.
fn short_session_id(id: &str) -> String {
  if let Some(rest) = id.strip_prefix("mcp:") {
    return format!("mcp {}", rest.rsplit(':').next().unwrap_or(rest));
  }
  if id.starts_with("manual:") {
    return "manual".to_string();
  }
  id.chars().take(8).collect()
}

pub fn render_sessions(sessions: &[Session], now: NaiveDateTime, window: ActiveWindow) {
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
      let short_id = short_session_id(&session.session_id);
      let mut line = format!("  {} {}  {} {} última atividade {}",
        session.agent.yellow(),
        short_id.dimmed(),
        session_label(session, now, window),
        separator(),
        ago,
      );
      if let (Some(id), Some(title)) = (session.task_id, &session.task_title) {
        line.push_str(&format!(" {} em {} {}", separator(), format!("#{id}").purple(), title));
      }
      if session.project_name.is_none() {
        line.push_str(&format!(" {} {}", separator(), session.cwd.dimmed()));
      }
      println!("{line}");
    }
  }
}

// ---------- Sugestões ----------

fn agent_display(created_by: &str) -> &str {
  created_by.strip_prefix(AGENT_PREFIX).unwrap_or(created_by)
}

pub fn render_suggestions(suggestions: &[Suggestion], now: NaiveDateTime) {
  if suggestions.is_empty() {
    println!("{}", "Nenhuma sugestão pendente.".dimmed());
    return;
  }
  for suggestion in suggestions {
    let ago = dates::parse_datetime(&suggestion.created_at)
      .map(|t| humanize_ago(t, now))
      .unwrap_or_else(|| suggestion.created_at.clone());
    let state = match suggestion.state {
      crate::persistence::SuggestionState::Pending => "".normal(),
      crate::persistence::SuggestionState::Approved => " aprovada".green(),
      crate::persistence::SuggestionState::Rejected => " rejeitada".red(),
    };
    println!("{} {} propõe {} {} {}  {}{}",
      format!("[{}]", suggestion.id).purple(),
      agent_display(&suggestion.created_by).yellow(),
      suggestions::describe(suggestion).bold(),
      format!("#{}", suggestion.task_id).purple(),
      suggestion.task_title,
      ago.dimmed(),
      state,
    );
    if let Some(reason) = &suggestion.reason {
      println!("    {} {}", "motivo:".dimmed(), reason);
    }
  }
}

// ---------- Board ----------

/// Uma linha já colorida e a largura que ela ocupa na tela (sem os códigos ANSI).
struct Cell {
  text: String,
  width: usize,
}

impl Cell {
  fn blank() -> Self {
    Cell { text: String::new(), width: 0 }
  }

  fn padded(&self, width: usize) -> String {
    format!("{}{}", self.text, " ".repeat(width.saturating_sub(self.width)))
  }
}

fn text_width(s: &str) -> usize {
  s.chars().count()
}

/// Quebra por palavras; palavras maiores que a linha são cortadas.
fn wrap(text: &str, width: usize) -> Vec<String> {
  let width = width.max(1);
  let mut lines = Vec::new();
  let mut current = String::new();

  for word in text.split_whitespace() {
    let mut word = word.to_string();
    while text_width(&word) > width {
      if !current.is_empty() {
        lines.push(std::mem::take(&mut current));
      }
      lines.push(word.chars().take(width).collect());
      word = word.chars().skip(width).collect();
    }
    if word.is_empty() {
      continue;
    }
    if current.is_empty() {
      current = word;
    } else if text_width(&current) + 1 + text_width(&word) <= width {
      current.push(' ');
      current.push_str(&word);
    } else {
      lines.push(std::mem::replace(&mut current, word));
    }
  }
  if !current.is_empty() || lines.is_empty() {
    lines.push(current);
  }
  lines
}

fn card(task: &Task, style: &ListStyle, width: usize) -> Vec<Cell> {
  let marker = format!("#{}", task.id);
  let indent = text_width(&marker) + 1;
  let inner = width.saturating_sub(indent).max(1);

  let mut cells = Vec::new();
  for (i, line) in wrap(&task.title, inner).into_iter().enumerate() {
    let prefix = if i == 0 { marker.purple().to_string() } else { " ".repeat(text_width(&marker)) };
    cells.push(Cell { width: indent + text_width(&line), text: format!("{prefix} {line}") });
  }

  // Etiquetas na linha de baixo, quebrando quando não cabem.
  let mut line = String::new();
  let mut line_width = 0;
  for tag in task_tags(task, style, false) {
    let tag_width = text_width(&tag);
    if line_width > 0 && line_width + 3 + tag_width > inner {
      cells.push(Cell { text: format!("{}{line}", " ".repeat(indent)), width: indent + line_width });
      line.clear();
      line_width = 0;
    }
    if line_width > 0 {
      line.push_str(&format!(" {} ", separator()));
      line_width += 3;
    }
    line.push_str(&tag.to_string());
    line_width += tag_width;
  }
  if line_width > 0 {
    cells.push(Cell { text: format!("{}{line}", " ".repeat(indent)), width: indent + line_width });
  }
  cells
}

fn board_column(title: &str, tasks: &[Task], style: &ListStyle, width: usize) -> Vec<Cell> {
  let header = format!("{title} ({})", tasks.len());
  let mut cells = vec![
    Cell { width: text_width(&header), text: header.bold().to_string() },
    Cell { width, text: "─".repeat(width).dimmed().to_string() },
  ];
  if tasks.is_empty() {
    cells.push(Cell { width: 5, text: "vazio".dimmed().to_string() });
  }
  for task in tasks {
    cells.extend(card(task, style, width));
  }
  cells
}

/// Estante | Carrinho | Feito (hoje). Em telas estreitas (ex.: Termux) as colunas ficam empilhadas.
pub fn render_board(shelf: &[Task], cart: &[Task], done: &[Task], style: &ListStyle, total_width: usize) {
  const GAP: &str = " │ ";
  const MIN_SIDE_BY_SIDE: usize = 72;
  let titles = ["Estante", "Carrinho", "Feito hoje"];
  let lists = [shelf, cart, done];

  if total_width < MIN_SIDE_BY_SIDE {
    let width = total_width.max(20);
    for (i, (title, tasks)) in titles.iter().zip(lists).enumerate() {
      if i > 0 {
        println!();
      }
      for cell in board_column(title, tasks, style, width) {
        println!("{}", cell.text);
      }
    }
    return;
  }

  let width = (total_width - 2 * text_width(GAP)) / 3;
  let mut columns: Vec<Vec<Cell>> = titles.iter().zip(lists)
    .map(|(title, tasks)| board_column(title, tasks, style, width))
    .collect();
  let rows = columns.iter().map(Vec::len).max().unwrap_or(0);
  for column in &mut columns {
    column.resize_with(rows, Cell::blank);
  }

  let gap = GAP.dimmed().to_string();
  for row in 0..rows {
    let line = columns.iter()
      .map(|column| column[row].padded(width))
      .collect::<Vec<_>>()
      .join(&gap);
    println!("{}", line.trim_end());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn shortens_session_ids() {
    assert_eq!(short_session_id("mcp:claude-code:1234"), "mcp 1234");
    assert_eq!(short_session_id("manual:antigravity:c:/x"), "manual");
    assert_eq!(short_session_id("9f1c2e3d-aaaa-bbbb"), "9f1c2e3d");
  }

  #[test]
  fn wraps_words_and_cuts_long_ones() {
    assert_eq!(wrap("Escrever a proposta da pesquisa", 12), vec!["Escrever a", "proposta da", "pesquisa"]);
    assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
    assert_eq!(wrap("abcdefgh", 4), vec!["abcd", "efgh"]);
    assert_eq!(wrap("", 10), vec![""]);
    assert_eq!(wrap("ação rápida", 6), vec!["ação", "rápida"]);
  }
}
