# BLOCO 16 — Auditoria da Ponte Rust/C++/Qt Desktop (`app/`) e Camada Nativa Android (`android/`)

**Data da auditoria:** 2026-11-02  
**Escopo do bloco:** Auditoria integral da ponte CXX-Qt entre o núcleo Rust e o Qt 6 no computador (`app/`) e no Android (`android/`), incluindo utilitários nativos C++ (`janela.cpp`, `pdf.cpp`, `vistacamera.cpp`, `diagnostico.cpp`), scripts de compilação (`build.rs`, `CMakeLists.txt`), manifesto Android (`AndroidManifest.xml`), leitor de câmera (`LeitorCamera.qml`), preparação de fontes (`preparar_fontes.py`), suíte de diagnóstico offscreen (`android/diagnostico_teste/`), script de fumaça em emulador (`android/testar.sh`) e verificação de paridade entre `app/` e `android/rust/`.

---

## 1. Arquivos Lidos Integralmente neste Bloco

### 1.1. Ponte Desktop e Utilitários C++/Fontes (`app/` — 10 arquivos + `.qmlls.ini`)
| Arquivo | Linhas | Escopo Analisado |
|---|---:|---|
| `app/Cargo.toml` | 1–17 | Manifesto do binário desktop (`rustinho`), dependências `cxx`, `cxx-qt 0.10`, `cxx-qt-lib`, `serde_json`, `winresource` |
| `app/build.rs` | 1–84 | Registro automático de `qml/*.qml`, singletons `Tema.qml` e `Api.qml`, `qml_android/LeitorCamera.qml`, fontes `.ttf`/`.woff2`, ícones `.png`, `cpp/pdf.cpp`, `cpp/janela.cpp`, flags `/utf-8` (MSVC), `PrintSupport` (Windows), `--no-zstd` (Android) e ícone `.ico` |
| `app/src/main.rs` | 1–43 | Entrada desktop: `rustinho_preparar_janela()`, `QGuiApplication`, `QQmlApplicationEngine`, filtro de argumentos de teste (`--abrir`, `--camera`), carga de `qrc:/qt/qml/rustinho/qml/Main.qml` |
| `app/src/ponte.rs` | 1–280 | Singleton `Sistema` (`#[cxx_qt::bridge]`): `chamar`, `chamar_demorado`, `abrir_em_segundo_plano`, `tentar_atualizacao_de_novo`, `imprimir`, `arquivo_local`, `ocupado`, `fazendo`, guarda `ANCORAS_DE_TESTE_COMPILADAS`, `Mutex<Option<Result<Servico, String>>>` e `AtomicBool` |
| `app/cpp/janela.h` | 1–14 | Declarações C++ de `rustinho_preparar_janela`, `rustinho_arquivo_local` e `rustinho_identidade_do_computador` |
| `app/cpp/janela.cpp` | 1–44 | Ícone da janela, conversão de URI (`file://` e `content://` no Android com cópia para `CacheLocation/importar`) e `QSysInfo::machineUniqueId()` |
| `app/cpp/pdf.h` | 1–14 | Declarações C++ de `rustinho_gerar_pdf`, `rustinho_abrir_arquivo`, `rustinho_gerar_etiquetas`, `rustinho_imprimir_html` e `rustinho_imprimir_etiquetas` |
| `app/cpp/pdf.cpp` | 1–291 | Renderização de HTML em PDF A4 (`QTextDocument` + `QPdfWriter` 300 DPI), desenho vetorial de folhas de etiquetas e barras EAN/Code128 em 600 DPI (`QPainter`), impressão direta via `QPrinter` no Windows |
| `app/qml_android/LeitorCamera.qml` | 1–140 | Leitor de código de barras por câmera no Android (`CameraPermission`, `CaptureSession`, `Camera`, `VistaCamera`, `Timer` de 450ms salvando `quadro.pgm` e chamando `codigo_ler_foto`) |
| `app/fontes/preparar_fontes.py` | 1–167 | Descompressão de `.woff2` para `.ttf` e ajuste de métricas verticais (`ascender`, `descender`, `sTypo*`, `usWin*`) da fonte `Cormorant Garamond` |
| `app/.qmlls.ini` | 1–3 | Configuração do QML Language Server |

