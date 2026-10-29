-- Decisão 3: janela de "ativo agora" por projeto (NULL = usa a global).
ALTER TABLE projects ADD COLUMN active_window_minutes INTEGER
  CHECK(active_window_minutes IS NULL OR active_window_minutes > 0);

-- Decisão 6: em qual tarefa a sessão está trabalhando.
ALTER TABLE agent_sessions ADD COLUMN task_id INTEGER REFERENCES tasks(task_id) ON DELETE SET NULL;

-- Decisão 5: mudanças que o agente propõe e só o usuário aplica.
CREATE TABLE suggestions (
  suggestion_id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id INTEGER NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  created_by TEXT NOT NULL,
  action TEXT NOT NULL CHECK(action IN ('DONE', 'MOVE', 'EDIT', 'DELETE')),
  payload TEXT NOT NULL DEFAULT '{}',
  reason TEXT,
  state TEXT NOT NULL DEFAULT 'PENDING' CHECK(state IN ('PENDING', 'APPROVED', 'REJECTED')),
  created_at DATETIME DEFAULT (DATETIME('now', 'localtime')),
  resolved_at DATETIME
);
