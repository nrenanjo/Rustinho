# Migrações do banco de dados

Regras para mudar a estrutura do banco do Rustinho. Escrito na dev.3.3 (passo 1.6, AUD33-ARQ-15 e
AUD33-N01; decisões DP-47 revista e DP-50). Atualize este documento a cada migração nova.

## 1. Como funciona

- Cada arquivo `nucleo/src/migracoes/NNNN_assunto.sql` é uma versão da estrutura. A lista
  `MIGRACOES` (`nucleo/src/dados/banco.rs`) inclui os arquivos na ordem dos números.
- O banco guarda **só a contagem** de migrações aplicadas (`PRAGMA user_version`). Não guarda qual
  linha de versões as aplicou.
- Ao abrir (`Banco::abrir`):
  1. um banco de estrutura anterior é conferido, copiado e atualizado; se algo der errado, volta da
     cópia (`migracao::atualizar_com_copia`);
  2. `banco::migrar` aplica as migrações que faltam;
  3. um número **maior** que o conhecido é recusado ("versão mais nova").
- A migração que recria tabelas começa com a linha `-- rustinho: sem_chaves_estrangeiras` (hoje só
  a 0023). Ela roda com as chaves estrangeiras desligadas e `legacy_alter_table`, e confere
  contagens e referências antes do COMMIT.
- `banco::conferir_estrutura` confere o que as migrações recentes deixam, depois de atualizar um
  banco antigo.
- **Nenhuma migração cria gatilho.** Os gatilhos `sync_*` são criados pelo programa ao abrir
  (`sync::criar_gatilhos`).

## 2. Migrações publicadas desta linha

| Nº | Arquivo | Conteúdo | Observação |
|---|---|---|---|
| 0001 | `0001_inicial.sql` | estrutura inicial (Etapa 1) | |
| 0002 | `0002_pessoas_compras_fiscal.sql` | clientes, fornecedores, compras pelo XML, regras fiscais | |
| 0003 | `0003_vendas_caixa_sync.sql` | vendas, orçamentos, caixa, troca de dados entre caixas | redefiniu 4 índices com o mesmo nome (`produto_codigo`, `pessoa_codigo`, `pessoa_documento`, `compra_chave`): história, não se repete (§6) |
| 0004 | `0004_crediario.sql` | crediário, convênio, contas a receber | |
| 0005 | `0005_financeiro.sql` | contas a pagar, contas da loja, cheques | |
| 0006 | `0006_encomendas.sql` | encomendas | |
| 0007 | `0007_devolucoes.sql` | trocas, devoluções, vale-troca | |
| 0008 | `0008_oficina.sql` | ordem de serviço da oficina | |
| 0009 | `0009_cadastro.sql` | unidades, serviço, oferta, tamanhos/cores, fiscal completo (0.9.1) | |
| 0010 | `0010_gerente.sql` | gerente e caixa (0.9.11) | |
| 0011 | `0011_backup_conflitos.sql` | backup e conflitos (0.9.11) | |
| 0012 | `0012_historia_das_alteracoes.sql` | troca de dados sem depender do relógio (R-08) | |
| 0013 | `0013_faixas_de_codigo.sql` | código interno das peças sem repetir entre caixas | |
| 0014 | `0014_codigo_confirmado.sql` | código com letras só vale depois de confirmado | |
| 0015 | `0015_codigo_das_pessoas.sql` | código de clientes e fornecedores sem repetir | |
| 0016 | `0016_codigo_das_pessoas_confirmado.sql` | código com letras de pessoa confirmado (PEND-07) | |
| 0017 | `0017_referencias_com_prova.sql` | atraso de sincronização não é inconsistência (R-05) | |
| 0018 | `0018_situacao_da_senha.sql` | situação da senha diante da política (F-17) | |
| 0019 | `0019_caminhos_da_rede.sql` | caminhos até cada outro caixa (PEND-10) | |
| 0020 | `0020_verificacao_da_nfe.sql` | verificação da assinatura do XML da NF-e (PEND-09) | |
| 0021 | `0021_dre_gerencial.sql` | DRE gerencial e indicadores | |
| 0022 | `0022_caixa_fechado.sql` | caixa fechado não muda mais (D-14, 0.9.14) | |
| 0023 | `0023_rascunho_e_pendencias.sql` | rascunho da OS e pendências da troca de dados (0.9.15) | recria tabelas (`sem_chaves_estrangeiras`) |
| 0024 | `0024_gtins_e_pendencias_xml.sql` | vários GTINs por peça e pendências de identificação do XML (0.9.18-dev.3.1) | **publicada**; a dev.3 paralela tem outra 0024 (§3) |
| 0025 | `0025_caixas_aprovados.sql` | registro assinado de caixas aprovados e retirados (protocolo 4, etapa 2; era a 0024 da `frente-p4`) | **não publicada**; número provisório até entrar na branch (§5). **Não é a 0025 da dev.3** (`venda_cancelamento`, §3): a linhagem (§4) recusa o banco dev.3/25 |
| 0026 | `0026_indices_e_busca_normalizada.sql` | passo 7.5: o índice composto do estoque com nome novo (B01) | **não publicada**; número provisório até entrar na branch (§5). Era a 0025 da preparação `prod-f75`, renumerada na integração (MIG-1 (a): quem entra depois renumera) |

