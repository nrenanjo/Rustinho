#!/usr/bin/env python3
"""Roteiro da lista de peças (dev.3.3, passo 1.3 — K06).

Abre o Rustinho de verdade numa tela virtual 1920 × 1080 (Xvfb), no ambiente
neutro dos outros roteiros, com uma loja montada PELO NÚCLEO
(`nucleo/tests/lojas_de_teste.rs::montar_lista_de_pecas`: 120 peças, a peça 2
com o código de fornecedor FAG-32206 e 2 pendências de XML). Nada de SQL direto.

Confere os requisitos da lista de peças registrados no histórico de decisões do
proprietário (dossiê dev.3.3, Categoria A, itens 1, 2 e 5):

  L1  a lista carrega peças;
  L2  densidade: cerca de 10 peças visíveis em 1080p (8 a 13);
  L3  o nome da peça sempre visível (nunca colapsa);
  L4  a primeira coluna se chama "Cód. forn." (H07; antes "Código");
  L5  sem código de fornecedor, a primeira coluna mostra o traço "—";
  L6  a primeira coluna nunca mostra o código interno;
  L7  nada empilhado na primeira coluna (nenhum "Interno:");
  L8  o campo de busca está na tela;
  L9  o botão Colar está na busca;
  L10 o botão Copiar está na busca;
  L11 Copiar e F11 não ficam um sobre o outro (H01);
  L12 a busca por texto filtra (FILTRO);
  L13 a busca pelo código do fornecedor (FAG-32206) acha a peça e mostra o
      código na primeira coluna, e o balão mostra "… · FAG-32206 · …";
  L14 a lista é contínua (N04, passo 7.5): nenhum "Anterior", "Próxima" nem
      "Página" na tela (antes: "Página 1 de 3", 50 por página);
  L15 PageDown anda pela lista inteira: passa da linha 100 sem página (a
      próxima parte vem sozinha) até a 120ª peça, e PageUp volta 8 linhas;
  L16 o aviso das pendências de XML aparece e não muda enquanto se digita;
  L17 o cabeçalho da primeira coluna cabe inteiro na coluna (84 px, sem
      reticências), em 1920 × 1080 e em 1366 × 768; no celular (360 × 780,
      --celular) não há cabeçalho e cada linha diz "Cód. forn.: …" (H07: o
      texto novo não pode quebrar a largura da coluna).

As falhas de achados já registrados, que ficam para passos seguintes, são
ESPERADAS (não reprovam o roteiro, mas aparecem no resultado); hoje não há
nenhuma (L11/H01 saiu no passo 7.1 e L4/H07 no 7.3). Se uma esperada passar,
o roteiro avisa e reprova (é hora de tirá-la da lista). Qualquer outra falha
reprova.

    python3 app/testes/telas_lista_pecas.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), cargo. Sai com 0 se aprovado.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":78")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA

ESPERADAS = {}
LARGURA_1A_COLUNA = 152  # o nome da peça começa em x = 152 (GEO:nomePeca)
LARGURA_COLUNA_CODIGO = 84  # a coluna do código do fornecedor (TelaPecas e LinhaPeca)


def montar_loja(pasta):
    """A loja pelo núcleo (os mesmos comandos das telas)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_lista_de_pecas"],
        cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def primeira_coluna(textos):
    """Os textos da primeira coluna das linhas da lista (abaixo do cabeçalho)."""
    nomes = [(y, t) for x, y, t in textos if t.startswith("GEO:nomePeca:")]
    if not nomes:
        return []
    y_min = min(y for y, _ in nomes) - 40
    return [(x, y, t) for x, y, t in textos
            if x < LARGURA_1A_COLUNA and y >= y_min and not t.startswith(("GEO:", "RET:"))]


def nomes_das_pecas(textos):
    """Os nomes das peças visíveis (o texto que está no centro de cada GEO:nomePeca)."""
    geos = [(x, y) for x, y, t in textos if t.startswith("GEO:nomePeca:")]
    return [t for x, y, t in textos if not t.startswith(("GEO:", "RET:")) and (x, y) in geos]


def aviso_xml(textos):
    return next((t for _, _, t in textos if "de XML aguardando" in t), "")


def texto_com(textos, trecho):
    return [(x, y, t) for x, y, t in textos if trecho in t and not t.startswith(("GEO:", "RET:"))]


def marcador(textos):
    """A lista na tela (TESTE_LISTA): (linhas, peças diferentes, ainda há mais, linha atual)."""
    for _, _, t in textos:
        if t.startswith("TESTE_LISTA:"):
            n, dif, mais, atual = t.split(":")[1:5]
            return int(n), int(dif), mais == "1", int(atual)
    return None


