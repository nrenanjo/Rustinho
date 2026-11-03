-- D-14 (0.9.14): um caixa já fechado não muda mais.
--
-- Cancelar uma venda, estornar um recebimento ou desfazer uma devolução de um
-- caixa que já foi fechado: o fechamento daquele dia continua como estava, e
-- o dinheiro que sai (ou volta) fica na gaveta aberta deste computador ou,
-- sem caixa aberto, no Caixa geral, ligado à operação de origem.

-- A operação foi feita depois de o caixa dela ter sido fechado: no resumo
-- daquele caixa ela continua contando como estava ao fechar.
ALTER TABLE venda ADD COLUMN cancelado_apos_fechar INTEGER NOT NULL DEFAULT 0;
ALTER TABLE recebimento ADD COLUMN estornado_apos_fechar INTEGER NOT NULL DEFAULT 0;
ALTER TABLE devolucao ADD COLUMN desfeita_apos_fechar INTEGER NOT NULL DEFAULT 0;

-- De onde veio o movimento: 'venda_cancelada', 'recebimento_estornado' ou
-- 'devolucao_desfeita' + o id dela (vazio nos movimentos comuns).
ALTER TABLE caixa_movimento ADD COLUMN origem_tipo TEXT NOT NULL DEFAULT '';
ALTER TABLE caixa_movimento ADD COLUMN origem_id TEXT;
ALTER TABLE movimento_conta ADD COLUMN origem_tipo TEXT NOT NULL DEFAULT '';

CREATE INDEX caixa_movimento_origem ON caixa_movimento (origem_id) WHERE origem_id IS NOT NULL;
CREATE INDEX movimento_conta_origem ON movimento_conta (origem_id) WHERE origem_id IS NOT NULL;
