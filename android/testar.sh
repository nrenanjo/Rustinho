#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, configura a loja,
# abre algumas telas e tira fotos. Tudo de mentira (Loja Modelo, senha 1234).
#
# Agora também CONFERE (C-09): se o APK não instala, se o app fecha sozinho,
# se uma tela não carrega ou se a câmera derruba o app, o teste fica
# vermelho. Desde a 0.9.18 também reprova quando o Rustinho mostra a tela
# "Não foi possível abrir" (o banco não abriu), quando nada do app aparece na
# tela (em branco ou só cinza, como no PEND-18), quando o app sai da frente e
# quando a tela esperada não aparece (a loja precisa ficar configurada e cada
# tela pedida precisa abrir de verdade). Antes disso o teste passava com a
# tela de erro (64 bits), com a tela cinza (32 bits) e sem nunca ter
# configurado a loja. A memória do app em cada conferência fica em
# fotos/memoria.txt. As fotos ficam em "fotos/" (a montagem guarda depois,
# mesmo se o teste falhar). Este script não usa nenhuma senha nem chave.
set -uo pipefail
mkdir -p fotos
: > fotos/falhas.txt
: > fotos/memoria.txt

PACOTE=io.github.nrenanjo.rustinho
ATIVIDADE=$PACOTE/org.qtproject.qt.android.bindings.QtActivity

