#!/usr/bin/env python3
"""Roteiro da lista contínua de Peças (dev.3.3, passo 7.5 — N04).

Abre o Rustinho de verdade numa tela virtual 1920 × 1080 (Xvfb), no ambiente
neutro dos outros roteiros, com uma loja de 20.000 peças montada PELO NÚCLEO
(`nucleo/tests/lojas_de_teste.rs::montar_lista_continua`, pelo comando da tela
`produto_salvar`; em release, porque são 20.000). Nada de SQL direto na loja.

Confere a N04 (requisito do proprietário de 10/11/2026: lista contínua, sem
paginação):

  C1 a lista abre com a primeira parte e o cabeçalho diz "20000 peças cadastradas";
  C2 nenhum "Anterior", "Próxima", "Página" nem "carregar mais" na tela;
  C3 descendo com PageDown, as linhas aparecem na ordem da lista inteira, cada
     peça uma vez, sem buraco (comparadas com a ordem lida do banco, só para
     leitura, pela coluna gravada `nome_normalizado`);
  C4 rolando até o fim (PageDown), as partes vêm sozinhas: a lista chega às
     20.000 peças, todas diferentes, sem mais nada para vir;
  C5 no fim, as últimas linhas da tela são as últimas peças da lista;
  C6 PageUp volta 8 linhas por vez, sem página;
  C7 a busca vale sobre todas: "pastilha lista" traz as 4.000 pastilhas, e a
     rolagem passa da primeira parte só com elas.

A memória do programa (VmRSS e VmHWM do processo) antes e depois de rolar até o
fim vai para o resultado (informação; a medida que vale é a do POCO C85, L6).

    python3 app/testes/telas_lista_continua.py [pasta para as fotos]

RUSTINHO_LOJA_PECAS=<n> muda o número de peças (só para ensaio; o critério da
N04 é com 20.000). Precisa de: Xvfb, xdotool, ImageMagick (import), cargo.
Sai com 0 se aprovado.
"""

import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":79")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA
PECAS = int(os.environ.get("RUSTINHO_LOJA_PECAS", "20000"))


