# Rastreabilidade das auditorias do Rustinho (até a 0.9.15)

Documento criado na revisão da 0.9.15 (27/10/2026) para que uma auditoria futura não precise reconstruir a origem de cada identificador por inferência. Ele guarda as listas originais e a situação de cada item, com a fonte.

## Fontes e séries (não misturar)

Há identificadores com o mesmo número em séries diferentes. Cite sempre a série.

| Série | O que é | Fonte original (fora do repositório, guardada pelo dono) |
|---|---|---|
| **F-01 a F-24** | Achados originais da 0.9.x | `Briefing_Final_Correcao_Rustinho_0.9.11.md`, seção 4 |
| **C-01 a C-16** | Achados complementares da 0.9.11 | o mesmo briefing, seção 5 |
| **R-01 a R-12 (0.9.11)** | Revisão sênior da 0.9.11 | `Auditoria_Rustinho_0.9.11_Revisao_Senior.md` |
| **R-01 a R-12 (0.9.14) e M-01 a M-06** | Auditoria da 0.9.14 | `docs/AUDITORIA-0.9.14.md` |
| **D-01 a D-18** | Defeitos achados nos testes de estabilidade e na auditoria da 0.9.14 | `docs/DEFEITOS.md` |
| **PEND-01 a PEND-30** | Pendências do projeto (PEND-24 a PEND-30 abertas na 0.9.15) | `PENDENCIAS.md` (raiz) |
| **FALHA-01 a FALHA-07** | Auditoria do Sunda sobre a 0.9.15 | relatório do Sunda (anexo) |
| **O-1 a O-8** | Omissões daquela auditoria, achadas na validação | "Validação da auditoria do Sunda — Rustinho 0.9.15" |
| **D1 a D4** | Requisitos da Simultaneidade | `docs/SIMULTANEIDADE-requisitos.md` |
| **RVI-01 a RVI-03** | Revisão independente do complemento C7 da 0.9.15 (28/10/2026) | `revisao_independente_final_rustinho_0.9.15.md` (anexo) |
| **0915-A em diante** | Achados próprios das revisões da 0.9.15 | esta matriz, seção final |

Atenção a homônimos comuns: **R-05 (0.9.11)** = referências quebradas entre caixas; **R-05 (0.9.14)** = tarefa da troca que morre num pânico (= O-3). **R-01 (0.9.11)** = NF-e sem conferência criptográfica; **R-01 (0.9.14)** = senha da rede de 4 caracteres. Os testes "F-01 a F-04" de `docs/FISCAL-assinatura-arquitetura.md` são outra coisa (testes da arquitetura fiscal), e "F-22" em `faixas.rs` é o código de um fornecedor num teste.

## F-01 a F-24 (fonte: briefing 0.9.11, seção 4)

Situação "r7" = matriz `Rustinho-matriz-auditorias-r7.md` (20/10/2026); "0.9.15" = esta revisão.

