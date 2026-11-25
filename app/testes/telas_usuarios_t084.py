#!/usr/bin/env python3
"""Roteiro da T084 (texto O do proprietário, 21/11/2026), atualizado pelo R1 (D-AUD-3, decisão de 25/11/2026): o
"Trocar usuário" saiu da página inicial; a tela de Usuários (pelo cartão do menu) tem o "Sair"; a tela de Usuários não
tem mais o "+ Novo usuário" (o formulário de criação já fica à direita), e o cadastro continua funcionando.

  ESTÁTICA  no código: o Início sem o "Trocar usuário" (R1); o "Sair" de Usuários chama `janela.trocarUsuario()`; e o
            "+ Novo usuário" só aparece na tela estreita (onde o formulário não fica à vista e o botão é a única entrada
            para ele).
  GERENTE   (Ana) o Início sem o "Trocar usuário"; o cartão Usuários do menu abre Usuários, com o "Sair".
  LARGA     (a tela de Usuários aberta direto, 1280 px) sem o "+ Novo usuário", com o formulário (Nome, as senhas,
            Salvar F2); um usuário novo é criado pelo formulário, com o F2, e entra na lista e no banco; clicar num
            usuário da lista abre a edição dele (o gerenciamento de antes continua).
  CAIXA     (Bia) o cartão Usuários pede a senha do gerente (o "Sair" fica do outro lado dela; R1).
  ESTREITA  (700 px) o "+ Novo usuário" continua e abre o formulário: o cadastro não fica sem entrada.

    python3 app/testes/telas_usuarios_t084.py [pasta para as fotos]
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
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":93")
TELA = base.TELA
QML = os.path.join(RAIZ, "app", "qml")
MARCA_USUARIOS = "Todos acessam todas as funções. Cada um entra com o próprio nome e senha."
MARCA_FORMULARIO = "Como aparece na tela de entrada"


def bloco_do_botao(texto, rotulo):
    """O corpo do Botao com `text: "<rotulo>"`."""
    i = texto.index(f'text: "{rotulo}"')
    inicio = texto.rindex("Botao {", 0, i)
    prof = 0
    for j in range(inicio, len(texto)):
        prof += {"{": 1, "}": -1}.get(texto[j], 0)
        if prof == 0 and texto[j] == "}":
            return texto[inicio:j + 1]
    return texto[inicio:]


def estatica(qml=QML):
    """[(o que falta)] no código."""
    faltas = []
    inicio = open(os.path.join(qml, "TelaInicio.qml"), encoding="utf-8").read()
    if 'text: "Trocar usuário"' in inicio:
        faltas.append("o Início ainda tem o Trocar usuário (R1 tirou)")
    usuarios = open(os.path.join(qml, "TelaUsuarios.qml"), encoding="utf-8").read()
    if "janela.trocarUsuario()" not in bloco_do_botao(usuarios, "Sair"):
        faltas.append("o Sair de Usuários não chama janela.trocarUsuario()")
    novo = bloco_do_botao(usuarios, "Novo usuário")
    if not re.search(r"visible:\s*Tema\.estreito\b", novo):
        faltas.append("o + Novo usuário aparece na tela larga")
    return faltas


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def redimensionar(largura, altura):
    ids = []
    for _ in range(150):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        ids = s.stdout.split()
        if ids:
            break
        time.sleep(0.2)
    if not ids:
        raise RuntimeError("INSTRUMENTAÇÃO: não achei a janela do Rustinho")
    for j in ids:
        xdo("windowsize", j, str(largura), str(altura))
        xdo("windowmove", j, "0", "0")
    xdo("mousemove", str(largura // 2), str(altura // 2))
    xdo("windowfocus", "--sync", ids[0])
    time.sleep(1.5)


def textos(r):
    return [t for _, _, t in r.textos()]


def botao(tx, nome):
    """O botão (um lugar de toque, pelo --medir-toque) com esse nome está na tela? O texto sozinho não basta: o
    painel do formulário tem o título "Novo usuário", com o mesmo texto do botão."""
    return any(x.startswith("TOQUE:") and x.split("|")[3] == nome and x.split("|")[-1] == "1" for x in tx)


def esperar(r, condicao, segundos=6.0):
    fim = time.time() + segundos
    tx = textos(r)
    while not condicao(tx) and time.time() < fim:
        time.sleep(0.5)
        tx = textos(r)
    return tx


def clicar_no_texto(r, texto):
    if not r.clicar(texto):
        raise RuntimeError(f"INSTRUMENTAÇÃO: não achei '{texto}' para clicar (veja {r.saida})")
    time.sleep(1.5)


def main():
    certo = True

    def ok(nome, condicao, detalhe=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" ({detalhe})"))
        certo &= bool(condicao)

    faltas = estatica()
    ok("ESTÁTICA: o Início sem o Trocar usuário e o Sair em Usuários (R1); o + Novo usuário só na tela estreita", not faltas,
       str(faltas))

    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-t084-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-t084-")
    antes = os.environ.get("RUSTINHO_ARGS_TESTE")
    os.environ["RUSTINHO_ARGS_TESTE"] = ((antes or "") + " --medir-toque").strip()
    pasta = os.path.join(raiz, "loja")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)

        # GERENTE, tela larga
        r = base.Rustinho(pasta, "TelaInicio", {}, fotos, "gerente")
        try:
            r.entrar("Ana")
            redimensionar(1280, 900)
            tx = esperar(r, lambda t: "Usuários" in t)
            if "Usuários" not in tx:
                raise RuntimeError(f"INSTRUMENTAÇÃO: a página inicial não apareceu (foto {r.foto('-instr')})")
            ok("GERENTE: o Início não tem o Trocar usuário (R1)", "Trocar usuário" not in tx, f"foto {r.foto('-1-inicio')}")
            clicar_no_texto(r, "Usuários")
            tx = esperar(r, lambda t: MARCA_USUARIOS in t)
            ok("GERENTE: o cartão Usuários abre a tela de Usuários, com o Sair (R1)",
               MARCA_USUARIOS in tx and "Sair" in tx, f"foto {r.foto('-3-cartao')}")
            with open(r.saida, encoding="utf-8", errors="replace") as reg:
                erros = [linha.strip() for linha in reg if "TypeError" in linha or "ReferenceError" in linha]
            ok("GERENTE: nenhum TypeError/ReferenceError no registro", not erros, str(erros[:3]))
        finally:
            r.fechar()

        # LARGA: a tela de Usuários aberta direto (o cadastro, sem depender do atalho)
        r = base.Rustinho(pasta, "TelaUsuarios", {}, fotos, "larga")
        try:
            r.entrar("Ana")
            redimensionar(1280, 900)
            tx = esperar(r, lambda t: MARCA_USUARIOS in t)
            if MARCA_USUARIOS not in tx:
                raise RuntimeError(f"INSTRUMENTAÇÃO: a tela de Usuários não abriu (foto {r.foto('-instr')})")
            f = r.foto("-1-tela")
            if not any(x.startswith("TOQUE:") for x in tx):
                raise RuntimeError("INSTRUMENTAÇÃO: sem as medidas de toque (--medir-toque)")
            ok("LARGA: na tela larga não há o + Novo usuário", not botao(tx, "Novo usuário"), f"foto {f}")
            ok("LARGA: o formulário de criação está à vista (Nome, senhas, Salvar)",
               all(x in tx for x in (MARCA_FORMULARIO, "Pelo menos 4 caracteres", "Repita a senha", "Salvar")),
               f"foto {f}")
            # Cria um usuário pelo formulário, com o F2.
            clicar_no_texto(r, MARCA_FORMULARIO)
            xdo("type", "--delay", "60", "Caio Teste")
            clicar_no_texto(r, "Pelo menos 4 caracteres")
            xdo("type", "--delay", "60", "4321")
            clicar_no_texto(r, "Repita a senha")
            xdo("type", "--delay", "60", "4321")
            xdo("key", "F2")
            time.sleep(2)
            tx = esperar(r, lambda t: any(x.startswith("Caio Teste") for x in t))
            f = r.foto("-2-criado")
            ok("LARGA: o usuário novo, criado pelo formulário e o F2, aparece na lista",
               any(x.startswith("Caio Teste") for x in tx), f"foto {f}")
            c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
            n = c.execute("SELECT COUNT(*) FROM usuario WHERE nome = 'Caio Teste' AND ativo = 1").fetchone()[0]
            c.close()
            ok("LARGA: o usuário novo está no banco, ativo", n == 1, f"{n} linhas")
            # Editar um usuário que existe continua (o gerenciamento de antes): o formulário mostra a Bia.
            clicar_no_texto(r, "Bia")
            tx = esperar(r, lambda t: any("Trocar a senha" in x for x in t))
            ok("LARGA: clicar num usuário abre a edição dele (o gerenciamento de antes)",
               any("Trocar a senha" in x for x in tx), f"foto {r.foto('-3-editar')}")
        finally:
            r.fechar()

        # CAIXA: o mesmo fluxo do cartão (a senha do gerente)
        r = base.Rustinho(pasta, "TelaInicio", {}, fotos, "caixa")
        try:
            r.entrar("Bia")
            redimensionar(1280, 900)
            esperar(r, lambda t: "Usuários" in t)
            clicar_no_texto(r, "Usuários")
            tx = esperar(r, lambda t: "Senha do gerente" in t)
            f = r.foto("-1-usuarios")
            ok("CAIXA: o cartão Usuários pede a senha do gerente (o Sair fica em Usuários; R1)", "Senha do gerente" in tx,
               f"foto {f}")
        finally:
            r.fechar()

        # ESTREITA: o + Novo usuário continua e abre o formulário
        r = base.Rustinho(pasta, "TelaUsuarios", {}, fotos, "estreita")
        try:
            r.entrar("Ana")
            redimensionar(700, 900)
            tx = esperar(r, lambda t: botao(t, "Novo usuário"))
            f = r.foto("-1-lista")
            ok("ESTREITA: o + Novo usuário continua (o formulário não fica à vista)", botao(tx, "Novo usuário"),
               f"foto {f}")
            ok("ESTREITA: o formulário não está à vista antes do botão", MARCA_FORMULARIO not in tx, f"foto {f}")
            if botao(tx, "Novo usuário"):
                clicar_no_texto(r, "Novo usuário")
                tx = esperar(r, lambda t: MARCA_FORMULARIO in t)
                ok("ESTREITA: ele abre o formulário de criação", MARCA_FORMULARIO in tx, f"foto {r.foto('-2-formulario')}")
        finally:
            r.fechar()
    finally:
        if antes is None:
            os.environ.pop("RUSTINHO_ARGS_TESTE", None)
        else:
            os.environ["RUSTINHO_ARGS_TESTE"] = antes
        shutil.rmtree(raiz, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()
    print(f"\nfotos em {fotos}")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
