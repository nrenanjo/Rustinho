# BLOCO 18 — Auditoria e Execução da Suíte Completa de Testes (Unitários, Integração, Estabilidade, PKI e Python UI)

> **Data da auditoria:** 2026-11-02
> **Escopo:** Inspeção integral de todos os módulos de teste em `nucleo/src/`, `nucleo/tests/`, `nucleo/examples/` e `app/testes/`, acompanhada da execução real e separada da suíte no ambiente de auditoria sem tocar em nenhum arquivo do projeto.

---

## 1. Inventário de Arquivos de Teste Lidos Integralmente no Bloco 18

> Nota: Somam-se a estes os arquivos de teste já lidos integralmente em blocos anteriores: `nucleo/src/testes_avisos.rs` (Bloco 6), `nucleo/src/testes_nav.rs` (Bloco 7A), `nucleo/examples/estoque_teste.rs` e `nucleo/tests/estavel/t2_estoque.rs` (Bloco 8B2/9A1), `nucleo/src/dre/testes.rs` (Bloco 10), `nucleo/src/nfe_assinatura/testes.rs` (Bloco 11), `nucleo/tests/caixas.rs` e `nucleo/src/teste_rede.rs` (Bloco 12), `nucleo/src/arquivos_teste_rar.rs` (Bloco 13) e `android/diagnostico_teste/*` (Bloco 16).

