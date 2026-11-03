# Teste rápido da 0.9.13 no Linux (AppImage)

Use uma pasta de dados separada para não misturar com a loja:

```
chmod +x Rustinho-0.9.13-Linux-x86_64.AppImage
RUSTINHO_DADOS=~/rustinho-teste ./Rustinho-0.9.13-Linux-x86_64.AppImage
```

Para apagar o teste depois: feche o Rustinho e apague a pasta `~/rustinho-teste`.

| # | Faça | Deve acontecer |
|---|---|---|
| 1 | Configure a loja com a senha `1234` | Aceita (só o tamanho conta: mínimo 4). `123` é recusada. |
| 2 | Configurações → Loja → CNPJ `12.ABC.345/01DE-35` | Aceita e mostra `12.ABC.345/01DE-35`. Com o último número trocado (`…-36`), recusa. |
| 3 | Cadastre um fornecedor com o mesmo CNPJ com letras | Aceita; a busca por `12abc` acha. |
| 4 | Cadastre 2 peças com custo e 1 sem custo (preço na hora); venda algumas | — |
| 5 | Painel → Resumo | Vendas líquidas, lucro bruto "incompleto" (por causa da peça sem custo), despesas, resultado estimado, ticket e avisos. |
| 6 | Financeiro → Lançar conta: Aluguel, categoria Aluguel, mês de referência deste mês | Conta lançada; na ficha aparece "referente a mm/aaaa". |
| 7 | Painel → Resultado (DRE) → Mês atual | Aluguel em "Despesas fixas"; resultado estimado; pendências (peça sem custo, classificação não confirmada, nenhum imposto lançado). |
| 8 | Toque em "Receita bruta de vendas" | Abre a lista das vendas que somam o total. |
| 9 | Hoje | O aluguel não entra (aviso de contas mensais). |
| 10 | Classificar categorias → marque a confirmação → Salvar | A pendência de classificação some. |
| 11 | Metas → Vendas líquidas, Loja, 1000 → Salvar meta | Aparece com o percentual atingido; no Resumo, o aviso da meta. |
| 12 | Imprimir | Gera o PDF da DRE. |
| 13 | Peças e vendedores | Rankings por quantidade, receita, lucro e margem; a peça sem custo não aparece no lucro. |
| 14 | Crie um usuário caixa, entre com ele e abra o Painel | Sem lucro; Resultado (DRE) pede a senha do gerente. |
| 15 | Olho (Esconder valores) em cada tela | Os valores somem, inclusive dentro dos textos. |

Isto é teste em um computador só. A troca de dados entre computadores continua sem validação física (FV-1 a FV-13).
