-- 0.9.19, etapa 3 (pacote E; FIS-P1 e FIS-P2): a nota fiscal emitida pelo
-- Rustinho e o histórico dela.
--
-- Ficam só neste computador (não viajam): quem emite é a estação fiscal, o
-- computador com o certificado A3 (DF-4) e o fiscal ligado (FIS-LIGA). A
-- numeração é por ambiente, modelo e série, e um número nunca se repete
-- (UNIQUE). O estado segue a máquina de estados de `integracoes::nota_fiscal`:
-- cada passo é gravado antes de chamar o emissor, e uma transmissão sem
-- resposta conclusiva fica como "desconhecida" (consultar antes de reenviar).
--
-- 100% aditiva sobre a estrutura 32: só cria as duas tabelas, vazias.

CREATE TABLE nota_fiscal (
    id                 TEXT PRIMARY KEY,
    -- 1 = produção, 2 = homologação (tpAmb do MOC 7.0)
    ambiente           INTEGER NOT NULL CHECK (ambiente IN (1, 2)),
    -- 55 = NF-e, 65 = NFC-e
    modelo             INTEGER NOT NULL CHECK (modelo IN (55, 65)),
    serie              INTEGER NOT NULL CHECK (serie BETWEEN 0 AND 999),
    -- 0 enquanto rascunho (sem número reservado)
    numero             INTEGER NOT NULL DEFAULT 0,
    chave              TEXT NOT NULL DEFAULT '',
    estado             TEXT NOT NULL,
    venda_id           TEXT,
    destinatario_doc   TEXT NOT NULL DEFAULT '',
    destinatario_nome  TEXT NOT NULL DEFAULT '',
    -- centavos
    valor              INTEGER NOT NULL DEFAULT 0,
    cstat              TEXT NOT NULL DEFAULT '',
    motivo             TEXT NOT NULL DEFAULT '',
    protocolo          TEXT NOT NULL DEFAULT '',
    xml_nome           TEXT NOT NULL DEFAULT '',
    criado_em          TEXT NOT NULL,
    atualizado_em      TEXT NOT NULL,
    dispositivo        TEXT NOT NULL
);

-- Um número por ambiente, modelo e série (o rascunho, com 0, fica de fora).
CREATE UNIQUE INDEX nota_fiscal_numero ON nota_fiscal (ambiente, modelo, serie, numero) WHERE numero > 0;
CREATE INDEX nota_fiscal_estado ON nota_fiscal (ambiente, estado, criado_em);

CREATE TABLE nota_fiscal_evento (
    id          TEXT PRIMARY KEY,
    nota_id     TEXT NOT NULL REFERENCES nota_fiscal (id),
    quando      TEXT NOT NULL,
    de          TEXT NOT NULL,
    para        TEXT NOT NULL,
    detalhe     TEXT NOT NULL DEFAULT '',
    usuario_id  TEXT
);

CREATE INDEX nota_fiscal_evento_nota ON nota_fiscal_evento (nota_id, quando);
