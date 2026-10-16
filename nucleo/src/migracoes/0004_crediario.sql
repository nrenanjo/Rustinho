-- Rustinho: Etapa 4 (parte A) — crediário, convênio e contas a receber.

-- Convênio: o cliente paga sempre no mesmo dia do mês (vazio = 30 dias).
ALTER TABLE pessoa ADD COLUMN dia_vencimento INTEGER;

-- Cada parcela que o cliente deve. Vem de uma venda no crediário ou é
-- lançada à mão (dívida antiga, caderninho...). O que já foi pago sai dos
-- recebimentos (não fica gravado aqui, para dois caixas não brigarem).
CREATE TABLE conta_receber (
    id                  TEXT PRIMARY KEY,
    cliente_id          TEXT NOT NULL REFERENCES pessoa (id),
    venda_id            TEXT REFERENCES venda (id),
    parcela             INTEGER NOT NULL DEFAULT 1,
    parcelas            INTEGER NOT NULL DEFAULT 1,
    descricao           TEXT NOT NULL DEFAULT '',
    valor               INTEGER NOT NULL,
    -- AAAA-MM-DD
    vencimento          TEXT NOT NULL,
    criado_em           TEXT NOT NULL,
    criado_por          TEXT,
    dispositivo         TEXT NOT NULL,
    cancelado_em        TEXT,
    cancelado_por       TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

CREATE INDEX conta_receber_cliente ON conta_receber (cliente_id, vencimento);
CREATE INDEX conta_receber_venda ON conta_receber (venda_id);
CREATE INDEX conta_receber_vencimento ON conta_receber (vencimento);

-- O cliente pagou (uma ou mais parcelas, inteiras ou em parte).
-- valor = o que o cliente entregou (já com juros e desconto, sem o troco).
CREATE TABLE recebimento (
    id             TEXT PRIMARY KEY,
    cliente_id     TEXT NOT NULL REFERENCES pessoa (id),
    numero         INTEGER NOT NULL,
    dispositivo    TEXT NOT NULL,
    caixa_nome     TEXT NOT NULL,
    sessao_id      TEXT,
    forma          TEXT NOT NULL,
    valor          INTEGER NOT NULL,
    juros          INTEGER NOT NULL DEFAULT 0,
    desconto       INTEGER NOT NULL DEFAULT 0,
    troco          INTEGER NOT NULL DEFAULT 0,
    observacoes    TEXT NOT NULL DEFAULT '',
    usuario_id     TEXT,
    criado_em      TEXT NOT NULL,
    estornado_em   TEXT,
    estornado_por  TEXT,
    motivo_estorno TEXT NOT NULL DEFAULT ''
);

CREATE INDEX recebimento_cliente ON recebimento (cliente_id, criado_em);
CREATE INDEX recebimento_sessao ON recebimento (sessao_id);

-- Quanto de cada parcela o recebimento pagou.
CREATE TABLE recebimento_parcela (
    id             TEXT PRIMARY KEY,
    recebimento_id TEXT NOT NULL REFERENCES recebimento (id) ON DELETE CASCADE,
    conta_id       TEXT NOT NULL REFERENCES conta_receber (id),
    valor          INTEGER NOT NULL
);

CREATE INDEX recebimento_parcela_recebimento ON recebimento_parcela (recebimento_id);
CREATE INDEX recebimento_parcela_conta ON recebimento_parcela (conta_id);

-- Cada parcela com o quanto já foi pago (sem os recebimentos estornados).
CREATE VIEW conta_receber_saldo AS
SELECT c.*,
       COALESCE((SELECT SUM(rp.valor)
                   FROM recebimento_parcela rp JOIN recebimento r ON r.id = rp.recebimento_id
                  WHERE rp.conta_id = c.id AND r.estornado_em IS NULL), 0) AS pago
  FROM conta_receber c;
