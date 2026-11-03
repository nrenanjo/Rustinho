-- F-17: situação da senha de cada usuário diante da política (senhas.rs).
-- Só deste computador (não entra em TABELAS, então não é sincronizada): a
-- avaliação só é possível no instante em que a pessoa digita a senha, no
-- login. Não guarda a senha, só o resultado. A linha vale enquanto o hash
-- da senha e o papel (gerente ou não) forem os mesmos de quando foi avaliada;
-- se outro caixa trocar a senha, a linha deixa de valer e a situação volta a
-- ser "não avaliada" até a próxima entrada.
CREATE TABLE senha_situacao (
    usuario_id TEXT PRIMARY KEY,
    hash       TEXT NOT NULL,
    gerente    INTEGER NOT NULL,
    problema   TEXT,            -- NULL = dentro da política
    avaliada_em TEXT NOT NULL
);
