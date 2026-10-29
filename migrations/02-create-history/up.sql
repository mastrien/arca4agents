CREATE TABLE task_history (
  task_history_id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_history_title TEXT NOT NULL,
  task_history_is_daily BOOLEAN NOT NULL DEFAULT 0,
  task_history_date DATETIME DEFAULT (DATETIME('now', 'localtime'))
);