-- 0.9.19, etapa 4 (007, T021 / R8-N8): a visão `produto_raiz` passa a dar, num círculo de junções (duas peças juntadas
-- uma na outra em caixas sem rede), a mesma raiz que `estoque::raiz`: o menor id dos nós do círculo. Antes, no círculo
-- cada peça ficava como ela mesma: a venda de B era somada em B e o estoque dela era lançado em A (o menor).
-- Fora do círculo nada muda: a raiz é a peça que não foi juntada. A peça que leva a um círculo fica com a raiz dele.
-- No máximo 64 passos, como em `estoque::raiz`. A linha sem `produto_id` (T029) fica de fora.
--
-- Só recria a visão: nenhuma tabela muda, nenhum registro entra ou sai.

DROP VIEW produto_raiz;

CREATE VIEW produto_raiz AS
WITH RECURSIVE cadeia (produto_id, no, n) AS (
    SELECT produto_id, para_id, 1 FROM produto_juncao WHERE produto_id IS NOT NULL
    UNION
    SELECT c.produto_id, j.para_id, c.n + 1 FROM cadeia c JOIN produto_juncao j ON j.produto_id = c.no
     WHERE c.n < 64
),
pecas AS (SELECT DISTINCT produto_id FROM cadeia)
SELECT p.produto_id,
       COALESCE(
           -- a peça que não foi juntada, no fim da cadeia
           (SELECT c.no FROM cadeia c
             WHERE c.produto_id = p.produto_id
               AND NOT EXISTS (SELECT 1 FROM produto_juncao x WHERE x.produto_id = c.no)
             LIMIT 1),
           -- no círculo: o menor id dos nós que voltam a si mesmos
           (SELECT MIN(c.no) FROM cadeia c
             WHERE c.produto_id = p.produto_id
               AND EXISTS (SELECT 1 FROM cadeia v WHERE v.produto_id = c.no AND v.no = c.no))
       ) AS raiz
  FROM pecas p;
