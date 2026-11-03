# BLOCO 8A — Auditoria de Código: API Central, Despacho de Comandos e Autorização do Gerente

**Data da auditoria:** 2026-11-02
**Escopo:** Revisão independente integral de `nucleo/src/api.rs` (`1–4550`, incluindo os 32 testes embutidos), confrontando contra os relatórios históricos #18 e #19 do Gemini 4 Argon (`RELATORIO_AUDITORIA_BLOCO_8A_GEMINI_4_ARGON.md` e `RETIFICACAO_BLOCO_8A_GEMINI_4_ARGON.md`) e os contratos documentais dos Blocos 1 a 5.
**Restrição de imutabilidade:** Nenhum arquivo do código-fonte ou documentação preexistente foi modificado.

---

## 1. Arquivo Lido Integralmente no Workspace

| # | Arquivo | Linhas no Workspace | Linhas no Gemini 4 (Bloco 8A) | Status de Leitura |
|---|---|---|---|---|
| 1 | `nucleo/src/api.rs` | `1–4550` (4.550 linhas) | 4.550 linhas | 100% lido |

---

## 2. Arquitetura de Despacho, Permissões, Idempotência e Resiliência em `nucleo/src/api.rs`

1. **Classificação Exaustiva de Comandos (`linhas 36–525`, testada por `todo_comando_tem_permissao` em `3950–4000`):**
   - `SEM_SESSAO` (`linhas 335–350`, 14 comandos): operam antes do login (`estado`, `configurar_inicio`, `usuarios_entrada`, `entrar`, `sync_juntar`, `sync_desistir_juntar`, `sync_ler`, `aparencia_salvar`, `codigo_ler_foto`, `backup_automatico`, `backup_ao_fechar`, `sair`, `numero_formatar`, `preco_calcular`).
   - `SO_LEITURA` (`linhas 354–449`, 94 comandos): consultas puras. Nos testes (`LEITURA_ESTRITA`, `linha 505`) e após pânico (`self.comprometido`), rodam com `PRAGMA query_only = true` (`linhas 725–754`) para garantir no nível do SQLite que nenhum comando de leitura grave no banco.
   - `DO_CAIXA` (`linhas 454–496`, 39 comandos): gravações permitidas a qualquer operador autenticado (com exceção de `produto_salvar` e `compra_lancar` quando alteram preços de venda de peças cadastradas, que exigem a ação `"precos"`).
   - `acao_do_comando` (`linhas 130–209`): mapeia cada comando restrito a uma das 8 ações configuráveis em `ACOES_DO_GERENTE` (`cancelar_venda`, `estornar`, `ajustar_estoque`, `backup`, `configuracoes`, `usuarios`, `financeiro`, `resultados`) ou às 3 ações em `SEMPRE_DO_GERENTE` (`gerente`, `precos`, `simultaneidade`, `linha 51`).
2. **Idempotência de Gravações (`NAO_REPETIR`, `linhas 277–332, 976–1015`):**
   - Os 51 comandos críticos de escrita em `NAO_REPETIR` exigem `chaveOperacao` não vazia (`FALHA-04`, `linhas 993–998`).
   - O hash `SHA-256(comando || 0x00 || chaveOperacao || 0x00 || usuario_id)` é guardado em `self.feitas` (`VecDeque` de até `2.000` entradas, `linha 332`) **somente quando a execução retorna `Ok(_)`** (`linhas 1010–1015`).
3. **Barreira de Pânico (`C-15`, `linhas 717–782`) e Modo Seguro (`A3`, `linhas 218–275, 788–814`):**
   - Qualquer `panic!` durante `executar` é capturado por `catch_unwind` (`linha 735`), executa `ROLLBACK` se `is_autocommit()` for `false` (`linhas 768–770`), limpa `self.liberacoes` e marca `self.comprometido = true`, bloqueando qualquer escrita subsequente até a reinicialização do processo.
   - Se o banco falhar no `integrity_check` na abertura (`em_modo_seguro()`), todos os comandos em `NAO_REPETIR` e `GRAVAM_TAMBEM` (exceto `backup_restaurar` e `backup_agora`, `linhas 260–265`) são bloqueados com `TEXTO_MODO_SEGURO`.

---

## 3. Revisão Independente dos Achados Históricos do Gemini 4 Argon (Bloco 8A)

