# Revisão comparativa Rustinho × backlog do Sintec NF-e 4 — triagem

Data: 22/10/2026. Fonte: PDF "RenaSoft | Backlog do Sintec NF-e 4" (62 páginas, 1.010 itens: cerca de 760 não fiscais e cerca de 250 fiscais ou bancários).
Método: leitura do código e dos testes. **Nada foi executado nem alterado.** "Tratado" quer dizer que o código e o teste existente cobrem o caso, sem ter rodado os testes nesta etapa.
Defeitos confirmados: **0** (confirmar exige sonda que falha; fica para a etapa seguinte).

## Itens analisados

| Situação no Sintec | O que o Rustinho faz (código) | Teste existente | Lacuna | Classe | Ação |
|---|---|---|---|---|---|
| Receber parcial de várias contas marca todas como pagas, ou parcial mais de uma vez | `receber.rs` `calcular` abate parcela por parcela até acabar o valor | `t3_1_parciais_e_quitacao` | nenhuma | Tratado | Nenhuma |
| Desconto negativo no receber | Recusa desconto negativo e desconto maior que a dívida | `limites_ao_pagar_e_receber` | nenhuma | Tratado | Nenhuma |
| Juros: "zerar manualmente", rotina de inserir juros | Juros editáveis, não negativos; o valor recebido tem que cobrir os juros | `t3_2_atraso_multa_e_juros`, `meses_e_juros` | nenhuma | Tratado | Nenhuma |
| Troco: subtrair das parcelas, valor errado | `vendas.rs`: troco só se houver dinheiro suficiente; no receber, só em dinheiro | `calcula_descontos_estoque_e_troco` | falta caso de crediário misturado com dinheiro e troco | Tratado, teste ausente | Escrever o teste (Opus) |
| Arredondamento das parcelas | `parcelar()`: a sobra de centavos vai na primeira parcela; a soma fecha | só indireto (`crediario_do_comeco_ao_fim`) | falta teste "soma das parcelas = valor" para vários valores e quantidades | Tratado, teste ausente | Escrever o teste (Opus) |
| Venda sem forma de pagamento ou com valor zerado | `concluir` exige pagamento e preço maior que zero | `valores_extremos_sao_recusados_sem_gravar` | nenhuma | Tratado | Nenhuma |
| Desconto errado, margem de desconto | Desconto máximo por peça; valida desconto maior que o item e que o total | `d09_7_preco_padrao_e_desconto_na_venda` | nenhuma | Tratado | Nenhuma |
| Estoque insuficiente | Avisa "falta estoque" e permite vender (saldo negativo); item pode ser encomendado | `t2_4_vender_mais_do_que_tem` | nenhuma (documentado) | Regra de negócio | Nenhuma |
| Estoque ao cancelar, devolver ou reabrir venda | Devolve no cancelamento e na devolução; reabrir venda não existe | `t2_1`, `t3_4` | nenhuma | Tratado | Nenhuma |
| Vendas simultâneas, estoque duplicado | Transação por comando e troca de dados entre caixas | `t2_5_vendas_simultaneas...` | nenhuma | Tratado | Nenhuma |
| Dashboard e Curva ABC contando venda cancelada | Consultas de painel, DRE e reposição filtram `situacao = 'concluida'` | `t4_*` | nenhuma | Tratado | Nenhuma |
| Lançamento de venda duplicado no caixa | D-14: um único movimento, vinculado à operação | `d14_*` | nenhuma | Tratado | Nenhuma |
| Duplo clique / duplo envio ("valores duplicados no fechamento") | Sem trava no servidor; a tela depende de a chamada ser síncrona e de a janela fechar | nenhum | Na venda, a 2ª ordem falha (carrinho vazio). No receber, 2ª ordem com contas "todas" e mesmo valor poderia baixar as parcelas seguintes | **Risco** | Teste na tela; se confirmar, trava no servidor (Opus) |
| Limite de crédito do cliente | Só avisa (`avisos_venda`); não bloqueia | nenhum | O Sintec passou a bloquear | **Decisão do dono** | Decidir: avisar ou bloquear |
| Quem pode dar desconto ou perdoar juros no receber | `receber_baixar` é do caixa, sem limite nem liberação do gerente; dá para quitar a dívida toda com desconto, recebendo R$ 0 | `limites_ao_pagar_e_receber` não cobre | A linha de auditoria não mostra desconto nem juros (só o valor); ficam só no registro do recebimento | **Risco / decisão do dono** | Decidir se exige liberação do gerente; ajuste de auditoria é pequeno |
| Permissões de módulo, bloqueio do preço de custo | Cada comando conferido em `api.rs` (caixa x gerente) | `t5_1..6`, `d09_1..9` | nenhuma | Tratado | Nenhuma |
| Excluir venda com financeiro lançado | Não há exclusão, só cancelamento; parcela paga impede o cancelamento | `t3_4` | nenhuma | Tratado | Nenhuma |
| Crédito do cliente e vale-troca (parcial, estorno) | `saldo_vale` por view; vale parcial permitido; estorno de recebimento | `devolve_com_vale...`, `abate_do_crediario...` | nenhuma | Tratado | Nenhuma |
| Cadastros: CNPJ alfanumérico, placa sem máscara, busca por nome, código de barras e código repetidos | Implementados | `confere_cnpj_numerico_e_alfanumerico`, `confere_placas`, `nomes_unicos...` | nenhuma | Tratado | Nenhuma |
| Importação de dados | Importa a partir de uma cópia | `t7_1..3` | falta a cópia real do SINTEC (pendência conhecida) | Já pendente | Teste real |
| Peso no código de barras da balança | Não existe essa função | — | — | Não se aplica | Só se o dono quiser |

