# R-05 (revisado) · Atraso de sincronização × inconsistência comprovada — critérios e testes antes do código

**Versão 2**, substitui o documento anterior (que previa "24 h viram inconsistente"). Ramo `versao-0.9.11`, versão 0.9.11 (hoje a base é a 0.9.12; este documento é histórico). Escrito antes do código; o código e os testes R5-T1 a R5-T11 vieram depois (migração 0017).
**Sua orientação (20/10, 10:44):** não classificar como inconsistente só porque passaram 24 h; separar atraso de inconsistência comprovada; bloquear só onde for realmente necessário.

## 1. O que muda em relação à versão 1

| Ponto | Antes (código de hoje e versão 1) | Agora |
|---|---|---|
| Referência que espera o registro "pai" | Depois de 24 h vira `inconsistente` sozinha | **Nunca vira inconsistente só por tempo.** Continua `esperando`. Passadas 24 h ganha o aviso "atrasada" (informativo, não bloqueia) |
| O que prova uma inconsistência | O relógio | **Só evidência** (§2) |
| O que bloqueia | Todas as famílias de registros | **Só as operações que mexem em dinheiro de um registro comprovadamente inconsistente** (§4) |
| Saída do gerente | "Aceitar como está" | Igual, **só** para registro comprovadamente inconsistente (auditada) |
| `esperando` e `apagado` | Não bloqueiam | Não bloqueiam |

## 2. Quando um registro é "inconsistente" (comprovado)

Uma referência quebrada (o registro aponta para um "pai" que não está no banco) é classificada assim, **nesta ordem**:

1. **Apagado** — a última entrada do log sobre o pai é "apagar". Não bloqueia (a correção é editar e escolher outro pai).
2. **Inconsistente, prova do log** — o log de alterações **diz** que o pai foi gravado, mas a tabela não o tem. Contradição entre dois registros locais: é prova por si. (Já é assim hoje.)
3. **Inconsistente, prova dos pares** — o log nunca mencionou o pai **e** o caixa tem pelo menos um outro caixa conhecido **e todos** os caixas conhecidos já fizeram uma troca **completa** (nada mais a enviar, contador no máximo do outro) **depois** de a referência aparecer **e** o pai continua ausente. Raciocínio: cada caixa tem uma cópia completa do log e o pai sempre é gravado antes do filho; se todos os que se conhecem já entregaram tudo o que têm e o pai não veio, ele não existe em lugar nenhum que este caixa alcance.
4. **Esperando** — todo o resto: ainda há caixa conhecido sem troca completa, ou não há caixa conhecido (o caixa pode simplesmente ainda não ter encontrado os outros). **Sem prova, não se bloqueia.**

Atraso: `esperando` há 24 h ou mais ganha o marcador **atrasada** (aparece na lista, na contagem "atrasadas" e muda o texto de ajuda para "confira se os outros caixas estão ligados e na mesma versão"). Atrasada **não** é uma classe de inconsistência e **não** bloqueia nada.

Os estados guardados passam a ser: `esperando`, `apagado`, `inconsistente` (com o motivo: `log` ou `pares`) e `aceito`.

## 3. Reavaliação, retorno e aceite

- A classificação é refeita a cada conferência (a cada troca de dados): **sem estado "grudado"**. Se um caixa novo aparece e ainda não trocou tudo, uma prova dos pares deixa de valer e o registro volta a `esperando`, desbloqueado.
- O pai chegou: a linha some e o bloqueio some sozinho.
- **Aceitar como está** (só gerente, só para `inconsistente`; motivo obrigatório; auditoria com quem/quando/motivo): estado `aceito`, libera as operações. Vale enquanto a falta for a mesma; se a falta mudar, volta a ser avaliada. Não inventa o pai.

## 4. O que bloqueia (e o que não bloqueia)

**Bloqueia**, para o registro afetado e **somente quando `inconsistente`** (nunca `esperando`, `apagado` ou `aceito`): as operações que movem dinheiro ou estoque *daquele* registro.

| Registro | Operações bloqueadas | Por que é necessário |
|---|---|---|
| `conta_receber` (sem cliente/venda) | `receber::receber`, `estornar`, `cancelar_conta` na conta | baixa/estorno sem saber de quem é a dívida gera recibo errado |
| `recebimento`, `recebimento_parcela` | `receber::estornar` | idem |
| `cheque` (sem cliente) | `cheques::depositar`, `repassar`, `devolver`, `desfazer`, `cancelar` | movem dinheiro e lançam no caixa |
| `conta_pagar`, `pagamento` (sem fornecedor/compra/categoria/conta da loja) | `pagar::pagar`, `estornar`, `alterar`, `cancelar` | movem dinheiro e saldo da conta da loja |
| `compra`, `compra_item` (sem fornecedor/produto) | `compras::lancar`, `atualizar`, `estornar` | movem estoque e custo |
| `devolucao`, `devolucao_item` (sem venda/item) | `devolucoes::desfazer` | mexe em estoque e dinheiro |
| `movimento_conta` (sem categoria/conta da loja) | `financeiro::estornar_movimento` | mexe em saldo (`transferir` só cria registros novos e não é bloqueada) |

**Não bloqueia** (ver, imprimir e exportar também continuam em todos os casos): vender, abrir e **fechar caixa**, receber/pagar **outras** contas do mesmo cliente/fornecedor, encomendas, ordem de serviço, comissões, cadastros, ajustes de estoque, e tudo que fica sob uma peça/pessoa ausente (sem o pai não há tela que chegue ao filho: `produto_*`, `pessoa_veiculo`, `estoque_movimento`, `venda_item`, `venda_pagamento`, `caixa_movimento`).
A diferença para a versão 1: saem do bloqueio encomendas, ordens de serviço, comissões e o fechamento do caixa. Não há "caso real de necessidade" nelas: errar ali não move dinheiro, e bloquear atrapalharia o dia da loja.