| Arquivo | Linhas | Escopo Auditado |
|---|---:|---|
| `nucleo/src/testes_0915.rs` | 1.149 | Migração 0023 (rascunhos e pendências), quedas simuladas em pontos de falha da migração, conflitos de OS/vendas entre caixas, abertura com `quick_check`. |
| `nucleo/src/testes_chave.rs` | 257 | Idempotência obrigatória (`FALHA-04` / `NAO_REPETIR`), rejeição sem `chaveOperacao`, não-repetição de vendas idênticas com mesma chave e varredura estática dos arquivos QML (`telas_mandam_a_chave`). |
| `nucleo/src/testes_loja.rs` | 796 | Criação de loja, pareamento (`juntar`), recusa de loja distinta (`FALHA-01`), desistência atômica (`A4`), troca de senha da rede (`A2`) e restauração de backup mantendo identidade da loja. |
| `nucleo/src/testes_modo_seguro.rs` | 408 | Corrupção da freelist SQLite (`quick_check != ok`), entrada em `somente_leitura` (`A3`), bloqueio de escrita/backup comum/sync e saída do modo seguro após restaurar backup íntegro. |
| `nucleo/src/testes_r03.rs` | 221 | Conflitos financeiros multi-caixa (`R-03`): parcela de crediário recebida em dois caixas, conta a pagar quitada em dois caixas e vale-crédito consumido em dois caixas. |
| `nucleo/src/testes_rvi03.rs` | 351 | Bancos de versão anterior (`v22`) corrompidos (`RVI-03`), truncados ou zerados (`0 bytes`): garantia de não-migração destrutiva e abertura em modo seguro. |
| `nucleo/src/testes_supervisao.rs` | 305 | Supervisão das threads de sincronização (`RVI-02`): reinício após pânico, parada definitiva após 5 quedas seguidas e exposição do motivo na API quando portas TCP `47810` / UDP `47811` estão ocupadas. |
| `nucleo/tests/contratos.rs` | 223 | Contratos JSON de entrada/sessão, vendas, caixa, supervisão do gerente, backup e tratamento de JSON malformado. |
| `nucleo/tests/contratos_telas.rs` | 787 | Contratos JSON ponta-a-ponta consumidos pelas telas de Receber, Pagar, Pessoas, Gerente/Códigos, Ordem de Serviço, Devolução, Compra por XML e DRE. |
| `nucleo/tests/fluxo.rs` | 475 | Fluxo comercial completo (cadastro, compra por ZIP, orçamento, venda, crediário, limite de crédito, bloqueio por tentativas de senha e permissões gerente vs. caixa). |
| `nucleo/tests/portatil.rs` | 164 | Isolamento de diretórios do modo portátil (`rustinho-portatil.txt`) em subprocesso real. |
| `nucleo/tests/apoio/chave.rs` | 115 | Helper de teste que injeta `chaveOperacao` automaticamente para os 52 comandos em `NAO_REPETIR`. |
| `nucleo/tests/apoio/cnpj.rs` | 28 | Helper de configuração de CNPJ válido da loja para testes de importação. |
| `nucleo/tests/estavel/main.rs` | 100 | Ponto de entrada da suíte `estavel` e utilitários `montar_loja_de_teste`, `executar_por_fora` e `desativar_por_fora`. |
| `nucleo/tests/estavel/comum.rs` | 480 | Fixture padrão da loja de teste (usuários Ana/Bia, peças, compras, vendas, crediário, avisos). |
| `nucleo/tests/estavel/a1_auditoria.rs` | 298 | Testes de regressão da auditoria `0.9.14` (`D-14`: cancelamento de venda, estorno de recebimento e exclusão de devolução de caixa já fechado). |
| `nucleo/tests/estavel/d09_custos.rs` | 680 | Testes de sigilo de custos/margens para o papel `caixa` e exigência de token de uso único do gerente (`D-09`). |
| `nucleo/tests/estavel/t1_backup.rs` | 190 | Restauração completa em outra pasta, rollback via cópia `.antes-restaurar` e rejeição de backup corrompido. |
| `nucleo/tests/estavel/t3_crediario.rs` | 206 | Baixas parciais, quitação, multa/juros por atraso, estorno, devolução e travas de balcão no crediário. |
| `nucleo/tests/estavel/t4_dre.rs` | 171 | Conferência aritmética do DRE mensal contra cálculo manual, restrição de acesso ao gerente e arredondamento de ticket médio. |
| `nucleo/tests/estavel/t5_permissoes.rs` | 303 | Matriz exaustiva de permissões `caixa` vs. `gerente`, escopos de consulta (`Simultaneidade` vs. `Gerente`) e invalidação imediata ao rebaixar papel. |
| `nucleo/tests/estavel/t6_recuperacao.rs` | 288 | Testes com `SIGKILL` (`kill -9`) em subprocessos reais durante vendas, criação de backup e restauração. |
| `nucleo/tests/estavel/t7_importacao.rs` | 297 | Importação de XML de compra com verificação de totais, preservação do arquivo de origem e não-vinculação automática de códigos repetidos. |
| `nucleo/tests/estavel/t8_sincronizacao.rs` | 235 | Sincronização TCP em subprocessos com queda de conexão no meio e rejeição por senha de rede incorreta. |
| `nucleo/tests/pki/gerar.py` | 269 | Gerador determinístico da cadeia PKI de teste (Raiz, AC Intermediária, Certificados A1/A3 de emitentes, expirados, revogados/inválidos e XMLs assinados). |
| `nucleo/tests/pki/gerar_acervo.py` | 195 | Gerador do acervo de casos de teste `A01`–`A55` para validação X.509 e XMLDSig. |
| `nucleo/tests/pki/gerar_alfanumerico.py` | 94 | Gerador de certificados e XMLs com CNPJ alfanumérico (`PEND-19`, vigência julho/2026). |
| `nucleo/tests/pki/gerar_c14n.py` | 35 | Gerador de vetores sintéticos C14N 1.0 conferidos contra `libxml2`. |
| `nucleo/examples/popular.rs` | 100 | Script de exemplo para popular banco com usuários, contas, despesas, movimentos e vendas. |
| `app/testes/telas_d09.py` | 324 | Teste E2E Xvfb (`xdotool` + `--textos-da-tela`) de sigilo de custos e liberação do gerente (`D-09` e `FALHA-05`). |
| `app/testes/telas_d14_portatil.py` | 190 | Teste E2E Xvfb do cancelamento de venda de caixa fechado (`D-14`) e do isolamento do modo portátil. |
| `app/testes/telas_falha07.py` | 183 | Teste E2E Xvfb do botão Olho / `F12` na tela `TelaReceber` (`FALHA-07` e `A5`). |
| `app/testes/telas_modo_seguro.py` | 184 | Teste E2E Xvfb da faixa vermelha e imutabilidade do arquivo `.db` em modo seguro (`A3` e `RVI-03`). |
| `app/testes/telas_navegacao_etapa1.py` | 346 | Teste E2E Xvfb da navegação guiada em `TelaCompras` e `TelaFiscal` (`0.9.17`). |
| `app/testes/telas_navegacao_guiada.py` | 341 | Teste E2E Xvfb do piloto de navegação guiada em `TelaPecas › Importar XML` (`0.9.16`). |
| `app/testes/telas_notificacoes.py` | 308 | Teste E2E Xvfb de notificações do Início (quadrados, faixas, gestos de arrasto, temas claro/escuro e janela estreita). |
| `app/testes/telas_rvi02.py` | 122 | Teste E2E Xvfb de exposição de falha de porta TCP `47810` ocupada no Início e em Configurações (`RVI-02`). |

