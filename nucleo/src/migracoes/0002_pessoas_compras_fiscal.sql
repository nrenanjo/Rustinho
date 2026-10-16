-- Rustinho: Etapa 2 — clientes, fornecedores, compras pelo XML e regras fiscais.

-- Clientes e fornecedores ficam na mesma tabela: uma empresa pode ser as duas
-- coisas (ex.: um mecânico que compra e vende peças para a loja).
CREATE TABLE pessoa (
    id             TEXT PRIMARY KEY,
    codigo         TEXT NOT NULL,
    cliente        INTEGER NOT NULL DEFAULT 0,
    fornecedor     INTEGER NOT NULL DEFAULT 0,
    -- 'F' pessoa física (CPF) ou 'J' pessoa jurídica (CNPJ)
    tipo           TEXT NOT NULL DEFAULT 'F' CHECK (tipo IN ('F', 'J')),
    nome           TEXT NOT NULL,
    fantasia       TEXT NOT NULL DEFAULT '',
    -- CPF ou CNPJ só com números e letras (o CNPJ novo pode ter letras)
    documento      TEXT NOT NULL DEFAULT '',
    ie             TEXT NOT NULL DEFAULT '',
    -- 1 contribuinte do ICMS, 2 isento, 9 não contribuinte (tabela da NF-e)
    indicador_ie   INTEGER NOT NULL DEFAULT 9,
    telefone       TEXT NOT NULL DEFAULT '',
    celular        TEXT NOT NULL DEFAULT '',
    email          TEXT NOT NULL DEFAULT '',
    cep            TEXT NOT NULL DEFAULT '',
    logradouro     TEXT NOT NULL DEFAULT '',
    numero         TEXT NOT NULL DEFAULT '',
    complemento    TEXT NOT NULL DEFAULT '',
    bairro         TEXT NOT NULL DEFAULT '',
    municipio      TEXT NOT NULL DEFAULT '',
    municipio_ibge TEXT NOT NULL DEFAULT '',
    uf             TEXT NOT NULL DEFAULT '',
    limite_credito INTEGER,
    observacoes    TEXT NOT NULL DEFAULT '',
    ativo          INTEGER NOT NULL DEFAULT 1,
    busca          TEXT NOT NULL DEFAULT '',
    criado_em      TEXT NOT NULL,
    criado_por     TEXT,
    atualizado_em  TEXT NOT NULL,
    atualizado_por TEXT,
    dispositivo    TEXT NOT NULL
);

CREATE UNIQUE INDEX pessoa_codigo ON pessoa (codigo);
CREATE UNIQUE INDEX pessoa_documento ON pessoa (documento) WHERE documento <> '';
CREATE INDEX pessoa_nome ON pessoa (nome);

-- Carros do cliente (para achar o cliente pela placa e, depois, na oficina).
CREATE TABLE pessoa_veiculo (
    id        TEXT PRIMARY KEY,
    pessoa_id TEXT NOT NULL REFERENCES pessoa (id) ON DELETE CASCADE,
    ordem     INTEGER NOT NULL DEFAULT 0,
    placa     TEXT NOT NULL DEFAULT '',
    descricao TEXT NOT NULL DEFAULT ''
);

CREATE INDEX pessoa_veiculo_pessoa ON pessoa_veiculo (pessoa_id);
CREATE INDEX pessoa_veiculo_placa ON pessoa_veiculo (placa);

-- Fiscal da peça.
-- csosn vazio = automático (o Rustinho escolhe pela regra de ST do estado).
ALTER TABLE produto ADD COLUMN csosn TEXT NOT NULL DEFAULT '';
-- PIS/Cofins monofásico (Lei 10.485/2002): a receita sai separada no PGDAS-D.
ALTER TABLE produto ADD COLUMN monofasico INTEGER NOT NULL DEFAULT 0;

-- O código que cada fornecedor usa para a peça, para ligar sozinho na
-- próxima nota. fator = quantas unidades da peça vêm em 1 unidade da nota
-- (em milésimos: 1 = 1000; caixa com 10 = 10000).
CREATE TABLE produto_fornecedor (
    fornecedor_id   TEXT NOT NULL REFERENCES pessoa (id) ON DELETE CASCADE,
    codigo_compacto TEXT NOT NULL,
    codigo          TEXT NOT NULL,
    produto_id      TEXT NOT NULL REFERENCES produto (id) ON DELETE CASCADE,
    fator           INTEGER NOT NULL DEFAULT 1000,
    atualizado_em   TEXT NOT NULL,
    PRIMARY KEY (fornecedor_id, codigo_compacto)
);

CREATE INDEX produto_fornecedor_produto ON produto_fornecedor (produto_id);

