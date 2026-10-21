# Roteiro de teste à mão — Linux (vale também para o Windows)

Para fazer **depois** dos testes automáticos e **antes** de chamar o Rustinho
de estável. Use dados de mentira e uma pasta separada; a loja de verdade e o
SINTEC não são tocados.

```
chmod +x Rustinho-*-Linux-x86_64.AppImage
RUSTINHO_DADOS=~/rustinho-teste ./Rustinho-*-Linux-x86_64.AppImage
```

Para apagar o teste: feche o Rustinho e apague a pasta `~/rustinho-teste`
(e `~/rustinho-teste-2` da parte B).

Anote em cada linha: ✔ (deu certo), ✘ (deu errado: o que apareceu) ou — (não deu
para fazer). Mande as fotos dos ✘.

## A. O que mudou na 0.9.13

Os 15 passos de `docs/TESTE-0.9.13-linux.md` (senha de 4, CNPJ com letras,
DRE, metas, olho). Faça primeiro: os cadastros dali servem para o resto.

## B. Backup e restauração (prioridade 1)

| # | Faça | Deve acontecer |
|---|---|---|
| B1 | Anote: total de vendas do mês (Painel), estoque de 3 peças, quanto o Zé deve, contas a pagar. | — |
| B2 | Configurações → Backup → Fazer backup agora. | Aparece a hora do último backup. |
| B3 | Feche. Abra uma loja nova: `RUSTINHO_DADOS=~/rustinho-teste-2 ./Rustinho…`. Configure com outro nome e senha. | Loja vazia. |
| B4 | Backup → Restaurar → escolha o arquivo da pasta `~/rustinho-teste/backups`. Senha errada primeiro. | Recusa e não muda nada. |
| B5 | Agora com a senha do gerente da loja do backup. | Pede para entrar de novo; os usuários são os da loja do backup. |
| B6 | Confira tudo o que anotou em B1. | Tudo igual. |
| B7 | Faça uma venda na loja restaurada. | Funciona; o estoque baixa. |
| B8 | Restaurar de novo, escolhendo a cópia "de antes de restaurar". | Volta a loja vazia do B3. |

## C. Estoque (prioridade 1)

| # | Faça | Deve acontecer |
|---|---|---|
| C1 | Peça com estoque 10. Venda 3. | Estoque 7. |
| C2 | Venda 2 e cancele a venda. | Estoque 7 de novo. |
| C3 | Devolva 1 da venda do C1 (boa) e 1 com defeito. | Estoque 8 (a com defeito não volta). |
| C4 | Tente devolver mais do que sobrou. | Recusa. |
| C5 | Compras → importe um XML de compra → lance. | Estoque das peças sobe. |
| C6 | Estorne a compra. | Estoque volta. |
| C7 | Estoque → ajustar para a quantidade contada (como gerente). Depois entre como caixa e tente. | Gerente ajusta; o caixa precisa da senha do gerente. |
| C8 | Venda mais do que tem. | Avisa antes; se confirmar, vende e o estoque fica negativo. |

## D. Crediário (prioridade 1)

| # | Faça | Deve acontecer |
|---|---|---|
| D1 | Venda R$ 300 no crediário para o Zé, em 3 vezes. | Zé deve R$ 300 (3 de R$ 100). |
| D2 | Receber do Zé R$ 50 em dinheiro. | Deve R$ 250; a 1ª parcela fica com R$ 50; a gaveta tem +R$ 50. |
| D3 | Receber R$ 150 no pix. | Fica só a 3ª (R$ 100); a gaveta não muda. |
| D4 | Receber R$ 120 em dinheiro. | Quita; troco de R$ 20. |
| D5 | Como gerente, estorne o último recebimento. | O Zé volta a dever R$ 100; o dinheiro sai da gaveta. |
| D6 | Configurações: multa 2%, juros 1% ao mês. (Atraso real só dá para ver com uma parcela vencida de verdade.) | Na parcela vencida aparecem multa e juros. |