### 1.2. Camada Nativa Android e Diagnóstico (`android/` — 18 arquivos + diretórios espelhados)
| Arquivo | Linhas | Escopo Analisado |
|---|---:|---|
| `android/CMakeLists.txt` | 1–64 | Build CMake (Qt 6.8, `cxx-qt-cmake`, `rustinho_android`, `minSdkVersion 28`, `targetSdkVersion 34`, `QT_QML_ROOT_PATH`) |
| `android/main.cpp` | 1–129 | Entrada Android: `Diagnostico`, detecção de ARM 32-bit e GPU PowerVR (`desenharPorSoftware` -> `QT_QUICK_BACKEND=software`), `RUSTINHO_DADOS`, registro de `VistaCamera`, filtro C-13 de `Intent.getStringExtra("rustinho")`, toast em falha de abertura |
| `android/vistacamera.h` | 1–39 | Declaração de `VistaCamera` (`QQuickPaintedItem` + `QVideoSink`) |
| `android/vistacamera.cpp` | 1–121 | Extração rápida do plano Y (tons de cinza) de quadros YUV (`NV12`, `NV21`, `YUV420P`, `YV12`, `YUV422P`, `Y8`), rotação/espelhamento Qt 6.7+, pintura via CPU e gravação em `PGM` |
| `android/diagnostico.h` | 1–68 | Declaração de `Diagnostico` (PEND-18: captura de logs Qt, marcos, verificação de tela uniforme e `Toast` JNI) |
| `android/diagnostico.cpp` | 1–229 | Implementação de `Diagnostico`: gravação limitada a 2 MB em `diagnostico.txt`, rotação para `diagnostico-anterior.txt`, captura de janela aos 8s e 20s (`grabWindow`), detecção de imagem de até 3 cores (`imagemUniforme`) e `Toast` na thread principal do Android |
| `android/DIAGNOSTICO.md` | 1–21 | Documentação de uso do diagnóstico e de `desenho.txt` (`software` vs `gpu`) |
| `android/testar.sh` | 1–127 | Teste automatizado via `adb` no emulador Android (onboarding, modo escuro, 19 telas, argumento malicioso C-13, rotação paisagem, botão voltar e câmera) |
| `android/assinatura/LEIA.md` | 1–61 | Documentação de assinatura do APK (`ANDROID_CHAVE_BASE64`, `ANDROID_CHAVE_SENHA`, `ANDROID_CHAVE_IMPRESSAO`) e migração de chave pública antiga |
| `android/pacote/AndroidManifest.xml` | 1–60 | Permissões (`INTERNET`, `ACCESS_NETWORK_STATE`, `ACCESS_WIFI_STATE`, `CHANGE_WIFI_MULTICAST_STATE`, `CAMERA`), `allowBackup="false"`, `FileProvider` |
| `android/pacote/res/xml/qtprovider_paths.xml` | 1–6 | Caminhos exportáveis pelo `FileProvider` (`files`, `cache`, `external_files`) |
| `android/rust/Cargo.toml` | 1–33 | Manifesto da `staticlib` `rustinho_android` (`overflow-checks = true`, `strip = true`) |
| `android/rust/build.rs` | 1–84 | Script de build (idêntico a `app/build.rs`) |
| `android/rust/src/lib.rs` | 1–4 | Declaração `pub mod ponte;` |
| `android/rust/src/ponte.rs` | 1–280 | Ponte CXX-Qt (idêntica a `app/src/ponte.rs`) |
| `android/rust/.qmlls.ini` | 1–3 | Configuração do QML Language Server |
| `android/diagnostico_teste/CMakeLists.txt` | 1–7 | Build do teste offscreen de `Diagnostico` |
| `android/diagnostico_teste/teste.cpp` | 1–31 | Runner C++ para os cenários `cinza`, `conteudo` e `quebrado` |
| `android/diagnostico_teste/cinza.qml` | 1–2 | Janela monocromática (`#E8E4DC`) sem `telaPronta` |
| `android/diagnostico_teste/conteudo.qml` | 1–8 | Janela com retângulo azul, textos e `diagnostico.telaPronta("TelaTeste")` |
| `android/diagnostico_teste/quebrado.qml` | 1–5 | QML inválido (`coisaInexistente: 1`) |
| `android/diagnostico_teste/rodar.sh` | 1–19 | Script de execução dos 3 cenários com `QT_QPA_PLATFORM=offscreen` |

