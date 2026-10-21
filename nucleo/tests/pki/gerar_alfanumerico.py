#!/usr/bin/env python3
"""Notas de TESTE com CNPJ alfanumérico (PEND-19). SEM VALIDADE – SÓ TESTE.

Usa as funções de `gerar.py`, mas com uma raiz e uma AC próprias (os
arquivos `raiz_alfanumerico_teste.der` e `ac_alfanumerico_teste.der`), para não
precisar refazer as outras notas de teste. As duas entram nas âncoras de
teste do Rustinho (só nos testes).

Regra (NT Conjunta 2025.001 e IN RFB 2.229/2024): as 12 primeiras posições do
CNPJ podem ter letras maiúsculas; os 2 dígitos finais são números; na conta do
dígito, cada caractere vale o código ASCII menos 48. A chave de acesso (44
posições) leva o CNPJ do emitente nas posições 7 a 20 e o dígito final segue a
mesma conta (módulo 11, pesos 2 a 9).

Para refazer:  python3 nucleo/tests/pki/gerar_alfanumerico.py
"""
import os
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import gerar as G  # noqa: E402

CNPJ_ALFA = "12ABC34501DE35"  # exemplo publicado pela Receita Federal


def valor(c):
    assert c.isdigit() or ("A" <= c <= "Z"), c
    return ord(c) - 48


def dv_mod11(texto):
    soma = sum(valor(c) * (2 + i % 8) for i, c in enumerate(reversed(texto)))
    r = soma % 11
    return 0 if r < 2 else 11 - r


def cnpj_com_dv(base12):
    d1 = dv_mod11(base12)
    d2 = dv_mod11(base12 + str(d1))
    return f"{base12}{d1}{d2}"


def chave(cnpj, numero="000012345", codigo="00000001"):
    sem_dv = f"352610{cnpj}55001{numero}1{codigo}"
    assert len(sem_dv) == 43, sem_dv
    return sem_dv + str(dv_mod11(sem_dv))


def modelo_alfa(cnpj):
    ch = chave(cnpj)
    m = G.MODELO
    antiga = "35261011222333000181550010000123451000000014"
    assert m.count(antiga) == 2, "o modelo mudou"
    m = m.replace(antiga, ch)
    m = m.replace("<cDV>4</cDV>", f"<cDV>{ch[-1]}</cDV>", 1)
    m = m.replace("<CNPJ>11222333000181</CNPJ>", f"<CNPJ>{cnpj}</CNPJ>", 1)
    return m, ch


def main():
    assert cnpj_com_dv("12ABC34501DE") == CNPJ_ALFA, cnpj_com_dv("12ABC34501DE")
    filial = cnpj_com_dv("12ABC3450002")  # mesma raiz (12ABC345), outro estabelecimento
    outra = cnpj_com_dv("ZZ9XY8770001")  # outra raiz

    raiz_k = G.chave(2048)
    nome_raiz = "Raiz ALFANUMERICA de TESTE do Rustinho SEM VALIDADE"
    raiz = G.emitir_ac(nome_raiz, raiz_k, G.nome(nome_raiz), raiz_k)
    ac_k = G.chave(2048)
    ac = G.emitir_ac("AC ALFANUMERICA de TESTE do Rustinho SEM VALIDADE", ac_k, raiz.subject, raiz_k)
    open(os.path.join(AQUI, "raiz_alfanumerico_teste.der"), "wb").write(G.der(raiz))
    open(os.path.join(AQUI, "ac_alfanumerico_teste.der"), "wb").write(G.der(ac))

    def folha(cnpj):
        k = G.chave(2048)
        return k, G.emitir_folha("EMITENTE ALFANUMERICO DE TESTE LTDA:" + cnpj, k, ac, ac_k, cnpj=cnpj)

    modelo, ch = modelo_alfa(CNPJ_ALFA)
    k, c = folha(CNPJ_ALFA)
    G.gravar("alfa_ok.xml", G.assinar(modelo, k, c))
    k, c = folha(filial)
    G.gravar("alfa_cert_filial.xml", G.assinar(modelo, k, c))
    k, c = folha(outra)
    G.gravar("alfa_cert_outra_raiz.xml", G.assinar(modelo, k, c))
    # Mesma nota, mas a chave com letra minúscula (fora do padrão): o Id e o
    # chNFe ficam iguais entre si; a leitura precisa recusar a coerência.
    minuscula = modelo.replace(ch, ch[:8] + ch[8].lower() + ch[9:])
    k, c = folha(CNPJ_ALFA)
    G.gravar("alfa_chave_minuscula.xml", G.assinar(minuscula, k, c))
    print("chave:", ch, "filial:", filial, "outra raiz:", outra)


if __name__ == "__main__":
    sys.exit(main())
