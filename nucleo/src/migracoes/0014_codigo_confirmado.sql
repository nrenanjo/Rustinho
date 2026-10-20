-- 0.9.11 (quinta parte): código com letras escrito à mão só vale depois de
-- confirmado.
--
-- Dois caixas sem rede podem cadastrar o mesmo código com letras (ex.:
-- "A-123") em peças diferentes. Quem decide qual fica com o código é o caixa
-- que distribui as faixas (`faixas`), numa gravação só. Até ele confirmar, a
-- peça é vendida e achada pelo nome, mas a etiqueta não sai e o leitor não
-- acha a peça por esse código.
--
-- 1 = código confirmado (ou que não precisa de confirmação: os de números
-- da faixa do caixa, os de antes desta versão, os vazios). 0 = esperando.
-- Os códigos que já existem ficam como estão (valem como confirmados).
ALTER TABLE produto ADD COLUMN codigo_validado INTEGER NOT NULL DEFAULT 1;

CREATE INDEX produto_codigo_a_confirmar ON produto (criado_em, id) WHERE codigo_validado = 0;
