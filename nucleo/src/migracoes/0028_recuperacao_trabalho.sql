-- Rustinho · migração 0028 (Pacote 3 da 0.9.19-dev.1: R1/R2 da recuperação do trabalho; REC-1 a REC-6):
-- 1) `trabalho`: retrato de trabalho em andamento (rascunhos de telas e percursos de navegação) com duas versões
--    (`estado_json` e `estado_anterior_json`) e integridade verificada por SHA-256 (`resumo_sha256` e
--    `resumo_anterior_sha256`), isolamento por usuário (`DR-3` + exceção auditável `7b` para usuário desativado),
--    marca de 30 dias (`DR-5`, sem apagar sozinho), sinalização de conflito de origem após restauração de backup
--    em outro caixa (`DR-4` / `DR-6`), revalidação contra alterações concorrentes (`base_atualizado_em`) e vínculo
--    com `operacao_feita` (`DR-7`).
-- 2) `sessao_local`: registro local de sessões e relógio de referência para detecção de interrupção inesperada
--    e proteção contra relógio recuado (`T-03`).
-- Ambas são tabelas LOCAIS (`Classe::Local` em `dados::esquema`): não viajam pela sincronização LAN e entram
-- integralmente no backup consistente do banco (`DR-4`).

CREATE TABLE IF NOT EXISTS trabalho (
    id                      TEXT PRIMARY KEY NOT NULL,
    usuario_id              TEXT NOT NULL,
    dispositivo_origem      TEXT NOT NULL,
    tipo                    TEXT NOT NULL,
    alvo_id                 TEXT NOT NULL DEFAULT '',
    titulo                  TEXT NOT NULL DEFAULT '',
    formato_versao          INTEGER NOT NULL DEFAULT 1,
    estado_json             TEXT NOT NULL,
    resumo_sha256           TEXT NOT NULL,
    estado_anterior_json    TEXT,
    resumo_anterior_sha256  TEXT,
    percurso_json           TEXT NOT NULL DEFAULT '[]',
    base_atualizado_em      TEXT NOT NULL DEFAULT '',
    chave_operacao          TEXT NOT NULL DEFAULT '',
    situacao                TEXT NOT NULL DEFAULT 'aberto',
    conflito_origem         INTEGER NOT NULL DEFAULT 0,
    criado_em               TEXT NOT NULL,
    atualizado_em           TEXT NOT NULL,
    atualizado_ms           INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS trabalho_usuario_situacao ON trabalho(usuario_id, situacao, atualizado_ms);
CREATE UNIQUE INDEX IF NOT EXISTS trabalho_unico_chave ON trabalho(usuario_id, dispositivo_origem, tipo, alvo_id);

CREATE TABLE IF NOT EXISTS sessao_local (
    id                INTEGER PRIMARY KEY,
    dispositivo_id    TEXT NOT NULL,
    usuario_id        TEXT NOT NULL DEFAULT '',
    aberta_em         TEXT NOT NULL,
    aberta_ms         INTEGER NOT NULL DEFAULT 0,
    encerrada_em      TEXT,
    limpa             INTEGER NOT NULL DEFAULT 0,
    relogio_recuado   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS sessao_local_disp ON sessao_local(dispositivo_id, id);
