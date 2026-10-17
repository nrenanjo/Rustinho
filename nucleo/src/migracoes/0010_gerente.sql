-- 0.9.11: gerente e caixa.
-- Quem configurou a loja (o primeiro usuário) vira gerente; os outros, caixa.
ALTER TABLE usuario ADD COLUMN gerente INTEGER NOT NULL DEFAULT 0;
UPDATE usuario SET gerente = 1
 WHERE id = (SELECT id FROM usuario ORDER BY criado_em, rowid LIMIT 1);

-- Senhas erradas seguidas, por usuário (só deste aparelho; fica guardado
-- mesmo fechando o programa). "ultima" em segundos desde 1970.
CREATE TABLE entrada_falha (
    usuario_id TEXT PRIMARY KEY,
    quantidade INTEGER NOT NULL,
    ultima     INTEGER NOT NULL
);

-- Troca de dados entre caixas: quando uma alteração feita aqui "ganhou" de
-- outra, feita antes num outro caixa que ainda não tinha chegado (os dois
-- mexeram no mesmo registro sem ver o do outro). Só deste aparelho.
CREATE TABLE sync_conflito (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tabela      TEXT NOT NULL,
    chave       TEXT NOT NULL,
    ficou       TEXT NOT NULL,
    perdeu      TEXT NOT NULL,
    dispositivo TEXT NOT NULL,
    quando      TEXT NOT NULL
);
