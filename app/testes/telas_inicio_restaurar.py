#!/usr/bin/env python3
"""T008 da 008 (E7-7; PEND-30 (a), decisão do proprietário de 26/11/2026): o Rustinho reinstalado oferece restaurar a
loja na primeira tela.

- REINSTALADO: a loja de teste com uma cópia de backup na pasta dela; o banco é apagado (a pasta de backup fica).
  A primeira tela mostra "Restaurar a loja" com a cópia; com a senha de um gerente da cópia, a loja volta e a tela de
  entrada aparece, com a Ana.
- CONTROLE: uma pasta sem cópia nenhuma: a faixa "Restaurar a loja" NÃO aparece (o texto não é fixo).

    python3 app/testes/telas_inicio_restaurar.py [pasta das fotos]
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_navegacao_casos import Placar, montar_loja, cargo_teste, xdo, SENHA, TELA  # noqa: E402

FAIXA = "Restaurar a loja a partir da cópia"


def reinstalado(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-inicio-restaurar-")
    try:
        montar_loja(pasta)
        cargo_teste({"RUSTINHO_LOJA_TESTE": pasta}, "fazer_backup_por_fora")
        for sufixo in ("", "-wal", "-shm"):
            caminho = os.path.join(pasta, "rustinho.db" + sufixo)
            if os.path.exists(caminho):
                os.remove(caminho)
        r = placar.abrir(pasta, "TelaBoasVindas", {}, fotos, "reinstalado")
        try:
            achou = r.esperar(lambda: r.tem("Encontrei cópias da loja"), 15)
            if not ok("a primeira tela oferece restaurar a loja", achou and r.tem(FAIXA), r.foto("-1")):
                return
            r.clicar(FAIXA)
            if not ok("a janelinha pede a senha de um gerente", r.esperar(lambda: r.tem("Senha de um gerente"), 8),
                      r.foto("-2")):
                return
            time.sleep(0.6)
            xdo("type", "--delay", "40", SENHA["Ana"])
            time.sleep(0.3)
            xdo("key", "Return")
            ok("restaurada, a tela de entrada aparece com a Ana",
               r.esperar(lambda: r.tem("Quem está usando?") and r.tem("Ana"), 30), r.foto("-3"))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def sem_copia(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-inicio-sem-copia-")
    try:
        r = placar.abrir(pasta, "TelaBoasVindas", {}, fotos, "sem_copia")
        try:
            abriu = r.esperar(lambda: r.tem("Começar"), 15)
            f = r.foto("-1")
            if ok("CONTROLE: a primeira tela abriu", abriu, f):
                ok("CONTROLE: sem cópia na pasta, não oferece restaurar", not r.tem(FAIXA), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-inicio-restaurar-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        for f in (sem_copia, reinstalado):
            try:
                f(placar, fotos)
            except Exception as e:
                placar.ok(f"{f.__name__} chegou ao fim ({e!r})", False)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