---

## 2. Quadro Separado: Testes Encontrados no Repositório vs. Testes Executados na Auditoria

### 2.1. Resumo Quantitativo de Testes Escritos no Repositório

| Categoria | Arquivos | Funções `#[test]` / Casos | Observação |
|---|---:|---:|---|
| Testes Unitários em `nucleo/src/**` (`--lib`) | 50 módulos com `#[cfg(test)]` | 413 funções `#[test]` (408 ativas + 5 `#[ignore]`) | Inclui 37 testes em `nfe_assinatura::testes` (35 ativos + 2 `#[ignore]` de acervo externo). |
| Testes de Integração em `nucleo/tests/*.rs` | 5 (`caixas`, `contratos`, `contratos_telas`, `fluxo`, `portatil`) | 28 funções `#[test]` (27 ativas + 1 helper `#[ignore]`) | Testam a API JSON e subprocessos do núcleo. |
| Suíte de Estabilidade em `nucleo/tests/estavel/*.rs` | 11 arquivos (`main`, `comum`, `a1`, `d09`, `t1`..`t8`) | 61 funções `#[test]` (57 ativas + 4 helpers `#[ignore]`) | Inclui testes com `SIGKILL` (`t6`) e rede TCP (`t8`). |
| Teste C++/QML de Diagnóstico Android (`android/diagnostico_teste/`) | 4 arquivos (`rodar.sh`, `teste.cpp`, 3 `.qml`) | 3 cenários (7 asserções) | Compila e roda binário Qt6 com `-platform offscreen`. |
| Scripts Python E2E de UI Desktop (`app/testes/telas_*.py`) | 8 scripts Python | 8 suítes Xvfb (~95 verificações visuais/SQL) | Exigem binário GUI `rustinho` compilado + `Xvfb`, `xdotool`, `import`/`convert`. |
| Verificador de Identidade Visual (`identidade/gerar_icones.py`) | 1 script Python | 17 arquivos verificados (`--conferir`) | Compara SHA-256 da fonte e bytes dos 16 PNGs + 1 ICO. |

### 2.2. Registro de Execução Real nesta Auditoria (Comandos, Ambiente e Resultados)

> **Isolamento garantido:** Nenhuma execução escreveu na árvore `/home/renan/Área de trabalho/Rustinh0`. Como o cache local `/home/renan/.cargo/registry` continha `zip-2.4.2` em vez de `zip-9.1.3` e o sandbox opera sem rede (`--offline`), as suítes Cargo foram executadas em cópia efêmera `/tmp/rustinho-test-runner` com `nucleo/` 100% inalterado e shim de compatibilidade `/tmp/zip-9.1.3` para `ZipFile::name() -> ZipResult<&str>`, removendo todos os temporários de `/tmp` ao final.

