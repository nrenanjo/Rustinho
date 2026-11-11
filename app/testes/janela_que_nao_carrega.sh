#!/bin/bash
# J04 (dev.3.3, passo 2.4): com a tela principal quebrada, o Rustinho avisa e
# sai (antes ficava aberto e invisível: nada na tela, o processo rodando).
#
# Monta uma cópia DESCARTÁVEL do código com um erro de propósito no Main.qml,
# abre o programa numa tela virtual (Xvfb) com dados temporários e confere:
#   J1  a janela de aviso aparece ("A tela principal do Rustinho não abriu");
#   J2  fechar o aviso (Esc) encerra o processo com código 1.
#
# Uso: app/testes/janela_que_nao_carrega.sh [pasta do código] [pasta das fotos]
#   (a pasta do código padrão é a deste repositório; CARGO_TARGET_DIR, se
#   definido, é a pasta da montagem: com a do repositório, as dependências
#   não são montadas de novo, mas o programa de lá fica quebrado até a
#   próxima montagem normal).
# Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
set -u
FONTE=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
FOTOS=${2:-$(mktemp -d -t rustinho-j04-fotos-XXXX)}
TELA=${RUSTINHO_TELA:-:89}
for p in Xvfb xdotool import cargo; do command -v $p > /dev/null || { echo "Falta o programa $p"; exit 2; }; done
case "$TELA" in :0 | :1) echo "Recuso a tela $TELA (pode ser a área de trabalho de verdade)"; exit 2 ;; esac
mkdir -p "$FOTOS"
COPIA=$(mktemp -d -t rustinho-j04-copia-XXXX)
DADOS=$(mktemp -d -t rustinho-j04-dados-XXXX)
tar -C "$FONTE" --exclude=./target --exclude=./.git --exclude=./.reversa -cf - . | tar -C "$COPIA" -xf -
# Um módulo que não existe: a montagem passa (o qmlcachegen recusa erro de
# sintaxe, mas não o que só falha ao abrir) e o Main.qml não carrega ao abrir,
# como quando falta uma parte do Qt na instalação.
sed -i '1i import ModuloQueNaoExisteDeProposito' "$COPIA/app/qml/Main.qml"
(cd "$COPIA" && cargo build -q -p rustinho) || { echo "a cópia não montou"; exit 2; }
PROGRAMA="${CARGO_TARGET_DIR:-$COPIA/target}/debug/rustinho"

Xvfb "$TELA" -screen 0 1280x800x24 2> /dev/null &
XVFB=$!
sleep 1
DISPLAY=$TELA RUSTINHO_DADOS=$DADOS QT_QUICK_BACKEND=software "$PROGRAMA" > "$FOTOS/saida.log" 2>&1 &
P=$!
JANELA=""
for _ in $(seq 1 40); do
    JANELA=$(DISPLAY=$TELA xdotool search --onlyvisible --name '^Rustinho$' 2> /dev/null | head -1)
    [ -n "$JANELA" ] && break
    sleep 0.5
done
sleep 1
DISPLAY=$TELA import -window root "$FOTOS/j04-tela.png" 2> /dev/null
falhas=0
if [ -n "$JANELA" ] && kill -0 $P 2> /dev/null; then
    echo "✔ J1 a janela de aviso apareceu (janela $JANELA)"
else
    vivo=$(kill -0 $P 2> /dev/null && echo "rodando, sem janela (invisível)" || echo "encerrado")
    echo "✘ J1 nenhuma janela de aviso; o processo está $vivo"
    falhas=$((falhas + 1))
fi
codigo=""
como=""
if [ -n "$JANELA" ]; then
    eval "$(DISPLAY=$TELA xdotool getwindowgeometry --shell "$JANELA")"
    # Sem gerenciador de janelas, o teclado só chega depois de um clique: um
    # clique num canto vazio do aviso e o Esc.
    DISPLAY=$TELA xdotool mousemove $((X + 8)) $((Y + 8)) click 1 sleep 0.5 key Escape
    for _ in $(seq 1 10); do kill -0 $P 2> /dev/null || break; sleep 0.5; done
    como="Esc"
    if kill -0 $P 2> /dev/null; then
        # O "Fechar" do aviso (560 × 220): à esquerda, logo abaixo do texto.
        DISPLAY=$TELA xdotool mousemove $((X + 52)) $((Y + 142)) click 1
        for _ in $(seq 1 10); do kill -0 $P 2> /dev/null || break; sleep 0.5; done
        como="Fechar (o Esc não fechou)"
    fi
fi
if kill -0 $P 2> /dev/null; then
    echo "✘ J2 o processo continua rodando depois de fechar o aviso"
    falhas=$((falhas + 1))
    kill $P 2> /dev/null
    wait $P 2> /dev/null
else
    wait $P
    codigo=$?
    if [ "$codigo" = 1 ]; then echo "✔ J2 fechar o aviso ($como) encerra com código 1"; else echo "✘ J2 código de saída $codigo (esperado 1)"; falhas=$((falhas + 1)); fi
fi
grep -m2 "não carregou\|Main.qml" "$FOTOS/saida.log" | sed 's/^/   registro: /'
kill $XVFB 2> /dev/null
rm -rf "$COPIA" "$DADOS"
echo "fotos em $FOTOS"
[ $falhas = 0 ]
