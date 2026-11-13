#!/usr/bin/env python3
"""Roteiro da impressão direta que falha (dev.3.3, J06).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros roteiros, com a loja de teste estável (montada pelo núcleo). A loja tem
a impressão direta ligada ("Imprimir direto na impressora padrão") e nenhuma
impressora: o programa roda sem `lp` ou com o CUPS apontando para um lugar
que não existe (`CUPS_SERVER`), então nada sai em impressora nenhuma.

Confere, na tela de Estoque (botão "Imprimir lista"):

  I1 com a impressão direta ligada e a impressora ausente, a tela avisa que
     não imprimiu e que abriu o PDF ("Não imprimiu na impressora; abri o PDF.");
  I2 o PDF foi criado (o caminho que a ponte devolve existe);
  I3 com a impressão direta desligada, o PDF é o esperado: nenhum aviso de
     que não imprimiu;
  I4 nenhum TypeError/ReferenceError no registro.

    python3 app/testes/telas_impressao_falha.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
"""

import glob
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":75")
# Nenhuma impressora de verdade, em nenhum computador: o CUPS (se houver)
# aponta para um lugar que não existe.
os.environ["CUPS_SERVER"] = "/nao-existe/rustinho-teste/cups.sock"
os.environ.pop("PRINTER", None)
os.environ.pop("LPDEST", None)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA
AVISO = "Não imprimiu na impressora; abri o PDF."


def direto(pasta, ligado):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    c.execute("INSERT OR REPLACE INTO config (chave, valor) VALUES ('imprimir_direto', ?)", ("1" if ligado else "0",))
    c.commit()
    c.close()


def pdfs(pasta):
    return set(glob.glob(os.path.join(pasta, "**", "*.pdf"), recursive=True))


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-impressao-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-dados-impressao-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}
    info = [f"lp no sistema: {shutil.which('lp') or 'não há'}; CUPS_SERVER={os.environ['CUPS_SERVER']}"]

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)
        direto(pasta, True)
        antes = pdfs(pasta)
        rr = u.Rustinho(pasta, "TelaEstoque", {}, fotos, "impressao_falha")
        try:
            for _ in range(150):
                s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                                   env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
                if s.stdout.split():
                    break
                time.sleep(0.2)
            time.sleep(0.5)
            rr.redimensionar(1920, 1080)
            time.sleep(1)
            rr.entrar("Ana")
            time.sleep(2.0)
            if not rr.clicar("Imprimir lista"):
                raise RuntimeError("não achei o botão Imprimir lista")
            time.sleep(1.5)
            t = rr.textos()
            rr.foto("-1-direto-sem-impressora")
            avisos = [tx for _, _, tx in t if "imprimiu" in tx.lower() or "impressora" in tx.lower()]
            novos = pdfs(pasta) - antes
            info.append(f"textos com “imprimiu”/“impressora” na tela: {avisos}")
            info.append(f"PDFs novos: {sorted(os.path.basename(p) for p in novos)}")
            conferir("I1", "impressão direta ligada e sem impressora: a tela avisa que não imprimiu e abriu o PDF",
                     AVISO in [tx for _, _, tx in t], f"textos {avisos}")
            conferir("I2", "o PDF foi criado", len(novos) == 1, f"{novos}")

            # Some sozinho (5,5 s); depois, com a impressão direta desligada.
            time.sleep(6.5)
            direto(pasta, False)
            antes = pdfs(pasta)
            if not rr.clicar("Imprimir lista"):
                raise RuntimeError("não achei o botão Imprimir lista (2ª vez)")
            time.sleep(1.5)
            t2 = rr.textos()
            rr.foto("-2-pdf-escolhido")
            avisos2 = [tx for _, _, tx in t2 if "imprimiu" in tx.lower()]
            conferir("I3", "impressão direta desligada: o PDF é o esperado, sem aviso de que não imprimiu",
                     not avisos2 and len(pdfs(pasta) - antes) <= 1, f"textos {avisos2}")
        finally:
            rr.fechar()
        with open(rr.saida, errors="replace") as f:
            erros = [ln.strip() for ln in f if re.search(r"TypeError|ReferenceError", ln)]
        conferir("I4", "nenhum TypeError/ReferenceError no registro", not erros, f"{erros[:3]}")
    finally:
        shutil.rmtree(pasta, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()

    for linha in info:
        print(f"  · {linha}")
    reprovado = False
    for codigo in sorted(resultados, key=lambda c: int(c[1:])):
        certo, nome, detalhe = resultados[codigo]
        if certo:
            print(f"  ✔ {codigo} {nome}")
        else:
            print(f"  ✘ {codigo} {nome} — {detalhe}")
            reprovado = True
    certas = sum(1 for c in resultados.values() if c[0])
    print(f"\n{certas} de {len(resultados)} conferências certas; fotos em {fotos}")
    sys.exit(1 if reprovado or len(resultados) != 4 else 0)


if __name__ == "__main__":
    main()