## E. DRE (prioridade 1)

| # | Faça | Deve acontecer |
|---|---|---|
| E1 | Painel → Resultado (DRE) → Mês atual. Some à mão as vendas, descontos, devoluções e despesas do mês. | Bate com a DRE. |
| E2 | Lance uma conta do mês passado (Mês de referência = mês passado). | Este mês não muda; o mês passado muda. |
| E3 | Mostre a DRE ao contador. | Ele confirma a classificação (Classificar categorias → confirmar). |

## F. Permissões (prioridade 1)

| # | Faça | Deve acontecer |
|---|---|---|
| F1 | Entre como caixa. Abra Estoque → Mais vendidos. | Não aparece lucro nem margem. |
| F2 | Início e Painel → Resumo. | Não aparece "A pagar" nem o aviso de contas a pagar. |
| F3 | Tente Financeiro, Usuários, Backup, cancelar venda, estornar, ajustar estoque, DRE (também pelos atalhos do teclado). | Sempre pede a senha do gerente. |
| F4 | Como gerente, desative o usuário caixa enquanto ele está aberto em outro computador (ou: desative, volte ao caixa e clique em qualquer coisa). | O caixa volta para a tela de entrada com "Seu usuário foi desativado". |
| F5 | Como caixa, abra uma peça que tem custo. Depois mude o preço dela e salve (F2). | Não aparece o custo nem o "% sobre o custo"; ao salvar, pede a senha do gerente. Com a senha, salva. Mude de novo: pede de novo. |
| F6 | Como caixa, lance uma compra por XML com uma peça nova. | Vê o custo de cada item; a peça nova sai com o preço; abrindo a peça, não aparece o custo. |
| F7 | Como caixa, abra o Financeiro com a senha do gerente e tente estornar um pagamento. | Pede a senha do gerente de novo (a do Financeiro não vale para estornar). |

## G. Fechamento inesperado (prioridade 1)

| # | Faça | Deve acontecer |
|---|---|---|
| G1 | No meio de uma venda (itens na tela, sem concluir), feche à força: `pkill -9 -f Rustinho`. Abra de novo. | Abre normal; a venda não concluída não existe; nada pela metade. |
| G2 | Conclua uma venda e logo em seguida feche à força. Abra. | A venda está lá, com o estoque baixado. |
| G3 | Faça backup agora e feche à força enquanto aparece "Fazendo o backup…". Abra e faça outro. | Os dados estão iguais; o backup novo funciona. |

## H. Importação do SINTEC (prioridade 2)

Use uma **cópia** da exportação (pen drive ou pasta copiada), nunca a pasta do
SINTEC. Anote antes quantos arquivos há na cópia.

| # | Faça | Deve acontecer |
|---|---|---|
| H1 | Configurações → Loja: CNPJ da loja. | — |
| H2 | Importar cadastros → escolha a pasta da cópia → Prévia. | Mostra quantas notas, clientes, fornecedores e peças. Anote. |
| H3 | Importar. | Os números batem com a prévia. |
| H4 | Importar de novo. | Nada novo. |
| H5 | Confira a cópia. | Mesma quantidade de arquivos; nada novo nem apagado. |
| H6 | Abra 3 peças importadas e compare com o SINTEC. | Nome, código e preço iguais aos da nota mais nova. |

## I. Troca de dados entre dois computadores (prioridade 2)

Só com dois computadores. São os testes FV-1 a FV-13 de
`docs/TESTES-estabilidade.md`. O mínimo antes de usar na loja: FV-1, FV-3,
FV-4, FV-8 e FV-10, conferindo no fim que os dois têm as mesmas vendas, o
mesmo estoque e nada repetido.

## J. Impressão

| # | Faça | Deve acontecer |
|---|---|---|
| J1 | Imprima uma venda, um orçamento, um recibo do crediário, o fechamento do caixa e a DRE. | Gera os PDFs, legíveis, com os valores certos. |
