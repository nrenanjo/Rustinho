-- Protocolo 4 (0.9.19-dev.2):
--   1. Coluna `dispositivo` em `caixa_aprovado` (decisão I3): vincula o UUID do
--      caixa dentro da aprovação assinada, detectando cópia de banco/pendrive.
--   2. Tabela local `p4_convite` (decisão A6): convites de uso único (128 bits,
--      validade 10 min) para entrada aprovada de caixas novos.
--
-- 100% aditiva sobre a estrutura 28 (e 24–27): nada existente muda nem perde dados.

ALTER TABLE caixa_aprovado ADD COLUMN dispositivo TEXT NOT NULL DEFAULT '';

CREATE TABLE p4_convite (
    id             TEXT PRIMARY KEY,
    segredo_hash   TEXT NOT NULL UNIQUE,
    segredo_ativo  TEXT,
    codigo_pedido  TEXT NOT NULL,
    nome_caixa     TEXT NOT NULL,
    gerente        TEXT NOT NULL,
    criado_em      TEXT NOT NULL,
    expira_em      TEXT NOT NULL,
    usado_em       TEXT,
    chave_aprovada TEXT
);

CREATE INDEX idx_p4_convite_expira ON p4_convite (expira_em);
