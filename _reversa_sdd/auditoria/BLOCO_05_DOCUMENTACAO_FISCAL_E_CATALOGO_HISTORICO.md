# BLOCO 05 (5A a 5D2) — Documentação Fiscal (`rustinho-fiscal v0.2.3` vs. `docs/`) e Catálogo Mestre de Achados Históricos

> **Fase do Plano:** Fase 1 — Reconhecimento, Acervo Documental e Requisitos Históricos
> **Status:** CONCLUÍDO ✅
> **Data:** 2026-11-02
> **Modo de Operação:** Somente Auditoria (Regra de Imutabilidade Absoluta ativa: `allowLegacyEdits: false`)

---

## 1. Escopo e Leitura Integral dos 29 Relatórios Originais do Gemini 4 Argon

Com a conclusão deste bloco, **100% do histórico em `docs/Rustinho-Auditoria-Gemini4/` foi lido na íntegra**, incluindo:
- `docs/Rustinho-Auditoria-Gemini4/HANDOFF_AUDITORIA_GEMINI_4_ARGON.md` (337 linhas);
- `docs/Rustinho-Auditoria-Gemini4/INDICE_DOS_RELATORIOS_ORIGINAIS.md` (74 linhas);
- Todos os **29 arquivos** em `docs/Rustinho-Auditoria-Gemini4/relatorios_originais/` (`Texto colado(20261031-143316).txt` até `Texto colado(20261101-214051).txt`, totalizando 964.611 bytes conferidos por SHA-256 no Bloco 0).

