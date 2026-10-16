#!/usr/bin/env bash
# Compila o Rustinho e deixa o executável nesta pasta, ao lado do código.
# Uso: ./compilar.sh
set -euo pipefail
cd "$(dirname "$0")"

falta=()
command -v cargo >/dev/null 2>&1 || falta+=("rust")
command -v c++ >/dev/null 2>&1 || falta+=("base-devel")
if command -v qmake6 >/dev/null 2>&1; then
    [ -d "$(qmake6 -query QT_INSTALL_QML)/QtQuick/Controls" ] || falta+=("qt6-declarative")
else
    falta+=("qt6-base" "qt6-declarative")
fi

if [ ${#falta[@]} -gt 0 ]; then
    echo "Faltam pacotes para compilar. Instale com:"
    echo
    echo "    sudo pacman -S --needed ${falta[*]}"
    echo
    echo "Depois rode ./compilar.sh de novo."
    exit 1
fi

echo "Compilando o Rustinho. A primeira vez demora alguns minutos e precisa de internet."
QMAKE=qmake6 cargo build --release
cp target/release/rustinho ./rustinho

echo
echo "Pronto. Para abrir:"
echo
echo "    ./rustinho"

if ! command -v rclone >/dev/null 2>&1; then
    echo
    echo "Para o backup no Google Drive (opcional), instale também:"
    echo
    echo "    sudo pacman -S rclone"
fi
