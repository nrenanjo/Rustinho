# Auditoria técnica do núcleo — 0.9.14 (21/10/2026)

Feita **antes** de preparar o Rustinho portátil para Windows. Nenhuma função
nova de negócio entrou; a parte fiscal ficou para depois. Regras de negócio
só mudaram onde o dono decidiu (D-14).

- **Ponto de partida:** commit `7d17112` (0.9.13 + D-09), ramo `versao-0.9.11`.
- **O que foi olhado:** o núcleo inteiro (`nucleo/src`, 44 mil linhas), com
  leitura linha a linha das partes de dinheiro (venda, caixa, crediário,
  contas a pagar, contas da loja, devolução, compra, estoque), da porta de
  entrada e das permissões (`api.rs`), da troca de dados (`sync.rs`), do
  backup e do banco; na interface, a ponte (`app/src`) e as chamadas que
  gravam (`TelaVender`, `TelaReceber`, `TelaCaixa`, `TelaVenda`).
- **Como:** leitura dirigida; para cada suspeita, um teste-sonda que prova o
  defeito **antes** de corrigir; depois da correção, cada parte desligada de
  propósito para conferir que o teste falha (mutação); `cargo audit`
  (bibliotecas com aviso de segurança); telas de verdade no X virtual.
- **Fora:** fiscal (NF-e/NFC-e), Android, Bluetooth.

## 1. Defeitos confirmados (e corrigidos)

| Nº | Prioridade | O que acontecia | Prova | Correção |
|---|---|---|---|---|
| D-14 | Alta | Cancelar uma venda, estornar um recebimento ou desfazer uma devolução de um caixa **já fechado** mudava o fechamento daquele dia (fechou certo com R$ 150,00 → depois mostrava "sobrou R$ 100,00"), e o dinheiro devolvido hoje não aparecia em gaveta nenhuma (ia faltar no fechamento de hoje). | 3 sondas: o fechamento de ontem passou de (esperado 150,00; diferença 0) para (50,00; +100,00), (50,00; +40,00) e (250,00; −100,00). | Decisão do dono: o caixa fechado nunca muda; o dinheiro sai/entra na gaveta aberta deste computador (sangria/suprimento ligado à operação) ou, sem caixa aberto, no Caixa geral ("Acerto de caixa já fechado"); sem movimento duplicado; auditoria. Migração 22 só acrescenta colunas. |
| D-16 | Média | Na troca de dados, uma configuração **só deste computador** que chegasse pela rede (a identidade do caixa, a senha da rede) era gravada; na saída os caixas só mandam as da loja, mas na chegada nada era conferido. Só um caixa com a senha da rede consegue (veja R-01). | Sonda: a identidade do caixa virou "invasor". | Na chegada, só as configurações da loja. |
| D-15 | Baixa | Na compra que muda o custo sem atualizar o preço, o % de lucro era refeito por uma conta do banco que arredonda errado quando o preço fica abaixo do custo (−19,99% em vez de −20,00%). | Sonda `d15_lucro_abaixo_do_custo`. | Usa a mesma conta do cadastro. |

Detalhes, testes e o que ficou de fora em `docs/DEFEITOS.md` (D-14, D-15, D-16).

## 2. Riscos que precisam de investigação ou decisão

Nenhum destes foi mudado: dependem de decisão do dono ou de teste que só dá
para fazer fora daqui.

| Nº | Assunto | O que pode acontecer | Recomendação |
|---|---|---|---|
| R-01 | Senha da rede (troca de dados) | Pode ter 4 caracteres (decisão da 0.9.13). Quem estiver na rede da loja e capturar um anúncio de um caixa consegue testar todas as senhas de 4 números fora da loja, no computador dele (Argon2id, mas são só 10 mil). Com ela, lê tudo o que os caixas trocam (clientes, vendas, os resumos das senhas) e pode mandar alterações. | Exigir 10 caracteres ou mais **só** para a senha da rede (não para os usuários). |
| R-02 | Cancelar uma venda de um caixa **aberto em outro computador** | A venda sai do total daquele caixa (que ainda está aberto) e o dinheiro sai deste: os dois fechamentos ficam errados. O mesmo para estorno e devolução desfeita. A D-14 trata só o caixa já fechado, como decidido. | Mesmo tratamento da D-14 (o dinheiro fica onde a operação foi feita), ou só deixar cancelar no próprio caixa. |
| R-03 | Dois caixas sem rede recebem (ou pagam) a mesma parcela | Depois da troca de dados, a parcela fica paga a mais e sai das listas sem aviso. | Aviso na pílula Gerente ("parcela paga a mais"), como o do orçamento vendido duas vezes. |
| R-04 | Configurações que chegam pela troca de dados | Confirmado e corrigido (D-16). | — |
| R-05 | Tarefas da troca de dados | Se uma delas parar por defeito, a troca fica parada até fechar e abrir o Rustinho; só uma linha no registro, nenhum aviso na tela. | Reiniciar a tarefa sozinha e avisar no Início. |
| R-06 | Senha de entrada com 4 números | 5 tentativas a cada 30 s: todas as 10 mil em cerca de 17 horas, no computador da loja. | Já decidido (0.9.13); fica registrado. |
| R-07 | Biblioteca `zip` 2.4.2 | `cargo audit` acusa 3 avisos (RUSTSEC-2026-0367, -0368, -0369; um crítico). Todos são da extração para pasta (`extract`), que o Rustinho **não usa** (lê cada arquivo pelo nome, na memória): hoje não afeta. | Atualizar para a `zip` 9 numa etapa própria (mudança grande de versão). |
| R-08 | Permissões da pasta no Windows | No Linux a pasta dos dados é só do usuário (700/600); no Windows não se mexe. `%LOCALAPPDATA%` já é do usuário, mas a pasta portátil fica com as permissões de onde for extraída. | O LEIA-ME pede uma pasta do próprio usuário; tratar junto com a PEND-13. |
| R-09 | Duplo clique ou Enter repetido em "Concluir venda" e "Receber" | Pela leitura do código, a tela fica parada até a resposta e o carrinho é limpo, então o segundo clique cai na tela vazia. Não há teste automático disso na tela de verdade. | Teste de tela e passo no roteiro. |
| R-10 | Arquivo do banco estragado | Não há conferência (`quick_check`) ao abrir nem na cópia do backup (só ao restaurar). Um banco estragado por disco ou falta de luz só aparece quando algo falha. | Conferir a cópia antes de guardar o backup e avisar no Início. |
| R-11 | Rede no Windows (PEND-13) | Prender a conexão à placa de rede e saber qual é cabo ou Wi-Fi ainda não existe no Windows. | Validar com dois computadores (FV-1, FV-6, FV-9, FV-12). |
| R-12 | Descartar o rascunho de uma compra | Apaga também as linhas de auditoria do rascunho (a importação some do histórico). | Manter a auditoria. |

