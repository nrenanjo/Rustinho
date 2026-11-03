-- 0.9.11 (sexta parte, PEND-07): código com letras de cliente/fornecedor só vale
-- depois de confirmado, como o das peças (veja 0014 e `faixas`).
--
-- 1 = código confirmado (ou que não precisa de confirmação: os de números da
-- faixa do caixa, os de antes desta versão, os vazios). 0 = esperando a
-- confirmação do caixa que distribui as faixas. Os códigos que já existem
-- ficam como estão (valem como confirmados) e nenhum é renumerado.
ALTER TABLE pessoa ADD COLUMN codigo_validado INTEGER NOT NULL DEFAULT 1;

CREATE INDEX pessoa_codigo_a_confirmar ON pessoa (criado_em, id) WHERE codigo_validado = 0;
