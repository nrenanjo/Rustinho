#!/usr/bin/env python3
"""T042 (barras de pesquisa em cápsula): confere as fotos de `capturas_t042.py`. Não é roteiro (a CI roda os
telas_*.py sem argumentos de modo; por isso este nome).

A posição de cada barra vem do próprio programa (linhas GEO:campoDeBusca / GEO:buscaDaInicio da ÚLTIMA leitura da tela
no registro da foto). Na foto da janelinha "Qual é a peça?", só a barra da janelinha (a de trás fica sob o véu).

  estilo PASTA PREFIXO   em cada foto PREFIXO-<tema>-<tela>.png, a barra tem de estar como a referência definitiva do
                         dono (17/11/2026, 16h50):
                         A barra medida é o DESENHO (a cápsula, GEO:<nome>Caixa), não a área de toque (no celular, 48 dp,
                         mais alta que o desenho em barras de 40 px); sem essa linha, a área de toque.
                           E1. o fundo da barra (a cor mais comum no miolo) é o cinza-escuro #3F3B44 (±10), nos 3 temas;
                           E2. pontas de cápsula: o ponto a 15% da altura, perto do canto do retângulo, fica FORA da barra;
                           E3. o halo magenta em volta, sem corte: a "magentice" m = (R + B)/2 − G sobe, no MESMO ponto,
                               em relação à foto do par (antes ↔ depois, sem halo no antes), nas 8 direções (cima, baixo,
                               lados e os 4 cantos) a 20% do alcance (no mínimo 5 px, fora do anel de foco antigo), e dos
                               dois lados, em cima e embaixo também a 40%; à esquerda (a margem da tela), também a 66%.
                               O de 20% é sempre exigido; um ponto mais longe que, na foto do par, já cai em OUTRO
                               componente (o cartão logo abaixo da barra) e não mudou fica dispensado: o halo está atrás dele.
                               Alcance: 30 px; 14 na tela estreita (pelo tamanho da foto, com a regra do Tema.qml). Sem a foto do par, a referência é a superfície (a cor mais comum a 36–44 px).
                         Com o programa de antes, tem de REPROVAR; com o de depois, passar.
  vizinhos PASTA         para cada par antes-/depois-: a barra fica no mesmo lugar e com o mesmo tamanho (GEO); na faixa do halo (alcance + 4 px em volta da barra, sem a barra e o
                         anel de foco antigo), nenhum pixel de outro componente (texto inclusive) pode ter mudado "rumo
                         ao magenta a partir dele mesmo" (o halo POR CIMA). A mudança "rumo ao magenta a partir da
                         superfície" é o halo ATRÁS (certo: bordas suavizadas, vãos, superfícies translúcidas).

Sai com 0 se tudo conferiu.
"""
import math
import re
import sys
from collections import Counter
from pathlib import Path

from PIL import Image

FUNDO_DA_BUSCA = (0x3F, 0x3B, 0x44)
LUZ = (0xF8, 0x2E, 0xF2)  # Tema.buscaLuz