## 3. Melhorias recomendadas (com justificativa, sem urgência)

| Nº | O quê | Por quê |
|---|---|---|
| M-01 | Dividir `api.rs` (3,8 mil linhas) e `sync.rs` (4,8 mil) por assunto. | Manutenção: hoje um comando novo mexe num arquivo enorme. As listas de permissão por nome continuam protegidas pelo teste `todo_comando_tem_permissao`. |
| M-02 | Uma função só para o "próximo número" (venda, orçamento, devolução, vale, recebimento: hoje são 4 cópias de `MAX + 1`). | Duplicação. |
| M-03 | Uma leitura só dos valores de pagamento em `vendas::calcular` (hoje 3 cópias). | Duplicação: uma correção numa cópia não chega às outras. |
| M-04 | Comandos que gravam mais de uma coisa fora de transação (`computador_salvar`, `unidade_salvar`) e auditoria gravada depois do `commit` (`senha_trocar`, `backup_restaurar`). | Se cair no meio, fica meio gravado (só configuração e histórico: não é dinheiro). |
| M-05 | As tarefas da troca de dados abrem uma conexão nova com o banco a cada volta (a cada 1 e 4 segundos). | Desperdício (não é defeito). |
| M-06 | Conferência do banco (`quick_check`) na cópia do backup. | R-10. |

## 4. Testes que ainda faltam

- **Sem teste de unidade:** `caixa.rs`, `pagar.rs`, `comissoes.rs`, `estoque.rs`, `config.rs` (cobertos em parte pelos testes de estabilidade); `cheques.rs` tem 1, `oficina.rs` 1, `encomendas.rs` 2.
- **Sem teste nenhum:** depositar, repassar e devolver cheque com desfazer; transferência entre contas e conferência de saldo; reabrir comissão; entregar encomenda; fluxo de caixa com cheques pré-datados; venda com vale-troca + dinheiro + troco.
- **Telas:** só a D-09, a D-14 e o portátil têm teste automático na tela de verdade; o resto é o roteiro à mão. Duplo clique (R-09).
- **Troca de dados com dois computadores de verdade** (FV-1 a FV-13) e no Windows (PEND-13).
- **Banco grande** (anos de vendas): tempo para abrir, fazer backup e trocar dados.
- **Relógio errado** no fechamento do caixa e na virada do dia.
- **Windows:** os testes do núcleo rodaram no Windows pela primeira vez nesta etapa (resultado abaixo).

## 5. Critérios objetivos para chamar o núcleo de maduro

1. Nenhum defeito aberto de prioridade alta ou média em `docs/DEFEITOS.md`.
2. `cargo test --workspace`, `cargo fmt --check` e `cargo clippy` limpos no Linux **e** no Windows, também no modo do programa final (`--release`).
3. Toda regra de dinheiro e de permissão com teste que **falha** quando a regra é desligada (já feito para D-09 e D-14).
4. Roteiro à mão inteiro (partes A a L) aprovado no Linux e no Windows 10/11, com as anotações.
5. FV-1 a FV-13 aprovados com dois computadores (o mínimo: FV-1, FV-3, FV-4, FV-8 e FV-10).
6. DRE conferida pelo dono e pelo contador (PEND-20).
7. Importação de uma **cópia** da exportação real do SINTEC, com os totais conferidos.
8. 30 dias de uso em paralelo na loja (Rustinho e SINTEC) sem diferença de caixa, estoque ou crediário que não tenha explicação.
9. R-01 a R-03 decididos (corrigidos ou aceitos por escrito).
10. `cargo audit` sem aviso que afete o Rustinho.

Hoje os itens 1 (só baixa/decisão abertas: nenhuma alta ou média), 3 (para
as regras testadas) e 10 (avisos que não afetam) estão atendidos; os outros
não. **O Rustinho não é estável ainda.**

## 6. O que foi executado (resultados reais)

Ver `docs/TESTES-estabilidade.md` (seção 0.9.14) e os registros entregues
com esta etapa.
