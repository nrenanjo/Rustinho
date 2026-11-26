-- 0.9.19, etapa 3 (FIS-P5 e FIS-P6): as regras de ST com a fonte de cada uma.
--
-- Cada regra guarda de onde veio (H §27 do texto H): a fonte (ato e endereço
-- oficial), o dispositivo e a data em que a fonte foi lida. `origem` diz quem
-- escreveu a linha: 'rustinho' (veio com o programa, conferida na fonte
-- oficial) ou 'loja' (o contador ou o gerente mudou à mão). Uma atualização das
-- regras do programa nunca sobrescreve uma linha 'loja' (FIS-P6).
--
-- As regras das outras UFs não entram aqui: uma migração de um passo não cria
-- registros (a conferência da atualização confere as contagens). Elas vêm do
-- catálogo do programa (`catalogo/regras_st_programa.json`), aplicado ao abrir
-- o banco (`fiscal::carregar_regras_do_programa`).
--
-- 100% aditiva sobre a estrutura 31: nada existente muda nem perde dados (as 4
-- regras de SP só ganham a fonte, e só se ninguém mexeu nelas); nenhum
-- registro é criado.

ALTER TABLE regra_st ADD COLUMN fonte TEXT NOT NULL DEFAULT '';
ALTER TABLE regra_st ADD COLUMN dispositivo TEXT NOT NULL DEFAULT '';
ALTER TABLE regra_st ADD COLUMN consultada_em TEXT NOT NULL DEFAULT '';
ALTER TABLE regra_st ADD COLUMN origem TEXT NOT NULL DEFAULT 'loja';

-- As 4 de SP que vieram na 0002: a fonte, se ninguém mexeu nelas. Isto não é
-- alteração da loja (cada caixa faz o mesmo): não vai para o log da troca.
UPDATE sync_estado SET aplicando = 1 WHERE id = 1;
UPDATE regra_st SET origem = 'rustinho', consultada_em = '2026-11-24',
       fonte = 'Portaria SRE 34/2026 (SEFAZ-SP), legislacao.fazenda.sp.gov.br',
       dispositivo = 'revoga o Anexo XIV (autopeças) da Portaria CAT 68/2019, efeitos a partir de 01/10/2026'
 WHERE id IN ('sp-autopecas-com-st', 'sp-autopecas-sem-st');
UPDATE regra_st SET origem = 'rustinho', consultada_em = '2026-11-24',
       fonte = 'Portaria SRE 34/2026 (SEFAZ-SP), legislacao.fazenda.sp.gov.br',
       dispositivo = 'revoga o Anexo VII (pneus) da Portaria CAT 68/2019, efeitos a partir de 01/10/2026'
 WHERE id IN ('sp-pneus-com-st', 'sp-pneus-sem-st');

UPDATE sync_estado SET aplicando = 0 WHERE id = 1;
