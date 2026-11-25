#!/usr/bin/env python3
"""R20 / DP-19 (decisão do proprietário, 25/11/2026) e M3 da revisão R1 da 006: para quem não vê os resultados (a Bia,
caixa), o custo some da lista do que comprar e do "Comprado por fornecedor" com o padrão D-09 do programa: "—" e "só o
gerente", e nunca um "R$" vazio. O gerente (Ana) continua vendo os valores.

Com o olho aberto, porque com ele fechado o custo some de qualquer jeito (OLHO-1).

    python3 app/testes/telas_r20_custos.py [pasta para as fotos]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import TELA, Rustinho, conferir, montar_loja  # noqa: E402

DINHEIRO = re.compile(r"R\$ ?-?[\d.]+,\d\d")
R_SOLTO = re.compile(r"(^|[^\w])R\$\s*$")


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r20-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r20-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        casos = [
            ("fornecedores", "TelaPainel", {"secao": "fornecedores"}, "Comprado"),
            ("comprar", "TelaEstoque", {"secao": "comprar"}, "Custo estimado"),
        ]
        for nome, tela, props, rotulo in casos:
            for quem, ve in (("Bia", False), ("Ana", True)):
                r = Rustinho(pasta, tela, props, fotos, f"r20-{nome}-{quem.lower()}")
                try:
                    r.entrar(quem)
                    time.sleep(2.5)
                    if not r.clicar("Mostrar valores"):
                        raise RuntimeError(f"não achei o botão Mostrar valores em {tela} (veja {r.saida})")
                    time.sleep(1.2)
                    textos = r.textos()
                    foto = r.foto("-1")
                    soltos = [t for _, _, t in textos if R_SOLTO.search(t.strip())]
                    if ve:
                        ok = any(DINHEIRO.search(t) for _, _, t in textos) and not soltos
                        print(("✔ " if ok else "✘ ") + f"{nome}, {quem} (gerente): o custo aparece"
                              + ("" if ok else f" (soltos {soltos}; foto: {foto})"))
                        tudo_certo &= ok
                    else:
                        tudo_certo &= conferir(f"{nome}, {quem} (caixa): \"{rotulo}\" com \"—\" e \"só o gerente\"", foto,
                                               textos, tem=["só o gerente"], igual=[rotulo, "—"])
                        ok = not soltos
                        print(("✔ " if ok else "✘ ") + f"{nome}, {quem} (caixa): nenhum \"R$\" vazio"
                              + ("" if ok else f": {soltos} (foto: {foto})"))
                        tudo_certo &= ok
                finally:
                    r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
