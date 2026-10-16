-- Rustinho: Etapa 4 (parte B) — contas a pagar, contas da loja, cheques,
-- fluxo de caixa e comissões.

-- Contas da loja: onde o dinheiro fica (o cofre "Caixa geral" e os bancos).
-- O saldo é o saldo inicial mais os movimentos, menos os pagamentos.
CREATE TABLE conta_loja (
    id            TEXT PRIMARY KEY,
    nome          TEXT NOT NULL,
    tipo          TEXT NOT NULL CHECK (tipo IN ('caixa', 'banco')),
    saldo_inicial INTEGER NOT NULL DEFAULT 0,
    ativo         INTEGER NOT NULL DEFAULT 1,
    ordem         INTEGER NOT NULL DEFAULT 0,
    criado_em     TEXT NOT NULL,
    atualizado_em TEXT NOT NULL
);

-- O mesmo id em todos os caixas, para a sincronização não duplicar.
INSERT INTO conta_loja (id, nome, tipo, ordem, criado_em, atualizado_em)
VALUES ('caixa-geral', 'Caixa geral', 'caixa', 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

-- Categorias simples (no lugar de um plano de contas).
CREATE TABLE categoria (
    id            TEXT PRIMARY KEY,
    nome          TEXT NOT NULL,
    tipo          TEXT NOT NULL CHECK (tipo IN ('despesa', 'receita')),
    ordem         INTEGER NOT NULL DEFAULT 0,
    ativo         INTEGER NOT NULL DEFAULT 1,
    atualizado_em TEXT NOT NULL
);

INSERT INTO categoria (id, nome, tipo, ordem, atualizado_em) VALUES
    ('cat-fornecedores', 'Fornecedores (peças)', 'despesa', 1, '2026-10-15T00:00:00Z'),
    ('cat-aluguel', 'Aluguel', 'despesa', 2, '2026-10-15T00:00:00Z'),
    ('cat-energia', 'Energia elétrica', 'despesa', 3, '2026-10-15T00:00:00Z'),
    ('cat-agua', 'Água', 'despesa', 4, '2026-10-15T00:00:00Z'),
    ('cat-telefone', 'Telefone e internet', 'despesa', 5, '2026-10-15T00:00:00Z'),
    ('cat-salarios', 'Salários', 'despesa', 6, '2026-10-15T00:00:00Z'),
    ('cat-prolabore', 'Pró-labore', 'despesa', 7, '2026-10-15T00:00:00Z'),
    ('cat-impostos', 'Impostos (DAS)', 'despesa', 8, '2026-10-15T00:00:00Z'),
    ('cat-contador', 'Contador', 'despesa', 9, '2026-10-15T00:00:00Z'),
    ('cat-frete', 'Frete', 'despesa', 10, '2026-10-15T00:00:00Z'),
    ('cat-comissoes', 'Comissões', 'despesa', 11, '2026-10-15T00:00:00Z'),
    ('cat-tarifas', 'Tarifas do banco', 'despesa', 12, '2026-10-15T00:00:00Z'),
    ('cat-manutencao', 'Manutenção da loja', 'despesa', 13, '2026-10-15T00:00:00Z'),
    ('cat-outras-despesas', 'Outras despesas', 'despesa', 14, '2026-10-15T00:00:00Z'),
    ('cat-outras-entradas', 'Outras entradas', 'receita', 15, '2026-10-15T00:00:00Z');

-- Cada parcela que a loja deve (de uma compra ou lançada à mão).
CREATE TABLE conta_pagar (
    id                  TEXT PRIMARY KEY,
    fornecedor_id       TEXT REFERENCES pessoa (id),
    compra_id           TEXT REFERENCES compra (id),
    categoria_id        TEXT REFERENCES categoria (id),
    descricao           TEXT NOT NULL DEFAULT '',
    -- número da nota, do boleto...
    documento           TEXT NOT NULL DEFAULT '',
    parcela             INTEGER NOT NULL DEFAULT 1,
    parcelas            INTEGER NOT NULL DEFAULT 1,
    valor               INTEGER NOT NULL,
    vencimento          TEXT NOT NULL,
    observacoes         TEXT NOT NULL DEFAULT '',
    criado_em           TEXT NOT NULL,
    criado_por          TEXT,
    dispositivo         TEXT NOT NULL,
    cancelado_em        TEXT,
    cancelado_por       TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

CREATE INDEX conta_pagar_vencimento ON conta_pagar (vencimento);
CREATE INDEX conta_pagar_fornecedor ON conta_pagar (fornecedor_id);
CREATE INDEX conta_pagar_compra ON conta_pagar (compra_id);

-- Um pagamento de uma parcela (inteira ou em parte).
-- valor = quanto abateu da parcela; saiu valor + juros - desconto.
-- De onde saiu: uma conta da loja ou a gaveta de um caixa (sessao_id).
CREATE TABLE pagamento (
    id                 TEXT PRIMARY KEY,
    conta_pagar_id     TEXT NOT NULL REFERENCES conta_pagar (id),
    -- AAAA-MM-DD em que o dinheiro sai (cheque: o "bom para")
    data               TEXT NOT NULL,
    valor              INTEGER NOT NULL,
    juros              INTEGER NOT NULL DEFAULT 0,
    desconto           INTEGER NOT NULL DEFAULT 0,
    forma              TEXT NOT NULL DEFAULT 'dinheiro',
    conta_loja_id      TEXT REFERENCES conta_loja (id),
    sessao_id          TEXT,
    caixa_movimento_id TEXT,
    cheque_numero      TEXT NOT NULL DEFAULT '',
    observacoes        TEXT NOT NULL DEFAULT '',
    usuario_id         TEXT,
    criado_em          TEXT NOT NULL,
    dispositivo        TEXT NOT NULL,
    estornado_em       TEXT,
    estornado_por      TEXT,
    motivo_estorno     TEXT NOT NULL DEFAULT ''
);

CREATE INDEX pagamento_conta ON pagamento (conta_pagar_id);
CREATE INDEX pagamento_data ON pagamento (data);
CREATE INDEX pagamento_conta_loja ON pagamento (conta_loja_id);

CREATE VIEW conta_pagar_saldo AS
SELECT c.*,
       COALESCE((SELECT SUM(p.valor) FROM pagamento p
                  WHERE p.conta_pagar_id = c.id AND p.estornado_em IS NULL), 0) AS pago
  FROM conta_pagar c;

-- Movimentos nas contas da loja. valor positivo entra, negativo sai.
-- tipo: entrada, saida (avulsas, com categoria); transferencia (par com o
-- mesmo grupo); sangria/suprimento (da gaveta); deposito_cheque;
-- cheque_devolvido; ajuste (conferência do saldo).
CREATE TABLE movimento_conta (
    id             TEXT PRIMARY KEY,
    conta_id       TEXT NOT NULL REFERENCES conta_loja (id),
    data           TEXT NOT NULL,
    valor          INTEGER NOT NULL,
    tipo           TEXT NOT NULL,
    categoria_id   TEXT REFERENCES categoria (id),
    descricao      TEXT NOT NULL DEFAULT '',
    grupo          TEXT,
    origem_id      TEXT,
    usuario_id     TEXT,
    criado_em      TEXT NOT NULL,
    dispositivo    TEXT NOT NULL,
    estornado_em   TEXT,
    estornado_por  TEXT,
    motivo_estorno TEXT NOT NULL DEFAULT ''
);

CREATE INDEX movimento_conta_conta ON movimento_conta (conta_id, data);
CREATE INDEX movimento_conta_data ON movimento_conta (data);

-- Sangria que vai para uma conta (ou suprimento que vem dela).
ALTER TABLE caixa_movimento ADD COLUMN conta_loja_id TEXT;

-- Cheques recebidos (venda ou crediário).
-- situacao: carteira, depositado, devolvido, repassado, cancelado.
CREATE TABLE cheque (
    id             TEXT PRIMARY KEY,
    cliente_id     TEXT REFERENCES pessoa (id),
    emitente       TEXT NOT NULL DEFAULT '',
    banco          TEXT NOT NULL DEFAULT '',
    numero         TEXT NOT NULL DEFAULT '',
    valor          INTEGER NOT NULL,
    bom_para       TEXT NOT NULL,
    venda_id       TEXT,
    recebimento_id TEXT,
    situacao       TEXT NOT NULL DEFAULT 'carteira',
    situacao_em    TEXT,
    situacao_por   TEXT,
    conta_loja_id  TEXT,
    observacoes    TEXT NOT NULL DEFAULT '',
    criado_em      TEXT NOT NULL,
    criado_por     TEXT,
    dispositivo    TEXT NOT NULL
);

CREATE INDEX cheque_bom_para ON cheque (situacao, bom_para);
CREATE INDEX cheque_venda ON cheque (venda_id);
CREATE INDEX cheque_recebimento ON cheque (recebimento_id);

-- Comissão: % padrão de cada vendedor (a da peça, se houver, vale mais).
ALTER TABLE usuario ADD COLUMN comissao INTEGER;

-- Comissão fechada de um período (vira uma conta a pagar).
CREATE TABLE comissao_fechamento (
    id             TEXT PRIMARY KEY,
    usuario_id     TEXT NOT NULL REFERENCES usuario (id),
    de             TEXT NOT NULL,
    ate            TEXT NOT NULL,
    vendas         INTEGER NOT NULL DEFAULT 0,
    total_vendido  INTEGER NOT NULL DEFAULT 0,
    valor          INTEGER NOT NULL,
    conta_pagar_id TEXT,
    criado_em      TEXT NOT NULL,
    criado_por     TEXT,
    dispositivo    TEXT NOT NULL,
    cancelado_em   TEXT
);

-- Compra: lançar as duplicatas da nota no "A pagar".
ALTER TABLE compra ADD COLUMN gerar_pagar INTEGER NOT NULL DEFAULT 1;