Um número publicado **nunca muda de conteúdo**.

## 3. A linha paralela dev.3: não é esta estrutura

A tag `v0.9.18-dev.3` (`8cb29af`) e a linha dev.3.1/dev.3.2 saem do mesmo commit (`67d006f`) e
seguem separadas. A dev.3 tem migrações próprias com números que esta linha também usa:

| Nº | Arquivo da dev.3 | O que deixa diferente da estrutura desta linha |
|---|---|---|
| 0024 | `0024_identificadores_produto.sql` | `produto_gtin` com outra definição (chave `produto_id, gtin`, sem `tipo`/`origem`); índice `produto_gtin_codigo` com outra definição; tabela `produto_fornecedor_alternativo` e índice `produto_fornecedor_alternativo_produto`; colunas `compra_item.gtin_tributavel`, `codigo_barra`, `codigo_barra_tributavel`; colunas `produto.fornecedor_principal_id` e `fornecedor_principal_codigo`; **não** tem `xml_pendencia_identificacao` |
| 0025 | `0025_cancelamento_idempotente.sql` | tabela `venda_cancelamento` |

O que os testes mediram em `4b253c6`, com as amostras de `nucleo/tests/amostras_dev3/`:

- um banco dev.3 na estrutura 24 é **aceito** como a 24 desta linha, e o arquivo é alterado;
- um banco dev.3 na estrutura 25 é recusado só pelo número, como "versão mais nova";
- com **qualquer** 0025 desta linha, o mesmo banco dev.3/25 passa a ser **aceito** e alterado
  (verificado numa cópia descartável com uma 0025 de teste).

Nenhum banco da linha dev.3 com dados de loja foi encontrado (investigação de 10/11:
`_reversa_sdd/auditoria_dev33/investigacao_dev3/`). O número sozinho não protege. Quem protege é a
regra do §4.

## 4. Regra de linhagem (DP-50)

1. **Estrutura esperada para cada número**, gerada das próprias migrações: aplicar os `.sql`
   0001..N em ordem num banco vazio, como `aplicar_migracao` (é o que
   `nucleo/tests/migracoes_linhagem.rs` faz em `estrutura_esperada`). Entram tabelas, índices e
   visões, com a definição de `sqlite_master`. Os gatilhos ficam de fora, porque o programa os cria
   ao abrir.
2. **Antes de qualquer migração e de qualquer gravação**, comparar a estrutura real do banco com a
   esperada para o `user_version` dele.
3. **Divergiu: recusar sem gravar.** O SHA-256 do arquivo tem de ficar igual. A mensagem diz que o
   banco é de outra linha de versões ou de estrutura desconhecida (não "versão mais nova"), e o
   diagnóstico registra a recusa.
4. Bancos desta linha, de qualquer número publicado, continuam abrindo e atualizando normalmente.

