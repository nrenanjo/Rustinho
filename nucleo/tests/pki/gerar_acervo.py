#!/usr/bin/env python3
"""Acervo FICTÍCIO de XMLs, como uma exportação do sistema antigo (SINTEC),
para o teste de importação (T7). SEM VALIDADE – SÓ TESTE.

Usa as funções de `gerar.py` com uma raiz e uma AC próprias
(`raiz_acervo_teste.der`, `ac_acervo_teste.der`), que entram nas âncoras de
teste do Rustinho (só nos testes). Nenhum dado é real: lojas, clientes,
fornecedores e peças são inventados; o CPF é o de exemplo dos manuais.

A conta à mão do que deve ser importado está em `nucleo/tests/estavel/t7_importacao.rs`.

Para refazer:  python3 nucleo/tests/pki/gerar_acervo.py
"""
import os
import sys
import zipfile

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import gerar as G  # noqa: E402

PASTA = os.path.join(AQUI, "xml", "acervo", "exportacao")
LOJA = "11444777000161"
NOME_LOJA = "LOJA MODELO AUTO PECAS"
CPF_JOAO = "52998224725"


def dv11(texto):
    soma = sum((ord(c) - 48) * (2 + i % 8) for i, c in enumerate(reversed(texto)))
    r = soma % 11
    return 0 if r < 2 else 11 - r


def cnpj(base12):
    d1 = dv11(base12)
    return f"{base12}{d1}{dv11(base12 + str(d1))}"


def ean13(base12):
    s = sum(int(c) * (3 if i % 2 else 1) for i, c in enumerate(base12))
    return base12 + str((10 - s % 10) % 10)


F1 = cnpj("334445550001")
F2 = cnpj("556667770001")
C1 = cnpj("778889990001")
G1 = ean13("789100000001")  # mesma peça na nota da loja e na do fornecedor F1
G3 = ean13("789100000003")
G5 = ean13("789100000005")


def chave(emit, mod, numero, codigo, aamm="2609"):
    sem = f"35{aamm}{emit}{mod}001{numero:09d}1{codigo:08d}"
    assert len(sem) == 43, sem
    return sem + str(dv11(sem))


def item(n, cprod, cean, xprod, ncm, ucom, qcom, vprod):
    return (f'<det nItem="{n}"><prod><cProd>{cprod}</cProd><cEAN>{cean or "SEM GTIN"}</cEAN><xProd>{xprod}</xProd>'
            f'<NCM>{ncm}</NCM><CFOP>5102</CFOP><uCom>{ucom}</uCom><qCom>{qcom:.4f}</qCom><vProd>{vprod:.2f}</vProd></prod>'
            f'<imposto><ICMS><ICMSSN102><orig>0</orig><CSOSN>102</CSOSN></ICMSSN102></ICMS></imposto></det>')


def nota(emit, nome_emit, dest, itens, numero, dia, fin="1", mod="55"):
    ch = chave(emit, mod, numero, numero * 7 + 3)
    if dest is None:
        d = ""
    else:
        tag, doc, nome = dest
        d = (f"<dest><{tag}>{doc}</{tag}><xNome>{nome}</xNome><enderDest><xLgr>RUA B</xLgr><nro>5</nro>"
             f"<xBairro>CENTRO</xBairro><xMun>APIAI</xMun><UF>SP</UF></enderDest></dest>")
    dets = "".join(item(i + 1, *it) for i, it in enumerate(itens))
    # As notas de fornecedor vêm como a SEFAZ devolve (nfeProc com o
    # protocolo), que é o que a tela de Compras exige.
    com_protocolo = emit != LOJA
    abre = '<nfeProc xmlns="http://www.portalfiscal.inf.br/nfe" versao="4.00">' if com_protocolo else ""
    texto = ('<?xml version="1.0" encoding="UTF-8"?>\n' + abre +
             f'<NFe xmlns="http://www.portalfiscal.inf.br/nfe"><infNFe Id="NFe{ch}" versao="4.00">'
             f"<ide><cUF>35</cUF><mod>{mod}</mod><serie>1</serie><nNF>{numero}</nNF>"
             f"<dhEmi>2026-09-{dia:02d}T10:00:00-03:00</dhEmi><tpNF>1</tpNF><tpAmb>1</tpAmb><finNFe>{fin}</finNFe>"
             "<natOp>VENDA</natOp></ide>"
             f"<emit><CNPJ>{emit}</CNPJ><xNome>{nome_emit}</xNome><enderEmit><UF>SP</UF></enderEmit></emit>"
             f"{d}{dets}</infNFe>@@ASSINATURA@@</NFe>")
    if com_protocolo:
        texto += (f'<protNFe versao="4.00"><infProt><chNFe>{ch}</chNFe><cStat>100</cStat>'
                  f"<dhRecbto>2026-09-{dia:02d}T10:01:00-03:00</dhRecbto><nProt>1352600000{numero:05d}</nProt>"
                  "<digVal>@@DIGVAL@@</digVal></infProt></protNFe></nfeProc>")
    return texto


