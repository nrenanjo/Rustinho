#!/usr/bin/env python3
"""Teste das telas do RVI-02 (revisão independente da 0.9.15).

Com a troca de dados ligada e a porta TCP da troca (47810) ocupada por outro
programa, o servidor da troca não começa. O Rustinho tenta de novo algumas
vezes e depois para de vez. O operador precisa ver isso na tela que usa:
o Início mostra o aviso (primeiro "não está funcionando", depois "parou",
com a porta ocupada como motivo) e Configurações › Caixas da loja mostra o
motivo, em vez de parecer que a troca está funcionando.

Usa a mesma loja de teste e o mesmo jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_rvi02.py [pasta para as fotos]
"""

import os
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import PROGRAMA, TELA, Rustinho, conferir, montar_loja, simples  # noqa: E402

PORTA_TCP = 47810


def montar_loja_com_troca(pasta):
    """A loja de teste de sempre, com a troca de dados ligada pela própria API
    (`sync_salvar`, como a tela de Configurações)."""
    os.environ["RUSTINHO_LOJA_TROCA"] = "1"
    try:
        montar_loja(pasta)
    finally:
        del os.environ["RUSTINHO_LOJA_TROCA"]
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    ligado = c.execute("SELECT valor FROM config WHERE chave = 'sync_ligado'").fetchone()
    c.close()
    assert ligado and ligado[0] == "1", "a troca de dados devia estar ligada na loja de teste"


def ocupar_porta():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("0.0.0.0", PORTA_TCP))
    s.listen(1)
    return s


def esperar_texto(r, pedaco, prazo):
    """Espera um texto aparecer na tela (o Início se atualiza sozinho)."""
    fim = time.time() + prazo
    while time.time() < fim:
        if any(pedaco in simples(t) for _, _, t in r.textos()):
            return True
        time.sleep(1)
    return False


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-rvi02-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-rvi02-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    ocupada = None
    try:
        montar_loja_com_troca(pasta)
        ocupada = ocupar_porta()
        print(f"porta TCP {PORTA_TCP} ocupada por este teste")

        r = Rustinho(pasta, "TelaInicio", {}, fotos, "rvi02-inicio")
        try:
            r.entrar("Ana")
            # Enquanto tenta de novo: o Início não diz que está tudo bem.
            ok = esperar_texto(r, simples("A troca de dados não está funcionando"), 20)
            print(("✔ " if ok else "✘ ") + "Início: enquanto tenta de novo, avisa que não está funcionando")
            r.foto("-1")
            tudo_certo &= ok
            # Depois do limite (5 falhas em ~30 s no programa): parou, com o motivo.
            ok = esperar_texto(r, simples("A troca de dados parou"), 60)
            print(("✔ " if ok else "✘ ") + "Início: depois do limite, aviso de que a troca parou")
            tudo_certo &= ok
            tudo_certo &= conferir(
                "Início: o motivo é a porta ocupada", r.foto("-2"), r.textos(),
                tem=[f"porta TCP {PORTA_TCP}", "outro programa pode estar usando essa porta"],
            )
        finally:
            r.fechar()
        time.sleep(1)

        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "caixas"}, fotos, "rvi02-config")
        try:
            r.entrar("Ana")
            ok = esperar_texto(r, simples("A troca de dados não está funcionando neste caixa"), 30)
            print(("✔ " if ok else "✘ ") + "Configurações › Caixas: a troca aparece como não funcionando")
            tudo_certo &= ok
            tudo_certo &= conferir(
                "Configurações › Caixas: com o motivo (porta TCP 47810)", r.foto("-1"), r.textos(),
                tem=[f"porta TCP {PORTA_TCP}"],
            )
        finally:
            r.fechar()
    finally:
        if ocupada:
            ocupada.close()
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if tudo_certo else 1)


if __name__ == "__main__":
    main()
