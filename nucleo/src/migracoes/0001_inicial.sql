-- Rustinho: estrutura inicial do banco de dados (Etapa 1).
--
-- Regras gerais:
-- * Os identificadores (id) são textos únicos (UUID v7). Assim, no futuro,
--   os 4 caixas podem criar registros sem um atrapalhar o outro.
-- * Dinheiro é guardado em centavos (inteiro): R$ 85,50 = 8550.
-- * Quantidades são guardadas em milésimos (inteiro): 1,5 = 1500.
-- * Percentuais são guardados em centésimos de ponto: 46,55% = 4655.
-- * Datas e horas em texto no padrão ISO 8601, sempre em UTC.
-- * O estoque não é um número solto: é a soma dos movimentos de estoque.

CREATE TABLE config (
    chave TEXT PRIMARY KEY,
    valor TEXT NOT NULL
);

CREATE TABLE usuario (
    id            TEXT PRIMARY KEY,
    nome          TEXT NOT NULL,
    senha_hash    TEXT NOT NULL,
    ativo         INTEGER NOT NULL DEFAULT 1,
    criado_em     TEXT NOT NULL,
    atualizado_em TEXT NOT NULL,
    ultimo_acesso TEXT,
    dispositivo   TEXT NOT NULL
);

CREATE TABLE tabela_preco (
    numero       INTEGER PRIMARY KEY CHECK (numero BETWEEN 1 AND 5),
    nome         TEXT NOT NULL,
    lucro_padrao INTEGER
);

INSERT INTO tabela_preco (numero, nome, lucro_padrao) VALUES
    (1, 'Revenda', 10000),
    (2, 'Tabela 2', NULL),
    (3, 'Tabela 3', NULL),
    (4, 'Tabela 4', NULL),
    (5, 'Tabela 5', NULL);

CREATE TABLE produto (
    id              TEXT PRIMARY KEY,
    codigo          TEXT NOT NULL,
    nome            TEXT NOT NULL,
    marca           TEXT NOT NULL DEFAULT '',
    unidade         TEXT NOT NULL DEFAULT 'UN',
    codigo_barras   TEXT NOT NULL DEFAULT '',
    codigo_original TEXT NOT NULL DEFAULT '',
    categoria       TEXT NOT NULL DEFAULT '',
    custo           INTEGER NOT NULL DEFAULT 0,
    estoque_minimo  INTEGER NOT NULL DEFAULT 0,
    local           TEXT NOT NULL DEFAULT '',
    ncm             TEXT NOT NULL DEFAULT '',
    cest            TEXT NOT NULL DEFAULT '',
    origem          INTEGER NOT NULL DEFAULT 0,
    anp_codigo      TEXT NOT NULL DEFAULT '',
    anp_descricao   TEXT NOT NULL DEFAULT '',
    peso_gramas     INTEGER NOT NULL DEFAULT 0,
    desconto_maximo INTEGER,
    comissao        INTEGER,
    observacoes     TEXT NOT NULL DEFAULT '',
    ativo           INTEGER NOT NULL DEFAULT 1,
    -- texto para a busca (nome, marca, códigos, carros...), sem acentos
    busca           TEXT NOT NULL DEFAULT '',
    -- todos os códigos compactos entre espaços, para achar o código exato
    codigos         TEXT NOT NULL DEFAULT '',
    criado_em       TEXT NOT NULL,
    criado_por      TEXT,
    atualizado_em   TEXT NOT NULL,
    atualizado_por  TEXT,
    dispositivo     TEXT NOT NULL
);

CREATE UNIQUE INDEX produto_codigo ON produto (codigo);
CREATE INDEX produto_codigo_barras ON produto (codigo_barras) WHERE codigo_barras <> '';
CREATE INDEX produto_nome ON produto (nome);

CREATE TABLE produto_preco (
    produto_id TEXT NOT NULL REFERENCES produto (id) ON DELETE CASCADE,
    tabela     INTEGER NOT NULL REFERENCES tabela_preco (numero),
    preco      INTEGER NOT NULL,
    lucro      INTEGER,
    PRIMARY KEY (produto_id, tabela)
);

CREATE TABLE produto_aplicacao (
    id          TEXT PRIMARY KEY,
    produto_id  TEXT NOT NULL REFERENCES produto (id) ON DELETE CASCADE,
    ordem       INTEGER NOT NULL DEFAULT 0,
    montadora   TEXT NOT NULL DEFAULT '',
    modelo      TEXT NOT NULL DEFAULT '',
    motor       TEXT NOT NULL DEFAULT '',
    ano_inicio  INTEGER,
    ano_fim     INTEGER,
    observacao  TEXT NOT NULL DEFAULT ''
);

CREATE INDEX produto_aplicacao_produto ON produto_aplicacao (produto_id);

CREATE TABLE produto_equivalente (
    id              TEXT PRIMARY KEY,
    produto_id      TEXT NOT NULL REFERENCES produto (id) ON DELETE CASCADE,
    ordem           INTEGER NOT NULL DEFAULT 0,
    marca           TEXT NOT NULL DEFAULT '',
    codigo          TEXT NOT NULL,
    codigo_compacto TEXT NOT NULL
);

CREATE INDEX produto_equivalente_produto ON produto_equivalente (produto_id);
CREATE INDEX produto_equivalente_codigo ON produto_equivalente (codigo_compacto);

CREATE TABLE estoque_movimento (
    id          TEXT PRIMARY KEY,
    produto_id  TEXT NOT NULL REFERENCES produto (id),
    quantidade  INTEGER NOT NULL,
    tipo        TEXT NOT NULL,
    motivo      TEXT NOT NULL DEFAULT '',
    usuario_id  TEXT,
    dispositivo TEXT NOT NULL,
    criado_em   TEXT NOT NULL
);

CREATE INDEX estoque_movimento_produto ON estoque_movimento (produto_id);

CREATE TABLE menu_uso (
    usuario_id TEXT NOT NULL REFERENCES usuario (id),
    item       TEXT NOT NULL,
    usos       INTEGER NOT NULL DEFAULT 0,
    fixo       INTEGER NOT NULL DEFAULT 0,
    ultimo_uso TEXT,
    PRIMARY KEY (usuario_id, item)
);

CREATE TABLE auditoria (
    id          TEXT PRIMARY KEY,
    quando      TEXT NOT NULL,
    usuario_id  TEXT,
    dispositivo TEXT NOT NULL,
    acao        TEXT NOT NULL,
    entidade    TEXT NOT NULL DEFAULT '',
    entidade_id TEXT NOT NULL DEFAULT '',
    detalhes    TEXT NOT NULL DEFAULT ''
);

CREATE INDEX auditoria_entidade ON auditoria (entidade, entidade_id, quando);
