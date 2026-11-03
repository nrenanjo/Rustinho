-- 0.9.1: unidades de medida, serviço, oferta, tamanhos/cores e fiscal completo.

-- Unidades de medida. A lista já vem arrumada (sem repetidas) e a loja pode
-- criar as suas. fracionada = vende quebrado (1,5 m; 0,350 kg).
CREATE TABLE unidade (
    sigla      TEXT PRIMARY KEY,
    descricao  TEXT NOT NULL DEFAULT '',
    fracionada INTEGER NOT NULL DEFAULT 0
);

INSERT INTO unidade (sigla, descricao, fracionada) VALUES
    ('UN', 'Unidade', 0),
    ('PC', 'Peça', 0),
    ('JG', 'Jogo', 0),
    ('KIT', 'Kit', 0),
    ('PAR', 'Par', 0),
    ('CJ', 'Conjunto', 0),
    ('CX', 'Caixa', 0),
    ('PCT', 'Pacote', 0),
    ('DZ', 'Dúzia', 0),
    ('FD', 'Fardo', 0),
    ('SC', 'Saco', 0),
    ('FR', 'Frasco', 0),
    ('GL', 'Galão', 0),
    ('BD', 'Balde', 0),
    ('LATA', 'Lata', 0),
    ('TB', 'Tubo', 0),
    ('PT', 'Pote', 0),
    ('CT', 'Cartela', 0),
    ('RL', 'Rolo', 0),
    ('KG', 'Quilo', 1),
    ('G', 'Grama', 1),
    ('TON', 'Tonelada', 1),
    ('LT', 'Litro', 1),
    ('ML', 'Mililitro', 1),
    ('MT', 'Metro', 1),
    ('CM', 'Centímetro', 1),
    ('MM', 'Milímetro', 1),
    ('M2', 'Metro quadrado', 1),
    ('M3', 'Metro cúbico', 1);

-- As antigas "L" e "M" viram "LT" e "MT" (a mesma coisa, sem repetir).
UPDATE produto SET unidade = 'LT' WHERE unidade = 'L';
UPDATE produto SET unidade = 'MT' WHERE unidade = 'M';
-- Unidades já usadas que não estão na lista entram como da loja.
INSERT OR IGNORE INTO unidade (sigla, descricao)
    SELECT DISTINCT unidade, unidade FROM produto WHERE unidade <> '';

-- produto ou servico (serviço não tem estoque).
ALTER TABLE produto ADD COLUMN tipo TEXT NOT NULL DEFAULT 'produto';

-- Oferta: troca o preço da primeira tabela até a data (vazio = sem prazo).
-- preco_aberto = o preço é digitado na hora da venda.
ALTER TABLE produto ADD COLUMN oferta_preco INTEGER;
ALTER TABLE produto ADD COLUMN oferta_ate TEXT NOT NULL DEFAULT '';
ALTER TABLE produto ADD COLUMN preco_aberto INTEGER NOT NULL DEFAULT 0;

-- Tamanhos e cores: cada um é um produto (com código, estoque e preço
-- próprios); os da mesma grade têm o mesmo "grade". variacao = "P", "Azul M".
ALTER TABLE produto ADD COLUMN grade TEXT NOT NULL DEFAULT '';
ALTER TABLE produto ADD COLUMN variacao TEXT NOT NULL DEFAULT '';
CREATE INDEX produto_grade ON produto (grade) WHERE grade <> '';

-- Dados fiscais para a nota (IPI, PIS/Cofins, combustível, serviço), em JSON.
ALTER TABLE produto ADD COLUMN fiscal TEXT NOT NULL DEFAULT '{}';
