#!/usr/bin/env python3
"""T040 (M-1 da R19): a luz da guia em anéis tem a mesma força da luz do Canvas antigo, nos três temas.

Abre o Rustinho de verdade numa tela virtual, no caminho do B-1 da `telas_c8_revisao` (Etiquetas › Ver duplicatas: a
guia destaca o cartão do código repetido), uma vez em cada tema (o de sempre, o AMOLED e o Planeta Roxo), e mede na
foto o **acréscimo de luz sobre o fundo** logo acima da borda do destaque: a 2 e a 8 px, contra uma linha do mesmo
fundo mais acima (a 34 px ou mais, onde a luz já quase acabou; as duas linhas conferidas na foto sem o destaque). A borda vem da posição que o próprio programa informa (`GEO:luzGuia`).

A referência é a luz do Canvas antigo (`99d479d`), medida uma vez com este mesmo roteiro (`--medir`) e guardada em
`REFERENCIA`. Passa se cada medida ficar a até 25% da referência (ou a 1 nível, quando a referência é pequena).

    python3 app/testes/telas_luz_guia_temas.py [pasta das fotos]
    python3 app/testes/telas_luz_guia_temas.py --medir [pasta]   (só imprime as medidas; não confere)
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, ler, montar_loja, por_fora  # noqa: E402

TEMAS = {"de sempre": "", "AMOLED": "amoled", "roxo": "roxo"}

# A luz do Canvas antigo (`99d479d`), em níveis de 0 a 255 (a média dos três canais), medida com `--medir` em 28/11/2026
# no Qt 6.12, desenho por processador: {tema: (acréscimo a 2 px, acréscimo a 8 px)}.
# Medida em `saidas_bateria/luz_ref_canvas3` (8 fotos de véu cheio por tema, todas iguais).
REFERENCIA = {"de sempre": (2.67, 2.33), "AMOLED": (2.33, 2.01), "roxo": (3.67, 3.0)}

DISTANCIAS = (2, 8)
FUNDO_A = 34


def linha(im, y, x0, x1):
    """A média dos três canais (0 a 255) na linha y, de x0 a x1 (de 4 em 4 px)."""
    px = [im.getpixel((x, y)) for x in range(x0, x1, 4)]
    return sum(sum(p[:3]) / 3 for p in px) / len(px)


def variacao(im, y, x0, x1):
    v = [sum(im.getpixel((x, y))[:3]) / 3 for x in range(x0, x1, 4)]
    m = sum(v) / len(v)
    return max(abs(a - m) for a in v)


def acrescimos(foto, foto_sem, ret):
    """{d: a luz a d px acima da borda de cima da luz `ret` (x, y, largura, altura, na janela, que fica no canto da
    tela)}, contra uma linha do MESMO fundo, mais acima, onde a luz já acabou. As duas linhas são conferidas na foto sem
    o destaque: mesma cor e sem texto (senão a medida não vale e volta None)."""
    x, y, w, _ = ret
    com, sem = Image.open(foto).convert("RGB"), Image.open(foto_sem).convert("RGB")
    x0, x1, topo = int(x + 40), int(x + w - 40), int(y)
    perto = [topo - d for d in DISTANCIAS]
    if any(variacao(sem, yy, x0, x1) > 2 for yy in perto):
        return None
    cor = linha(sem, perto[0], x0, x1)
    longe = next((yy for yy in range(topo - FUNDO_A, topo - 140, -1)
                  if variacao(sem, yy, x0, x1) <= 2 and abs(linha(sem, yy, x0, x1) - cor) <= 2), None)
    if longe is None:
        return None
    base = linha(com, longe, x0, x1)
    return ({d: round(linha(com, topo - d, x0, x1) - base, 2) for d in DISTANCIAS}
            | {"longe": topo - longe, "veu": round(linha(sem, longe, x0, x1) - base, 2)})


def escolher(medidas):
    """Das fotos durante o destaque, só as de véu cheio (o escurecimento do fundo a pelo menos 90% do maior: fora do
    esmaecer de entrada e de saída); delas, a mediana de cada distância."""
    medidas = [m for m in medidas if m is not None]
    if not medidas:
        return None
    cheio = max(m["veu"] for m in medidas)
    if cheio <= 0:
        return None
    boas = [m for m in medidas if m["veu"] >= 0.9 * cheio]
    out = {}
    for d in DISTANCIAS:
        v = sorted(m[d] for m in boas)
        out[d] = v[len(v) // 2]
    return out | {"fotos": len(boas), "veu": round(cheio, 2), "longe": boas[0]["longe"]}


def ret_da_luz(r):
    for _, _, t in r.textos():
        if t.startswith("GEO:luzGuia:"):
            x, y, w, h = (float(v) for v in t.split(":", 2)[2].split(","))
            if w > 0 and h > 0:
                return (x, y, w, h)
    return None


def medir_tema(placar, fotos, nome, tema):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-luz-")
    try:
        montar_loja(pasta)
        if tema:
            por_fora(pasta, f"INSERT INTO config (chave, valor) VALUES ('tela_tema', '{tema}') "
                            "ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor;")
        compra = ler(pasta, "SELECT id FROM compra ORDER BY rowid LIMIT 1")[0][0]
        travada_id = ler(pasta, "SELECT i.produto_id FROM compra_item i WHERE i.compra_id = ? ORDER BY i.ordem LIMIT 1",
                         compra)[0][0]
        codigo = ler(pasta, "SELECT codigo FROM produto WHERE nome = 'Filtro de óleo'")[0][0]
        por_fora(pasta, f"UPDATE produto SET codigo = '{codigo}' WHERE id = '{travada_id}'")
        r = placar.abrir(pasta, "TelaEtiquetas", {"compraId": compra}, fotos, f"luz-{tema or 'sempre'}")
        try:
            r.entrar("Ana")
            if not r.esperar(lambda: r.tem_igual("Ver duplicatas")):
                return None
            r.clicar("Ver duplicatas")
            r.esperar(lambda: r.tem("Códigos das peças") and r.tem_igual("Dar código novo"))
            # O destaque começa depois de a lista assentar e dura 6 s: a posição da luz e várias fotos; fica a maior
            # luz (a de opacidade cheia).
            ret = None
            fotos_com = []
            for k in range(10):
                time.sleep(0.5)
                if ret is None:
                    ret = ret_da_luz(r)
                if ret is None and os.environ.get("RUSTINHO_LUZ_RET"):
                    # Só para medir a referência no Canvas antigo, que não informa a posição da luz: a mesma posição
                    # medida nos anéis (o desenho da tela é o mesmo).
                    ret = tuple(float(v) for v in os.environ["RUSTINHO_LUZ_RET"].split(","))
                fotos_com.append(r.foto(f"-{k}"))
            # A foto sem o destaque, depois que ele acaba (6 s).
            time.sleep(9)
            sem = r.foto("-sem")
            melhores = escolher([acrescimos(f, sem, ret) for f in fotos_com]) if ret is not None else None
            print(f"luz ({nome}): borda {ret}; acréscimos {melhores}", flush=True)
            return melhores
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    medir = "--medir" in sys.argv or os.environ.get("RUSTINHO_LUZ_MEDIR") == "1"
    args = [a for a in sys.argv[1:] if a != "--medir"]
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = args[0] if args else tempfile.mkdtemp(prefix="rustinho-fotos-luz-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        for nome, tema in TEMAS.items():
            m = medir_tema(placar, fotos, nome, tema)
            if medir:
                continue
            ref = REFERENCIA.get(nome)
            if m is None or ref is None:
                placar.ok(f"luz ({nome}): medida e referência", False, f"medida={m} referência={ref}")
                continue
            for d, esperado in zip(DISTANCIAS, ref):
                folga = max(0.25 * abs(esperado), 1.0)
                placar.ok(f"luz ({nome}) a {d} px igual à do Canvas antigo ({m[d]} contra {esperado}, folga {folga:.1f})",
                          abs(m[d] - esperado) <= folga)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        if not medir:
            placar.ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    if not medir:
        print(f"{placar.certos} certas, {placar.errados} erradas", flush=True)
        sys.exit(1 if placar.errados else 0)


if __name__ == "__main__":
    main()
