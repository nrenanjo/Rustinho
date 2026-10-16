-- Rustinho: Etapa 3 — vendas, orçamentos, caixa e sincronização entre caixas.

-- Compra: quando o custo baixa, a pessoa escolhe se o preço baixa junto.
ALTER TABLE compra_item ADD COLUMN baixar_preco INTEGER NOT NULL DEFAULT 0;

-- Com 4 caixas trabalhando ao mesmo tempo (até sem rede), dois caixas
-- podem criar o mesmo código. Por isso os códigos deixam de ser "únicos" no
-- banco; o Rustinho continua conferindo ao salvar e avisa se repetir.
DROP INDEX produto_codigo;
CREATE INDEX produto_codigo ON produto (codigo);
DROP INDEX pessoa_codigo;
CREATE INDEX pessoa_codigo ON pessoa (codigo);
DROP INDEX pessoa_documento;
CREATE INDEX pessoa_documento ON pessoa (documento) WHERE documento <> '';
DROP INDEX compra_chave;
CREATE INDEX compra_chave ON compra (chave) WHERE chave <> '';

-- Caixa: cada computador abre e fecha o seu.
CREATE TABLE caixa_sessao (
    id            TEXT PRIMARY KEY,
    dispositivo   TEXT NOT NULL,
    caixa_nome    TEXT NOT NULL,
    aberto_em     TEXT NOT NULL,
    aberto_por    TEXT,
    valor_inicial INTEGER NOT NULL DEFAULT 0,
    fechado_em    TEXT,
    fechado_por   TEXT,
    -- dinheiro contado na gaveta ao fechar, e o que o sistema esperava
    contado       INTEGER,
    esperado      INTEGER,
    observacoes   TEXT NOT NULL DEFAULT ''
);

CREATE INDEX caixa_sessao_dispositivo ON caixa_sessao (dispositivo, aberto_em);

CREATE TABLE caixa_movimento (
    id         TEXT PRIMARY KEY,
    sessao_id  TEXT NOT NULL REFERENCES caixa_sessao (id),
    -- sangria tira dinheiro da gaveta; suprimento põe
    tipo       TEXT NOT NULL CHECK (tipo IN ('sangria', 'suprimento')),
    valor      INTEGER NOT NULL,
    motivo     TEXT NOT NULL DEFAULT '',
    usuario_id TEXT,
    criado_em  TEXT NOT NULL
);

CREATE INDEX caixa_movimento_sessao ON caixa_movimento (sessao_id);

-- Venda e orçamento usam a mesma estrutura.
-- situacao da venda: concluida, cancelada
-- situacao do orçamento: aberto, vendido, cancelado
CREATE TABLE venda (
    id                  TEXT PRIMARY KEY,
    tipo                TEXT NOT NULL CHECK (tipo IN ('venda', 'orcamento')),
    numero              INTEGER NOT NULL,
    dispositivo         TEXT NOT NULL,
    caixa_nome          TEXT NOT NULL,
    situacao            TEXT NOT NULL,
    sessao_id           TEXT,
    cliente_id          TEXT,
    cliente_nome        TEXT NOT NULL DEFAULT '',
    cliente_documento   TEXT NOT NULL DEFAULT '',
    vendedor_id         TEXT,
    tabela              INTEGER NOT NULL DEFAULT 1,
    subtotal            INTEGER NOT NULL,
    desconto            INTEGER NOT NULL DEFAULT 0,
    total               INTEGER NOT NULL,
    troco               INTEGER NOT NULL DEFAULT 0,
    validade            TEXT,
    observacoes         TEXT NOT NULL DEFAULT '',
    orcamento_id        TEXT,
    criado_em           TEXT NOT NULL,
    cancelado_em        TEXT,
    cancelado_por       TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

CREATE INDEX venda_tipo_data ON venda (tipo, criado_em);
CREATE INDEX venda_sessao ON venda (sessao_id);
CREATE INDEX venda_cliente ON venda (cliente_id);

CREATE TABLE venda_item (
    id         TEXT PRIMARY KEY,
    venda_id   TEXT NOT NULL REFERENCES venda (id) ON DELETE CASCADE,
    ordem      INTEGER NOT NULL,
    produto_id TEXT,
    codigo     TEXT NOT NULL DEFAULT '',
    descricao  TEXT NOT NULL,
    unidade    TEXT NOT NULL DEFAULT 'UN',
    -- milésimos
    quantidade INTEGER NOT NULL,
    -- preço de cada unidade, antes do desconto
    preco      INTEGER NOT NULL,
    -- desconto do item (em centavos, já somado)
    desconto   INTEGER NOT NULL DEFAULT 0,
    total      INTEGER NOT NULL,
    -- custo de cada unidade na hora da venda (para lucro e comissão)
    custo      INTEGER NOT NULL DEFAULT 0,
    comissao   INTEGER
);

CREATE INDEX venda_item_venda ON venda_item (venda_id, ordem);
CREATE INDEX venda_item_produto ON venda_item (produto_id);

-- forma: dinheiro, pix, debito, credito, crediario, outro
CREATE TABLE venda_pagamento (
    id       TEXT PRIMARY KEY,
    venda_id TEXT NOT NULL REFERENCES venda (id) ON DELETE CASCADE,
    forma    TEXT NOT NULL,
    valor    INTEGER NOT NULL
);

CREATE INDEX venda_pagamento_venda ON venda_pagamento (venda_id);

-- ---------------------------------------------------------------------------
-- Sincronização entre os caixas.
--
-- Toda alteração nas tabelas da loja vira uma linha em "alteracao" (feita
-- por gatilhos que o Rustinho cria ao abrir). Cada caixa pede aos outros as
-- alterações que ainda não tem e aplica. Vale a alteração mais recente de
-- cada registro (o id é um UUID v7, que já vem em ordem de tempo).
-- ---------------------------------------------------------------------------

CREATE TABLE alteracao (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    id          TEXT NOT NULL UNIQUE,
    tabela      TEXT NOT NULL,
    -- chave primária do registro, em JSON (ex.: ["id-da-peça"])
    chave       TEXT NOT NULL,
    operacao    TEXT NOT NULL CHECK (operacao IN ('gravar', 'apagar')),
    -- o registro inteiro em JSON (vazio quando apaga)
    dados       TEXT,
    dispositivo TEXT NOT NULL,
    quando      TEXT NOT NULL
);

CREATE INDEX alteracao_registro ON alteracao (tabela, chave, id);

-- 1 enquanto o Rustinho aplica alterações vindas de outro caixa (para os
-- gatilhos não registrarem de novo).
CREATE TABLE sync_estado (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    aplicando INTEGER NOT NULL DEFAULT 0
);

INSERT INTO sync_estado (id, aplicando) VALUES (1, 0);

-- Os outros caixas que este já encontrou.
CREATE TABLE sync_par (
    dispositivo     TEXT PRIMARY KEY,
    nome            TEXT NOT NULL DEFAULT '',
    endereco        TEXT NOT NULL DEFAULT '',
    -- última alteração (seq do outro caixa) que este já recebeu
    cursor          INTEGER NOT NULL DEFAULT 0,
    visto_em        TEXT,
    sincronizado_em TEXT,
    erro            TEXT NOT NULL DEFAULT ''
);
