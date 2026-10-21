# DRE gerencial e indicadores (0.9.13)

> A DRE gerencial **não substitui** a escrituração contábil nem a DRE oficial.
> A classificação das categorias é uma sugestão do Rustinho e precisa ser
> **confirmada pelo contador**. O resultado é sempre chamado de
> **"resultado gerencial estimado"**, nunca de lucro líquido.

## 1. Etapa 1: auditoria (o que já existia)

| Dado | Onde está | Serve para | Lacuna encontrada |
|---|---|---|---|
| Vendas concluídas | `venda` (`tipo = 'venda'`, `situacao` concluida/cancelada, `criado_em` UTC) | receita bruta, descontos, quantidade, ticket | Venda só existe depois de concluída (não há venda "em andamento" gravada). Cancelar muda a `situacao` e grava `cancelado_em`, sem lançamento de estorno. |
| Itens | `venda_item` (`quantidade` em milésimos, `preco`, `desconto` do item, `total`, `custo` unitário **na hora da venda**) | receita por peça, CMV | `custo = 0` é "sem custo no cadastro" (o Painel antigo contava como lucro inteiro). Serviço = item sem `produto_id` ou peça do tipo `servico`. |
| Desconto do cabeçalho | `venda.desconto` = desconto dos itens + desconto geral | deduções; rateio por peça | O desconto geral não é gravado por item: rateio proporcional ao total do item. |
| Custo | `produto.custo`: último custo de entrada (nota + frete, seguro, IPI, ST rateados), copiado para o item na venda | CMV histórico | Não é custo médio. A cópia na venda é o custo histórico disponível. |
| Devoluções | `devolucao` / `devolucao_item` (`valor` devolvido, `custo`, `volta_estoque`) | deduções, volta do CMV | Desfazer a devolução grava `cancelado_em` (antes sumia do mês). |
| Contas a pagar | `conta_pagar` (`categoria_id`, `vencimento`, `compra_id`, cancelamento) e `pagamento` (`juros`, `desconto`) | despesas por competência, resultado financeiro | **Não havia data de competência.** Duplicatas de compra (`compra_id`) são estoque, não despesa. |
| Movimentos avulsos | `movimento_conta` (`entrada`/`saida` com categoria; transferência, sangria, ajuste…) | despesas e receitas pagas na hora | Transferência, sangria, suprimento, depósito de cheque e ajuste de saldo não são resultado. |
| Categorias | `categoria` (despesa/receita, 14 de fábrica) | classificação | **Não havia classificação para a DRE.** |
| Crediário | `recebimento` (`juros`, `desconto`) | receitas/despesas financeiras | Receber parcela não é venda nova. Dívida lançada à mão (sem venda) não é receita. |
| Comissões | `comissao_fechamento` → conta a pagar "Comissões" | despesa variável | Entra quando é fechada; competência = mês do fim do período. |
| Taxas de cartão | — | despesa variável | **Não são registradas.** Precisam ser lançadas como despesa. |
| Metas | — | atingimento | **Não existiam.** |
| Vendedor | `venda.vendedor_id` = quem concluiu a venda | desempenho por vendedor | Uma venda tem um vendedor só. |
| Empresa | uma loja (um CNPJ) por banco | isolamento | Não há várias empresas no mesmo banco: nada é somado entre empresas. |

## 2. O que foi acrescentado (migração 0021)

- `categoria.dre_grupo`: em que linha da DRE a categoria entra (vazio = sem classificação, vira pendência).
- `conta_pagar.competencia` (AAAA-MM): mês a que a conta se refere. Vazio = usa o mês do vencimento **e aparece como pendência**.
- `meta`: metas mensais (padrão) ou semanais, da loja ou por vendedor.
- Configuração `dre_classificacao_confirmada`: o gerente confirma a classificação (fica na auditoria).
- Permissão nova do gerente **"Resultados"** (lucro, custos, despesas e DRE). Sem ela, o Painel mostra só vendas, quantidade e ticket.

## 3. Regras (regime de competência gerencial)