falhou() { echo "FALHA: $*" | tee -a fotos/falhas.txt; }
foto() { sleep "${2:-4}"; adb exec-out screencap -p > "fotos/$1.png" || falhou "foto $1"; }
tecla() { adb shell input keyevent "$1"; sleep 1; }
texto() { adb shell input text "$1"; sleep 1; }
vivo() { [ -n "$(adb shell pidof $PACOTE | tr -d '\r')" ]; }
# O app está na frente (não voltou para a tela inicial do Android)?
na_frente() { adb shell dumpsys activity activities | grep -E "ResumedActivity" | grep -q "$PACOTE"; }
# Tem algo do app desenhado na tela? Uma tela em branco, ou só um degradê
# (a tela cinza do PEND-18), não tem nenhuma linha com cores diferentes de um
# lado ao outro. Olha só a metade de cima e o meio de cada linha, sem as
# barras do Android (em cima; de lado quando deitado). O teclado da tela,
# quando aparece, fica embaixo e não conta. Pede 10% das linhas olhadas com
# desenho: as telas do app dão 25% ou mais; a cinza dá 0%, e 3% com a caixa
# de texto do próprio Android por cima. Deitado e com teclado, o Android
# pode cobrir tudo com o editor dele: aí quem denuncia é o registro.
desenhada() {
    adb exec-out screencap > fotos/tela.raw 2> /dev/null || { echo "sem foto"; return 1; }
    python3 - fotos/tela.raw <<'PY'
import struct, sys
d = open(sys.argv[1], "rb").read()
if len(d) >= 12:
    w, h = struct.unpack_from("<II", d, 0)
    cab = len(d) - w * h * 4
    if w and h and cab in (12, 16):
        linhas = desenho = 0
        x0, x1 = w * 8 // 100, w * 90 // 100
        for y in range(h * 8 // 100, h // 2, 8):
            ini = cab + (y * w + x0) * 4
            linha = d[ini:ini + (x1 - x0) * 4]
            linhas += 1
            desenho += linha != linha[:4] * (x1 - x0)
        print(f"{desenho} de {linhas} linhas com desenho")
        sys.exit(0 if desenho * 10 >= linhas else 1)
    print(f"formato da foto desconhecido: {w}x{h}, {len(d)} bytes")
    sys.exit(1)
print(f"foto vazia ou cortada: {len(d)} bytes")
sys.exit(1)
PY
    local r=$?
    rm -f fotos/tela.raw
    return $r
}
# Memória do app agora (o resumo do dumpsys meminfo): no emulador de 64 bits
# o app chegou a 2,7 GB e o Android o matou; aqui fica de onde vem.
memoria() {
    {
        echo "== $1 ($(date +%T))"
        adb shell dumpsys meminfo $PACOTE 2> /dev/null |
            grep -E "^ *(Native Heap|Dalvik Heap|Gfx dev|EGL mtrack|GL mtrack|Other dev|Other mmap|\.so mmap|Unknown|TOTAL)" || echo "(sem app)"
        # Os maiores arquivos do app (pasta privada: só com o adb como root,
        # que as imagens google_apis do emulador permitem; no computador a
        # memória não cresce de uma abertura para outra, aqui cresce).
        echo "-- arquivos (kB)"
        adb shell "du -ak /data/data/$PACOTE /sdcard/Android/data/$PACOTE 2> /dev/null | sort -n | tail -8"
    } >> fotos/memoria.txt
}
# Em que tela o app está? A cada troca de tela o app escreve "Rustinho: tela
# pronta: <Tela>_..." no logcat (só no Android, diagnostico.cpp). Conta só o
# processo de agora.
tela_atual() {
    local pid
    pid=$(adb shell pidof $PACOTE | tr -d '\r')
    [ -n "$pid" ] || return 0
    adb logcat -d | awk -v p="$pid" '$3 == p' | grep -o "Rustinho: tela pronta: [A-Za-z]*" | tail -1 | sed 's/.*: //'
}
# Digita como uma pessoa: um toque em cada tecla do teclado da tela (só
# letras minúsculas; o teclado põe a maiúscula do começo). As posições são
# as do teclado QWERTY do emulador (1080 × 2400), iguais no Android 9 e 14.
digitar() {
    local palavra=$1 i letra xy
    for ((i = 0; i < ${#palavra}; i++)); do
        letra=${palavra:i:1}
        case "$letra" in
            q) xy="58 1717" ;; w) xy="166 1717" ;; e) xy="272 1717" ;; r) xy="379 1717" ;; t) xy="486 1717" ;;
            y) xy="593 1717" ;; u) xy="700 1717" ;; i) xy="808 1717" ;; o) xy="914 1717" ;; p) xy="1021 1717" ;;
            a) xy="112 1872" ;; s) xy="218 1872" ;; d) xy="326 1872" ;; f) xy="432 1872" ;; g) xy="539 1872" ;;
            h) xy="646 1872" ;; j) xy="754 1872" ;; k) xy="860 1872" ;; l) xy="967 1872" ;;
            z) xy="218 2027" ;; x) xy="326 2027" ;; c) xy="432 2027" ;; v) xy="539 2027" ;; b) xy="646 2027" ;;
            n) xy="754 2027" ;; m) xy="860 2027" ;;
            *) falhou "digitar: sem a tecla '$letra'"; return 1 ;;
        esac
        adb shell input tap $xy
        sleep 0.4
    done
}
# Toca no item que tem esse texto, achado na árvore de acessibilidade do
# Android (o Qt mostra os itens do QML para ela). Guarda a árvore em fotos/.
tocar_texto() {
    local alvo=$1 nome=$2 xy
    adb shell uiautomator dump /sdcard/tela.xml > /dev/null 2>&1
    adb shell cat /sdcard/tela.xml > "fotos/$nome.xml" 2> /dev/null
    xy=$(python3 - "fotos/$nome.xml" "$alvo" <<'PY'
import re, sys
x = open(sys.argv[1], encoding="utf-8", errors="replace").read()
for m in re.finditer(r"<node [^>]*>", x):
    n = m.group(0)
    if f'text="{sys.argv[2]}"' in n or f'content-desc="{sys.argv[2]}"' in n:
        b = re.search(r'bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', n)
        if b:
            x1, y1, x2, y2 = map(int, b.groups())
            print((x1 + x2) // 2, (y1 + y2) // 2)
            break
PY
)
    [ -n "$xy" ] || return 1
    adb shell input tap $xy
}
# Aviso do sistema por cima de tudo ("Pixel Launcher isn't responding": no
# emulador lento até o lançador do Android trava). Ele engole os toques e
# fica com o foco, então o teclado não abre no Rustinho.
# - Outro app travado (o lançador): "Close app". Com "Wait", o aviso do
#   lançador voltava 5 s depois, sempre por cima, e nenhum campo recebia o
#   cursor (execução 42947158143: 4 vezes seguidas, o Rustinho sem travar).
# - O próprio Rustinho: "Wait" (não fecha o app que está sendo testado) e
#   fica anotado à parte, para não se perder entre os avisos do emulador.
fechar_avisos() {
    local i titulo
    for i in 1 2 3; do
        adb shell uiautomator dump /sdcard/tela.xml > /dev/null 2>&1
        adb shell cat /sdcard/tela.xml > fotos/aviso.xml 2> /dev/null
        grep -q -E "isn(&apos;|.)t responding|não está respondendo" fotos/aviso.xml || return 0
        titulo=$(grep -o -E '"[^"]*(isn(&apos;|.)t responding|não está respondendo)' fotos/aviso.xml | head -1)
        echo "aviso do sistema por cima do app: $titulo" | tee -a fotos/avisos_do_sistema.txt
        if printf '%s' "$titulo" | grep -q "Rustinho"; then
            echo "  → o próprio Rustinho não respondeu: Wait" | tee -a fotos/avisos_do_sistema.txt fotos/rustinho_nao_respondeu.txt
            tocar_texto "Wait" aviso || tocar_texto "Aguardar" aviso || true
        else
            echo "  → outro app do emulador travado: Close app" | tee -a fotos/avisos_do_sistema.txt
            tocar_texto "Close app" aviso || tocar_texto "Fechar app" aviso \
                || tocar_texto "Wait" aviso || tocar_texto "Aguardar" aviso || true
        fi
        sleep 3
    done
}
# --- Digitação conferida (N03, dev.3.3) ------------------------------------
# No emulador lento, o aviso do sistema pode aparecer a qualquer momento e
# fechar o teclado: os toques nas teclas caíam no formulário e a perda só
# aparecia no fim, como recusa do cadastro ("As duas senhas não são iguais"),
# parecendo defeito do programa (execução 42907779755, tentativa 1). Agora
# cada campo é preenchido assim: sem aviso por cima, campo tocado, teclado
# aberto; depois, a foto do campo confirma que ele recebeu o texto. Se não
# recebeu, limpa e digita de novo (até 3 vezes) e anota em
# fotos/instabilidade.txt. A falha que vem do emulador sai marcada
# "[ambiente do emulador]", para não ser confundida com defeito do programa.
AMBIENTE="[ambiente do emulador]"
instavel() { echo "$(date +%T) $*" | tee -a fotos/instabilidade.txt; }
arvore() {
    adb shell uiautomator dump /sdcard/tela.xml > /dev/null 2>&1
    adb shell cat /sdcard/tela.xml > "fotos/$1.xml" 2> /dev/null
}
# O campo de texto n da tela (na ordem da árvore de acessibilidade):
# "n x1 y1 x2 y2".
campo() {
    python3 - "$1" "$2" <<'PY'
import re, sys
x = open(sys.argv[1], encoding="utf-8", errors="replace").read()
n = 0
for m in re.finditer(r"<node [^>]*>", x):
    no = m.group(0)
    if "TextField" not in (re.search(r'resource-id="([^"]*)"', no) or [None, ""])[1]:
        continue
    n += 1
    b = re.search(r'bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', no)
    if sys.argv[2] == "foco":
        if 'focused="true"' in no:
            print(n)
    elif n == int(sys.argv[2]) and b:
        print(n, *b.groups())
PY
}
# focar <n>: o cursor no campo n, com o teclado aberto. O app usa
# "adjustPan" (o Android desloca a janela para o campo ficar acima do
# teclado), então só se toca num campo com o teclado fechado: um campo
# coberto pelo teclado faria o toque cair numa tecla.
focar() {
    local n=$1 i j linha x1 y1 x2 y2
    for i in 1 2 3; do
        arvore "foco$n"
        [ "$(campo "fotos/foco$n.xml" foco)" = "$n" ] && teclado_aberto && return 0
        # O "voltar" com o teclado aberto fecha só o teclado.
        if teclado_aberto; then tecla KEYCODE_BACK; fi
        arvore "foco$n"
        linha=$(campo "fotos/foco$n.xml" "$n")
        [ -n "$linha" ] || { sleep 2; continue; }
        read -r _ x1 y1 x2 y2 <<< "$linha"
        # Fora da tela (embaixo, perto da barra do Android, ou em cima): rola.
        if [ "$y2" -gt 2250 ]; then adb shell input swipe 540 1600 540 1000 400; sleep 1; continue; fi
        if [ "$y1" -lt 200 ]; then adb shell input swipe 540 1000 540 1600 400; sleep 1; continue; fi
        adb shell input tap $(((x1 + x2) / 2)) $(((y1 + y2) / 2))
        for j in 1 2 3 4 5 6; do teclado_aberto && break; sleep 1; done
    done
    arvore "foco$n"
    [ "$(campo "fotos/foco$n.xml" foco)" = "$n" ] && teclado_aberto
}
# O que a acessibilidade diz do campo n. Campo com dica: a dica (o Qt a dá
# mesmo com texto digitado). Campo sem dica: o conteúdo ("••••" numa senha,
# o texto nos outros). Medido na execução 42933015475.
descricao() {
    python3 - "$1" "$2" <<'PY'
import re, sys
x = open(sys.argv[1], encoding="utf-8", errors="replace").read()
n = 0
for m in re.finditer(r"<node [^>]*>", x):
    no = m.group(0)
    if "TextField" not in (re.search(r'resource-id="([^"]*)"', no) or [None, ""])[1]:
        continue
    n += 1
    if n == int(sys.argv[2]):
        d = re.search(r'content-desc="([^"]*)"', no)
        print(d.group(1) if d else "")
PY
}
# O campo sem dica mostra o texto digitado (ou "•" no lugar de cada letra
# de uma senha)? Conta letras, não bytes (o "•" ocupa 3 bytes).
mostra_o_texto() {
    python3 -c 'import sys; v, p = sys.argv[1], sys.argv[2]; sys.exit(0 if len(v) == len(p) and (v.lower() == p.lower() or set(v) == {"\u2022"}) else 1)' "$1" "$2"
}
aviso_na_tela() {
    arvore aviso
    grep -q -E "isn(&apos;|.)t responding|não está respondendo" fotos/aviso.xml
}
teclado_aberto() { adb shell dumpsys input_method 2> /dev/null | grep -q "mInputShown=true"; }
# Mede o campo na tela agora: "direita esquerda" = linhas com desenho na faixa
# da direita (de 35% a 95% da largura) e colunas com desenho na da esquerda.
# A dica do campo ocupa as duas; um texto curto digitado (nome, senha) só a
# esquerda; o cursor sozinho, 2 ou 3 colunas. Medido nas fotos da execução
# 42907779755: campo da loja vazio = 9 linhas à direita; com "Loja" = 0
# linhas à direita e 75 colunas à esquerda.
medir_campo() {
    adb exec-out screencap > fotos/tela.raw 2> /dev/null || { echo "-1 -1"; return; }
    python3 - fotos/tela.raw "$@" <<'PY'
import struct, sys
from collections import Counter
d = open(sys.argv[1], "rb").read()
x1, y1, x2, y2 = map(int, sys.argv[2:6])
w, h = struct.unpack_from("<II", d, 0)
cab = len(d) - w * h * 4
if not w or cab not in (12, 16) or y2 > h or x2 > w:
    print(-1, -1)
    sys.exit()
def px(x, y):
    i = cab + (y * w + x) * 4
    return d[i:i + 3]
def difere(a, b):
    return max(abs(a[k] - b[k]) for k in range(3)) > 40
larg = x2 - x1
ys = range(y1 + 28, y2 - 28)
fundo = Counter(px(x, y) for y in ys[::4] for x in range(x1 + 10, x2 - 10, 7)).most_common(1)[0][0]
dx0, dx1 = x1 + larg * 35 // 100, x1 + larg * 95 // 100
linhas = sum(1 for y in ys[::3] if sum(1 for x in range(dx0, dx1, 2) if difere(px(x, y), fundo)) > 2)
colunas = sum(1 for x in range(x1 + 10, dx0) if any(difere(px(x, y), fundo) for y in ys[::2]))
print(linhas, colunas)
PY
    rm -f fotos/tela.raw
}
# Fecha o teclado (o "voltar" só quando ele está aberto: com ele fechado, o
# "voltar" sairia da tela).
fechar_teclado() {
    local i
    teclado_aberto && tecla KEYCODE_BACK
    for i in 1 2 3 4 5 6; do teclado_aberto || break; sleep 0.5; done
    sleep 0.5
}
# medir_sem_teclado <n>: a foto do campo n com o teclado FECHADO e o campo
# dentro da tela. Com o teclado aberto, o Android às vezes não desloca a
# janela a tempo e o campo fica embaixo do teclado: a foto mediria o teclado
# (execução 42944215226). Imprime "direita esquerda" (veja medir_campo).
medir_sem_teclado() {
    local n=$1 i linha x1 y1 x2 y2
    fechar_teclado
    for i in 1 2 3; do
        arvore "campo$n"
        linha=$(campo "fotos/campo$n.xml" "$n")
        [ -n "$linha" ] || { echo "-1 -1"; return; }
        read -r _ x1 y1 x2 y2 <<< "$linha"
        if [ "$y2" -gt 2250 ]; then adb shell input swipe 540 1600 540 1000 400; sleep 1; continue; fi
        if [ "$y1" -lt 200 ]; then adb shell input swipe 540 1000 540 1600 400; sleep 1; continue; fi
        medir_campo "$x1" "$y1" "$x2" "$y2"
        return
    done
    echo "-1 -1"
}
# Ensaio permanente da conferência: na 1ª tentativa do 1º campo, o roteiro
# não digita nada, como se o emulador tivesse perdido tudo. A conferência
# tem de perceber e digitar de novo; se não perceber, o roteiro está errado.
ENSAIO_PENDENTE=1
# preencher <n> <texto> <teclas|texto>: preenche o campo n e confere.
# "teclas" toca tecla por tecla (nomes: o "input text" troca letras de lugar
# no teclado do Android 14); "texto" usa o "input text" (senhas: campo sem
# sugestões do teclado).
preencher() {
    local n=$1 palavra=$2 modo=$3 t linha x1 y1 x2 y2 antes esq0 depois esq dica visto recebeu ensaio max=3
    # O ensaio não gasta uma das 3 tentativas de verdade.
    [ -n "$ENSAIO_PENDENTE" ] && max=4
    for t in $(seq 1 $max); do
        fechar_avisos
        if ! focar "$n"; then instavel "campo $n: não deu para pôr o cursor no campo com o teclado aberto (tentativa $t)"; continue; fi
        sleep 1
        # Com o teclado aberto, a janela pode ter sido deslocada: a posição de agora.
        arvore "campo$n"
        linha=$(campo "fotos/campo$n.xml" "$n")
        if [ -z "$linha" ]; then instavel "campo $n: sumiu ao abrir o teclado (tentativa $t)"; continue; fi
        read -r _ x1 y1 x2 y2 <<< "$linha"
        # Limpa o que uma tentativa anterior deixou.
        adb shell input keyevent KEYCODE_MOVE_END
        adb shell input keyevent $(printf 'KEYCODE_DEL %.0s' $(seq 1 24))
        sleep 1
        # Vazio, o campo com dica mostra a dica na acessibilidade; o sem dica,
        # nada. O com dica é conferido pela foto (a dica some); o sem dica,
        # pela acessibilidade (que mostra o que foi digitado). A foto não serve
        # para o sem dica: na tela de entrada, o campo da senha fica embaixo do
        # teclado (execução 42933015475).
        arvore "campo$n"
        dica=$(descricao "fotos/campo$n.xml" "$n")
        if [ -n "$dica" ] && mostra_o_texto "$dica" "$palavra"; then
            instavel "campo $n: não limpou o que a tentativa anterior deixou (tentativa $t)"
            continue
        fi
        antes=-1; esq0=-1
        if [ -n "$dica" ]; then
            # A foto de antes, com o teclado fechado; depois o cursor volta.
            read -r antes esq0 <<< "$(medir_sem_teclado "$n")"
            if [ "$antes" -le 2 ]; then
                # Com o teclado fechado e o campo vazio, a dica TEM de aparecer:
                # se não aparece, a foto não está medindo o campo.
                falhou "[roteiro] campo $n: a dica não aparece na foto do campo vazio (dir=$antes esq=$esq0)"
                return 1
            fi
            if ! focar "$n"; then instavel "campo $n: o cursor não voltou ao campo (tentativa $t)"; continue; fi
            sleep 1
        fi
        ensaio=0
        if [ -n "$ENSAIO_PENDENTE" ]; then
            ENSAIO_PENDENTE=""
            ensaio=1
            echo "campo $n, tentativa $t: digitação omitida de propósito (ensaio da conferência)" >> fotos/ensaio_n03.txt
        elif [ "$modo" = teclas ]; then
            digitar "$palavra"
        else
            texto "$palavra"
        fi
        if aviso_na_tela; then instavel "campo $n: aviso do sistema durante a digitação (tentativa $t)"; continue; fi
        recebeu=1
        if [ -n "$dica" ]; then
            # A dica sumiu da faixa da direita e apareceu texto à esquerda
            # (foto com o teclado fechado, como a de antes).
            read -r depois esq <<< "$(medir_sem_teclado "$n")"
            echo "campo $n tentativa $t: dica \"$dica\" antes(dir=$antes esq=$esq0) depois(dir=$depois esq=$esq)" >> fotos/medidas_campos.txt
            [ "$antes" -gt 2 ] && [ "$depois" -ge 0 ] && [ "$depois" -le 1 ] && [ "$esq" -ge 8 ] && recebeu=0
        else
            # A acessibilidade mostra o texto digitado (ou um "•" por letra).
            arvore "campo$n"
            visto=$(descricao "fotos/campo$n.xml" "$n")
            echo "campo $n tentativa $t: sem dica; a acessibilidade mostra \"$visto\"" >> fotos/medidas_campos.txt
            mostra_o_texto "$visto" "$palavra" && recebeu=0
        fi
        if [ "$recebeu" = 0 ]; then
            if [ "$ensaio" = 1 ]; then
                falhou "[roteiro] a conferência do campo $n não percebeu a digitação omitida de propósito"
                return 1
            fi
            return 0
        fi
        if [ "$ensaio" = 1 ]; then
            echo "  → percebida: o campo não recebeu o texto (ensaio aprovado)" >> fotos/ensaio_n03.txt
        else
            instavel "campo $n: o texto não chegou ao campo (tentativa $t; veja medidas_campos.txt)"
        fi
    done
    falhou "$AMBIENTE campo $n: não deu para digitar \"$palavra\" em 3 tentativas (veja instabilidade.txt)"
    return 1
}
# Espera o app chegar à tela pedida; se não chegar no prazo, reprova.
# chegou <Tela> <segundos>: espera sem reprovar (quem chama decide).
chegou() {
    local i
    for ((i = 0; i < $2; i += 2)); do
        [ "$(tela_atual)" = "$1" ] && return 0
        sleep 2
    done
    return 1
}
esperar_tela() {
    local nome=$1 limite=$2 onde=$3 atual
    chegou "$nome" "$limite" && return 0
    atual=$(tela_atual)
    # Com um aviso do sistema por cima, a culpa pode ser do emulador.
    if aviso_na_tela; then
        falhou "$AMBIENTE $onde: a tela $nome não apareceu em ${limite}s (está em: ${atual:-nenhuma}; aviso do sistema na tela)"
    else
        falhou "$onde: a tela $nome não apareceu em ${limite}s (está em: ${atual:-nenhuma})"
    fi
    return 1
}
# O app está aberto, na frente, desenhado e não teve erro grave desde a
# última conferência?
conferir() {
    local onde=$1 d
    vivo || falhou "$onde: o app fechou"
    na_frente || falhou "$onde: o app não está na frente"
    d=$(desenhada) || falhou "$onde: nada do app na tela ($d)"
    memoria "$onde"
    adb logcat -d > fotos/logcat.txt
    # Fechou com erro, uma tela não carregou (componente que falta), o banco
    # não abriu (tela "Não foi possível abrir") ou o Qt não conseguiu pôr o
    # desenho na tela (sem contexto do OpenGL, nem o modo "software" aparece).
    if grep -E "FATAL EXCEPTION|Fatal signal|is not a type|Type [A-Za-z]+ unavailable|Cannot load library|Rustinho: erro ao abrir|Failed to create QRhi|QEGLPlatformContext: Failed to create context" fotos/logcat.txt \
        | grep -v -F -f fotos/ja_visto.txt > fotos/novo.txt 2>/dev/null && [ -s fotos/novo.txt ]; then
        falhou "$onde: $(head -3 fotos/novo.txt | tr '\n' ' ')"
        cat fotos/novo.txt >> fotos/ja_visto.txt
    fi
}
# Guarda o registro, o diagnóstico e as listas de erros; reprova se houve
# falha. Chamado no fim, ou antes se não deu para configurar a loja (aí as
# telas seguintes não teriam o que conferir).
encerrar() {
    adb logcat -d | grep -i -E "camera|multimedia" | tail -40 > fotos/camera.txt
    adb shell pidof $PACOTE > fotos/pid.txt || echo "fechou" > fotos/pid.txt
    adb logcat -d > fotos/logcat.txt
    grep -F "Rustinho: " fotos/logcat.txt >> fotos/diag.txt || true
    # O diagnóstico do próprio app (PEND-18), quando o Android deixa ler a pasta.
    adb shell cat "/sdcard/Android/data/$PACOTE/files/diagnostico.txt" > fotos/diagnostico.txt 2> /dev/null || true
    grep -E "$(adb shell pidof $PACOTE | tr -d '\r')|AndroidRuntime|DEBUG" fotos/logcat.txt | grep -i -E "qml|warn|error|fatal|crash|qt" | tail -150 > fotos/erros.txt || true
    grep -E "lowmemorykiller: Kill '$PACOTE'|ANR in $PACOTE" -A 3 fotos/logcat.txt > fotos/memoria_mortes.txt || true
    # A causa (N03) é a da PRIMEIRA falha: "[ambiente do emulador]" =
    # ambiente; "[roteiro]" (a conferência errou) = roteiro de teste; sem
    # marca = programa. Falhas sem marca depois de uma do ambiente podem ser
    # consequência dela (ex.: sem entrar, a tela seguinte não abre): o
    # resultado diz quantas, e todas ficam em falhas.txt. Vai para o resultado
    # da CI e para o manifesto.
    if [ ! -s fotos/falhas.txt ]; then
        echo nenhuma > fotos/causa.txt
    else
        local c outras
        case "$(head -1 fotos/falhas.txt)" in
            *"$AMBIENTE"*) c="ambiente do emulador" ;;
            *"[roteiro]"*) c="roteiro de teste" ;;
            *) c="programa" ;;
        esac
        outras=$(grep -c -v -F -e "$AMBIENTE" -e "[roteiro]" fotos/falhas.txt)
        if [ "$c" != programa ] && [ "$outras" -gt 0 ]; then
            c="$c (a primeira falha; depois dela, $outras sem marca, talvez consequência: veja falhas.txt)"
        fi
        echo "$c" > fotos/causa.txt
    fi
    ls -la fotos
    if [ -s fotos/falhas.txt ]; then
        echo "O teste encontrou problemas:"
        cat fotos/falhas.txt
        exit 1
    fi
    echo "Tudo certo."
    exit 0
}
: > fotos/ja_visto.txt

