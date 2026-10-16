#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, configura a loja,
# abre algumas telas e tira fotos. Tudo de mentira (Loja Modelo, senha 1234).
set -x
mkdir -p fotos
APK=$(ls saida/*.apk | head -1)
adb install -r "$APK" 2>&1 | tee fotos/instalar.txt
PACOTE=io.github.nrenanjo.rustinho
ATIVIDADE=$PACOTE/org.qtproject.qt.android.bindings.QtActivity
PASTA=/storage/emulated/0/Android/data/$PACOTE/files
adb shell mkdir -p $PASTA
adb shell pm grant $PACOTE android.permission.CAMERA || true
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 0
adb logcat -c
foto() { sleep "${2:-4}"; adb exec-out screencap -p > "fotos/$1.png"; }
tecla() { adb shell input keyevent "$1"; sleep 1; }
texto() { adb shell input text "$1"; sleep 1; }

adb shell am start -n $ATIVIDADE
foto 01-primeira-vez 40
texto "LojaModelo"; tecla KEYCODE_ENTER
tecla KEYCODE_ENTER
texto "Maria"; tecla KEYCODE_ENTER
texto "1234"; tecla KEYCODE_ENTER
texto "1234"; tecla KEYCODE_ENTER
foto 02-inicio 6
adb shell input swipe 500 1800 500 600 400
foto 03-inicio-rolado 3

abrir() {
    adb shell am force-stop $PACOTE
    sleep 2
    adb shell "echo '--abrir $1' > $PASTA/argumentos.txt"
    adb shell am start -n $ATIVIDADE
    sleep 15
    texto "1234"; tecla KEYCODE_ENTER
}
for t in TelaVender TelaPecas TelaCaixa TelaCompras TelaConfiguracoes TelaPainel TelaFinanceiro; do
    abrir $t
    foto "10-$t" 5
done
# Deitado.
abrir TelaVender
adb shell settings put system user_rotation 1
foto 20-deitado-vender 8
abrir TelaPecas
foto 21-deitado-pecas 6
adb shell settings put system user_rotation 0
sleep 4
# Voltar do Android: volta ao Início.
tecla KEYCODE_BACK
foto 30-voltar 3
adb shell pidof $PACOTE > fotos/pid.txt || echo "fechou" > fotos/pid.txt
adb logcat -d > fotos/logcat.txt
grep -E "$(adb shell pidof $PACOTE | tr -d '\r')|AndroidRuntime|DEBUG" fotos/logcat.txt | grep -i -E "qml|warn|error|fatal|crash|qt" | tail -150 > fotos/erros.txt
ls -la fotos
exit 0
