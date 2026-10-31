-- Uma leitura por (agente, janela, instante). Permite que o celular receba as leituras do PC
-- pela sincronização sem duplicar quando o mesmo intervalo chega duas vezes.
DELETE FROM usage_samples WHERE sample_id NOT IN (
  SELECT MAX(sample_id) FROM usage_samples GROUP BY agent, window_key, sampled_at
);
DROP INDEX usage_samples_series;
CREATE UNIQUE INDEX usage_samples_series ON usage_samples(agent, window_key, sampled_at);
