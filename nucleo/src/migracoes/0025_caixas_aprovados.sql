-- Protocolo 4, etapa 2 (D1, E5): registro assinado dos caixas aprovados e
-- dos retirados (veja `identidade_caixa::aprovados`).
--
-- Cada linha leva a assinatura Ed25519 de quem a fez sobre todos os campos.
-- Nada vale só por estar aqui: a conferência refaz a cadeia de assinaturas
-- até a raiz da loja a cada consulta (linha mexida = linha que não vale).
-- Chaves e assinaturas em hexadecimal minúsculo (64 e 128 caracteres).
--
-- NÃO entram em sync::TABELAS (a troca de dados do protocolo 3): um caixa
-- da 0.9.17 não conhece estas tabelas, e a troca de hoje só exige a senha da
-- rede. A replicação delas é das etapas 4 (canal autenticado) e 7
-- (convivência com o protocolo 3).
--
-- Só acrescenta duas tabelas vazias: nada do que já existe muda.

CREATE TABLE caixa_aprovado (
    chave       TEXT NOT NULL,  -- Ed25519 pública do caixa aprovado
    aprovador   TEXT NOT NULL,  -- Ed25519 pública de quem aprovou (= chave, só na raiz)
    chave_troca TEXT NOT NULL,  -- X25519 pública do caixa aprovado (canal, etapa 4)
    loja_id     TEXT NOT NULL,
    nome        TEXT NOT NULL,
    gerente     TEXT NOT NULL,  -- usuário gerente que autorizou
    aprovado_em TEXT NOT NULL,  -- declarado (e assinado) por quem aprovou
    assinatura  TEXT NOT NULL,  -- Ed25519 do aprovador sobre todos os campos
    PRIMARY KEY (chave, aprovador)
);

CREATE INDEX caixa_aprovado_loja ON caixa_aprovado (loja_id);

CREATE TABLE caixa_retirado (
    chave       TEXT NOT NULL,  -- Ed25519 pública do caixa retirado
    retirador   TEXT NOT NULL,  -- Ed25519 pública de quem retirou
    loja_id     TEXT NOT NULL,
    motivo      TEXT NOT NULL,
    gerente     TEXT NOT NULL,  -- usuário gerente que autorizou
    retirado_em TEXT NOT NULL,  -- declarado (e assinado) por quem retirou
    assinatura  TEXT NOT NULL,  -- Ed25519 do retirador sobre todos os campos
    PRIMARY KEY (chave, retirador)
);

CREATE INDEX caixa_retirado_loja ON caixa_retirado (loja_id);
