-- Rustinho · migração 0027 (R0 da recuperação do trabalho; QT-1, preparada em 14/11/2026): a operação já
-- feita, gravada NA MESMA TRANSAÇÃO da própria operação (venda, recebimento, caixa...). Depois de uma queda,
-- de um reinício ou de uma repetição com a mesma chave, o núcleo devolve o resultado da primeira vez em vez
-- de gravar de novo; a mesma chave com outro pedido é recusada.
-- Tabela LOCAL: não viaja entre os caixas. O número final segue a ordem de incorporação (MIG-1).

CREATE TABLE IF NOT EXISTS operacao_feita (
    chave      BLOB PRIMARY KEY NOT NULL,   -- SHA-256 de (comando, chave da operação, usuário), como antes na memória
    comando    TEXT NOT NULL,
    usuario_id TEXT NOT NULL,
    pedido     BLOB NOT NULL,               -- SHA-256 dos dados do comando em forma canônica, sem a chave e sem senhas
    resultado  TEXT NOT NULL,               -- a resposta completa da primeira vez (o que a tela usa)
    feita_em   TEXT NOT NULL,
    feita_ms   INTEGER NOT NULL DEFAULT 0,  -- QT-1b: só a ordem e a hora no aviso (quem decide é a abertura)
    -- QT-1b (depois da revisão independente, 15/11): a abertura do programa em que a operação foi feita; se a tela
    -- seguiu depois dela (entregue: a resposta chegou à tela); e um resumo curto (tipo e valor) para o aviso.
    abertura   INTEGER NOT NULL DEFAULT 0,
    entregue   INTEGER NOT NULL DEFAULT 0,
    resumo     TEXT NOT NULL DEFAULT ''
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS operacao_feita_quando ON operacao_feita(feita_em);
CREATE INDEX IF NOT EXISTS operacao_feita_abertura ON operacao_feita(abertura);

-- QT-1b (aviso de fechamento inesperado): a operação que começou e ainda não terminou. Entra antes do comando, numa
-- transação própria; sai NA MESMA transação que grava a operação (ou quando o comando falha de forma limpa).
CREATE TABLE IF NOT EXISTS operacao_em_curso (
    chave       BLOB PRIMARY KEY NOT NULL,
    comando     TEXT NOT NULL,
    usuario_id  TEXT NOT NULL,
    iniciada_em TEXT NOT NULL,
    iniciada_ms INTEGER NOT NULL DEFAULT 0,
    abertura    INTEGER NOT NULL DEFAULT 0,
    resumo      TEXT NOT NULL DEFAULT ''
) WITHOUT ROWID;

-- QT-1b: cada abertura do programa, numerada (1, 2, 3…). Quem decide o que entra no aviso é o NÚMERO, não o relógio
-- (o relógio pode voltar; duas quedas seguidas não apagam a primeira). `limpa` = encerrada pelo caminho normal;
-- `com_erro` = houve um erro interno (pânico) nela: mesmo fechando direito, a abertura seguinte avisa.
CREATE TABLE IF NOT EXISTS programa_abertura (
    numero     INTEGER PRIMARY KEY,
    aberta_em  TEXT NOT NULL,
    aberta_ms  INTEGER NOT NULL DEFAULT 0,
    limpa      INTEGER NOT NULL DEFAULT 0,
    com_erro   INTEGER NOT NULL DEFAULT 0
);

-- QT-1b: quem já conferiu o aviso de cada abertura que caiu (por usuário: o "Conferi" de um não tira do outro).
CREATE TABLE IF NOT EXISTS fechamento_visto (
    usuario_id TEXT NOT NULL,
    abertura   INTEGER NOT NULL,
    PRIMARY KEY (usuario_id, abertura)
) WITHOUT ROWID;
