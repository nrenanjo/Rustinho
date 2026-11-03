# BLOCO 01 — Inventário Documental e Reconciliação do Acervo do Projeto

> **Data:** 2026-11-02
> **Escopo:** Revisão independente dos relatórios originais do Bloco 1 do Gemini 4 Argon (`Texto colado(20261031-144516).txt` e ajustes em `Texto colado(20261031-145308).txt`) e auditoria integral da documentação nativa do projeto Rustinho 0.9.17 (`README.md`, `MANUAL.md`, `PENDENCIAS.md`, `LICENSE` e os 24 documentos em `docs/*.md`).
> **Política de Imutabilidade:** 100% somente-leitura sobre os arquivos do projeto legado.

---

## 1. Revisão Independente do Bloco 1 Histórico (Gemini 4 Argon)

### 1.1 O que o Gemini 4 Argon auditou no Bloco 1 original
Nos relatórios `Texto colado(20261031-144516).txt` e `Texto colado(20261031-145308).txt` (ajustes do Bloco 1), o agente anterior auditou o **acervo documental externo de auditoria** recebido em sua sessão (`rus-rus.zip` e `Log_Consolidado_Rustinho_0.9.17.pdf`), que continha logs de sessões com agentes anteriores ("Sintec", "Argon", "Sonnet", "Opus", "Composer") e 34 capturas de tela únicas.

| Apontamento do Gemini 4 Argon (Bloco 1) | Status na Revisão Independente | Evidência / Justificativa Técnica |
|---|---|---|
| **B1-HIST-01:** Distinção entre 19 materiais principais de sessão e 138 entradas totais do `rus-rus.zip` (21 `.md` com 18 únicos, 39 `.log` com 37 únicos, 61 `.png` com 28 únicos + 6 imagens de `.docx` = 34 imagens únicas). | `NÃO APLICÁVEL` (ao workspace atual) / `CONFIRMADO` (no histórico preservado) | Os arquivos brutos `rus-rus.zip` e `Log_Consolidado_Rustinho_0.9.17.pdf` não fazem parte da árvore do repositório `Rustinh0`. Contudo, as transcrições consolidadas pelo Gemini 4 Argon estão integralmente preservadas nos 29 relatórios em `docs/Rustinho-Auditoria-Gemini4/relatorios_originais/`. |
| **B1-HIST-02:** Limitação de leitura das páginas 96 a 257 de `Log_Consolidado_Rustinho_0.9.17.pdf` (onde residiam trechos integrais dos artefatos de especificação da crate `rustinho-fiscal`: `CRATE-FISCAL-0.1.0.md`, `CRATE-FISCAL-REGRAS.md`, `CRATE-FISCAL-TESTES.md` e `NFE-2-ASSINATURA.md`). | `CONFIRMADO` (como limitação documental histórica) | As páginas 96–257 do PDF externo não estão no repositório. Entretanto, o repositório atual contém `docs/FISCAL-assinatura-arquitetura.md`, `docs/AVALIACAO-FISCAL-esqueleto.md`, `docs/PLANO-fiscal-e-plataformas.md`, `docs/AUDITORIA-FISCAL-C8.md` e `docs/MATRIZ-homologacao-C8.md`, que estavam inacessíveis ao Gemini 4 Argon por truncamento do ZIP e agora foram lidos na íntegra nesta auditoria. |
| **B1-HIST-03:** Omissão de `MANUAL.md` e de toda a pasta `docs/*.md` do projeto no Bloco 1 original do Gemini 4 Argon. | `CORRIGIDO NESTA AUDITORIA` | No ambiente do Gemini 4 Argon, o stream Deflate de `rustinho-0.9.17.zip` estava truncado antes de `MANUAL.md` e `docs/`. Na árvore atual em disco, todos os 4 arquivos da raiz e todos os 24 arquivos Markdown em `docs/*.md` estão íntegros e foram lidos integralmente neste Bloco 1 (3.403 linhas totais). |

---

## 2. Inventário Completo e Hashes SHA-256 da Documentação do Projeto (`Raiz` + `docs/*.md`)

Todos os **28 arquivos documentais nativos** do projeto (totalizando **3.403 linhas**) foram lidos do início ao fim:

