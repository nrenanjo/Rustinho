# Plano de Exploração, Especificação e Auditoria Independente — Rustinh0 (v0.9.17)

> Criado e revisado pelo Reversa em 2026-11-02
> Marque cada tarefa com ✅ quando concluída.

---

## 🔒 Regra Inegociável de Imutabilidade e Escopo de Escrita

1. **Proibição absoluta de alterar o código-fonte:** **Nenhum arquivo do código-fonte** (`.rs`, `.qml`, `.cpp`, `.h`, `.sql`, `.py`, `.sh`, `.toml`, `.lock`, `.yml`, `.xml`, `.iss`, etc.) nem qualquer documento ou recurso pré-existente do projeto Rustinho 0.9.17 pode ser alterado, corrigido, refatorado, formatado ou excluído durante a auditoria.
2. **Permissão exclusiva para relatórios e especificações do Reversa:** **Somente relatórios de auditoria e especificações do Reversa** podem ser criados ou modificados, restritos exclusivamente às pastas próprias do Reversa:
   - `_reversa_sdd/` (relatórios da auditoria em `_reversa_sdd/auditoria/` e especificações SDD do Reversa: `code-analysis`, `domain`, `architecture`, `specs`, `review`)
   - `.reversa/` (controle de estado em `state.json`, superfície em `context/surface.json` e progresso neste `plan.md`)

---

## Diretrizes Obrigatórias de Execução por Bloco

1. **Recomeço integral desde o Bloco 0:** nenhum arquivo ou achado anterior é dado como garantido sem nova inspeção direta na árvore atual.
2. **Leitura integral do histórico (`docs/Rustinho-Auditoria-Gemini4/`):** ler `HANDOFF_AUDITORIA_GEMINI_4_ARGON.md`, `INDICE_DOS_RELATORIOS_ORIGINAIS.md` e todos os 29 arquivos em `relatorios_originais/`, preservando todos os achados anteriores sem descartá-los.
3. **Revisão independente de cada achado anterior:** confrontar cada ID (`NOTIF-*`, `NAV-*`, `B6-*`, `B7A-*`, `B7B-*`, `B8A-*`, `B8B1-*`, `B9A1-*`, `B9A2-*`, `B9A2C-*`, `B9B-*` e apontamentos dos Blocos 0–5D2) com o código-fonte atual e reclassificar como:
   - `CONFIRMADO` | `NÃO CONFIRMADO` | `REFUTADO` | `CORRIGIDO NO CÓDIGO ATUAL` | `NÃO APLICÁVEL` | `DEPENDENTE DE TESTE RUNTIME`
4. **Taxonomia estrita de achados (novos e revisados):**
   - **Defeito comprovado:** falha demonstrada no código atual com caminho, símbolo, linhas exatas (`nl -ba`), trecho literal, cenário de disparo e impacto.
   - **Risco:** fragilidade real que depende de condição externa, concorrência, ambiente ou dado específico.
   - **Hipótese:** suspeita que exige validação adicional ou execução runtime.
   - **Melhoria:** oportunidade de clareza, manutenibilidade, desempenho ou ergonomia sem violar requisito funcional imediato.
5. **Separação rigorosa de testes:** contabilizar e relatar em quadros separados:
   - (a) Código lido
   - (b) Testes encontrados/escritos no repositório
   - (c) Testes efetivamente executados nesta auditoria (com comando somente-leitura, ambiente, saída e resultado) ou bloqueados/não executados (com motivo técnico explícito).
6. **Trabalho em blocos pequenos com persistência:** a cada bloco concluído, gravar o relatório detalhado em `_reversa_sdd/auditoria/`, atualizar as especificações do Reversa em `_reversa_sdd/` e registrar o checkpoint em `.reversa/state.json` e `.reversa/plan.md`.

---

## Fase 1: Reconhecimento, Acervo Histórico e Auditoria Documental (Blocos 0 a 5D2) 🔍

