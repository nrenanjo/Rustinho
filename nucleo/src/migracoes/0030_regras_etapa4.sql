-- Etapa 4 da 0.9.19 Beta (decisões do proprietário de 25/11/2026):
--   1. Decisão 6 (R15, vale-troca usado além do valor): `vale.acertado` guarda o quanto do que passou do valor já foi
--      acertado pela compensação do gerente (a dívida lançada no crediário do cliente, ou a loja assumindo). O
--      conflito de "vale usado a mais" só existe enquanto `saldo + acertado < 0`. Viaja com o vale.
--
-- 100% aditiva sobre a estrutura 29: nada existente muda nem perde dados.

ALTER TABLE vale ADD COLUMN acertado INTEGER NOT NULL DEFAULT 0;
