-- Etapa 6 (começo): ordem de serviço da oficina.

CREATE TABLE ordem_servico (
    id            TEXT PRIMARY KEY,
    numero        INTEGER NOT NULL,
    dispositivo   TEXT NOT NULL,
    caixa_nome    TEXT NOT NULL,
    -- orcamento → aprovada (em serviço) → pronta → entregue; ou cancelada
    situacao      TEXT NOT NULL DEFAULT 'orcamento'
                  CHECK (situacao IN ('orcamento', 'aprovada', 'pronta', 'entregue', 'cancelada')),
    cliente_id    TEXT,
    cliente_nome  TEXT NOT NULL DEFAULT '',
    telefone      TEXT NOT NULL DEFAULT '',
    placa         TEXT NOT NULL DEFAULT '',
    veiculo       TEXT NOT NULL DEFAULT '',
    km            TEXT NOT NULL DEFAULT '',
    -- o que o cliente reclamou e o que o mecânico achou
    defeito       TEXT NOT NULL DEFAULT '',
    diagnostico   TEXT NOT NULL DEFAULT '',
    observacoes   TEXT NOT NULL DEFAULT '',
    mecanico_id   TEXT,
    -- AAAA-MM-DD
    previsao      TEXT,
    -- desconto no total (centavos)
    desconto      INTEGER NOT NULL DEFAULT 0,
    venda_id      TEXT,
    criado_em     TEXT NOT NULL,
    criado_por    TEXT,
    atualizado_em TEXT NOT NULL,
    aprovada_em   TEXT,
    pronta_em     TEXT,
    entregue_em   TEXT,
    cancelada_em  TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

CREATE TABLE ordem_servico_item (
    id          TEXT PRIMARY KEY,
    ordem_id    TEXT NOT NULL REFERENCES ordem_servico (id) ON DELETE CASCADE,
    ordem       INTEGER NOT NULL,
    -- peca (do estoque) ou servico (mão de obra)
    tipo        TEXT NOT NULL CHECK (tipo IN ('peca', 'servico')),
    produto_id  TEXT,
    codigo      TEXT NOT NULL DEFAULT '',
    descricao   TEXT NOT NULL,
    unidade     TEXT NOT NULL DEFAULT 'UN',
    -- milésimos
    quantidade  INTEGER NOT NULL,
    preco       INTEGER NOT NULL,
    desconto    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX ordem_servico_situacao ON ordem_servico (situacao, criado_em);
CREATE INDEX ordem_servico_placa ON ordem_servico (placa);
CREATE INDEX ordem_servico_venda ON ordem_servico (venda_id);
CREATE INDEX ordem_servico_item_ordem ON ordem_servico_item (ordem_id);