def dist(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def magentice(c):
    return (c[0] + c[2]) / 2 - c[1]


def _barras_do_bloco(bloco):
    out = set()
    for l in bloco.splitlines():
        m = re.search(r"GEO:(campoDeBusca|buscaDaInicio):(-?[\d.]+),(-?[\d.]+),([\d.]+),([\d.]+)", l)
        if m:
            out.add((m.group(1),) + tuple(round(float(v)) for v in m.groups()[1:]))
    return out


def barras(log, foto):
    blocos = Path(log).read_text(errors="replace").split("TEXTOS-DA-TELA-INICIO")
    out = _barras_do_bloco(blocos[-1])
    if "janelinha" in Path(foto).name:
        # só a barra da janelinha: tira as da tela de trás, as da última leitura ANTES de o conjunto de barras mudar
        # (anda das leituras finais para trás enquanto elas mostram as mesmas barras da última: a captura do telefone
        # lê de novo depois do clique, para ver se precisa rolar; e a primeira leitura pode ser de antes de a janela
        # virar telefone, com a barra de trás em outro lugar).
        conjuntos = [_barras_do_bloco(b) for b in blocos[1:]]
        i = len(conjuntos) - 1
        while i > 0 and conjuntos[i - 1] == out:
            i -= 1
        out = out - (conjuntos[i - 1] if i > 0 else set())
    return sorted(out)


def capsula(log, barra):
    """O desenho da barra (a cápsula), informado na MESMA leitura como GEO:<nome>Caixa, dentro da área de toque `barra`.
    No celular a área de toque tem pelo menos 48 dp e o desenho fica com a altura de sempre, no meio (Campo.qml, A15):
    o halo é em volta do desenho. Sem a linha (o programa de antes não a informa), a própria área de toque."""
    nome, x, y, bw, bh = barra
    bloco = Path(log).read_text(errors="replace").split("TEXTOS-DA-TELA-INICIO")[-1]
    for l in bloco.splitlines():
        m = re.search(rf"GEO:{nome}Caixa:(-?[\d.]+),(-?[\d.]+),([\d.]+),([\d.]+)", l)
        if m:
            cx, cy, cw, ch = (round(float(v)) for v in m.groups())
            if abs(cx - x) <= 1 and abs(cw - bw) <= 1 and y - 1 <= cy and cy + ch <= y + bh + 1:
                return (nome, cx, cy, cw, ch)
    return barra


def alcance(foto):
    """O alcance do halo pela tela da foto, com a regra do Tema.qml (estreito: largura < 760, ou altura < 560 com largura
    < 1100): 14 px na tela estreita, 30 nas outras. A foto é a da janela inteira."""
    w, h = Image.open(foto).size
    return 14 if (w < 760 or (h < 560 and w < 1100)) else 30


def superficie(px, w, h, x, y, bw, bh, r0, r1):
    cont = Counter()
    for yy in range(max(0, y - r1), min(h, y + bh + r1), 2):
        for xx in range(max(0, x - r1), min(w, x + bw + r1), 2):
            if not (x - r0 <= xx <= x + bw + r0 and y - r0 <= yy <= y + bh + r0):
                cont[px[xx, yy]] += 1
    return cont.most_common(1)[0][0] if cont else (0, 0, 0)


def estilo(pasta, prefixo):
    ruins = conferidas = 0
    for foto in sorted(Path(pasta).glob(f"{prefixo}-*.png")):
        log = foto.with_suffix(".log")
        bs = barras(log, foto) if log.exists() else []
        if not bs:
            print(f"{foto.name}: nenhuma barra de pesquisa no registro")
            ruins += 1
            continue
        im = Image.open(foto).convert("RGB")
        px = im.load()
        w, h = im.size
        A = alcance(foto)
        outro = foto.with_name(foto.name.replace("antes", "depois", 1) if foto.name.startswith("antes") else foto.name.replace("depois", "antes", 1))
        po = None
        if outro.exists():
            io = Image.open(outro).convert("RGB")
            po = io.load() if io.size == im.size else None
        for barra in bs:
            nome, x, y, bw, bh = capsula(log, barra)
            conferidas += 1
            miolo = Counter(px[xx, yy] for xx in range(x + 12, x + bw - 12, 3) for yy in range(y + 10, y + bh - 10, 2)
                            if 0 <= xx < w and 0 <= yy < h)
            fundo = miolo.most_common(1)[0][0] if miolo else (0, 0, 0)
            e1 = dist(fundo, FUNDO_DA_BUSCA) <= 10
            cx, cy = x + round(bh * 0.15), y + round(bh * 0.15)
            canto = px[cx, cy] if 0 <= cx < w and 0 <= cy < h else (0, 0, 0)
            e2 = dist(canto, fundo) > 30
            sup = superficie(px, w, h, x, y, bw, bh, A + 6, A + 14)
            r = bh / 2
            # (nome, ponto da borda da cápsula, direção para fora)
            d45 = math.sqrt(0.5)
            dirs = [("esq", (x, y + r), (-1, 0)), ("dir", (x + bw, y + r), (1, 0)),
                    ("cima", (x + bw / 2, y), (0, -1)), ("baixo", (x + bw / 2, y + bh), (0, 1)),
                    ("cantoNO", (x + r - r * d45, y + r - r * d45), (-d45, -d45)), ("cantoNE", (x + bw - r + r * d45, y + r - r * d45), (d45, -d45)),
                    ("cantoSO", (x + r - r * d45, y + r + r * d45), (-d45, d45)), ("cantoSE", (x + bw - r + r * d45, y + r + r * d45), (d45, d45))]
            exigido = {"esq": (0.2, 0.4, 0.66), "dir": (0.2, 0.4), "cima": (0.2, 0.4), "baixo": (0.2, 0.4)}
            minimo = {0.2: 20, 0.4: 8, 0.66: 2}
            faltas, atras = [], []
            for nd, (bx, by), (dx, dy) in dirs:
                perto = None  # o pixel do par no ponto de 20% desta direção (no fundo)
                for f in exigido.get(nd, (0.2,)):
                    d = max(5, round(A * f))
                    qx, qy = round(bx + dx * d), round(by + dy * d)
                    if not (0 <= qx < w and 0 <= qy < h):
                        continue
                    ref = (qx, qy)
                    if x - 5 <= qx <= x + bw + 4 and y - 5 <= qy <= y + bh + 4:
                        # o ponto fica dentro do retângulo da barra ANTIGA (os cantos da cápsula): no par, a referência é o
                        # fundo logo fora dele, no mesmo canto
                        ref = (x - 7 if dx < 0 else x + bw + 6, y - 7 if dy < 0 else y + bh + 6)
                    if f == 0.2 and po is not None:
                        perto = po[ref]
                    # Mais longe que 20%: se no par o ponto já é OUTRO componente (cor diferente da do ponto de 20%, que é
                    # o fundo) e o pixel não mudou, o halo está ATRÁS dele, como deve (o cartão ou a linha logo abaixo da
                    # barra). O de 20% é sempre exigido; o halo por cima é a conferência dos vizinhos.
                    if (f > 0.2 and po is not None and perto is not None and dist(po[ref], perto) > 6
                            and dist(px[qx, qy], po[ref]) <= 6):
                        atras.append(f"{nd}@{d}px")
                        continue
                    subida = magentice(px[qx, qy]) - magentice(po[ref] if po is not None else sup)
                    if subida < minimo[f]:
                        faltas.append(f"{nd}@{d}px:{subida:+.0f}")
            e3 = not faltas
            ok = e1 and e2 and e3
            ruins += not ok
            desenho = "" if (nome, x, y, bw, bh) == barra else f" (área de toque {barra[1]},{barra[2]},{barra[3]}x{barra[4]})"
            print(f"{foto.name} [{nome} {x},{y},{bw}x{bh}{desenho}]: E1 fundo {fundo} {'ok' if e1 else 'ERRADO'} · "
                  f"E2 canto {canto} {'ok' if e2 else 'ERRADO'} · E3 halo {'ok' if e3 else 'FALTA ' + ' '.join(faltas[:6])}"
                  f"{' (atrás de outro componente: ' + ' '.join(atras) + ')' if atras else ''} (superfície {sup})")
    print(f"ESTILO ({prefixo}): {conferidas} barra(s), {ruins} fora da referência")
    return conferidas > 0 and ruins == 0


def cos(a, b):
    na = math.sqrt(sum(v * v for v in a))
    nb = math.sqrt(sum(v * v for v in b))
    return sum(p * q for p, q in zip(a, b)) / (na * nb) if na and nb else 0.0


def vizinhos(pasta):
    ruins = conferidas = 0
    for dep in sorted(Path(pasta).glob("depois*.png")):
        ant = dep.with_name(dep.name.replace("depois", "antes", 1))
        log = dep.with_suffix(".log")
        if not ant.exists() or not log.exists():
            continue
        A = Image.open(ant).convert("RGB")
        D = Image.open(dep).convert("RGB")
        if A.size != D.size:
            print(f"{dep.name}: tamanho diferente do antes")
            ruins += 1
            continue
        w, h = A.size
        pa, pd = A.load(), D.load()
        m = alcance(dep) + 4
        geo_a, geo_d = barras(ant.with_suffix(".log"), ant) if ant.with_suffix(".log").exists() else None, barras(log, dep)
        if geo_a is not None and geo_a != geo_d:  # a posição e o tamanho da barra não mudam (o pedido do dono)
            print(f"{dep.name}: a barra MUDOU de lugar ou de tamanho: antes {geo_a}, depois {geo_d}")
            ruins += 1
        janela = Counter(pa[xx, yy] for xx in range(0, w, 7) for yy in range(0, h, 7)).most_common(1)[0][0]
        for nome, x, y, bw, bh in barras(log, dep):
            conferidas += 1
            sup = superficie(pa, w, h, x, y, bw, bh, 5, m)
            marcados = []
            for yy in range(max(0, y - m), min(h, y + bh + m)):
                for xx in range(max(0, x - m), min(w, x + bw + m)):
                    if x - 5 <= xx <= x + bw + 4 and y - 5 <= yy <= y + bh + 4:
                        continue  # a barra e o anel de foco da barra antiga (4 px)
                    c, n = pa[xx, yy], pd[xx, yy]
                    if dist(c, sup) <= 6 or dist(c, janela) <= 6 or dist(c, n) <= 10:  # a superfície da barra e o fundo da janela
                        continue
                    delta = tuple(n[i] - c[i] for i in range(3))
                    por_cima = cos(delta, tuple(LUZ[i] - c[i] for i in range(3)))
                    atras = cos(delta, tuple(LUZ[i] - sup[i] for i in range(3)))
                    # o halo sempre deixa o pixel mais magenta; texto que mudou de verdade (a hora no Histórico) não
                    if por_cima > 0.8 and por_cima > atras + 0.1 and magentice(n) - magentice(c) >= 8:
                        marcados.append((xx, yy))
            ok = len(marcados) <= 20
            ruins += not ok
            onde = ""
            if marcados:
                xs, ys = sorted(p[0] for p in marcados), sorted(p[1] for p in marcados)
                onde = f" (x de {xs[0]} a {xs[-1]}, y de {ys[0]} a {ys[-1]})"
            print(f"{dep.name} [{nome} {x},{y},{bw}x{bh}]: {len(marcados)} pixel(s) com o halo por cima{onde} → "
                  f"{'ok' if ok else 'O HALO PINTA POR CIMA'}")
    print(f"VIZINHOS: {conferidas} barra(s), {ruins} com o halo por cima de outro componente")
    return conferidas > 0 and ruins == 0


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "estilo":
        sys.exit(0 if estilo(sys.argv[2], sys.argv[3]) else 1)
    if len(sys.argv) >= 3 and sys.argv[1] == "vizinhos":
        sys.exit(0 if vizinhos(sys.argv[2]) else 1)
    sys.exit(__doc__)