Especificamente para os **Blocos 5A, 5B, 5C, 5D1 e 5D2**, foram lidos e confrontados os relatórios originais:
1. `Texto colado(20261031-175537).txt` (#6 — Ajustes Bloco 4 + Bloco 5A);
2. `Texto colado(20261031-180246).txt` (#7 — Ajustes Bloco 5A + Bloco 5B);
3. `Texto colado(20261031-181453).txt` (#8 — Revisão Bloco 5B);
4. `Texto colado(20261031-182054).txt` (#9 — Consolidação Capítulos 1–12 até Bloco 5B);
5. `Texto colado(20261031-225337).txt` (#10 — Ajustes finais Bloco 5B + Bloco 5C);
6. `Texto colado(20261031-225742).txt` (#11 — Ajuste Bloco 5C + Bloco 5D1);
7. `Texto colado(20261031-230307).txt` (#12 — Ajuste Bloco 5D1 + Bloco 5D2);
8. `Texto colado(20261101-040237).txt` (#13 — Ajuste Bloco 5D2 + Bloco 6).

---

## 2. Distinção Arquitetural Crítica: Crate Externo `rustinho-fiscal v0.2.3` vs. Repositório Principal `Rustinho 0.9.17`

O confronto entre os relatórios dos Blocos 5A–5D2 do Gemini 4 Argon e a árvore real do repositório `/home/renan/Área de trabalho/Rustinh0` estabelece uma distinção fundamental de engenharia reversa:

| Aspecto | Crate Externo `rustinho-fiscal v0.2.3` (Auditado nos Blocos 5A–5D2 do PDF histórico) | Repositório Principal `Rustinho 0.9.17` (Nosso Workspace Atual) |
|---|---|---|
| **Localização** | Crate isolado (`v0.2.3`, `2026-04-12`), cujos 11 arquivos `.md` estavam nas páginas 54–69 do PDF histórico e cujos 8 arquivos `.rs` estavam nas páginas 96–257 (inacessíveis ao Gemini 4). **Não está presente no workspace atual.** | Workspace `/home/renan/Área de trabalho/Rustinh0` (`Cargo.toml` com members `["nucleo", "app"]`, mais `android/rust`). |
| **Escopo Fiscal Implementado** | Protótipo de adaptador de emissão de NFC-e (modelo 65) e NF-e (modelo 55) via `ACBrMonitorPLUS` (`ENT.TXT` / `SAI.TXT`), geração de INI (`ini_builder.rs`), pré-validação fiscal (`validator.rs`), máquina de estados de emissão (`state_machine.rs`) e guarda de caminhos (`security.rs`). | **Recepção e assinatura XML local:** `nucleo/src/nfe_assinatura/` (7 arquivos `.rs`: `mod.rs`, `c14n.rs`, `cadeia.rs`, `certificado.rs`, `icp_brasil.rs`, `xmldsig.rs`, `testes.rs`) e `nucleo/src/nfe_xml.rs` (importação de XML de compra com verificação criptográfica RSA-SHA1/SHA256 e cadeia ICP-Brasil v5/v10/v11/v12), além da UI de configuração `app/qml/TelaFiscal.qml` e tabelas/colunas fiscais em `nucleo/migrations/` e `nucleo/src/fiscal.rs`. **Não emite NF-e/NFC-e na SEFAZ** (`PEND-15` / `C8-05`). |
| **Integração entre ambos** | Nenhuma (`0` chamadas entre o Rustinho principal e `rustinho-fiscal`). | Confirmado em `PENDENCIAS.md:31` (`PEND-15`) e `docs/AUDITORIA-FISCAL-C8.md:107-115` (`C8-05`): a emissão SEFAZ permanece fora do escopo operacional atual da versão 0.9.17. |

---

## 3. Revisão e Preservação Integral dos Achados dos Blocos 5A a 5D2 (`rustinho-fiscal v0.2.3` vs. `docs/`)

### 3.1. Inventário dos 11 Documentos do Crate `rustinho-fiscal v0.2.3` Auditados no Histórico

| Bloco | Documento no PDF Histórico | Páginas PDF | Status da Revisão Independente | Síntese Técnica Preservada |
|---|---|---|---|---|
| **5A** | `rustinho-fiscal/README.md` | 54 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Declara `v0.2.3` como **NÃO PRONTO PARA EMISSÃO FISCAL EM PRODUÇÃO**. Enumera 8 módulos planejados (`domain.rs`, `validator.rs`, `ini_builder.rs`, `state_machine.rs`, `acbr.rs`, `sefaz_codes.rs`, `security.rs`, `lib.rs`) e 5 bloqueadores de produção (testes reais em homologação SEFAZ, qualificação ACBrMonitorPLUS, revisão contábil por UF, testes de carga/timeout, persistência transacional no app hospedeiro). |
| **5A** | `rustinho-fiscal/CHANGELOG.md` | 54–56 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Evolução `0.2.0` → `0.2.1` → `0.2.2` → `0.2.3`: remoção de `f64` da fronteira monetária (uso de `MoneyCents(i64)` e `RateBps(u32)`), algoritmo `apportion_sale_discount_cents` com conservação exata da soma em centavos, sanitização de INI (`sanitize_ini_value`), guarda de caminhos (`safe_join_path`), parser multi-seção de `SAI.TXT` e limpeza oportunista de INIs temporários (`> 72h`). |
| **5B** | `rustinho-fiscal/docs/ESCOPO-FISCAL-V0.2.md` | 56–57 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Define a *allowlist* técnica de apenas **4 combinações CFOP/CST/CSOSN** (`SimplesNacional + 102 + 5102`, `SimplesNacional + 500 + 5405`, `RegimeNormal + 00 + 5102`, `SimplesExcessoSublimite + 00 + 5102`) e **12 lacunas fiscais bloqueadas** (RTC `IBS/CBS/IS`, `IPI`, frete/seguro/outras despesas, interestadual/DIFAL, importação/exportação, benefícios `cBenef`/desoneração/FCP, monofásico combustíveis, contingência `tpEmis != 1`, NF-e complementar/ajuste/devolução, `tPag=90`). |
| **5B** | `rustinho-fiscal/docs/PLANO-HOMOLOGACAO.md` | 57–58 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Roteiro de 6 fases, 10 cenários de homologação SEFAZ (`H01` a `H10`, incluindo `H05` timeout/queda de rede e `H06` `cStat=539/204`) e 6 critérios de aceite. |
| **5B** | `rustinho-fiscal/docs/RELATORIO-TESTES-LOCAL.md` | 58–59 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Declara explicitamente que `rustc` e `cargo` **não estavam instalados** na sandbox onde `rustinho-fiscal v0.2.3` foi gerado (`0` testes Rust executados; apenas checagem léxica Python e simulação exaustiva Python de `5.659.572` combinações de rateio de desconto). |
| **5C** | `rustinho-fiscal/docs/ARQUITETURA.md` | 60–61 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Descreve os 8 módulos, a política de tipos numéricos (`MoneyCents(i64)`, `RateBps(u32)`, `Quantity` com `raw: u64` e `scale: 0..=4`), a máquina de estados de 10 estados (`DRAFT` a `CANCELLED`, incluindo `UNKNOWN_REQUIRES_QUERY`) e o fluxo de 9 passos do `AcbrMonitorAdapter::emit`. |
| **5C** | `rustinho-fiscal/docs/DECISOES-TECNICAS.md` | 61–62 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Formaliza 8 ADRs (`ADR-001` a `ADR-008`), incluindo bloqueio de segunda chamada `NFe.CriarEnviarNFe` após timeout (`ADR-003`), escrita de INI com `create_new(true)` + rename para `.TMP` + limpeza `> 72h` (`ADR-004`) e rejeição de caracteres de controle/metacaracteres INI (`ADR-005`). |
| **5D1** | `rustinho-fiscal/docs/INTEGRACAO-ACBR.md` | 62–64 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Especifica os 5 comandos ACBrMonitorPLUS (`ATIVAR`, `CriarEnviarNFe`, `ConsultarNFe`, `CancelarNFe`, `StatusServico`), geração de INI NFe 4.00 e parser de `SAI.TXT`. Revela que `AcbrMonitorAdapter::query_protocol` exige caminho de XML local dentro de `xml_authorized_roots` para executar `NFe.ConsultarNFe`. |
| **5D1** | `rustinho-fiscal/docs/MANUAL-OPERACIONAL.md` | 64–66 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Procedimento em 4 etapas para resolver `UNKNOWN_REQUIRES_QUERY` sem duplicidade de numeração e política de guarda de XML autorizados/cancelados por **5 anos + ano corrente** (Art. 173, I do CTN e Ajuste SINIEF 07/05). |
| **5D2** | `rustinho-fiscal/docs/MATRIZ-PROVEDORES.md` | 66–67 | `CONFIRMADO NO HISTÓRICO` / `CONFLITO ARQUITETURAL CONFIRMADO COM DOCS NATIVOS` | Compara `ACBrMonitorPLUS`, `ACBrLibNFe`, Rust Nativo e API SaaS. Documenta que `ACBrMonitorPLUS` roda apenas em **x86_64 Desktop** (exigindo `Xvfb` + GTK2/Qt em Linux headless) e **não roda em Android (`ARM64`/`ARMv7`)**. |
| **5D2** | `rustinho-fiscal/docs/SEGURANCA.md` | 68–69 | `CONFIRMADO NO HISTÓRICO` / `NÃO APLICÁVEL AO WORKSPACE PRINCIPAL (CRATE EXTERNO)` | Documenta superfície de ataque e 3 limitações residuais: janela TOCTOU entre `safe_join_path` e `fs::read_to_string`/`File::open`, ausência de configuração programática de ACLs Windows em diretórios de INI temporário com PII (CPF/CNPJ/endereço) e limpeza oportunista (`> 72h`) dependente de nova chamada `emit()`. |

### 3.2. Confronto Direto entre os Achados dos Blocos 5A–5D2 e a Arquitetura Fiscal Nativa do Repositório `Rustinho 0.9.17`

Ao cruzar a auditoria documental de `rustinho-fiscal v0.2.3` com os 4 documentos fiscais nativos do repositório (`docs/FISCAL-assinatura-arquitetura.md`, `docs/AVALIACAO-FISCAL-esqueleto.md`, `docs/PLANO-fiscal-e-plataformas.md`, `docs/AUDITORIA-FISCAL-C8.md`) e com a árvore de código de `nucleo/src/`, confirmam-se 4 conclusões técnicas:

1. **Convergência de Escopo sobre `PEND-15` / `C8-05`:**
   - Tanto `docs/AUDITORIA-FISCAL-C8.md` quanto os relatórios dos Blocos 5A–5D2 confirmam que o Rustinho 0.9.17 **não realiza emissão fiscal eletrônica em produção** (não transmite lotes SOAP para a SEFAZ nem integra `rustinho-fiscal`).
2. **Divergência Tecnológica de Provedor (`ACBrMonitorPLUS` vs. Rust Puro Multiplataforma):**
   - Enquanto o protótipo externo `rustinho-fiscal v0.2.3` apostou em `ACBrMonitorPLUS` via troca de arquivos `ENT.TXT`/`SAI.TXT` (que não funciona em Android ARM64 e exige Xvfb em Linux), o plano arquitetural nativo do repositório (`docs/FISCAL-assinatura-arquitetura.md` e `docs/PLANO-fiscal-e-plataformas.md`) escolheu **Rust puro sem OpenSSL e sem dependências externas** para funcionar identicamente em Windows, Linux e Android.
3. **Módulos Fiscais Reais Presentes no Repositório Principal (Não Auditados pelo Gemini 4 Argon):**
   - O repositório `Rustinho 0.9.17` possui em `nucleo/src/` três módulos fiscais reais que nunca foram abertos pelo Gemini 4 Argon:
     - `nucleo/src/nfe_assinatura/` (`mod.rs`, `c14n.rs`, `cadeia.rs`, `certificado.rs`, `icp_brasil.rs`, `xmldsig.rs`, `testes.rs`) — implementação própria em Rust puro de Canonical XML 1.0, XMLDSig (`rsa-sha1` e `rsa-sha256`), leitura de certificado A1 PKCS#12 (`.pfx`) e validação da cadeia ICP-Brasil (v5, v10, v11, v12);
     - `nucleo/src/nfe_xml.rs` — parser e verificador criptográfico de XML de NF-e de compra (`ler_verificada`), utilizado por `nucleo/src/compras.rs`;
     - `nucleo/src/fiscal.rs` — cadastro e validação da configuração fiscal da loja (regime tributário, ambiente, série, CSC, certificado A1, NCM/CFOP/CSOSN/CST).
   - **Ação garantida no Plano:** Todos esses módulos serão auditados integralmente no código-fonte real no **Bloco 11 (Fase 3)**.
4. **Riscos Arquiteturais do Protótipo `rustinho-fiscal v0.2.3` Preservados como Requisitos Preventivos:**
   - Caso uma futura implementação de emissão SEFAZ venha a incorporar partes de `rustinho-fiscal v0.2.3`, permanecem registrados como bloqueadores técnicos:
     - Omissão de `MEI` e de `RegimeNormal + CST 60 + CFOP 5405` (substituição tributária cobrada anteriormente, essencial em autopeças!) na allowlist de `validator.rs`;
     - Exigência de arquivo XML local em `query_protocol` durante a reconciliação de `UNKNOWN_REQUIRES_QUERY` quando o timeout ocorreu antes da gravação do XML;
     - Janela TOCTOU em `safe_join_path` e retenção de PII em arquivos `.INI.PROCESSED.TMP` por até 72h.

---

## 4. Catálogo Mestre Consolidado de Todos os Achados do Gemini 4 Argon (Blocos 6 a 9B)

Para cumprir rigorosamente os Objetivos 3 e 6 do plano aprovado, listamos abaixo **todos os 62 achados técnicos** documentados pelo Gemini 4 Argon ao longo dos Blocos 6, 7A, 7B, 8A, 8B1, 8B2, 9A1, 9A2, 9A2 Complementar e 9B. Nenhum achado foi descartado. Cada um será verificado independentemente no código-fonte real do workspace nos Blocos 6 a 9B da Fase 2.

### 4.1. Achados do Bloco 6 (Notificações, Avisos e Tela Inicial)

| ID Histórico | Classificação Original (Gemini 4) | Alvo no Código (`0.9.17`) | Resumo do Achado a Revalidar no Bloco 6 |
|---|---|---|---|
| **NOTIF-01** | Defeito Comprovado | `nucleo/src/avisos.rs:132` vs. `:302, 344, 381` | `acao_padrao` mapeia `"conflitos" \| "codigos" \| "codigos_bloqueio"` para `"Resolver no Gerente"` em vez de `"Resolver em Duplicatas"`, apesar de `.secao("duplicatas")`. |
| **NOTIF-02** | Corrigido na 0.9.17 | `nucleo/src/avisos.rs:133` | Na 0.9.17, `"outra_loja" => "Ver caixas"` já está alinhado com `.secao("simultaneidade")`. |
| **NOTIF-03** | Defeito Comprovado | `app/qml/TelaInicio.qml:16` + `app/qml/TelaConfiguracoes.qml:135-141, 471-505` | Operador `Caixa` que clica em notificação de Simultaneidade e cancela `PedirGerente` permanece em `TelaConfiguracoes` (`secao = "loja"`) em vez de voltar à tela anterior. |
| **B6-01** | Defeito Comprovado | `app/qml/Aviso.qml:10` | Posicionamento vertical no rodapé (`y: parent.height - height - 30`) em vez do topo com `24 px`. |
| **B6-02** | Defeito Comprovado | `nucleo/src/avisos.rs:780-783` + `app/qml/Notificacao.qml:43` | `so_do_modo_seguro` retorna `do_modo_seguro(conn, "modo_seguro")` sem chamar `completar(&mut lista)`, deixando `titulo_longo`, `resumo` e `acao` vazios (`""`) e ocultando o botão de ação no modo seguro. |
| **B6-03** | Defeito Comprovado | `app/qml/TelaInicio.qml:31-37` | `esconderAviso(chave)` não remove o aviso de `tela.avisos` após `r.ok === true`, exigindo sair e voltar à tela para sumir. |
| **B6-04** | Defeito Comprovado | `nucleo/src/avisos.rs:747-750` + `app/qml/Notificacao.qml:114` | `curto()` escapa `&`, `<`, `>` em HTML para `"backup"` e `"backup_nuvem"` (`ATENCAO`), mas `Notificacao.qml:114` renderiza `ATENCAO` com `Text.PlainText`, exibindo entidades literais (`&amp;`). |
| **B6-05** | Defeito Comprovado (vs. doc) | `app/qml/Campo.qml:151-201` | Desenha lupa à esquerda e `"F11"` interno à direita nos campos de busca, divergindo da especificação documental de colar/copiar e `"F11"` externo. |
| **B6-06** | Melhoria Proposta | `nucleo/src/avisos.rs:143, 609, 769` | Rótulo genérico `"Abrir"` para `"troca_parada"`, `"troca_tentando"` e `"troca_voltou"`, e ausência do prefixo `"Simultaneidade ›"` no `resumo` de `"troca_tentando"` e `"troca_voltou"`. |
| **B6-07** | Risco (Injeção `StyledText`) | `app/qml/Notificacao.qml:58-65, 114` | Função `linha()` concatena `a.resumo` ou `a.texto` sem escapar `<`, `>`, `&` quando `Text.StyledText` é usado para `INFORMATIVA`. |
| **B6-08** | Hipótese (Fuso UTC vs. Local) | `nucleo/src/avisos.rs:617-623` | `dias_desde` usa `julianday('now')` (UTC) sobre strings `YYYY-MM-DD`, podendo divergir entre 21h00 e 23h59 BRT. |

### 4.2. Achados dos Blocos 7A e 7B (Navegação Contextual C8, Guias, Configurações, Compras e Importação)

| ID Histórico | Classificação Original (Gemini 4) | Alvo no Código (`0.9.17`) | Resumo do Achado a Revalidar nos Blocos 7A e 7B |
|---|---|---|---|
| **B7A-01** | Defeito Comprovado | `app/qml/Main.qml:422-431` | `Shortcut { sequence: "F11" }` global não verifica `!janela.guiaAtiva`, empilhando `TelaPecas` por cima da tela-alvo durante uma guia ativa sem encerrar a guia. |
| **B7A-02** | Risco de Estado Órfão | `app/qml/Main.qml:145-169` + `app/qml/TelaConfiguracoes.qml:49` | `inicio()` e `trocarPara()` não limpam `guiaAtiva` diretamente; além disso, `Component.onDestruction` em `TelaConfiguracoes.qml:49` referencia `janela` (`ApplicationWindow.window`), que é `null` quando o item já foi desparentado do `StackView`. |
| **B7A-03** | Risco / Melhoria | `app/qml/ImportarCadastros.qml:17-29, 70-89` | `Popup` sem `Flickable`/limite de altura para telas baixas e `depoisDeImportar` usa `avisar(r.erro, "erro")` em vez de `guiar` caso receba `requisito`. |
| **B7A-04** | Melhoria Proposta | `app/qml/Main.qml:350-367` | `faixaModoSeguro` no topo é estática (sem clique para `TelaBackup`). |
| **B7A-05** | Melhoria Proposta | `nucleo/src/testes_nav.rs:14-31` | Diretórios temporários em `std::env::temp_dir()` não são removidos ao final dos 20 testes. |
| **NAV-01** | Defeito Comprovado | `app/qml/TelaConfiguracoes.qml:562-572` + `app/qml/TelaFiscal.qml:67` | Acionar `janela.guiar(r)` de dentro de `TelaFiscal.qml` (embutida via `Loader` na aba `"fiscal"` de `TelaConfiguracoes.qml`) empilha uma **segunda** `TelaConfiguracoes` na `pilha` e, ao voltar (`voltar()`), não recarrega os dados da primeira instância. |
| **NAV-02** | Defeito Comprovado | `app/qml/TelaConfiguracoes.qml:513-530` + `app/qml/Guia.qml:60-67` | Trocar manualmente de aba nas `Pilulas` de `TelaConfiguracoes.qml` durante uma guia ativa oculta a seção do campo-alvo (`alvo.visible == false`), fazendo o recorte escuro sumir enquanto a faixa superior da guia fica presa sem destaque. |
| **NAV-03 / B7B-02** | Defeito Comprovado | `app/qml/TelaCompras.qml:47, 62-63, 90-92` vs. `nucleo/src/compras.rs:157` | Variável morta `let deOutra = 0` na importação em lote de XMLs e mensagem obsoleta dizendo `"O Rustinho ainda não verifica a assinatura digital do XML"` (sendo que `compras::importar_xml` na linha 157 chama `nfe_xml::ler_verificada`). |
| **NAV-04 / B7B-03** | Risco de Contorno Interno | `nucleo/src/compras.rs:75-86` vs. `nucleo/src/api.rs:1509` | A trava `exigir_cnpj_da_loja` em compras está em `api.rs:1509` (`"compra_importar"`) e `compras::importar_xml` (`177`), mas `compras::conferir_nota` (`75-86`) não chama `exigir_cnpj_da_loja` diretamente. |
| **B7B-01** | Defeito Comprovado | `app/qml/TelaConfiguracoes.qml:123-148, 492-504` | Quando um operador `Caixa` cancela o modal inicial `PedirGerente` em uma guia e depois clica em `botaoSenhaGerente` (`liberar()`, linhas 143–148), `liberar()` define `liberado = true` mas não chama `mostrarCampoDaGuia()`. |
| **B7B-04** | Risco de UX | `app/qml/TelaConfiguracoes.qml:169-216` | Se `config_salvar` falhar em outro campo (ex.: `loja` vazio) durante uma guia apontando para `loja.cnpj`, `salvar()` move o foco para `loja` mantendo o recorte da guia em `lojaCnpj`. |

### 4.3. Achados do Bloco 8A (`nucleo/src/api.rs` — Despachante, Sessões, Permissões e Transações)

| ID Histórico | Classificação Original (Gemini 4) | Alvo no Código (`0.9.17`) | Resumo do Achado a Revalidar no Bloco 8A |
|---|---|---|---|
| **B8A-01a** | Defeito Comprovado | `nucleo/src/api.rs:541-546, 940-948, 1016-1028, 2503, 2600, 2840` | `Liberacao::Uma` não armazena `acao`, e `liberada_esta_chamada` ignora o parâmetro `acao` de `exigir` e só consome o token ao final de `executar_comando` (`1016`). Uma única senha de gerente libera múltiplas checagens distintas na mesma chamada (`"configuracoes"` + `"precos"` em `config_salvar`; `"configuracoes"` + `muda_rede` em `sync_salvar`; `"usuarios"` + `mexe_em_gerente` em `usuario_salvar`). |
| **B8A-01b** | Defeito Comprovado | `nucleo/src/api.rs:1027-1029` vs. `:2503, 2600, 2840` | Como `self.exigir(acao, sessao)?` na linha 1028 roda antes do corpo do comando, a 1ª falha retorna apenas a ação básica (`"configurações da loja"` ou `"usuários"`), ocultando do gerente no modal `PedirGerente` que o payload também altera regras de lucro (`"precos"`), senha da rede ou outro gerente. |
| **B8A-01c** | Defeito Comprovado | `nucleo/src/api.rs:149, 1098-1101, 2600` | Em `gerente_liberar`, `nome_da_acao(acao_do_comando(comando).unwrap_or("precos"))` resolve `"config_salvar"` sempre para `Some("configuracoes")`, gravando na auditoria `"Liberou com senha: configurações da loja"` mesmo quando `"configuracoes"` estava liberado para todos e a senha foi exigida exclusivamente por `self.exigir("precos", sessao)?` na linha 2600. |
| **B8A-02** | Melhoria Proposta | `nucleo/src/api.rs:1946-1952` vs. `:1933-1945` | `"aviso_esconder"` executa `avisos::esconder` e `sync::registrar` diretamente em `&self.banco.conn` sem abrir transação explícita `tx`, diferentemente de `"aviso_minimizar"`. |
| **B8A-03** | Hipótese | `nucleo/src/api.rs:1572-1595` (`"compra_lancar"`) | Verificar em `compras::lancar` se o recálculo de preços respeita `preco_travado`. |

### 4.4. Achados dos Blocos 8B1 e 8B2 (Busca, Produtos, Unidades, Estoque e Telas de Peças)

| ID Histórico | Classificação Original (Gemini 4) | Alvo no Código (`0.9.17`) | Resumo do Achado a Revalidar nos Blocos 8B1 e 8B2 |
|---|---|---|---|
| **B8B1-01** | Defeito Comprovado | `nucleo/src/produtos.rs:163-192` + `nucleo/src/busca.rs:150-241` | `texto_busca` não inclui `produto_fornecedor.codigo_fornecedor` nem `produto_codigo_antigo.codigo` na coluna `p.busca`; e `busca.rs` só consulta essas tabelas por igualdade exata da busca inteira (`codigo_compacto = ?1`), falhando em buscas parciais ou combinadas (`"0986af0053 gol"`). |
| **B8B1-02** | Defeito Comprovado | `nucleo/src/busca.rs:51-68, 179-189, 250-284` | `normalizar_termos` não separa fronteiras letra↔dígito; buscar `"rg08"` ou `"rg-08"` gera `"rg08"`, que não casa com `"RG 08"` em `p.busca`. |
| **B8B1-03** | Defeito Comprovado | `nucleo/src/busca.rs:57-59, 69-74, 252-270` | Qualquer número de 4 dígitos em `1900..=2100` é extraído como `ano` veicular (`como_ano`) e removido de `normais`; peças sem aplicação veicular com `"2000"` no nome/descrição são descartadas por `serve_no_ano` quando há outro termo textual (`"parafuso 2000"`). |
| **B8B1-04** | Defeito Comprovado | `nucleo/src/busca.rs:76-82, 179-189` | `parece_codigo` exige `s.len() >= 3` e dígito decimal, impedindo que códigos de 1–2 dígitos ou só de letras acionem o caminho de código quando há mais de 1 termo. |
| **B8B1-05** | Defeito Comprovado | `nucleo/src/unidades.rs:20-31` + `nucleo/src/produtos.rs:1219-1221` | `unidades::listar` e `produtos::sugestoes` contam produtos inativos (`ativo = 0`) e serviços (`tipo = 'servico'`) em `quantos_produtos` e na ordenação. |
| **B8B1-06** | Defeito Comprovado (vs. doc) | `nucleo/src/unidades.rs:13-39` + `app/qml/EscolherUnidade.qml:6-176` | A regra documental das 10 unidades em 2 fileiras (`6 + 4`) com janela móvel de 24h de vendas líquidas e substituição posicional da direita para a esquerda (`10 → 6`) não está implementada. |
| **B8B1-07** | Defeito Comprovado | `nucleo/src/unidades.rs:46-62` + `app/qml/EscolherUnidade.qml:29, 178-246` | `unidades::salvar` usa `ON CONFLICT (sigla) DO UPDATE`, permitindo sobrescrever silenciosamente unidade existente ao criar `"Nova unidade"`. |
| **B8B1-08** | Defeito Comprovado | `nucleo/src/produtos.rs:749-757` vs. `:494, 508` | `produtos::salvar` insere `codigo_antes` em `produto_codigo_antigo` sem verificar `!faixas::esperando_confirmacao(conn, &c)?` (ao contrário de `trocar_codigo`). |
| **B8B1-09** | Defeito Comprovado | `nucleo/src/produtos.rs:403-432, 760-764`, `nucleo/src/estoque.rs:24-97`, `app/qml/TelaPeca.qml:598`, `app/qml/AjusteEstoque.qml:66-76` | Nem o cadastro inicial de estoque, nem `estoque::lancar`/`ajustar`, nem a UI validam `unidades::fracionada`, permitindo gravar estoque fracionário (`0,5 UN`) em unidade indivisível. |
| **B8B1-10** | Melhoria Proposta | `app/qml/TelaPecas.qml:621-657` + `app/qml/TelaPeca.qml:1209-1231` | Exibir aviso explícito caso o operador tente excluir peça com histórico (`podeExcluir == false`) por atalho ou fluxo direto. |
| **B8B1-11** | Risco de Desempenho (N+1) | `nucleo/src/busca.rs:189, 240, 297, 304` | Para cada candidato do `LIMIT`, `busca::buscar` executa até 3 queries adicionais (`aplicacoes_do_produto`, `precos::do_produto`, `precos::oferta_vigente`). |
| **B8B1-12** | Risco Transacional | `nucleo/src/produtos.rs:1010-1089` + `nucleo/src/api.rs:1330-1340` | `produtos::criar_variacao` e `"unidade_salvar"` executam múltiplos comandos SQL e `sync::registrar` sem abrir transação explícita `tx`. |

### 4.5. Achados dos Blocos 9A1, 9A2, 9A2 Complementar e 9B (Vendas, Devoluções, Caixa, Estoque e Responsividade Mobile)

| ID Histórico | Classificação Original (Gemini 4) | Alvo no Código (`0.9.17`) | Resumo do Achado a Revalidar nos Blocos 9A1, 9A2 e 9B |
|---|---|---|---|
| **B9A1-01** | Defeito Comprovado | `nucleo/src/devolucoes.rs:281-308, 548-556` + `app/qml/TelaDevolucao.qml:242-254` | `devolucoes::calcular` e `devolver` (e `TelaDevolucao.qml`) não verificam `unidades::fracionada`, permitindo devolver `0,5 UN` de peça indivisível e somar `+500` milésimos no estoque. |
| **B9A1-02** | Defeito Comprovado | `nucleo/src/vendas.rs:829-854` + `nucleo/src/caixa.rs:305-354, 431-437` | `vendas::cancelar` com sessão de caixa aberta (`!fechado`) não verifica se há saldo suficiente em `esperado_dinheiro` na gaveta (permitindo gaveta negativa após sangria) nem se `sessao_id` pertence ao computador (`dispositivo`) atual. |
| **B9A1-03** | Defeito Comprovado | `nucleo/src/devolucoes.rs:637-657, 859-865` | `devolucoes::desfazer` verifica apenas `caixa::sessao_fechada(conn, d.6.as_deref())?`, sem comparar o dispositivo da sessão `d.6` com o dispositivo onde o desfazimento ocorre. |
| **B9A1-04** | Defeito Comprovado | `nucleo/examples/estoque_teste.rs:5-11, 22, 34, 70-91` | Utilitário de exemplo chama `s.chamar` diretamente para comandos de `api::NAO_REPETIR` sem `"chaveOperacao"`, entrando em `panic!`. |
| **B9A1-05** | Risco de Integridade (R-05) | `nucleo/src/vendas.rs:790` + `nucleo/src/devolucoes.rs:398` | `vendas::cancelar` e `devolucoes::devolver` não chamam `sync::exigir_editavel(conn, "venda", id)` nem `exigir_filhos_editaveis` para `"venda_item"`. |
| **B9A1-06** | Defeito Comprovado (promovido em 9A2C) | `nucleo/src/vendas.rs:415-423, 467-475` + `app/qml/TelaVender.qml:59, 311, 784-794` | O desconto global da venda (`e.desconto` / `descontoTotal`) permite ultrapassar o `desconto_maximo` cadastrado na peça e concluir a venda sem senha de gerente. |
| **B9A1-07** | Hipótese | `nucleo/src/devolucoes.rs:453-485, 611-616` vs. `nucleo/src/receber.rs` | Verificar em `receber::estornar` se o estorno manual bloqueia recebimentos com `forma == "devolucao"`. |
| **B9A1-08** | Melhoria Proposta | `nucleo/src/devolucoes.rs:532-569` + `nucleo/src/vendas.rs:549-787` | Devolução de peça sem defeito (`!l.defeito`) e vendas não gravam na entidade `"produto"` da tabela `auditoria`, não aparecendo no quadro Histórico de `TelaPeca.qml`. |
| **B9A1-09** | Melhoria Proposta | `nucleo/src/caixa.rs`, `estoque.rs`, `vendas.rs`, `devolucoes.rs` | Funções de escrita recebem `&Connection` sem `SAVEPOINT` interno (dependendo da transação de `api.rs`). |
| **B9A2-01** | Defeito Comprovado | `app/qml/TelaDevolucao.qml:54-74, 385-400` | Clicar no chip `"Veio com defeito"` antes de preencher a quantidade (ou antes de `"Devolver tudo"`) não marca `defeito = true` nos itens adicionados depois, fazendo a peça defeituosa voltar ao estoque bom. |
| **B9A2-02** | Defeito Comprovado | `app/qml/AjusteEstoque.qml:45-56` + `app/qml/TelaVenda.qml:312-322` | Cancelar o modal `PedirGerente` exibe o erro `"Só o gerente pode: ..."` dentro de `quantidade.erro` (`AjusteEstoque.qml`) ou `motivo.erro` (`TelaVenda.qml`). |
| **B9A2-03** | Defeito Comprovado | `app/qml/TelaCaixa.qml:310-312, 326-331, 347-351` | `movimento.abrirCom` pré-seleciona `contaMov.indice = 0` (1ª conta da loja em vez de `"Nenhuma conta"`) e atribui erros de `x.campo === "conta"` a `valorMov.erro`. |
| **B9A2-04** | Risco / Omissão Visual | `app/qml/TelaVenda.qml:1-330` | `TelaVenda.qml` não indica se a venda concluída já sofreu devoluções parciais ou totais. |
| **B9A2-05** | Melhoria Proposta | `app/qml/TelaVenda.qml:34-38, 105, 313`, `TelaCaixa.qml:158, 225`, `TelaDevolucao.qml:151` | Passar chave de idempotência em `venda_cancelar`, destacar `esperado` negativo em vermelho, exibir `"Caixas anteriores"` com caixa aberto e remover `&& (true)`. |
| **B9A2C-01** | Defeito Comprovado | `app/qml/TelaVender.qml:914-916, 941-962` | Botão `"Vale-troca"` (`completarVale()`) no modal `receber` lê `calc.falta` (`"0,00"`, pois `"Dinheiro"` já vem preenchido com o total) e preenche `campoVale.texto = "0,00"`. |
| **B9A2C-02** | Defeito Comprovado | `app/qml/SecaoComprar.qml:99-105, 349` + `nucleo/src/api.rs:2099-2102` | `"estoque_sugestao"` e `SecaoComprar.qml` expõem `custoTotal` para operador `Caixa` sem verificar a permissão `"resultados"` (D-09). |
| **B9A2C-03** | Defeito Comprovado | `app/qml/SecaoEncomendas.qml:351-369, 461` | `TextoErro` na linha 461 só exibe erro se `nova_.campoErro === ""`, ocultando erros com `campo` igual a `"descricao"`, `"observacoes"`, `"chave"`, `"repetida"` ou `"modoSeguro"`. |
| **B9A2C-04** | Defeito Comprovado | `app/qml/TelaVender.qml:286-307, 784-794` | `abrirReceber()` não verifica `calculo.erroDesconto` antes de abrir o modal `receber`, e `descontoTotal` não recalcula em `onEditado`. |
| **B9A2C-05** | Defeito Comprovado | `app/qml/TelaVender.qml:56, 1352-1364` | `TelaVender.qml` envia sempre `tabela: 1` e ignora a tabela de preços de orçamentos carregados. |
| **B9A2C-06** | Defeito Comprovado | `app/qml/TelaTrocas.qml:272-291, 300-310` | Aba `"Já devolvidas"` omite o nome do caixa da devolução/venda, mantém `"Desfazer"` ativo quando `valeUsado == true` e diz que peças com defeito saem do estoque. |
| **B9A2C-07** | Melhoria Proposta | `app/qml/TelaVender.qml:1287`, `SecaoVendidos.qml:214`, `SecaoEncomendas.qml:273` | Validar erros antes de abrir `caixaOrcamento`, incluir unidade no chip de `SecaoVendidos.qml` e padronizar callback de `PerguntaMotivo`. |
| **B9B-01** | Defeito Comprovado (Mobile) | `app/qml/TelaVender.qml:11, 207-226, 478, 679` | `limparVenda()` não reseta `verLateral = false`, prendendo o usuário na página lateral vazia no Android após concluir venda, salvar orçamento ou limpar carrinho. |
| **B9B-02** | Defeito Comprovado (Mobile) | `app/qml/TelaVender.qml:877-1206` | `Popup { id: receber }` não possui `Flickable` nem limite de altura e estoura a largura horizontal na linha de Vale-troca (`220 + 150 px` fixos) em celulares. |
| **B9B-03** | Defeito Comprovado (Mobile) | `app/qml/SecaoComprar.qml:270-358` (`cPeca`) | Três colunas laterais fixas (`132 + 150 + 150 = 508 px` com margens) esmagam a coluna do nome da peça para largura zero/negativa em smartphones. |
| **B9B-04** | Defeito Comprovado (Mobile) | `app/qml/TelaTrocas.qml:153-223, 254-333` | Em `Tema.celular === true`, os botões `"Imprimir"`, `"Usar no Vender"` e `"Desfazer"` ficam visíveis na mesma linha horizontal sem quebrar linha, esmagando os textos. |
| **B9B-05** | Defeito Comprovado (Mobile) | `app/qml/TelaDevolucao.qml:150-168, 267-269, 406-428` | Em `Tema.estreito === true`, `TextoErro` e os botões `"Trocar agora"`/`"Devolver"` ficam exclusivamente na aba `"Como devolver"`, invisíveis quando o usuário está na aba `"Peças"`. |
| **B9B-06** | Defeito Comprovado (Mobile) | `app/qml/SecaoEncomendas.qml:300-469` (`Painel { id: nova_ }`) | Modal sem `Flickable` e sem limite de altura estoura a tela do celular ao abrir sugestões `achados` com o teclado virtual aberto. |
| **B9B-07** | Problema de Usabilidade Mobile | `app/qml/Campo.qml:50-56` + chamadas `.focar()` em modais | `Campo.focar()` não foca nem abre o teclado virtual no Android quando `Qt.inputMethod.visible == false`. |
| **B9B-08** | Problema de Usabilidade Mobile | `app/qml/SecaoEncomendas.qml:193-275` (`Row { id: acoesEnc }`) | `Row` sem quebra de linha (`Flow`) pode cortar botões em telas estreitas quando múltiplos botões estão visíveis. |
| **B9B-09** | Problema de Usabilidade Mobile | `SecaoVendidos.qml:70-100`, `TelaTrocas.qml:75-102`, `SecaoComprar.qml:108-139`, `SecaoParados.qml:78-109` | Barras horizontais de filtros e pílulas sem `colunasEstreito: 1` comprimem o campo de busca em telas estreitas. |

---

## 5. Registro de Código Lido, Testes Encontrados e Testes Executados (Bloco 5)

- **Relatórios históricos lidos neste bloco:** 29/29 arquivos de `docs/Rustinho-Auditoria-Gemini4/relatorios_originais/` concluídos (`100%`), mais `HANDOFF_AUDITORIA_GEMINI_4_ARGON.md` e `INDICE_DOS_RELATORIOS_ORIGINAIS.md`.
- **Documentação fiscal nativa confrontada:** `docs/FISCAL-assinatura-arquitetura.md`, `docs/AVALIACAO-FISCAL-esqueleto.md`, `docs/PLANO-fiscal-e-plataformas.md`, `docs/AUDITORIA-FISCAL-C8.md` e `PENDENCIAS.md`.
- **Testes automatizados encontrados na documentação histórica dos Blocos 5A–5D2:** 0 testes executados no crate externo `rustinho-fiscal v0.2.3` (apenas simulação Python de rateio).
- **Testes executados em runtime neste bloco:** **0 testes executados** (etapa documental e de consolidação do catálogo histórico).

