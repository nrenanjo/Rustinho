-- Etapa 0.5: trocas e devoluções, com vale-troca.

-- Devolução de uma venda: o que voltou e como o dinheiro voltou ao cliente.
CREATE TABLE devolucao (
    id             TEXT PRIMARY KEY,
    numero         INTEGER NOT NULL,
    dispositivo    TEXT NOT NULL,
    caixa_nome     TEXT NOT NULL,
    -- caixa aberto na hora (o dinheiro devolvido sai da gaveta dele)
    sessao_id      TEXT,
    venda_id       TEXT NOT NULL REFERENCES venda (id),
    cliente_id     TEXT,
    cliente_nome   TEXT NOT NULL DEFAULT '',
    motivo         TEXT NOT NULL DEFAULT '',
    -- total devolvido e como (em centavos; a soma das partes é o total)
    valor          INTEGER NOT NULL,
    dinheiro       INTEGER NOT NULL DEFAULT 0,
    pix            INTEGER NOT NULL DEFAULT 0,
    vale           INTEGER NOT NULL DEFAULT 0,
    crediario      INTEGER NOT NULL DEFAULT 0,
    vale_id        TEXT,
    recebimento_id TEXT,
    usuario_id     TEXT,
    criado_em      TEXT NOT NULL,
    cancelado_em   TEXT,
    cancelado_por  TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

CREATE TABLE devolucao_item (
    id            TEXT PRIMARY KEY,
    devolucao_id  TEXT NOT NULL REFERENCES devolucao (id) ON DELETE CASCADE,
    venda_item_id TEXT NOT NULL REFERENCES venda_item (id),
    produto_id    TEXT,
    descricao     TEXT NOT NULL,
    unidade       TEXT NOT NULL DEFAULT 'UN',
    -- milésimos
    quantidade    INTEGER NOT NULL,
    -- parte do total do item (antes do desconto da venda) e o devolvido
    valor_item    INTEGER NOT NULL,
    valor         INTEGER NOT NULL,
    custo         INTEGER NOT NULL DEFAULT 0,
    -- 0 = com defeito: não volta para a prateleira
    volta_estoque INTEGER NOT NULL DEFAULT 1
);

-- Vale-troca: crédito para usar em outra compra (de uma vez ou aos poucos).
CREATE TABLE vale (
    id            TEXT PRIMARY KEY,
    numero        INTEGER NOT NULL,
    dispositivo   TEXT NOT NULL,
    caixa_nome    TEXT NOT NULL,
    cliente_id    TEXT,
    cliente_nome  TEXT NOT NULL DEFAULT '',
    valor         INTEGER NOT NULL,
    devolucao_id  TEXT,
    usuario_id    TEXT,
    criado_em     TEXT NOT NULL,
    cancelado_em  TEXT,
    cancelado_por TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

-- Pagamento com vale-troca: qual vale foi usado.
ALTER TABLE venda_pagamento ADD COLUMN vale_id TEXT;

CREATE INDEX devolucao_venda ON devolucao (venda_id);
CREATE INDEX devolucao_criado ON devolucao (criado_em);
CREATE INDEX devolucao_item_venda_item ON devolucao_item (venda_item_id);
CREATE INDEX venda_pagamento_vale ON venda_pagamento (vale_id);

-- Quanto ainda há em cada vale (venda cancelada devolve o que usou).
CREATE VIEW vale_saldo AS
SELECT v.*,
       v.valor - COALESCE((SELECT SUM(p.valor) FROM venda_pagamento p JOIN venda x ON x.id = p.venda_id
                            WHERE p.vale_id = v.id AND x.situacao = 'concluida'), 0) AS saldo
  FROM vale v;
