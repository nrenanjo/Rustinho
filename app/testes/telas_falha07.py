#!/usr/bin/env python3
"""Teste das telas da FALHA-07 (auditoria da 0.9.15): o Olho na tela Receber.

Com o Olho fechado, nenhum valor em reais aparece em Receber (os 4 totais do
topo, os valores por cliente, as parcelas, a janela de receber e o recibo);
aberto, tudo volta. O botão e o F12 fazem o mesmo, sem reabrir a tela, e a
cobrança continua funcionando com o Olho fechado.

A5 (decisão do dono, complemento da 0.9.15): o campo onde se digita o valor
continua legível (é a entrada da cobrança) e mostra o que se digita, sem
reexibir os outros valores; com a janela de receber aberta, nem o F12 nem o
clique no Olho mudam o Olho (fechado continua fechado, aberto continua aberto).

Usa a mesma loja de teste e o mesmo jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_falha07.py [pasta para as fotos]
"""

import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import PROGRAMA, TELA, Rustinho, conferir, montar_loja, simples, xdo  # noqa: E402

DINHEIRO = re.compile(r"r\$ ?-?[\d.]+,\d\d")


def valores(textos):
    return [t for _, _, t in textos if DINHEIRO.search(simples(t))]


def olho(nome, r, sufixo, fechado):
    textos = r.textos()
    foto = r.foto(sufixo)
    achados = valores(textos)
    if fechado:
        ok = not achados
        print(("✔ " if ok else "✘ ") + nome + ("" if ok else f": apareceu {achados[:4]} (foto: {foto})"))
    else:
        ok = bool(achados)
        print(("✔ " if ok else "✘ ") + nome + ("" if ok else f": nenhum valor apareceu (foto: {foto})"))
    return ok


def janela_sem_valores(nome, r, sufixo):
    """Nenhum valor em reais na tela, a não ser o campo onde se digita o
    valor (só o número, sem "R$")."""
    textos = r.textos()
    foto = r.foto(sufixo)
    mostrados = [t for t in valores(textos) if not re.fullmatch(r"[\d.]+,\d\d", t.strip())]
    ok = not mostrados
    print(("✔ " if ok else "✘ ") + nome + ("" if ok else f": {mostrados[:4]} (foto: {foto})"))
    return ok


def campo_legivel(nome, r, sufixo):
    """O campo do valor (só o número) aparece com o Olho fechado."""
    textos = r.textos()
    foto = r.foto(sufixo)
    ok = any(re.fullmatch(r"[\d.]+,\d\d", t.strip()) for _, _, t in textos)
    print(("✔ " if ok else "✘ ") + nome + ("" if ok else f" (foto: {foto})"))
    return ok


