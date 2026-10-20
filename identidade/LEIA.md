# Identidade visual do Rustinho — ícone

**Fonte única:** `identidade/rustinho-icone-original.png` (1254 × 1254, RGBA, opaca; enviada pelo dono do projeto em 20/10/2026).
O arquivo nunca é alterado. `identidade/gerar_icones.py` só **redimensiona** (Lanczos) o quadrado inteiro: sem recorte, sem borda extra, sem distorção, cores RGB iguais. Para refazer tudo: `python3 identidade/gerar_icones.py` (precisa do Pillow); para só conferir se os arquivos batem com a fonte: `python3 identidade/gerar_icones.py --conferir` (sai com erro se algo estiver diferente).

## Quem usa cada arquivo

| Plataforma | Arquivos | Usado por |
|---|---|---|
| **Todas** (janela do programa) | `app/icone/rustinho.png` (256 px) | embutido no programa pelo `app/build.rs` (`qrc_resources`) e aplicado em `app/cpp/janela.cpp` (`QGuiApplication::setWindowIcon`) — Linux, Windows e Android |
| **Linux** — atalhos, menu, barra de tarefas | `instalador/icones/hicolor/{16,22,24,32,48,64,128,256,512}/apps/rustinho.png`; `instalador/rustinho.desktop` (`Icon=rustinho`) | AppImage: `linuxdeploy --icon-file …` (todas as medidas) no `.github/workflows/montar.yml`; o `.desktop` aponta para o nome `rustinho` |
| **Linux** — arquivos soltos | `instalador/rustinho-256.png`, `instalador/rustinho.png` (512) | referência do instalador / uso manual |
| **Windows** — executável, atalhos, instalador | `instalador/rustinho.ico` (16, 20, 24, 32, 40, 48, 64, 128, 256; cada quadro é um PNG) | `app/build.rs` (`set_icon`, no executável); `instalador/rustinho.iss` (`SetupIconFile`, atalhos do menu e da área de trabalho) |
| **Android** — ícone do aplicativo (APKs de 32 e 64 bits usam os mesmos recursos) | `android/pacote/res/mipmap-{mdpi 48, hdpi 72, xhdpi 96, xxhdpi 144, xxxhdpi 192}/ic_launcher.png` | `android/pacote/AndroidManifest.xml` (`android:icon="@mipmap/ic_launcher"`) |

## O que **não** foi mudado e está pendente de decisão

Veja `PENDENCIAS.md`: **PEND-11** (autorização para usar o símbolo da Rust — registrada à parte, não resolvida) e **PEND-12** (cantos pretos, ícone adaptativo do Android e versão simplificada em 16 px: propostas que mudariam a aparência e dependem da sua aprovação).
