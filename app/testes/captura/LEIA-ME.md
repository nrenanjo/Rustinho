# Revisão visual (L5): ferramentas de captura

Versionadas na dev.3.3 (passo 1.7), a partir das que a revisão visual da auditoria usou
(`_reversa_sdd/auditoria_dev33/REVISAO_VISUAL.md`). O procedimento é o do
`PLANO_VALIDACAO_E_PUBLICACAO_DEV33.md` §8.

| Arquivo | Para quê |
|---|---|
| `captura.sh` | prepara o pacote (SHA-256 conferido), abre o Rustinho numa tela virtual só dela, sem rede, e tira foto + textos da tela |
| `percorrer.sh` | passa pelas telas clicando nos textos que o próprio Rustinho informa e grava cada captura |

## Regras (não negociáveis)

- **O pacote é o que vai ser publicado.** O SHA-256 é conferido contra o `SHA256SUMS.txt` do
  manifesto da execução da CI, e o pacote é extraído numa pasta nova.
- **A tela virtual é sempre a da ferramenta.** Toda foto, tecla e clique vai para o Xvfb que o
  `captura.sh` abriu, com a tela escrita no próprio comando (`import -display`). Se essa tela não
  estiver de pé, a ordem é recusada. A tela `:0`/`:1` é recusada sempre. Na auditoria, uma passada
  sem isso fotografou a área de trabalho real (`REVISAO_VISUAL.md` §6).
- **Ambiente neutro e sem rede:** `env -i`; `HOME`, `XDG_*` e `RUSTINHO_DADOS` dentro da pasta da
  campanha; sem Wayland nem barramento da sessão; `unshare -rn`; `QT_QPA_PLATFORM=xcb`.
- **Dados de mentira, criados pela interface**, como uma pessoa faria. O banco só é lido, e só em
  modo somente leitura (`sqlite3 "file:…/rustinho.db?mode=ro"`). Nunca `~/.local/share/rustinho`,
  nunca dados de uma loja de verdade, nunca a VM.
- **Cada captura** vai com o `.txt` (textos e posições do modo de captura) e entra no
  `SHA256SUMS.txt` da pasta.

## Precisa de

Xvfb, xdotool, ImageMagick (`import`), `xdpyinfo` (x11-utils), `unshare` (util-linux), `sha256sum`.

## Passo a passo

```bash
# 1. O pacote e o manifesto da execução da CI (Actions › CI › execução).
gh run download <execução> -n Rustinho-Linux -n Manifesto -D pacote/

# 2. Uma pasta nova para a campanha.
export CAPTURA_PASTA=$(mktemp -d /tmp/rustinho-l5-XXXX)
C=app/testes/captura/captura.sh

# 3. Conferir e extrair.
$C preparar pacote/Rustinho-*.AppImage pacote/SHA256SUMS.txt

# 4. Primeira abertura (sem opções), para criar os dados de demonstração pela tela.
$C iniciar 1920x1080
#    ... criar os dados (lista abaixo) com $C clique/texto/tecla ...
$C parar

# 5. Com o modo de captura (textos e posições): computador.
$C iniciar 1920x1080 --textos-da-tela
$C janela 1920x1080
app/testes/captura/percorrer.sh cinza "$CAPTURA_PASTA/capturas/1920x1080/cinza"
$C parar

# 6. Celular (modo celular do computador).
$C iniciar 360x780 --textos-da-tela --celular
$C janela 360x780
ALTURA=780 app/testes/captura/percorrer.sh roxo "$CAPTURA_PASTA/capturas/celular/roxo"
$C parar
```

## Dados de demonstração (os textos que o `percorrer.sh` procura)

- Loja "Auto Pecas Modelo", Caixa 1, gerente **Ana** (senha 1234).
- Peças: **"Amortecedor dianteiro Onix"**, **"Pastilha de freio dianteira Gol"** e mais duas.
- Cliente **"Oficina do Joao"**; um fornecedor.
- Caixa aberto com R$ 100,00.
- Duas vendas: uma em dinheiro e uma no crediário da "Oficina do Joao".
- Um recebimento, depois estornado.
- Dois backups.
- Mais 4 usuários (para a entrada no celular com 5 nomes).

Um texto que não aparece vira **AVISO** no fim da passada (falta de dado ou defeito); a passada não
para.

## Matriz da campanha

| Tamanho | Cinza (`cinza`) | AMOLED (`amoled`) | Planeta Roxo (`roxo`) | Modo |
|---|---|---|---|---|
| 1920 × 1080 | ✓ | ✓ | ✓ | `tudo` |
| 1366 × 768 | ✓ | ✓ | ✓ | `denso` (só as telas densas) ou `tudo` |
| 360 × 780 (celular) | ✓ | ✓ | ✓ | `tudo`, com `ALTURA=780` |

O modo celular do computador **não é Android**: usa mouse, não tem teclado na tela, nem a
densidade nem a câmera de um celular. O Android real é a L6 (checklist do Moto e6i).
