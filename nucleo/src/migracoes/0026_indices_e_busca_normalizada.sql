-- Rustinho · migração 0026 (0.9.18-dev.3.3, passo 7.5: desempenho da busca e da
-- lista de Peças). Só aditiva: nenhum dado que existia muda.
--
-- O número é PROVISÓRIO: só se fixa no commit do projeto (MIG-1 (a)). Na
-- preparação (ramo prod-f75) ela era a 0025; na integração a 0025 ficou com o
-- registro de caixas do protocolo 4 (`0025_caixas_aprovados.sql`), que entrou
-- antes, e esta foi renumerada para 0026. A linha paralela dev.3 não tem 0026;
-- um banco dela continua recusado pela conferência da estrutura (DP-50).

-- B01: o índice composto do estoque que a 0006 quis criar. Lá o comando foi
-- `CREATE INDEX IF NOT EXISTS estoque_movimento_produto ...`, mas esse nome já
-- existia desde a 0001 (só com `produto_id`), e o SQLite não fez nada: o índice
-- nunca existiu. Agora, com nome novo (nunca reaproveitar nome de índice, B02).
-- Serve aos "parados" (a última entrada de cada peça, por tipo e data).
CREATE INDEX estoque_movimento_produto_tipo_data ON estoque_movimento (produto_id, tipo, criado_em);

-- B04 e PEND-39: o nome da peça sem acento e em minúsculas, gravado
-- (`produto.nome_normalizado`, o mesmo `normalizar(nome)` que a lista usava),
-- para a lista "alteradas por último" desempatar por uma coluna com índice, sem
-- chamar a função do Rustinho para cada peça a cada parte. É derivada do nome
-- (DP-15): quem grava a peça (`catalogo::produtos`) grava as duas juntas.
ALTER TABLE produto ADD COLUMN nome_normalizado TEXT NOT NULL DEFAULT '';

-- O preenchimento das peças que já existem não é alteração da loja: não entra
-- no log da troca de dados (os gatilhos da troca ficam parados enquanto grava,
-- como na marca da restauração do backup). Tudo na transação da migração.
UPDATE sync_estado SET aplicando = 1 WHERE id = 1;
UPDATE produto SET nome_normalizado = normalizar(nome);
UPDATE sync_estado SET aplicando = 0 WHERE id = 1;

-- A lista sem busca na ordem da tela (alteradas por último; empate: o nome sem
-- acento, o código e o id), lida pelo índice, parte a parte. O `ativo` no fim
-- serve ao filtro das inativas sem ler a tabela.
CREATE INDEX produto_alteradas_por_ultimo ON produto (atualizado_em DESC, nome_normalizado, codigo, id, ativo);

-- PEND-39 (a busca com texto): o filtro grosso da busca (`busca LIKE '%…%'`, e
-- os códigos e o nome da peça que ela confere depois) lê só este índice, bem
-- menor que a tabela (sem o fiscal, as observações...), e não a tabela inteira.
-- O resultado é o mesmo (a ordem final não depende da ordem da leitura). O
-- `ativo` vai no fim: na frente, ele atrairia a lista sem busca para este índice
-- (que não tem a ordem dela) e a faria ordenar tudo de novo.
CREATE INDEX produto_texto_de_busca ON produto (busca, codigos, nome, id, ativo);
