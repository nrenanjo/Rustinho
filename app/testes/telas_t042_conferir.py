#!/usr/bin/env python3
"""T042 (barras de pesquisa em cápsula): confere as fotos de `telas_t042_capturas.py`.

A posição de cada barra vem do próprio programa (linhas GEO:campoDeBusca / GEO:buscaDaInicio do registro da foto).

  estilo PASTA PREFIXO   em cada foto PREFIXO-<tema>-<tela>.png, a barra de pesquisa tem de estar como a referência
                         definitiva do dono (17/11/2026, 16h50):
                           E1. o fundo da barra (a cor mais comum no miolo dela) é o cinza-escuro #3F3B44 (±10), nos três temas;
                           E2. as pontas são de cápsula: o ponto a 15% da altura, perto do canto do retângulo, fica FORA da
                               barra (não é o fundo dela);
                           E3. o halo magenta: a 12 px à esquerda da barra, na altura do meio, a luz é magenta
                               (vermelho e azul pelo menos 40 acima do verde).
                         Com o programa de antes, tem de REPROVAR; com o de depois, passar.
  vizinhos PASTA         para cada par antes-/depois-: na faixa do halo (34 px em volta da barra, sem a barra e o anel de
                         foco antigo), o MIOLO sólido de qualquer outro componente (o pixel e os 8 vizinhos iguais no antes)
                         fica igual no depois (±24). O halo passa ATRÁS dos vizinhos, nunca por cima. A superfície onde a
                         barra está (a janela, ou o painel dela, a cor mais comum da faixa no antes) não é vizinho.

Sai com 0 se tudo conferiu.
"""
import re
import sys
from collections import Counter
from pathlib import Path

from PIL import Image

FUNDO_DA_BUSCA = (0x3F, 0x3B, 0x44)


def dist(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def barras(log):
    out = set()
    for l in Path(log).read_text(errors="replace").splitlines():
        m = re.search(r"GEO:(campoDeBusca|buscaDaInicio):(-?[\d.]+),(-?[\d.]+),([\d.]+),([\d.]+)", l)
        if m:
            out.add((m.group(1),) + tuple(round(float(v)) for v in m.groups()[1:]))
    return sorted(out)


def estilo(pasta, prefixo):
    ruins = conferidas = 0
    for foto in sorted(Path(pasta).glob(f"{prefixo}-*.png")):
        log = foto.with_suffix(".log")
        bs = barras(log) if log.exists() else []
        if not bs:
            print(f"{foto.name}: nenhuma barra de pesquisa no registro")
            continue
        im = Image.open(foto).convert("RGB")
        px = im.load()
        w, h = im.size
        for nome, x, y, bw, bh in bs:
            conferidas += 1
            miolo = Counter(px[xx, yy] for xx in range(x + 12, x + bw - 12, 3) for yy in range(y + 10, y + bh - 10, 2)
                            if 0 <= xx < w and 0 <= yy < h)
            fundo = miolo.most_common(1)[0][0] if miolo else (0, 0, 0)
            e1 = dist(fundo, FUNDO_DA_BUSCA) <= 10
            cx, cy = x + round(bh * 0.15), y + round(bh * 0.15)
            canto = px[cx, cy] if 0 <= cx < w and 0 <= cy < h else (0, 0, 0)
            e2 = dist(canto, fundo) > 30
            lx, ly = x - 12, y + bh // 2
            luz = px[lx, ly] if 0 <= lx < w else (0, 0, 0)
            e3 = luz[0] - luz[1] >= 40 and luz[2] - luz[1] >= 40
            ok = e1 and e2 and e3
            ruins += not ok
            print(f"{foto.name} [{nome} {x},{y},{bw}x{bh}]: E1 fundo {fundo} {'ok' if e1 else 'ERRADO'} · "
                  f"E2 canto {canto} {'ok' if e2 else 'ERRADO'} · E3 luz {luz} {'ok' if e3 else 'ERRADO'}")
    print(f"ESTILO ({prefixo}): {conferidas} barra(s), {ruins} fora da referência")
    return conferidas > 0 and ruins == 0


def vizinhos(pasta):
    ruins = conferidas = 0
    for dep in sorted(Path(pasta).glob("depois-*.png")):
        ant = dep.with_name(dep.name.replace("depois-", "antes-", 1))
        log = dep.with_suffix(".log")
        if not ant.exists() or not log.exists():
            continue
        A = Image.open(ant).convert("RGB")
        D = Image.open(dep).convert("RGB")
        w, h = A.size
        pa, pd = A.load(), D.load()
        fundo = Counter(pa[x, y] for x in range(0, w, 7) for y in range(0, h, 7)).most_common(1)[0][0]
        for nome, x, y, bw, bh in barras(log):
            conferidas += 1
            m, marcados = 34, []
            # a superfície onde a barra está (a janela, ou um painel, como no Histórico): a cor mais comum da faixa em
            # volta da barra, no antes. O halo sobre ela é o desenho pedido; sobre outro componente, não.
            faixa = Counter(pa[xx, yy] for yy in range(max(0, y - m), min(h, y + bh + m), 2) for xx in range(max(0, x - m), min(w, x + bw + m), 2)
                            if not (x - 5 <= xx <= x + bw + 4 and y - 5 <= yy <= y + bh + 4))
            superficie = faixa.most_common(1)[0][0] if faixa else fundo
            for yy in range(max(1, y - m), min(h - 1, y + bh + m)):
                for xx in range(max(1, x - m), min(w - 1, x + bw + m)):
                    if x - 5 <= xx <= x + bw + 4 and y - 5 <= yy <= y + bh + 4:
                        continue  # a barra e o anel de foco da barra antiga (4 px)
                    c = pa[xx, yy]
                    if dist(c, fundo) <= 30 or dist(c, superficie) <= 30:
                        continue
                    if any(dist(pa[xx + i, yy + j], c) > 6 for i in (-1, 0, 1) for j in (-1, 0, 1)):
                        continue  # borda suavizada ou texto fino: muda quando a luz passa ATRÁS, o que é certo
                    if dist(c, pd[xx, yy]) > 24:
                        marcados.append((xx, yy))
            ok = len(marcados) <= 20
            ruins += not ok
            onde = ""
            if marcados:
                xs, ys = sorted(p[0] for p in marcados), sorted(p[1] for p in marcados)
                onde = f" (x de {xs[0]} a {xs[-1]}, y de {ys[0]} a {ys[-1]})"
            print(f"{dep.name} [{nome} {x},{y},{bw}x{bh}]: {len(marcados)} pixel(s) de vizinho mudados{onde} → "
                  f"{'ok' if ok else 'O HALO PINTA POR CIMA'}")
    print(f"VIZINHOS: {conferidas} barra(s), {ruins} com o halo por cima de outro componente")
    return conferidas > 0 and ruins == 0


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "estilo":
        sys.exit(0 if estilo(sys.argv[2], sys.argv[3]) else 1)
    if len(sys.argv) >= 3 and sys.argv[1] == "vizinhos":
        sys.exit(0 if vizinhos(sys.argv[2]) else 1)
    sys.exit(__doc__)
