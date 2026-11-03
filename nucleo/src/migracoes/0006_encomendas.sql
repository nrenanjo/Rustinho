-- Encomendas: a peça que o cliente pediu e a loja não tem.
--
-- situacao: aberta (falta pedir ao fornecedor), pedida, chegou, entregue,
-- cancelada. Se o cliente já pagou (tudo ou um sinal), venda_id aponta a
-- venda; o que falta pagar fica no Receber dessa venda.
CREATE TABLE encomenda (
    id            TEXT PRIMARY KEY,
    numero        INTEGER NOT NULL,
    cliente_id    TEXT REFERENCES pessoa (id),
    cliente_nome  TEXT NOT NULL DEFAULT '',
    telefone      TEXT NOT NULL DEFAULT '',
    -- peça cadastrada (ou vazio, só com a descrição)
    produto_id    TEXT REFERENCES produto (id),
    descricao     TEXT NOT NULL,
    -- milésimos
    quantidade    INTEGER NOT NULL,
    venda_id      TEXT,
    situacao      TEXT NOT NULL DEFAULT 'aberta',
    -- situação antes de "chegou" (para voltar se a compra for estornada)
    situacao_antes TEXT NOT NULL DEFAULT '',
    fornecedor_id TEXT REFERENCES pessoa (id),
    compra_id     TEXT,
    observacoes   TEXT NOT NULL DEFAULT '',
    pedida_em     TEXT,
    chegou_em     TEXT,
    avisado_em    TEXT,
    entregue_em   TEXT,
    -- venda feita na entrega (quando não foi paga antes)
    venda_entrega_id TEXT,
    cancelada_em  TEXT,
    motivo        TEXT NOT NULL DEFAULT '',
    criado_em     TEXT NOT NULL,
    criado_por    TEXT,
    atualizado_em TEXT NOT NULL,
    dispositivo   TEXT NOT NULL
);

CREATE INDEX encomenda_situacao ON encomenda (situacao, criado_em);
CREATE INDEX encomenda_produto ON encomenda (produto_id, situacao);
CREATE INDEX encomenda_venda ON encomenda (venda_id);
CREATE INDEX encomenda_compra ON encomenda (compra_id);

-- Para as contas de "mais vendidos" e "parados".
CREATE INDEX IF NOT EXISTS venda_item_produto ON venda_item (produto_id);
CREATE INDEX IF NOT EXISTS venda_criado ON venda (tipo, situacao, criado_em);
CREATE INDEX IF NOT EXISTS estoque_movimento_produto ON estoque_movimento (produto_id, tipo, criado_em);
