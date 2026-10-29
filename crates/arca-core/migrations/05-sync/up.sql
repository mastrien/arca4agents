-- Sincronização entre dispositivos (spec-apps.md, seção 3).
-- arca_uuid() e arca_clock() são funções registradas pelo arca-core em cada conexão.
-- arca_clock() devolve um relógio lógico híbrido ordenável: '<ms>:<contador>:<dispositivo>'.

CREATE TABLE sync_tombstones (
  entity TEXT NOT NULL,
  uuid TEXT NOT NULL,
  sync_clock TEXT NOT NULL,
  PRIMARY KEY (entity, uuid)
);

-- Servidores com que este dispositivo sincroniza (lado cliente).
CREATE TABLE sync_peers (
  peer_device TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  token TEXT NOT NULL,
  -- Cursores: até onde já recebemos do servidor (relógio dele) e já enviamos (relógio local).
  last_pulled TEXT NOT NULL DEFAULT '',
  last_pushed TEXT NOT NULL DEFAULT '',
  last_synced_at DATETIME
);

ALTER TABLE projects ADD COLUMN uuid TEXT;
ALTER TABLE projects ADD COLUMN sync_clock TEXT;
UPDATE projects SET uuid = arca_uuid(), sync_clock = arca_clock();
CREATE UNIQUE INDEX projects_uuid ON projects(uuid);
CREATE INDEX projects_sync_clock ON projects(sync_clock);

CREATE TRIGGER projects_sync_insert AFTER INSERT ON projects
FOR EACH ROW WHEN NEW.uuid IS NULL OR NEW.sync_clock IS NULL
BEGIN
  UPDATE projects SET uuid = COALESCE(NEW.uuid, arca_uuid()), sync_clock = COALESCE(NEW.sync_clock, arca_clock())
  WHERE project_id = NEW.project_id;
END;

-- Escritas locais não mexem em sync_clock; a sincronização grava o relógio remoto explicitamente.
CREATE TRIGGER projects_sync_update AFTER UPDATE ON projects
FOR EACH ROW WHEN NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE projects SET sync_clock = arca_clock() WHERE project_id = NEW.project_id;
END;

CREATE TRIGGER projects_sync_delete AFTER DELETE ON projects
FOR EACH ROW WHEN OLD.uuid IS NOT NULL
BEGIN
  INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES ('project', OLD.uuid, arca_clock())
    ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = excluded.sync_clock;
END;

ALTER TABLE tasks ADD COLUMN uuid TEXT;
ALTER TABLE tasks ADD COLUMN sync_clock TEXT;
UPDATE tasks SET uuid = arca_uuid(), sync_clock = arca_clock();
CREATE UNIQUE INDEX tasks_uuid ON tasks(uuid);
CREATE INDEX tasks_sync_clock ON tasks(sync_clock);

CREATE TRIGGER tasks_sync_insert AFTER INSERT ON tasks
FOR EACH ROW WHEN NEW.uuid IS NULL OR NEW.sync_clock IS NULL
BEGIN
  UPDATE tasks SET uuid = COALESCE(NEW.uuid, arca_uuid()), sync_clock = COALESCE(NEW.sync_clock, arca_clock())
  WHERE task_id = NEW.task_id;
END;

-- Escritas locais não mexem em sync_clock; a sincronização grava o relógio remoto explicitamente.
CREATE TRIGGER tasks_sync_update AFTER UPDATE ON tasks
FOR EACH ROW WHEN NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE tasks SET sync_clock = arca_clock() WHERE task_id = NEW.task_id;
END;

CREATE TRIGGER tasks_sync_delete AFTER DELETE ON tasks
FOR EACH ROW WHEN OLD.uuid IS NOT NULL
BEGIN
  INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES ('task', OLD.uuid, arca_clock())
    ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = excluded.sync_clock;
END;

ALTER TABLE task_history ADD COLUMN uuid TEXT;
ALTER TABLE task_history ADD COLUMN sync_clock TEXT;
UPDATE task_history SET uuid = arca_uuid(), sync_clock = arca_clock();
CREATE UNIQUE INDEX task_history_uuid ON task_history(uuid);
CREATE INDEX task_history_sync_clock ON task_history(sync_clock);

CREATE TRIGGER task_history_sync_insert AFTER INSERT ON task_history
FOR EACH ROW WHEN NEW.uuid IS NULL OR NEW.sync_clock IS NULL
BEGIN
  UPDATE task_history SET uuid = COALESCE(NEW.uuid, arca_uuid()), sync_clock = COALESCE(NEW.sync_clock, arca_clock())
  WHERE task_history_id = NEW.task_history_id;
END;

-- Escritas locais não mexem em sync_clock; a sincronização grava o relógio remoto explicitamente.
CREATE TRIGGER task_history_sync_update AFTER UPDATE ON task_history
FOR EACH ROW WHEN NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE task_history SET sync_clock = arca_clock() WHERE task_history_id = NEW.task_history_id;
END;

CREATE TRIGGER task_history_sync_delete AFTER DELETE ON task_history
FOR EACH ROW WHEN OLD.uuid IS NOT NULL
BEGIN
  INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES ('history', OLD.uuid, arca_clock())
    ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = excluded.sync_clock;
END;

ALTER TABLE suggestions ADD COLUMN uuid TEXT;
ALTER TABLE suggestions ADD COLUMN sync_clock TEXT;
UPDATE suggestions SET uuid = arca_uuid(), sync_clock = arca_clock();
CREATE UNIQUE INDEX suggestions_uuid ON suggestions(uuid);
CREATE INDEX suggestions_sync_clock ON suggestions(sync_clock);

CREATE TRIGGER suggestions_sync_insert AFTER INSERT ON suggestions
FOR EACH ROW WHEN NEW.uuid IS NULL OR NEW.sync_clock IS NULL
BEGIN
  UPDATE suggestions SET uuid = COALESCE(NEW.uuid, arca_uuid()), sync_clock = COALESCE(NEW.sync_clock, arca_clock())
  WHERE suggestion_id = NEW.suggestion_id;
END;

-- Escritas locais não mexem em sync_clock; a sincronização grava o relógio remoto explicitamente.
CREATE TRIGGER suggestions_sync_update AFTER UPDATE ON suggestions
FOR EACH ROW WHEN NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE suggestions SET sync_clock = arca_clock() WHERE suggestion_id = NEW.suggestion_id;
END;

CREATE TRIGGER suggestions_sync_delete AFTER DELETE ON suggestions
FOR EACH ROW WHEN OLD.uuid IS NOT NULL
BEGIN
  INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES ('suggestion', OLD.uuid, arca_clock())
    ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = excluded.sync_clock;
END;

-- O trigger de updated_at da migration 01 também disparava ao aplicar uma mudança remota com o
-- mesmo updated_at; a atualização dele recarimbava sync_clock e a linha voltava ao outro aparelho
-- para sempre. Agora ele só reage a escritas locais (as que não mudam sync_clock).
DROP TRIGGER update_tasks_updated_at;
CREATE TRIGGER update_tasks_updated_at
AFTER UPDATE ON tasks
FOR EACH ROW
WHEN OLD.updated_at = NEW.updated_at AND NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE tasks
  SET updated_at = DATETIME('now', 'localtime')
  WHERE task_id = OLD.task_id;
END;
