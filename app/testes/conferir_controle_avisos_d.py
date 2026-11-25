#!/usr/bin/env python3
"""Prova determinística do critério do controle do D em `telas_avisos_topo.py` (estabilização de 25/11/2026).

- O caso da CI 45574350469 (1,49 s na tela, o programa dizendo "tempo"): o critério de antes reprovava; o novo aprova.
- O novo critério continua reprovando quando o programa diz outra coisa ("toque", "" = ainda aberto) ou quando o aviso
  não sumiu (None); e, num programa de antes (sem a linha), continua pelo tempo (>= 1,5 s).
- O roteiro usa mesmo a função no controle do D (checagem pela árvore do código, B3 da revisão E1).

    python3 app/testes/conferir_controle_avisos_d.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_avisos_topo import controle_valido  # noqa: E402


def criterio_de_antes(natural, _como):
    return natural is not None and natural >= 1.5


CASOS = [
    # (natural, o programa diz, de antes, novo)
    (1.49, "tempo", False, True),   # a CI 45574350469
    (3.2, "tempo", True, True),
    (1.49, "toque", False, False),
    (3.2, "toque", True, False),    # sem toque no controle, "toque" é defeito (antes reprovava pelo 2º ok do D)
    (3.2, "", True, False),
    (None, "tempo", False, False),
    (None, None, False, False),
    (1.49, None, False, False),     # programa de antes: pelo tempo
    (2.0, None, True, True),
]


def ligacao_no_cenario():
    """B3 da revisão E1: o controle do D no roteiro usa mesmo `controle_valido(natural, como_controle)` (sem isso, a
    prova da função passaria com o roteiro voltando ao critério antigo)."""
    import ast
    caminho = os.path.join(os.path.dirname(os.path.abspath(__file__)), "telas_avisos_topo.py")
    arvore = ast.parse(open(caminho, encoding="utf-8").read())
    for no in ast.walk(arvore):
        if (isinstance(no, ast.Call) and isinstance(no.func, ast.Name) and no.func.id == "ok" and no.args
                and isinstance(no.args[0], ast.Constant)
                and str(no.args[0].value).startswith("D (controle): sem toque, o aviso some sozinho")):
            usa = [c for c in ast.walk(no.args[1]) if isinstance(c, ast.Call)
                   and isinstance(c.func, ast.Name) and c.func.id == "controle_valido"
                   and [a.id for a in c.args if isinstance(a, ast.Name)] == ["natural", "como_controle"]]
            return bool(usa)
    return False


def main():
    erros = []
    if not ligacao_no_cenario():
        erros.append("o controle do D no roteiro não usa controle_valido(natural, como_controle)")
    for natural, como, antes, novo in CASOS:
        if criterio_de_antes(natural, como) != antes:
            erros.append(f"critério de antes com {natural}/{como!r}: esperado {antes}")
        if controle_valido(natural, como) != novo:
            erros.append(f"critério novo com {natural}/{como!r}: esperado {novo}")
    for e in erros:
        print("✘", e)
    print("RESULTADO:", "tudo certo" if not erros else "HÁ FALHA", f"({len(CASOS)} casos)")
    return 1 if erros else 0


if __name__ == "__main__":
    sys.exit(main())