### 1.3. Verificação de Paridade `app/` vs `android/rust/` (Somente Leitura)
- Executado `diff -rq` comparando:
  - `app/qml` vs `android/rust/qml` → **0 diferenças** (100 arquivos QML idênticos).
  - `app/base` vs `android/rust/base` → **0 diferenças** (`Api.qml` e `Tema.qml` idênticos).
  - `app/cpp` vs `android/rust/cpp` e `android/rust/include/rustinho/cpp` → **0 diferenças** (`janela.cpp`, `janela.h`, `pdf.cpp`, `pdf.h` idênticos).
  - `app/fontes` vs `android/rust/fontes` → **0 diferenças**.
  - `app/icone` vs `android/rust/icone` → **0 diferenças**.
  - `app/qml_android` vs `android/rust/qml_android` → **0 diferenças** (`LeitorCamera.qml` idêntico).
  - `app/src/ponte.rs` vs `android/rust/src/ponte.rs` → **0 diferenças**.
  - `app/build.rs` vs `android/rust/build.rs` → **0 diferenças**.

---

## 2. Revisão dos Achados Históricos (Gemini 4 Argon) Relacionados a este Bloco

Os pontos históricos que tangenciam a ponte C++/Qt e o Android foram verificados diretamente no código-fonte:

1. **Guarda contra binário compilado com âncoras de teste da NF-e (`PEND-09`):**
   - **Veredito:** `CONFIRMADO (IMPLEMENTADO CORRETAMENTE)` em `app/src/ponte.rs:111–114` e `android/rust/src/ponte.rs:111–114`: `abrir_servico()` verifica `if rustinho_nucleo::nfe_assinatura::ANCORAS_DE_TESTE_COMPILADAS` e recusa abrir o banco.
2. **Mitigação de injeção de argumentos via Intent extra no Android (`C-13`):**
   - **Veredito:** `CONFIRMADO (IMPLEMENTADO CORRETAMENTE)` em `android/main.cpp:86–108`: o extra `"rustinho"` da `Intent` é filtrado por expressão regular estrita (`^Tela[A-Za-z]{1,40}$`), aceitando exclusivamente `--camera` e `--abrir Tela...`, descartando quaisquer outros parâmetros (`--props`, caminhos relativos, etc.), como também exercitado em `android/testar.sh:90–94`.
3. **Diagnóstico de tela cinza e fallback de renderização por software em 32 bits / PowerVR (`PEND-18`):**
   - **Veredito:** `CONFIRMADO (IMPLEMENTADO CORRETAMENTE)` em `android/main.cpp:26–66` e `android/diagnostico.cpp:1–229`: dispositivos ARMv7 32 bits (`__arm__ && !__aarch64__`) e GPUs PowerVR (`ro.hardware.egl`, `ro.hardware.vulkan`, `ro.board.platform` contendo `"powervr"` ou `"pvr"`) ativam automaticamente `QT_QUICK_BACKEND=software`, e `Diagnostico` monitora o carregamento e captura fotos aos 8s e 20s.

---

## 3. Novos Achados Identificados no Bloco 16

### `NEW-B16-01` — `rustinho_arquivo_local` copia arquivos `content://` (incluindo certificados `.pfx` e backups `.db`/`.rustinho`) para `CacheLocation/importar/` no Android e nunca os remove após a operação
- **Classificação:** `Risco` (Segurança / Privacidade e Armazenamento no Android)
- **Severidade:** Média
- **Localização:** `app/cpp/janela.cpp:18–35` (`rustinho_arquivo_local`), chamado por `app/qml/TelaConfiguracoes.qml:1098` (importação de certificado A1 `.pfx`), `app/qml/SecaoBackup.qml` (restauração de backup), `app/qml/TelaCompra.qml` e `app/qml/ImportarCadastros.qml`
- **Evidência no código (`app/cpp/janela.cpp:25–34`):**
  ```cpp
  QFile f(endereco);
  if (!f.open(QIODevice::ReadOnly))
      return {};
  const QString pasta = QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + QStringLiteral("/importar");
  QDir().mkpath(pasta);
  const QString nome = QFileInfo(u.path()).fileName();
  const QString destino = pasta + QLatin1Char('/') + (nome.isEmpty() ? QStringLiteral("arquivo.bin") : nome);
  QFile::remove(destino);
  return f.copy(destino) ? destino : QString();
  ```
