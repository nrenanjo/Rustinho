"""Gera as fontes Cormorant Garamond "Ajustada" usadas nos títulos do Rustinho.

Mesmo ajuste feito no livro do Baralho Cigano: na versão usada da Cormorant
Garamond, os acentos das minúsculas (á, â, ê, í, ô, ã...) ficam altos e
estreitos, longe da letra. Este script aproxima os acentos da letra-base e
suaviza suas proporções.

Também troca os números para "algarismos alinhados" (lining figures), como
o livro fazia nos títulos (font-variant-numeric: lining-nums). Sem isso,
nomes como "Óleo 5W30 1 L" ficam com números de altura variável.

A fonte é distribuída sob a SIL Open Font License, que permite modificações.
Como esta é uma versão modificada, a família passa a se chamar
"Cormorant Garamond Ajustada".

As fontes Source Sans 3 e Source Serif 4 não têm nenhuma alteração no
desenho; só são convertidas de .woff2 para .ttf (o PDF dos impressos só
consegue embutir fontes .ttf).

Não é preciso rodar este script para compilar o Rustinho: os arquivos .ttf
gerados já estão nesta pasta. Ele fica aqui para quem quiser refazer.

Uso (precisa do pacote Python fonttools com brotli):
    python3 app/fontes/preparar_fontes.py
"""
import pathlib

from fontTools.ttLib import TTFont

PASTA = pathlib.Path(__file__).resolve().parent
ORIGINAIS = PASTA / "originais"
FAMILIA = "Cormorant Garamond Ajustada"

# arquivo original -> (estilo, peso, itálico)
ARQUIVOS = {
    "cormorant-garamond-latin-500-normal.woff2": ("Medium", 500, False),
    "cormorant-garamond-latin-600-normal.woff2": ("SemiBold", 600, False),
    "cormorant-garamond-latin-700-normal.woff2": ("Bold", 700, False),
    "cormorant-garamond-latin-500-italic.woff2": ("Medium Italic", 500, True),
    "cormorant-garamond-latin-600-italic.woff2": ("SemiBold Italic", 600, True),
}

# glifo: (escala horizontal, escala vertical) — os mesmos valores do livro
ACENTOS = {
    "aacute": (1.0, 0.80), "eacute": (1.0, 0.80), "iacute": (1.0, 0.80),
    "oacute": (1.0, 0.80), "uacute": (1.0, 0.80), "agrave": (1.0, 0.80),
    "acircumflex": (1.14, 0.70), "ecircumflex": (1.14, 0.70), "ocircumflex": (1.14, 0.70),
    "atilde": (1.0, 0.95), "otilde": (1.0, 0.95),
}
FOLGA = 34  # distância entre a altura-x e a base do acento (unidades da fonte)


def contornos(glifo):
    ini = 0
    for fim in glifo.endPtsOfContours:
        yield list(range(ini, fim + 1))
        ini = fim + 1


def ajustar_acentos(fonte):
    glyf, hmtx = fonte["glyf"], fonte["hmtx"]
    x = glyf["x"]
    altura_x = x.yMax if hasattr(x, "yMax") else 396
    base_acento = altura_x + FOLGA
    for nome, (sx, sy) in ACENTOS.items():
        if nome not in glyf:
            continue
        g = glyf[nome]
        if g.isComposite():
            continue
        coords = g.coordinates
        idx = [i for c in contornos(g) if min(coords[j][1] for j in c) > altura_x + 20 for i in c]
        if not idx:
            continue
        xs = [coords[i][0] for i in idx]
        ys = [coords[i][1] for i in idx]
        cx, y0 = (min(xs) + max(xs)) / 2, min(ys)
        for i in idx:
            px, py = coords[i]
            coords[i] = (round(cx + (px - cx) * sx), round(base_acento + (py - y0) * sy))
        g.recalcBounds(glyf)
        largura, _ = hmtx[nome]
        hmtx[nome] = (largura, g.xMin)


