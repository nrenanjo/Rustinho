-- PEND-09: verificação da assinatura do XML da NF-e e prova da autorização
-- da SEFAZ (docs/PEND-09-criterios.md §5).
--
-- xml_verificacao: 'verificada' (assinatura do emitente conferida na
--   importação) ou 'nao_verificada' (importada antes desta versão; D5: não
--   bloqueia nada, só é mostrado).
-- xml_prova: resumo do que foi provado (JSON: titular, tipo, cadeia, raiz).
-- autorizacao: 'anterior' (compra de antes desta versão, D5), 'aguardando'
--   (assinatura conferida, autorização da SEFAZ ainda não comprovada; NÃO
--   pode ser lançada), 'confirmada_gerente' (D7: conferência humana do
--   gerente no portal, provisória), 'negada' (rejeitada, cancelada ou
--   denegada: nunca pode ser lançada) e, no futuro, 'comprovada_sefaz'
--   (consulta automática, PEND-15).
-- autorizacao_dados: o registro do gerente (JSON: quem, quando, chave,
--   protocolo, situação conferida).
ALTER TABLE compra ADD COLUMN xml_verificacao TEXT NOT NULL DEFAULT 'nao_verificada';
ALTER TABLE compra ADD COLUMN xml_prova TEXT NOT NULL DEFAULT '';
ALTER TABLE compra ADD COLUMN autorizacao TEXT NOT NULL DEFAULT 'anterior';
ALTER TABLE compra ADD COLUMN autorizacao_dados TEXT NOT NULL DEFAULT '';
