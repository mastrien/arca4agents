use crate::persistence::{Task, Status};
use std::io::{self, Write};
use colored::Colorize;

fn render_list(tasks: Vec<Task>, id: Option<bool>, status: Option<bool>, created_at: Option<bool>) -> () {

  if tasks.is_empty() {
    println!("{}", "Empty".red())
  }

  // Status can be false if you want to render only shelf
  let stdout = io::stdout();
  let mut handle = io::BufWriter::new(stdout);

  let mut line;

  for (index, task) in tasks.iter().enumerate() {
    let id_str = format!("#{}", task.id);
    let daily_str = if task.daily {"[DAILY]"} else {""};
    let status_str = if matches!(task.status, Status::DONE) {"[DONE]".green()} else {"".normal()};
    let created_at_str = format!("{}:", task.created_at);
    
    line = format!("{} {}{} {} {}",
      if id.is_some() && (id.unwrap() == true) {id_str.purple()} else {format!("[{}]", index).purple()},
      if created_at.is_some() && (created_at.unwrap() == true) {created_at_str.yellow()} else {"".to_string().normal()},
      task.title,
      daily_str.cyan(),
      if status.is_some() && (status.unwrap() == true) {status_str} else {"".normal()},
    );

    writeln!(handle, "{}", &line.trim_end()).unwrap();

    line.clear()
  }

  handle.flush().unwrap();

}

pub fn render_all(tasks: Vec<Task>, id: Option<bool>) -> () {
  let mut cart: Vec<Task> = Vec::new();
  let mut shelf: Vec<Task> = Vec::new();

  for task in tasks {
    if matches!(task.status, Status::SHELF) {
      shelf.push(task)
    } else {
      cart.push(task)
    }
  }

  println!("Cart");
  render_list(cart, id, Some(true), Some(false));
  println!("");
  println!("Shelf");
  render_list(shelf, id, Some(false), Some(false));
}

pub fn render_cart(tasks: Vec<Task>, id: Option<bool>) -> () {
  let mut cart: Vec<Task> = Vec::new();

  for task in tasks {
    if !matches!(task.status, Status::SHELF) {
      cart.push(task)
    }
  }

  println!("Cart");
  render_list(cart, id, Some(true), Some(false));
  println!("");
}

pub fn render_shelf(tasks: Vec<Task>, id: Option<bool>) -> () {
  let mut shelf: Vec<Task> = Vec::new();

  for task in tasks {
    if matches!(task.status, Status::SHELF) {
      shelf.push(task)
    }
  }

  println!("Cart");
  render_list(shelf, id, Some(true), Some(false));
  println!("");
}

pub fn render_history(tasks: Vec<Task>) -> () {
  println!("History");
  render_list(tasks, Some(true), Some(true), Some(true));
}