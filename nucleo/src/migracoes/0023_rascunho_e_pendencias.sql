-- rustinho: sem_chaves_estrangeiras
--
-- 0.9.15: rascunho de ordem de serviço, operação comercial no log de
-- alterações, pendências da troca de dados e versão dos outros caixas.
--
-- A primeira linha avisa quem roda as migrações (banco::migrar): esta
-- migração recria a tabela ordem_servico. Com as chaves estrangeiras ligadas,
-- o DROP TABLE faria um DELETE implícito e o ON DELETE CASCADE apagaria
-- todos os itens das OS. Por isso ela roda com `foreign_keys = OFF` (o que
-- só tem efeito fora da transação) e `legacy_alter_table = ON`; dentro da
-- transação, antes do COMMIT, quem roda confere que as contagens e as
-- referências continuam iguais.

-- 1) ordem_servico com a situação 'rascunho' (os 12 passos do SQLite para
--    mudar um CHECK: tabela nova, copiar, apagar a velha, renomear).
CREATE TABLE ordem_servico_nova (
    id            TEXT PRIMARY KEY,
    numero        INTEGER NOT NULL,
    dispositivo   TEXT NOT NULL,
    caixa_nome    TEXT NOT NULL,
    -- rascunho (guardado ao sair, sem número) → orcamento → aprovada (em
    -- serviço) → pronta → entregue; ou cancelada
    situacao      TEXT NOT NULL DEFAULT 'orcamento'
                  CHECK (situacao IN ('rascunho', 'orcamento', 'aprovada', 'pronta', 'entregue', 'cancelada')),
    cliente_id    TEXT,
    cliente_nome  TEXT NOT NULL DEFAULT '',
    telefone      TEXT NOT NULL DEFAULT '',
    placa         TEXT NOT NULL DEFAULT '',
    veiculo       TEXT NOT NULL DEFAULT '',
    km            TEXT NOT NULL DEFAULT '',
    defeito       TEXT NOT NULL DEFAULT '',
    diagnostico   TEXT NOT NULL DEFAULT '',
    observacoes   TEXT NOT NULL DEFAULT '',
    mecanico_id   TEXT,
    previsao      TEXT,
    desconto      INTEGER NOT NULL DEFAULT 0,
    venda_id      TEXT,
    criado_em     TEXT NOT NULL,
    criado_por    TEXT,
    atualizado_em TEXT NOT NULL,
    aprovada_em   TEXT,
    pronta_em     TEXT,
    entregue_em   TEXT,
    cancelada_em  TEXT,
    motivo_cancelamento TEXT NOT NULL DEFAULT ''
);

INSERT INTO ordem_servico_nova
      (id, numero, dispositivo, caixa_nome, situacao, cliente_id, cliente_nome, telefone, placa, veiculo, km,
       defeito, diagnostico, observacoes, mecanico_id, previsao, desconto, venda_id, criado_em, criado_por,
       atualizado_em, aprovada_em, pronta_em, entregue_em, cancelada_em, motivo_cancelamento)
SELECT id, numero, dispositivo, caixa_nome, situacao, cliente_id, cliente_nome, telefone, placa, veiculo, km,
       defeito, diagnostico, observacoes, mecanico_id, previsao, desconto, venda_id, criado_em, criado_por,
       atualizado_em, aprovada_em, pronta_em, entregue_em, cancelada_em, motivo_cancelamento
  FROM ordem_servico;

-- Os gatilhos da sincronização em ordem_servico somem junto com a tabela;
-- são criados de novo ao abrir o banco (sync::criar_gatilhos).
DROP TABLE ordem_servico;
ALTER TABLE ordem_servico_nova RENAME TO ordem_servico;

CREATE INDEX ordem_servico_situacao ON ordem_servico (situacao, criado_em);
CREATE INDEX ordem_servico_placa ON ordem_servico (placa);
CREATE INDEX ordem_servico_venda ON ordem_servico (venda_id);

-- 2) A operação comercial de cada alteração: todas as alterações gravadas
--    na mesma transação (uma venda com itens, pagamentos e movimentos) têm o
--    mesmo id. Alterações de antes desta versão ficam sem (NULL).
ALTER TABLE alteracao ADD COLUMN operacao_id TEXT;
CREATE INDEX alteracao_operacao ON alteracao (operacao_id) WHERE operacao_id IS NOT NULL;
-- Os caixas que aparecem no log (para saber todos os caixas da loja).
CREATE INDEX alteracao_dispositivo ON alteracao (dispositivo);

-- 3) Pendências da troca de dados (só deste computador; não vão para os
--    outros caixas). Uma alteração que não pôde ser aplicada fica aqui
--    inteira, com todos os dados, até poder ser aplicada; nunca é jogada fora.
--    lado 'recebida': veio do caixa `origem` e não entrou nas tabelas.
--    lado 'nao_enviada': deste caixa (ou repassada por ele) que o caixa `par`,
--    de versão antiga, não consegue gravar; a troca com ele para nela.
CREATE TABLE sync_pendencia (
    lado          TEXT NOT NULL CHECK (lado IN ('recebida', 'nao_enviada')),
    par           TEXT NOT NULL DEFAULT '',
    alteracao_id  TEXT NOT NULL,
    origem        TEXT NOT NULL DEFAULT '',
    seq           INTEGER NOT NULL DEFAULT 0,
    tabela        TEXT NOT NULL DEFAULT '',
    chave         TEXT NOT NULL DEFAULT '',
    operacao      TEXT NOT NULL DEFAULT '',
    dados         TEXT,
    dispositivo   TEXT NOT NULL DEFAULT '',
    quando        TEXT NOT NULL DEFAULT '',
    base          TEXT,
    ordem         INTEGER NOT NULL DEFAULT 0,
    historia      TEXT,
    compactada    INTEGER NOT NULL DEFAULT 0,
    operacao_id   TEXT,
    motivo        TEXT NOT NULL
                  CHECK (motivo IN ('incompativel_esquema', 'invalida', 'erro_aplicacao', 'dependencia_suspensa')),
    detalhe       TEXT NOT NULL DEFAULT '',
    estado        TEXT NOT NULL DEFAULT 'pendente' CHECK (estado IN ('pendente', 'resolvida', 'arquivada_manual')),
    tentativas    INTEGER NOT NULL DEFAULT 0,
    ultimo_erro   TEXT NOT NULL DEFAULT '',
    criado_em     TEXT NOT NULL,
    atualizado_em TEXT NOT NULL,
    resolvido_em  TEXT,
    arquivada_por TEXT,
    arquivada_motivo TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (lado, par, alteracao_id)
);

CREATE INDEX sync_pendencia_estado ON sync_pendencia (estado, lado, seq);

-- 4) Versão dos outros caixas: a estrutura do banco que cada um informou, os
--    recursos da troca de dados que ele entende e quando confirmou pela
--    última vez (o prazo de 7 dias para os rascunhos de OS conta daqui).
ALTER TABLE sync_par ADD COLUMN esquema INTEGER;
ALTER TABLE sync_par ADD COLUMN recursos TEXT NOT NULL DEFAULT '';
ALTER TABLE sync_par ADD COLUMN compativel_em TEXT;

-- 5) Tela inicial sem "Mais usados" (0.9.15): fixar muda a ordem das
--    pílulas; a 1ª fixada fica na 1ª posição, a 2ª na 2ª... A ordem vem de
--    quando cada uma foi fixada (as fixadas antes desta versão, sem data,
--    ficam na ordem do menu, antes das novas).
ALTER TABLE menu_uso ADD COLUMN fixado_em TEXT;
