# BLOCO 17 — Auditoria do Instalador, CI/CD, Scripts de Build e Identidade

**Data da auditoria:** 2026-11-02  
**Escopo do bloco:** Auditoria integral dos arquivos de configuração da raiz (`Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `compilar.sh`), pipelines de integração e entrega contínua (`.github/workflows/montar.yml`, `.github/workflows/montar-teste-windows.yml`, `.github/workflows/android.yml`, `.github/scripts/resumir_testes.py`), empacotamento e instaladores (`instalador/rustinho.iss`, `instalador/rustinho.desktop`, `instalador/appimage/DependenciasDoAppImage.qml`, `instalador/portatil/*`) e pipeline de identidade visual (`identidade/gerar_icones.py`, `identidade/LEIA.md`).

---

## 1. Arquivos Lidos Integralmente neste Bloco

| Arquivo | Linhas | Escopo Analisado |
|---|---:|---|
| `Cargo.toml` | 1–16 | Workspace raiz (`members = ["nucleo", "app"]`, `exclude = ["android/rust"]`, `version = "0.9.17"`, `overflow-checks = true`, `strip = true`) |
| `Cargo.lock` / `android/rust/Cargo.lock` | Verificados | Formato v4, versão `0.9.17` sincronizada em `rustinho`, `rustinho-nucleo` e `rustinho_android` |
| `rustfmt.toml` | 1–2 | Configuração `max_width = 120`, `use_small_heuristics = "Max"` |
| `compilar.sh` | 1–39 | Script local de compilação em Linux (verifica `cargo`, `c++`, `qmake6`, `QtQuick/Controls`, compila `--release` e copia para `./rustinho`) |
| `.github/workflows/montar.yml` | 1–204 | Pipeline principal (`main`): `fmt`, `clippy`, `cargo test` (debug e release), instalador Windows (Inno Setup + `rclone` com verificação SHA-256), Linux AppImage (`linuxdeploy` + `linuxdeploy-plugin-qt` com verificação SHA-256 e checagem de `Qt.labs.folderlistmodel`) e publicação em GitHub Releases |
| `.github/workflows/montar-teste-windows.yml` | 1–382 | Pipeline de teste e pré-lançamento (`build-0.9.14`..`0.9.17`): testes Linux e Windows, pacote Windows portátil (`.zip`) com teste de isolamento offscreen, Linux AppImage com verificação `ldd` e abertura em `xvfb-run`, e criação de rascunho de release (`[rascunho]`) |
| `.github/workflows/android.yml` | 1–343 | Pipeline Android (`64bits` `arm64-v8a` e `32bits` `armeabi-v7a`): Qt 6.8.3 via `aqtinstall`, NDK 26.1, assinatura com chave da loja ou chave efêmera de teste, verificação `apksigner` contra chave antiga pública (`C-10`), publicação condicional com `ANDROID_CHAVE_IMPRESSAO` e teste em emulador (`reactivecircus/android-emulator-runner` + `android/testar.sh`) |
| `.github/scripts/resumir_testes.py` | 1–64 | Parser da saída do `cargo test` para anotações do GitHub Actions e variáveis `resumo`/`falhas` do `GITHUB_OUTPUT` |
| `instalador/rustinho.iss` | 1–162 | Script Inno Setup 6: instalação 64 bits (`MinVersion=10.0.17763`), regras de firewall via `netsh`, diálogo customizado de desinstalação com opção de preservar ou apagar `%LOCALAPPDATA%\Rustinho` com dupla confirmação |
| `instalador/rustinho.desktop` | 1–9 | Entrada freedesktop (`Name=Rustinho`, `Exec=rustinho`, `Icon=rustinho`, `StartupWMClass=rustinho`) |
| `instalador/appimage/DependenciasDoAppImage.qml` | 1–16 | Arquivo QML auxiliar para forçar o `linuxdeploy-plugin-qt` a empacotar `QtQuick.Dialogs.quickimpl` e `Qt.labs.folderlistmodel` |
| `instalador/portatil/rustinho-portatil.txt` | 1–5 | Arquivo marcador que ativa o modo portátil no núcleo (`portatil.rs`) |
| `instalador/portatil/LEIA-ME.txt` | 1–62 | Instruções de uso e isolamento do pacote Windows portátil |
| `instalador/portatil/NOTAS-DA-VERSAO.md` | 1–77 | Template Markdown das notas de versão de rascunho (`{{ZIP}}`, `{{SHA_ZIP}}`, `{{APPIMAGE}}`, etc.) |
| `identidade/gerar_icones.py` | 1–100 | Gerador determinístico de todos os PNGs e `.ico` a partir de `identidade/rustinho-icone-original.png` (com verificação de SHA-256 `b3cf7259...`) |
| `identidade/LEIA.md` | 1–18 | Documentação da matriz de ícones por plataforma e pendências `PEND-11` / `PEND-12` |

---

## 2. Revisão dos Achados Históricos (Gemini 4 Argon) Relacionados a este Bloco

1. **Verificação de integridade SHA-256 de ferramentas externas na CI (`C-11`):**
   - **Veredito:** `CONFIRMADO (IMPLEMENTADO CORRETAMENTE)` em `.github/workflows/montar.yml:83–84, 130, 136–138` e `.github/workflows/montar-teste-windows.yml:125–126, 244, 248–250`: os binários `rclone.zip` (Windows e Linux), `linuxdeploy` e `linuxdeploy-plugin-qt` têm tags fixas e verificação explícita de hash SHA-256 antes da execução.
2. **Proteção contra publicação de APK com chave de teste ou chave pública antiga (`C-10`):**
   - **Veredito:** `CONFIRMADO (IMPLEMENTADO CORRETAMENTE)` em `.github/workflows/android.yml:151–168, 203–249`: a pipeline verifica com `apksigner verify --print-certs` que o certificado do APK coincide com a chave usada, bloqueia o hash `F00729D099BFBBC2C1F74AB560F580CC708E16F028BF32C39F293AEEC983223A` da chave pública antiga e exige `assinatura.txt == "loja"` e correspondência exata com `vars.ANDROID_CHAVE_IMPRESSAO` antes de publicar em Releases.
3. **Verificação de isolamento do pacote Windows portátil (`FALHA-07` / `C-08`):**
   - **Veredito:** `CONFIRMADO (IMPLEMENTADO CORRETAMENTE)` em `.github/workflows/montar-teste-windows.yml:146–173`: a etapa `Conferir o isolamento (abre uma cópia)` executa o portátil com `QT_QPA_PLATFORM=offscreen` e `RUSTINHO_DADOS` apontado para outra pasta, comprovando que o banco é criado exclusivamente em `dados\rustinho.db` da cópia e que nada é criado em `%LOCALAPPDATA%\Rustinho` nem em `RUSTINHO_DADOS`.

---

## 3. Novos Achados Identificados no Bloco 17

### `NEW-B17-01` — `.github/workflows/montar-teste-windows.yml` omite `:$PWD/instalador/appimage` de `QML_SOURCES_PATHS` e não verifica `Qt.labs.folderlistmodel` ao gerar o AppImage de teste/rascunho
- **Classificação:** `Defeito comprovado`
- **Severidade:** Média
- **Localização:** `.github/workflows/montar-teste-windows.yml:253–261` (em contraste com `.github/workflows/montar.yml:141–151` e `instalador/appimage/DependenciasDoAppImage.qml:1–16`)
- **Evidência no código:**
  - Em `.github/workflows/montar.yml:141–151` (pipeline de produção):
    ```yaml
    # instalador/appimage: módulos que as janelas de arquivo do Qt usam (ver o arquivo).
    export QML_SOURCES_PATHS="$PWD/app/qml:$PWD/app/base:$PWD/instalador/appimage"
    ...
    # Sem este módulo, "Escolher arquivos" fecha o programa: não publica.
    test -f AppDir/usr/qml/Qt/labs/folderlistmodel/qmldir || { echo "AppImage sem Qt.labs.folderlistmodel"; exit 1; }
    ```
  - Em `.github/workflows/montar-teste-windows.yml:253–261` (pipeline de teste e rascunho):
    ```yaml
    export QMAKE="$QT_ROOT_DIR/bin/qmake"
    export QML_SOURCES_PATHS="$PWD/app/qml:$PWD/app/base"
    export LD_LIBRARY_PATH="$QT_ROOT_DIR/lib:${LD_LIBRARY_PATH:-}"
    export OUTPUT="Rustinho-$VERSAO-Linux-x86_64.AppImage"
    ./linuxdeploy --appdir AppDir \
      --desktop-file instalador/rustinho.desktop \
      $(for n in 16 22 24 32 48 64 128 256 512; do printf -- '--icon-file instalador/icones/hicolor/%sx%s/apps/rustinho.png ' "$n" "$n"; done) \
      --icon-filename rustinho \
      --plugin qt --output appimage
    ```
- **Análise técnica:**
  - Como documentado em `instalador/appimage/DependenciasDoAppImage.qml:4–6`, o `linuxdeploy-plugin-qt` só inclui no AppImage os módulos QML encontrados nos `import` dos diretórios listados em `QML_SOURCES_PATHS`. Sem `:$PWD/instalador/appimage`, o módulo `Qt.labs.folderlistmodel` (usado internamente por `QtQuick.Dialogs` quando não há portal nativo) fica de fora do AppImage.
  - A etapa de verificação subsequente em `montar-teste-windows.yml:270–319` apenas abre a tela inicial (`Main.qml`) por 8 segundos em `xvfb-run`, sem acionar `FileDialog`/`FolderDialog`. Assim, o AppImage gerado por `montar-teste-windows.yml` (usado nos testes de pré-lançamento e anexado aos rascunhos `[rascunho]`) sai sem `Qt.labs.folderlistmodel`, quebrando "Escolher arquivos" e "Escolher uma pasta" em sistemas Linux que usam o diálogo QML do Qt.

---

### `NEW-B17-02` — Template `instalador/portatil/NOTAS-DA-VERSAO.md` está desatualizado (parado na versão `0.9.15`, sem as mudanças da `0.9.16` e da `0.9.17`)
- **Classificação:** `Defeito comprovado` (Documentação de Release Automatizada)
- **Severidade:** Baixa
- **Localização:** `instalador/portatil/NOTAS-DA-VERSAO.md:58–77` e `.github/workflows/montar-teste-windows.yml:360–376`
- **Evidência no código (`instalador/portatil/NOTAS-DA-VERSAO.md:58–71`):**
  ```markdown
  - Importação de uma cópia da exportação real do SINTEC.
  - Decisões pendentes da auditoria: R-01 (senha da rede curta), R-02 (cancelar venda de um caixa aberto em outro computador), R-03 (parcela paga a mais por dois caixas sem rede).

  ### O que mudou na 0.9.15
  ...
  ### O que mudou na 0.9.14
  ```
- **Análise técnica:**
  - O job `rascunho` de `.github/workflows/montar-teste-windows.yml` gera automaticamente o corpo da release `Rustinho 0.9.17 — versão de TESTE` a partir de `instalador/portatil/NOTAS-DA-VERSAO.md`.
  - O arquivo não possui seções `### O que mudou na 0.9.17` nem `### O que mudou na 0.9.16`, mantendo apenas o histórico até a `0.9.15` e referências antigas (como R-03 na lista de pendentes, já tratada em `testes_r03.rs`).

---

### `NEW-B17-03` — Regra de firewall do instalador Windows (`instalador/rustinho.iss`) aplica-se apenas a `profile=private,domain` e libera todas as portas de entrada para o executável
- **Classificação:** `Risco`
- **Severidade:** Baixa
- **Localização:** `instalador/rustinho.iss:61–64`
- **Evidência no código (`instalador/rustinho.iss:61–64`):**
  ```iss
  ; Libera o Rustinho no firewall do Windows: os caixas da loja se acham e
  ; trocam dados pelo Wi-Fi (portas 47810 e 47811).
  Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Rustinho"""; Flags: runhidden waituntilterminated
  Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""Rustinho"" dir=in action=allow program=""{app}\rustinho.exe"" enable=yes profile=private,domain"; Flags: runhidden waituntilterminated
  ```
- **Análise técnica:**
  1. No Windows 10/11, conexões Wi-Fi recém-conectadas recebem por padrão o perfil **Público** (`Public`), a menos que o usuário altere manualmente o perfil da rede para **Privado** nas configurações do Windows. Como a regra especifica `profile=private,domain`, caixas em Wi-Fi classificado como `Public` não são cobertos pela regra do instalador.
  2. Apesar do comentário mencionar `portas 47810 e 47811`, o comando `netsh` não restringe `protocol=TCP localport=47810` e `protocol=UDP localport=47811`, liberando qualquer porta de entrada aberta por `rustinho.exe` nos perfis privado e de domínio.

---

### `NEW-B17-04` — `cargo clippy` na CI executa apenas sobre `rustinho-nucleo`, não cobrindo `app` (`rustinho`) nem `android/rust` (`rustinho_android`)
- **Classificação:** `Melhoria`
- **Severidade:** Baixa
- **Localização:** `.github/workflows/montar.yml:40` e `.github/workflows/montar-teste-windows.yml:41`
- **Evidência no código:**
  - `.github/workflows/montar.yml:40`: `cargo clippy -p rustinho-nucleo --all-targets --message-format=short -- -D clippy::correctness -D clippy::suspicious`
  - `.github/workflows/montar-teste-windows.yml:41`: `cargo clippy -p rustinho-nucleo --all-targets -- -D clippy::correctness -D clippy::suspicious`
- **Análise técnica:**
  - Como o job `testes` não instala o Qt 6, o `clippy` é restrito a `-p rustinho-nucleo`. Os jobs `linux` e `windows` (que já instalam o Qt 6 e compilam `-p rustinho`) e o job `apk` em `android.yml` não executam `clippy` nem `rustfmt --check` sobre `app/` e `android/rust/`.

---

### `NEW-B17-05` — `compilar.sh` usa `cp target/release/rustinho ./rustinho` (que falha com `Text file busy` se o binário estiver aberto) e assume `pacman`
- **Classificação:** `Melhoria`
- **Severidade:** Baixa
- **Localização:** `compilar.sh:16–27`
- **Evidência no código (`compilar.sh:26–27`):**
  ```bash
  QMAKE=qmake6 cargo build --release
  cp target/release/rustinho ./rustinho
  ```
- **Análise técnica:**
  - No Linux, `cp` tenta truncar o arquivo de destino existente in-place (`open(..., O_WRONLY | O_TRUNC)`), o que retorna `ETXTBSY` (`Text file busy`) caso uma instância de `./rustinho` esteja em execução no momento da recompilação (usar `install -m 755` ou `rm -f ./rustinho && cp ...` evita o erro).

---

## 4. Registro Separado de Leitura vs Testes Encontrados vs Testes Executados

1. **Código e configurações lidos:**
   - Lidos integralmente todos os 16 arquivos do Bloco 17 (`Cargo.toml`, `Cargo.lock`, `android/rust/Cargo.lock`, `rustfmt.toml`, `compilar.sh`, `.github/workflows/montar.yml`, `.github/workflows/montar-teste-windows.yml`, `.github/workflows/android.yml`, `.github/scripts/resumir_testes.py`, `instalador/rustinho.iss`, `instalador/rustinho.desktop`, `instalador/appimage/DependenciasDoAppImage.qml`, `instalador/portatil/rustinho-portatil.txt`, `instalador/portatil/LEIA-ME.txt`, `instalador/portatil/NOTAS-DA-VERSAO.md`, `identidade/gerar_icones.py`, `identidade/LEIA.md`).
2. **Testes encontrados neste escopo:**
   - Verificações embutidas nos workflows de CI:
     - `montar.yml:151`: verificação de presença de `AppDir/usr/qml/Qt/labs/folderlistmodel/qmldir`.
     - `montar-teste-windows.yml:146–173`: teste automatizado de isolamento do modo portátil no Windows (`qoffscreen.dll`).
     - `montar-teste-windows.yml:270–319`: teste automatizado de dependências `ldd` e abertura do AppImage em `xvfb-run`.
     - `android.yml:151–168`: verificação de assinatura `apksigner` e certificado SHA-256 do APK.
     - `identidade/gerar_icones.py --conferir`: verificação determinística de SHA-256 e bytes de todos os 17 ícones PNG e 1 `.ico`.
3. **Testes executados neste bloco:**
   - Nenhum teste foi executado neste bloco (a execução da suíte completa de testes está concentrada e registrada no Bloco 18).

