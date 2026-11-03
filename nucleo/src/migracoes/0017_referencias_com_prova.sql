-- R-05 (revisado): atraso de sincronização não é inconsistência. Uma
-- referência só é "inconsistente" com prova (o log diz que o registro existe e
-- a tabela não tem, ou todos os caixas conhecidos já entregaram tudo depois de
-- ela aparecer e o registro continua faltando); só o gerente "aceita" uma
-- inconsistência, com motivo.

-- Quando a última troca COMPLETA com este caixa terminou (nada mais a receber,
-- contador no máximo do outro). Zerada quando a troca recomeça do zero.
ALTER TABLE sync_par ADD COLUMN completo_em TEXT;

-- Por que a referência é inconsistente: 'log' ou 'pares' (vazio nos outros estados).
ALTER TABLE sync_referencia ADD COLUMN motivo TEXT NOT NULL DEFAULT '';
-- Aceite do gerente (estado 'aceito').
ALTER TABLE sync_referencia ADD COLUMN aceita_por TEXT;
ALTER TABLE sync_referencia ADD COLUMN aceita_em TEXT;
ALTER TABLE sync_referencia ADD COLUMN aceite_motivo TEXT;

CREATE INDEX sync_referencia_por_registro ON sync_referencia (tabela, estado);

-- As que a regra antiga chamou de "inconsistente" só porque esperaram 24 horas
-- voltam a "esperando" agora (a data de começo fica, então aparecem como
-- atrasadas); a próxima conferência as reavalia e só as comprovadas voltam a
-- ser inconsistentes. Nada é apagado.
UPDATE sync_referencia SET estado = 'esperando', motivo = '' WHERE estado = 'inconsistente';