**Situação: implementada no passo 2.4** (`banco::conferir_linhagem`, chamada no começo de
`Banco::abrir`):
- a conferência abre o arquivo **só para leitura**, antes da cópia, da migração e até do diário WAL;
- ela compara a **forma** da estrutura, pelo conteúdo, não o texto do `CREATE` de `sqlite_master`,
  que versões diferentes do SQLite reescrevem. A forma é: as colunas de cada tabela (nome, tipo, não
  nulo, posição na chave), as colunas e a unicidade de cada índice, e as visões. Assim um banco
  legítimo, atualizado ao longo do tempo por Rustinhos antigos, não é recusado por diferença de
  texto;
- **conferido no passo 2.4**:
  - as migrações de todas as versões desta linha que estão no repositório (v0.9.1, v0.9.14, a base
    0.9.17, `main`, dev, dev.2, dev.3.1, dev.3.2) deixam exatamente a forma esperada de hoje;
  - a linha dev.3 não deixa;
- número maior que o desta versão: se o banco tem tudo o que esta versão conhece, igual, é "versão
  mais nova" (desta linha); senão, é de outra linha;
- a mensagem da recusa é `banco::TEXTO_OUTRA_LINHA`, e o registro técnico anota as diferenças;
- **a primeira 0025 desta linha pode entrar** (a 2.4 está feita).

## 5. Ordem prevista das migrações da dev.3.3 (DP-47 revista)

A reserva de 05/11 é substituída: ela previa uma 0024 para os caixas aprovados (`frente-p4`), mas a
0024 já foi publicada com os GTINs. A numeração continua da 0025, na ordem em que cada migração
**entra** na branch. O número só fica definitivo ao entrar, e então este documento é atualizado.

| Conteúdo | Passo | Observação |
|---|---|---|
| M-A: pendências: tipo de resolução, motivo, quem resolveu ou descartou, chaves de NF-e | 4.5 | só colunas novas; depois da 2.4 |
| M-B: decisão de identificação por chave natural | 5.3 | conforme DP-41/42 |
| M-C: caixas aprovados e retirados (era a 0024 da `frente-p4`) | 5.5 | **preparada** como `0025_caixas_aprovados.sql` (ramo de preparação `prod-p4`, 13/11) |
| M-D: demais registros do protocolo 4 e da recuperação, se precisarem de banco | 5.5 | conforme DP-44 |
| M-E: índices de desempenho (inclui o composto da 0006, com nome novo) e o nome sem acento gravado | 7.5 | **preparada** como `0026_indices_e_busca_normalizada.sql` (no ramo `prod-f75` era a 0025; renumerada na integração, 13/11, porque a 0025 do protocolo 4 entrou antes) |
| M-F: nome e código gravados no item da OS, se o item não os tiver | 6.5 | só se preciso |

O número sai **no commit**, como o próximo livre (MIG-1 (a), `PLANO_MIGRACOES.md`): a tabela é de
conteúdos, não de números. Dois ramos com o mesmo número novo: quem entra depois renumera (nome do
arquivo, linha em `MIGRACOES`, bloco em `conferir_estrutura`, sentinela e o teste da migração),
antes da publicação. Hoje a M-C e a M-E estão preparadas, cada uma, como 0025: a ordem em que a 5.5
e a 7.5 entrarem decide quem fica com a 0025.

## 6. Checklist de uma migração nova

1. Próximo número, sem buraco nem repetição: arquivo `NNNN_assunto.sql`, no fim de `MIGRACOES`.
2. **Só aditiva:** colunas, tabelas e índices novos. Recriar tabela só com a primeira linha
   `-- rustinho: sem_chaves_estrangeiras`.
3. **Nunca reaproveitar nome de índice.** Não vale apagar e criar de novo com outra definição: dê
   outro nome. E nunca `CREATE INDEX IF NOT EXISTS` com um nome que já existe: o SQLite não faz
   nada e não avisa (foi assim que o índice composto do estoque da 0006 nunca existiu, B01/B02).
