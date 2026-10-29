use std::sync::LazyLock;

use anyhow::{Context};
use include_dir::{include_dir, Dir};
use rusqlite::{Connection, params};
use rusqlite_migration::Migrations;
use std::path::Path;

// Embed the entire migrations directory into the binary at compile-time
static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

static MIGRATIONS: LazyLock<Migrations<'static>> =
  LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

pub struct Database {
  conn: Connection,
}

#[derive(Clone)]
pub struct Task {
  pub id: u32,
  pub title: String,
  pub daily: bool,
  pub status: Status,
  pub created_at: String,
}

#[derive(Clone)]
pub enum Status {
  DONE,
  CART,
  SHELF,
  UNKNOWN,
}

impl Database {
  pub fn init(db_path: impl AsRef<Path>) -> anyhow::Result<Self> {

    let mut conn = Connection::open(db_path)
      .context("Failed to open SQLite database")?;

    MIGRATIONS.to_latest(&mut conn)?;

    Ok(Self { conn })
  }

  pub fn conn(&self) -> &Connection {
    &self.conn
  }

  pub fn get_all(&self) -> anyhow::Result<Vec<Task>> {
    let mut stmt = self.conn.prepare("SELECT * FROM tasks ORDER BY task_id ASC")
      .context("Fail while prepating SQL query")?;

    let item_iter = stmt.query_map([], |row| {
      Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        status: match row.get::<_, String>(2)?.as_str() {
          "DONE" => Status::DONE,
          "CART" => Status::CART,
          "SHELF" => Status::SHELF,
          _ => Status::UNKNOWN,
        },
        daily: row.get(3)?,
        created_at: row.get(4)?,
      })
    })?;

    let mut tasks = Vec::new();
    for item in item_iter {
      tasks.push(item?);
    }

    Ok(tasks)
  }

  pub fn get_cart(&self) -> anyhow::Result<Vec<Task>> {
    let mut stmt = self.conn.prepare("SELECT * FROM tasks WHERE NOT task_status='SHELF' ORDER BY task_id ASC")
      .context("Fail while prepating SQL query")?;

    let item_iter = stmt.query_map([], |row| {
      Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        status: match row.get::<_, String>(2)?.as_str() {
          "DONE" => Status::DONE,
          "CART" => Status::CART,
          "SHELF" => Status::SHELF,
          _ => Status::UNKNOWN,
        },
        daily: row.get(3)?,
        created_at: row.get(4)?,
      })
    })?;

    let mut tasks = Vec::new();
    for item in item_iter {
      tasks.push(item?);
    }

    Ok(tasks)
  }

  pub fn get_shelf(&self) -> anyhow::Result<Vec<Task>> {
let mut stmt = self.conn.prepare("SELECT * FROM tasks WHERE task_status='SHELF' ORDER BY task_id ASC")
      .context("Fail while prepating SQL query")?;

    let item_iter = stmt.query_map([], |row| {
      Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        status: match row.get::<_, String>(2)?.as_str() {
          "DONE" => Status::DONE,
          "CART" => Status::CART,
          "SHELF" => Status::SHELF,
          _ => Status::UNKNOWN,
        },
        daily: row.get(3)?,
        created_at: row.get(4)?,
      })
    })?;

    let mut tasks = Vec::new();
    for item in item_iter {
      tasks.push(item?);
    }

    Ok(tasks)
  }

  pub fn add_task(&self, title: &str, daily: bool, cart: bool) -> anyhow::Result<i64> {
    self.conn.execute(
      "INSERT INTO tasks (task_title, task_is_daily, task_status) VALUES (?1, ?2, ?3)",
      params![title, daily, if !cart {"SHELF"} else {"CART"}]
    ).context("Fail to insert task")?;

    Ok(self.conn.last_insert_rowid())
  }

  pub fn add_history(&self, title: &str, daily: bool) -> () {
    self.conn.execute(
      "INSERT INTO task_history (task_history_title, task_history_is_daily) VALUES (?1, ?2)",
        params![title, daily]
    ).context("Fail to insert history").unwrap();
  }

  pub fn delete_task(&self, id: u32) -> () {
    self.conn.execute(
      "DELETE FROM tasks WHERE task_id=?1",
      params![id]
    ).context("Fail to delete task").unwrap();
  }

  pub fn edit_task(&self, id: u32, title: Option<&str>, daily: Option<bool>) -> anyhow::Result<usize> {
    if title.is_none() && daily.is_none() {
      return Ok(0)
    }

    let rows_affected = self.conn.execute(
      "UPDATE tasks SET task_title = COALESCE(?1, task_title), task_is_daily = COALESCE(?2, task_is_daily) WHERE task_id = ?3",
      params![title, daily, id]
    ).context("Fail to update task")?;

    Ok(rows_affected)
  }

  pub fn change_task_status(&self, id: u32, status: Status)  {
    let status_str = match status {
      Status::CART => "CART",
      Status::SHELF => "SHELF",
      Status::DONE => "DONE",
      _ => "UNKNOWN"
    };

    self.conn.execute("UPDATE tasks SET task_status = ?1 WHERE task_id = ?2", params!(status_str, id))
      .context("Fail to mark task as done").unwrap();
  }

  pub fn get_task_by_position(&self, position: u8, is_shelf: bool) -> Task {
    if !is_shelf {
      self.get_cart().unwrap()[position as usize].clone()
    } else {
      self.get_shelf().unwrap()[position as usize].clone()
    }
  }

  pub fn get_history(&self) -> anyhow::Result<Vec<Task>> {
    let mut stmt = self.conn.prepare("SELECT * FROM task_history ORDER BY task_history_id ASC")
      .context("Fail while prepating SQL query")?;

    let item_iter = stmt.query_map([], |row| {
      Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        status: Status::DONE,
        daily: row.get(2)?,
        created_at: row.get(3)?,
      })
    })?;

    let mut tasks = Vec::new();
    for item in item_iter {
      tasks.push(item?);
    }

    Ok(tasks)
  }
}