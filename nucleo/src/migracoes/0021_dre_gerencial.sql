-- DRE gerencial e indicadores (docs/DRE-gerencial.md).
--
-- dre_grupo: em que linha da DRE a categoria entra. Vazio (NULL) = sem
-- classificação: o valor aparece como pendência e conta como despesa.
--   tributos_venda            Tributos sobre as vendas (dedução da receita)
--   variavel                  Despesas variáveis de venda
--   fixa                      Despesas operacionais fixas
--   outra_despesa_operacional / outra_receita_operacional
--   despesa_financeira / receita_financeira
--   despesa_nao_operacional / receita_nao_operacional
--   estoque                   Compra de mercadoria (entra pelo CMV, não aqui)
--   fora                      Fora do resultado (empréstimo, aporte, retirada...)
-- As de fábrica recebem uma SUGESTÃO, que o contador precisa confirmar.
ALTER TABLE categoria ADD COLUMN dre_grupo TEXT;

UPDATE categoria SET dre_grupo = 'estoque' WHERE id = 'cat-fornecedores';
UPDATE categoria SET dre_grupo = 'fixa'
 WHERE id IN ('cat-aluguel', 'cat-energia', 'cat-agua', 'cat-telefone', 'cat-salarios', 'cat-prolabore',
              'cat-contador', 'cat-manutencao');
UPDATE categoria SET dre_grupo = 'tributos_venda' WHERE id = 'cat-impostos';
UPDATE categoria SET dre_grupo = 'variavel' WHERE id IN ('cat-frete', 'cat-comissoes');
UPDATE categoria SET dre_grupo = 'despesa_financeira' WHERE id = 'cat-tarifas';
UPDATE categoria SET dre_grupo = 'outra_despesa_operacional' WHERE id = 'cat-outras-despesas';
UPDATE categoria SET dre_grupo = 'outra_receita_operacional' WHERE id = 'cat-outras-entradas';

-- Mês a que a conta se refere (AAAA-MM). Vazio = o mês do vencimento, e a
-- DRE mostra como pendência.
ALTER TABLE conta_pagar ADD COLUMN competencia TEXT;

-- Metas. O id é montado com o período, o indicador e o vendedor, para ser o
-- mesmo em todos os caixas (a sincronização não duplica).
--   periodo: 'mes' ou 'semana'
--   indicador: 'vendas' (centavos), 'lucro_bruto' (centavos),
--              'margem' (centésimos de ponto: 2550 = 25,50%), 'quantidade' (vendas)
CREATE TABLE meta (
    id            TEXT PRIMARY KEY,
    periodo       TEXT NOT NULL CHECK (periodo IN ('mes', 'semana')),
    indicador     TEXT NOT NULL CHECK (indicador IN ('vendas', 'lucro_bruto', 'margem', 'quantidade')),
    vendedor_id   TEXT,
    valor         INTEGER NOT NULL,
    ativo         INTEGER NOT NULL DEFAULT 1,
    atualizado_em TEXT NOT NULL,
    atualizado_por TEXT
);