- **Receita bruta** = soma de `venda.subtotal` das vendas concluídas no período (data da venda), **inclusive as canceladas depois** — o mês fechado não muda.
- **Cancelamento de venda concluída**: entra como estorno (dedução) na data do cancelamento; o CMV volta na mesma data (a peça volta ao estoque).
- **Devoluções**: dedução pelo valor devolvido, na data da devolução. O custo volta ao CMV só se a peça **voltou ao estoque**; peça com defeito mantém o custo (aparece como "perda com defeito"). Devolução desfeita depois volta a receita na data em que foi desfeita.
- **Tributos sobre as vendas**: só o que estiver lançado em categoria classificada como "Tributos sobre as vendas" (ex.: DAS). Nenhuma alíquota é presumida.
- **CMV** = quantidade × custo do item na hora da venda. Peça sem custo (custo 0) **não vira zero**: o CMV fica incompleto e a receita dessas peças aparece separada.
- **Despesas** por competência: contas a pagar pelo mês de competência (ou do vencimento, como pendência), não pela data do pagamento; pagar uma conta não cria outra despesa. Movimentos avulsos pela data. Duplicatas de compra de mercadoria ficam **fora** (entram pelo CMV quando a peça é vendida).
- **Contas mensais** (as que têm competência por mês) só entram em períodos que contêm o mês inteiro, ou no mês corrente até hoje. Em "Hoje", "7 dias" e períodos quebrados, elas ficam de fora e isso é avisado.
- **Resultado financeiro**: juros pagos e descontos concedidos no crediário (despesa); juros recebidos e descontos obtidos (receita); mais as categorias classificadas como financeiras.
- **Fora do resultado**: transferências, sangria, suprimento, depósito de cheque, ajustes de saldo, empréstimos e aportes (categoria "Fora do resultado"), compras de mercadoria.
- **Indicadores comerciais** (vendas líquidas, quantidade, ticket, itens por venda, peças, vendedores) usam as vendas concluídas e **não canceladas**; devoluções e cancelamentos aparecem separados.

## 4. Fórmulas

```
Receita líquida            = receita bruta − descontos − devoluções − cancelamentos − tributos sobre vendas
Lucro bruto                = receita líquida − CMV
Margem bruta (%)           = lucro bruto ÷ receita líquida × 100      (não calcula se a receita líquida ≤ 0)
Margem de contribuição     = lucro bruto − despesas variáveis
Resultado operacional est. = margem de contribuição − despesas fixas + outras receitas operacionais
                             − outras despesas operacionais − despesas sem classificação
Resultado financeiro       = receitas financeiras − despesas financeiras
Resultado gerencial est.   = resultado operacional + resultado financeiro
                             + receitas não operacionais − despesas não operacionais
Vendas líquidas            = soma das vendas concluídas e não canceladas (já com desconto)
Ticket médio               = vendas líquidas ÷ quantidade de vendas
Itens por venda            = unidades de peças vendidas ÷ quantidade de vendas
Margem do produto (%)      = (receita da peça − desconto geral rateado − CMV da peça) ÷ receita da peça × 100
Atingimento da meta (%)    = realizado ÷ meta × 100
```

Despesas sem classificação **entram** no resultado (como despesa) e deixam o resultado incompleto: nada some do relatório.

## 5. Completude

O resultado é "completo com os dados registrados" só quando não há:
peça vendida sem custo; despesa sem categoria ou com categoria sem classificação;
conta sem competência; classificação não confirmada pelo gerente; contas mensais
fora do período. Cada pendência aparece com quantidade e valor.

## 6. Conferência manual (cenário do teste `dre_cenario_manual`)

Mês corrente, tudo lançado pelo teste:

| Lançamento | Valor |
|---|---|
| Venda 1 à vista: 2 × R$ 100,00 (custo R$ 60,00), desconto geral R$ 20,00 | bruto 200,00; desconto 20,00 |
| Venda 2 a prazo (crediário): 2 × R$ 100,00 (custo R$ 60,00) | bruto 200,00 |
| Venda 3: 1 × R$ 50,00, peça **sem custo** | bruto 50,00 |
| Devolução de 1 peça da venda 2 (voltou ao estoque, abateu o crediário) | devolvido 100,00; custo volta 60,00 |
| Aluguel (fixa), competência do mês | 1.000,00 |
| DAS (tributos sobre vendas), competência do mês | 30,00 |
| Recebimento do resto do crediário (R$ 100,00) com R$ 2,00 de juros | só o juro entra |

Conta à mão:

```
Receita bruta        200 + 200 + 50                  = 450,00
Descontos                                            =  20,00
Devoluções                                           = 100,00
Tributos (DAS)                                       =  30,00
Receita líquida      450 − 20 − 100 − 30             = 300,00
CMV                  2×60 + 2×60 − 60 (volta)        = 180,00  (incompleto: venda 3 sem custo)
Lucro bruto          300 − 180                       = 120,00  · margem 40%
Despesas fixas                                       = 1.000,00
Resultado operacional 120 − 1.000                    = −880,00
Resultado financeiro  +2,00 (juros recebidos)        =   2,00
Resultado gerencial estimado                         = −878,00 (incompleto)
Vendas líquidas (comercial) 180 + 200 + 50           = 430,00 · 3 vendas · ticket 143,33 · 1,6 peça por venda
```

O teste `dre::testes::dre_cenario_manual` confere exatamente estes números; os
outros testes `dre::testes::*` cobrem venda cancelada em outro mês, devolução
desfeita, peça com defeito, pagamento posterior, compra de estoque, contas
mensais em "Hoje", conta sem competência, categoria sem classificação, mês sem
dados, alteração de custo depois da venda, rankings, vendedores, metas e
rastreabilidade (cada total = soma do detalhe).

**Antes de declarar o recurso concluído, a conferência precisa ser aprovada
pelo dono da loja (e pelo contador, para a classificação).**
