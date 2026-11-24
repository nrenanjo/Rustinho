#!/usr/bin/env python3
"""Prova da correção do BUG-20261124-JNAP, variante local (24/11/2026).

A conferência "o Rustinho do teste está na tela virtual, sem Wayland" lia
`/proc/<pid>/environ` logo depois de criar o processo. Antes do `exec`, esse
arquivo ainda é o do Python que criou o processo: com o `DISPLAY=:0` da sessão de
quem roda a bateria, ela dizia na hora "não está na tela virtual" e o roteiro
parava (mais vezes com a máquina carregada).

Aqui um processo espera 1,5 s antes de virar o "programa" (uma cópia do `sleep`)
com o DISPLAY da tela virtual. Casos:
  A1  o processo vira o programa com a tela virtual: a conferência diz SIM
      (a conferência de antes dizia NÃO, lendo o ambiente do pai);
  A2  o processo vira o programa com outra tela: NÃO;
  A3  o processo termina sem virar o programa: NÃO, sem esperar além do prazo.

Não abre janela nenhuma.

    python3 app/testes/conferir_ambiente_antes_do_exec.py
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

# O pai com a sessão de quem roda a bateria (o caso que reprovava).
os.environ["DISPLAY"] = ":0"
os.environ.pop("WAYLAND_DISPLAY", None)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402

VIRA = "import os, sys, time; time.sleep(1.5); os.execve(sys.argv[1], [sys.argv[1], '4'], {'DISPLAY': sys.argv[2]})"


def conferir(pid, programa):
    try:
        return nav.conferir_ambiente(pid, programa=programa)
    except TypeError:  # a conferência de antes não sabia esperar o programa
        return nav.conferir_ambiente(pid)


def caso(falso, codigo, *args):
    p = subprocess.Popen([sys.executable, "-c", codigo, *args])
    inicio = time.monotonic()
    try:
        return conferir(p.pid, falso), time.monotonic() - inicio
    finally:
        p.kill()
        p.wait()


def main():
    pasta = tempfile.mkdtemp(prefix="rustinho-conferir-ambiente-")
    falso = os.path.join(pasta, "rustinho-falso")
    shutil.copy("/usr/bin/sleep", falso)
    resultados = []
    try:
        sim, _ = caso(falso, VIRA, falso, nav.TELA)
        resultados.append(("A1", "o processo que vira o programa na tela virtual passa", sim is True))
        nao, _ = caso(falso, VIRA, falso, ":0")
        resultados.append(("A2", "o programa em outra tela não passa", nao is False))
        morto, demora = caso(falso, "import sys; sys.exit(0)")
        resultados.append(("A3", f"o processo que termina antes não passa nem prende ({demora:.1f} s)",
                           morto is False and demora < 6))
    finally:
        shutil.rmtree(pasta, ignore_errors=True)
    for codigo, nome, certo in resultados:
        print(f"  {'✔' if certo else '✘'} {codigo} {nome}")
    certas = sum(1 for *_, c in resultados if c)
    print(f"RESULTADO: {certas} de {len(resultados)} certas")
    sys.exit(0 if certas == len(resultados) else 1)


if __name__ == "__main__":
    main()
