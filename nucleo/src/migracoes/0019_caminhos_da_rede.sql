-- PEND-10: os caminhos (endereços) até cada outro caixa.
-- Um caixa é o seu `dispositivo`; o endereço é só um jeito de chegar nele, e ele
-- pode ter vários (cabo, Wi-Fi, digitado). Só deste computador (não entra em
-- TABELAS): cada caixa enxerga os outros por redes diferentes.
-- `sync_par.cursor`, `marca` e `geracao` continuam pertencendo ao dispositivo.
CREATE TABLE sync_endereco (
    endereco     TEXT PRIMARY KEY,                -- "ip:porta"
    dispositivo  TEXT NOT NULL DEFAULT '',        -- '' = digitado, ainda sem conversa
    digitado     INTEGER NOT NULL DEFAULT 0,      -- veio de Configurações: não some sozinho
    visto_ms     INTEGER NOT NULL DEFAULT 0,      -- último anúncio/pedido vindo dele
    ultimo_ok_ms INTEGER NOT NULL DEFAULT 0,      -- última puxada que funcionou
    tentado_ms   INTEGER NOT NULL DEFAULT 0,      -- última tentativa (rodízio entre os mortos)
    falhas       INTEGER NOT NULL DEFAULT 0,      -- seguidas
    proxima_ms   INTEGER NOT NULL DEFAULT 0,      -- espera depois de falhar
    erro         TEXT NOT NULL DEFAULT '',        -- último erro deste caminho
    interface    TEXT NOT NULL DEFAULT '',        -- classe CONFIRMADA ("cabo", "Wi-Fi", ...) ou ''
    confirmada   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX sync_endereco_dispositivo ON sync_endereco (dispositivo);

-- Os endereços que os pares já tinham continuam valendo.
INSERT INTO sync_endereco (endereco, dispositivo, visto_ms)
SELECT endereco, dispositivo, CAST(strftime('%s', 'now') AS INTEGER) * 1000
  FROM sync_par WHERE endereco <> '';