- **Análise técnica:**
  - No Android, o seletor de arquivos (`FileDialog`) entrega URIs do Storage Access Framework (`content://...`). Para que o núcleo em Rust consiga abrir o arquivo via `std::fs::File`, `rustinho_arquivo_local` copia o conteúdo para `<CacheLocation>/importar/<nome>`.
  - Porém, nem a ponte (`ponte.rs`) nem o núcleo removem essa cópia temporária após a conclusão da operação:
    1. Quando o usuário importa um certificado digital A1 (`.pfx`/`.p12`) no Android (`TelaConfiguracoes.qml:1098`), o núcleo criptografa os bytes ao gravar no SQLite (`certificado_pfx`), mas **uma cópia integral do arquivo `.pfx` original permanece indefinidamente em `<CacheLocation>/importar/`**.
    2. Quando o usuário importa backups (`.rustinho`/`.db`), XMLs de NF-e ou planilhas com nomes distintos, as cópias acumulam-se indefinidamente na pasta de cache do aplicativo.

---

### `NEW-B16-02` — Condição de corrida em `VistaCamera::m_imagem` entre a thread GUI (`chegou` / `salvar`) e a thread de renderização do Qt Quick (`paint`) no modo GPU
- **Classificação:** `Risco` (Concorrência C++/Qt Quick)
- **Severidade:** Baixa a Média
- **Localização:** `android/vistacamera.h:36–38` e `android/vistacamera.cpp:85–121`
- **Evidência no código (`android/vistacamera.cpp:85–111`):**
  ```cpp
  void VistaCamera::chegou(const QVideoFrame& quadro)
  {
      if (!m_imagem.isNull() && m_relogio.elapsed() < 80)
          return;
      m_relogio.restart();
      QImage nova = paraCinza(quadro);
      if (nova.isNull())
          return;
      const bool primeiro = m_imagem.isNull();
      m_imagem = std::move(nova);
      if (primeiro)
          emit temQuadroChanged();
      update();
  }

  void VistaCamera::paint(QPainter* pintor)
  {
      if (m_imagem.isNull() || width() <= 0 || height() <= 0)
          return;
      const qreal escala = qMax(width() / m_imagem.width(), height() / m_imagem.height());
      ...
      pintor->drawImage(alvo, m_imagem);
  }
  ```
- **Análise técnica:**
  - `VistaCamera` herda de `QQuickPaintedItem`. Em aparelhos Android de 64 bits com GPU não-PowerVR (onde `QT_QUICK_BACKEND` permanece no render loop `threaded` padrão do Qt Quick), `QQuickPaintedItem::paint(QPainter*)` é executado na `QSGRenderThread` durante a fase de renderização (quando a thread principal/GUI **não** está bloqueada), enquanto `VistaCamera::chegou` é executado na thread GUI.
  - A atribuição `m_imagem = std::move(nova)` na thread GUI modifica o ponteiro interno `d` da instância `m_imagem` sem `QMutex`, enquanto `VistaCamera::paint` na `QSGRenderThread` lê `m_imagem.isNull()`, `m_imagem.width()`, `m_imagem.height()` e `pintor->drawImage(alvo, m_imagem)` diretamente sobre o mesmo membro `m_imagem`.

---

### `NEW-B16-03` — Acesso concorrente sem trava a `Diagnostico::m_erros` no `QtMessageHandler` global (`Diagnostico::captador`)
- **Classificação:** `Risco` (Concorrência C++)
- **Severidade:** Baixa
- **Localização:** `android/diagnostico.cpp:90–118`, `android/diagnostico.cpp:225–226` e `android/diagnostico.h:56`
- **Evidência no código (`android/diagnostico.cpp:102–118`):**
  ```cpp
  void Diagnostico::captador(QtMsgType tipo, const QMessageLogContext& contexto, const QString& texto)
  {
      if (g_instancia) {
          ...
          g_instancia->escrever(QStringLiteral("[%1] %2%3").arg(QString::fromLatin1(nome), texto.left(600), onde));
          const bool ruido = texto.startsWith(QLatin1String("QStandardPaths")) || texto.contains(QLatin1String("XDG_RUNTIME_DIR"));
          if ((tipo == QtWarningMsg || tipo == QtCriticalMsg || tipo == QtFatalMsg) && !ruido && g_instancia->m_erros.size() < 6)
              g_instancia->m_erros << texto.left(300);
      }
      if (g_anterior)
          g_anterior(tipo, contexto, texto);
  }
  ```
- **Análise técnica:**
  - O método `escrever()` protege a escrita no arquivo `g_arquivo` com `QMutexLocker l(&g_trava);` (`linha 92`), mas libera o mutex ao retornar.
  - Logo em seguida (`linhas 113–114`), `Diagnostico::captador` — que é um `QtMessageHandler` global chamado por qualquer thread do processo (threads de rede, multimídia, renderização ou trabalho) — lê e modifica `g_instancia->m_erros` (`QStringList`) fora de `g_trava`, enquanto a thread GUI lê `m_erros` em `primeirosErros()` (`main.cpp:119–120`) e `verificar()` (`diagnostico.cpp:225–226`).