| ID | Título original | Situação (r7) | Situação na 0.9.15 e evidência |
|---|---|---|---|
| F-01 | Assinatura Android conhecida | Parcial; não confirmado | Igual (PEND-06): publicação só com a chave da loja, segredos não cadastrados; não montado na 0.9.15 |
| F-02 | KDF de sincronização | Parcial | Igual: Argon2id com parâmetros fixos e testados; velocidade no celular não medida |
| F-03 | Thread por conexão sem limite | Corrigido (com C-01) | Igual: no máximo 8 atendimentos, vaga RAII (`sync.rs`, `Vaga`) |
| F-04 | Chaves estrangeiras desligadas durante a sync | Parcial (R-05 0.9.11) | Referências com prova (0.9.12); falha nova da 0.9.15 corrigida aqui (FALHA-03) |
| F-05 | "Último UUID vence" depende do relógio | Parcial | Igual: ordem e história, não relógio; aviso de relógio |
| F-06 | Log de alterações sem retenção | Corrigido | Igual (compactação) |
| F-07 | Senha de backup em claro e sincronizada | Corrigido (com C-03) | Igual (chave da loja X25519) |
| F-08 | Restore concorrente com sync | Corrigido (com C-12) | Igual (pausa da troca; um Rustinho por banco) |
| F-09 | ZIP sem limite global de expansão | Corrigido | Igual |
| F-10 | RAR descompactado antes de validar tamanho | Corrigido | Igual |
| F-11 | Backup/restore inteiro na memória | Parcial | Igual (limites; teste do limite existe desde a 0.9.12) |
| F-12 | Orçamento não reivindicado ao vender | Corrigido no caixa; parcial entre caixas | Igual (entre caixas sem rede vai para o gerente) |
| F-13 | Cliente inexistente na venda | Corrigido | Igual |
| F-14 | `i128` para `i64` sem checagem | Corrigido (com R-10 0.9.11, C-05) | Igual |
| F-15 | XML de NF-e não autorizado / destino errado | Parcial | Assinatura e cadeia conferidas (PEND-09); autorização da SEFAZ não (PEND-15, bloqueador) |
| F-16 | Nome de usuário duplicado após sync | Corrigido (com C-06) | Igual |
| F-17 | Todos tinham acesso a tudo / senha mínima curta | Parcial | Parcial por decisão do dono: mínimo de 4 caracteres (mantido nesta rodada) |
| F-18 | CI com permissões amplas e artefatos mutáveis | Parcial (com C-11) | Igual; o robô do Windows passou a disparar na branch da 0.9.15 e à mão (FALHA-07); sem execução real |
| F-19 | Erro de banco vira `false` no estoque | Corrigido | Igual (não reconferido em detalhe) |
| F-20 | Pânico sem diagnóstico persistente | Corrigido (com C-16) | Igual (registro com arquivo e linha, sem dados) |
| F-21 | Módulos/telas grandes | Presente (dívida) | Presente e maior (sync.rs ~6 mil linhas; api.rs ~4,1 mil); M-01 da 0.9.14 |
| F-22 | Testes insuficientes de concorrência e recuperação; CI com falsa confiança no Android | Não confirmado (com R-12 0.9.11, C-09) | Melhor no núcleo (testes de queda, rede, supervisão da troca nesta rodada); Android não montado na 0.9.15 |
| F-23 | Parser numérico aceita separadores malformados | Corrigido | Igual |
| F-24 | Motor fiscal/emissão incompleto | Não confirmado (fora do escopo) | Presente: emissão não existe; contrato de integração v0.1 em revisão |

## C-01 a C-16 (fonte: briefing 0.9.11, seção 5)

| ID | Título original | Situação (r7) |
|---|---|---|
| C-01 | Contador de conexões TCP pode ficar ocupado após pânico | Corrigido (vaga RAII) |
| C-02 | Conflitos detectados só num sentido, sem fluxo de resolução | Parcial (código corrigido; rede real na PEND-05) |
| C-03 | Chave real dos backups em claro e sincronizada | Corrigido |
| C-04 | `cStat=100/150` não prova autenticação da SEFAZ | Parcial (ver F-15, PEND-09, PEND-15) |
| C-05 | Somatórios financeiros podem exceder `i64` | Corrigido |
| C-06 | "Pelo menos um gerente" violável por sync | Corrigido |
| C-07 | Senha de 4 caracteres e cobertura de permissões | Parcial (4 caracteres por decisão do dono) |
| C-08 | Limites de importação/backup no Android de 2 GB | Corrigido no computador; celular não confirmado |
| C-09 | Workflow Android verde mesmo com falha | Não confirmado |
| C-10 | Publicador pode ignorar a assinatura em APK existente | Parcial (segredos não cadastrados) |
| C-11 | Dependências do build sem checksum | Parcial (sem execução real no GitHub) |
| C-12 | Bloqueio de restore só dentro de um processo | Corrigido (um Rustinho por banco) |
| C-13 | Argumentos internos de teste aceitos no binário Release | **Reverificado na C8 (28/10/2026): continua aceito, risco baixo, não mudado.** `Main.qml`: `--abrir` só depois de entrar e só com nome de tela (`^Tela[A-Za-z]{1,40}$`), `--props` inválido é ignorado; `--textos-da-tela` liga o Ctrl+Shift+F12 que escreve os textos da tela na saída (sem senhas); `--camera` abre o leitor. Retirar do binário de produção é decisão de empacotamento |
| C-14 | Contrato da API JSON sem suíte de regressão | Parcial em r7; hoje existe `tests/contratos_telas.rs` (campos que as telas leem), além de `tests/contratos.rs` |
| C-15 | `catch_unwind` não prova consistência | Corrigido (serviço "comprometido" até reabrir) |
| C-16 | Gancho de pânico pode gravar dados sensíveis | Corrigido |

## R-01 a R-12 da revisão da 0.9.11 (situação r7)

