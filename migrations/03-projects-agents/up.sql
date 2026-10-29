CREATE TABLE projects (
  project_id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_name TEXT NOT NULL UNIQUE,
  agent_policy TEXT NOT NULL DEFAULT 'OWN'
    CHECK(agent_policy IN ('NONE', 'OWN', 'ALL')),
  created_at DATETIME DEFAULT (DATETIME('now', 'localtime'))
);

CREATE TABLE project_folders (
  folder_id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  folder_path TEXT NOT NULL UNIQUE
);

ALTER TABLE tasks ADD COLUMN due_date DATE;
ALTER TABLE tasks ADD COLUMN done_at DATETIME;
ALTER TABLE tasks ADD COLUMN project_id INTEGER REFERENCES projects(project_id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN created_by TEXT NOT NULL DEFAULT 'user';

-- Tarefas concluídas antes desta versão não têm done_at; updated_at é a melhor estimativa.
UPDATE tasks SET done_at = updated_at WHERE task_status = 'DONE';

ALTER TABLE task_history ADD COLUMN task_history_due_date DATE;
ALTER TABLE task_history ADD COLUMN task_history_project_id INTEGER;
ALTER TABLE task_history ADD COLUMN task_history_created_by TEXT NOT NULL DEFAULT 'user';

CREATE TABLE agent_sessions (
  session_id TEXT PRIMARY KEY,
  agent TEXT NOT NULL,
  project_id INTEGER REFERENCES projects(project_id) ON DELETE SET NULL,
  cwd TEXT NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('WORKING', 'WAITING', 'ENDED')),
  started_at DATETIME DEFAULT (DATETIME('now', 'localtime')),
  last_seen_at DATETIME DEFAULT (DATETIME('now', 'localtime'))
);

CREATE TABLE app_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
