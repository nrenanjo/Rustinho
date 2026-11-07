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
# emulador lento até o lançador do Android trava). Ele engole os toques no
# teclado; toca em "Wait" para ele sair (não fecha app nenhum).
fechar_avisos() {
    local i
    for i in 1 2 3; do
        adb shell uiautomator dump /sdcard/tela.xml > /dev/null 2>&1
        adb shell cat /sdcard/tela.xml > fotos/aviso.xml 2> /dev/null
        grep -q -E "isn(&apos;|.)t responding|não está respondendo" fotos/aviso.xml || return 0
        echo "aviso do sistema por cima do app: $(grep -o -E '"[^"]*(isn(&apos;|.)t responding|não está respondendo)' fotos/aviso.xml | head -1)" | tee -a fotos/avisos_do_sistema.txt
        tocar_texto "Wait" aviso || tocar_texto "Aguardar" aviso || true
        sleep 3
    done
}
# Espera o app chegar à tela pedida; se não chegar no prazo, reprova.
esperar_tela() {
    local nome=$1 limite=$2 onde=$3 atual="" i
    for ((i = 0; i < limite; i += 2)); do
        atual=$(tela_atual)
        [ "$atual" = "$nome" ] && return 0
        sleep 2
    done
    falhou "$onde: a tela $nome não apareceu em ${limite}s (está em: ${atual:-nenhuma})"
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
# O primeiro campo (nome da loja) já vem com o cursor e o teclado da tela
# aberto. Os nomes vão tecla por tecla, como uma pessoa digita (com "input
# text" o teclado do Android 14 trocava letras de lugar: "Loja" virava "ojaL",
# o mesmo sintoma do comentário de Campo.qml sobre "texto previsto"). As senhas
# vão por "input text" (campo de senha não tem sugestões do teclado).
adb shell uiautomator dump /sdcard/tela.xml > /dev/null 2>&1 && adb shell cat /sdcard/tela.xml > fotos/01-arvore.xml 2> /dev/null
fechar_avisos
digitar "loja"
foto 01a-loja 2
tecla KEYCODE_ENTER
tecla KEYCODE_ENTER
fechar_avisos
digitar "maria"
foto 01b-nome 2
tecla KEYCODE_ENTER
fechar_avisos
texto "1234"; tecla KEYCODE_ENTER
fechar_avisos
texto "1234"; tecla KEYCODE_ENTER
# A senha passa pelo Argon2: no emulador (código ARM traduzido) demora.
if ! esperar_tela TelaInicio 120 "configurar a loja (Enter no último campo)"; then
    # O Enter no último campo não começou: tenta o botão, como uma pessoa
    # faria (o "voltar" fecha só o teclado).
    foto 02-sem-comecar 1
    tecla KEYCODE_BACK
    adb shell input swipe 540 1800 540 600 400
    sleep 2
    tocar_texto "Começar" 02-arvore || falhou "configurar a loja: o botão Começar não foi achado na tela"
    esperar_tela TelaInicio 120 "configurar a loja (botão Começar)"
fi
foto 02-inicio 3
conferir "configurar a loja"
[ "$(tela_atual)" = TelaInicio ] || { falhou "a loja não ficou configurada: as telas seguintes não teriam o que conferir"; encerrar; }
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
    fechar_avisos
    texto "1234"; tecla KEYCODE_ENTER
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