def cabecalho_1a_coluna(textos):
    """O rótulo do cabeçalho da 1ª coluna, pelas linhas RET: (--medir-toque):
    (x, largura, largura natural, texto), ou None se não há cabeçalho. É o
    Rotulo à esquerda do "Peça…", na mesma altura. A largura natural maior que
    a largura quer dizer reticências (o Rotulo corta o fim)."""
    rets = []
    for _, _, t in textos:
        if t.startswith("RET:"):
            numeros, _, componente, texto = t[4:].split("|", 3)
            rets.append(([float(v) for v in numeros.split(",")], componente, texto))
    peca = [v for v, c, tx in rets if c == "Rotulo" and tx.startswith("Peça")]
    if not peca:
        return None
    x_peca, y_peca = peca[0][0], peca[0][1]
    antes = sorted((v[0], v[2], v[6], tx) for v, c, tx in rets
                   if c == "Rotulo" and abs(v[1] - y_peca) < 4 and v[0] < x_peca)
    return antes[0] if antes else None


def cabe_na_coluna(cab):
    """O cabeçalho existe, a coluna continua com 84 px e o texto cabe inteiro."""
    return cab is not None and abs(cab[1] - LARGURA_COLUNA_CODIGO) < 0.5 and cab[2] <= cab[1] + 0.5


def com_pagina(textos):
    """Textos de paginação na tela (não pode haver: N04)."""
    return [tx for _, _, tx in textos if not tx.startswith(("GEO:", "RET:", "TOQUE:"))
            and (re.search(r"\bP[áa]gina\b", tx) or tx.strip() in ("Anterior", "Próxima"))]


