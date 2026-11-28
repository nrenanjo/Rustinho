#!/usr/bin/env python3
"""T040 (B-1 da R20): a escolha das fotos de véu cheio no `telas_luz_guia_temas` não pode ser puxada por uma foto fora
de série. Sem janela: monta as medidas de 10 fotos (como as do roxo, véu 52,33 e luz 4,33) e põe nelas uma foto do
começo do esmaecer, em que o véu dobra por um instante (véu 64, luz 2,4). Antes (o "cheio" era o MAIOR véu), só essa
foto ficava e a medida saía 2,4: um falso vermelho. Depois (o "cheio" é a mediana, e o que passa de 1,1 vez a mediana
sai), a medida é a das 9 fotos boas.

    python3 app/testes/qml_luz_escolher.py   (entra sozinho na L3 e na CI, como os `qml_*.py`)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_luz_guia_temas as luz  # noqa: E402


def medida(veu, l2, l8):
    return {2: l2, 8: l8, "longe": 48, "veu": veu}


def main():
    erros = []
    boas = [medida(52.33, 4.33, 2.67) for _ in range(9)]
    fora = medida(64.0, 2.4, 1.5)  # o véu dobrado do começo do esmaecer
    sem_destaque = [medida(0.0, -20.0, -20.0), medida(0.0, -20.0, -20.0)]
    m = luz.escolher(sem_destaque + boas + [fora])
    if m is None or m[2] != 4.33 or m[8] != 2.67 or m["fotos"] != 9:
        erros.append(f"com uma foto fora de série: {m} (esperado 4,33 / 2,67 com 9 fotos)")
    else:
        print(f"ok: a foto fora de série não puxa a medida ({m})")
    m = luz.escolher(sem_destaque + [medida(52.33, 4.33, 2.67), medida(52.0, 4.0, 2.5)])
    if m is not None:
        erros.append(f"com só 2 fotos de véu cheio a medida devia ser recusada (None), veio {m}")
    else:
        print("ok: com menos de 3 fotos de véu cheio, a medida é recusada")
    for e in erros:
        print("ERRO:", e)
    return 1 if erros else 0


if __name__ == "__main__":
    sys.exit(main())
