#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, configura a loja,
# abre algumas telas e tira fotos. Tudo de mentira (Loja Modelo, senha 1234).
#
# Agora também CONFERE (C-09): se o APK não instala, se o app fecha sozinho,
# se uma tela não carrega ou se a câmera derruba o app, o teste fica
# vermelho. Desde a 0.9.18 também reprova quando o Rustinho mostra a tela
# "Não foi possível abrir" (o banco não abriu), quando nada do app aparece na
# tela (em branco ou só cinza, como no PEND-18) e quando o app sai da frente.
# Antes disso o teste passava com a tela de erro (64 bits) e com a tela cinza
# (32 bits). As fotos ficam em "fotos/" (a montagem guarda depois, mesmo se
# o teste falhar). Este script não usa nenhuma senha nem chave.
set -uo pipefail
mkdir -p fotos
: > fotos/falhas.txt

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
# quando aparece, fica embaixo e não conta.
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
        sys.exit(0 if desenho * 50 >= linhas else 1)
print("formato da foto desconhecido")
sys.exit(1)
PY
    local r=$?
    rm -f fotos/tela.raw
    return $r
}
# O app está aberto, na frente, desenhado e não teve erro grave desde a
# última conferência?
conferir() {
    local onde=$1 d
    vivo || falhou "$onde: o app fechou"
    na_frente || falhou "$onde: o app não está na frente"
    d=$(desenhada) || falhou "$onde: nada do app na tela ($d)"
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
: > fotos/ja_visto.txt

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
foto 01-primeira-vez 40
conferir "primeira vez"
# O primeiro campo (nome da loja) já vem com o cursor.
# Nome da loja e da pessoa pelo teclado da tela (toques nas teclas), como
# uma pessoa digita: "Loja" e "Maria".
for xy in "972 1872" "918 1717" "756 1872" "108 1872"; do adb shell input tap $xy; sleep 0.6; done
foto 01a-loja-tocada 2
tecla KEYCODE_ENTER
tecla KEYCODE_ENTER
for xy in "864 2027" "108 1872" "378 1717" "810 1717" "108 1872"; do adb shell input tap $xy; sleep 0.6; done
foto 01b-nome-tocado 2
tecla KEYCODE_ENTER
texto "1234"; tecla KEYCODE_ENTER
texto "1234"; tecla KEYCODE_ENTER
foto 02-inicio 6
conferir "configurar a loja"
# Modo escuro: a lua, ao lado de "Trocar usuário". No teste de 32 bits fica
# escuro até o fim; no de 64, volta para o sol.
adb shell input tap 187 472
foto 02b-escuro 3
[ -n "${CURTO:-}" ] || { adb shell input tap 91 472; sleep 2; }
adb shell input swipe 500 1800 500 600 400
foto 03-inicio-rolado 3
conferir "início"

abrir() {
    adb shell am force-stop $PACOTE
    sleep 2
    adb shell am start -n $ATIVIDADE --es rustinho "'--abrir $1'"
    sleep 15
    texto "1234"; tecla KEYCODE_ENTER
}
TELAS="TelaVender TelaPecas TelaCaixa TelaCompras TelaConfiguracoes TelaPainel TelaFinanceiro \
       TelaPeca TelaPessoa TelaPessoas TelaReceber TelaOrcamentos TelaOrdens TelaOrdem TelaEstoque \
       TelaEtiquetas TelaTrocas TelaUsuarios TelaFiscal"
# Teste curto (32 bits, mais lento no emulador): só algumas telas.
[ -n "${CURTO:-}" ] && TELAS="TelaVender TelaConfiguracoes TelaPeca TelaFinanceiro"
for t in $TELAS; do
    abrir $t
    foto "10-$t" 5
    adb shell input swipe 540 1700 540 700 500
    foto "11-$t-rolada" 3
    conferir "$t"
done
# Argumento estranho mandado por outro app: ignorado, o app não fecha (C-13).
adb shell am force-stop $PACOTE; sleep 2
adb shell am start -n $ATIVIDADE --es rustinho "'--abrir ../../x --props {quebrado'"
sleep 15
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
foto 30-voltar 3
conferir "voltar do Android"
# A câmera por último (no emulador, ela mostra uma sala de mentira):
# um toque no botão da câmera, no fim do campo de busca da venda.
abrir TelaVender
foto 14-vender-antes-da-camera 12
adb shell input tap 985 414
foto 15-camera 12
conferir "câmera"
adb logcat -d | grep -i -E "camera|multimedia" | tail -40 > fotos/camera.txt
adb shell pidof $PACOTE > fotos/pid.txt || echo "fechou" > fotos/pid.txt
adb logcat -d > fotos/logcat.txt
grep -i "Rustinho:" fotos/logcat.txt >> fotos/diag.txt || true
# O diagnóstico do próprio app (PEND-18), quando o Android deixa ler a pasta.
adb shell cat "/sdcard/Android/data/$PACOTE/files/diagnostico.txt" > fotos/diagnostico.txt 2> /dev/null || true
grep -E "$(adb shell pidof $PACOTE | tr -d '\r')|AndroidRuntime|DEBUG" fotos/logcat.txt | grep -i -E "qml|warn|error|fatal|crash|qt" | tail -150 > fotos/erros.txt || true
ls -la fotos
if [ -s fotos/falhas.txt ]; then
    echo "O teste encontrou problemas:"
    cat fotos/falhas.txt
    exit 1
fi
echo "Tudo certo."