-- Nota de compra (entrada). situacao: rascunho, lancada, estornada.
CREATE TABLE compra (
    id              TEXT PRIMARY KEY,
    fornecedor_id   TEXT REFERENCES pessoa (id),
    chave           TEXT NOT NULL DEFAULT '',
    numero          TEXT NOT NULL DEFAULT '',
    serie           TEXT NOT NULL DEFAULT '',
    emissao         TEXT NOT NULL DEFAULT '',
    natureza        TEXT NOT NULL DEFAULT '',
    total_produtos  INTEGER NOT NULL DEFAULT 0,
    frete           INTEGER NOT NULL DEFAULT 0,
    seguro          INTEGER NOT NULL DEFAULT 0,
    desconto        INTEGER NOT NULL DEFAULT 0,
    outras          INTEGER NOT NULL DEFAULT 0,
    ipi             INTEGER NOT NULL DEFAULT 0,
    icms_st         INTEGER NOT NULL DEFAULT 0,
    total           INTEGER NOT NULL DEFAULT 0,
    -- frete pago à parte (transportadora), dividido entre as peças
    frete_extra     INTEGER NOT NULL DEFAULT 0,
    atualizar_precos INTEGER NOT NULL DEFAULT 1,
    situacao        TEXT NOT NULL DEFAULT 'rascunho'
                    CHECK (situacao IN ('rascunho', 'lancada', 'estornada')),
    destinatario    TEXT NOT NULL DEFAULT '',
    xml             TEXT NOT NULL DEFAULT '',
    observacoes     TEXT NOT NULL DEFAULT '',
    lancada_em      TEXT,
    lancada_por     TEXT,
    criado_em       TEXT NOT NULL,
    criado_por      TEXT,
    dispositivo     TEXT NOT NULL
);

CREATE UNIQUE INDEX compra_chave ON compra (chave) WHERE chave <> '' AND situacao <> 'estornada';
CREATE INDEX compra_emissao ON compra (emissao);

-- Item da nota. acao: ligar (peça que já existe), nova (cadastrar), ignorar
-- (não entra no estoque, ex.: material de uso da loja).
CREATE TABLE compra_item (
    id                TEXT PRIMARY KEY,
    compra_id         TEXT NOT NULL REFERENCES compra (id) ON DELETE CASCADE,
    ordem             INTEGER NOT NULL,
    codigo_fornecedor TEXT NOT NULL DEFAULT '',
    gtin              TEXT NOT NULL DEFAULT '',
    descricao         TEXT NOT NULL DEFAULT '',
    ncm               TEXT NOT NULL DEFAULT '',
    cest              TEXT NOT NULL DEFAULT '',
    cfop              TEXT NOT NULL DEFAULT '',
    unidade           TEXT NOT NULL DEFAULT '',
    origem            INTEGER NOT NULL DEFAULT 0,
    cst_icms          TEXT NOT NULL DEFAULT '',
    cst_pis           TEXT NOT NULL DEFAULT '',
    quantidade        INTEGER NOT NULL,
    valor_produtos    INTEGER NOT NULL,
    frete             INTEGER NOT NULL DEFAULT 0,
    seguro            INTEGER NOT NULL DEFAULT 0,
    desconto          INTEGER NOT NULL DEFAULT 0,
    outras            INTEGER NOT NULL DEFAULT 0,
    ipi               INTEGER NOT NULL DEFAULT 0,
    icms_st           INTEGER NOT NULL DEFAULT 0,
    acao              TEXT NOT NULL DEFAULT '' CHECK (acao IN ('', 'ligar', 'nova', 'ignorar')),
    produto_id        TEXT REFERENCES produto (id),
    -- como a peça foi achada: fornecedor, barras, codigo (ou vazio)
    achado_por        TEXT NOT NULL DEFAULT '',
    fator             INTEGER NOT NULL DEFAULT 1000
);

CREATE INDEX compra_item_compra ON compra_item (compra_id, ordem);

-- Regras de substituição tributária do ICMS por estado, início do CEST e
-- período. O contador pode mudar estas regras na tela "Conferência fiscal".
CREATE TABLE regra_st (
    id          TEXT PRIMARY KEY,
    uf          TEXT NOT NULL,
    cest_inicio TEXT NOT NULL,
    vigente_de  TEXT NOT NULL,
    vigente_ate TEXT,
    com_st      INTEGER NOT NULL,
    descricao   TEXT NOT NULL DEFAULT ''
);

-- São Paulo: autopeças (CEST 01) e pneus (CEST 16) saíram da ST em
-- 01/10/2026 (Portaria SRE nº 34/2026).
INSERT INTO regra_st (id, uf, cest_inicio, vigente_de, vigente_ate, com_st, descricao) VALUES
    ('sp-autopecas-com-st', 'SP', '01', '2000-01-01', '2026-09-30', 1, 'Autopeças com ST (antigo Anexo XIV da Portaria CAT 68/2019)'),
    ('sp-autopecas-sem-st', 'SP', '01', '2026-10-01', NULL, 0, 'Autopeças fora da ST desde 01/10/2026 (Portaria SRE 34/2026)'),
    ('sp-pneus-com-st', 'SP', '16', '2000-01-01', '2026-09-30', 1, 'Pneus e câmaras com ST'),
    ('sp-pneus-sem-st', 'SP', '16', '2026-10-01', NULL, 0, 'Pneus e câmaras fora da ST desde 01/10/2026 (Portaria SRE 34/2026)');