| # | Comando Executado | Escopo | Resultado Obtido |
|---:|---|---|---|
| 1 | `cargo fmt --all --check` | Formatação de todo o workspace (`nucleo`, `app`) | **PASSOU (`exit 0`)** — 0 divergências de formatação. |
| 2 | `CARGO_TARGET_DIR=/tmp/rustinho-audit-target cargo clippy --offline --locked -p rustinho-nucleo --all-targets` | Tentativa direta com `--locked --offline` | **BLOQUEADO PELO CACHE LOCAL (`exit 101`)** — `Cargo.lock` pede `zip 9.1.3`, mas `~/.cargo/registry` local possui apenas `zip-2.4.2`. |
| 3 | `cargo clippy --manifest-path /tmp/rustinho-test-runner/Cargo.toml --offline -p rustinho-nucleo --all-targets -- -D clippy::correctness -D clippy::suspicious` | Análise estática Clippy em todos os alvos de `rustinho-nucleo` | **PASSOU (`exit 0`)** — 0 erros `correctness`/`suspicious`; apenas 1 warning de estilo (`clippy::chunks_exact_to_as_chunks` em `nucleo/src/codigo.rs:58`). |
| 4 | `bash android/diagnostico_teste/rodar.sh /tmp/rustinho-diag-teste` | Suíte C++/Qt6 do detector de tela cinza do Android (`offscreen`) | **PASSOU (`exit 0` — `TUDO OK`)** — 7/7 verificações aprovadas (`[cinza]`, `[conteudo]`, `[quebrado]`). |
| 5 | `cargo test --manifest-path /tmp/rustinho-test-runner/Cargo.toml --offline -p rustinho-nucleo --test caixas --test contratos --test contratos_telas --test fluxo --test portatil` | Todas as 5 suítes de integração (`caixas`, `contratos`, `contratos_telas`, `fluxo`, `portatil`) | **PASSOU 100% (`exit 0`)** — `caixas`: 9/9 ok; `contratos`: 4/4 ok; `contratos_telas`: 8/8 ok; `fluxo`: 5/5 ok; `portatil`: 1/1 ok (1 helper `ignored`). Total: **27/27 aprovados**. |
| 6 | `cargo test --manifest-path /tmp/rustinho-test-runner/Cargo.toml --offline -p rustinho-nucleo --test estavel` (+ reexecução sequencial `--test-threads=1` de `t8_sincronizacao`) | Suíte completa de estabilidade (`a1_auditoria`, `d09_custos`, `t1_backup`..`t8_sincronizacao`) | **PASSOU 100% (`57/57 ativos ok`, 4 helpers `ignored`)** — Em execução paralela massiva (57 testes simultâneos), `t8_1` e `t8_2` sofreram timeout de 5s (`os error 11`) por contenção de CPU no Argon2id debug; executados com `--test-threads=1`, `t8_1`, `t8_2` e `t8_filho_puxa` passaram (`3/3 ok`). |
| 7 | `cargo test --manifest-path /tmp/rustinho-test-runner/Cargo.toml --offline -p rustinho-nucleo --features ancoras-de-teste nfe_assinatura::testes` e `--test estavel t7_importacao` | Suíte criptográfica PKI / ICP-Brasil / C14N / XMLDSig com `ancoras-de-teste` | **PASSOU 100% (`exit 0`)** — **35/35 ativos aprovados** em `nfe_assinatura::testes` (0,18s) e **3/3 aprovados** em `t7_importacao`. |
| 8 | `cargo test --manifest-path /tmp/rustinho-test-runner/Cargo.toml --offline -p rustinho-nucleo --lib` (+ reexecução individual `--test-threads=1` dos 15 testes de rede) | Todos os 408 testes unitários ativos de `rustinho-nucleo` | **402/408 aprovados (`98,5%`)**: 392 passaram de primeira em paralelo + 10 testes de rede passaram com `--test-threads=1`. Os **6 restantes** falharam exclusivamente pelas restrições de rede do namespace isolado do sandbox (`bind` em `127.0.0.2:47810` e broadcast UDP `255.255.255.255:47811` sem rota externa). |
| 9 | `cargo run ... --example estoque_teste /tmp/rustinho-estoque-ex.db` e `--example popular /tmp/rustinho-popular-ex.db` | Execução dos 2 exemplos de `nucleo/examples/` | **FALHARAM (`panic!`)** — Comprovado em runtime o defeito `NEW-B18-01` (ambos chamam comandos de `NAO_REPETIR` sem `chaveOperacao`). |
| 10 | `python3 identidade/gerar_icones.py --conferir` | Verificação dos ícones gerados | **FALHOU (`exit 1`)** — Reportou `DIFERENTE` nos dois PNGs `512x512` (`instalador/rustinho.png` e `hicolor/512x512/apps/rustinho.png`), embora `ImageChops.difference` comprove que 100% dos pixels são idênticos (`bbox: None`), divergindo apenas na compressão zlib do Pillow 3.14 (`NEW-B18-03`). |
| 11 | Scripts Python `app/testes/telas_*.py` | 8 scripts E2E Xvfb | **NÃO EXECUTADOS (bloqueio de ambiente offline)** — Exigem o binário GUI `target/debug/rustinho` compilado com `cxx-qt` (`cxx-build-1.0.205` e `clap-4.6.7` ausentes do cache offline local de `~/.cargo`). Auditados integralmente por inspeção estática. |

