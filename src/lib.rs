pub mod persistence;
pub mod render;
pub mod db_path_solver;

use crate::persistence::Status;

pub fn list(db: persistence::Database, shelf: Option<bool>, cart: Option<bool>, id: Option<bool>) {
  
  let include_cart = if cart.is_none() && !shelf.is_none() { false } else { true };
  let include_shelf = if !cart.is_none() && shelf.is_none() { false } else { true };

  
  if include_cart && include_shelf {
    // Render all
    let tasks = db.get_all().unwrap();
    render::render_all(tasks, id);
  } else if include_cart {
    // Render cart only
    let tasks = db.get_cart().unwrap();
    render::render_cart(tasks, id);
  } else if include_shelf {
    // Render shelf
    let tasks = db.get_shelf().unwrap();
    render::render_shelf(tasks, id);
  }
}

pub fn add(db: persistence::Database, title: String, daily: bool) {
  db.add_task(&title, daily, true).unwrap();
}

pub fn done(db: persistence::Database, position: u8) {
  let task = db.get_task_by_position(position, false);
  db.change_task_status(task.id, Status::DONE);
}

pub fn edit(db: persistence::Database, id: u32, title: Option<&str>, daily: Option<bool>) {
  db.edit_task(id, title, daily).unwrap();
}

pub fn delete(db: persistence::Database, id: u32) {
  db.delete_task(id);
}

pub fn take(db: persistence::Database, position_in_shelf: u8) {
  let task = db.get_task_by_position(position_in_shelf, true);
  db.change_task_status(task.id, Status::CART);
}

pub fn putback(db: persistence::Database, position_in_cart: u8) {
  let task = db.get_task_by_position(position_in_cart, false);
  db.change_task_status(task.id, Status::SHELF);
}

pub fn check(db: persistence::Database) {
  let tasks = db.get_cart().unwrap();
  for task in tasks {
    if matches!(task.status, Status::DONE){

      db.add_history(&task.title, task.daily);

      if task.daily {
        db.change_task_status(task.id, Status::CART);
      } else {
        db.delete_task(task.id);
      }

    }
  }
  println!("Cart checked.")
}

pub fn history(db: persistence::Database) {
  let h = db.get_history().unwrap();
  render::render_history(h)
}