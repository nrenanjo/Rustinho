#!/bin/bash
# Ferramenta da revisão visual (L5) — dev.3.3, passo 1.7. Veja LEIA-ME.md.
#
# Abre o PACOTE (o mesmo que vai ser publicado, com o SHA-256 conferido) numa
# tela virtual SÓ DELA, com pasta pessoal e dados descartáveis e SEM rede, e
# tira fotos e textos da tela. Toda ordem para a tela (foto, tecla, clique)
# vai para a tela virtual que esta ferramenta abriu, com a tela escrita no
# próprio comando; se essa tela não estiver de pé, a ordem é recusada (lição
# da auditoria: REVISAO_VISUAL.md §6, fotos que saíram da área de trabalho real).
#
# Variáveis:
#   CAPTURA_PASTA  pasta descartável da campanha (obrigatória)
#   CAPTURA_TELA   tela virtual (padrão :93; nunca :0)
#
# Uso:
#   captura.sh preparar Pacote.AppImage SHA256SUMS.txt|<sha256>
#   captura.sh iniciar 1920x1080 [opções do Rustinho...]   (ex.: --textos-da-tela --celular)
#   captura.sh janela 1366x768
#   captura.sh captura <pasta> <nome> [espera]   (foto + textos + SHA256SUMS da pasta)
#   captura.sh foto <arquivo.png> | textos | tecla <teclas...> | texto "abc" | clique X Y | mouse X Y | rolar baixo|cima
#   captura.sh parar
set -u

PASTA=${CAPTURA_PASTA:?defina CAPTURA_PASTA (pasta descartável da campanha)}
TELA=${CAPTURA_TELA:-:93}
APP="$PASTA/app/squashfs-root/AppRun"
CASA="$PASTA/casa"
DADOS="$PASTA/dados"
LOG="$PASTA/rustinho.log"
PIDS="$PASTA/pids"

falha() { echo "captura.sh: $*" >&2; exit 1; }

case "$TELA" in :0 | :0.* | :1 | :1.*) falha "a tela $TELA costuma ser a área de trabalho de verdade; use outra (ex.: :93)" ;; esac
case "$(realpath -m "$PASTA")" in
    "$(realpath -m "$HOME")" | "$(realpath -m "$HOME")/.local/share/rustinho"*) falha "CAPTURA_PASTA não pode ser a pasta pessoal nem a do Rustinho" ;;
esac

# A tela virtual desta ferramenta está de pé? (o Xvfb que ela mesma abriu)
tela_nossa() {
    [ -f "$PIDS/xvfb" ] || return 1
    local pid
    pid=$(cat "$PIDS/xvfb")
    [ "$(ps -p "$pid" -o args= 2>/dev/null | awk '{print $1, $2}')" = "Xvfb $TELA" ]
}
exigir_tela() { tela_nossa || falha "a tela virtual $TELA desta ferramenta não está aberta: nada foi enviado (rode 'iniciar')"; }
x() { DISPLAY="$TELA" "$@"; }

case "${1:-}" in
preparar)
    pacote=$2
    esperado=$3
    [ -f "$pacote" ] || falha "pacote não encontrado: $pacote"
    if [ -f "$esperado" ]; then
        esperado=$(awk -v n="$(basename "$pacote")" '$2 == n || $2 == "*"n { print $1 }' "$esperado")
        [ -n "$esperado" ] || falha "$(basename "$pacote") não está em $3"
    fi
    h=$(sha256sum "$pacote" | cut -d' ' -f1)
    [ "$h" = "$esperado" ] || falha "SHA-256 do pacote ($h) diferente do esperado ($esperado)"
    [ -e "$PASTA/app" ] && falha "$PASTA/app já existe: use uma pasta de campanha nova"
    mkdir -p "$PASTA/app"
    cp "$pacote" "$PASTA/app/pacote.AppImage"
    chmod +x "$PASTA/app/pacote.AppImage"
    (cd "$PASTA/app" && ./pacote.AppImage --appimage-extract >/dev/null) || falha "não deu para extrair o pacote"
    printf '%s  %s\n' "$h" "$(basename "$pacote")" > "$PASTA/pacote.sha256"
    echo "preparado: $(basename "$pacote") (SHA-256 $h) em $PASTA/app"
    ;;