---

### `NEW-B16-04` — `VistaCamera::salvar` não garante a criação do diretório pai (`CacheLocation`) antes de `QImage::save`
- **Classificação:** `Risco`
- **Severidade:** Baixa
- **Localização:** `android/vistacamera.cpp:113–121` e `app/qml_android/LeitorCamera.qml:16, 76`
- **Evidência no código (`android/vistacamera.cpp:113–121`):**
  ```cpp
  bool VistaCamera::salvar(const QString& caminho, int larguraMaxima)
  {
      if (m_imagem.isNull())
          return false;
      QImage imagem = m_imagem;
      if (larguraMaxima > 0 && imagem.width() > larguraMaxima)
          imagem = imagem.scaledToWidth(larguraMaxima, Qt::SmoothTransformation);
      return imagem.save(caminho, "PGM");
  }
  ```
- **Análise técnica:**
  - `LeitorCamera.qml:16` define `foto` como `Api.caminho(StandardPaths.writableLocation(StandardPaths.CacheLocation)) + "/quadro.pgm"`.
  - Diferentemente de `rustinho_gerar_pdf` (`pdf.cpp:19`), `rustinho_gerar_etiquetas` (`pdf.cpp:241`) e `rustinho_arquivo_local` (`janela.cpp:29`), que chamam `QDir().mkpath(...)` antes de gravar, `VistaCamera::salvar` chama `imagem.save(caminho, "PGM")` diretamente. Caso o diretório retornado por `CacheLocation` ainda não exista no sistema de arquivos no primeiro uso da câmera, `QImage::save` falha silenciosamente retornando `false`.

---

### `NEW-B16-05` — Caminhos absolutos de máquina de desenvolvimento em `app/.qmlls.ini` e `android/rust/.qmlls.ini`
- **Classificação:** `Melhoria` (Higiene de Configuração)
- **Severidade:** Baixa
- **Localização:** `app/.qmlls.ini:1–3` e `android/rust/.qmlls.ini:1–3`
- **Evidência no código:**
  - `app/.qmlls.ini:2`: `buildDir="/home/claude/rustinho/target/cxxqt/qml_modules"`
  - `android/rust/.qmlls.ini:2`: `buildDir="/home/claude/rustinho/android/rust/target/cxxqt/qml_modules"`
- **Análise técnica:**
  - Ambos os arquivos `.qmlls.ini` versionados no projeto contêm caminhos absolutos fixos (`/home/claude/rustinho/...`) específicos de outro ambiente.

---

## 4. Registro Separado de Leitura vs Testes Encontrados vs Testes Executados

1. **Código lido:**
   - Lidos integralmente 11 arquivos em `app/` (`Cargo.toml`, `build.rs`, `src/main.rs`, `src/ponte.rs`, `cpp/janela.cpp`, `cpp/janela.h`, `cpp/pdf.cpp`, `cpp/pdf.h`, `qml_android/LeitorCamera.qml`, `fontes/preparar_fontes.py`, `.qmlls.ini`) e 22 arquivos em `android/` (`CMakeLists.txt`, `main.cpp`, `vistacamera.cpp`, `vistacamera.h`, `diagnostico.cpp`, `diagnostico.h`, `DIAGNOSTICO.md`, `testar.sh`, `assinatura/LEIA.md`, `pacote/AndroidManifest.xml`, `pacote/res/xml/qtprovider_paths.xml`, `rust/Cargo.toml`, `rust/build.rs`, `rust/src/lib.rs`, `rust/src/ponte.rs`, `rust/.qmlls.ini`, `diagnostico_teste/*`), além de verificação `diff -rq` entre todas as pastas compartilhadas de `app/` e `android/rust/`.
2. **Testes encontrados neste escopo:**
   - `android/diagnostico_teste/` (`CMakeLists.txt`, `teste.cpp`, `cinza.qml`, `conteudo.qml`, `quebrado.qml`, `rodar.sh`): teste automatizado offscreen da classe `Diagnostico`.
   - `android/testar.sh`: script de fumaça E2E em emulador Android via `adb`.
3. **Testes executados neste bloco:**
   - Nenhum teste de runtime foi executado neste bloco (a execução da suíte completa de testes está concentrada e registrada separadamente no Bloco 18).