4. **Nunca usar nome da linha dev.3:** `produto_fornecedor_alternativo`,
   `produto_fornecedor_alternativo_produto`, `venda_cancelamento` (e qualquer objeto das amostras
   dev.3).
5. Bloco em `banco::conferir_estrutura` para o que a migração deixa.
6. Teste "não muda o que existia" (dados e contagens iguais) e teste de restaurar um backup antigo.
7. Sentinela: `assert_eq!(MIGRACOES.len(), N)` no teste do `banco.rs`.
8. Atualizar este documento: §2 (publicadas) e §5 (ordem prevista).
9. Rodar `cargo test -p rustinho-nucleo --test migracoes_linhagem`. A numeração e os itens 3 e 4
   são conferidos automaticamente (`numeracao_das_migracoes_continua_e_igual_a_lista_do_banco`,
   `nenhuma_migracao_reaproveita_nome`, `b02_nenhum_indice_if_not_exists_ignorado`).
10. Testar só com bancos sintéticos ou com as amostras. **Nunca migrar banco real para testar.**

## 7. Testes

`nucleo/tests/migracoes_linhagem.rs`:

| Teste | Tipo | O que garante |
|---|---|---|
| `estrutura_esperada_e_a_do_programa` | trava | a estrutura gerada dos `.sql` é a que o programa deixa num banco novo; nenhuma migração cria gatilho |
| `amostra_dev32_e_a_estrutura_24_desta_linha` | trava | a amostra dev.3.2/24 bate com a 24 gerada |
| `amostras_dev3_nao_sao_a_estrutura_desta_linha` | trava | as amostras dev.3/24 e dev.3/25 diferem nos pontos do §3 |
| `banco_dev32_estrutura24_abre_normalmente` | trava | abre (e atualiza) sem perder dados |
| `banco_na_estrutura_23_atualiza_para_a_atual` | trava | 0.9.15 a 0.9.17 atualizam |
| `numeracao_das_migracoes_continua_e_igual_a_lista_do_banco` | trava | §6, item 1; a 0024 é a desta linha |
| `nenhuma_migracao_reaproveita_nome` | trava | §6, itens 3 e 4 |
| `b02_nenhum_indice_if_not_exists_ignorado` | trava (passo 7.5) | §6, item 3: nenhum `IF NOT EXISTS` ignorado; o único da história (0006) tem o índice que o corrige |
| `b02_a_trava_pega_um_if_not_exists_ignorado` | trava (passo 7.5) | a trava acima acusa um caso de exemplo |
| `b01_indice_composto_do_estoque_com_nome_novo` | trava (passo 7.5) | o índice `(produto_id, tipo, criado_em)` existe num banco novo e num atualizado |
| `b01_parados_usa_o_indice_composto` | trava (passo 7.5) | o plano da conta dos parados usa o índice |
| `banco::testes::m26_nao_muda_o_que_existia` | trava (passo 7.5) | §6, item 6: a 25 → 26 (e a 24 → 26) não muda as tabelas da loja nem grava no log da troca |
| `banco::testes::m26_backup_de_estrutura_anterior_restaura_e_atualiza` | trava (passo 7.5) | §6, item 6: backup da estrutura 25 (e da 24) restaura e atualiza para a 26 |
| `banco_dev3_estrutura24_e_recusado_sem_gravar` | trava (era caracterização até a 2.4) | §4 |
| `banco_dev3_estrutura25_e_recusado_como_outra_linha` | trava (era caracterização até a 2.4) | §4 |
| `banco_de_estrutura_desconhecida_e_recusado_sem_gravar` | trava (era caracterização até a 2.4) | §4 |
| `banco::testes::n01_todas_as_estruturas_desta_linha_passam` | trava | §4, item 4: nenhuma estrutura desta linha (1 a N) é recusada |
| `banco::testes::n01_banco_de_um_rustinho_mais_novo_desta_linha` | trava | número maior, com tudo igual: "versão mais nova" |

As amostras ficam em `nucleo/tests/amostras_dev3/`. Veja o LEIA-ME de lá: origem, cópias limpas e
SHA-256.
