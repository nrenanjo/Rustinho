-- 0.9.11 (quarta parte): código interno das peças sem repetir entre caixas.
--
-- Cada caixa só dá códigos de números dentro de uma faixa que é só dele
-- ("faixa_codigo"). Quem dá as faixas é um caixa só, o "distribuidor"
-- (configuração da loja "codigos_distribuidor"), numa gravação só, que fica
-- guardada antes de o caixa usar qualquer código dela. A faixa é do caixa e
-- da "geração" de códigos dele: depois de restaurar um backup (ou de o banco
-- ir para outro computador), o caixa ganha geração nova e pede outra faixa;
-- a antiga nunca mais é usada por ninguém. Nada aqui é apagado nem liberado
-- de novo, nem quando um caixa fica muito tempo sem rede.
--
-- Se, mesmo assim, duas faixas de donos diferentes se cruzarem (duas lojas
-- juntadas, dois distribuidores ao mesmo tempo), as duas ficam guardadas,
-- ninguém mais dá código no pedaço cruzado e o gerente vê na pílula Gerente.
CREATE TABLE faixa_codigo (
    inicio       INTEGER NOT NULL,
    fim          INTEGER NOT NULL,
    -- dono: o caixa e a geração de códigos dele
    dispositivo  TEXT NOT NULL,
    geracao      TEXT NOT NULL,
    -- nome do caixa dono quando pediu (para a tela do gerente)
    caixa        TEXT NOT NULL DEFAULT '',
    -- o caixa que deu a faixa
    distribuidor TEXT NOT NULL,
    criado_em    TEXT NOT NULL,
    PRIMARY KEY (inicio, dispositivo, geracao),
    CHECK (inicio >= 1 AND fim >= inicio)
);

-- Códigos que uma peça já teve (quando o código é trocado). Etiquetas velhas
-- continuam na prateleira: o leitor que achar um código antigo de uma peça
-- e o código atual de outra pergunta qual é, e o código antigo nunca é dado
-- a outra peça.
CREATE TABLE produto_codigo_antigo (
    codigo          TEXT NOT NULL,
    produto_id      TEXT NOT NULL REFERENCES produto (id),
    codigo_compacto TEXT NOT NULL,
    trocado_em      TEXT NOT NULL,
    PRIMARY KEY (codigo, produto_id)
);
CREATE INDEX produto_codigo_antigo_compacto ON produto_codigo_antigo (codigo_compacto);
CREATE INDEX produto_codigo_antigo_produto ON produto_codigo_antigo (produto_id);

-- Para achar depressa o maior código de números dentro de uma faixa.
CREATE INDEX produto_codigo_numero ON produto (CAST(codigo AS INTEGER))
    WHERE codigo GLOB '[1-9]*' AND codigo NOT GLOB '*[^0-9]*' AND length(codigo) <= 9;
CREATE INDEX produto_codigo_antigo_numero ON produto_codigo_antigo (CAST(codigo AS INTEGER))
    WHERE codigo GLOB '[1-9]*' AND codigo NOT GLOB '*[^0-9]*' AND length(codigo) <= 9;
CREATE INDEX produto_sem_codigo ON produto (dispositivo) WHERE codigo = '';

-- Banco que já tinha usuários (loja que já usava o Rustinho antes das
-- faixas): a primeira faixa sai depois de conferir os códigos que existem
-- (veja `codigos::garantir_distribuidor`). Só deste computador.
INSERT INTO config (chave, valor) SELECT 'codigos_legado', '1' WHERE EXISTS (SELECT 1 FROM usuario);