def recebimentos(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    n = c.execute("SELECT COUNT(*) FROM recebimento").fetchone()[0]
    c.close()
    return n


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-f07-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-f07-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        r = Rustinho(pasta, "TelaReceber", {}, fotos, "receber")
        try:
            r.entrar("Ana")
            time.sleep(1.5)
            tudo_certo &= conferir("receber: a tela abriu", r.foto("-0"), r.textos(), tem=["Total a receber", "Zé da Oficina"])
            tudo_certo &= olho("receber: Olho fechado, sem valores (totais e clientes)", r, "-1", True)
            if not r.clicar("Mostrar valores"):
                raise RuntimeError(f"não achei o botão Mostrar valores (veja {r.saida})")
            time.sleep(1)
            tudo_certo &= olho("receber: Olho aberto pelo botão, valores aparecem", r, "-2", False)
            xdo("key", "F12")
            time.sleep(1)
            tudo_certo &= olho("receber: F12 fecha de novo, sem reabrir a tela", r, "-3", True)
            xdo("key", "F12")
            time.sleep(1)
            tudo_certo &= olho("receber: F12 abre de novo", r, "-4", False)
            # Cliente: parcelas e o que já foi pago.
            if not r.clicar("Zé da Oficina"):
                raise RuntimeError(f"não achei o cliente (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= olho("receber › cliente: Olho aberto, valores das parcelas aparecem", r, "-5", False)
            xdo("key", "F12")
            time.sleep(1)
            tudo_certo &= olho("receber › cliente: Olho fechado, sem valores das parcelas", r, "-6", True)
            # A5: onde fica o botão do Olho (para clicar nele com a janela aberta).
            olho_xy = next(((x, y) for x, y, t in r.textos() if simples(t) == simples("Mostrar valores")), None)
            if olho_xy is None:
                raise RuntimeError(f"não achei o botão do Olho (veja {r.saida})")
            # Cobrança com o Olho fechado: abre a janela de receber.
            antes = recebimentos(pasta)
            if not r.clicar("Receber tudo"):
                raise RuntimeError(f"não achei Receber tudo (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= janela_sem_valores("receber › janela de receber: Olho fechado, sem valores", r, "-7")
            tudo_certo &= campo_legivel("receber › janela de receber: o campo do valor continua legível", r, "-7a")
            # A5: F12 e o clique no Olho com a janela aberta não mudam nada.
            xdo("key", "F12")
            time.sleep(1)
            tudo_certo &= janela_sem_valores("receber › janela aberta: F12 não abre o Olho", r, "-7b")
            xdo("mousemove", str(olho_xy[0]), str(olho_xy[1]), "click", "1")
            time.sleep(1)
            tudo_certo &= janela_sem_valores("receber › janela aberta: o clique no Olho não abre", r, "-7c")
            # A5: o campo ativo mostra o que se digita, e o resto continua escondido.
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "60", "60,00")
            time.sleep(1)
            textos = r.textos()
            ok = any(t.strip() == "60,00" for _, _, t in textos)
            print(("✔ " if ok else "✘ ") + "receber › janela de receber: o valor digitado aparece no campo")
            tudo_certo &= ok
            tudo_certo &= janela_sem_valores("receber › janela de receber: digitando, o resto continua escondido", r, "-7d")
            xdo("key", "Return")
            time.sleep(2)
            ok = recebimentos(pasta) == antes + 1
            print(("✔ " if ok else "✘ ") + "receber: a cobrança funcionou com o Olho fechado")
            tudo_certo &= ok
            tudo_certo &= olho("receber › recibo: Olho fechado, sem valores", r, "-8", True)
            # Fecha o recibo ("Pronto"). O F12 apertado com a janela aberta
            # não mudou o Olho: continua fechado.
            xdo("key", "Return")
            time.sleep(1)
            tudo_certo &= olho("receber › cliente: depois da janela, o Olho continua fechado", r, "-8b", True)
            xdo("key", "F12")
            time.sleep(1)
            tudo_certo &= olho("receber › cliente: Olho aberto de novo, o recebimento aparece", r, "-9", False)
            # A5: com o Olho aberto, a janela mostra os valores; o F12 com ela
            # aberta não fecha o Olho; fechar a janela (Esc) não recebe nada.
            antes = recebimentos(pasta)
            if not r.clicar("Receber tudo"):
                raise RuntimeError(f"não achei Receber tudo de novo (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= olho("receber › janela de receber: Olho aberto, valores aparecem", r, "-10", False)
            xdo("key", "F12")
            time.sleep(1)
            tudo_certo &= olho("receber › janela aberta: F12 não fecha o Olho", r, "-10b", False)
            xdo("key", "Escape")
            time.sleep(1)
            ok = recebimentos(pasta) == antes
            print(("✔ " if ok else "✘ ") + "receber: fechar a janela não recebe nada")
            tudo_certo &= ok
            tudo_certo &= olho("receber › cliente: depois da janela, o Olho continua aberto", r, "-11", False)
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if tudo_certo else 1)


if __name__ == "__main__":
    main()
