#!/usr/bin/env python3
"""Teste do `esperar_entrada` do `geometria_telas.py` (bateria da 009, 28/11).

Na bateria, o G3 leu a tela 2 s depois de abrir o programa e achou "Abrindo o Rustinho…" (o banco abre em segundo
plano, e sob carga passou de 2 s): 0 nomes, reprovado. Aqui, um programa de mentira devolve a tela de abertura nas
primeiras leituras e a de entrada depois, como no registro daquela rodada; o `esperar_entrada` tem de devolver os
nomes. Também confere que, se a entrada nunca aparece, ele desiste no prazo (sem travar). Não abre janela nenhuma.

    python3 app/testes/qml_geometria_espera_entrada.py   (entra sozinho na L3 e na CI, como os `qml_*.py`)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import geometria_telas as g  # noqa: E402

ABRINDO = [(180, 350, "Abrindo o Rustinho…"), (180, 402, "Não feche o Rustinho: isto pode levar alguns minutos.")]
ENTRADA = [(86, 300, "Quem está usando?")] + [(46, 360 + 70 * i, n) for i, n in enumerate(
    ("Ana", "Bia", "Carlos", "Dani", "Eduarda"))]


class Relogio:
    def __init__(self):
        self.agora = 0.0

    def time(self):
        return self.agora

    def sleep(self, s):
        self.agora += s


class Programa:
    """Abertura lenta: `lentas` leituras na tela de abertura (a primeira pode falhar, como uma janela que não
    responde), depois a de entrada."""

    def __init__(self, lentas):
        self.lentas = lentas
        self.leituras = 0

    def textos(self):
        self.leituras += 1
        if self.leituras == 1:
            raise RuntimeError("o Rustinho não escreveu os textos da tela")
        return list(ABRINDO if self.leituras <= self.lentas else ENTRADA)


def main():
    relogio = Relogio()
    g.time = relogio  # o roteiro usa time.time() e time.sleep(); aqui o tempo é de mentira
    g.esperar_janela = lambda: None
    erros = []

    p = Programa(lentas=12)  # ~6 s em "Abrindo", bem mais que os 2 s da espera fixa
    t = g.esperar_entrada(p)
    nomes = [tx for _, _, tx in t if tx in ("Ana", "Bia", "Carlos", "Dani", "Eduarda")]
    if len(nomes) != 5:
        erros.append(f"abertura lenta: esperava os 5 nomes, veio {nomes} depois de {p.leituras} leituras")
    else:
        print(f"ok: abertura lenta ({p.leituras} leituras, {relogio.agora:.1f} s de mentira): os 5 nomes")

    relogio.agora = 0.0
    p = Programa(lentas=10 ** 9)  # a entrada nunca aparece
    t = g.esperar_entrada(p, segundos=60)
    if any(tx == "Quem está usando?" for _, _, tx in t) or not 60 <= relogio.agora <= 62:
        erros.append(f"sem entrada: esperava desistir em ~60 s, desistiu em {relogio.agora:.1f} s")
    else:
        print(f"ok: sem entrada, desistiu em {relogio.agora:.1f} s de mentira")

    for e in erros:
        print("ERRO:", e)
    return 1 if erros else 0


if __name__ == "__main__":
    sys.exit(main())