R-01 NF-e sem conferência criptográfica: parcial em r7, implementada depois (PEND-09), autorização da SEFAZ aberta (PEND-15) · R-02 data malformada derruba: corrigido · R-03 limite da importação para XML solto: corrigido · R-04 `estoque_dias` "leitura" que grava: corrigido · R-05 referências quebradas: parcial em r7, com prova desde a 0.9.12 e falha nova corrigida na 0.9.15 (FALHA-03) · R-06 motor sem esperar as tarefas: corrigido · R-07 trava que falha: corrigido · R-08 conflitos dependem do relógio: parcial · R-09 rajada UDP força o Argon2: corrigido · R-10 somas fora do limite: corrigido · R-11 contratos Rust–QML: parcial · R-12 teste do Android: não confirmado.

## R-01 a R-12 e M-01 a M-06 da auditoria da 0.9.14 (fonte: `docs/AUDITORIA-0.9.14.md`)

| ID | Assunto | Situação depois da 0.9.15 |
|---|---|---|
| R-01 | Senha da rede de 4 caracteres (ataque offline ao anúncio UDP 47811) | **Aberto (bloqueador).** C8: desenho do protocolo 4 em `docs/PROPOSTA-D1-R01-protocolo-4.md` (chaves por computador, canal TLS 1.3/`rustls` ou Noise, anúncio assinado, nada derivado de senha na rede); **não implementado** — muda o protocolo, espera decisão do dono (briefing §16) |
| R-02 | Cancelar/estornar com o caixa aberto em outro computador | **Aberto (bloqueador).** C8: regra por caso (estende a D-14: a sessão de outro computador nunca muda; o dinheiro fica na gaveta deste ou no Caixa geral) em `docs/PROPOSTA-R02-0915L-cancelamentos.md`; **não implementada** — muda dinheiro, espera decisão. Evidência: auditoria da 0.9.14 e leitura do código; não reproduzido com dois caixas na C8 |
| R-03 | Parcela paga a mais por dois caixas sem rede | **Detecção feita na C8 (28/10/2026); compensação continua manual.** Parcela a receber e conta a pagar pagas a mais e vale com saldo negativo viram conflito (`pago_a_mais`, `vale_a_mais`) depois da troca, com quem/quanto/quais operações/qual caixa; nada apagado; corrigido pelo gerente, sai sozinho. Testes `r03_*` (3, falharam antes: `logs/09-r03-antes.log`; depois 3/3 e 38/38 de conflitos e troca: `09-r03-depois.log`). Falta: decisão sobre compensação pronta (PROPOSTA-R02 §6.4); teste com dois caixas físicos |
| R-04 | Configurações locais que chegam pela troca | Corrigido (D-16) |
| R-05 | Tarefa da troca para num pânico | **Corrigido na 0.9.15 (O-3)**: supervisão, volta sozinha, aviso, para de vez se cair demais |
| R-06 | Senha de entrada de 4 números | Igual (espera depois de erros; 4 caracteres por decisão) |
| R-07 | `zip` 2.4.2 (3 avisos do cargo audit) | **Corrigido na C8:** `zip` 9.1.3 (só `deflate`), no computador e no `Cargo.lock` do Android; uma mudança de API; backup feito com a zip 2 restaura com a 9 (`logs/08-r07-compat-backup-antigo.log`); `cargo audit` sem avisos (`logs/11-audit*.log`, base de avisos de 28/10/2026). Falta: compilar Windows e Android com a zip 9 |
| R-08 | Permissões da pasta no Windows | **Aberto, não testável aqui** (sem Windows). Lido na C8: o instalador pede administrador e o "apagar dados" usa a pasta do usuário que desinstala (PEND-32). Matriz W2, W5 |
| R-09 | Duplo clique / Enter repetido | **Corrigido na 0.9.15 (FALHA-04)**: chave por operação, obrigatória |
| R-10 | Banco estragado sem `quick_check` | **Corrigido na 0.9.15 (FALHA-06)**: ao abrir e no backup |
| R-11 | Rede no Windows (PEND-13) | **Aberto, não testável aqui** (sem Windows; PEND-13). Matriz W3 |
| R-12 | Descartar rascunho de compra apaga a auditoria | **Corrigido na 0.9.15 (O-4)** |
| M-01 | Dividir `api.rs` e `sync.rs` | **Igual (dívida técnica, não refeito na C8):** hoje 4.515 e 6.430 linhas |
| M-02 | "Próximo número" repetido | **Igual:** `MAX(numero)` em vendas, devoluções, receber, encomendas, oficina |
| M-03 | Leitura dos pagamentos em `vendas::calcular` repetida | Não revisto na C8 |
| M-04 | Gravações fora de transação / auditoria depois do commit | **Corrigido na C8** para `computador_salvar` e `senha_trocar` (e `estoque_dias`, achado C8-01); `unidade_salvar` já era uma gravação só. Testes `m04_*`, `c8_estoque_dias_*` (falharam antes: `logs/07-m04-antes.log`). **Fica:** auditoria do `backup_restaurar` depois de trocar o banco (não cabe numa transação) |
| M-05 | Conexão nova com o banco a cada volta da troca | **Igual** (desperdício, não defeito) |
| M-06 | `quick_check` na cópia do backup | Feito na 0.9.15 (FALHA-06); a cópia gravada já era conferida pela leitura de volta |