### Bloco 0 — Recepção, Integridade, Inventário Geral e Leitura do Handoff
- [ ] **Scout / Auditoria** — Inventariar a árvore completa do repositório Rustinho 0.9.17 (contagem de arquivos por linguagem/pasta, hashes SHA-256 dos artefatos-chave, verificação de ferramentas e toolchain disponíveis no ambiente).
- [ ] **Scout / Auditoria** — Ler integralmente `docs/Rustinho-Auditoria-Gemini4/HANDOFF_AUDITORIA_GEMINI_4_ARGON.md` e `docs/Rustinho-Auditoria-Gemini4/INDICE_DOS_RELATORIOS_ORIGINAIS.md`, conferindo os hashes SHA-256 dos 29 relatórios originais.
- [ ] **Scout / Auditoria** — Ler os relatórios originais referentes ao Bloco 0 e reconciliar limites de acesso (o que está presente na árvore atual vs. referências históricas a `rus-rus.zip` e PDF de 257 páginas).
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_00_INTEGRIDADE_E_INVENTARIO.md` e `.reversa/context/surface.json`.

### Bloco 1 — Inventário Documental e Reconciliação do Acervo do Projeto
- [ ] **Arqueólogo / Auditoria** — Ler integralmente os relatórios originais do Bloco 1 em `docs/Rustinho-Auditoria-Gemini4/relatorios_originais/`.
- [ ] **Arqueólogo / Auditoria** — Auditar e reconciliar a documentação da raiz (`README.md`, `MANUAL.md`, `PENDENCIAS.md`, `LICENSE`) e todos os 22 documentos em `docs/*.md` (`AUDITORIA-0.9.14.md`, `AUDITORIA-FISCAL-C8.md`, `AVALIACAO-FISCAL-esqueleto.md`, `DEFEITOS.md`, `DIRETRIZ-CENTRAL.md`, `DRE-gerencial.md`, `FISCAL-assinatura-arquitetura.md`, `INTEGRACOES_EXTERNAS.md`, `MATRIZ-homologacao-C8.md`, `PEND-09-criterios.md`, `PEND-09-pacote-acs.md`, `PEND-10-criterios.md`, `PLANO-fiscal-e-plataformas.md`, `PROPOSTA-D1-R01-protocolo-4.md`, `PROPOSTA-R02-0915L-cancelamentos.md`, `R-01-proposta-senha-da-rede.md`, `R-05-criterios.md`, `RASTREABILIDADE-auditorias.md`, `REFERENCIA-nfe-e-boleto.md`, `REVISAO-SINTEC-triagem.md`, `ROTEIRO-linux-completo.md`, `SIMULTANEIDADE-requisitos.md`, `TESTE-0.9.13-linux.md`, `TESTES-estabilidade.md`).
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_01_ACERVO_DOCUMENTAL.md`.

### Blocos 2, 3 e 4 — Padrão Geral, Unidades, Notificações e Navegação Contextual (Documental)
- [ ] **Arqueólogo / Auditoria** — Ler os relatórios originais dos Blocos 2, 3 e 4 e revalidar os contratos documentais:
  - Bloco 2: identidade visual, componentes QML, campo de busca e regras das 10 unidades em 2 linhas (top 5 por produtos ativos, top 5 por vendas líquidas em janela móvel de 24h com devoluções vinculadas, substituição posicional 10→6 no desktop, ordenação alfabética e sem repetição).
  - Bloco 3: distinção entre avisos persistentes e rápidos, categorias de gravidade e comportamento na tela inicial.
  - Bloco 4: matriz de navegação contextual guiada, linkagem de telas, foco e abandono/retomada de guias.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_02_A_04_REQUISITOS_DOCUMENTAIS.md`.

### Blocos 5A a 5D2 — Documentação Fiscal, Arquitetura e Catálogo Completo de Achados Históricos
- [ ] **Arqueólogo / Auditoria** — Ler os relatórios originais dos Blocos 5A a 5D2 (documentação da crate fiscal, allowlist CFOP/CST/CSOSN, rateio de desconto, gates pendentes e limites do Apêndice H) e confrontar com a documentação fiscal presente em `docs/`.
- [ ] **Arqueólogo / Auditoria** — Ler todos os demais relatórios originais e retificações dos Blocos 6 a 9B em `docs/Rustinho-Auditoria-Gemini4/relatorios_originais/` para consolidar o catálogo mestre de achados históricos.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_05_DOCUMENTACAO_FISCAL_E_CATALOGO_HISTORICO.md`.

---

## Fase 2: Reauditoria Independente dos Blocos 6 a 9B + Componentes Correlatos 🏗️

### Bloco 6 — Notificações, Avisos e Tela Inicial
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `nucleo/src/avisos.rs`, `nucleo/src/testes_avisos.rs`, `app/qml/Notificacao.qml`, `app/qml/IconeFuncao.qml`, `app/qml/Aviso.qml`, `app/qml/TelaInicio.qml`.
- [ ] **Revisão de Achados** — Auditar independentemente `NOTIF-01`, `NOTIF-02`, `NOTIF-03`, `B6-01` a `B6-08` e buscar novos achados.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_06_NOTIFICACOES_E_TELA_INICIO.md`.

### Bloco 7A — Navegação Contextual Guiada e Requisitos (Parte 1)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `nucleo/src/requisitos.rs`, `nucleo/src/testes_nav.rs`, `app/qml/Main.qml`, `app/qml/Guia.qml`, `app/qml/ImportarCadastros.qml`, `app/qml/ConflitoCnpj.qml`.
- [ ] **Revisão de Achados** — Auditar independentemente `B7A-01` a `B7A-05`, `NAV-01`, `NAV-02` e buscar novos achados.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_07A_NAVEGACAO_E_REQUISITOS.md`.

### Bloco 7B — Configurações, Compras, Importação XML e Campos
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `nucleo/src/importar.rs`, `nucleo/src/compras.rs`, `app/qml/TelaConfiguracoes.qml`, `app/qml/TelaCompras.qml`, `app/qml/TelaCompra.qml`, `app/qml/LinhaCompra.qml`, `app/qml/LinhaItemCompra.qml`, `app/qml/TelaFiscal.qml`, `app/qml/PedirGerente.qml`, `app/qml/Campo.qml`.
- [ ] **Revisão de Achados** — Auditar independentemente `NOTIF-03`, `B7B-01` a `B7B-04`, `NAV-01`, `NAV-02`, `B6-05`, uso de `abandonarGuia` e código morto apontado.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_07B_CONFIG_COMPRAS_IMPORTACAO.md`.

### Bloco 8A — API Central, Despacho de Comandos e Autorização do Gerente
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura) `nucleo/src/api.rs` (todas as 4.550 linhas e 32 testes embutidos).
- [ ] **Revisão de Achados** — Auditar independentemente `B8A-01a/b/c`, escopo/consumo de tokens de liberação do gerente, múltiplos efeitos em mesmo payload, `gerente_liberar` e contratos de comandos.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_08A_API_E_PERMISSOES.md`.

### Bloco 8B1 — Busca, Produtos, Códigos e Unidades (Núcleo)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `nucleo/src/busca.rs`, `nucleo/src/produtos.rs`, `nucleo/src/unidades.rs`, `nucleo/src/codigo.rs`, `nucleo/src/gtin.rs`, `nucleo/src/faixas.rs`, `nucleo/src/etiquetas.rs`.
- [ ] **Revisão de Achados** — Auditar independentemente `B8B1-01` a `B8B1-12` e buscar novos achados.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_08B1_BUSCA_PRODUTOS_UNIDADES_NUCLEO.md`.

### Bloco 8B2 — Interface de Busca, Peças, Unidades e Estoque
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `nucleo/src/estoque.rs`, `app/qml/EscolherUnidade.qml`, `app/qml/TelaPecas.qml`, `app/qml/TelaPeca.qml`, `app/qml/EscolherPeca.qml`, `app/qml/LinhaPeca.qml`, `app/qml/LinhaAplicacao.qml`, `app/qml/LinhaEquivalente.qml`, `app/qml/LinhaPreco.qml`, `app/qml/LinhaVeiculo.qml`, `app/qml/TelaEtiquetas.qml`.
- [ ] **Revisão de Achados** — Auditar independentemente os fechamentos/confirmações de `B8B1-06`, `B8B1-07`, `B8B1-09`, `B8B1-10` na interface e buscar novos achados.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_08B2_INTERFACE_PECAS_UNIDADES_ESTOQUE.md`.

### Bloco 9A1 — Núcleo de Vendas, Devoluções, Caixa, Encomendas e Reposição
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `nucleo/src/caixa.rs`, `nucleo/src/vendas.rs`, `nucleo/src/devolucoes.rs`, `nucleo/src/encomendas.rs`, `nucleo/src/reposicao.rs`, `nucleo/examples/estoque_teste.rs`, `nucleo/tests/caixas.rs`, `nucleo/tests/estavel/t2_estoque.rs`.
- [ ] **Revisão de Achados** — Auditar independentemente `B9A1-01` a `B9A1-09` e buscar novos achados.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_09A1_NUCLEO_VENDAS_CAIXA_DEVOLUCOES.md`.

### Bloco 9A2 e 9A2 Complementar — Telas Desktop de Estoque, Vendas, Caixa, Trocas e Compras
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura): `app/qml/AjusteEstoque.qml`, `app/qml/LinhaVenda.qml`, `app/qml/LinhaCarrinho.qml`, `app/qml/TelaCaixa.qml`, `app/qml/TelaDevolucao.qml`, `app/qml/TelaEstoque.qml`, `app/qml/TelaVenda.qml`, `app/qml/SecaoComprar.qml`, `app/qml/SecaoEncomendas.qml`, `app/qml/SecaoParados.qml`, `app/qml/SecaoVendidos.qml`, `app/qml/TelaTrocas.qml`, `app/qml/TelaVender.qml`, `app/qml/TelaOrcamentos.qml`.
- [ ] **Revisão de Achados** — Auditar independentemente `B9A2-01` a `B9A2-05`, `B9A2C-01` a `B9A2C-06` e reflexos de `B9A1-*` na UI.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_09A2_TELAS_VENDAS_CAIXA_TROCAS.md`.

### Bloco 9B — Telas Responsivas Compartilhadas Desktop/Android
- [ ] **Arqueólogo / Auditoria** — Auditar o comportamento móvel (`Tema.celular`, `Tema.estreito`, `Tema.fino`, `Tema.baixo`) nas 12 telas compartilhadas do Bloco 9A2/9A2C.
- [ ] **Revisão de Achados** — Auditar independentemente `B9B-01` a `B9B-09` e os 14 casos herdados reportados para Android.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_09B_RESPONSIVIDADE_MOBILE.md`.

---

## Fase 3: Auditoria Completa dos Módulos Não Cobertos Anteriormente (Blocos 10 a 18) 🧠

### Bloco 10 — Financeiro, Contas a Receber/Pagar, Cheques, Comissões e DRE (Núcleo + UI)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Núcleo: `nucleo/src/receber.rs` (incluindo verificação da hipótese pendente `B9A1-07`), `nucleo/src/financeiro.rs`, `nucleo/src/pagar.rs`, `nucleo/src/cheques.rs`, `nucleo/src/comissoes.rs`, `nucleo/src/dre.rs`, `nucleo/src/dre/testes.rs`.
  - QML Financeiro/Receber: `app/qml/TelaFinanceiro.qml`, `app/qml/TelaReceber.qml`, `app/qml/SecaoCheques.qml`, `app/qml/SecaoClientes.qml`, `app/qml/SecaoComissoes.qml`, `app/qml/SecaoContas.qml`, `app/qml/SecaoDre.qml`, `app/qml/SecaoFluxo.qml`, `app/qml/SecaoFornecedores.qml`, `app/qml/SecaoIndicadores.qml`, `app/qml/SecaoPagamentos.qml`, `app/qml/SecaoPagar.qml`, `app/qml/SecaoResumo.qml`, `app/qml/PainelCheques.qml`, `app/qml/DetalheDre.qml`, `app/qml/MetasDre.qml`, `app/qml/ClassificarCategorias.qml`, `app/qml/LinhaContaPagar.qml`, `app/qml/LinhaDevedor.qml`, `app/qml/LinhaFluxo.qml`, `app/qml/LinhaParcela.qml`, `app/qml/LinhaRanking.qml`.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_10_FINANCEIRO_RECEBER_PAGAR_DRE.md`.

### Bloco 11 — Fiscal, NF-e XML, Assinatura Digital ICP-Brasil e Impressão (Núcleo + UI)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Núcleo: `nucleo/src/fiscal.rs`, `nucleo/src/nfe_xml.rs`, `nucleo/src/nfe_assinatura/mod.rs`, `nucleo/src/nfe_assinatura/ancoras.rs`, `nucleo/src/nfe_assinatura/c14n.rs`, `nucleo/src/nfe_assinatura/testes.rs`, `nucleo/src/impressao.rs`, `nucleo/dados/icp-brasil/*`, `exemplos/nfe_exemplo.xml`.
  - QML Fiscal: `app/qml/EmissaoFiscal.qml`, `app/qml/LinhaRegraSt.qml` (em conjunto com `TelaFiscal.qml`).
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_11_FISCAL_NFE_ASSINATURA_IMPRESSAO.md`.

### Bloco 12 — Sincronização, Rede Local, Computador e Modo Portátil (Núcleo + UI)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Núcleo: `nucleo/src/sync.rs`, `nucleo/src/rede_local.rs`, `nucleo/src/computador.rs`, `nucleo/src/portatil.rs`, `nucleo/src/teste_rede.rs`.
  - QML: `app/qml/TelaJuntar.qml` e fluxos de sincronização/rede.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_12_SYNC_REDE_COMPUTADOR_PORTATIL.md`.

### Bloco 13 — Banco de Dados, Migrações SQL (0001 a 0023), Backup e Arquivos
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Núcleo: `nucleo/src/banco.rs`, `nucleo/src/migracao.rs`, `nucleo/src/backup.rs`, `nucleo/src/arquivos.rs`, `nucleo/src/arquivos_teste_rar.rs`, `nucleo/src/caminhos.rs`.
  - SQL: todas as 23 migrações em `nucleo/src/migracoes/0001_inicial.sql` a `0023_rascunho_e_pendencias.sql`.
  - QML: `app/qml/SecaoBackup.qml`.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_13_BANCO_MIGRACOES_BACKUP_ARQUIVOS.md`.

### Bloco 14 — Segurança, Usuários, Senhas, Supervisão, Registro e Auditoria (Núcleo + UI)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Núcleo: `nucleo/src/usuarios.rs`, `nucleo/src/senhas.rs`, `nucleo/src/supervisao.rs`, `nucleo/src/registro.rs`, `nucleo/src/auditoria.rs`.
  - QML: `app/qml/TelaEntrar.qml`, `app/qml/TelaBoasVindas.qml`, `app/qml/TelaUsuarios.qml`, `app/qml/PerguntaMotivo.qml`.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_14_SEGURANCA_USUARIOS_SUPERVISAO_AUDITORIA.md`.

### Bloco 15 — Pessoas, Oficina, Painel, Loja, Configuração e Utilitários Base (Núcleo + UI)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Núcleo: `nucleo/src/pessoas.rs`, `nucleo/src/oficina.rs`, `nucleo/src/painel.rs`, `nucleo/src/loja.rs`, `nucleo/src/config.rs`, `nucleo/src/documento.rs`, `nucleo/src/numeros.rs`, `nucleo/src/texto.rs`, `nucleo/src/tempo.rs`, `nucleo/src/erro.rs`, `nucleo/src/menu.rs`, `nucleo/src/lib.rs`.
  - QML Pessoas, Oficina, Painel e Componentes Base restantes: `app/qml/TelaPessoas.qml`, `app/qml/TelaPessoa.qml`, `app/qml/LinhaPessoa.qml`, `app/qml/TelaOrdens.qml`, `app/qml/TelaOrdem.qml`, `app/qml/TelaPainel.qml`, `app/qml/Painel.qml`, `app/base/Api.qml`, `app/base/Tema.qml`, `app/qml/Aparencia.qml`, `app/qml/AreaTexto.qml`, `app/qml/Balao.qml`, `app/qml/Botao.qml`, `app/qml/Cabecalho.qml`, `app/qml/CampoNumero.qml`, `app/qml/CartaoFuncao.qml`, `app/qml/CartaoNumero.qml`, `app/qml/Chip.qml`, `app/qml/Dialogo.qml`, `app/qml/Etapas.qml`, `app/qml/Faixa.qml`, `app/qml/GraficoBarras.qml`, `app/qml/Icone.qml`, `app/qml/Linha.qml`, `app/qml/Marcar.qml`, `app/qml/Nota.qml`, `app/qml/Olho.qml`, `app/qml/Pilulas.qml`, `app/qml/Rotulo.qml`, `app/qml/Selecao.qml`, `app/qml/TextoErro.qml`, `app/qml/Titulo.qml`.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_15_PESSOAS_OFICINA_PAINEL_COMPONENTES_BASE.md`.

### Bloco 16 — Ponte Rust/C++/Qt Desktop (`app/`) e Camada Nativa Android (`android/`)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Desktop bridge: `app/Cargo.toml`, `app/build.rs`, `app/src/main.rs`, `app/src/ponte.rs`, `app/cpp/janela.cpp`, `app/cpp/janela.h`, `app/cpp/pdf.cpp`, `app/cpp/pdf.h`, `app/qml_android/LeitorCamera.qml`, `app/fontes/preparar_fontes.py`.
  - Android: `android/CMakeLists.txt`, `android/main.cpp`, `android/vistacamera.cpp`, `android/vistacamera.h`, `android/diagnostico.cpp`, `android/diagnostico.h`, `android/DIAGNOSTICO.md`, `android/testar.sh`, `android/assinatura/LEIA.md`, `android/pacote/AndroidManifest.xml`, `android/pacote/res/*`, `android/diagnostico_teste/*`, `android/rust/Cargo.toml`, `android/rust/build.rs`, `android/rust/src/lib.rs`, `android/rust/src/ponte.rs`.
  - Comparação exata (`diff` somente-leitura) entre `app/{qml,base,cpp,fontes}` e `android/rust/{qml,base,cpp,fontes}` para comprovar igualdade ou detectar divergências entre árvores desktop e Android.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_16_PONTE_CPP_DESKTOP_E_ANDROID.md`.

### Bloco 17 — Instalador, CI/CD, Scripts de Build e Identidade
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Raiz e build: `Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `compilar.sh`.
  - CI/CD: `.github/workflows/montar.yml`, `.github/workflows/montar-teste-windows.yml`, `.github/workflows/android.yml`, `.github/scripts/resumir_testes.py`.
  - Instalador e Identidade: `instalador/rustinho.iss`, `instalador/rustinho.desktop`, `instalador/appimage/DependenciasDoAppImage.qml`, `instalador/portatil/*`, `identidade/gerar_icones.py`, `identidade/LEIA.md`.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_17_INSTALADOR_CICD_BUILD.md`.

### Bloco 18 — Suíte Completa de Testes (Unitários, Integração, Estabilidade, PKI e Python UI)
- [ ] **Arqueólogo / Auditoria** — Inspecionar integralmente (somente leitura):
  - Módulos de teste em `nucleo/src/`: `testes_0915.rs`, `testes_chave.rs`, `testes_loja.rs`, `testes_modo_seguro.rs`, `testes_r03.rs`, `testes_rvi03.rs`, `testes_supervisao.rs`.
  - Testes de integração e estabilidade em `nucleo/tests/`: `contratos.rs`, `contratos_telas.rs`, `fluxo.rs`, `portatil.rs`, `apoio/chave.rs`, `apoio/cnpj.rs`, `estavel/main.rs`, `estavel/comum.rs`, `estavel/a1_auditoria.rs`, `estavel/d09_custos.rs`, `estavel/t1_backup.rs`, `estavel/t3_crediario.rs`, `estavel/t4_dre.rs`, `estavel/t5_permissoes.rs`, `estavel/t6_recuperacao.rs`, `estavel/t7_importacao.rs`, `estavel/t8_sincronizacao.rs`, `pki/*.py`, `pki/*.xml`, `examples/popular.rs`.
  - Testes Python de UI em `app/testes/`: `telas_d09.py`, `telas_d14_portatil.py`, `telas_falha07.py`, `telas_modo_seguro.py`, `telas_navegacao_etapa1.py`, `telas_navegacao_guiada.py`, `telas_notificacoes.py`, `telas_rvi02.py`.
- [ ] **Execução e Registro Separado de Testes** — Executar a suíte de testes possível no ambiente sem alterar nenhum arquivo do código-fonte, registrando comandos, saídas e resultados separadamente dos testes apenas lidos/escritos.
- [ ] **Relatório e Spec Reversa** — Gerar `_reversa_sdd/auditoria/BLOCO_18_AUDITORIA_E_EXECUCAO_DE_TESTES.md`.

---

## Fase 4: Interpretação, Especificações SDD e Consolidação Final 📝✅

### Bloco 19 — Especificações SDD do Reversa e Relatório Consolidado Final da Auditoria
- [ ] **Detetive / Arquiteto / Redator** — Consolidar em `_reversa_sdd/` as especificações executáveis do sistema (`domain/`, `architecture/`, `specs/` e matrizes C4/ERD/RBAC/rastreabilidade) a partir das evidências verificadas nos Blocos 0 a 18.
- [ ] **Revisor / Consolidação** — Consolidar a Matriz de Revisão de todos os achados anteriores do Gemini 4 Argon (status final de cada ID, linhas atualizadas, evidências).
- [ ] **Revisor / Consolidação** — Consolidar todos os novos achados (separados em Defeitos Comprovados, Riscos, Hipóteses e Melhorias).
- [ ] **Revisor / Consolidação** — Consolidar o quadro geral de cobertura de arquivos e linhas auditadas (Produção, Testes, Documentação e Infraestrutura) e o quadro separado de Testes Escritos vs. Testes Executados.
- [ ] **Relatório Final** — Gerar `_reversa_sdd/auditoria/RELATORIO_FINAL_AUDITORIA_0_9_17.md` e `_reversa_sdd/review/confidence-report.md`.
