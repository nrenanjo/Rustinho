## ⚠️ Versão de TESTE (pré-lançamento) — o Rustinho ainda NÃO é estável

Use só com **dados de mentira**. Não use na loja. Esta versão serve para
fazer o roteiro de testes no Windows 10/11 e no Linux.

Os dois arquivos saíram do **mesmo commit**: `{{COMMIT}}`.
Instalador do Windows e APK do Android não fazem parte desta versão.

### Windows 10 e 11 (64 bits) — `{{ZIP}}`

1. Baixe o ZIP. Clique nele com o botão direito → **Extrair tudo** (para uma pasta sua, por exemplo na Área de Trabalho). Não abra o programa de dentro do ZIP.
2. Abra a pasta extraída e dê dois cliques em `rustinho.exe`.
3. Se aparecer "O Windows protegeu o computador": **Mais informações** → **Executar assim mesmo** (o programa não é assinado).
4. Na primeira vez, configure uma loja de mentira (ex.: loja "Loja Teste", gerente "Ana", senha "1234").

O `LEIA-ME.txt` e o `ROTEIRO-DE-TESTE.txt` vêm dentro da pasta.

### Linux (x86-64) — `{{APPIMAGE}}` (para o CachyOS e outros Linux de 64 bits)

1. Baixe o arquivo para uma pasta sua.
2. No terminal, nessa pasta: `chmod +x {{APPIMAGE}}`
3. Para testar com uma pasta de dados separada (recomendado):
   `RUSTINHO_DADOS=~/rustinho-teste ./{{APPIMAGE}}`
   Sem `RUSTINHO_DADOS`, o AppImage usa a pasta de sempre do Rustinho no Linux (`~/.local/share/rustinho`).
4. Na primeira vez, configure uma loja de mentira.

Precisa do FUSE 2 (no CachyOS/Arch: `sudo pacman -S --needed fuse2`). O Qt, as telas e o rclone vão dentro do AppImage. Na sessão Wayland do KDE, ele abre pelo XWayland.

### Dados de teste

- **Windows (ZIP portátil):** tudo fica na própria pasta e não abre nem muda o Rustinho instalado no computador: banco, backups, registro de falhas e PDFs na pasta `dados`; a pasta de sempre (`%LOCALAPPDATA%\Rustinho`) e `RUSTINHO_DADOS` não são usadas; backups só dentro da pasta; Google Drive com ligação própria; troca de dados só com outro Rustinho portátil (portas 47820 e 47821, e outra chave). Para apagar o teste, apague a pasta.
- **Linux (AppImage normal):** use `RUSTINHO_DADOS=~/rustinho-teste` (passo 3) para separar os dados de teste; para apagar o teste, apague essa pasta. Nos testes, não ligue a troca de dados com a senha da rede de uma loja de verdade nem o Google Drive da loja.

### Conferir os arquivos (SHA-256)

| Arquivo | SHA-256 |
|---|---|
| `{{ZIP}}` | `{{SHA_ZIP}}` |
| `{{APPIMAGE}}` | `{{SHA_APPIMAGE}}` |

Windows (PowerShell, na pasta do arquivo): `Get-FileHash .\{{ZIP}} -Algorithm SHA256`
Linux: `sha256sum {{APPIMAGE}}`

### O que foi testado (só por programas, nesta montagem)

- Testes automáticos do núcleo **no Linux** (robô do GitHub, este commit): {{TESTES_LINUX}}.
- Testes automáticos do núcleo **no Windows** (robô do GitHub, este commit): {{TESTES_WINDOWS}}. Falharam: {{FALHAS_WINDOWS}}.
- Isolamento do portátil **no Windows** (robô): uma cópia do ZIP aberta sem tela criou o banco dentro da pasta dela e nada em `%LOCALAPPDATA%\Rustinho` nem em `RUSTINHO_DADOS`.
- AppImage **no Linux** (robô): {{CONFERENCIA_LINUX}}.
- Antes da montagem, no computador de desenvolvimento (Linux): todos os testes automáticos e as telas de verdade numa tela virtual (D-09, D-14 e o portátil), com cada regra nova desligada de propósito para conferir que o teste falha.

### O que ainda NÃO foi testado (pendente)

- **Nenhum dos dois pacotes foi aberto num computador de verdade** (Windows 10/11 ou Linux, com tela, mouse e teclado).
- O roteiro à mão inteiro (`ROTEIRO-DE-TESTE.txt`), no Windows e no Linux.
- Troca de dados entre dois computadores de verdade (FV-1 a FV-13) e, no Windows, prender a conexão à placa de rede (PEND-13).
- Conferência da DRE pelo dono e pelo contador (PEND-20).
- Importação de uma cópia da exportação real do SINTEC.
- Decisões pendentes da auditoria: R-01 (senha da rede curta), R-02 (cancelar venda de um caixa aberto em outro computador), R-03 (parcela paga a mais por dois caixas sem rede).

### O que mudou na 0.9.14

- **Caixa já fechado não muda mais (D-14):** cancelar uma venda, estornar um recebimento ou desfazer uma devolução de um caixa já fechado não mexe no fechamento dele; o dinheiro sai (ou entra) na gaveta aberta deste computador ou, sem caixa aberto, no Caixa geral.
- **Troca de dados (D-16):** configurações que são só deste computador não são aceitas se chegarem pela rede.
- **Compras (D-15):** % de lucro abaixo do custo com o arredondamento certo.
- **Rustinho portátil para Windows** (ZIP), para testar sem instalar e sem encostar na loja.
- Auditoria técnica do núcleo: `docs/AUDITORIA-0.9.14.md`.