def gravar(rel, texto):
    p = os.path.join(PASTA, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w", encoding="utf-8", newline="") as f:
        f.write(texto)
    return p


def main():
    raiz_k = G.chave(2048)
    nome_raiz = "Raiz do ACERVO de TESTE do Rustinho SEM VALIDADE"
    raiz = G.emitir_ac(nome_raiz, raiz_k, G.nome(nome_raiz), raiz_k)
    ac_k = G.chave(2048)
    ac = G.emitir_ac("AC do ACERVO de TESTE do Rustinho SEM VALIDADE", ac_k, raiz.subject, raiz_k)
    open(os.path.join(AQUI, "raiz_acervo_teste.der"), "wb").write(G.der(raiz))
    open(os.path.join(AQUI, "ac_acervo_teste.der"), "wb").write(G.der(ac))
    folhas = {}

    def assinar(emit, texto):
        if emit not in folhas:
            k = G.chave(2048)
            folhas[emit] = (k, G.emitir_folha("EMITENTE DO ACERVO DE TESTE LTDA:" + emit, k, ac, ac_k, cnpj=emit))
        k, c = folhas[emit]
        return G.assinar(texto, k, c)

    if os.path.isdir(PASTA):
        import shutil
        shutil.rmtree(PASTA)

    # Compras (notas de fornecedores para a loja).
    n501 = assinar(F1, nota(F1, "DISTRIBUIDORA UM DE TESTE LTDA", ("CNPJ", LOJA, NOME_LOJA), [
        ("F1-100", G1, "FILTRO DE OLEO TESTE", "84212300", "UN", 10, 150.00),
        ("F1-200", "", "CORREIA DENTADA TESTE", "40103100", "UN", 5, 100.00),
    ], 501, 1))
    n502 = assinar(F1, nota(F1, "DISTRIBUIDORA UM DE TESTE LTDA", ("CNPJ", LOJA, NOME_LOJA), [
        ("F1-100", G1, "FILTRO DE OLEO TESTE", "84212300", "UN", 10, 160.00),
        ("F1-300", G5, "VELA DE IGNICAO TESTE", "85111000", "JG", 2, 90.00),
    ], 502, 15))
    # D-13: o fornecedor usa o mesmo código em duas peças diferentes.
    n503 = assinar(F1, nota(F1, "DISTRIBUIDORA UM DE TESTE LTDA", ("CNPJ", LOJA, NOME_LOJA), [
        ("F1-200", "", "CORREIA DENTADA TESTE", "40103100", "UN", 5, 100.00),
        ("F1-200", "", "CORREIA DENTADA TESTE REFORCADA", "40103100", "UN", 2, 70.00),
    ], 503, 18))
    n900 = assinar(F2, nota(F2, "AUTO DISTRIBUIDORA DOIS DE TESTE", ("CNPJ", LOJA, NOME_LOJA), [
        ("B-1", "", "LAMPADA H4 TESTE", "85392190", "UN", 20, 120.00),
        ("B-2", "", "FUSIVEL 10A TESTE", "85361000", "PC", 50, 25.00),
    ], 900, 10))
    n901 = assinar(F2, nota(F2, "AUTO DISTRIBUIDORA DOIS DE TESTE", ("CNPJ", LOJA, NOME_LOJA), [
        ("B-9", "", "PECA DEVOLVIDA TESTE", "85361000", "UN", 1, 10.00),
    ], 901, 11, fin="4"))
    gravar("compras/nf501.xml", n501)
    gravar("compras/nf502.xml", n502)
    gravar("compras/nf503.xml", n503)
    gravar("compras/nf900.xml", n900)
    gravar("compras/nf900-copia.xml", n900)  # a mesma nota em dois arquivos
    gravar("compras/nf901-devolucao.xml", n901)

    # Vendas da loja.
    n10 = assinar(LOJA, nota(LOJA, NOME_LOJA, ("CNPJ", C1, "OFICINA DO ZE DE TESTE LTDA"), [
        ("1001", G1, "FILTRO DE OLEO TESTE", "84212300", "UN", 2, 60.00),
        ("1002", "", "PASTILHA DE FREIO TESTE", "68138110", "JG", 1, 50.00),
    ], 10, 5))
    n11 = assinar(LOJA, nota(LOJA, NOME_LOJA, ("CPF", CPF_JOAO, "JOAO DA SILVA TESTE"), [
        ("1001", G1, "FILTRO DE OLEO TESTE", "84212300", "UN", 1, 35.00),
        ("1003", G3, "AMORTECEDOR TESTE", "87088000", "UN", 1, 80.00),
    ], 11, 20))
    n12 = assinar(LOJA, nota(LOJA, NOME_LOJA, None, [
        ("1004", "", "PALHETA TESTE", "85129000", "UN", 4, 50.00),
    ], 12, 21, mod="65"))
    n13 = assinar(LOJA, nota(LOJA, NOME_LOJA, ("CNPJ", C1, "OFICINA DO ZE DE TESTE LTDA"), [
        ("1002", "", "PASTILHA DE FREIO TESTE", "68138110", "JG", 1, 50.00),
    ], 13, 25))
    n14 = assinar(LOJA, nota(LOJA, NOME_LOJA, ("CNPJ", F1, "DISTRIBUIDORA UM DE TESTE LTDA"), [
        ("F1-100", G1, "FILTRO DE OLEO TESTE", "84212300", "UN", 1, 15.00),
    ], 14, 26, fin="4"))
    # D-13: o mesmo código em duas peças diferentes (como acontece no sistema
    # antigo) e o mesmo código com a descrição só escrita de outro jeito.
    n15 = assinar(LOJA, nota(LOJA, NOME_LOJA, ("CNPJ", C1, "OFICINA DO ZE DE TESTE LTDA"), [
        ("1005", "", "CORREIA EM V TESTE 13X950 -", "40103200", "UN", 1, 70.00),
        ("1005", "", "CORREIA EM V TESTE 13X980 -", "40103200", "UN", 1, 65.00),
        ("1002", "", "PASTILHA DE FREIO  TESTE -", "68138110", "JG", 1, 50.00),
    ], 15, 27))
    gravar("vendas/nf15.xml", n15)
    z = os.path.join(PASTA, "vendas", "2026-09.zip")
    os.makedirs(os.path.dirname(z), exist_ok=True)
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as arq:
        for nome_arq, t in [("nf10.xml", n10), ("nf11.xml", n11), ("nfce12.xml", n12)]:
            info = zipfile.ZipInfo(nome_arq, date_time=(2026, 9, 30, 12, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            arq.writestr(info, t.encode("utf-8"))
    gravar("vendas/nf13.xml", n13)
    gravar("vendas/nf14-devolucao.xml", n14)

    # O que não é nota (ou foi mexido depois de assinado).
    gravar("outros/nf10-adulterada.xml", n10.replace("PASTILHA DE FREIO TESTE", "PASTILHA ALTERADA"))
    gravar("outros/evento-cancelamento.xml",
           '<?xml version="1.0" encoding="UTF-8"?><procEventoNFe xmlns="http://www.portalfiscal.inf.br/nfe" versao="1.00">'
           '<evento><infEvento Id="ID1101113526091144477700016155001000000010100000073"><tpEvento>110111</tpEvento>'
           '</infEvento></evento></procEventoNFe>')
    gravar("outros/lixo.xml", "isto nao e um xml de nota")
    print("acervo de teste gravado em", PASTA)


if __name__ == "__main__":
    main()
