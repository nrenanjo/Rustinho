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


def focar_janela(proc):
    """No Xvfb sem gerenciador de janelas, entrega o foco de teclado à 2ª instância pelo PID."""
    fim = time.time() + 15
    while time.time() < fim:
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(proc.pid)],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        wids = s.stdout.split()
        if wids:
            subprocess.run(["xdotool", "windowfocus", "--sync", wids[-1]],
                           env=dict(os.environ, DISPLAY=TELA), check=False)
            return
        time.sleep(0.2)


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
                "Configurações › Caixas: com o motivo (porta TCP 47810) e botão Tentar de novo", r.foto("-1"), r.textos(),
                tem=[f"porta TCP {PORTA_TCP}", "Tentar de novo"],
            )
            # RD-B: quando a porta é liberada e a pessoa clica em "Tentar de novo", a troca volta a funcionar na hora.
            ocupada.close()
            ocupada = None
            r.clicar("Tentar de novo")
            ok_volta = esperar_texto(r, simples("Troca de dados funcionando novamente"), 15)
            print(("✔ " if ok_volta else "✘ ") + "Configurações › Caixas: Tentar de novo retoma a troca quando a porta libera")
            r.foto("-2-voltou")
            tudo_certo &= ok_volta

            # RD-B (2 processos reais do Rustinho no mesmo computador em pastas diferentes):
            # Com a 1ª instância agora segurando a porta 47810, abre uma 2ª instância em `pasta2`
            # e verifica que ela identifica a loja e o caixa da 1ª instância na tela e retoma após a 1ª fechar.
            pasta2 = tempfile.mkdtemp(prefix="rustinho-loja2-rvi02-")
            try:
                montar_loja_com_troca(pasta2)
                r2 = Rustinho(pasta2, "TelaConfiguracoes", {"secao": "caixas"}, fotos, "rvi02-segunda-copia")
                try:
                    focar_janela(r2.p)
                    r2.entrar("Ana")
                    ok_id = esperar_texto(r2, simples("outro Rustinho aberto neste computador"), 30)
                    print(("✔ " if ok_id else "✘ ") + "2ª instância real: identifica que outro Rustinho deste computador ocupa a porta")
                    tudo_certo &= ok_id
                    tudo_certo &= conferir(
                        "2ª instância real: mostra loja/caixa do 1º Rustinho e botão Tentar de novo",
                        r2.foto("-1-conflito-duas-copias"),
                        r2.textos(),
                        tem=["outro Rustinho aberto neste computador", "Caixa 1", "Tentar de novo"],
                    )
                    # Fecha a 1ª instância e clica em "Tentar de novo" na 2ª instância:
                    r.fechar()
                    time.sleep(1)
                    r2.clicar("Tentar de novo")
                    ok_r2 = esperar_texto(r2, simples("Troca de dados funcionando novamente"), 15)
                    print(("✔ " if ok_r2 else "✘ ") + "2ª instância real: assume a porta ao clicar em Tentar de novo após fechar a 1ª")
                    r2.foto("-2-assumiu")
                    tudo_certo &= ok_r2
                finally:
                    r2.fechar()
            finally:
                shutil.rmtree(pasta2, ignore_errors=True)
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
