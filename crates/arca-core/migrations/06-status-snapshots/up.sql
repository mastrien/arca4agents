-- Foto diária do tamanho da estante e do carrinho, para os gráficos ao longo do tempo.
-- O banco só guarda o status atual de cada tarefa; sem esta tabela o passado se perde.
--
-- Uma linha por dia, com o estado do fim do dia: os triggers abaixo recontam a cada escrita em
-- `tasks` (app, CLI, MCP, sincronização), e a última escrita do dia vence. Um dia sem linha é
-- um dia sem escritas, ou seja, igual ao último dia registrado antes dele.
-- Não entra na sincronização: cada aparelho conta a partir das próprias tarefas, que já são
-- sincronizadas.
CREATE TABLE status_snapshots (
  snapshot_date DATE PRIMARY KEY,
  shelf INTEGER NOT NULL,
  cart INTEGER NOT NULL
);

-- O `WHERE true` desfaz a ambiguidade entre o SELECT e o ON CONFLICT do upsert.
CREATE TRIGGER status_snapshot_after_insert AFTER INSERT ON tasks
BEGIN
  INSERT INTO status_snapshots (snapshot_date, shelf, cart)
    SELECT DATE('now', 'localtime'), COALESCE(SUM(task_status = 'SHELF'), 0), COALESCE(SUM(task_status = 'CART'), 0)
    FROM tasks WHERE true
    ON CONFLICT(snapshot_date) DO UPDATE SET shelf = excluded.shelf, cart = excluded.cart;
END;

CREATE TRIGGER status_snapshot_after_update AFTER UPDATE OF task_status ON tasks
WHEN OLD.task_status IS NOT NEW.task_status
BEGIN
  INSERT INTO status_snapshots (snapshot_date, shelf, cart)
    SELECT DATE('now', 'localtime'), COALESCE(SUM(task_status = 'SHELF'), 0), COALESCE(SUM(task_status = 'CART'), 0)
    FROM tasks WHERE true
    ON CONFLICT(snapshot_date) DO UPDATE SET shelf = excluded.shelf, cart = excluded.cart;
END;

CREATE TRIGGER status_snapshot_after_delete AFTER DELETE ON tasks
BEGIN
  INSERT INTO status_snapshots (snapshot_date, shelf, cart)
    SELECT DATE('now', 'localtime'), COALESCE(SUM(task_status = 'SHELF'), 0), COALESCE(SUM(task_status = 'CART'), 0)
    FROM tasks WHERE true
    ON CONFLICT(snapshot_date) DO UPDATE SET shelf = excluded.shelf, cart = excluded.cart;
END;

-- Primeira foto: o estado de hoje.
INSERT INTO status_snapshots (snapshot_date, shelf, cart)
  SELECT DATE('now', 'localtime'), COALESCE(SUM(task_status = 'SHELF'), 0), COALESCE(SUM(task_status = 'CART'), 0)
  FROM tasks;
