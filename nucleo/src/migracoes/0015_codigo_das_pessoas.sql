-- 0.9.11 (quinta parte): código de clientes e fornecedores sem repetir entre caixas.
--
-- Os códigos de números de clientes e fornecedores passam a sair da mesma
-- faixa do caixa que os das peças (veja `faixas`). Nenhum código existente é
-- trocado. Este índice só serve para achar depressa o maior código de números
-- dentro de uma faixa.
CREATE INDEX pessoa_codigo_numero ON pessoa (CAST(codigo AS INTEGER))
    WHERE codigo GLOB '[1-9]*' AND codigo NOT GLOB '*[^0-9]*' AND length(codigo) <= 9;
CREATE INDEX pessoa_sem_codigo ON pessoa (dispositivo) WHERE codigo = '';
