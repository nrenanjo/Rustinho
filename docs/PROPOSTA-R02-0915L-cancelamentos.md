# Proposta: R-02 (caixa aberto em outro computador) e 0915-L (cancelamentos em cascata)

**Situação (28/10/2026):** proposta para decisão do dono. **Não implementada**: muda o comportamento de dinheiro, e o briefing manda definir cada caso e documentar os impactos antes de mexer nos fluxos. R-02 continua **bloqueador do uso comercial** (PEND-24). A parte de **detecção** da R-03 (parcela paga a mais, vale usado a mais) foi feita nesta etapa (registro C8, item 9), porque só avisa e não muda valor nenhum.

## 1. A regra que já vale (D-14, decisão de 21/10/2026)

Cancelar, estornar ou desfazer **depois de o caixa da operação estar fechado**: o caixa fechado nunca muda; o dinheiro sai (ou entra) na **gaveta aberta deste computador**; sem gaveta aberta, no **Caixa geral** ("Acerto de caixa já fechado"); o movimento fica ligado à operação de origem, com auditoria, sem duplicar; Pix e cartão não mexem em gaveta. Código: `caixa::sessao_fechada`, `caixa::compensar_caixa_fechado`, marca `cancelado_apos_fechar` (migração 22).

## 2. O problema da R-02

Fonte: a auditoria da 0.9.14 (R-02) e a leitura do código nesta etapa (`vendas::cancelar`, `receber::estornar`, `pagar::estornar`); **não foi reproduzido de novo com dois caixas nesta etapa** — a reprodução vem com a implementação (seção 5). Quando a gaveta da operação **ainda está aberta em outro computador**, `sessao_fechada` diz "não fechada" e o fluxo segue como se fosse a gaveta deste computador:

| Operação (comando) | Hoje, com a gaveta aberta em **outro** computador | Efeito |
|---|---|---|
| Cancelar venda (`venda_cancelar`, `vendas::cancelar`) | a venda sai do total da sessão do outro computador | o fechamento de lá fica com falta; o dinheiro devolvido sai daqui sem registro aqui |
| Estornar recebimento (`recebimento_estornar`, `receber::estornar`) | igual | idem |
| Desfazer devolução (`devolucao_desfazer`) | igual (o dinheiro que tinha saído de lá "volta" para lá) | idem, ao contrário |
| Estornar pagamento que saiu da gaveta de outro computador (`pagamento_estornar`) | o dinheiro volta como ajuste no **Caixa geral**, mesmo com a gaveta deste computador aberta | diverge da regra D-14 ("gaveta aberta deste computador") |

## 3. Proposta para cada caso (direção do briefing: a regra da D-14)

Regra geral: **a sessão de outro computador nunca muda por uma operação feita aqui**. Ela é tratada como a fechada da D-14: o que foi feito aqui fica aqui.

| Caso | Proposta |
|---|---|
| Venda cancelada aqui, gaveta da venda aberta em outro computador | Igual à D-14: marca a venda como "cancelada fora da sessão" (a marca da migração 22, com nome mais geral), o resumo da sessão de lá não muda; o dinheiro em espécie sai da gaveta aberta **deste** computador (sangria ligada à venda) ou, sem gaveta aberta, do Caixa geral |
| Recebimento estornado aqui, gaveta aberta em outro computador | idem (sangria aqui ou Caixa geral) |
| Devolução desfeita aqui, gaveta aberta em outro computador | idem, ao contrário (suprimento aqui ou Caixa geral) |
| Pagamento estornado aqui que saiu da gaveta de outro computador | o dinheiro entra na gaveta aberta **deste** computador (suprimento ligado ao pagamento); sem gaveta aberta, Caixa geral — igual à D-14 |
| Mesma operação feita no próprio computador da gaveta aberta | como hoje |
| Pix e cartão | como hoje (não mexem em gaveta) |
| Gaveta deste computador sem dinheiro bastante | recusa tudo, nada muda (como a D-14) |

Alternativa mais restritiva (a outra opção da auditoria da 0.9.14): só deixar cancelar/estornar no computador da gaveta aberta. **Não recomendada**: bloqueia cancelamentos legítimos quando aquele computador está desligado ou sem rede (o briefing pede para não bloquear cancelamentos legítimos sem tratamento seguro).

Detalhe que precisa ficar claro na tela: a auditoria e o recibo dizem "o dinheiro saiu da gaveta do Caixa X (este computador)", e o fechamento do outro computador mostra a operação como "cancelada em outro caixa, sem mexer nesta gaveta".

## 4. 0915-L — cancelamentos em cascata sem `exigir_editavel`

Os caminhos diretos (estornar recebimento, alterar/pagar/cancelar/estornar conta a pagar, cheques, desfazer devolução, compra) chamam `sync::exigir_editavel`, que bloqueia mexer num registro que espera um pai ainda não recebido ou que está inconsistente. Os caminhos em cascata não chamam:

| Caminho | Quem chama | O que pode acontecer hoje |
|---|---|---|
| `receber::cancelar_da_venda` | cancelar venda | cancela parcelas do crediário que estão esperando a troca de dados (recusa se alguma já foi paga) |
| `pagar::cancelar_da_compra` | estornar compra | cancela contas a pagar que estão esperando a troca |
| `cheques::cancelar_de` | cancelar venda / estornar recebimento | cancela cheque que está esperando a troca |
| abatimento do crediário em `devolucoes::devolver` | devolução | abate parcela que está esperando a troca |

Proposta: na cascata, conferir os filhos **antes** de mudar qualquer coisa (na mesma transação):
- filho **inconsistente** (o pai sumiu de todos os caixas): recusa a operação inteira com a mensagem de hoje ("resolva a diferença em Simultaneidade antes") — igual ao caminho direto;
- filho **esperando** (o pai ainda vai chegar): também recusa, dizendo qual registro espera e que a operação poderá ser feita quando a troca de dados terminar;
- nenhum filho bloqueado: como hoje.

Impacto: uma venda ou compra cuja parcela/conta/cheque ainda está esperando a troca de dados passa a não poder ser cancelada até a troca terminar (hoje passa). Isso troca um risco (cancelar algo cujo estado real ainda não chegou) por uma espera; o gerente vê o motivo. **Precisa da sua decisão**, porque pode travar cancelamentos que hoje passam.

## 5. Testes que a implementação terá de ter (R-02 e 0915-L)
- Dois caixas, gaveta aberta no outro: cancelar venda, estornar recebimento, desfazer devolução, estornar pagamento; conferir os dois fechamentos e o Caixa geral; repetir a operação (recusada); com e sem gaveta aberta aqui; Pix/cartão.
- Rede desconectada e troca depois: o outro computador recebe a operação e o fechamento dele não muda.
- Reabrir os programas no meio.
- Cascata com filho esperando, inconsistente e normal, em cada um dos 4 caminhos; nada muda quando recusa.
- As regras preservadas: duas vendas iguais continuam duas; idempotência pela chave da operação; D-14 continua.

## 6. Decisões do dono necessárias
1. R-02: aprovar a regra da seção 3 (a outra sessão nunca muda; o dinheiro fica aqui ou no Caixa geral).
2. Estorno de pagamento: passar a usar a gaveta aberta deste computador (hoje vai para o Caixa geral).
3. 0915-L: aprovar a recusa em cascata com filho esperando ou inconsistente (seção 4).
4. R-03: a detecção foi feita; decidir se, além de avisar, o Rustinho deve oferecer uma compensação pronta (por exemplo, "devolver ao cliente" como saída da gaveta), ou se o estorno pela tela de cada coisa basta.