iniciar)
    tam=$2
    shift 2
    [ -x "$APP" ] || falha "rode 'preparar' antes"
    tela_nossa && falha "a tela $TELA já está aberta por esta ferramenta (rode 'parar')"
    DISPLAY="$TELA" xdpyinfo >/dev/null 2>&1 && falha "a tela $TELA já existe e não é desta ferramenta"
    mkdir -p "$CASA/.config" "$CASA/.local/share" "$CASA/.cache" "$CASA/run" "$DADOS" "$PIDS"
    chmod 700 "$CASA/run"
    Xvfb "$TELA" -screen 0 "${tam}x24" -nolisten tcp >/dev/null 2>&1 &
    echo $! > "$PIDS/xvfb"
    for _ in $(seq 1 50); do DISPLAY="$TELA" xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done
    tela_nossa || falha "o Xvfb não abriu a tela $TELA"
    # Ambiente neutro (como app/testes/telas_d09.py): sem Wayland, sem a sessão
    # do desktop, sem barramento; e sem rede (unshare -rn).
    env -i PATH=/usr/bin:/bin DISPLAY="$TELA" HOME="$CASA" \
        XDG_CONFIG_HOME="$CASA/.config" XDG_DATA_HOME="$CASA/.local/share" XDG_CACHE_HOME="$CASA/.cache" \
        XDG_RUNTIME_DIR="$CASA/run" RUSTINHO_DADOS="$DADOS" LANG=pt_BR.UTF-8 LC_ALL=pt_BR.UTF-8 \
        QT_QPA_PLATFORM=xcb QT_QUICK_BACKEND=software QT_FORCE_STDERR_LOGGING=1 \
        unshare -rn "$APP" "$@" >>"$LOG" 2>&1 &
    echo $! > "$PIDS/app"
    for _ in $(seq 1 150); do x xdotool search --onlyvisible --class rustinho >/dev/null 2>&1 && break; sleep 0.2; done
    x xdotool search --onlyvisible --class rustinho >/dev/null 2>&1 || falha "a janela do Rustinho não apareceu (veja $LOG)"
    echo "iniciado na tela $TELA ($tam): $*"
    ;;
parar)
    for f in app xvfb; do
        [ -f "$PIDS/$f" ] && kill "$(cat "$PIDS/$f")" 2>/dev/null
        rm -f "$PIDS/$f"
    done
    sleep 1
    echo "parado"
    ;;
janela)
    exigir_tela
    for w in $(x xdotool search --class rustinho); do x xdotool windowmove "$w" 0 0 windowsize "$w" "${2%x*}" "${2#*x}"; done
    sleep 1
    echo "janela: $2"
    ;;
foto)
    exigir_tela
    mkdir -p "$(dirname "$2")"
    import -display "$TELA" -window root "$2" && echo "foto: $2"
    ;;
textos)
    exigir_tela
    n=$(wc -l < "$LOG")
    x xdotool key ctrl+shift+F12
    sleep 1.2
    tail -n +"$((n + 1))" "$LOG" | sed -n '/TEXTOS-DA-TELA-INICIO/,/TEXTOS-DA-TELA-FIM/p'
    ;;
captura)
    exigir_tela
    mkdir -p "$2"
    sleep "${4:-1.2}"
    import -display "$TELA" -window root "$2/$3.png" || falha "foto $3"
    "$0" textos > "$2/$3.txt"
    (cd "$2" && sha256sum "$3.png" "$3.txt" >> SHA256SUMS.txt)
    echo "$2/$3.png"
    ;;
tecla)
    exigir_tela
    shift
    x xdotool key --delay 80 "$@"
    sleep 0.6
    ;;
texto)
    exigir_tela
    x xdotool type --delay 40 "$2"
    sleep 0.4
    ;;
clique)
    exigir_tela
    x xdotool mousemove "$2" "$3" click 1
    sleep 0.7
    ;;
mouse)
    exigir_tela
    x xdotool mousemove "$2" "$3"
    sleep 0.5
    ;;
rolar)
    exigir_tela
    botao=5
    [ "${2:-baixo}" = cima ] && botao=4
    x xdotool click "$botao" click "$botao" click "$botao"
    sleep 0.8
    ;;
*)
    sed -n '2,25p' "$0" >&2
    exit 2
    ;;
esac