def figuras_alinhadas(fonte):
    """Faz os números 0-9 usarem os desenhos alinhados (.lf) por padrão."""
    glifos = set(fonte.getGlyphOrder())
    for tabela in fonte["cmap"].tables:
        if not tabela.isUnicode():
            continue
        for codigo in range(0x30, 0x3A):
            atual = tabela.cmap.get(codigo)
            if atual and f"{atual}.lf" in glifos:
                tabela.cmap[codigo] = f"{atual}.lf"


def renomear(fonte, estilo, peso, italico, familia=FAMILIA):
    """Deixa os nomes internos limpos: uma família com vários pesos."""
    FAMILIA_ = familia
    nome_ps = FAMILIA_.replace(" ", "") + "-" + estilo.replace(" ", "")
    legado = "Bold Italic" if peso == 700 and italico else "Bold" if peso == 700 else "Italic" if italico else "Regular"
    tabela = fonte["name"]
    for rec_id in (1, 2, 3, 4, 6, 16, 17, 21, 22, 25):
        tabela.removeNames(nameID=rec_id)
    tabela.setName(FAMILIA_, 1, 3, 1, 0x409)
    tabela.setName(legado, 2, 3, 1, 0x409)
    tabela.setName(f"{nome_ps};Rustinho", 3, 3, 1, 0x409)
    tabela.setName(f"{FAMILIA_} {estilo}", 4, 3, 1, 0x409)
    tabela.setName(nome_ps, 6, 3, 1, 0x409)
    tabela.setName(FAMILIA_, 16, 3, 1, 0x409)
    tabela.setName(estilo, 17, 3, 1, 0x409)
    os2 = fonte["OS/2"]
    os2.usWeightClass = peso
    sel = os2.fsSelection & ~(1 | 32 | 64)
    if italico:
        sel |= 1
    if peso == 700:
        sel |= 32
    if not italico and peso != 700:
        sel |= 64 if peso == 400 else 0
    os2.fsSelection = sel
    fonte["head"].macStyle = (1 if peso == 700 else 0) | (2 if italico else 0)


# arquivo original -> (família, estilo, peso, itálico)
SOURCE = {
    "source-sans-3-latin-400-normal.woff2": ("Source Sans 3", "Regular", 400, False),
    "source-sans-3-latin-400-italic.woff2": ("Source Sans 3", "Italic", 400, True),
    "source-sans-3-latin-600-normal.woff2": ("Source Sans 3", "SemiBold", 600, False),
    "source-sans-3-latin-700-normal.woff2": ("Source Sans 3", "Bold", 700, False),
    "source-serif-4-latin-400-normal.woff2": ("Source Serif 4", "Regular", 400, False),
    "source-serif-4-latin-400-italic.woff2": ("Source Serif 4", "Italic", 400, True),
    "source-serif-4-latin-600-normal.woff2": ("Source Serif 4", "SemiBold", 600, False),
    "source-serif-4-latin-600-italic.woff2": ("Source Serif 4", "SemiBold Italic", 600, True),
}


def converter_source():
    """Source Sans 3 e Source Serif 4: .woff2 original → .ttf, com o mesmo
    desenho e nomes internos limpos (os arquivos baixados vinham com a
    família "Source Sans 3 ExtraLight", o que confundia o PDF)."""
    for arquivo, (familia, estilo, peso, italico) in SOURCE.items():
        fonte = TTFont(ORIGINAIS / arquivo)
        fonte.flavor = None
        renomear(fonte, estilo, peso, italico, familia)
        saida = PASTA / arquivo.replace(".woff2", ".ttf")
        fonte.save(saida)
        print(saida.name)


def main():
    converter_source()
    for arquivo, (estilo, peso, italico) in ARQUIVOS.items():
        fonte = TTFont(ORIGINAIS / arquivo)
        fonte.flavor = None
        if not italico:
            ajustar_acentos(fonte)
        figuras_alinhadas(fonte)
        renomear(fonte, estilo, peso, italico)
        saida = PASTA / ("CormorantGaramondAjustada-" + estilo.replace(" ", "") + ".ttf")
        fonte.save(saida)
        print(saida.name)


if __name__ == "__main__":
    main()
