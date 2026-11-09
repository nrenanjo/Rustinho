-- Referências importadas de NF-e permanecem separadas por função.
ALTER TABLE compra_item ADD COLUMN gtin_tributavel TEXT NOT NULL DEFAULT '';
ALTER TABLE compra_item ADD COLUMN codigo_barra TEXT NOT NULL DEFAULT '';
ALTER TABLE compra_item ADD COLUMN codigo_barra_tributavel TEXT NOT NULL DEFAULT '';

-- Um produto pode ter vários GTINs (unidade comercial e tributável).
-- O índice não é único entre produtos: duplicidades precisam ser detectadas
-- como ambiguidade, nunca resolvidas por substituição silenciosa.
CREATE TABLE produto_gtin (
    produto_id TEXT NOT NULL REFERENCES produto (id) ON DELETE CASCADE,
    gtin       TEXT NOT NULL CHECK (
        length(gtin) IN (8, 12, 13, 14)
        AND gtin NOT GLOB '*[^0-9]*'
    ),
    PRIMARY KEY (produto_id, gtin)
);

CREATE INDEX produto_gtin_codigo ON produto_gtin (gtin);

-- Mapeamentos alternativos preservam o vínculo anterior quando uma escolha
-- manual confirma que o mesmo código do fornecedor também identifica outra
-- peça. Em imports futuros essa situação será ambígua e pedirá nova escolha.
CREATE TABLE produto_fornecedor_alternativo (
    fornecedor_id   TEXT NOT NULL REFERENCES pessoa (id) ON DELETE CASCADE,
    codigo_compacto TEXT NOT NULL,
    codigo          TEXT NOT NULL,
    produto_id      TEXT NOT NULL REFERENCES produto (id) ON DELETE CASCADE,
    fator           INTEGER NOT NULL DEFAULT 1000,
    atualizado_em   TEXT NOT NULL,
    PRIMARY KEY (fornecedor_id, codigo_compacto, produto_id)
);

CREATE INDEX produto_fornecedor_alternativo_produto
    ON produto_fornecedor_alternativo (produto_id);

-- A escolha do código principal é explícita e replicada na linha do produto.
ALTER TABLE produto ADD COLUMN fornecedor_principal_id TEXT NOT NULL DEFAULT '';
ALTER TABLE produto ADD COLUMN fornecedor_principal_codigo TEXT NOT NULL DEFAULT '';