---

## 3. Novos Achados Descobertos no Bloco 18

### `NEW-B18-01` — Exemplos `nucleo/examples/estoque_teste.rs` e `nucleo/examples/popular.rs` entram em pânico ao rodar porque não enviam `"chaveOperacao"` nos comandos de `NAO_REPETIR`
- **Classificação:** **Defeito comprovado**
- **Severidade:** Baixa (afeta apenas os dois binários de exemplo em `nucleo/examples/`, não o binário de produção)
- **Arquivos e linhas:**
  - `nucleo/examples/estoque_teste.rs:14–31, 41, 56, 70, 128, 134, 149`
  - `nucleo/examples/popular.rs:5–11, 23, 27, 32, 39, 44, 49, 54, 59, 62, 65, 77`
- **Evidência literal (`nucleo/examples/estoque_teste.rs:14–31`):**
  ```rust
  fn chamar(s: &Sessao, cmd: &str, dados: serde_json::Value) -> serde_json::Value {
      serde_json::from_str(&s.chamar(cmd, &dados.to_string())).unwrap()
  }
  ...
      let r = chamar(
          &s,
          "config_inicial",
          json!({"lojaNome": "Auto Peças Teste", "gerenteNome": "Carlos", "gerenteSenha": "1234"}),
      );
      println!("config_inicial: {}", r["ok"]);
      let r = chamar(&s, "sessao_entrar", json!({"nome": "Carlos", "senha": "1234"}));
      println!("entrar: {}", r["ok"]);

      let r = chamar(
          &s,
          "produto_salvar",
          json!({
              "nome": "Óleo Motor 5W30 Sintético 1L",
  ...
      let prod_oleo = r["id"].as_str().unwrap().to_string();
  ```
- **Evidência de execução real:**
  - `estoque_teste`: `thread 'main' panicked at nucleo/examples/estoque_teste.rs:30:70: called Option::unwrap() on a None value` — porque `"config_inicial"` e `"produto_salvar"` pertencem à lista `NAO_REPETIR` (`nucleo/src/api.rs:64–110`) e, desde a `0.9.15` (`FALHA-04`), são recusados com `{"ok": false, "erro": "Falta a chave da operação...", "campo": "chave"}` quando chamados sem `"chaveOperacao"`.
  - `popular`: `thread 'main' panicked at nucleo/examples/popular.rs:18:70: called Option::unwrap() on a None value` (e, mesmo com sessão aberta, entra em `panic!("{c}: {r}")` na linha 8 ao chamar `"usuario_salvar"` na linha 23 sem `"chaveOperacao"`).

---

### `NEW-B18-02` — Nenhum dos 8 testes E2E de interface (`app/testes/telas_*.py`) é executado no pipeline de CI (`.github/workflows/ci.yml`)
- **Classificação:** **Melhoria** (lacuna de cobertura automatizada em CI)
- **Severidade:** Baixa / Média
- **Arquivos e linhas:**
  - `.github/workflows/ci.yml:1–52`
  - `app/testes/telas_d09.py`, `telas_d14_portatil.py`, `telas_falha07.py`, `telas_modo_seguro.py`, `telas_navegacao_etapa1.py`, `telas_navegacao_guiada.py`, `telas_notificacoes.py`, `telas_rvi02.py`
- **Análise técnica:**
  - O workflow `.github/workflows/ci.yml` já instala o Qt 6.8 no Ubuntu e compila o binário desktop (`cargo build --workspace`), mas encerra sem executar nenhum dos 8 scripts `app/testes/telas_*.py` nem o teste de diagnóstico Android (`bash android/diagnostico_teste/rodar.sh`).
  - Adicionalmente, 4 dos 8 scripts (`telas_d09.py`, `telas_d14_portatil.py`, `telas_falha07.py`, `telas_modo_seguro.py` e `telas_rvi02.py`) compartilham o display Xvfb fixo `TELA = ":79"` (`telas_d09.py:33`), portanto só podem rodar em série, nunca em paralelo.