## D-01 a D-18 (fonte: `docs/DEFEITOS.md`)

Todos corrigidos antes da 0.9.15. Na 0.9.15: **D-09** ganhou o isolamento da consulta da Simultaneidade (FALHA-05; a senha dela não abre mais a pílula Gerente); **D-14** continua, e R-02 (0.9.14) depende dele; **D-16** continua valendo (a identidade da loja nova é configuração da loja, replicada; as marcas novas `loja_par:*`, `juntar_tentativa` e `sync_outra_loja` são deste computador e não vão pela rede).

## Auditoria do Sunda (0.9.15) e omissões

| ID | Situação final | Onde |
|---|---|---|
| FALHA-01 | Corrigida: desistência da junção (V01a); identidade da loja e recusa de lojas independentes (V01b); ligar a rede só com o gerente. Complemento (análise independente): loja escolhida fixada na entrada do caixa novo (reproduzida a fusão com caixa sem usuários, duas lojas e lotes de 1, nas duas ordens); senha da rede só com o gerente também desligada (A2); desistência só com tentativa, atômica, sem apagar o que é local (A4). **D1 continua pendente** | `nucleo/src/loja.rs`, `sync.rs`, `api.rs`; testes `testes_loja.rs` (`falha01_*`, `a2_*`, `a4_*`) |
| FALHA-02 | Corrigida como apresentação (caixa junto do número); sem índice único | `TelaOrdens.qml`, `TelaOrdem.qml`, `TelaVender.qml`; teste `f02_*` |
| FALHA-03 | Corrigida (pai guardado em pendência não vira inconsistência) | `sync.rs` `registrar_referencias`; testes `v03_*` |
| FALHA-04 | Corrigida (chave por operação em todas as telas; obrigatória no núcleo; sem janela de tempo) | `Api.qml`, `Main.qml`, 18 telas e seções, `api.rs`; `testes_chave.rs` |
| FALHA-05 | Corrigida (consulta "simultaneidade") | `api.rs`, `TelaConfiguracoes.qml`; `t5_10`, telas D-09 |
| FALHA-06 | Corrigida (cópia reaproveitada por SHA-256, nome único da falha, `quick_check` ao abrir e no backup). Complemento A3: banco com defeito → modo seguro (só leitura; nada comercial gravado; troca de dados parada; cópia do banco com defeito só para o suporte, `rustinho-defeito-…`, fora da retenção) | `migracao.rs`, `banco.rs`, `backup.rs`, `api.rs`, `sync.rs`; testes `f06_*`, `testes_modo_seguro.rs`, `app/testes/telas_modo_seguro.py` |
| FALHA-07 | Corrigida (Olho em Receber; gatilho do robô). Complemento A5: teste de tela do campo do valor, do F12 e do clique no Olho com a janela aberta (21 de 21) | `TelaReceber.qml`, `LinhaDevedor.qml`, `LinhaParcela.qml`, workflow; `app/testes/telas_falha07.py` |
| O-1 | Corrigida (`ctutils` 0.4.6) no computador **e** no app de Android (que tem `Cargo.lock` próprio; ver 0915-I) | `Cargo.lock`, `android/rust/Cargo.lock` |
| O-2 | D1 inexistente: **pendente (bloqueador)**; desenho proposto na C8, não implementado | `docs/SIMULTANEIDADE-requisitos.md`, `docs/PROPOSTA-D1-R01-protocolo-4.md` |
| O-3 | Corrigida (= R-05 da 0.9.14) | `supervisao.rs` |
| O-4 | Corrigida (= R-12 da 0.9.14) | `compras.rs` |
| O-5 | Corrigida (espera com prazo) | teste `encerrar_com_rede_lenta_ou_com_erro` |
| O-6 | Corrigida (sem depender do relógio) | `t35_*`, `reserva_concorrente` |
| O-7 | Corrigida (manual e documentos) | este documento, `MANUAL.md`, `PENDENCIAS.md`, `docs/INTEGRACOES_EXTERNAS.md` |
| O-8 | Revistos na C8: M-04 corrigido; M-01, M-02, M-05 iguais (dívida técnica); M-03 não revisto; R-08 e R-11 sem Windows aqui | linhas R-08, R-11, M-01..M-05 acima |

