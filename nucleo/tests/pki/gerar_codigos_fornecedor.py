#!/usr/bin/env python3
"""Notas FICTÍCIAS dos cenários 1, 5, 6 e 9 do N02 (código do fornecedor na
importação de XML). SEM VALIDADE – SÓ TESTE.

Dev.3.3, passo 3.0. As raízes de teste que já existem não guardaram as chaves,
então não dá para assinar notas novas com elas. Estas notas usam uma raiz e uma
AC próprias (`raiz_codigos_teste.der`, `ac_codigos_teste.der`), que entram nas
âncoras de teste do Rustinho (só nos testes; o programa montado não as tem).
As chaves são geradas na hora e descartadas, como em `gerar_acervo.py`, de onde
vêm a loja, o modelo da nota e as contas de CNPJ e GTIN.

Nenhum dado é real: o fornecedor "PATRAL" tem CNPJ inventado. O GTIN
7908168151578 e o código 00008373 são os números do caso relatado pelo
proprietário, sem nenhum outro dado da nota verdadeira.

  nf601: PATRAL, cProd 00008373, cEAN e cEANTrib 7908168151578 (cenário 1)
  nf602: PATRAL, cProd 8373 sem GTIN, mesma descrição (cenário 5)
  nf603: PATRAL, cProd igual ao GTIN de outra peça, sem GTIN (cenário 6)
  nf604: PATRAL, cEAN comercial diferente do cEANTrib tributável (cenário 9)

Os testes estão em `nucleo/tests/caracterizacao_dev33.rs` (`n02_c*`); o
requisito, em `_reversa_sdd/auditoria_dev33/REQUISITO_CODIGO_FORNECEDOR_XML.md` §4.

Para refazer:  python3 nucleo/tests/pki/gerar_codigos_fornecedor.py
"""
import os
import shutil
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import gerar as G  # noqa: E402
import gerar_acervo as A  # noqa: E402

PASTA = os.path.join(AQUI, "xml", "codigos")
PATRAL = A.cnpj("446667770001")
NOME_PATRAL = "PATRAL PECAS DE TESTE LTDA"
GTIN_PATRAL = "7908168151578"
GTIN_OUTRA = A.ean13("789100000009")
GTIN_COMERCIAL = A.ean13("789100000011")
GTIN_TRIBUTAVEL = A.ean13("789100000012")


def com_cean_trib(texto, vprod, gtin):
    """Põe o cEANTrib logo depois do vProd do item, na ordem do leiaute."""
    marca = f"<vProd>{vprod:.2f}</vProd>"
    assert texto.count(marca) == 1, marca
    return texto.replace(marca, marca + f"<cEANTrib>{gtin}</cEANTrib>")


def main():
    raiz_k = G.chave(2048)
    nome_raiz = "Raiz dos CODIGOS de TESTE do Rustinho SEM VALIDADE"
    raiz = G.emitir_ac(nome_raiz, raiz_k, G.nome(nome_raiz), raiz_k)
    ac_k = G.chave(2048)
    ac = G.emitir_ac("AC dos CODIGOS de TESTE do Rustinho SEM VALIDADE", ac_k, raiz.subject, raiz_k)
    open(os.path.join(AQUI, "raiz_codigos_teste.der"), "wb").write(G.der(raiz))
    open(os.path.join(AQUI, "ac_codigos_teste.der"), "wb").write(G.der(ac))
    folha_k = G.chave(2048)
    folha = G.emitir_folha(NOME_PATRAL + ":" + PATRAL, folha_k, ac, ac_k, cnpj=PATRAL)

    loja = ("CNPJ", A.LOJA, A.NOME_LOJA)
    notas = {
        "nf601_patral_00008373.xml": com_cean_trib(A.nota(PATRAL, NOME_PATRAL, loja, [
            ("00008373", GTIN_PATRAL, "PASTILHA DE FREIO DIANTEIRA TESTE", "68138110", "JG", 2, 180.00),
        ], 601, 2), 180.00, GTIN_PATRAL),
        "nf602_patral_8373.xml": A.nota(PATRAL, NOME_PATRAL, loja, [
            ("8373", "", "PASTILHA DE FREIO DIANTEIRA TESTE", "68138110", "JG", 1, 90.00),
        ], 602, 16),
        "nf603_patral_cprod_igual_gtin.xml": A.nota(PATRAL, NOME_PATRAL, loja, [
            (GTIN_OUTRA, "", "FILTRO DE AR TESTE", "84213100", "UN", 3, 60.00),
        ], 603, 20),
        "nf604_patral_gtin_tributavel.xml": com_cean_trib(A.nota(PATRAL, NOME_PATRAL, loja, [
            ("PT-9", GTIN_COMERCIAL, "LAMPADA H7 TESTE CAIXA COM 10", "85392190", "CX", 1, 50.00),
        ], 604, 22), 50.00, GTIN_TRIBUTAVEL),
    }
    if os.path.isdir(PASTA):
        shutil.rmtree(PASTA)
    os.makedirs(PASTA)
    for nome_arq, texto in notas.items():
        with open(os.path.join(PASTA, nome_arq), "w", encoding="utf-8", newline="") as f:
            f.write(G.assinar(texto, folha_k, folha))
    print("PATRAL (CNPJ de teste):", PATRAL)
    print("GTIN da outra peça:", GTIN_OUTRA, "| comercial:", GTIN_COMERCIAL, "| tributável:", GTIN_TRIBUTAVEL)
    print("notas de teste gravadas em", PASTA)


if __name__ == "__main__":
    main()
