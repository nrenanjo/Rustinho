#!/usr/bin/env python3
"""I8 (dev3.3): regressão dos achados M-1, B-1, B-4 e B-5 da revisão independente do VIS2 (Fable, 16/11/2026;
`revisao_independente/fable/RELATORIO_REVISAO_VIS2_AVISOS.md`), no desenho do celular (360 × 780), com a loja de listas
compridas do revisor. Tirado da sonda dele (`i8_celular.py`, que só media), com conferências estritas. Mede o que o
programa DESENHA (as linhas RET: e TOQUE: do --medir-toque), e a rolagem só conta quando a lista andou de fato (B-6).

  M-1  O que comprar: o nome da peça e o da encomenda aparecem (largura de desenho maior que zero) e nenhum texto da
       linha passa da borda direita.
  B-1  Mais vendidos: a área de toque das pílulas "Por valor / Por quantidade" não cruza a 1ª linha da lista.
  B-4  O que comprar e Encomendas: com a lista rolada, depois de "Já pedi" numa linha, as linhas vizinhas continuam na
       área da lista (antes, a lista voltava ao começo e só o topo ficava à vista).
  B-5  Painel › Movimento por hora com três horas empatadas no pico: nenhum par de rótulos de hora se cruza.

    RUSTINHO_TELA=:88 python3 app/testes/telas_i8_revisao.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o congelado, para o "antes")
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

os.environ["RUSTINHO_CASOS_CELULAR"] = "1"  # este roteiro é só do desenho do celular
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_i8_busca_celular import SQL_CHEIA  # noqa: E402  (a mesma loja de listas compridas)

TELA = nav.TELA
W, H = nav.CELULAR_DP

# Três vendas concluídas, ontem, às 3h, 14h e 22h: três horas empatadas no pico (a sonda do revisor).
SQL_EMPATE = """
UPDATE venda SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', 'localtime', 'start of day', '-1 day',
    CASE (SELECT COUNT(*) FROM venda v2 WHERE v2.tipo = 'venda' AND v2.situacao = 'concluida' AND v2.rowid < venda.rowid)
      WHEN 0 THEN '+3 hours' WHEN 1 THEN '+14 hours' ELSE '+22 hours' END, '+30 minutes', 'utc')
 WHERE tipo = 'venda' AND situacao = 'concluida';