## Achados novos da revisão integral da 0.9.15 (não vieram de auditoria anterior)

| Código | Achado | Situação |
|---|---|---|
| 0915-A | `Api.gravar` reaproveitava a chave por "mesmo corpo em 10 s", contra a regra "duas vendas iguais são preservadas" | Corrigido (FALHA-04) |
| 0915-B | Comentários de `migracao.rs` falavam em retenção das cópias, que não existe (regra: nunca apagar) | Corrigido |
| 0915-C | `falha-migracao-<segundo>.db`: duas falhas no mesmo segundo perdiam o segundo banco | Corrigido (FALHA-06) |
| 0915-D | Comentários e nomes de testes falavam em senha de 8/12; a regra é 4 | Corrigido (só texto) |
| 0915-E | Ligar a troca de dados era de "Configurações" (liberável para todos) | Corrigido: sempre do gerente (decisão a confirmar) |
| 0915-F | Avisos de diferenças e códigos apontavam para a pílula Gerente, onde esse conteúdo não está | Corrigido: abrem Simultaneidade › Duplicatas |
| 0915-G | Contagens da validação: `NAO_REPETIR` tem 51 comandos (não 53) e 11 já tinham chave (não 9) | Registrado |
| 0915-H | O backup já conferia a cópia gravada (lendo de volta); faltava conferir o banco de origem | Corrigido (FALHA-06) |
| 0915-I | O app de Android tem `Cargo.lock` próprio (`android/rust/`, fora do workspace): continuava com `ctutils` 0.4.3 depois da correção do O-1 no computador | Corrigido (`cargo update -p ctutils --precise 0.4.6`; o mesmo arquivo passou a registrar o núcleo 0.9.15, que a montagem faria sozinha). `cargo audit` do Android: 3 avisos, só os da zip. Android não montado aqui |
| 0915-J | `android/rust/Cargo.toml` diz versão 0.9.13 (o resto do projeto é 0.9.15) | Registrado, não mudado (número de versão é decisão de publicação) |
| 0915-K | `sync_salvar` gravava as interfaces e depois recusava o endereço inválido (gravação pela metade) | Corrigido: uma transação; teste `k_salvar_a_troca_de_dados_e_tudo_ou_nada` |
| 0915-L | Cancelamentos em cascata (`receber::cancelar_da_venda`, `pagar::cancelar_da_compra`, `cheques::cancelar_de`, abatimento do crediário na devolução) sem `exigir_editavel` | Registrado (PEND-29): decisão de regra junto com R-02/R-03 | — C8: proposta (recusar a cascata com filho esperando ou inconsistente) em `docs/PROPOSTA-R02-0915L-cancelamentos.md` §4; **não implementada**, espera decisão
| 0915-M | Número da encomenda da tabela inteira (não por caixa); a tela não mostra o caixa | **Tratado na C8 como a FALHA-02:** o caixa aparece junto do número em encomendas, comprar, vender e na lista de compra impressa; número sem índice único. Teste `m0915_mesmo_numero_de_dois_caixas_mostra_o_caixa` (falhou antes: `logs/06-0915m-antes.log`) |
| 0915-N | `compra_lancar` com "os preços seguem o % de lucro" mudava o preço de venda de peça cadastrada sem o gerente (contra a D-09) | Corrigido: pede o gerente quando algum preço muda; testes `d09_11` (reprodução) e `d09_12` (controle) |
| 0915-O | Banco com certos defeitos (lista de páginas livres) nem abria antes do complemento (a preparação grava); outros abriam e aceitavam gravação | Corrigido pelo modo seguro (A3) |
| 0915-P | Banco zerado (0 bytes) abre como primeira vez (loja vazia), mesmo com backups da loja na pasta | Registrado (PEND-30): é também o caminho de recuperação de hoje; decisão de fluxo. Teste `p0915_banco_zerado_…` fixa o comportamento |