## 5. Como o gerente recupera cada caso

| Caso | O que acontece | O que o gerente faz |
|---|---|---|
| `esperando` | meio de uma troca, ou um caixa desligado | Nada: resolve sozinho. Se ficar **atrasada** (≥ 24 h): ligar os outros caixas, mesma rede, mesma versão; ver em Caixas da loja se algum par está com erro |
| `inconsistente` (log) | banco restaurado de backup/troca interrompida | Fazer backup; deixar os caixas trocarem dados (o Rustinho recomeça a troca do zero depois de restauração); se continuar, **Aceitar como está** com o motivo |
| `inconsistente` (pares) | todos os caixas conhecidos entregaram tudo e o pai não existe (o caixa que o criou foi perdido) | Restaurar o backup do caixa perdido num computador e juntar de novo; ou **Aceitar como está** com o motivo |
| `apagado` | o pai foi apagado em um caixa | Abrir o registro e escolher outro pai (não bloqueia) |
| operação recusada | mensagem diz qual registro, qual pai falta e o motivo da prova | os passos acima |

## 6. Critérios de aceitação

| ID | Critério |
|---|---|
| R5-1 | Nenhuma referência vira `inconsistente` por tempo. Com `desde` de 30 dias e sem prova, continua `esperando` e **atrasada**. |
| R5-2 | `esperando` atrasada tem marcador próprio e contagem própria (`atrasadas`), distinta da de `quebradas`. |
| R5-3 | Prova do log classifica `inconsistente` (motivo `log`). |
| R5-4 | Prova dos pares exige: ≥ 1 par conhecido, **todos** com troca completa depois do `desde`. Se faltar um só, é `esperando`. Sem nenhum par, é `esperando`. |
| R5-5 | A classificação é refeita a cada conferência; a prova dos pares se desfaz se aparece um par sem troca completa. |
| R5-6 | O pai chega: a linha some e o bloqueio some sem ação do gerente. |
| R5-7 | `exigir_editavel` recusa as operações da tabela do §4 só para `inconsistente`; `esperando`, `apagado` e `aceito` passam. |
| R5-8 | Operação sobre **outro** registro do mesmo cliente/fornecedor passa; ver/imprimir passam; vender e fechar caixa passam. |
| R5-9 | "Aceitar como está": só gerente; exige motivo; grava quem/quando/motivo na auditoria; libera; se a falta mudar volta a ser avaliado. |
| R5-10 | Todo comando de gravação das famílias do §4 chama `exigir_editavel` (teste de varredura que falha se uma operação nova esquecer). |
| R5-11 | A troca de dados marca, por par, o momento da última troca **completa** (`sync_par.completo_em`) sem mudar o protocolo. |

## 7. Testes planejados (escritos junto do código, rodando antes das mutações)

| ID | Teste | Critério |
|---|---|---|
| R5-T1 | referência com `desde` de 30 dias, sem pares: `esperando` + atrasada, **não** `inconsistente`; operação passa | R5-1, R5-2, R5-4, R5-7 |
| R5-T2 | log diz que o pai existe e a tabela não: `inconsistente` (log); operação de dinheiro recusada com mensagem | R5-3, R5-7 |
| R5-T3 | dois pares, só um com troca completa depois do `desde`: `esperando`; o segundo completa: `inconsistente` (pares); um terceiro par novo entra sem troca: volta a `esperando` | R5-4, R5-5 |
| R5-T4 | pai chega: linha e bloqueio somem sozinhos | R5-6 |
| R5-T5 | `apagado` não bloqueia; a correção (trocar o pai) passa | R5-7 |
| R5-T6 | uma operação por família do §4 recusada quando `inconsistente` e liberada em `esperando`/`apagado`/`aceito` | R5-7 |
| R5-T7 | outro registro do mesmo cliente passa; venda e fechamento de caixa passam; leitura passa | R5-8 |
| R5-T8 | aceite: operador recusado; gerente sem motivo recusado; gerente com motivo libera e audita; falta diferente reavalia | R5-9 |
| R5-T9 | varredura: lê o código-fonte de `receber`, `cheques`, `pagar`, `compras`, `devolucoes`, `financeiro` e falha se uma função pública de gravação listada não chama `exigir_editavel` | R5-10 |
| R5-T10 | `completo_em` é gravado só quando a troca termina sem `temMais` e com contador no máximo do outro; corte no meio não grava | R5-11 |
| R5-T11 | migração 16 → 17 preserva `sync_referencia` existente (estados antigos `inconsistente` por tempo são reavaliados na próxima conferência) | R5-1 |
| R5-M | mutações: voltar a promover por tempo; prova dos pares com "algum" em vez de "todos"; tirar `exigir_editavel` de uma operação; bloquear `esperando`; aceite sem motivo | todos |

## 8. O que continua não coberto (limites)

- Banner vermelho/amarelo **dentro** da tela de cada registro: não faz parte desta entrega; o aviso fica na lista de Configurações › Caixas da loja e na mensagem da recusa.
- A prova dos pares supõe que cada caixa conhecido entrega o log inteiro e o pai é gravado antes do filho (é o desenho atual). Rede real com vários caixas é validação física (PEND-05).
- Um caixa aposentado que nunca mais liga impede a prova dos pares; o registro fica `esperando/atrasada` (sem bloqueio). Esquecer um par não existe hoje e não é criado aqui.
