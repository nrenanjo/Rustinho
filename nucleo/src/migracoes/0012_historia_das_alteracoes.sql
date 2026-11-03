-- 0.9.11 (terceira parte): troca de dados que não depende do relógio (R-08).
--
-- "ordem": contador de cada registro. Uma alteração feita em cima de outra
-- sempre tem ordem maior que ela (base + 1). Vale a de maior ordem; empate
-- (duas feitas ao mesmo tempo em caixas diferentes): a de maior id. Assim
-- todos os caixas escolhem a mesma, em qualquer ordem de chegada, e uma
-- alteração feita depois nunca perde só porque o relógio do caixa estava
-- atrasado.
--
-- "historia": as versões anteriores do registro que o caixa conhecia (a base
-- primeiro, até 8), em JSON. Um "+" no fim quer dizer que a história não
-- chega até o começo do registro. Com ela se sabe se duas alterações são uma
-- depois da outra ou se foram feitas sem uma ver a outra (conflito). Quando
-- não dá para saber, conta como conflito: o gerente vê e as duas ficam
-- guardadas.
ALTER TABLE alteracao ADD COLUMN ordem INTEGER NOT NULL DEFAULT 0;
ALTER TABLE alteracao ADD COLUMN historia TEXT;
CREATE INDEX alteracao_ordem ON alteracao (tabela, chave, ordem, id);

-- Compactar o log não apaga mais as versões velhas: só tira os dados delas
-- ("compactada" = 1). Ficam o id, a base e a ordem, que são pequenos. Assim
-- qualquer caixa consegue seguir a cadeia de versões até o começo e saber
-- com certeza se uma alteração veio depois da outra, e uma alteração
-- repetida (reenvio, backup restaurado em outro caixa) é reconhecida pelo
-- id e não vira aviso de novo.
ALTER TABLE alteracao ADD COLUMN compactada INTEGER NOT NULL DEFAULT 0;
CREATE INDEX alteracao_base ON alteracao (base);

-- Conflitos: a história da versão que perdeu (para saber se outra alteração
-- já partiu dela) e se o conflito é certo ("concorrente": feitas sem uma
-- ver a outra) ou se não deu para saber a ordem ("incerta").
ALTER TABLE sync_conflito ADD COLUMN base_perdeu TEXT;
ALTER TABLE sync_conflito ADD COLUMN historia_perdeu TEXT;
ALTER TABLE sync_conflito ADD COLUMN certeza TEXT NOT NULL DEFAULT 'concorrente';
CREATE INDEX sync_conflito_perdeu ON sync_conflito (perdeu);

-- A decisão do gerente sobre um conflito. Vai para os outros caixas (é
-- uma tabela da loja), que fecham o mesmo conflito sem perguntar de novo.
CREATE TABLE sync_resolucao (
    id         TEXT PRIMARY KEY,
    tabela     TEXT NOT NULL,
    chave      TEXT NOT NULL,
    -- a versão que valia quando o gerente decidiu e a que tinha perdido
    ficou      TEXT NOT NULL,
    perdeu     TEXT NOT NULL,
    -- "usar" (voltou a que tinha perdido), "manter" (ficou como estava) ou
    -- "aplicada": dois gerentes decidiram diferente sobre o mesmo conflito;
    -- este registro diz qual regra valeu e quais decisões houve.
    escolha    TEXT NOT NULL CHECK (escolha IN ('usar', 'manter', 'aplicada')),
    usuario_id TEXT,
    quando     TEXT NOT NULL,
    -- só em "aplicada": os ids das decisões que se contradisseram (JSON)
    decisoes   TEXT
);
CREATE INDEX sync_resolucao_perdeu ON sync_resolucao (perdeu);

-- Referências que não batem depois da troca de dados (R-05), uma por linha,
-- só deste caixa. Some sozinha quando o registro que faltava chega; nada é
-- apagado para esconder. "estado": esperando (o registro apontado ainda não
-- chegou), apagado (foi apagado em algum caixa) ou inconsistente (não
-- chegou em 24 horas, ou o log diz que existe e a tabela não tem).
CREATE TABLE sync_referencia (
    tabela  TEXT NOT NULL,
    -- chave do registro que aponta, em JSON
    chave   TEXT NOT NULL,
    -- tabela e chave do registro que falta
    pai     TEXT NOT NULL,
    falta   TEXT NOT NULL,
    estado  TEXT NOT NULL,
    desde   TEXT NOT NULL,
    PRIMARY KEY (tabela, chave, pai)
);

-- Contador de cada outro caixa com a "marca" (o id da última alteração
-- recebida dele) e a "geração" do banco dele. Se o outro caixa voltou de um
-- backup, foi trocado por um computador novo ou teve o banco copiado à mão,
-- a marca não bate (ou a geração mudou): este caixa recomeça do zero com ele.
-- As repetidas são reconhecidas pelo id e não são aplicadas de novo.
ALTER TABLE sync_par ADD COLUMN marca TEXT;
ALTER TABLE sync_par ADD COLUMN geracao TEXT;
