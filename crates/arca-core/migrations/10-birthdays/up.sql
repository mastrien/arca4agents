-- Anotador de aniversários, agrupando as pessoas por grupo livre.
CREATE TABLE birthdays (
  birthday_id INTEGER PRIMARY KEY AUTOINCREMENT,
  person_name TEXT NOT NULL,
  group_name TEXT NOT NULL DEFAULT 'Geral',
  birth_month INTEGER NOT NULL CHECK(birth_month BETWEEN 1 AND 12),
  birth_day INTEGER NOT NULL CHECK(birth_day BETWEEN 1 AND 31),
  birth_year INTEGER,
  note TEXT,
  created_at DATETIME DEFAULT (DATETIME('now', 'localtime')),
  uuid TEXT,
  sync_clock TEXT
);

CREATE UNIQUE INDEX birthdays_uuid ON birthdays(uuid);
CREATE INDEX birthdays_sync_clock ON birthdays(sync_clock);

CREATE TRIGGER birthdays_sync_insert AFTER INSERT ON birthdays
FOR EACH ROW WHEN NEW.uuid IS NULL OR NEW.sync_clock IS NULL
BEGIN
  UPDATE birthdays SET uuid = COALESCE(NEW.uuid, arca_uuid()), sync_clock = COALESCE(NEW.sync_clock, arca_clock())
  WHERE birthday_id = NEW.birthday_id;
END;

CREATE TRIGGER birthdays_sync_update AFTER UPDATE ON birthdays
FOR EACH ROW WHEN NEW.sync_clock IS OLD.sync_clock
BEGIN
  UPDATE birthdays SET sync_clock = arca_clock() WHERE birthday_id = NEW.birthday_id;
END;

CREATE TRIGGER birthdays_sync_delete AFTER DELETE ON birthdays
FOR EACH ROW WHEN OLD.uuid IS NOT NULL
BEGIN
  INSERT INTO sync_tombstones (entity, uuid, sync_clock) VALUES ('birthday', OLD.uuid, arca_clock())
    ON CONFLICT(entity, uuid) DO UPDATE SET sync_clock = excluded.sync_clock;
END;
