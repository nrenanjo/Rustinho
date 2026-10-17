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
