#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, abre e tira fotos.
set -x
mkdir -p fotos
APK=$(ls saida/*.apk | head -1)
adb install -r "$APK" 2>&1 | tee fotos/instalar.txt
PACOTE=io.github.nrenanjo.rustinho
adb shell pm grant $PACOTE android.permission.CAMERA || true
adb logcat -c
adb shell monkey -p $PACOTE -c android.intent.category.LAUNCHER 1
sleep 40
adb exec-out screencap -p > fotos/1-abriu.png
# Primeira vez: nome da loja, usuário e senha.
adb shell input text "Loja%sModelo"
sleep 2
adb exec-out screencap -p > fotos/2-digitou.png
# Deitado.
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 1
sleep 8
adb exec-out screencap -p > fotos/3-deitado.png
adb shell settings put system user_rotation 0
sleep 5
adb shell input keyevent KEYCODE_BACK
sleep 3
adb exec-out screencap -p > fotos/4-voltar.png
adb shell pidof $PACOTE > fotos/pid.txt || echo "fechou" > fotos/pid.txt
adb logcat -d > fotos/logcat.txt
grep -i -E "qml|rustinho|libRustinho|fatal|crash|AndroidRuntime|qt\.|warning|error" fotos/logcat.txt | grep -v -i "chatty" | tail -120 > fotos/erros.txt
ls -la fotos
exit 0
