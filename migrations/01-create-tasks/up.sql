CREATE TABLE tasks (
  task_id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_title TEXT NOT NULL,
  task_status TEXT CHECK(task_status IN ('DONE', 'CART', 'SHELF')) NOT NULL DEFAULT 'SHELF',
  task_is_daily BOOLEAN NOT NULL DEFAULT 0,
  created_at DATETIME DEFAULT (DATETIME('now', 'localtime')),
  updated_at DATETIME DEFAULT (DATETIME('now', 'localtime'))
);

CREATE TRIGGER update_tasks_updated_at
AFTER UPDATE ON tasks
FOR EACH ROW
WHEN OLD.updated_at = NEW.updated_at
BEGIN
  UPDATE tasks
  SET updated_at = DATETIME('now', 'localtime')
  WHERE task_id = OLD.task_id;
END;