-- Rustinho · migração 0024 (0.9.18): múltiplos GTINs por peça (comercial vs
-- tributável, com origem preservada) e pendências persistentes de identificação
-- na importação de XML.

CREATE TABLE IF NOT EXISTS produto_gtin (
    id          TEXT PRIMARY KEY,
    produto_id  TEXT NOT NULL REFERENCES produto(id) ON DELETE CASCADE,
    gtin        TEXT NOT NULL,
    tipo        TEXT NOT NULL DEFAULT 'comercial' CHECK (tipo IN ('comercial', 'tributavel')),
    origem      TEXT NOT NULL DEFAULT 'manual' CHECK (origem IN ('cEAN', 'cEANTrib', 'cBarra', 'cBarraTrib', 'manual')),
    ordem       INTEGER NOT NULL DEFAULT 0,
    criado_em   TEXT NOT NULL DEFAULT '',
    UNIQUE (produto_id, gtin, tipo, origem)
);

CREATE INDEX IF NOT EXISTS produto_gtin_produto ON produto_gtin(produto_id, ordem);
CREATE INDEX IF NOT EXISTS produto_gtin_codigo  ON produto_gtin(gtin);

CREATE TABLE IF NOT EXISTS xml_pendencia_identificacao (
    id                   TEXT PRIMARY KEY,
    fornecedor_documento TEXT NOT NULL DEFAULT '',
    fornecedor_nome      TEXT NOT NULL DEFAULT '',
    fornecedor_id        TEXT REFERENCES pessoa(id) ON DELETE SET NULL,
    chave_nfe            TEXT NOT NULL DEFAULT '',
    numero_nfe           TEXT NOT NULL DEFAULT '',
    emissao_nfe          TEXT NOT NULL DEFAULT '',
    codigo_fornecedor    TEXT NOT NULL DEFAULT '',
    codigo_compacto      TEXT NOT NULL DEFAULT '',
    gtin_comercial       TEXT NOT NULL DEFAULT '',
    origem_comercial     TEXT NOT NULL DEFAULT '',
    gtin_tributavel      TEXT NOT NULL DEFAULT '',
    origem_tributavel    TEXT NOT NULL DEFAULT '',
    gtins_json           TEXT NOT NULL DEFAULT '[]',
    descricao            TEXT NOT NULL DEFAULT '',
    descricao_compacta   TEXT NOT NULL DEFAULT '',
    ncm                  TEXT NOT NULL DEFAULT '',
    cest                 TEXT NOT NULL DEFAULT '',
    cfop                 TEXT NOT NULL DEFAULT '',
    unidade              TEXT NOT NULL DEFAULT 'UN',
    origem_mercadoria    TEXT NOT NULL DEFAULT '0',
    origem_fiscal        INTEGER NOT NULL DEFAULT 0,
    cst_icms             TEXT NOT NULL DEFAULT '',
    cst_pis              TEXT NOT NULL DEFAULT '',
    quantidade           INTEGER NOT NULL DEFAULT 1000,
    valor_unitario       INTEGER NOT NULL DEFAULT 0,
    valor_produtos       INTEGER NOT NULL DEFAULT 0,
    frete                INTEGER NOT NULL DEFAULT 0,
    seguro               INTEGER NOT NULL DEFAULT 0,
    desconto             INTEGER NOT NULL DEFAULT 0,
    outras               INTEGER NOT NULL DEFAULT 0,
    ipi                  INTEGER NOT NULL DEFAULT 0,
    icms_st              INTEGER NOT NULL DEFAULT 0,
    motivo               TEXT NOT NULL DEFAULT '',
    candidatos_json      TEXT NOT NULL DEFAULT '[]',
    situacao             TEXT NOT NULL DEFAULT 'pendente' CHECK (situacao IN ('pendente', 'resolvida')),
    produto_id           TEXT REFERENCES produto(id) ON DELETE SET NULL,
    criado_em            TEXT NOT NULL DEFAULT '',
    resolvido_em         TEXT
);

CREATE INDEX IF NOT EXISTS xml_pendencia_situacao   ON xml_pendencia_identificacao(situacao, criado_em DESC);
CREATE INDEX IF NOT EXISTS xml_pendencia_fornecedor ON xml_pendencia_identificacao(fornecedor_documento, codigo_compacto);

ALTER TABLE compra_item ADD COLUMN gtin_tributavel        TEXT NOT NULL DEFAULT '';
ALTER TABLE compra_item ADD COLUMN origem_gtin            TEXT NOT NULL DEFAULT '';
ALTER TABLE compra_item ADD COLUMN origem_gtin_tributavel TEXT NOT NULL DEFAULT '';
ALTER TABLE compra_item ADD COLUMN gtins_json             TEXT NOT NULL DEFAULT '[]';
