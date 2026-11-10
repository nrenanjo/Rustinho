#!/bin/bash
# Percorre as telas da revisão visual (L5) — dev.3.3, passo 1.7. Veja LEIA-ME.md.
#
#   percorrer.sh <tema: cinza|amoled|roxo> <pasta> [tudo|denso]
#
# Clica nos textos que o próprio Rustinho informa (modo de captura,
# --textos-da-tela), sem coordenadas fixas: serve para 1920 × 1080, 1366 × 768
# e o modo celular (360 × 780; ALTURA=780: a tela rola até o texto aparecer).
# Precisa dos dados de demonstração do LEIA-ME (criados pela interface).
# Um texto que não aparece é AVISADO (falta de dado ou defeito) e a passada
# segue; o fim lista os avisos.
set -u
DIR=$(cd "$(dirname "$0")" && pwd)
R="$DIR/captura.sh"
tema=$1
P=$2
modo=${3:-tudo}
ALTURA=${ALTURA:-1080}
mkdir -p "$P"
avisos=()

dump() { "$R" textos | grep -v "^0	0"; }
clicartexto() {
    local xy x y
    for _ in 1 2 3 4 5 6 7 8; do
        xy=$(dump | awk -F'\t' -v t="$1" '$3 == t { print $1, $2; exit }')
        [ -n "$xy" ] || { echo "AVISO: sem '$1' na tela" >&2; avisos+=("sem '$1'"); return 1; }
        x=${xy% *}
        y=${xy#* }
        if [ "$y" -gt $((ALTURA - 50)) ]; then "$R" mouse 180 $((ALTURA / 2)); "$R" rolar baixo
        elif [ "$y" -lt 50 ]; then "$R" mouse 180 $((ALTURA / 2)); "$R" rolar cima
        else break; fi
    done
    "$R" clique "$x" "$y"
    sleep 1.3
}
inicio() {
    for _ in 1 2 3 4; do
        dump | grep -q "Bom dia\|Boa tarde\|Boa noite" && return 0
        "$R" tecla Escape
        sleep 1.2
    done
    echo "AVISO: não voltei à Início" >&2
    avisos+=("não voltou à Início")
}
cap() { "$R" captura "$P" "$1" "${2:-1.5}" >/dev/null && echo "ok $1"; }

inicio
case $tema in
    cinza) clicartexto "Tema claro" ;;
    amoled) clicartexto "Tema escuro" ;;
    roxo) clicartexto "Planeta Roxo" ;;
    *) echo "tema: cinza, amoled ou roxo" >&2; exit 2 ;;
esac
cap 01_inicio
clicartexto "Peças"; cap 02_pecas_lista
clicartexto "Amortecedor dianteiro Onix"; clicartexto "Mostrar valores"; cap 03_peca_ficha; "$R" tecla Escape; sleep 1.2
clicartexto "Importar XML"; cap 19_importar_xml; "$R" tecla Escape; sleep 1.2
clicartexto "Etiquetas"; cap 18_etiquetas; inicio
clicartexto "Vender / Receber"; cap 04_vender_vazio
"$R" texto "pastilha"; sleep 1.5; "$R" tecla Return; sleep 1.5; "$R" tecla Return; sleep 1.5
cap 04c_vender_carrinho
"$R" tecla F2; cap 04d_vender_pagamento
clicartexto "Crediário"; cap 04e_vender_pagamento_crediario
"$R" tecla Escape; sleep 1
# Tira a peça do carrinho (lixeira à direita da linha; o Esc não sai com o carrinho cheio).
geo=$(dump | awk -F'\t' '$3 ~ /^GEO:campoDeBusca:/ { sub("GEO:campoDeBusca:", "", $3); print $3; exit }')
if [ -n "$geo" ]; then
    IFS=, read -r gx _ gw _ <<< "$geo"
    yl=$(dump | awk -F'\t' '$3 == "Pastilha de freio dianteira Gol" { print $2; exit }')
    [ -n "$yl" ] && "$R" clique $((gx + gw - 30)) $((yl + 11))
    sleep 1
fi
clicartexto "Receber"; clicartexto "Oficina do Joao"; cap 05_receber_cliente
clicartexto "Receber tudo"; cap 05b_receber_dialogo; "$R" tecla Escape; sleep 1
inicio
if [ "$modo" = tudo ]; then
    clicartexto "Orçamentos"; cap 06_orcamentos; inicio
    clicartexto "Trocas e devoluções"; cap 08_trocas; inicio
    clicartexto "Clientes"; cap 09_clientes; clicartexto "Oficina do Joao"; cap 10_cliente_ficha; inicio
    clicartexto "Compras"; cap 11_compras; inicio
    clicartexto "Estoque / Fornecedores"; cap 12a_estoque_comprar; clicartexto "Mais vendidos"; cap 12b_estoque_mais_vendidos; inicio
    clicartexto "Ordens de serviço"; cap 14a_ordens; inicio
    clicartexto "Usuários"; cap 16_usuarios; inicio
fi
clicartexto "Caixa"; cap 07_caixa; inicio
clicartexto "Financeiro"; cap 13a_fin_pagar; clicartexto "Fluxo de caixa"; cap 13d_fin_fluxo; inicio
clicartexto "Ordens de serviço"; "$R" tecla F2; cap 14b_ordem_nova; inicio
clicartexto "Painel"; cap 15a_painel_resumo; clicartexto "Resultado (DRE)"; cap 15b_painel_dre 2
clicartexto "Peças e vendedores"; cap 15c_painel_pecas_vendedores 2; inicio
clicartexto "Configurações"; cap 17a_config_loja
clicartexto "Vendas"; cap 17b_config_vendas
clicartexto "Simultaneidade"; cap 17e_config_simult 2
clicartexto "Backup"; cap 17h_config_backup
clicartexto "Fiscal"; cap 17i_config_fiscal
clicartexto "Gerente"; cap 17j_config_gerente; inicio
echo "fim: $tema $modo; avisos: ${#avisos[@]}"
for a in "${avisos[@]}"; do echo "  - $a"; done
