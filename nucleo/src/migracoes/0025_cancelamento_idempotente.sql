-- Duas caixas offline não podem aplicar os efeitos de cancelar a mesma venda
-- como se fossem operações independentes. O id local muda, mas venda_id é
-- estável: ao convergir, a restrição suspende o lote concorrente inteiro.
CREATE TABLE venda_cancelamento (
    id          TEXT PRIMARY KEY,
    venda_id    TEXT NOT NULL UNIQUE REFERENCES venda (id),
    dispositivo TEXT NOT NULL,
    usuario_id  TEXT,
    criado_em   TEXT NOT NULL,
    motivo      TEXT NOT NULL
);
