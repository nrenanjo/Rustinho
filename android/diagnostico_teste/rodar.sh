#!/bin/bash
# Teste no computador do diagnóstico do Android (PEND-18). Uso: bash rodar.sh [pasta de trabalho]
# Compila ../diagnostico.cpp com um programa de teste e confere 3 cenários.
set -u
AQUI=$(cd "$(dirname "$0")" && pwd)
T=${1:-$(mktemp -d)}
mkdir -p "$T/b" && cd "$T/b" && cmake -S "$AQUI" -B . >/dev/null && make 2>&1 | grep -E 'error|Error' && { echo "FALHA: não compilou"; exit 1; }
falhas=0
conferir() { if ! grep -q -- "$2" "$T/$1/diagnostico.txt"; then echo "FALHA [$1]: faltou '$2'"; falhas=$((falhas+1)); else echo "ok [$1]: $2"; fi; }
nao_ter() { if grep -q -- "$2" "$T/$1/diagnostico.txt"; then echo "FALHA [$1]: não devia ter '$2'"; falhas=$((falhas+1)); else echo "ok [$1]: sem '$2'"; fi; }
cd "$AQUI"
for c in cinza conteudo quebrado; do
  rm -rf "$T/$c"; mkdir -p "$T/$c"
  RUSTINHO_DIAGNOSTICO_PASTA="$T/$c" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software "$T/b/teste" $c >/dev/null 2>&1
done
conferir cinza "uniforme (uma cor só): SIM"; conferir cinza "aviso na tela"; ls "$T"/cinza/captura-*.png >/dev/null 2>&1 || { echo "FALHA [cinza]: sem foto"; falhas=$((falhas+1)); }
conferir conteudo "uniforme (uma cor só): não"; conferir conteudo "tela pronta: TelaTeste"; nao_ter conteudo "aviso na tela"
conferir quebrado "Main.qml NÃO carregou"; conferir quebrado "coisaInexistente"
[ $falhas -eq 0 ] && echo "TUDO OK" || { echo "$falhas falha(s)"; exit 1; }
