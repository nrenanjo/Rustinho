# Dependências e Toolchain — Rustinho 0.9.17

> Gerado pelo Scout / Bloco 0 em 2026-11-02

---

## 1. Workspace e Crates

| Crate | Caminho | Tipo | Versão | Edição |
|---|---|---|---|---|
| Workspace raiz | `Cargo.toml` | Workspace (`members = ["nucleo", "app"]`, `exclude = ["android/rust"]`) | `0.9.17` | `2021` |
| `rustinho-nucleo` | `nucleo/Cargo.toml` | Biblioteca core (`rlib`) | `0.9.17` | `2021` |
| `rustinho` | `app/Cargo.toml` | Binário Desktop Qt/QML | `0.9.17` | `2021` |
| `rustinho_android` | `android/rust/Cargo.toml` | Biblioteca estática (`staticlib`) para CMake/Qt Android | `0.9.17` | `2021` |

> Perfil de compilação (`[profile.release]`): `strip = true` e `overflow-checks = true` (tanto no `Cargo.toml` raiz quanto em `android/rust/Cargo.toml`).

---

## 2. Dependências de `rustinho-nucleo` (`nucleo/Cargo.toml`)

| Crate | Versão Declarada | Features | Papel no Sistema |
|---|---|---|---|
| `rusqlite` | `0.40` | `backup`, `bundled`, `functions`, `hooks` | Persistência SQLite embarcada, migrações, backup e hooks. |
| `serde` | `1.0.229` | `derive` | Serialização/desserialização de structs e contratos JSON. |
| `serde_json` | `1.0.154` | padrão | Protocolo JSON entre QML/C++ e `api.rs` + sincronização. |
| `argon2` | `0.6` | padrão | Hashing de senhas de usuários e gerentes. |
| `chacha20poly1305` | `0.11.0` | padrão | Criptografia autenticada (AEAD) para sincronização e backups. |
| `x25519-dalek` | `2` | `static_secrets` | Troca de chaves Diffie-Hellman na sincronização em rede local. |
| `sha2` | `0.11.1` | padrão | Hashes SHA-256 / SHA-512 (integridade, tokens, ICP-Brasil, C14N). |
| `getrandom` | `0.3` | padrão | Geração de entropia criptográfica. |
| `ring` | `0.17` | padrão | Verificação criptográfica RSA/SHA-1 e RSA/SHA-256 de assinaturas XML NF-e. |
| `rustls-webpki` | `0.103` | `ring`, `std`, `alloc` | Validação de cadeia de certificação X.509 ICP-Brasil. |
| `rustls-pki-types` | `1` | padrão | Tipos de certificados e âncoras de confiança para `webpki`. |
| `x509-parser` | `0.18` | `default-features = false` | Parsing de certificados X.509 (extensões, CNPJ em `otherName`, validade). |
| `roxmltree` | `0.21.1` | padrão | Parsing DOM somente-leitura de XML de NF-e e C14N. |
| `base64` | `0.22` | padrão | Codificação/decodificação Base64 (assinaturas XML, chaves, payloads). |
| `time` | `0.3.55` | `formatting`, `parsing`, `macros` | Manipulação de datas, fusos e carimbos de tempo. |
| `uuid` | `1.28.0` | `v7` | Geração de identificadores únicos ordenáveis no tempo (UUIDv7). |
| `unicode-normalization` | `0.1.25` | padrão | Normalização Unicode e remoção de acentos na busca. |
| `zip` | `9` | `deflate` (`default-features = false`) | Leitura de pacotes ZIP (importação de XMLs em lote, acervo ICP-Brasil). |
| `rars` | `0.11` | `default-features = false` | Leitura de arquivos RAR na importação de XMLs. |
| `rxing` | `0.9` | `decoders`, `oned`, `qrcode`, `encoding_rs` | Decodificação de códigos de barras 1D e QR Code. |
| `if-addrs` | `0.15.0` | padrão | Enumeração de interfaces de rede local. |
| `socket2` | `0.6` | `all` | Controle de sockets TCP/UDP para descoberta e sincronização LAN. |

### Dev-Dependencies e Features de Teste (`nucleo/Cargo.toml`)
- Feature `ancoras-de-teste`: substitui âncoras da ICP-Brasil por raízes fictícias de teste (`nucleo/tests/pki`). Possui trava em `nfe_assinatura` para impedir abertura em produção se compilada.
- `libc = "0.2"`: bind em múltiplos endereços loopback (`127.0.0.x`) para testes de rate-limit de rede.
- `rxing = "0.9"` com feature adicional `encoders` para geração de códigos em testes.

---

## 3. Dependências de Interface (`app/Cargo.toml` e `android/rust/Cargo.toml`)

| Crate | Versão Declarada | Features | Papel |
|---|---|---|---|
| `rustinho-nucleo` | `path = "../nucleo"` | padrão | Núcleo de domínio compartilhado entre Desktop e Android. |
| `cxx` | `1.0.176` | padrão | Interoperabilidade segura Rust ↔ C++. |
| `cxx-qt` | `0.10` | padrão | Bindings `QObject` / sinais / propriedades entre Rust e Qt6 QML. |
| `cxx-qt-lib` | `0.10` | `qt_gui`, `qt_qml` | Tipos fundamentais Qt (`QString`, `QUrl`, `QGuiApplication`, `QQmlApplicationEngine`). |
| `cxx-qt-build` | `0.10` | `link_qt_object_files` (no desktop) | Geração de código C++/Rust e empacotamento de recursos QRC em `build.rs`. |
| `winresource` | `0.1` | padrão | Metadados e ícone do executável Windows. |

