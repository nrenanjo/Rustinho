# Inventário Geral do Projeto — Rustinho 0.9.17

> Gerado pelo Scout / Bloco 0 em 2026-11-02
> Raiz auditada: `/home/renan/Área de trabalho/Rustinh0`

---

## 1. Visão Geral da Árvore (636 arquivos do projeto)

> Nota: Excluídos da contagem os diretórios de ferramenta e metadados do agente (`.reversa/`, `_reversa_sdd/`, `.agents/`, `.claude/`, `__pycache__/`, `AGENTS.md`, `CLAUDE.md`).

| Diretório de Topo | Quantidade de Arquivos | Papel Arquitetural |
|---|---:|---|
| `nucleo/` | 220 | Crate `rustinho-nucleo`: regras de negócio, SQLite/migrações (23 SQLs), criptografia, assinatura NF-e ICP-Brasil, sincronização LAN, despacho `api.rs` e suíte de testes unitários/integração/PKI. |
| `android/` | 172 | Empacotamento CMake + Qt6 para Android (`main.cpp`, `vistacamera.cpp/.h`, `diagnostico.cpp/.h`, `AndroidManifest.xml`) e crate estática `android/rust/` com espelho de `qml/`, `base/`, `cpp/` e `fontes/`. |
| `app/` | 153 | Crate binária desktop `rustinho`: ponte `cxx-qt` (`src/main.rs`, `src/ponte.rs`), C++ (`janela.cpp/.h`, `pdf.cpp/.h`), 100 telas/componentes em `app/qml/`, 1 em `app/qml_android/`, 2 singletons em `app/base/` e 8 scripts Python de teste em `app/testes/`. |
| `docs/` | 55 | 22 documentos técnicos e de requisitos do próprio Rustinho (`docs/*.md`) + pasta histórica `docs/Rustinho-Auditoria-Gemini4/` (Handoff, Índice e 29 relatórios originais). |
| `instalador/` | 18 | Scripts e manifestos de distribuição Linux (`.desktop`, AppImage QML), Windows (Inno Setup `rustinho.iss`), modo portátil e ícones `hicolor`. |
| `(raiz)` | 10 | `Cargo.toml`, `Cargo.lock`, `README.md`, `MANUAL.md`, `PENDENCIAS.md`, `LICENSE`, `compilar.sh`, `rustfmt.toml`, `.gitignore`, `.gitattributes`. |
| `.github/` | 4 | Workflows de CI/CD (`montar.yml`, `montar-teste-windows.yml`, `android.yml`) e `scripts/resumir_testes.py`. |
| `identidade/` | 3 | `gerar_icones.py`, `LEIA.md`, `rustinho-icone-original.png`. |
| `exemplos/` | 1 | `nfe_exemplo.xml`. |

---

## 2. Distribuição por Extensão de Arquivo

| Extensão | Arquivos | Observação |
|---|---:|---|
| `.qml` | 210 | 100 em `app/qml/`, 1 em `app/qml_android/`, 2 em `app/base/`, 100 em `android/rust/qml/`, 1 em `android/rust/qml_android/`, 2 em `android/rust/base/`, 3 em `android/diagnostico_teste/`, 1 em `instalador/appimage/`. |
| `.rs` | 96 | 69 em `nucleo/src/` (incluindo `dre/testes.rs` e 4 em `nfe_assinatura/`), 2 em `nucleo/examples/`, 19 em `nucleo/tests/`, 3 em `app/` (`build.rs`, `main.rs`, `ponte.rs`), 3 em `android/rust/` (`build.rs`, `lib.rs`, `ponte.rs`). |
| `.xml` | 76 | `AndroidManifest.xml`, `file_paths.xml`, `exemplos/nfe_exemplo.xml`, `nucleo/tests/nfe_exemplo.xml` e 72 XMLs de teste em `nucleo/tests/pki/`. |
| `.txt` | 40 | Inclui os 29 relatórios originais em `docs/Rustinho-Auditoria-Gemini4/relatorios_originais/`, licenças de fontes, `hashsha512x.txt` e textos do modo portátil. |
| `.md` | 33 | 3 na raiz (`README.md`, `MANUAL.md`, `PENDENCIAS.md`), 22 em `docs/*.md`, 2 em `docs/Rustinho-Auditoria-Gemini4/`, 2 em `android/`, 1 em `identidade/`, 1 em `instalador/portatil/`, mais READMEs auxiliares. |
| `.sql` | 23 | Migrações numeradas `0001_inicial.sql` até `0023_rascunho_e_pendencias.sql` em `nucleo/src/migracoes/`. |
| `.py` | 16 | 8 em `app/testes/`, 4 em `nucleo/tests/pki/`, 2 `preparar_fontes.py`, 1 em `identidade/gerar_icones.py`, 1 em `.github/scripts/resumir_testes.py`. |
| `.cpp` / `.h` | 18 | 10 `.cpp` e 8 `.h` em `app/cpp/`, `android/` e `android/rust/cpp/`. |
| `.c14n` / `.der` | 26 | 16 `.c14n` e 10 `.der` em `nucleo/tests/pki/`. |
| `.ttf` / `.woff2` / `.png` / `.ico` | 76 | Fontes tipográficas e ícones de interface/instalador. |
| `.toml` / `.lock` / `.yml` / `.sh` / `.iss` / `.desktop` / `.ini` / `.zip` | 19 | Manifesto Cargo, locks, workflows GitHub Actions, scripts shell, instalador Inno Setup e pacote `ACcompactadox.zip` da ICP-Brasil. |

