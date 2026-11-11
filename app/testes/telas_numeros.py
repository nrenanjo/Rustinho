#!/usr/bin/env python3
"""Roteiro dos números das telas (dev.3.3, passo 2.2: H13 e ARQ-09).

Abre o Rustinho de verdade numa tela virtual 1920 × 1080 (Xvfb), no ambiente
neutro dos outros roteiros, com uma loja montada PELO NÚCLEO
(`nucleo/tests/lojas_de_teste.rs::montar_numeros_das_telas`): Ana (gerente), o
caixa aberto e uma peça com o código de barras 7891000000014.

  V1  Vender: "1.5*7891000000014" e depois "1*7891000000014" deixam a peça
      com 2,5 no carrinho (a tela lia o ponto como milhar: 15 + 1 = 16);
  C1  Configurações › Crediário: multa "2.5" e juros "1" dão o exemplo da
      conta da cobrança, "R$ 102,83" com "R$ 2,83 de multa e juros" (a tela
      lia 25% de multa e mostrava R$ 125,33).

Antes da correção (telas de 4b253c6): V1 e C1 falham.

    python3 app/testes/telas_numeros.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":87")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA
CODIGO = "7891000000014"


def montar_loja(pasta):
    """A loja pelo núcleo (os mesmos comandos das telas)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_numeros_das_telas"],
        cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def esperar_janela():
    for _ in range(150):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        if s.stdout.split():
            break
        time.sleep(0.2)
    time.sleep(0.5)


def so_textos(t):
    return [tx for _, _, tx in t if not tx.startswith(("GEO:", "RET:", "TOQUE:", "TESTE_PILHA:"))]


def abrir(modelo, tela, props, fotos, nome):
    rr = u.Rustinho(modelo, tela, props, fotos, nome)
    esperar_janela()
    rr.redimensionar(1920, 1080)
    time.sleep(1)
    rr.entrar("Ana")
    time.sleep(3.0)
    return rr


def campos_com_dica(t, dica):
    """Os campos de texto com essa dica, da esquerda para a direita: [(x, y, w, h)]."""
    achados = []
    for _, _, tx in t:
        if tx.startswith("TOQUE:campo|"):
            partes = tx.split("|")
            if len(partes) >= 5 and partes[3] == dica:
                x, y, w, h = (float(v) for v in partes[4].split(","))
                achados.append((x, y, w, h))
    return sorted(achados)


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-numeros-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-numeros-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        montar_loja(modelo)

        # ---------- H13: a soma do carrinho ----------
        rr = abrir(modelo, "TelaVender", {}, fotos, "vender")
        try:
            g = u.geos(rr.textos())
            for texto in (f"1.5*{CODIGO}", f"1*{CODIGO}"):
                rr.clicar_em(g["campoDeBusca"])
                base.xdo("key", "ctrl+a")
                base.xdo("type", "--delay", "50", texto)
                base.xdo("key", "Return")
                time.sleep(2.0)
            rr.foto("-1-carrinho")
            t = so_textos(rr.textos())
            quantidades = [tx for tx in t if tx in ("2,5", "2,500", "16", "2.5")]
            conferir("V1", "\"1.5*código\" + \"1*código\" = 2,5 no carrinho",
                     "2,5" in quantidades and "16" not in quantidades, f"vistos: {quantidades}")
        finally:
            rr.fechar()

        # ---------- ARQ-09: o exemplo de atraso do crediário ----------
        rr = abrir(modelo, "TelaConfiguracoes", {"secao": "receber"}, fotos, "crediario")
        try:
            zeros = campos_com_dica(rr.textos(), "0")
            if len(zeros) < 2:
                conferir("C1", "o exemplo de atraso é a conta da cobrança", False, f"campos com dica 0: {zeros}")
            else:
                for (x, y, w, h), valor in zip(zeros[:2], ("2.5", "1")):
                    rr.clicar_em((x, y, w, h))
                    base.xdo("key", "ctrl+a")
                    base.xdo("type", "--delay", "50", valor)
                    time.sleep(0.5)
                time.sleep(1.5)
                rr.foto("-2-exemplo")
                exemplo = next((tx for tx in so_textos(rr.textos()) if tx.startswith("Exemplo:")), "")
                conferir("C1", "multa 2.5% e juros 1% dão R$ 102,83 (R$ 2,83 de multa e juros)",
                         "R$ 102,83" in exemplo and "R$ 2,83 de multa e juros" in exemplo, f"{exemplo!r}")
        finally:
            rr.fechar()
    finally:
        xvfb.terminate()

    falhas = 0
    for codigo in sorted(resultados):
        certo, nome, detalhe = resultados[codigo]
        print(f"{'✔' if certo else '✘'} {codigo} {nome}" + (f" · {detalhe}" if detalhe else ""))
        falhas += 0 if certo else 1
    print(f"fotos em {fotos}")
    sys.exit(1 if falhas else 0)


if __name__ == "__main__":
    main()
