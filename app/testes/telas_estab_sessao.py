#!/usr/bin/env python3
"""Frente estab (0.9.18), item 7: as janelinhas quando a sessão acaba.

Um caixa (Bia, sem ser gerente) abre no Início uma função do gerente
(Financeiro): aparece o pedido da senha do gerente. Enquanto ele está aberto,
a Bia é desativada "em outro caixa" (o teste grava direto no banco, como a
troca de dados faria). O Início confere de novo a cada 20 s; a sessão acaba e
o Rustinho volta para a tela de entrada. O pedido da senha do gerente era da
sessão que acabou: não pode continuar aberto por cima da tela de entrada
(antes da correção, continuava; e, ao fechá-lo, rodava o "desistiu" de uma
tela que já não existe). Também confere que a faixa da navegação guiada não
sobra (`largarGuia`), e que a próxima pessoa entra normalmente.

Abre o Rustinho numa tela virtual (Xvfb), com o ambiente neutro de
telas_d09.py (só X11, sem Wayland, sem o tema do desktop). Nada abre na área
de trabalho.

    RUSTINHO_TELA=:65 python3 app/testes/telas_estab_sessao.py [pasta para as fotos]
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
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":65")
TELA = base.TELA

# O pedido da senha do gerente no modo consulta (o texto da nota dele).
PEDIDO = "você só consulta"
ENTRADA = "Quem está usando?"


def montar_loja(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
         "--ignored", "--exact", "montar_loja_de_teste"],
        cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def ambiente_do_processo(pid):
    """DISPLAY, QT_QPA_PLATFORM e se há WAYLAND_DISPLAY no processo do Rustinho."""
    for _ in range(20):
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(x.split(b"=", 1) for x in f.read().split(b"\0") if b"=" in x)
        except OSError:
            env = {}
        if b"DISPLAY" in env:
            return (env.get(b"DISPLAY", b"").decode(), env.get(b"QT_QPA_PLATFORM", b"").decode(),
                    b"WAYLAND_DISPLAY" in env)
        time.sleep(0.25)
    return ("", "", True)


def desativar(pasta, nome):
    """Como a troca de dados faria ao receber de outro caixa: o usuário fica
    inativo. Os gatilhos da troca ficam parados nesta gravação (eles usam
    funções que só o Rustinho tem)."""
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"), timeout=20, isolation_level=None)
    # Os gatilhos citam estas funções; com "aplicando" ligado, não as chamam,
    # mas o SQLite precisa conhecê-las para montar o gatilho.
    for nome_f, n in (("rustinho_id", 0), ("rustinho_agora", 0), ("rustinho_operacao", 0),
                      ("rustinho_historia", 3), ("normalizar", 1)):
        c.create_function(nome_f, n, lambda *x: None)
    c.execute("BEGIN IMMEDIATE")
    c.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1")
    c.execute("UPDATE usuario SET ativo = 0 WHERE nome = ?", (nome,))
    c.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1")
    c.execute("COMMIT")
    c.close()


class Rustinho(base.Rustinho):
    def tem(self, pedaco):
        return any(simples(pedaco) in simples(t) for _, _, t in self.textos())

    def esperar(self, condicao, segundos=8):
        fim = time.time() + segundos
        while time.time() < fim:
            if condicao():
                return True
            time.sleep(0.5)
        return condicao()


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-estab-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-estab-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True
    erros_js = []

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"), flush=True)
        certo &= bool(condicao)

    try:
        montar_loja(pasta)
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "1-sessao")
        try:
            display, qpa, wayland = ambiente_do_processo(r.p.pid)
            print(f"  processo {r.p.pid}: DISPLAY={display} QT_QPA_PLATFORM={qpa} WAYLAND_DISPLAY={'sim' if wayland else 'não'}")
            ok("ambiente: tela virtual, xcb, sem Wayland", display == TELA and qpa == "xcb" and not wayland)
            r.entrar("Bia")
            ok("Início aberto", r.esperar(lambda: r.tem("Financeiro")), r.foto("-0"))
            r.clicar("Financeiro")
            ok("pede a senha do gerente", r.esperar(lambda: r.tem(PEDIDO)), r.foto("-1"))
            desativar(pasta, "Bia")
            # O Início confere de novo a cada 20 s.
            voltou = r.esperar(lambda: r.tem(ENTRADA), 40)
            f = r.foto("-2")
            ok("a sessão acabou: tela de entrada", voltou, f)
            ok("o motivo aparece", r.tem("desativado"), f)
            ok("o pedido da senha do gerente NÃO ficou aberto por cima da entrada", not r.tem(PEDIDO), f)
            ok("nenhuma faixa da navegação guiada sobrou", not r.tem("conclua a operação") and not r.tem("Não foi salvo"), f)
            # Esc na entrada não roda nada da tela que saiu.
            base.xdo("key", "Escape")
            time.sleep(1)
            # A próxima pessoa entra normalmente.
            r.entrar("Ana")
            f = r.foto("-3")
            ok("a Ana entra e vê o Início", r.esperar(lambda: r.tem("Financeiro") and not r.tem(ENTRADA)), f)
            ok("sem o pedido da senha", not r.tem(PEDIDO), f)
        finally:
            r.fechar()
            with open(r.saida, encoding="utf-8", errors="replace") as fl:
                for linha in fl:
                    if "TypeError" in linha or "ReferenceError" in linha:
                        erros_js.append(linha.strip())
        ok("nenhum TypeError/ReferenceError no registro do Qt", not erros_js)
        for linha in erros_js[:10]:
            print("   ", linha)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
        shutil.rmtree(pasta, ignore_errors=True)
    print("TUDO CERTO" if certo else "FALHOU")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
