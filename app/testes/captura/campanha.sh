#!/bin/bash
# Campanha L5 inteira (três tamanhos × três temas) com o captura.sh e o
# percorrer.sh desta pasta — preparação da fase 9 (prep 8/9). Veja LEIA-ME.md.
#
#   campanha.sh <saída> [programa] [rodadas...]
#
# programa: o executável a fotografar (padrão: target/debug/rustinho desta
# árvore — ENSAIO; a campanha de verdade usa o pacote da CI, com o
# `captura.sh preparar`). rodadas: "1920x1080:cinza" etc. (padrão: as nove).
#
# Cada rodada tem pasta, tela virtual, loja e banco próprios: a loja é montada
# de novo pelo `montar_campanha_l5` (nucleo/tests/lojas_de_teste.rs), pelos
# comandos das telas. Entra como Ana (1234), percorre e fecha. Nada sai da
# pasta de saída. Rode SEMPRE pelo executor isolado (sem rede, pasta pessoal
# própria) e com a trava da tela.
set -u
SAIDA=$1
PROG=${2:-$PWD/target/debug/rustinho}
shift $(( $# >= 2 ? 2 : 1 ))
RODADAS=${*:-"1920x1080:cinza 1920x1080:amoled 1920x1080:roxo 1366x768:cinza 1366x768:amoled 1366x768:roxo 360x780:cinza 360x780:amoled 360x780:roxo"}
DIR=$(cd "$(dirname "$0")" && pwd)
C="$DIR/captura.sh"
mkdir -p "$SAIDA"
[ -x "$PROG" ] || { echo "programa não encontrado: $PROG" >&2; exit 2; }
sha256sum "$PROG" > "$SAIDA/programa.sha256"
: > "$SAIDA/rodadas.txt"

entrar() {
    # Tela de entrada: o nome da Ana, a senha e Enter; espera a Início.
    local xy
    for _ in 1 2 3 4 5 6; do
        xy=$("$C" textos | awk -F'\t' '$3 == "Ana" { print $1, $2; exit }')
        [ -n "$xy" ] && break
        sleep 1
    done
    [ -n "$xy" ] && "$C" clique ${xy% *} ${xy#* }
    "$C" texto "1234"
    "$C" tecla Return
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        "$C" textos | grep -q "Bom dia\|Boa tarde\|Boa noite" && return 0
        sleep 1.5
    done
    return 1
}

for r in $RODADAS; do
    tam=${r%%:*}
    tema=${r#*:}
    case $tam in
        360x780) pasta=celular_360x780; extra=--celular; modo=tudo ;;
        1366x768) pasta=1366x768; extra=; modo=denso ;;
        *) pasta=$tam; extra=; modo=tudo ;;
    esac
    P="$SAIDA/rodadas/${tam}_$tema"
    F="$SAIDA/capturas/$pasta/$tema"
    rm -rf "$P"
    mkdir -p "$P/app/squashfs-root" "$F"
    printf '#!/bin/sh\nexec "%s" "$@"\n' "$PROG" > "$P/app/squashfs-root/AppRun"
    chmod +x "$P/app/squashfs-root/AppRun"
    inicio=$(date +%s)
    RUSTINHO_LOJA_TESTE="$P/dados" cargo test -q -p rustinho-nucleo --test lojas_de_teste -- --ignored --exact montar_campanha_l5 > "$P/montar.txt" 2>&1 \
        || { printf '%s\t%s\tloja não montada\n' "$tam" "$tema" >> "$SAIDA/rodadas.txt"; continue; }
    sha256sum "$P/dados/rustinho.db" > "$P/banco_montado.sha256"
    export CAPTURA_PASTA="$P"
    if ! "$C" iniciar "$tam" --textos-da-tela $extra > "$P/iniciar.txt" 2>&1; then
        "$C" parar > /dev/null 2>&1
        printf '%s\t%s\tnão abriu\n' "$tam" "$tema" >> "$SAIDA/rodadas.txt"
        continue
    fi
    "$C" janela "$tam" > /dev/null
    if ! entrar; then
        "$C" captura "$P" nao_entrou > /dev/null
        "$C" parar > /dev/null
        printf '%s\t%s\tnão entrou (veja %s/nao_entrou.png)\n' "$tam" "$tema" "$P" >> "$SAIDA/rodadas.txt"
        continue
    fi
    ALTURA=${tam#*x} "$DIR/percorrer.sh" "$tema" "$F" "$modo" > "$P/percorrer.txt" 2>&1
    "$C" parar > /dev/null
    avisos=$(grep -c '^AVISO' "$P/percorrer.txt")
    fotos=$(find "$F" -name '*.png' | wc -l)
    printf '%s\t%s\t%s fotos, %s avisos, %ss\n' "$tam" "$tema" "$fotos" "$avisos" "$(($(date +%s) - inicio))" >> "$SAIDA/rodadas.txt"
done
(cd "$SAIDA/capturas" && find . -name '*.png' -o -name '*.txt' | grep -v SHA256SUMS | sort | xargs sha256sum) > "$SAIDA/SHA256SUMS.txt"
cat "$SAIDA/rodadas.txt"