| # | Caminho do Arquivo | Linhas | SHA-256 | Papel Documental no Projeto |
|---|---:|---:|---|---|
| 1 | `README.md` | 58 | `37205a750de9d2a7325096fa35ef427a02ddfa90a3ad3157ee250188e8e00491` | Apresentação do Rustinho, módulos, arquitetura enxuta e instruções de compilação/testes. |
| 2 | `MANUAL.md` | 336 | `098ca8416ae4426811d9bcc3e39e579e6aa284cce3d429beaa8b30b4a3f820ac` | Manual completo do lojista/balconista (17 capítulos + Apêndice com 68 telas QML). |
| 3 | `PENDENCIAS.md` | 91 | `88ae79f8ac7588aef14c66f2ba32c303ebd331f3d2e4de4283a859178ace299e` | Registro oficial de pendências concluídas (`PEND-01`..`14`, `16`, `19`..`23`, `26`..`28`) e em aberto (`PEND-15`, `17`, `18`, `24`, `25`, `29`..`33`). |
| 4 | `LICENSE` | 21 | `641a062ccf927561e79ec719dda13298a1f135cd6aa337fae4be12249bc9d95f` | Licença MIT (Copyright 2026). |
| 5 | `docs/AUDITORIA-0.9.14.md` | 97 | `5d736f165f5b200b32442685a797a9aff35f8f4983838505c6725c39bdfd96c2` | Auditoria técnica da versão 0.9.14 (`FALHA-01` a `FALHA-07` e `RISCO-01` a `RISCO-07`). |
| 6 | `docs/AUDITORIA-FISCAL-C8.md` | 49 | `d098fe676da6f5984cf36ec38293ec7e160db43c202738b36bae752581ed1a7e` | Parecer de prontidão para substituir o Sintec até 15/12/2026 (**NÃO PRONTO** pela ausência de SEFAZ online). |
| 7 | `docs/AVALIACAO-FISCAL-esqueleto.md` | 229 | `f227936e063b7f0b05780010947cced62479b2a317638e2771c08701190c7580` | Avaliação crítica de propostas externas para o módulo fiscal, rejeitando over-engineering e definindo escopo enxuto. |
| 8 | `docs/DEFEITOS.md` | 28 | `8c4942beed068f85a57df410d11397994d4d38692ba46fb7c1965575cc6e1b71` | Histórico de defeitos `D-01` a `D-18` (todos marcados como corrigidos com testes de regressão). |
| 9 | `docs/DIRETRIZ-CENTRAL.md` | 130 | `e6663c5fa654a7c64d096c21687401292589018f29cda02f3b78c5a529462897` | Diretriz arquitetural inegociável: simplicidade para o balconista, zero siglas contábeis na UI, limites de tamanho por arquivo. |
| 10 | `docs/DRE-gerencial.md` | 124 | `566b9d51504f5367b88b9ac09a9f8bb6463618d2e5de2c58a9ceaaee5ae16add` | Especificação do DRE gerencial por competência, conciliação caixa vs. resultado, 6 indicadores e alertas automáticos. |
| 11 | `docs/FISCAL-assinatura-arquitetura.md` | 171 | `2b69249f9bb0b6cf07bab9d208deb1a4953cb43c202738b36bae752581ed1a7e`* | Arquitetura de `nucleo/src/nfe_assinatura/` (XMLDSig, C14N 1.0, PKCS#12 A1, cadeia ICP-Brasil v5/v10/v11/v12). |
| 12 | `docs/INTEGRACOES_EXTERNAS.md` | 93 | `f7bea9f80c0c6f1a9d452ead1a2fb189a4131d7cade19c9222c08b39adbdea89` | Fronteira explícita: o que existe hoje (UDP LAN, XML local, HTML local) vs. o que não existe (SEFAZ online, CNAB/PIX, nuvem). |
| 13 | `docs/MATRIZ-homologacao-C8.md` | 58 | `f7cec0b241164f0789e146cb5fe658d50cd338dbacef3da4ce91e5ec387edc67` | Matriz de homologação operacional e fiscal (Ciclo 8), separando itens aprovados (`C8-01`, `C8-06`) e pendentes (`C8-02`..`05`, `C8-07`). |
| 14 | `docs/PEND-09-criterios.md` | 333 | `a355c1ccb741d36d8914f556769e44c9d73f6370b49902c421f1f22e29623018` | Critérios objetivos da `PEND-09` (validação de assinatura XMLDSig e cadeia ICP-Brasil na importação de NF-e). |
| 15 | `docs/PEND-09-pacote-acs.md` | 79 | `21ccb646e32583f4e651397d302824bd1ed05441a4630977997de1e48d178dd8` | Rastreabilidade das 15 âncoras ICP-Brasil embutidas em `nucleo/dados/icp-brasil/manifesto.json`. |
| 16 | `docs/PEND-10-criterios.md` | 279 | `5df50cb1e315bdf72464326049307defd436ca1d3e6ffa516c027af6207c08e1` | Critérios objetivos da `PEND-10` (modo somente-leitura para pen drive desatualizado e resolução de divergência de loja). |
| 17 | `docs/PLANO-fiscal-e-plataformas.md` | 256 | `8aec52fb971c7f791935d06d2493fb113ca78c4a16cc6e05fd75103f8262a75a` | Plano em 6 etapas para emissão fiscal completa e empacotamento Windows/Linux/Android. |
| 18 | `docs/PROPOSTA-D1-R01-protocolo-4.md` | 93 | `83b8f0c6f5ace363a956ada56089e01992457be576f47fd96b08114db869f43d` | Proposta de protocolo de rede v4 (KDF + pareamento autorizado pelo gerente + identidade por dispositivo) — **não implementada**. |
| 19 | `docs/PROPOSTA-R02-0915L-cancelamentos.md` | 67 | `e0959e1a8c90d61daa864b33ca8275c4900b19840c0795ecbbf7ab94a0b7416d` | Proposta de lançamentos compensatórios para `R-02` e bloqueio de cascata `0915-L` — **não implementada**. |
| 20 | `docs/R-01-proposta-senha-da-rede.md` | 40 | `c6d621fa19186ac93339dfb998bdd0d022362c6f34647ea129bc25c6f5fc1196` | Proposta inicial para mitigar `R-01` (força bruta offline sobre anúncio UDP 47811). |
| 21 | `docs/R-05-criterios.md` | 99 | `096f0a6e19b3f5c96fcffc811cb32c4969c7a3e1799923f0dfdcb76350feacbe` | Critérios de aceite do modo de manutenção (`R-05`) em falhas de migração ou integridade do SQLite. |
| 22 | `docs/RASTREABILIDADE-auditorias.md` | 165 | `6770835174420b5c495e2304536c666d76c94bafb237e07e4a1b975b404ad967` | Matriz mestre de rastreabilidade de 7 séries de auditorias internas (`F-01`..`24`, `C-01`..`16`, `R-01`..`12`, `FALHA-01`..`07`, `0915-A`..`P`, `RVI-01`..`03`, `C8-01`..`07`). |
| 23 | `docs/REFERENCIA-nfe-e-boleto.md` | 65 | `79b84129dab6a8c35f9a4bfb4b1fafd15ee1584a0bdd27731e912a774ad6e8cb` | Referência de projetos open-source em Rust/Python/PHP para NF-e 4.00 e boletos. |
| 24 | `docs/REVISAO-SINTEC-triagem.md` | 80 | `39bfd652ce9d3e77245b50324e6474055338282856f14ed461e04d86696ec93f` | Triagem da revisão de substituição do Sintec (confirma correções em 0.9.16 e lista lacunas abertas). |
| 25 | `docs/ROTEIRO-linux-completo.md` | 155 | `be46a5db03acded9f28acc893404640fe48f6db0ae5c1e25e9d10685f3f01323` | Roteiro de verificação ponta a ponta em Linux (Etapas 0 a 16). |
| 26 | `docs/SIMULTANEIDADE-requisitos.md` | 96 | `9030b484937e6249127b08f97738573cd550956cb1c7ab813bf96667db1c5177` | Requisitos `A1`..`A5`, `B1`..`B3`, `C1`..`C3` (implementados) e `D1`..`D4` (**pendentes**). |
| 27 | `docs/TESTE-0.9.13-linux.md` | 30 | `2dab1539c4ae7c437a8ee8f29a5db4e579419f794585a544dc47a7b5a57a55d8` | Relatório de execução dos testes na versão 0.9.13 em Linux. |
| 28 | `docs/TESTES-estabilidade.md` | 81 | `f85be3434cdfba025b1b18fa0acaa724a1b11ee3c22560942a92b3195e3cc7ef` | Especificação da suíte `nucleo/tests/estavel.rs` (`T1` a `T8`). |

*(\*Nota: o SHA-256 exato de `docs/FISCAL-assinatura-arquitetura.md` é `2b69249f9bb0b6cf07bab9d208deb1a4953cb43f4525b04d999aaf0e7b7eba15`.)*

---

## 3. Síntese dos Contratos Arquiteturais e Pendências Reconhecidas na Documentação Nativa

### 3.1 Diretrizes Arquiteturais Inegociáveis (`docs/DIRETRIZ-CENTRAL.md`)
1. **Público-alvo:** Balconistas e donos de autopeças pequenas (1 a 3 computadores), sem conhecimento contábil/fiscal.
2. **Linguagem da interface:** Proibido expor siglas fiscais cruas (`CFOP`, `CST`, `CSOSN`, `NCM`, `ICMS-ST`, `PIS`, `COFINS`, `SEFAZ`) nas telas de operação diária; o sistema traduz perguntas humanas simples em códigos fiscais nos bastidores (`TelaFiscal.qml` é a única tela avançada para dados da contabilidade).
3. **Orçamento de tamanho de arquivos (`docs/DIRETRIZ-CENTRAL.md`, linhas 91–97):**
   - Módulo Rust normal: até ~350 linhas.
   - Módulo Rust denso (XML, assinatura, DRE): até ~450 linhas, dividido em submódulos se passar disso.
   - Tela QML: até ~300 linhas.
   - Componente QML reutilizável: até ~120 linhas.
   - *(Observação de auditoria: confrontaremos esses limites com a árvore real nos próximos blocos; por exemplo, `nucleo/src/api.rs` possui 4.550 linhas e `nucleo/src/sync.rs` possui 1.500 linhas).*

### 3.2 Pendências Abertas Documentadas pelo Próprio Projeto em 0.9.17 (`PENDENCIAS.md` + `docs/RASTREABILIDADE-auditorias.md`)

A documentação interna de 0.9.17 reconhece explicitamente **10 pendências abertas** (`PEND-15`, `PEND-17`, `PEND-18`, `PEND-24`, `PEND-25`, `PEND-29`, `PEND-30`, `PEND-31`, `PEND-32`, `PEND-33`):

| ID Pendência | Série / Código Interno | Classificação Preliminar | Descrição Documentada e Arquivos Envolvidos |
|---|---|---|---|
| **PEND-15** | `C8-05` / Bloqueador Sintec | **Risco / Lacuna Funcional Bloqueadora** | O Rustinho 0.9.17 **não transmite NF-e (55) nem NFC-e (65) à SEFAZ**, não gera protocolo de autorização (`protNFe`) e não consulta a SEFAZ online na importação. O prazo da licença do Sintec NFe 4 vence em **15/12/2026** (`docs/AUDITORIA-FISCAL-C8.md`, linhas 7–48). |
| **PEND-17** | Visual Mobile (`LinhaItemCompra.qml`) | **Defeito comprovado (documentado)** | Na tela móvel de compra (`app/qml/LinhaItemCompra.qml`), os botões de ação sobrepõem o texto `"Que peça é esta?"` (`PENDENCIAS.md`, linha 31). Será inspecionado no código no Bloco 7B. |
| **PEND-18** | Android 32-bit (`Moto e6i`) | **Hipótese / Dependente de Runtime** | Tela cinza no aparelho físico Motorola Moto e6i (Android 10, `armeabi-v7a`) mesmo com `QSG_RHI_BACKEND=opengl` (`PENDENCIAS.md`, linhas 33–39). Será analisado no Bloco 16. |
| **PEND-24 (`R-01`)** | Segurança de Rede Local (`rede_local.rs`, `sync.rs`) | **Risco (Segurança)** | Senha da rede de apenas 4 caracteres permite força bruta offline contra o pacote UDP 47811 (`docs/PROPOSTA-D1-R01-protocolo-4.md`). Será auditado no Bloco 12. |
| **PEND-24 (`R-02`)** | Integridade Multi-Caixa (`caixa.rs`, `vendas.rs`, `pagar.rs`, `receber.rs`) | **Defeito comprovado / Risco Operacional** | Cancelar venda, recebimento ou pagamento neste computador quando a sessão original de caixa **ainda está aberta em outro computador** altera o total esperado do outro computador enquanto o dinheiro sai/entra neste (`docs/PROPOSTA-R02-0915L-cancelamentos.md`, linhas 9–15). `D-14` protege apenas sessões **fechadas**. |
| **PEND-24 (`R-03`)** | Simultaneidade Financeira (`sync.rs`, `receber.rs`, `pagar.rs`) | **Risco Operacional** | Pagamento simultâneo da mesma parcela/conta ou uso duplo do mesmo vale em dois caixas offline é **detectado** após sincronização (`pago_a_mais`, `vale_a_mais`), mas exige acerto manual pelo gerente. |
| **PEND-25 (`D1–D4`)** | Controle de Dispositivos (`SIMULTANEIDADE-requisitos.md`) | **Risco (Segurança / Governança)** | Pareamento com aprovação do gerente (`D1`), chave criptográfica por computador (`D2`), revogação de computador (`D3`) e trilha assinada por dispositivo (`D4`) não estão implementados (`docs/SIMULTANEIDADE-requisitos.md`, linhas 67–95). |
| **PEND-29 (`0915-L`)** | Imutabilidade em Cascata (`receber.rs`, `pagar.rs`, `cheques.rs`, `devolucoes.rs`) | **Defeito comprovado (Lógica de Domínio)** | Operações em cascata (`receber::cancelar_da_venda`, `pagar::cancelar_da_compra`, `cheques::cancelar_de`, abatimento de crédito em devolução) não chamam `sync::exigir_editavel`, permitindo mutar registros criados em outro computador (`docs/PROPOSTA-R02-0915L-cancelamentos.md`, linhas 17–21). |
| **PEND-30 (`0915-P`)** | Recuperação de Banco (`caminhos.rs`, `banco.rs`, `api.rs`) | **Defeito comprovado / Risco de Perda de Dados** | Um arquivo `rustinho.db` de `0 bytes` abre como instalação nova (boas-vindas) mesmo quando já existem cópias de segurança na pasta, em vez de acionar o modo de manutenção (`PENDENCIAS.md`, linhas 64–68). Será auditado no Bloco 13. |
| **PEND-31 (`C8-02` / `C8-07`)** | Permissões e Histórico (`unidades.rs`, `api.rs`) | **Defeito comprovado (RBAC / Auditoria)** | `unidade_salvar` altera `fracionada` em unidade existente sem senha do gerente e sem gravar histórico; `fiscal_automatico` pode ser chamado por qualquer operador sem exigir gerente (`PENDENCIAS.md`, linhas 70–75). Será auditado nos Blocos 8A, 8B1 e 11. |
| **PEND-32 (`C8-03`)** | Instalador Windows (`instalador/rustinho.iss`) | **Risco (Empacotamento)** | Desinstalador Inno Setup com `PrivilegesRequired=admin` resolve `{localappdata}` para a conta do administrador, podendo não remover a pasta de dados do operador padrão (`PENDENCIAS.md`, linhas 77–81). Será auditado no Bloco 17. |
| **PEND-33 (`C8-04`)** | Diagnóstico Android (`android/diagnostico.cpp`) | **Risco (Privacidade / Segurança Local)** | Log de diagnóstico Android gravado em `Android/data/<pacote>/files` (armazenamento externo acessível por USB/ADB) em vez de diretório interno privado (`PENDENCIAS.md`, linhas 83–87). Será auditado no Bloco 16. |

### 3.3 Divergência Documental Identificada na Própria Documentação de 0.9.17
- **`0915-J` em `docs/RASTREABILIDADE-auditorias.md` (linha 141) vs. `android/rust/Cargo.toml` (linha 5):**
  - Em `docs/RASTREABILIDADE-auditorias.md` (linha 141), o item `0915-J` consta como `ABERTO` com a descrição `"Versão divergente em android/rust/Cargo.toml (0.9.13 vs 0.9.15)"`.
  - Na inspeção direta de `android/rust/Cargo.toml` (linha 5) realizada no Bloco 0, a versão atual já foi atualizada para `version = "0.9.17"`, alinhada a `nucleo/Cargo.toml` e `app/Cargo.toml`. Portanto, `0915-J` está **CORRIGIDO NO CÓDIGO ATUAL (0.9.17)**, embora a tabela de `docs/RASTREABILIDADE-auditorias.md` não tenha sido atualizada nesse ponto.

---

## 4. Contabilização Separada: Leitura vs. Testes Encontrados vs. Testes Executados

| Categoria | Quantidade | Detalhamento |
|---|---:|---|
| **(a) Documentação lida integralmente neste bloco** | **28 arquivos (3.403 linhas)** + 2 relatórios históricos (`Texto colado(20261031-144516).txt`, `Texto colado(20261031-145308).txt`) | `README.md` (58), `MANUAL.md` (336), `PENDENCIAS.md` (91), `LICENSE` (21) e 24 arquivos em `docs/*.md` (2.897 linhas). |
| **(b) Testes encontrados/descritos na documentação** | **8 suítes de estabilidade (`T1`–`T8`)** + roteiros e matrizes (`ROTEIRO-linux-completo.md`, `MATRIZ-homologacao-C8.md`) | Apenas catalogados documentalmente neste bloco; a inspeção de código dos testes ocorrerá nos Blocos 6, 7A, 8A, 9A1, 10, 11 e 18. |
| **(c) Testes executados neste bloco** | **0** | Bloco estritamente documental; nenhuma execução de teste de código aplicável nesta etapa. |

