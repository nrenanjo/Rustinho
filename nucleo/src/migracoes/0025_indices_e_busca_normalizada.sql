-- Rustinho · migração 0025 (0.9.18-dev.3.3, passo 7.5: desempenho da busca e da
-- lista de Peças). Só aditiva: nenhum dado que existia muda.
--
-- O número é PROVISÓRIO: só se fixa no commit do projeto (MIG-1 (a)); quem
-- entrar antes fica com a 0025 e o outro renumera. Esta 0025 NÃO é a 0025 da
-- linha paralela dev.3 (`0025_cancelamento_idempotente.sql`): um banco daquela
-- linha continua recusado pela conferência da estrutura (DP-50).

-- B01: o índice composto do estoque que a 0006 quis criar. Lá o comando foi
-- `CREATE INDEX IF NOT EXISTS estoque_movimento_produto ...`, mas esse nome já
-- existia desde a 0001 (só com `produto_id`), e o SQLite não fez nada: o índice
-- nunca existiu. Agora, com nome novo (nunca reaproveitar nome de índice, B02).
-- Serve aos "parados" (a última entrada de cada peça, por tipo e data).
CREATE INDEX estoque_movimento_produto_tipo_data ON estoque_movimento (produto_id, tipo, criado_em);
