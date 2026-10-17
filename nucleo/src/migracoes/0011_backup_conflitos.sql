-- 0.9.11 (segunda parte).
-- Backup: a parte secreta da chave da loja, trancada com a senha deste
-- gerente (vai junto com o usuário para os outros caixas). Vazio = este
-- usuário não abre os backups.
ALTER TABLE usuario ADD COLUMN backup_acesso TEXT;

-- Conflitos entre caixas: o que perdeu fica guardado para o gerente ver e,
-- se quiser, usar. "tipo": registro (os dois mexeram no mesmo cadastro),
-- orcamento (o mesmo orçamento virou duas vendas) ou gerente (a loja ficou
-- sem gerente e um voltou a ser).
ALTER TABLE sync_conflito ADD COLUMN tipo TEXT NOT NULL DEFAULT 'registro';
ALTER TABLE sync_conflito ADD COLUMN dados_perdeu TEXT;
ALTER TABLE sync_conflito ADD COLUMN dispositivo_ficou TEXT NOT NULL DEFAULT '';
ALTER TABLE sync_conflito ADD COLUMN resolvido_em TEXT;
ALTER TABLE sync_conflito ADD COLUMN resolvido_por TEXT;
ALTER TABLE sync_conflito ADD COLUMN resolucao TEXT;
CREATE INDEX sync_conflito_aberto ON sync_conflito (resolvido_em);
