#!/usr/bin/env python3
"""Teste da quantidade das etiquetas (0.9.18, B8B1-09 da auditoria).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros testes das telas, em Etiquetas com a peça "Filtro de óleo", e confere:

  Q1. Digitar "12" na quantidade fica "12". (A perda de foco que a
      auditoria descreve não se repetiu aqui nem com o código antigo, no Qt
      do computador: este item confere que digitar funciona, não prova a
      correção. A quantidade passou a mudar no próprio item, sem recriar a
      lista a cada tecla, o que também evita fechar o teclado do celular.)
  Q2. O total acompanha ("12 etiquetas").
  Q3. Com a quantidade 0, o F2 não faz nada (como o botão "Imprimir", que
      fica desligado). Com o atalho antigo, o F2 mostrava "Escolha as peças
      e quantas etiquetas de cada." (este item falha no código antigo).
  Q4. Nenhum TypeError/ReferenceError nos registros.

    python3 app/testes/telas_etiquetas_quantidade.py [pasta para as fotos]

Precisa de: Xvfb, xdotool e ImageMagick (import). Sai com 0 se tudo conferiu.
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":77")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(u.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({u.PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-etiquetas-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-etiquetas-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    res = tema.Resultado()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(modelo)
        c = sqlite3.connect(os.path.join(modelo, "rustinho.db"))
        filtro = c.execute("SELECT id FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0]
        c.close()
        rr = u.Rustinho(modelo, "TelaEtiquetas", {"produtoId": filtro}, fotos, "etiquetas")
        try:
            rr.entrar("Ana")
            time.sleep(2.5)
            g = u.geos(rr.textos())
            caixa = g.get("campoQuantidadeEtiquetaCaixa")
            if not caixa:
                res.conferir("a linha da peça com a quantidade apareceu", False, rr.saida)
                return
            rr.clicar_em(caixa)
            base.xdo("key", "ctrl+a")
            base.xdo("type", "--delay", "120", "12")
            time.sleep(0.8)
            textos = rr.textos()
            g = u.geos(textos)
            caixa = g.get("campoQuantidadeEtiquetaCaixa")
            dentro = [t for x, y, t in textos if caixa and u.dentro(x, y, caixa)
                      and not t.startswith(("GEO:", "RET:", "TOQUE:"))]
            foto = rr.foto("-12")
            res.conferir("Q1 digitar \"12\" fica \"12\" (o campo não perde o foco no meio)", "12" in dentro,
                         f"no campo: {dentro}; foto {foto}")
            todos = [t for _, _, t in textos]
            res.conferir("Q2 o total acompanha (12 etiquetas)", any(t.startswith("12 etiquetas") for t in todos),
                         f"{[t for t in todos if 'etiqueta' in t][:4]}")
            # Quantidade 0: imprimir desligado e F2 sem efeito.
            base.xdo("key", "ctrl+a")
            base.xdo("type", "--delay", "80", "0")
            time.sleep(0.8)
            antes = [t for _, _, t in rr.textos() if not t.startswith(("GEO:", "RET:", "TOQUE:"))]
            base.xdo("key", "F2")
            time.sleep(2)
            depois = [t for _, _, t in rr.textos() if not t.startswith(("GEO:", "RET:", "TOQUE:"))]
            foto = rr.foto("-zero-f2")
            novos = [t for t in depois if t not in antes]
            res.conferir("Q3 com 0 etiqueta, o F2 não imprime (nada novo na tela)", not novos and "Imprimir" in antes,
                         f"novos: {novos[:5]}; foto {foto}")
        finally:
            rr.fechar()
        erros = [f for f in os.listdir(fotos) if f.endswith(".log")
                 and any(e in open(os.path.join(fotos, f), errors="replace").read() for e in ("TypeError", "ReferenceError"))]
        res.conferir("Q4 nenhum TypeError/ReferenceError nos registros", not erros, ", ".join(erros))
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(modelo, ignore_errors=True)
    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha. Fotos em {fotos}")
    for f in res.falhas:
        print("  ✘ " + f)
    sys.exit(0 if not res.falhas else 1)


if __name__ == "__main__":
    main()