## Comparação com a auditoria 0.9.14
- R-03 (parcela paga duas vezes por dois caixas offline) segue aberto; é do mesmo tipo do risco de duplo envio.
- R-01 e R-02 não aparecem no changelog.

## Lista fiscal para etapa futura (nada implementado)
- Notas: NF-e (cerca de 150 itens), NFC-e, NFS-e e DANFSe nacional, SAT, MDF-e, CT-e.
- Tributação: CST, CFOP, ICMS, IPI, PIS, COFINS, NCM, NBS, CEST, IBS/CBS (reforma tributária), tabela IBPT.
- Infraestrutura: certificado digital, SEFAZ, contingência, SPED, Sintegra.
- Meios de pagamento da SEFAZ e campo `cAut` da operadora de cartão.
- Bancário (cerca de 50 itens: boleto, CNAB, OFX, conciliação): não é fiscal, mas é outro módulo, fora desta revisão.

### Avisos do Sintec para a etapa fiscal (22/10/2026)
- Reforma Tributária adiada (Ato Conjunto RFB/CGIBS nº 1, de 22/12/2025): em 2026 a apuração de IBS e CBS é só informativa, sem efeito tributário; não há penalidade por deixar os campos em branco no início. Regras e prazos ainda serão definidos: manter a lógica fiscal separada e fácil de atualizar.
- Regime normal: validar o CClasTrib dos produtos com o contador. A loja marca CSOSN 102 (Simples Nacional): perguntar ao contador se isso se aplica.
- Fim do SAT em São Paulo desde 01/01/2026: o Rustinho não terá SAT; o foco de balcão é a NFC-e.
- Pendente: certificado A3 (e-CNPJ em cartão com leitor USB) e conversa com o contador.

## Próxima etapa (Opus 5.5, esforço alto)
1. Teste na tela do duplo clique no receber; se confirmar, correção.
2. Testes da soma das parcelas e do crediário com troco.
3. Depende de decisão do dono: limite de crédito e desconto do caixa no receber.
