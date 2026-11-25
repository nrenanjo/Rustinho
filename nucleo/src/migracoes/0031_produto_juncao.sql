-- Etapa 4 da 0.9.19 Beta, R13 (DP-42, decisão do proprietário: aceitar a peça cadastrada em dobro e mostrar em
-- Duplicatas, para o gerente desativar uma e passar as ligações para a outra):
--   `produto_juncao`: a peça D foi juntada na peça K (`para_id`). Tabela própria que viaja (chave D): uma edição da
--   ficha de D em outro caixa não desfaz a junção (B-1 da revisão de desenho). A raiz de uma peça segue estas
--   linhas (`produtos::raiz`).
--
-- 100% aditiva sobre a estrutura 30: nada existente muda nem perde dados.

CREATE TABLE produto_juncao (
    produto_id TEXT PRIMARY KEY REFERENCES produto (id),
    para_id    TEXT NOT NULL REFERENCES produto (id),
    em         TEXT NOT NULL,
    por        TEXT,
    motivo     TEXT NOT NULL DEFAULT ''
);

CREATE INDEX produto_juncao_para ON produto_juncao (para_id);