| ID Histórico | Título / Descrição Original | Classificação Revisada | Natureza | Evidência Exata no Código-Fonte (`arquivo:linhas`) |
|---|---|---|---|---|
| **B8A-01a** | *(Alegação original no Relatório #18, retificada no Relatório #19):* `gerente_liberar` e `chaveOperacao` impediriam a repetição da chamada após digitar a senha do gerente, ou deixariam token `Liberacao::Uma` pendurado se a chamada falhasse por validação. | **REFUTADO (quanto ao bloqueio de retry)** / **CONFIRMADO PARCIALMENTE (quanto ao resíduo de `Liberacao::Uma` por até 3 min em falha de validação)** | **Risco** (baixo) | Em `Main.qml:465–484` (`chamarComGerente`), o objeto `envio` (com a mesma `chaveOperacao`) é reutilizado na 1ª tentativa, em `pedirGravacao` e no retry. Em `api.rs:1010–1015`, `chave` só entra em `self.feitas` quando `r` é `Ok(_)`, logo o retry funciona. Porém, se o retry falhar por erro de validação de negócio (`r.is_err()`), `Liberacao::Uma` não é consumida (`linhas 1016–1021`); como a correção do campo pelo usuário na tela gera um novo `envio` (novo `resumo`), a `Liberacao::Uma` antiga permanece em `self.liberacoes` até expirar `LIBERACAO_CURTA` (3 min, `linha 32`) ou ser substituída por outro `gerente_liberar` do mesmo comando (`linhas 1088–1091`). |
| **B8A-01b** | **Mascaramento de escopo e autorização implícita de 2ª barreira em comandos com dupla verificação (`config_salvar`, `usuario_salvar`, `sync_salvar`):** Esses 3 comandos possuem um 1º gate genérico em `executar_comando` (`linha 1028`: `self.exigir(acao, sessao)?` para `"configuracoes"` ou `"usuarios"`) e um 2º gate específico dentro do handler (`config_salvar` linha `2600`: `self.exigir("precos", sessao)?`; `usuario_salvar` linha `2840`: `mexe_em_gerente`; `sync_salvar` linha `2503`: `muda_rede`). Quando a ação do 1º gate está marcada como `soGerente: true` (padrão de fábrica), a 1ª tentativa falha logo na linha `1028` com a mensagem genérica (`"Só o gerente pode: configurações da loja"` ou `"usuários"`), ocultando do gerente que o payload também altera regras de preço (`"precos"`), mexe em conta de gerente ou liga/troca a senha da rede. Como `Liberacao::Uma` (`linhas 543, 940–948`) valida apenas `(usuario_id, comando, resumo)` e não a `acao`, a senha dada para o 1º gate aprova silenciosamente o 2º gate na mesma execução! | **CONFIRMADO** | **Defeito comprovado** (mascaramento de motivo de autorização no modal `PedirGerente` e no log de auditoria) | `nucleo/src/api.rs:543` (`Liberacao::Uma`), `940–969` (`liberada_esta_chamada` / `exigir`), `1027–1029` (1º gate), `1098–1101` (log de `gerente.liberar`), `2502–2508` (`sync_salvar`), `2598–2601` (`config_salvar`), `2838–2842` (`usuario_salvar`). |
| **B8A-01c** | **Registro de auditoria de `gerente_liberar` (`gerente.liberar`) usa sempre `acao_do_comando(comando).unwrap_or("precos")`:** Em `api.rs:1098–1101`, quando o gerente autoriza uma gravação avulsa (`Liberacao::Uma`), o detalhe gravado em `auditoria` usa `nome_da_acao(acao_do_comando(comando).unwrap_or("precos"))`. Assim, quando `"configuracoes"` ou `"usuarios"` estão liberados para todos (`acoes_livres`), mas `sync_salvar`, `config_salvar` ou `usuario_salvar` pedem senha especificamente por causa do 2º gate (`muda_rede`, `"precos"` ou `mexe_em_gerente`), o log `gerente.liberar` registra que o gerente liberou `"configurações da loja"` ou `"usuários"`, e não `"precos"`, `"conta de gerente"` ou `"senha da rede"`. | **CONFIRMADO** | **Defeito comprovado** (imprecisão na trilha de auditoria `gerente.liberar`) | `nucleo/src/api.rs:1098–1101`. |
| **B8A-02** | **`compra_conferir_autorizacao` valida senha de gerente diretamente sem atualizar `segredo_backup` e sem usar `Liberacao`:** Em `api.rs:1583–1600`, o comando `compra_conferir_autorizacao` (que está em `DO_CAIXA`, `linha 472`) exige sempre `d.senha` no próprio payload e chama `usuarios::conferir_gerente(&self.banco.conn, &d.senha)`. Ao contrário de `gerente_liberar` (`linha 1069`) e `entrar` (`linha 2768`), quando a senha do gerente é conferida com sucesso em `compra_conferir_autorizacao`, `self.segredo_backup` não é atualizado via `self.segredo_conferido(&g.id, &d.senha)?`. *(Nota: exigir a senha explícita no payload mesmo que a sessão já seja de gerente é intencional pela regra `D7` de `PEND-09`, pois grava o `g.id` do gerente conferente em `autorizado_por`.)* | **CONFIRMADO PARCIALMENTE** | **Melhoria** (atualizar `segredo_backup` ao validar senha de gerente em `compra_conferir_autorizacao`) | `nucleo/src/api.rs:472, 1069, 1583–1600, 2768`. |
| **B8A-03** | **Alegação de que `compra_lancar` ignoraria preços travados ao recalcular preços de venda:** Conforme já demonstrado no Bloco 7B (`nucleo/src/compras.rs:1286–1308`), o esquema de `produto_preco` representa preço fixo (sem margem automática) com `lucro = NULL`, e `compras::atualizar_custo` executa `let Some(lucro) = lucro else { continue };` (`compras.rs:1293`), pulando todas as tabelas de preço fixo. Em `api.rs:1557–1575`, `compra_lancar` verifica `r.precos_atualizados > 0` dentro da transação e desfaz toda a transação se `self.exigir("precos", sessao)` não estiver satisfeito. | **REFUTADO** | — (Comportamento correto no código-fonte) | `nucleo/src/api.rs:1557–1575`; `nucleo/src/compras.rs:1286–1308`. |

---

## 4. Novos Achados Identificados no Bloco 8A (`NEW-B8A-01` a `NEW-B8A-04`)

| ID | Classificação | Arquivo e Linhas | Descrição Técnica e Evidência |
|---|---|---|---|
| **NEW-B8A-01** | **Defeito comprovado** (estado em memória dessincronizado após `backup_restaurar`) | `nucleo/src/api.rs:2548–2585` | **Após `backup_restaurar` (fora do modo seguro), `self.banco.dispositivo`, `self.segredo_backup` e `self.liberacoes` não são atualizados/limpos em memória:** Na linha `2578`, `backup_restaurar` faz apenas `self.sessao = None;`. Porém: (1) `self.banco.dispositivo` em memória continua com o `dispositivo_id` do banco **anterior** à restauração, enquanto a tabela `config` do banco restaurado pode conter outro `dispositivo_id` (todas as gravações e auditorias subsequentes até fechar o programa usarão `self.banco.dispositivo` antigo, diferente do que `auditoria_listar` na linha `1783` lê de `config::ler(..., "dispositivo_id")`!); (2) `self.segredo_backup` permanece com a chave secreta de backup do banco antigo se o próximo login não for de gerente (`linhas 2767–2769`); (3) `self.liberacoes` e `self.feitas` não são limpas no momento da restauração. |
| **NEW-B8A-02** | **Defeito comprovado** (bloqueio silencioso de edição de margens padrão em `config_salvar` quando operador `Caixa` recebe liberação do gerente) | `nucleo/src/api.rs:2587–2601, 2917–2919, 2956–2965` | **Operador `Caixa` não consegue ver nem editar o `% de lucro sugerido` das tabelas de preço em `TelaConfiguracoes` mesmo após digitar a senha do gerente:** Em `config_ler` (`linha 2588`) e `config_salvar` (`linha 2594`), o núcleo usa `let ve = self.ve_custos(sessao)?;`, que chama `self.pode_sem_senha("resultados", sessao)` (`linha 2918`). Como `pode_sem_senha` verifica apenas `sessao.gerente || acoes_livres.contains("resultados")` e **ignora** tanto `Liberacao::Consulta` quanto `Liberacao::Uma`, um operador no modo `Caixa` (quando `"resultados"` é só do gerente) recebe `verLucro: false` e `lucro_padrao: ""` em `config_ler` mesmo depois de clicar em `"Digitar senha do gerente"` em `TelaConfiguracoes.qml:156`, e em `config_salvar` (`linhas 2595–2597`) `config::manter_lucro` descarta qualquer alteração de lucro antes mesmo de chegar ao `self.exigir("precos", sessao)` na linha `2600`! Isso torna a linha `2600` (`if !regras.is_empty() { self.exigir("precos", sessao)?; }`) **inalcançável** a menos que `"resultados"` esteja liberado para todos na pílula Gerente! |
| **NEW-B8A-03** | **Defeito comprovado** (`gerente_encerrar` limpa liberações globalmente em vez de por escopo/tela) | `nucleo/src/api.rs:1121–1124` vs. `app/qml/TelaConfiguracoes.qml:49` | **`gerente_encerrar` apaga `self.liberacoes.clear()` inteiro:** Quando uma tela chama `Api.chamar("gerente_encerrar")` no seu `Component.onDestruction` (como `TelaConfiguracoes.qml:49`, `TelaFinanceiro.qml`, `TelaUsuarios.qml`, `TelaDre.qml`), o comando limpa **todas** as entradas de `self.liberacoes`. No cenário `NAV-01` (onde uma 2ª instância de `TelaConfiguracoes` é empilhada sobre a 1ª via `janela.guiar` e depois desempilhada ao concluir a guia), o `Component.onDestruction` da 2ª instância chama `gerente_encerrar` e revoga a liberação de gerente da 1ª instância que continua aberta na tela. |
| **NEW-B8A-04** | **Melhoria** (inconsistência entre `pode_sem_senha` e `pode_ler` para `"resultados"`) | `nucleo/src/api.rs:2033` vs. `2113, 2130` | **Em `painel_resumo` (`linha 2033`) usa-se `self.pode_ler("resultados", sessao)`, mas em `estoque_mais_vendidos` (`linha 2113`) e `estoque_parados` (`linha 2130`) usa-se `self.pode_sem_senha("resultados", sessao)`:** Se uma consulta de `"resultados"` for aberta com senha do gerente (`Liberacao::Consulta`), `painel_resumo` exibe lucro/margem, mas `estoque_mais_vendidos` e `estoque_parados` continuam ocultando lucro/margem/valor parado por não consultarem `pode_ler`. |

---

## 5. Registro Separado de Testes (Código Lido vs. Testes Encontrados vs. Testes Executados)

### 5.1. Testes Unitários Lidos no Código-Fonte (`nucleo/src/api.rs:3440–4550` — 32 funções `#[test]`)

| # | Nome da Função `#[test]` | Linhas | Escopo Verificado no Código |
|---|---|---|---|
| 1 | `leitor_e_etiqueta_com_codigo_repetido` | `3460–3489` | Código repetido entre lojas sincronizadas: `produto_por_codigo` retorna `opcoes`, bloqueia impressão de etiqueta e exibe em `codigos_situacao`. |
| 2 | `copia_do_banco_so_o_gerente_registra_como_caixa_novo` | `3495–3533` | Detecção de cópia de banco (`computador_id` divergente), bloqueio de geração de códigos e registro como caixa novo (`codigos_registrar_caixa_novo`). |
| 3 | `banco_sem_registro_do_computador_o_gerente_confirma` | `3538–3550` | Banco legado sem `computador_id`: confirmação pelo gerente via `codigos_confirmar_computador`. |
| 4 | `codigo_de_cliente_sai_da_faixa_e_o_gerente_resolve_repetido` | `3555–3582` | `PEND-01`: código sequencial de pessoa na faixa do caixa e resolução de código repetido via `codigos_pessoa_dar_novo`. |
| 5 | `panico_antes_do_commit_desfaz_e_bloqueia` | `3595–3613` | `C-15`: pânico antes do commit faz rollback automático, oculta dados sensíveis da mensagem de erro e ativa `self.comprometido`. |
| 6 | `panico_depois_do_commit_nao_repete` | `3618–3629` | `C-15`: pânico após commit preserva a gravação única e bloqueia novas escritas até reabrir o serviço. |
| 7 | `so_leitura_nao_grava` | `3635–3691` | `R-04` / `RVI-01`: executa todos os 94 comandos de `SO_LEITURA` com 5 payloads variados verificando `total_changes()` inalterado e `query_only = true`. |
| 8 | `rvi01_whatsapp_depois_do_panico_nao_grava` | `3721–3745` | `RVI-01`: `encomenda_whatsapp` é bloqueado quando `self.comprometido == true`. |
| 9 | `rvi01_whatsapp_marca_avisado_sem_panico` | `3750–3757` | `RVI-01`: `encomenda_whatsapp` grava `avisado_em` e retorna link `https://wa.me/...`. |
| 10 | `rvi01_quem_grava_nao_e_so_leitura` | `3764–3777` | Disjunção estrita entre `SO_LEITURA` e (`NAO_REPETIR` $\cup$ `GRAVAM_TAMBEM` $\cup$ `DO_CAIXA`). |
| 11 | `m04_senha_trocar_e_o_historico_juntos` | `3794–3806` | `M-04`: atomicidade transacional entre troca de senha e registro na tabela `auditoria`. |
| 12 | `m04_computador_salvar_tudo_ou_nada` | `3810–3816` | `M-04`: atomicidade transacional de `computador_salvar`. |
| 13 | `c8_estoque_dias_e_o_historico_juntos` | `3820–3828` | Atomicidade transacional entre `estoque_dias` e `auditoria`. |
| 14 | `um_rustinho_por_banco` | `3833–3842` | `C-12`: lock exclusivo `<banco>.trava` impede duas instâncias no mesmo arquivo. |
| 15 | `trava_que_falha_recusa` | `3846–3856` | `R-07`: falha de lock em sistema de arquivos de rede ou sem permissão recusa abertura. |
| 16 | `dados_so_do_usuario` | `3862–3876` | Permissões Unix `0700` na pasta/registro e `0600` no banco SQLite. |
| 17 | `fechar_com_tarefa_travada_segura_a_trava` | `3882–3906` | `R-06`/`R-07`: se thread de sync não encerrar no prazo, `std::mem::forget` retém o file lock até o término do processo. |
| 18 | `pasta_ruim_da_erro_e_a_boa_abre` | `3911–3923` | `R-07`: tratamento de erro sem pânico quando o caminho pai é um arquivo regular. |
| 19 | `permissoes_conferidas` | `3928–3940` | Teste unitário de `arquivos::so_do_usuario` (`0600`). |
| 20 | `todo_comando_tem_permissao` | `3950–4000` | `C-07`: introspecção estática de `api.rs` e de `app/{qml,qml_android,base}` garantindo que todo comando despachado tem classe de permissão e que toda chamada `Api.chamar("...")` nas telas existe na API. |
| 21 | `r5_t6_operacoes_de_dinheiro_recusam_so_o_inconsistente` | `4024–4106` | `R-05`: verifica que as 15 operações financeiras recusam registros em estado `inconsistente` em `sync_referencia`, mas permitem `esperando`, `apagado`, `aceito` e outros registros. |
| 22 | `r5_t7_o_resto_da_loja_continua` | `4111–4151` | `R-05`: uma parcela inconsistente de cliente não bloqueia outra parcela do mesmo cliente nem o fechamento do caixa. |
| 23 | `r5_api_aceitar_exige_gerente_motivo_e_audita` | `4155–4172` | `R-05`: comando `referencia_aceitar` exige gerente, motivo não vazio e grava auditoria. |
| 24 | `r5_t9_varredura_toda_operacao_de_dinheiro_chama_o_bloqueio` | `4177–4207` | `R-05`: inspeção do código-fonte de `receber.rs`, `cheques.rs`, `pagar.rs`, `compras.rs`, `devolucoes.rs` e `financeiro.rs` garantindo presença de `exigir_editavel` / `exigir_filhos_editaveis`. |
| 25 | `f17_a1_primeira_configuracao` | `4259–4271` | `F-17`: política mínima de 4 caracteres em `configurar_inicio` com rollback total em caso de recusa. |
| 26 | `f17_a2_usuario_salvar` | `4275–4323` | `F-17`: política de senha em `usuario_salvar` para operadores e gerentes. |
| 27 | `f17_a3_senha_trocar` | `4327–4349` | `F-17`: limites de 4 a 128 caracteres em `senha_trocar`. |
| 28 | `f17_a4_senha_antiga_entra_com_aviso` | `4353–4408` | `F-17`: senhas antigas (< 4 chars) permitem login, geram aviso local (`senha.<id>`), gravam 1 auditoria e nunca expõem a senha em claro. |
| 29 | `f17_a5_esconder_painel_e_troca` | `4413–4463` | `F-17`: ocultação individual do aviso de senha fraca, visibilidade de `senhaFraca` restrita ao gerente em `usuarios_listar` e limpeza ao trocar a senha. |
| 30 | `f17_a6_hash_trocado_por_outro_caixa` | `4467–4481` | `F-17`: invalidação automática da avaliação em `senha_situacao` quando `senha_hash` muda via sincronização. |
| 31 | `f17_a7_senha_longa_e_unicode` | `4485–4517` | `F-17`: suporte a Unicode (acentos, CJK, espaços), teto de 128 caracteres e rejeição rápida de payload de 100.000 caracteres antes do Argon2. |
| 32 | `f17_a8_senha_da_rede` | `4522–4549` | `F-17`: política de senha da rede em `sync_salvar` e preservação de senha antiga quando o campo vem em branco. |

### 5.2. Testes Executados
- **Nenhum teste foi executado em runtime nesta etapa**, mantendo separação estrita entre leitura estática e execução, conforme diretriz do plano.