## Revisão independente do complemento C7 (RVI) e etapa de estabilidade (C8, 28/10/2026)

| ID | Gravidade | Achado | Situação e evidência |
|---|---|---|---|
| RVI-01 | Média | `encomenda_whatsapp` estava em `SO_LEITURA` e grava `avisado_em` quando a encomenda chegou: liberado depois de um pânico | **Corrigido**: vai para `DO_CAIXA`, grava numa transação; depois de um pânico toda consulta roda com o banco só para leitura (`query_only`), e nos testes sempre (a suíte inteira acusa uma leitura que grave: só este comando foi achado). Testes `rvi01_*` (falharam antes), `so_leitura_nao_grava` com encomenda que chegou; mutação |
| RVI-02 | Média | Porta da troca de dados ocupada: a tarefa terminava sem aviso na tela; o limite parava só na 6ª queda | **Corrigido**: falha ao começar ou fim sem pedido contam como queda, com motivo; espera crescente; para na 5ª em 10 min; aviso no Início ("não está funcionando" / "parou", com o motivo) e em Configurações › Caixas; Início confere de novo a cada 20 s. Testes `rvi02_*` (TCP, UDP, falha que passa, limite exato, pânico, API, troca desligada), tela `telas_rvi02.py` 5/5; mutação |
| RVI-03 | Média | Banco de versão anterior com defeito: a atualização (cópia, migração) rodava antes da conferência do modo seguro | **Corrigido**: conferido antes de copiar e de atualizar; com defeito abre em modo seguro na estrutura antiga, sem nada mexido; cópia que não sai → conferência completa decide (modo seguro ou erro claro, sem nada pela metade); Início e menu funcionam com a estrutura antiga. Testes `rvi03_*` (falharam antes), tela `telas_modo_seguro.py` (caso do banco antigo) 9/9; mutação |
| Suíte (C7) | — | 1 falha ambiental: `dois_caixas_pela_rede` com porta fixa ocupada | **Corrigido**: nenhum teste de rede usa porta fixa (o sistema escolhe ao abrir, `Motor::portas`); asserções que aceitavam "Connection refused" conferem o motivo. Prova com as portas antigas ocupadas de propósito |
| C8-01 | Média | `estoque_dias` gravava os dias e o histórico fora de transação (mesma família da M-04) | **Corrigido**: uma transação; teste `c8_estoque_dias_e_o_historico_juntos` (falhou antes) |
| C8-02 | Baixa | `unidade_salvar` muda "fracionada" de uma unidade em uso sem gerente e sem histórico | **Registrado (PEND-31)**: regra ambígua, espera decisão |
| C8-03 | Baixa | Desinstalar no Windows apaga `{localappdata}` de quem desinstala como administrador | **Registrado (PEND-32)**: não testado (sem Windows) |
| C8-04 | Baixa | Android: registro de diagnóstico em `Android/data/<pacote>/files` | **Registrado (PEND-33)**: não testado no aparelho |
| C8-07 | Baixa | `fiscal_automatico` (tirar o CSOSN próprio das peças) é de qualquer usuário (com histórico, numa transação) | **Registrado (PEND-31)**: decidir se mudança fiscal da peça é do gerente |
| C8-05 | Alta (bloqueador) | Integração fiscal: sem emissão 55/65, sem consulta à SEFAZ; adaptador `rustinho-fiscal` fora deste repositório; contrato v0.1; DF-1 a DF-6 abertas; Sintec até 15/12/2026 | **Aberto**: `docs/AUDITORIA-FISCAL-C8.md`; nenhuma operação fiscal real |
| C8-06 | — | Matriz de homologação física | `docs/MATRIZ-homologacao-C8.md`: Windows, Android, rede física, periféricos e fiscal **não executados** |
| Suíte (C8) | — | Execução final | `cargo test --workspace --no-fail-fast`: **476 passaram, 0 falharam, 9 ignorados** (linha de base C8-fase 1: 469/0/8; +7 testes novos: m0915 1, m04 2 + C8-01 1, r03 3; +1 ignorado: `c8_backup_da_zip2_restaura_com_a_zip9`, só com o backup antigo à mão). `logs/10-suite-final.log` |