"""


def medir(r):
    """(textos, toques, rets) de agora, das linhas TOQUE: e RET: do --medir-toque."""
    textos = r.textos()
    toques, rets = [], []
    for _, _, t in r._medidas:
        if t.startswith("TOQUE:"):
            p = t[6:].split("|")
            if len(p) != 6:
                continue
            x, y, w, h = (float(v) for v in p[4].split(","))
            toques.append(dict(tipo=p[0], componente=p[1], nome=p[3], x=x, y=y, w=w, h=h, ativo=p[5] == "1"))
        elif t.startswith("RET:"):
            ret, alinhamento, comp, texto = t[4:].split("|", 3)
            v = [float(a) for a in ret.split(",")]
            rets.append(dict(x=v[0], y=v[1], w=v[2], h=v[3], cw=v[4], ch=v[5], alinhamento=int(alinhamento),
                             texto=texto))
    return textos, toques, rets


def pintado(t):
    """(x0, x1) do que o texto pinta: a largura do desenho, pelo alinhamento."""
    pw = min(t["cw"], t["w"])
    a = t["alinhamento"]
    x0 = t["x"] + (t["w"] - pw) if a & 2 else t["x"] + (t["w"] - pw) / 2 if a & 4 else t["x"]
    return x0, x0 + pw


def arrasto(x, y0, y1):
    """Arrasta o dedo de y0 a y1, devagar e parado antes de soltar (sem impulso)."""
    nav.xdo("mousemove", str(x), str(y0))
    nav.xdo("mousedown", "1")
    for i in range(1, 11):
        nav.xdo("mousemove", str(x), str(int(y0 + (y1 - y0) * i / 10)))
        time.sleep(0.03)
    time.sleep(0.2)
    nav.xdo("mouseup", "1")
    time.sleep(0.8)


def ponto_livre(toques, topo, base, x=24):
    """Um y entre topo e base, em x, fora dos campos de texto (o mouse da simulação seleciona texto no campo)."""
    campos = [(t["y"] - 6, t["y"] + t["h"] + 6) for t in toques if t["tipo"] == "campo" and t["x"] <= x <= t["x"] + t["w"]]
    for y in range(int(base), int(topo), -4):
        if not any(a <= y <= b for a, b in campos):
            return y
    return None


def area_da_lista(toques):
    """(topo, base) da área que rola: abaixo das pílulas das partes do Estoque, até o pé da janela."""
    partes = [t for t in toques if t["nome"] in ("O que comprar", "Encomendas", "Mais vendidos", "Parados")
              and t["y"] < H / 2]
    return (max(t["y"] + t["h"] for t in partes) + 14 if partes else 400), H - 14


def abrir_secao(placar, modelo, secao, fotos, nome):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-i8rev-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    r = placar.abrir(pasta, "TelaEstoque", {"secao": secao}, fotos, nome)
    r.entrar("Ana")
    r.esperar(lambda: r.tem("Estoque / Fornecedores"), 15)
    time.sleep(2)
    return r, pasta


def rolar_lista(r, n=3):
    """Rola a lista n vezes; devolve (antes, depois): o y do 1º texto de linha visto, para provar que andou."""
    _, toques, rets = medir(r)
    topo, base = area_da_lista(toques)
    antes = min((t["y"] for t in rets if "da sonda" in t["texto"]), default=None)
    for _ in range(n):
        y0 = ponto_livre(toques, topo + 12, base - 6)
        arrasto(24, y0, topo + 12)
        _, toques, rets = medir(r)
    depois = min((t["y"] for t in rets if "da sonda" in t["texto"]), default=None)
    return topo, base, antes, depois


def caso_m1(placar, modelo, fotos):
    ok = placar.ok
    r, pasta = abrir_secao(placar, modelo, "comprar", fotos, "m1-comprar")
    try:
        topo, base, antes, depois = rolar_lista(r)
        _, _, rets = medir(r)
        f = r.foto("-1")
        ok("M-1: a lista de O que comprar rolou (as linhas subiram)", antes is not None and depois is not None and
           depois < antes - 40, f"{f} 1º texto de linha: y {antes} → {depois}")
        na_area = [t for t in rets if topo <= t["y"] and t["y"] + t["h"] <= base]
        nomes = [t for t in na_area if t["texto"].startswith(("Peça da sonda", "Encomenda da sonda"))
                 and t["h"] >= 18]  # o nome (fonte de 18 a 20 px), e não a 2ª linha da encomenda
        ok("M-1: há linhas de peça e de encomenda à vista na lista", len(nomes) >= 2, f"{f} {len(nomes)} nomes")
        sem_largura = [t["texto"] for t in nomes if min(t["cw"], t["w"]) < 40]
        ok("M-1: o nome de cada linha à vista aparece (largura de desenho de pelo menos 40 px)", not sem_largura,
           f"{f} sem largura: {sem_largura[:3]}")
        passam = [t["texto"] for t in na_area if pintado(t)[1] > W - 8]
        ok("M-1: nenhum texto das linhas passa da borda direita", not passam, f"{f} passam: {passam[:3]}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def caso_b1(placar, modelo, fotos):
    ok = placar.ok
    r, pasta = abrir_secao(placar, modelo, "vendidos", fotos, "b1-vendidos")
    try:
        _, toques, rets = medir(r)
        f = r.foto("-1")
        pilulas = [t for t in toques if t["nome"] in ("Por valor", "Por quantidade")]
        # A 1ª linha: a área de toque larga (a linha inteira abre a peça) mais alta abaixo do começo das pílulas.
        inicio_pilulas = min((t["y"] for t in pilulas), default=0)
        linhas = [t for t in toques if t["tipo"] in ("tap", "mousearea") and t["w"] > 250 and 40 <= t["h"] <= 140
                  and t["y"] > inicio_pilulas]
        if not ok("B-1: as pílulas e a 1ª linha estão à vista", bool(pilulas) and bool(linhas),
                  f"{f} pílulas={len(pilulas)} linhas={len(linhas)}"):
            return
        fundo_pilulas = max(t["y"] + t["h"] for t in pilulas)
        primeira = min(t["y"] for t in linhas)
        ok("B-1: a área de toque das pílulas não cruza a 1ª linha da lista", fundo_pilulas <= primeira + 0.5,
           f"{f} pílulas até y={fundo_pilulas:.0f}; 1ª linha a partir de y={primeira:.0f}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def caso_b4(placar, modelo, fotos, secao):
    ok = placar.ok
    r, pasta = abrir_secao(placar, modelo, secao, fotos, f"b4-{secao}")
    try:
        topo, base, antes, depois = rolar_lista(r)
        textos, _, rets = medir(r)
        f = r.foto("-1")
        if not ok(f"B-4 ({secao}): a lista rolou antes da ação", antes is not None and depois is not None and
                  depois < antes - 40, f"{f} 1º texto de linha: y {antes} → {depois}"):
            return
        vistas = {t["texto"] for t in rets if "Encomenda da sonda" in t["texto"] and topo <= t["y"] and
                  t["y"] + t["h"] <= base}
        botoes = sorted((y, x) for x, y, t in textos if t == "Já pedi" and topo + 10 < y < base - 10)
        if not ok(f"B-4 ({secao}): há encomendas e um \"Já pedi\" à vista", len(vistas) >= 2 and bool(botoes),
                  f"{f} vistas={sorted(vistas)} botões={len(botoes)}"):
            return
        y, x = botoes[0]
        nav.xdo("mousemove", str(x), str(y), "click", "1")
        time.sleep(1.8)
        _, _, rets2 = medir(r)
        f2 = r.foto("-2-depois")
        vistas2 = {t["texto"] for t in rets2 if "Encomenda da sonda" in t["texto"] and topo <= t["y"] and
                   t["y"] + t["h"] <= base}
        ficaram = vistas & vistas2
        ok(f"B-4 ({secao}): depois de \"Já pedi\", as linhas vizinhas continuam na área da lista", len(ficaram) >= 1,
           f"{f2} antes={sorted(vistas)} depois={sorted(vistas2)}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def caso_b5(placar, modelo, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-i8rev-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    nav.por_fora(pasta, SQL_EMPATE)
    r = placar.abrir(pasta, "TelaPainel", {"secao": "resumo"}, fotos, "b5-empate")
    try:
        r.entrar("Ana")
        r.esperar(lambda: r.tem("Como a loja está indo"), 15)
        time.sleep(2.5)
        rets = []
        for _ in range(9):
            _, _, rets = medir(r)
            if any("Movimento por hora" in t["texto"] and 80 < t["y"] < H - 260 for t in rets):
                break
            r.arrastar_para_cima(0.3)
        f = r.foto("-1")
        titulo = [t for t in rets if "Movimento por hora" in t["texto"]]
        if not ok("B-5: o Movimento por hora está à vista", bool(titulo), f):
            return
        y0 = titulo[0]["y"]
        horas = [t for t in rets if re.fullmatch(r"\d{1,2}h", t["texto"].strip()) and y0 < t["y"] < y0 + 320]
        cruzados = []
        for i, a in enumerate(horas):
            for b in horas[i + 1:]:
                (a0, a1), (b0, b1) = pintado(a), pintado(b)
                if abs(a["y"] - b["y"]) < 4 and min(a1, b1) - max(a0, b0) > 0.5:
                    cruzados.append((a["texto"], b["texto"]))
        ok("B-5: há rótulos de hora à vista", len(horas) >= 3, f"{f} {len(horas)} rótulos")
        ok("B-5: com três horas empatadas no pico, nenhum par de rótulos de hora se cruza", not cruzados,
           f"{f} cruzados: {cruzados}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(nav.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({nav.PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-i8rev-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    print("BINÁRIO:", nav.PROGRAMA, flush=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = nav.Placar()
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-i8rev-modelo-")
    try:
        nav.montar_loja(modelo)
        nav.por_fora(modelo, SQL_CHEIA)
        for nome, caso in (("M-1", lambda: caso_m1(placar, modelo, fotos)),
                           ("B-1", lambda: caso_b1(placar, modelo, fotos)),
                           ("B-4 comprar", lambda: caso_b4(placar, modelo, fotos, "comprar")),
                           ("B-4 encomendas", lambda: caso_b4(placar, modelo, fotos, "encomendas")),
                           ("B-5", lambda: caso_b5(placar, modelo, fotos))):
            print(f"--- {nome}", flush=True)
            try:
                caso()
            except Exception as e:
                placar.ok(f"{nome} chegou ao fim ({e!r})", False)
    finally:
        shutil.rmtree(modelo, ignore_errors=True)
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
