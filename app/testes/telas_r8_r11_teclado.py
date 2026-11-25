#!/usr/bin/env python3
"""R8 / D-AUD-8 e R11 / D-AUD-9 (decisões do proprietário, 25/11/2026): o teclado no balcão.

A. Fechar o caixa:
   - o Enter no "Dinheiro contado" não fecha o caixa;
   - "Fechar e imprimir" pede confirmação, com o contado, o esperado e a diferença;
   - o Enter não confirma a pergunta.
   O caixa da loja de teste continua aberto: o roteiro desiste com o Esc. Fechar de verdade imprime, e a conta do
   fechamento é conferida pelos testes do núcleo.
B. Pergunta de perigo ("Descartar alterações?" na ficha da peça): o Enter não confirma; a pergunta continua aberta.
C. Botões das linhas pelo teclado (Trocas › feitas): sem o mouse em cima, o Tab chega à linha e os botões dela
   ("Desfazer") aparecem.

Usa a loja de teste e o jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_r8_r11_teclado.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import TELA, Rustinho, conferir, montar_loja, simples, xdo  # noqa: E402


def caixas_abertos(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    n = c.execute("SELECT COUNT(*) FROM caixa_sessao WHERE fechado_em IS NULL").fetchone()[0]
    c.close()
    return n


def tem(textos, pedaco):
    return any(simples(pedaco) in simples(t) for _, _, t in textos)


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r8-r11-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r8-r11-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        filtro, _compra, _oleo = montar_loja(pasta)

        # ---- A. Fechar o caixa ----
        r = Rustinho(pasta, "TelaCaixa", {}, fotos, "r8-fechar")
        try:
            r.entrar("Ana")
            time.sleep(2)
            if not r.clicar("Fechar caixa"):
                raise RuntimeError(f"não achei o botão Fechar caixa (veja {r.saida})")
            time.sleep(1.5)
            xdo("type", "--delay", "60", "150")
            xdo("key", "Return")
            time.sleep(1.5)
            textos = r.textos()
            ok = caixas_abertos(pasta) == 1 and not tem(textos, "Fechar o caixa?")
            print(("✔ " if ok else "✘ ") + "caixa: o Enter no dinheiro contado não fecha o caixa"
                  + ("" if ok else f" (foto: {r.foto('-1')})"))
            tudo_certo &= ok
            if not r.clicar("Fechar e imprimir"):
                raise RuntimeError(f"não achei o botão Fechar e imprimir (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa: a pergunta mostra o contado, o esperado e a diferença", r.foto("-2"), r.textos(),
                                   tem=["Fechar o caixa?", "Contado: R$ 150,00", "Esperado na gaveta: R$"])
            xdo("key", "Return")
            time.sleep(1.5)
            textos = r.textos()
            ok = caixas_abertos(pasta) == 1 and tem(textos, "Fechar o caixa?")
            print(("✔ " if ok else "✘ ") + "caixa: o Enter não confirma a pergunta (o caixa continua aberto)"
                  + ("" if ok else f" (foto: {r.foto('-3')})"))
            tudo_certo &= ok
            xdo("key", "Escape")
            time.sleep(1)
            ok = caixas_abertos(pasta) == 1
            print(("✔ " if ok else "✘ ") + "caixa: o Esc desiste e o caixa continua aberto")
            tudo_certo &= ok
        finally:
            r.fechar()

        # ---- B. Pergunta de perigo ----
        r = Rustinho(pasta, "TelaPeca", {"produtoId": filtro}, fotos, "r8-perigo")
        try:
            r.entrar("Ana")
            time.sleep(2)
            # O nome aparece no título e no campo: o campo é o último (como em telas_seta_esc.py).
            pos = [(x, y) for x, y, t in r.textos_para_clicar() if t == "Filtro de óleo"]
            if not pos:
                raise RuntimeError(f"não achei o campo do nome da peça (veja {r.saida})")
            xdo("mousemove", str(pos[-1][0]), str(pos[-1][1]), "click", "1")
            time.sleep(0.5)
            xdo("key", "End")
            xdo("type", "--delay", "60", " X")
            time.sleep(0.5)
            xdo("key", "Escape")
            time.sleep(1.5)
            tudo_certo &= conferir("perigo: o Esc na ficha alterada pergunta", r.foto("-1"), r.textos(),
                                   tem=["Descartar alterações?"])
            xdo("key", "Return")
            time.sleep(1.5)
            textos = r.textos()
            ok = tem(textos, "Descartar alterações?") and tem(textos, "Filtro de óleo X")
            print(("✔ " if ok else "✘ ") + "perigo: o Enter não confirma o descarte (a pergunta e a mudança continuam)"
                  + ("" if ok else f" (foto: {r.foto('-2')})"))
            tudo_certo &= ok
        finally:
            r.fechar()

        # ---- C. Botões das linhas pelo teclado ----
        r = Rustinho(pasta, "TelaTrocas", {"secao": "feitas"}, fotos, "r11-trocas")
        try:
            r.entrar("Ana")
            time.sleep(2)
            xdo("mousemove", "1", "1")
            time.sleep(0.5)
            antes = r.textos()
            escondido = not any(simples(t) == simples("Desfazer") for _, _, t in antes)
            print(("✔ " if escondido else "✘ ") + "linhas: sem o mouse em cima, o Desfazer da linha está escondido"
                  + ("" if escondido else f" (foto: {r.foto('-1')})"))
            tudo_certo &= escondido
            apareceu = False
            for _ in range(40):
                xdo("key", "Tab")
                time.sleep(0.25)
                if any(simples(t) == simples("Desfazer") for _, _, t in r.textos()):
                    apareceu = True
                    break
            foto = r.foto("-2")
            print(("✔ " if apareceu else "✘ ") + "linhas: o Tab chega à linha e o Desfazer aparece, sem o mouse"
                  + ("" if apareceu else f" (foto: {foto})"))
            tudo_certo &= apareceu
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
