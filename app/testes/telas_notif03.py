#!/usr/bin/env python3
"""NOTIF-01 e NOTIF-03 (0.9.18), pela tela.

NOTIF-01: o botão das notificações que levam a Simultaneidade › Duplicatas
diz "Ver duplicatas" (antes: "Resolver no Gerente").

NOTIF-03: um caixa (Bia, sem ser gerente) clica no botão de uma notificação
que abre uma parte do gerente em Configurações e desiste da senha (Voltar ou
Esc): volta ao Início, em vez de ficar preso em Configurações › Loja. Com a
senha do gerente, entra na parte certa. Controle: dentro de Configurações,
desistir da senha ao tocar na pílula Simultaneidade continua deixando a
pessoa em Configurações (ela escolheu estar ali).

Abre o Rustinho numa tela virtual (Xvfb), com o ambiente neutro de
telas_d09.py (só X11, sem Wayland, sem o tema do desktop). Nada abre na área
de trabalho.

    python3 app/testes/telas_notif03.py [pasta para as fotos]
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":95")
TELA = base.TELA


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def montar_loja(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_LOJA_AVISOS="1")
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
         "--ignored", "--exact", "montar_loja_de_teste"],
        cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def conferir_ambiente(pid):
    """O Rustinho do teste tem de estar na tela virtual, sem Wayland. Espera o
    processo terminar de começar (logo depois de criado, /proc ainda pode
    estar vazio)."""
    for _ in range(20):
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(x.split(b"=", 1) for x in f.read().split(b"\0") if b"=" in x)
        except OSError:
            env = {}
        if b"DISPLAY" in env:
            return env.get(b"DISPLAY") == TELA.encode() and b"WAYLAND_DISPLAY" not in env
        time.sleep(0.25)
    return False


# O pedido da senha do gerente (PedirGerente, modo consulta). "Senha do
# gerente" sozinho não serve: é também um botão do cabeçalho para o caixa.
PEDIDO = "você só consulta"


class Rustinho(base.Rustinho):
    def pilha(self):
        for _, _, t in self.textos():
            if t.startswith("TESTE_PILHA:"):
                return int(t.split(":")[1])
        return None

    def tem(self, pedaco):
        return any(simples(pedaco) in simples(t) for _, _, t in self.textos())

    def abrir_notificacao(self):
        """O botão da notificação que leva a Duplicatas (o texto antigo vale
        para rodar o teste no programa de antes da correção)."""
        return self.clicar("Ver duplicatas") or self.clicar("Resolver no Gerente")

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
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-notif03-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-notif03-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True
    logs = []

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"), flush=True)
        certo &= bool(condicao)

    try:
        montar_loja(pasta)

        # 1. Notificação → senha → Voltar (botão).
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "1-voltar")
        logs.append(r.saida)
        try:
            ok("ambiente: tela virtual, sem Wayland", conferir_ambiente(r.p.pid))
            r.entrar("Bia")
            f = r.foto("-0")
            ok("NOTIF-01: botão 'Ver duplicatas' na notificação", r.tem("Ver duplicatas"), f)
            ok("NOTIF-01: nenhum 'Resolver no Gerente'", not r.tem("Resolver no Gerente"), f)
            inicio = r.pilha()
            ok("Início aberto", inicio is not None, f)
            r.abrir_notificacao()
            ok("pede a senha do gerente", r.esperar(lambda: r.tem(PEDIDO)), r.foto("-1"))
            ok("Configurações por cima do Início", r.pilha() == inicio + 1)
            r.clicar_seta_de_cima()
            f = r.foto("-2")
            ok("NOTIF-03: desistir (Voltar) volta ao Início", r.esperar(lambda: r.pilha() == inicio), f)
            ok("NOTIF-03: sem a senha aberta", not r.tem(PEDIDO), f)
            ok("NOTIF-03: a notificação continua lá", r.tem("Ver duplicatas") or r.tem("Resolver no Gerente"), f)
        finally:
            r.fechar()

        # 2. O mesmo, desistindo com Esc.
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "2-esc")
        logs.append(r.saida)
        try:
            ok("ambiente: tela virtual, sem Wayland (2)", conferir_ambiente(r.p.pid))
            r.entrar("Bia")
            inicio = r.pilha()
            r.abrir_notificacao()
            ok("pede a senha do gerente (2)", r.esperar(lambda: r.tem(PEDIDO)), r.foto("-1"))
            xdo("key", "Escape")
            f = r.foto("-2")
            ok("NOTIF-03: desistir (Esc) volta ao Início", r.esperar(lambda: r.pilha() == inicio), f)
        finally:
            r.fechar()

        # 3. Com a senha do gerente: entra em Duplicatas.
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "3-senha")
        logs.append(r.saida)
        try:
            ok("ambiente: tela virtual, sem Wayland (3)", conferir_ambiente(r.p.pid))
            r.entrar("Bia")
            inicio = r.pilha()
            r.abrir_notificacao()
            r.esperar(lambda: r.tem(PEDIDO))
            time.sleep(0.5)
            xdo("type", "--delay", "80", base.SENHA["Ana"])
            xdo("key", "Return")
            f = r.foto("-1")
            ok("com a senha, fica em Configurações",
               r.esperar(lambda: r.pilha() == inicio + 1 and not r.tem(PEDIDO)), f)
            ok("e mostra Duplicatas", r.tem("Duplicatas"), f)
        finally:
            r.fechar()

        # 4. Controle: Configurações aberta pela pessoa; a pílula
        # Simultaneidade pede a senha; desistir deixa em Configurações.
        r = Rustinho(pasta, "TelaConfiguracoes", {}, fotos, "4-pilula")
        logs.append(r.saida)
        try:
            ok("ambiente: tela virtual, sem Wayland (4)", conferir_ambiente(r.p.pid))
            r.entrar("Bia")
            f = r.foto("-0")
            ok("Configurações aberta direto (Loja)", r.pilha() is not None, f)
            antes = r.pilha()
            r.clicar("Simultaneidade")
            ok("a pílula pede a senha", r.esperar(lambda: r.tem(PEDIDO)), r.foto("-1"))
            r.clicar_seta_de_cima()
            f = r.foto("-2")
            ok("controle: desistir pela pílula continua em Configurações",
               r.esperar(lambda: not r.tem(PEDIDO)) and r.pilha() == antes, f)
        finally:
            r.fechar()

        erros = []
        for log in logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("TUDO CERTO" if certo else "HOUVE FALHAS")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
