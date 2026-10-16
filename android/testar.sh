#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, configura a loja,
# abre algumas telas e tira fotos. Tudo de mentira (Loja Modelo, senha 1234).
set -x
mkdir -p fotos
APK=$(ls saida/*.apk | head -1)
adb install -r "$APK" 2>&1 | tee fotos/instalar.txt
PACOTE=io.github.nrenanjo.rustinho
ATIVIDADE=$PACOTE/org.qtproject.qt.android.bindings.QtActivity
adb shell pm grant $PACOTE android.permission.CAMERA || true
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb logcat -c
foto() { sleep "${2:-4}"; adb exec-out screencap -p > "fotos/$1.png"; }
tecla() { adb shell input keyevent "$1"; sleep 1; }
texto() { adb shell input text "$1"; sleep 1; }

adb shell am start -n $ATIVIDADE
foto 01-primeira-vez 40
# O primeiro campo (nome da loja) já vem com o cursor.
# Nome da loja e da pessoa pelo teclado da tela (toques nas teclas), como
# uma pessoa digita: "Loja" e "Maria".
for xy in "972 1872" "918 1717" "756 1872" "108 1872"; do adb shell input tap $xy; sleep 0.6; done
foto 01a-loja-tocada 2
tecla KEYCODE_ENTER
tecla KEYCODE_ENTER
# O nome pelo teclado da tela (toques nas teclas), como uma pessoa digita.
for xy in "864 2027" "108 1872" "378 1717" "810 1717" "108 1872"; do adb shell input tap $xy; sleep 0.6; done
foto 01b-nome-tocado 2
tecla KEYCODE_ENTER
texto "1234"; tecla KEYCODE_ENTER
texto "1234"; tecla KEYCODE_ENTER
foto 02-inicio 6
adb shell input swipe 500 1800 500 600 400
foto 03-inicio-rolado 3

abrir() {
    adb shell am force-stop $PACOTE
    sleep 2
    adb shell am start -n $ATIVIDADE --es rustinho "'--abrir $1'"
    sleep 15
    texto "1234"; tecla KEYCODE_ENTER
}
for t in TelaVender TelaPecas TelaCaixa TelaCompras TelaConfiguracoes TelaPainel TelaFinanceiro \
         TelaPeca TelaPessoa TelaPessoas TelaReceber TelaOrcamentos TelaOrdens TelaOrdem TelaEstoque \
         TelaEtiquetas TelaTrocas TelaUsuarios TelaFiscal; do
    abrir $t
    foto "10-$t" 5
    adb shell input swipe 540 1700 540 700 500
    foto "11-$t-rolada" 3
done
# A câmera (no emulador, ela mostra uma sala de mentira).
abrir "TelaVender --camera"
foto 15-camera 8
# Deitado.
abrir TelaVender
adb shell settings put system user_rotation 1
adb shell wm user-rotation lock 1
foto 20-deitado-vender 8
abrir TelaPecas
foto 21-deitado-pecas 6
adb shell settings put system user_rotation 0
adb shell wm user-rotation lock 0
sleep 4
# Voltar do Android: volta ao Início.
tecla KEYCODE_BACK
foto 30-voltar 3
adb shell pidof $PACOTE > fotos/pid.txt || echo "fechou" > fotos/pid.txt
adb logcat -d > fotos/logcat.txt
grep -i "Rustinho:" fotos/logcat.txt >> fotos/diag.txt
grep -E "$(adb shell pidof $PACOTE | tr -d '\r')|AndroidRuntime|DEBUG" fotos/logcat.txt | grep -i -E "qml|warn|error|fatal|crash|qt" | tail -150 > fotos/erros.txt
ls -la fotos
exit 0
