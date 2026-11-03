#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, configura a loja,
# abre algumas telas e tira fotos. Tudo de mentira (Loja Modelo, senha 1234).
#
# Agora também CONFERE (C-09): se o APK não instala, se o app fecha sozinho,
# se uma tela não carrega ou se a câmera derruba o app, o teste fica
# vermelho. As fotos ficam em "fotos/" (a montagem guarda depois, mesmo se
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
# O app está aberto e não teve erro grave desde a última conferência?
conferir() {
    local onde=$1
    vivo || falhou "$onde: o app fechou"
    adb logcat -d > fotos/logcat.txt
    # Fechou com erro, ou uma tela não carregou (componente que falta).
    if grep -E "FATAL EXCEPTION|Fatal signal|is not a type|Type [A-Za-z]+ unavailable|Cannot load library" fotos/logcat.txt \
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
# Voltar do Android: volta ao Início.
tecla KEYCODE_BACK
foto 30-voltar 3
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
grep -E "$(adb shell pidof $PACOTE | tr -d '\r')|AndroidRuntime|DEBUG" fotos/logcat.txt | grep -i -E "qml|warn|error|fatal|crash|qt" | tail -150 > fotos/erros.txt || true
ls -la fotos
if [ -s fotos/falhas.txt ]; then
    echo "O teste encontrou problemas:"
    cat fotos/falhas.txt
    exit 1
fi
echo "Tudo certo."
