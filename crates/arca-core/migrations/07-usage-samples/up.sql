-- Cota de uso dos agentes (Claude Code, Codex) ao longo do tempo, para os gráficos.
--
-- Cada linha é uma leitura de uma janela de limite: `window_key` identifica a janela dentro do
-- agente (`five_hour`, `seven_day`, `primary`, ...), `window_minutes` diz o tamanho dela quando a
-- fonte informa, e `resets_at` é quando ela zera (horário local).
-- Só entra uma leitura nova quando o valor muda ou a última ficou velha (ver usage.rs), para a
-- statusline do Claude Code, que roda o tempo todo, não encher o banco.
-- Ainda não entra na sincronização.
CREATE TABLE usage_samples (
  sample_id INTEGER PRIMARY KEY,
  agent TEXT NOT NULL,
  window_key TEXT NOT NULL,
  used_percent REAL NOT NULL,
  window_minutes INTEGER,
  resets_at DATETIME,
  sampled_at DATETIME NOT NULL
);

CREATE INDEX usage_samples_series ON usage_samples(agent, window_key, sampled_at);
