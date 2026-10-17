-- 0.9.11 (segunda parte).
-- Backup: a parte secreta da chave da loja, trancada com a senha deste
-- gerente (vai junto com o usuário para os outros caixas). Vazio = este
-- usuário não abre os backups.
ALTER TABLE usuario ADD COLUMN backup_acesso TEXT;

-- Conflitos entre caixas: o que perdeu fica guardado para o gerente ver e,
-- se quiser, usar. "tipo": registro (os dois mexeram no mesmo cadastro),
-- orcamento (o mesmo orçamento virou duas vendas) ou gerente (a loja ficou
-- sem gerente e um voltou a ser).
-- Um rascunho da 0.9.11 (que não chegou a sair) criou a estrutura 10 sem
-- esta tabela: cria aqui se faltar, igual à da 0010.
CREATE TABLE IF NOT EXISTS sync_conflito (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tabela      TEXT NOT NULL,
    chave       TEXT NOT NULL,
    ficou       TEXT NOT NULL,
    perdeu      TEXT NOT NULL,
    dispositivo TEXT NOT NULL,
    quando      TEXT NOT NULL
);
ALTER TABLE sync_conflito ADD COLUMN tipo TEXT NOT NULL DEFAULT 'registro';
ALTER TABLE sync_conflito ADD COLUMN dados_perdeu TEXT;
ALTER TABLE sync_conflito ADD COLUMN dispositivo_ficou TEXT NOT NULL DEFAULT '';
ALTER TABLE sync_conflito ADD COLUMN resolvido_em TEXT;
ALTER TABLE sync_conflito ADD COLUMN resolvido_por TEXT;
ALTER TABLE sync_conflito ADD COLUMN resolucao TEXT;
CREATE INDEX sync_conflito_aberto ON sync_conflito (resolvido_em);

-- Cada alteração anota sobre qual versão do registro ela foi feita ("base":
-- o id da alteração mais nova que o caixa conhecia). Assim dá para saber se
-- dois caixas mexeram no mesmo registro sem ver o do outro, sem confiar só
-- no relógio.
ALTER TABLE alteracao ADD COLUMN base TEXT;
