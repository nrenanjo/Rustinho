#!/usr/bin/env python3
"""Roteiro da T090 (OPUS-E-3 da auditoria da 0.9.18-dev.3.3.3): na pergunta que pede um motivo (estornar, tirar dívida,
desfazer devolução, cancelar OS...), a recusa do núcleo não aparecia depois que a pessoa tinha digitado no campo.

A causa: o `Campo` limpa o próprio erro a cada tecla (`campo.erro = ""`), e essa atribuição desfaz a ligação `erro:
motivo.erro` da janelinha; daí em diante o erro novo não chegava ao campo, e a janelinha ficava aberta sem dizer nada.

  CONTROLE  Receber › Zé › "Tirar" a dívida, Enter com o motivo vazio: o núcleo recusa ("Diga o motivo.") e aparece.
  DIGITADO  de novo, digitar "a", apagar e dar Enter: a mesma recusa tem de aparecer (antes, nada aparecia).

    python3 app/testes/telas_motivo_erro_t090.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":94")
TELA = base.TELA
RECUSA = "Diga o motivo."


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def montar_loja(pasta):
    """A loja de teste com os avisos (tem a dívida manual do Zé, "Caderno", R$ 50, que dá para tirar)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_LOJA_AVISOS="1")
    subprocess.run(["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--", "--ignored", "--exact",
                    "montar_loja_de_teste"], cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True)
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    ze = c.execute("SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'").fetchone()[0]
    c.close()
    return ze


def textos(r):
    return [t for _, _, t in r.textos()]


def esperar(r, condicao, segundos=6.0):
    fim = time.time() + segundos
    tx = textos(r)
    while not condicao(tx) and time.time() < fim:
        time.sleep(0.5)
        tx = textos(r)
    return tx


def main():
    certo = True

    def ok(nome, condicao, detalhe=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" ({detalhe})"))
        certo &= bool(condicao)

    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-t090-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-t090-")
    pasta = os.path.join(raiz, "loja")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        ze = montar_loja(pasta)
        r = base.Rustinho(pasta, "TelaReceber", {"clienteInicial": ze}, fotos, "receber")
        try:
            r.entrar("Ana")
            tx = esperar(r, lambda t: "Tirar" in t)
            if "Tirar" not in tx:
                raise RuntimeError(f"INSTRUMENTAÇÃO: o Tirar da dívida do Zé não apareceu (foto {r.foto('-instr')})")

            # CONTROLE: o motivo vazio, sem digitar nada.
            r.clicar("Tirar")
            tx = esperar(r, lambda t: "Tirar esta dívida?" in t)
            if "Tirar esta dívida?" not in tx:
                raise RuntimeError(f"INSTRUMENTAÇÃO: a pergunta não abriu (foto {r.foto('-instr2')})")
            xdo("key", "Return")
            tx = esperar(r, lambda t: RECUSA in t)
            ok("CONTROLE: com o motivo vazio, a recusa do núcleo aparece", RECUSA in tx, f"foto {r.foto('-1-controle')}")
            xdo("key", "Escape")
            time.sleep(1.5)

            # DIGITADO: digitar, apagar e confirmar.
            r.clicar("Tirar")
            tx = esperar(r, lambda t: "Tirar esta dívida?" in t)
            xdo("type", "--delay", "80", "a")
            time.sleep(0.4)
            xdo("key", "BackSpace")
            time.sleep(0.4)
            xdo("key", "Return")
            tx = esperar(r, lambda t: RECUSA in t)
            f = r.foto("-2-digitado")
            ok("DIGITADO: depois de digitar no campo, a recusa do núcleo também aparece", RECUSA in tx, f"foto {f}")
            ok("DIGITADO: a pergunta continua aberta (nada foi tirado)", "Tirar esta dívida?" in tx, f"foto {f}")
            c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
            n = c.execute("SELECT COUNT(*) FROM conta_receber WHERE cancelado_em IS NOT NULL").fetchone()[0]
            c.close()
            ok("a dívida não foi tirada", n == 0, f"{n} tiradas")
            with open(r.saida, encoding="utf-8", errors="replace") as reg:
                erros = [linha.strip() for linha in reg if "TypeError" in linha or "ReferenceError" in linha]
            ok("nenhum TypeError/ReferenceError no registro", not erros, str(erros[:3]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(raiz, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()
    print(f"\nfotos em {fotos}")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
