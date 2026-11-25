#!/usr/bin/env python3
"""R1 / D-AUD-3 (decisão do proprietário, 25/11/2026): sair da conta pela tela de Usuários.

- O Início não tem mais o botão "Trocar usuário".
- A tela de Usuários tem o botão "Sair".
- "Sair" encerra a sessão de quem está usando e volta à entrada, **sem fechar o
  Rustinho**: outra pessoa entra com o próprio usuário e o Início a cumprimenta pelo
  nome (cada venda fica no nome de quem fez).

    python3 app/testes/telas_r1_sair.py [pasta para as fotos]
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import TELA, Rustinho, conferir, montar_loja, simples, xdo  # noqa: E402


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r1-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r1-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "r1-inicio")
        try:
            r.entrar("Ana")
            time.sleep(2)
            tudo_certo &= conferir("início: sem o botão \"Trocar usuário\"", r.foto("-1"), r.textos(),
                                   tem=["Buscar peça:"], sem_igual=["Trocar usuário"])
        finally:
            r.fechar()

        r = Rustinho(pasta, "TelaUsuarios", {}, fotos, "r1-usuarios")
        try:
            r.entrar("Ana")
            time.sleep(2)
            tudo_certo &= conferir("usuários: tem o botão \"Sair\"", r.foto("-1"), r.textos(), igual=["Sair"])
            if not r.clicar("Sair"):
                raise RuntimeError(f"não achei o botão Sair (veja {r.saida})")
            time.sleep(2)
            # Volta à entrada sem fechar o programa; outra pessoa entra com o próprio usuário.
            r.entrar("Bia")
            time.sleep(2)
            textos = r.textos()
            # O roteiro abre Usuários a cada entrada (--abrir): a Bia, caixa, vê Usuários só com o Sair (decisão 1);
            # o cumprimento pelo nome no Início está no caso da Bia, mais abaixo (a Ana volta e é cumprimentada).
            entrou_bia = any(t == "Para trocar de pessoa no caixa, toque em Sair." for _, _, t in textos)
            foto = r.foto("-2-bia")
            print(("✔ " if entrou_bia else "✘ ")
                  + "sair: a Bia entra no mesmo Rustinho (Usuários só com o Sair, sem a senha do gerente)"
                  + ("" if entrou_bia else f" (foto: {foto})"))
            tudo_certo &= entrou_bia
            vivo = r.p.poll() is None
            print(("✔ " if vivo else "✘ ") + "sair: o Rustinho não fechou")
            tudo_certo &= vivo
        finally:
            r.fechar()
        # Decisão 1 do proprietário (25/11/2026): a Bia (caixa) abre Usuários pelo Início sem a senha do gerente e vê
        # só o "Sair" (sem a lista nem a ficha); mexer nos usuários continua pedindo a senha.
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "r1-bia")
        try:
            r.entrar("Bia")
            time.sleep(2)
            if not r.clicar("Usuários"):
                raise RuntimeError(f"não achei o ladrilho Usuários no Início (veja {r.saida})")
            time.sleep(2)
            textos = r.textos()
            tudo_certo &= conferir("bia: Usuários abre sem a senha e mostra só o Sair", r.foto("-1"), textos,
                                   igual=["Sair", "Mexer nos usuários (senha do gerente)"],
                                   tem=["Para trocar de pessoa no caixa, toque em Sair."],
                                   sem_igual=["Salvar", "Senha do gerente", "Ana"])
            # B3 da revisão R6: o F2 (gravar) não faz nada no modo "só Sair": nenhum pedido de senha aparece.
            xdo("key", "F2")
            time.sleep(1.5)
            tudo_certo &= conferir("bia: o F2 não pede a senha do gerente (não há ficha para gravar)", r.foto("-f2"),
                                   r.textos(), igual=["Sair"], sem_igual=["Senha do gerente"])
            if not r.clicar("Sair"):
                raise RuntimeError(f"não achei o botão Sair (veja {r.saida})")
            time.sleep(2)
            r.entrar("Ana")
            time.sleep(2)
            textos = r.textos()
            ok = any(simples(t).endswith(", ana") for _, _, t in textos)
            print(("✔ " if ok else "✘ ") + "bia: o Sair volta à entrada, e a Ana entra"
                  + ("" if ok else f" (foto: {r.foto('-2-ana')})"))
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
