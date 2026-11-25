#!/usr/bin/env python3
"""OLHO-1 (decisão do proprietário, 25/11/2026): o Olho de Hórus como privacidade
comercial na tela, durante o atendimento.

Com o olho fechado:
- na ficha da peça, o custo e o % de lucro somem **com os títulos** ("Custo", "% sobre o
  custo"); o preço de venda continua visível e **sobe** para o lugar deles;
- no Caixa, o saldo ("Dinheiro na gaveta") fica discreto (sem o valor em reais);
- R21: o diálogo "Fechar o caixa" mostra o esperado na gaveta mesmo com o olho fechado.

Com o olho aberto, tudo volta. O olho só muda o que aparece: nada é gravado.

Usa a loja de teste e o jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_olho1.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import re

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import TELA, Rustinho, conferir, montar_loja, simples, xdo  # noqa: E402

DINHEIRO = re.compile(r"R\$ ?-?[\d.]+,\d\d")


def y_de(textos, texto):
    for _, y, t in textos:
        if simples(t) == simples(texto):
            return y
    return None


def preco_e_custo(pasta, peca):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    v = c.execute(
        "SELECT p.custo, pp.preco FROM produto p JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1 "
        "WHERE p.id = ?",
        (peca,),
    ).fetchone()
    c.close()
    return v


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-olho1-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-olho1-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        filtro, _compra, _oleo = montar_loja(pasta)
        antes = preco_e_custo(pasta, filtro)

        # ---- 1. A ficha da peça ----
        r = Rustinho(pasta, "TelaPeca", {"produtoId": filtro}, fotos, "olho1-peca")
        try:
            r.entrar("Ana")
            time.sleep(2)
            # Ao entrar, o olho começa fechado (como sempre foi).
            fechado = r.textos()
            tudo_certo &= conferir(
                "peça, olho fechado: sem custo, sem % de lucro e sem os títulos; o preço fica",
                r.foto("-1-fechado"),
                fechado,
                tem=["Preço de revenda", "Mostrar valores"],
                sem_igual=["Custo", "% sobre o custo"],
                sem_percentual=True,
            )
            y_fechado = y_de(fechado, "Preço de revenda")
            if not r.clicar("Mostrar valores"):
                raise RuntimeError(f"não achei o botão Mostrar valores (veja {r.saida})")
            time.sleep(1)
            aberto = r.textos()
            tudo_certo &= conferir(
                "peça, olho aberto: o custo e o % de lucro voltam, com os títulos",
                r.foto("-2-aberto"),
                aberto,
                tem=["Preço de revenda", "Esconder"],
                igual=["Custo", "% sobre o custo"],
            )
            y_aberto = y_de(aberto, "Preço de revenda")
            sobe = y_fechado is not None and y_aberto is not None and y_fechado < y_aberto
            print(("✔ " if sobe else "✘ ") + "peça: com o olho fechado, o preço sobe para o lugar do custo"
                  + ("" if sobe else f" (y fechado {y_fechado}, y aberto {y_aberto})"))
            tudo_certo &= sobe
            if not r.clicar("Esconder"):
                raise RuntimeError(f"não achei o botão Esconder (veja {r.saida})")
            time.sleep(1)
            tudo_certo &= conferir(
                "peça, olho fechado de novo: o custo e o % somem outra vez",
                r.foto("-3-fechado-de-novo"),
                r.textos(),
                tem=["Preço de revenda"],
                sem_igual=["Custo", "% sobre o custo"],
                sem_percentual=True,
            )
        finally:
            r.fechar()
        depois = preco_e_custo(pasta, filtro)
        igual = antes == depois
        print(("✔ " if igual else "✘ ") + "peça: o olho não muda nada gravado (custo e preço)"
              + ("" if igual else f" (antes {antes}, depois {depois})"))
        tudo_certo &= igual

        # ---- 2. O Caixa (a loja de teste já tem o caixa aberto, com vendas) ----
        r = Rustinho(pasta, "TelaCaixa", {}, fotos, "olho1-caixa")
        try:
            r.entrar("Ana")
            time.sleep(2)
            fechado = r.textos()
            com_valor = [t for _, _, t in fechado if DINHEIRO.search(t)]
            ok = any(simples(t) == simples("Dinheiro na gaveta") for _, _, t in fechado) and not com_valor
            foto = r.foto("-1-fechado")
            print(("✔ " if ok else "✘ ") + "caixa, olho fechado: o saldo e os totais ficam discretos"
                  + ("" if ok else f": apareceu {com_valor[:4]} (foto: {foto})"))
            tudo_certo &= ok
            # R21 (DP-20): conferência tradicional. Com o olho fechado, o diálogo "Fechar o caixa" mostra o
            # esperado na gaveta antes da contagem (o olho não interfere no fechamento). Esc fecha sem fechar o caixa.
            if not r.clicar("Fechar caixa"):
                raise RuntimeError(f"não achei o botão Fechar caixa (veja {r.saida})")
            time.sleep(1.5)
            esperado = [t for _, _, t in r.textos() if t.startswith("Esperado na gaveta: R$ ") and DINHEIRO.search(t)]
            foto = r.foto("-1b-fechar")
            print(("✔ " if esperado else "✘ ") + "caixa, olho fechado: o diálogo de fechar mostra o esperado na gaveta"
                  + (f" ({esperado[0]})" if esperado else f" (foto: {foto})"))
            tudo_certo &= bool(esperado)
            xdo("key", "Escape")
            time.sleep(1)
            if not r.clicar("Mostrar valores"):
                raise RuntimeError(f"não achei o botão Mostrar valores no Caixa (veja {r.saida})")
            time.sleep(1)
            aberto = r.textos()
            com_valor = [t for _, _, t in aberto if DINHEIRO.search(t)]
            ok = len(com_valor) >= 3
            foto = r.foto("-2-aberto")
            print(("✔ " if ok else "✘ ") + "caixa, olho aberto: o saldo e os totais aparecem"
                  + ("" if ok else f": só {com_valor} (foto: {foto})"))
            tudo_certo &= ok
        finally:
            r.fechar()

        # ---- 3. Outras telas com custo ou margem: somem com o título ----
        casos = [
            ("compra", "TelaCompra", {"compraId": _compra}, "Custo das peças"),
            ("estoque", "TelaEstoque", {"secao": "comprar"}, "Custo estimado"),
            ("configuracoes", "TelaConfiguracoes", {"secao": "vendas"}, "Lucro sugerido"),
        ]
        for nome, tela, props, titulo in casos:
            r = Rustinho(pasta, tela, props, fotos, f"olho1-{nome}")
            try:
                r.entrar("Ana")
                time.sleep(2)
                tudo_certo &= conferir(
                    f"{nome}, olho fechado: \"{titulo}\" some", r.foto("-1-fechado"), r.textos(),
                    tem=["Mostrar valores"], sem_igual=[titulo],
                )
                if not r.clicar("Mostrar valores"):
                    raise RuntimeError(f"não achei o botão Mostrar valores em {tela} (veja {r.saida})")
                time.sleep(1)
                tudo_certo &= conferir(
                    f"{nome}, olho aberto: \"{titulo}\" volta", r.foto("-2-aberto"), r.textos(), igual=[titulo],
                )
            finally:
                r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