---

### `NEW-B18-03` — `identidade/gerar_icones.py --conferir` compara bytes PNG comprimidos em vez de pixels decodificados, gerando falso-positivo entre versões de Pillow/zlib
- **Classificação:** **Risco** (fragilidade de reprodutibilidade entre ambientes)
- **Severidade:** Baixa
- **Arquivo e linhas:** `identidade/gerar_icones.py:54–57, 81–88`
- **Evidência literal (`identidade/gerar_icones.py:81–88`):**
  ```python
          if conferir:
              try:
                  atual = open(destino, "rb").read()
              except FileNotFoundError:
                  atual = b""
              if atual != dados:
                  print(f"DIFERENTE: {rel}")
                  diferentes += 1
  ```
- **Evidência de execução real:**
  - Ao executar `python3 identidade/gerar_icones.py --conferir` com Python 3.14.8, o script retornou `exit 1` acusando `DIFERENTE: instalador/rustinho.png` e `DIFERENTE: instalador/icones/hicolor/512x512/apps/rustinho.png`.
  - A verificação pixel a pixel (`ImageChops.difference(f, p).getbbox()`) retornou `None` (100% dos 262.144 pixels RGBA são idênticos); a diferença ocorre exclusivamente no tamanho do stream zlib produzido por `im.save(b, "PNG", optimize=True)` entre diferentes versões de Pillow/zlib.

---

### `NEW-B18-04` — Teste unitário `rede_local::testes::t05_erro_de_uma_interface_nao_impede_as_outras_e_registra_uma_vez` falha em máquinas offline sem rota de broadcast IPv4
- **Classificação:** **Risco** (teste dependente de estado de rede do host)
- **Severidade:** Baixa
- **Arquivo e linhas:** `nucleo/src/rede_local.rs:1005–1034`
- **Evidência de execução real:**
  - `assertion left == right failed: [(203.0.113.77, "Cannot assign requested address (os error 99)"), (0.0.0.0, "Network is unreachable (os error 101)")] left: 2 right: 1`.
  - O teste assume que o envio UDP broadcast por `0.0.0.0 -> 255.255.255.255:47811` sempre terá sucesso na máquina onde `cargo test` roda. Em qualquer ambiente offline ou container/namespace sem interface externa com rota de broadcast ativa, `0.0.0.0` também falha com `ENETUNREACH (os error 101)`, fazendo `erros.len()` ser `2` em vez de `1`.

---

### `NEW-B18-05` — Disputa de portas fixas (`47810`/`47811`) e timeout de Argon2id quando os testes de rede de `rustinho-nucleo` rodam em paralelo
- **Classificação:** **Risco** (flakiness em execução paralela de `cargo test`)
- **Severidade:** Baixa / Média
- **Arquivos e linhas:**
  - `nucleo/src/testes_supervisao.rs:118–145` (`rvi02_porta_tcp_ocupada_aparece_e_para_depois_do_limite`)
  - `nucleo/src/testes_modo_seguro.rs:169–235` (`a3_sem_troca_de_dados_com_banco_com_defeito`)
  - `nucleo/src/testes_loja.rs:212–655`
  - `nucleo/tests/estavel/t8_sincronizacao.rs:25–155`
- **Evidência de execução real:**
  - Na execução padrão paralela (`cargo test -p rustinho-nucleo`), 10 testes de rede em `--lib` e 2 em `--test estavel` falharam por timeout de leitura de socket de 5s (`Resource temporarily unavailable (os error 11)`) ou colisão na porta `47810` (`PORTA_TCP`).
  - Reexecutados individualmente com `--test-threads=1`, **todos os 12 passaram de imediato (`ok`)**, comprovando que os testes que abrem `MotorSync` na porta padrão `47810` ou executam múltiplas derivações Argon2id (`64 MiB`, 3 iterações em perfil debug não otimizado) competem entre si quando agendados simultaneamente pelo test runner do Rust.