def digitar_na_busca(rr, g, texto):
    rr.clicar_em(g["campoDeBusca"])
    base.xdo("key", "ctrl+a")
    if texto:
        base.xdo("type", "--delay", "50", texto)
    else:
        base.xdo("key", "BackSpace")
    time.sleep(1.5)


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-lista-pecas-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-dados-pecas-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        montar_loja(modelo)
        rr = u.Rustinho(modelo, "TelaPecas", {}, fotos, "lista_pecas")
        try:
            for _ in range(150):  # a janela pode demorar a aparecer (como no modo celular do ajudante)
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
            g = u.geos(t)
            nomes_geo = [(float(x), float(y), tx) for x, y, tx in t if tx.startswith("GEO:nomePeca:")]

            conferir("L1", "a lista carrega peças", len(nomes_geo) >= 8, f"{len(nomes_geo)} linhas")
            visiveis = [n for n in nomes_geo if 0 < n[1] < 1080]
            conferir("L2", "cerca de 10 peças visíveis em 1080p", 8 <= len(visiveis) <= 13, f"{len(visiveis)} visíveis")
            larguras = [float(tx.rsplit(":", 1)[1].split(",")[2]) for _, _, tx in nomes_geo]
            conferir("L3", "o nome da peça sempre visível", larguras and min(larguras) > 40, f"larguras {larguras[:5]}")
            cabecalho = [tx for x, y, tx in t if x < LARGURA_1A_COLUNA and tx in ("Código", "Cód. forn.", "CÓD. FORNECEDOR")]
            conferir("L4", "a primeira coluna se chama \"Cód. forn.\"", "Cód. forn." in cabecalho, f"cabeçalho: {cabecalho}")
            col1 = primeira_coluna(t)
            tracos = [tx for _, _, tx in col1 if tx == "—"]
            conferir("L5", "sem código de fornecedor, a primeira coluna mostra \"—\"", len(tracos) >= 5, f"{len(tracos)} traços")
            internos = [tx for _, _, tx in col1 if re.fullmatch(r"7\d{3}|\d{1,6}", tx)]
            conferir("L6", "a primeira coluna nunca mostra o código interno", not internos, f"internos: {internos[:5]}")
            conferir("L7", "nada empilhado na primeira coluna", not texto_com(t, "Interno:"), "")
            conferir("L8", "o campo de busca está na tela", "campoDeBusca" in g, f"{g.get('campoDeBusca')}")
            colar = texto_com(t, "Colar na busca")
            copiar = texto_com(t, "Copiar a busca")
            conferir("L9", "o botão Colar está na busca", bool(colar), f"{colar[:1]}")
            conferir("L10", "o botão Copiar está na busca", bool(copiar), f"{copiar[:1]}")
            f11 = [(x, y, tx) for x, y, tx in t if tx == "F11"]
            sobre = [(c, f) for c in copiar for f in f11 if abs(c[0] - f[0]) < 24 and abs(c[1] - f[1]) < 12]
            conferir("L11", "Copiar e F11 não ficam um sobre o outro", not sobre,
                     f"Copiar {copiar[:1]} × F11 {f11[:1]}")
            aviso_antes = aviso_xml(t)
            cab_1920 = cabecalho_1a_coluna(t)

            digitar_na_busca(rr, g, "FILTRO")
            rr.foto("-2-busca-filtro")
            t2 = rr.textos()
            n2 = nomes_das_pecas(t2)
            conferir("L12", "a busca por texto filtra", n2 and all("FILTRO" in n.upper() for n in n2), f"{n2[:3]}")
            aviso_depois = aviso_xml(t2)
            conferir("L16", "o aviso das pendências de XML aparece e não muda ao digitar",
                     aviso_antes != "" and aviso_antes == aviso_depois, f"antes {aviso_antes[:50]!r} depois {aviso_depois[:50]!r}")

            digitar_na_busca(rr, g, "FAG-32206")
            rr.foto("-3-busca-fornecedor")
            t3 = rr.textos()
            n3 = nomes_das_pecas(t3)
            col3 = primeira_coluna(t3)
            achou = any("CORREIA DENTADA" in n.upper() for n in n3)
            na_coluna = [(x, y) for x, y, tx in col3 if tx == "FAG-32206"]
            balao = []
            if na_coluna:
                x, y = na_coluna[0]
                px, py = rr.ponto(x, y)
                base.xdo("mousemove", str(px), str(py))
                time.sleep(1.5)
                rr.foto("-4-balao")
                balao = [tx for _, _, tx in rr.textos() if "FAG-32206" in tx and "·" in tx]
            conferir("L13", "a busca pelo código do fornecedor acha a peça, mostra o código e o balão",
                     achou and na_coluna and balao, f"achou={achou} coluna={na_coluna[:1]} balão={balao[:1]}")

            digitar_na_busca(rr, g, "")
            t4 = rr.textos()
            m4 = marcador(t4)
            conferir("L14", "lista contínua: nenhum \"Anterior\", \"Próxima\" nem \"Página\"",
                     not com_pagina(t4) and m4 is not None and m4[:3] == (100, 100, True),
                     f"paginação {com_pagina(t4)}; marcador {m4}")
            for _ in range(14):
                base.xdo("key", "Next")
                time.sleep(0.4)
            t5 = rr.textos()
            m5 = marcador(t5)
            rr.foto("-5-depois-da-linha-100")
            base.xdo("key", "Prior")
            time.sleep(1.2)
            m6 = marcador(rr.textos())
            conferir("L15", "PageDown passa da linha 100 sem página até a 120ª peça, e PageUp volta 8 linhas",
                     m5 == (120, 120, False, 112) and m6 is not None and m6[3] == 104 and not com_pagina(t5),
                     f"depois de 14 PageDown {m5}; depois de PageUp {m6}; paginação {com_pagina(t5)}")
            rr.foto("-6-final")
            rr.redimensionar(1366, 768)
            time.sleep(1.5)
            rr.foto("-7-1366x768")
            cab_1366 = cabecalho_1a_coluna(rr.textos())
        finally:
            rr.fechar()
        # O celular (360 × 780, o desenho do Android): a lista sem cabeçalho.
        # Nesta loja as primeiras linhas ficam abaixo do cartão das pendências
        # (fora da tela): basta uma linha criada com "Cód. forn.: …".
        rc = u.Rustinho(modelo, "TelaPecas", {}, fotos, "lista_pecas_celular", celular=True)
        try:
            rc.entrar("Ana")
            time.sleep(3.0)
            rc.foto("-1-lista")
            tc = rc.textos()
            cab_celular = cabecalho_1a_coluna(tc)
            linhas_celular = [tx for _, _, tx in tc if tx.startswith("Cód. forn.: ")]
        finally:
            rc.fechar()
        conferir("L17", "o cabeçalho da 1ª coluna cabe inteiro na coluna de 84 px (1920 e 1366); no celular, "
                        "sem cabeçalho e \"Cód. forn.: …\" na linha",
                 cabe_na_coluna(cab_1920) and cabe_na_coluna(cab_1366) and cab_celular is None and len(linhas_celular) >= 1,
                 f"1920: {cab_1920}; 1366: {cab_1366}; celular: cabeçalho {cab_celular}, "
                 f"{len(linhas_celular)} linhas com \"Cód. forn.: \" (x, largura, largura natural, texto)")
    finally:
        shutil.rmtree(modelo, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()

    reprovado = False
    for codigo in sorted(resultados, key=lambda c: int(c[1:])):
        certo, nome, detalhe = resultados[codigo]
        if certo and codigo in ESPERADAS:
            print(f"  ⚠ {codigo} {nome}: PASSOU, mas estava como falha esperada ({ESPERADAS[codigo]}) — tirar da lista")
            reprovado = True
        elif certo:
            print(f"  ✔ {codigo} {nome}")
        elif codigo in ESPERADAS:
            print(f"  ✘ {codigo} {nome} — falha ESPERADA: {ESPERADAS[codigo]} · {detalhe}")
        else:
            print(f"  ✘ {codigo} {nome} — {detalhe}")
            reprovado = True
    certas = sum(1 for c in resultados.values() if c[0])
    print(f"\n{certas} de {len(resultados)} conferências certas; falhas esperadas: "
          f"{sum(1 for k, v in resultados.items() if not v[0] and k in ESPERADAS)}; fotos em {fotos}")
    sys.exit(1 if reprovado or len(resultados) != 17 else 0)


if __name__ == "__main__":
    main()