def montar_loja(pasta):
    """A loja pelo núcleo (os mesmos comandos das telas), em release."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_LOJA_PECAS=str(PECAS))
    inicio = time.time()
    subprocess.run(
        ["cargo", "test", "--release", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_lista_continua"],
        cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )
    return time.time() - inicio


def ordem_do_banco(pasta):
    """Os nomes na ordem da lista "alteradas por último", lidos do banco só
    para leitura (o SQLite de fora lê a coluna gravada, sem a função do
    Rustinho: B04)."""
    c = sqlite3.connect(f"file:{os.path.join(pasta, 'rustinho.db')}?mode=ro", uri=True)
    try:
        return [n for (n,) in c.execute(
            "SELECT nome FROM produto WHERE ativo = 1 "
            "ORDER BY atualizado_em DESC, nome_normalizado ASC, codigo ASC, id ASC")]
    finally:
        c.close()


def marcador(textos):
    """(linhas na lista, peças diferentes, ainda há mais, linha atual)."""
    for _, _, t in textos:
        if t.startswith("TESTE_LISTA:"):
            n, dif, mais, atual = t.split(":")[1:5]
            return int(n), int(dif), mais == "1", int(atual)
    return None


def nomes_na_tela(textos):
    """Os nomes das peças visíveis, de cima para baixo."""
    geos = {(x, y) for x, y, t in textos if t.startswith("GEO:nomePeca:")}
    return [t for x, y, t in sorted(textos, key=lambda v: v[1])
            if (x, y) in geos and not t.startswith(("GEO:", "RET:"))]


def memoria(pid):
    campos = {}
    with open(f"/proc/{pid}/status") as f:
        for linha in f:
            if linha.startswith(("VmRSS:", "VmHWM:")):
                k, v = linha.split(":", 1)
                campos[k] = int(v.split()[0])
    return campos


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-lista-continua-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-lista-continua-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}
    info = []

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        info.append(f"loja de {PECAS} peças montada pelo núcleo em {montar_loja(modelo):.0f} s")
        esperado = ordem_do_banco(modelo)
        info.append(f"ordem esperada lida do banco: {len(esperado)} peças")
        rr = u.Rustinho(modelo, "TelaPecas", {}, fotos, "lista_continua")
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
            time.sleep(3.0)
            rr.foto("-1-lista")
            t = rr.textos()
            m = marcador(t)
            cabecalho = [tx for _, _, tx in t if "peças cadastradas" in tx]
            conferir("C1", f"a lista abre com a primeira parte e o cabeçalho diz \"{PECAS} peças cadastradas\"",
                     m is not None and m[0] == 100 and m[2] and f"{PECAS} peças cadastradas" in cabecalho,
                     f"marcador {m}, cabeçalho {cabecalho}")

            def sem_pagina(textos):
                proibidos = [tx for _, _, tx in textos if not tx.startswith(("GEO:", "RET:", "TOQUE:"))
                             and (re.search(r"\bP[áa]gina\b", tx) or tx.strip() in ("Anterior", "Próxima")
                                  or "carregar mais" in tx.lower() or "mostrar mais" in tx.lower())]
                return proibidos

            proibidos = sem_pagina(t)

            # C3: PageDown 40 vezes, lendo a tela a cada vez.
            seq = []
            problemas = []
            for passo in range(41):
                if passo:
                    base.xdo("key", "Next")
                    time.sleep(0.35)
                tt = rr.textos()
                proibidos += sem_pagina(tt)
                vis = nomes_na_tela(tt)
                if len(set(vis)) != len(vis):
                    problemas.append(f"passo {passo}: nome repetido na tela {vis}")
                vistos = set(seq)
                j = next((k for k, n in enumerate(vis) if n not in vistos), len(vis))
                if any(n in vistos for n in vis[j:]):
                    problemas.append(f"passo {passo}: linha já vista depois de uma nova {vis}")
                if j and seq[-j:] != vis[:j]:
                    problemas.append(f"passo {passo}: a tela não continua a anterior {vis[:j]} × {seq[-j:]}")
                seq += vis[j:]
            conferir("C2", "nenhum \"Anterior\", \"Próxima\", \"Página\" nem \"carregar mais\"", not proibidos,
                     f"{proibidos[:5]}")
            conferir("C3", "PageDown mostra a lista na ordem, cada peça uma vez, sem buraco",
                     not problemas and len(seq) >= 300 and seq == esperado[:len(seq)],
                     f"{len(seq)} linhas lidas; problemas {problemas[:2]}; "
                     f"primeira diferença {next((k for k, (a, b) in enumerate(zip(seq, esperado)) if a != b), None)}")
            rr.foto("-2-depois-de-320-linhas")

            # C4: até o fim, pelo teclado.
            antes = memoria(rr.p.pid)
            inicio = time.time()
            m = marcador(rr.textos())
            while m and (m[2] or m[3] < m[0] - 1) and time.time() - inicio < 900:
                base.xdo("key", "--repeat", "200", "--delay", "8", "Next")
                time.sleep(1.0)
                m = marcador(rr.textos())
            gasto = time.time() - inicio
            depois = memoria(rr.p.pid)
            info.append(f"rolar até o fim pelo teclado: {gasto:.0f} s; marcador {m}")
            info.append(f"memória do programa (kB): antes {antes}, depois {depois}")
            conferir("C4", f"rolando até o fim, as partes vêm sozinhas e a lista chega às {PECAS} peças, todas diferentes",
                     m is not None and m[0] == PECAS and m[1] == PECAS and not m[2] and m[3] == PECAS - 1,
                     f"marcador {m} em {gasto:.0f} s")
            rr.foto("-3-fim")
            tf = rr.textos()
            vis = nomes_na_tela(tf)
            conferir("C5", "no fim, as últimas linhas da tela são as últimas peças da lista",
                     vis and vis == esperado[-len(vis):] and vis[-1] == esperado[-1], f"tela {vis[-3:]} × {esperado[-3:]}")

            base.xdo("key", "Prior")
            time.sleep(0.4)
            base.xdo("key", "Prior")
            time.sleep(0.4)
            base.xdo("key", "Prior")
            time.sleep(0.8)
            tp = rr.textos()
            m2 = marcador(tp)
            conferir("C6", "PageUp volta 8 linhas por vez, sem página",
                     m2 is not None and m2[3] == PECAS - 1 - 24 and not sem_pagina(tp), f"marcador {m2}")

            # C7: a busca vale sobre todas.
            g = u.geos(tp)
            rr.clicar_em(g["campoDeBusca"])
            base.xdo("key", "ctrl+a")
            base.xdo("type", "--delay", "50", "pastilha lista")
            time.sleep(3.0)
            for _ in range(16):
                base.xdo("key", "Next")
                time.sleep(0.3)
            time.sleep(1.0)
            tb = rr.textos()
            mb = marcador(tb)
            vb = nomes_na_tela(tb)
            rr.foto("-4-busca")
            conferir("C7", "a busca vale sobre todas e a rolagem passa da primeira parte só com elas",
                     mb is not None and mb[0] > 100 and mb[1] == mb[0] and vb
                     and all("PASTILHA LISTA" in n for n in vb) and not sem_pagina(tb),
                     f"marcador {mb}; tela {vb[:2]}")
        finally:
            rr.fechar()
    finally:
        shutil.rmtree(modelo, ignore_errors=True)
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
    sys.exit(1 if reprovado or len(resultados) != 7 else 0)


if __name__ == "__main__":
    main()
