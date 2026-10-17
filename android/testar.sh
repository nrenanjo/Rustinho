#!/bin/bash
# Roda dentro do emulador (montagem automática): instala, configura a loja,
# abre algumas telas e tira fotos. Tudo de mentira (Loja Modelo, senha 1234).
set -x
mkdir -p fotos
# Manda as fotos para o ramo "testes-android" (também no meio, caso o
# emulador caia antes do fim).
guardar() {
    [ -n "$TOKEN_FOTOS" ] || return 0
    rm -rf /tmp/r && mkdir -p /tmp/r && cp -r fotos /tmp/r/
    (cd /tmp/r && git init -q -b "${RAMO_FOTOS:-testes-android}" && git config user.name montagem &&
     git config user.email montagem@users.noreply.github.com && git add -A &&
     git commit -qm "Fotos do teste ${GITHUB_SHA::7} ($1)" &&
     git push -q -f "https://x-access-token:$TOKEN_FOTOS@github.com/$GITHUB_REPOSITORY.git" "${RAMO_FOTOS:-testes-android}") || true
}
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
# Modo escuro: a lua, ao lado de "Trocar usuário". No teste de 32 bits fica
# escuro até o fim; no de 64, volta para o sol.
adb shell input tap 187 472
foto 02b-escuro 3
[ -n "$CURTO" ] || { adb shell input tap 91 472; sleep 2; }
adb shell input swipe 500 1800 500 600 400
foto 03-inicio-rolado 3

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
[ -n "$CURTO" ] && TELAS="TelaVender TelaConfiguracoes TelaPeca TelaFinanceiro"
for t in $TELAS; do
    abrir $t
    foto "10-$t" 5
    adb shell input swipe 540 1700 540 700 500
    foto "11-$t-rolada" 3
    adb logcat -d > fotos/logcat.txt
    guardar "$t"
done
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
guardar antes-da-camera
# A câmera por último (no emulador, ela mostra uma sala de mentira):
# um toque no botão da câmera, no fim do campo de busca da venda.
abrir TelaVender
foto 14-vender-antes-da-camera 12
adb shell input tap 985 414
foto 15-camera 12
adb logcat -d | grep -i -E "camera|multimedia" | tail -40 > fotos/camera.txt
adb shell pidof $PACOTE > fotos/pid.txt || echo "fechou" > fotos/pid.txt
adb logcat -d > fotos/logcat.txt
grep -i "Rustinho:" fotos/logcat.txt >> fotos/diag.txt
grep -E "$(adb shell pidof $PACOTE | tr -d '\r')|AndroidRuntime|DEBUG" fotos/logcat.txt | grep -i -E "qml|warn|error|fatal|crash|qt" | tail -150 > fotos/erros.txt
ls -la fotos
guardar fim
exit 0
