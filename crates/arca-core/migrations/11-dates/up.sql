-- Acompanhador de datas: eventos e prazos independentes de tarefa.
CREATE TABLE tracked_dates (
  date_id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  target_date DATE NOT NULL,
  project_id INTEGER REFERENCES projects(project_id) ON DELETE SET NULL,
  note TEXT,
  created_at DATETIME DEFAULT (DATETIME('now', 'localtime')),
  uuid TEXT,
  sync_clock TEXT
);

CREATE UNIQUE INDEX tracked_dates_uuid ON tracked_dates(uuid);
CREATE INDEX tracked_dates_sync_clock ON tracked_dates(sync_clock);

CREATE TRIGGER tracked_dates_sync_insert AFTER INSERT ON tracked_dates
FOR EACH ROW WHEN NEW.uuid IS NULL OR NEW.sync_clock IS NULL
BEGIN
  UPDATE tracked_dates SET uuid = COALESCE(NEW.uuid, arca_uuid()), sync_clock = COALESCE(NEW.sync_clock, arca_clock())
  WHERE date_id = NEW.date_id;
END;

CREATE TRIGGER tracked_dates_sync_update AFTER UPDATE ON tracked_dates
FOR EACH ROW WHEN NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE tracked_dates SET sync_clock = arca_clock() WHERE date_id = NEW.date_id;
END;

CREATE TRIGGER tracked_dates_sync_delete AFTER DELETE ON tracked_dates
FOR EACH ROW WHEN OLD.uuid IS NOT NULL
BEGIN
  INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES ('tracked_date', OLD.uuid, arca_clock())
    ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = excluded.sync_clock;
END;

-- Força uma sincronização completa ao atualizar para incluir aniversários e datas já criadas no par.
UPDATE sync_peers SET last_pulled = '', last_pushed = '';