# adb como root (só para ler o tamanho dos arquivos do app; veja memoria()).
adb root > fotos/adb_root.txt 2>&1 || true
adb wait-for-device
sleep 3
APK=$(ls saida/*.apk | head -1)
if ! adb install -r "$APK" 2>&1 | tee fotos/instalar.txt | grep -q "Success"; then
    falhou "o APK não instalou"
    exit 1
fi
adb shell pm grant $PACOTE android.permission.CAMERA || true
# Só para experiência (execução manual da CI com "desenho"): força o modo de
# desenho pelo desenho.txt da pasta de diagnóstico do app (main.cpp).
case "${DESENHO:-}" in
    gpu | software)
        adb shell "mkdir -p /sdcard/Android/data/$PACOTE/files && echo $DESENHO > /sdcard/Android/data/$PACOTE/files/desenho.txt" &&
            echo "modo de desenho pedido no desenho.txt: $DESENHO" | tee -a fotos/diag.txt
        ;;
esac
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb logcat -c

adb shell am start -n $ATIVIDADE
esperar_tela TelaBoasVindas 90 "primeira vez"
foto 01-primeira-vez 5
conferir "primeira vez"
# O teclado já abre no primeiro campo: guarda o que o Android diz dele (a
# conferência "teclado aberto" lê o mInputShown daqui).
adb shell dumpsys input_method > fotos/teclado_dumpsys.txt 2>&1 || true
# Os campos da tela de boas-vindas, na ordem: 1 nome da loja, 2 nome deste
# aparelho (já vem "Celular 1"), 3 seu nome, 4 senha, 5 repita a senha. Os
# nomes vão tecla por tecla, como uma pessoa digita (com "input text" o
# teclado do Android 14 trocava letras de lugar: "Loja" virava "ojaL", o mesmo
# sintoma do comentário de Campo.qml sobre "texto previsto"). As senhas vão
# por "input text" (campo de senha não tem sugestões do teclado).
#
# O que o app responde decide o resultado (registro do próprio app):
# "cadastro inicial pedido" = o app recebeu o pedido; "cadastro inicial
# recusado (campo): motivo" = o app recusou. Recusa depois de instabilidade
# do emulador = ambiente (digita tudo de novo). Recusa com os campos
# conferidos e sem instabilidade, duas vezes = defeito do programa.
arvore 01-arvore
pedidos() { adb logcat -d | grep -c "Rustinho: cadastro inicial pedido"; }
recusa_do_app() { adb logcat -d | grep -o "Rustinho: cadastro inicial recusado.*" | tail -1; }
instabilidades() { if [ -f fotos/instabilidade.txt ]; then wc -l < fotos/instabilidade.txt; else echo 0; fi; }
configurar_loja() {
    local tentativa p0 i0 recusa
    for tentativa in 1 2; do
        i0=$(instabilidades)
        preencher 1 loja teclas || return 1
        [ "$tentativa" = 1 ] && foto 01a-loja 1
        preencher 3 maria teclas || return 1
        [ "$tentativa" = 1 ] && foto 01b-nome 1
        preencher 4 1234 texto || return 1
        preencher 5 1234 texto || return 1
        foto "01c-preenchido-$tentativa" 1
        p0=$(pedidos)
        tecla KEYCODE_ENTER
        # A senha passa pelo Argon2: no emulador (código ARM traduzido) demora.
        chegou TelaInicio 120 && return 0
        foto "02-sem-comecar-$tentativa" 1
        if [ "$(pedidos)" -gt "$p0" ]; then
            recusa=$(recusa_do_app)
            if [ -z "$recusa" ]; then
                falhou "configurar a loja: o cadastro foi pedido e o Início não apareceu em 120s"
                return 1
            fi
            if [ "$(instabilidades)" -gt "$i0" ] || aviso_na_tela; then
                instavel "configurar a loja: o app recusou o cadastro depois de instabilidade do emulador ($recusa)"
            elif [ "$tentativa" = 2 ]; then
                falhou "configurar a loja: o app recusou duas vezes o cadastro com os campos conferidos ($recusa)"
                return 1
            else
                instavel "configurar a loja: o app recusou o cadastro com os campos conferidos ($recusa); de novo, para separar emulador de programa"
            fi
        elif aviso_na_tela; then
            instavel "configurar a loja: o Enter no último campo não chegou ao app (aviso do sistema na tela)"
        else
            falhou "configurar a loja: o Enter no último campo não começou o cadastro"
            # Segunda via, como uma pessoa faria (o "voltar" fecha só o teclado).
            teclado_aberto && tecla KEYCODE_BACK
            adb shell input swipe 540 1800 540 600 400
            sleep 2
            tocar_texto "Começar" 02-arvore || { falhou "configurar a loja: o botão Começar não foi achado na tela"; return 1; }
            chegou TelaInicio 120 && return 0
            falhou "configurar a loja: nem o botão Começar levou ao Início ($(recusa_do_app))"
            return 1
        fi
    done
    falhou "$AMBIENTE configurar a loja: o cadastro não passou em 2 tentativas por instabilidade do emulador (veja instabilidade.txt)"
    return 1
}
configurar_loja
foto 02-inicio 3
conferir "configurar a loja"
if [ "$(tela_atual)" != TelaInicio ]; then
    # Herda a causa: se o cadastro parou por instabilidade do emulador, esta
    # falha também é do ambiente.
    if tail -1 fotos/falhas.txt 2> /dev/null | grep -q -F "$AMBIENTE"; then
        falhou "$AMBIENTE a loja não ficou configurada: as telas seguintes não teriam o que conferir"
    else
        falhou "a loja não ficou configurada: as telas seguintes não teriam o que conferir"
    fi
    encerrar
fi
# Modo escuro: a lua, ao lado de "Trocar usuário". No teste de 32 bits fica
# escuro até o fim; no de 64, volta para o sol.
adb shell input tap 187 472
foto 02b-escuro 3
[ -n "${CURTO:-}" ] || { adb shell input tap 91 472; sleep 2; }
adb shell input swipe 500 1800 500 600 400
foto 03-inicio-rolado 3
conferir "início"

# Abre o app de novo direto numa tela (só nos testes), entra com a senha e
# espera a tela pedida aparecer.
abrir() {
    adb shell am force-stop $PACOTE
    sleep 2
    adb shell am start -n $ATIVIDADE --es rustinho "'--abrir $1'"
    esperar_tela TelaEntrar 90 "$1 (entrada)" || return 0
    # A senha do cadastro (1234) abre a tela: confere também que o cadastro
    # gravou a senha digitada, letra por letra.
    preencher 1 1234 texto || return 0
    tecla KEYCODE_ENTER
    esperar_tela "$1" 120 "$1"
}
TELAS="TelaVender TelaPecas TelaCaixa TelaCompras TelaConfiguracoes TelaPainel TelaFinanceiro \
       TelaPeca TelaPessoa TelaPessoas TelaReceber TelaOrcamentos TelaOrdens TelaOrdem TelaEstoque \
       TelaEtiquetas TelaTrocas TelaUsuarios TelaFiscal"
# Teste curto (32 bits, mais lento no emulador): só algumas telas.
[ -n "${CURTO:-}" ] && TELAS="TelaVender TelaConfiguracoes TelaPeca TelaFinanceiro"
for t in $TELAS; do
    abrir $t
    foto "10-$t" 3
    adb shell input swipe 540 1700 540 700 500
    foto "11-$t-rolada" 3
    conferir "$t"
done
# Argumento estranho mandado por outro app: ignorado, o app não fecha (C-13).
adb shell am force-stop $PACOTE; sleep 2
adb shell am start -n $ATIVIDADE --es rustinho "'--abrir ../../x --props {quebrado'"
esperar_tela TelaEntrar 90 "argumento estranho"
conferir "argumento estranho"
# Deitado.
abrir TelaVender
adb shell settings put system user_rotation 1
adb shell wm user-rotation lock 1
foto 20-deitado-vender 8
abrir TelaPecas
foto 21-deitado-pecas 6
conferir "deitado"
adb shell settings put system user_rotation 0
adb shell wm user-rotation lock 0
sleep 4
# Voltar do Android: volta ao Início (o app continua na frente).
tecla KEYCODE_BACK
esperar_tela TelaInicio 30 "voltar do Android"
foto 30-voltar 3
conferir "voltar do Android"
# A câmera por último (no emulador, ela mostra uma sala de mentira):
# um toque no botão da câmera, no fim do campo de busca da venda.
abrir TelaVender
foto 14-vender-antes-da-camera 5
adb shell input tap 985 414
foto 15-camera 12
conferir "câmera"
encerrar
