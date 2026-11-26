-- 0.9.19, etapa 4 (007, T021 / R8-N8): a visão da raiz (agora `peca_raiz`) passa a dar, num círculo de junções (duas peças juntadas
-- uma na outra em caixas sem rede), a mesma raiz que `estoque::raiz`: o menor id dos nós do círculo. Antes, no círculo
-- cada peça ficava como ela mesma: a venda de B era somada em B e o estoque dela era lançado em A (o menor).
-- Fora do círculo nada muda: a raiz é a peça que não foi juntada. A peça que leva a um círculo fica com a raiz dele.
-- A visão segue `estoque::raiz` passo a passo (Cód-3 da R3), inclusive acima de 64 passos: no máximo 64 passos; a peça
-- sem junção no fim; no círculo, o menor id do trecho que repetiu; sem fim nem círculo em 64 passos, a peça do passo
-- 64. A linha sem `produto_id` (T029) fica de fora.
--
-- Nome novo, `peca_raiz`: uma visão nunca é recriada com o mesmo nome e outra definição (a estrutura tem de ser
-- distinguível pelo conteúdo; `tests/migracoes_linhagem.rs`). A `produto_raiz` da 0031 sai.
--
-- Só troca a visão: nenhuma tabela muda, nenhum registro entra ou sai.

DROP VIEW produto_raiz;

CREATE VIEW peca_raiz AS
WITH RECURSIVE passo (produto_id, n, no) AS (
    -- o caminho de cada peça juntada: a posição 0 é ela mesma; cada passo segue a junção; no máximo 64 passos
    SELECT produto_id, 0, produto_id FROM produto_juncao WHERE produto_id IS NOT NULL
    UNION ALL
    SELECT p.produto_id, p.n + 1, j.para_id FROM passo p JOIN produto_juncao j ON j.produto_id = p.no
     WHERE p.n < 64
),
repete (produto_id, k) AS (
    -- a primeira posição que repete uma anterior (o círculo foi visto)
    SELECT a.produto_id, MIN(a.n) FROM passo a
      JOIN passo b ON b.produto_id = a.produto_id AND b.no = a.no AND b.n < a.n
     GROUP BY a.produto_id
),
pecas AS (SELECT DISTINCT produto_id FROM passo)
SELECT x.produto_id,
       COALESCE(
           -- 1. a peça que não foi juntada, no fim do caminho
           (SELECT c.no FROM passo c
             WHERE c.produto_id = x.produto_id
               AND NOT EXISTS (SELECT 1 FROM produto_juncao j WHERE j.produto_id = c.no)
             LIMIT 1),
           -- 2. no círculo: o menor id do trecho entre a primeira vez do nó que repetiu e a repetição
           (SELECT MIN(c.no) FROM passo c, repete r
             WHERE r.produto_id = x.produto_id AND c.produto_id = x.produto_id
               AND c.n < r.k
               AND c.n >= (SELECT MIN(b.n) FROM passo b
                            WHERE b.produto_id = x.produto_id
                              AND b.no = (SELECT a.no FROM passo a WHERE a.produto_id = x.produto_id AND a.n = r.k))),
           -- 3. 64 passos sem fim e sem círculo visto: a peça do passo 64 (como `estoque::raiz`)
           (SELECT c.no FROM passo c WHERE c.produto_id = x.produto_id AND c.n = 64)
       ) AS raiz
  FROM pecas x;
