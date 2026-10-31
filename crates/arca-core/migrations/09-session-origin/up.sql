-- Link da sessão no controle remoto nativo do agente (hoje, o Remote Control do Claude Code:
-- https://claude.ai/code/session_...), lido do transcript pelo `arca ping`.
ALTER TABLE agent_sessions ADD COLUMN remote_url TEXT;

-- Sessões que começaram antes de a pasta delas ser ligada a um projeto passam a ter o projeto
-- (a pasta ligada mais específica que contém o cwd).
UPDATE agent_sessions SET project_id = (
  SELECT f.project_id FROM project_folders f
  WHERE agent_sessions.cwd = f.folder_path
     OR substr(agent_sessions.cwd, 1, length(f.folder_path) + 1) = f.folder_path || '/'
  ORDER BY length(f.folder_path) DESC LIMIT 1
) WHERE project_id IS NULL;
