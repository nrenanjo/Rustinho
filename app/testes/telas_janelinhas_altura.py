#!/usr/bin/env python3
"""T020 (feature 001-conclusao-dev33, 16/11/2026): MEDE o risco do H06 nas janelinhas de lançamento com prévia de parcelas,
irmãs da do Receber corrigida (BUG-20261116-OPEO). Só mede: não muda nada no programa.

Janelinhas (as duas sem rolagem no miolo, pelo levantamento estático, e com uma prévia que cresce com as parcelas):
  - "Lançar dívida" (TelaReceber, `lancar`): Cliente, Do que é, Valor + Parcelas + 1º vencimento lado a lado, prévia.
  - "Lançar conta a pagar" (TelaFinanceiro › A pagar, `lancamento` da SecaoPagar): 9 campos, prévia.
Casos: vazia (como abre) e 12 parcelas (valor, 12 parcelas e o 1º vencimento preenchidos: a prévia mostra as 12).
Para cada caso e tamanho, confere pelo que o programa informa (--medir-toque):
  - o botão "Lançar" da janelinha está inteiro dentro da janela, ou fica, depois de rolar o miolo;
  - os campos de texto à vista cabem na largura da janela e têm pelo menos 40 de largura (os lado a lado no celular).

    RUSTINHO_TELA=:68 python3 telas_janelinhas_altura.py [pasta das fotos]      (rodar da raiz da árvore testada)
    RUSTINHO_CASOS_CELULAR=1   (os tamanhos de celular; sem ela, os de computador)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

# Os auxiliares de tela da árvore testada (rodar da raiz dela), senão os desta pasta.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.getcwd(), "app", "testes"))
import telas_navegacao_casos as nav  # noqa: E402
import telas_receber_altura as alt  # noqa: E402

TELA = nav.TELA
xdo = nav.xdo
TAMANHOS = [t for t in alt.TAMANHOS]

JANELINHAS = {
    "receber · Lançar dívida": dict(tela="TelaReceber", props={}, abrir="Lançar dívida",
                                    sinal="Para quem já devia antes do Rustinho",
                                    campos=[("Valor", "1200"), ("Parcelas", "12"), ("1º vencimento", "15/12/2026")]),
    "pagar · Lançar conta a pagar": dict(tela="TelaFinanceiro", props={"secao": "pagar"}, abrir="Lançar conta",
                                         sinal="Lançar conta a pagar",
                                         campos=[("Valor total", "1200"), ("Parcelas", "12"), ("Vencimento", "15/12/2026")]),
}
CASOS = ["vazia", "12 parcelas"]


class Rustinho(nav.Rustinho):
    """O mesmo Rustinho da sonda do Receber (telas_receber_altura.Rustinho), abrindo a tela pedida."""

    def __init__(self, pasta, tela, props, fotos, nome, celular, tamanho):
        self.fotos, self.nome = fotos, nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true", QT_SCALE_FACTOR="1")
        env.pop("WAYLAND_DISPLAY", None)
        self.log = open(self.saida, "w")
        if celular:
            nav.CELULAR_DP = tamanho
        args = [nav.PROGRAMA, "--textos-da-tela", "--medir-toque"] + (["--celular"] if celular else [])
        self.p = subprocess.Popen(args + ["--abrir", tela, "--props", json.dumps(props)],
                                  env=env, stdout=self.log, stderr=self.log)
        if not nav.conferir_ambiente(self.p.pid):
            self.fechar()
            raise RuntimeError("o Rustinho do teste não está na tela virtual (ou tem Wayland): parei antes de clicar")
        self._preparar_janela_celular(*tamanho)
        self.ambiente_ok = True


def campos_toque(r):
    out = []
    for t in alt.medidas(r):
        if t.startswith("TOQUE:campo|"):
            p = t.split("|")
            out.append(tuple(float(v) for v in p[4].split(",")))
    return out


def rotulo_ret(r, rotulo):
    achados = []
    for t in alt.medidas(r):
        if t.startswith("RET:"):
            geo, resto = t[4:].split("|", 1)
            texto = resto.split("|")[-1]
            if texto.strip() == rotulo:
                v = [float(a) for a in geo.split(",")]
                achados.append((v[0], v[1], v[2], v[3]))
    return achados


def campos_da_janelinha(r):
    """Os campos (TOQUE:campo) da janelinha aberta: os informados depois da linha GEO:fundoDaJanelinha (o programa
    informa primeiro a tela de trás, depois a janelinha e os itens dela). None se não há janelinha nas medidas. Lê as
    linhas na ordem do programa nos dois desenhos (no celular, alt.medidas junta as GEO: no fim e perde a ordem).
    O marcador é o nome que o Painel.qml dá ao fundo da janelinha para os testes. Não serve GEO:Painel: nenhum
    objectName do programa a produz; ela aparece no Qt 6.12 e não no Qt 6.8.3 dos pacotes, e a CI 44835430265
    reprovou por isso (20/11)."""
    if nav.CELULAR:
        r._ponteiro_dentro()
    out, achou = [], False
    for _x, _y, t in nav.base.Rustinho.textos(r):
        if t.startswith("GEO:fundoDaJanelinha:"):
            out, achou = [], True
        elif achou and t.startswith("TOQUE:campo|"):
            out.append(tuple(float(v) for v in t.split("|")[4].split(",")))
    return out if achou else None


def miolo_geo(r):
    """O retângulo do miolo que rola na janelinha (GEO:rolagemDoLancar / rolagemDoLancamento), se houver."""
    for linha in alt.medidas(r):
        for nome in ("GEO:rolagemDoLancar:", "GEO:rolagemDoLancamento:"):
            if linha.startswith(nome):
                return tuple(float(v) for v in linha[len(nome):].split(",")[:4])
    return None


def preencher(r, rotulo, texto):
    """Toca no campo logo abaixo do rótulo (o da janelinha, desenhado por último) e troca o texto dele. Se a janelinha
    tem miolo que rola e o campo está fora do recorte dele, rola o miolo até o campo aparecer (R6 da revisão da T040:
    tocar fora do recorte mandava o texto para outro campo)."""
    for _ in range(5):
        rs = rotulo_ret(r, rotulo)
        m = miolo_geo(r)
        if not rs or not m:
            break
        ly = rs[-1][1]
        mx, my, mw, mh = m
        if my <= ly and ly + 90 <= my + mh:
            break
        cx, cy = int(mx + mw / 2), int(my + mh / 2)
        passo = int(min(mh * 0.6, max(40, abs(ly + 45 - cy)))) * (1 if ly + 45 > cy else -1)
        xdo("mousemove", str(cx), str(cy))
        xdo("mousedown", "1")
        for i in range(1, 9):
            xdo("mousemove", str(cx), str(cy - passo * i // 8))
            time.sleep(0.03)
        time.sleep(0.15)
        xdo("mouseup", "1")
        time.sleep(0.5)
    rs = rotulo_ret(r, rotulo)
    if not rs:
        raise RuntimeError(f"INSTRUMENTAÇÃO: o rótulo {rotulo!r} não está à vista")
    lx, ly, lw, lh = rs[-1]
    # Só os campos da janelinha: as medidas também trazem os da tela de trás, cobertos pelo véu. Na T056 (20/11) a
    # busca de A pagar passou a ficar 16 abaixo do rótulo "Valor total", mais perto que o campo dele, e o toque ia
    # para um ponto da janelinha sem campo (o texto caía no campo que já tinha o foco).
    campos = campos_da_janelinha(r)
    if campos is None:
        raise RuntimeError("INSTRUMENTAÇÃO: a janelinha (GEO:fundoDaJanelinha) não está nas medidas")
    cands = []
    for x, y, w, h in campos:
        cy = y + h / 2
        if 0 < cy - ly < 90:
            cands.append((cy - ly, abs((x + w / 2) - (lx + lw / 2)), x, y, w, h))
    if not cands:
        raise RuntimeError(f"INSTRUMENTAÇÃO: nenhum campo logo abaixo do rótulo {rotulo!r}")
    _, _, x, y, w, h = sorted(cands)[0]
    xdo("mousemove", str(int(x + w / 2)), str(int(y + h / 2)), "click", "1")
    time.sleep(0.4)
    xdo("key", "ctrl+a")
    xdo("type", "--delay", "70", texto)
    time.sleep(0.6)


def caso(placar, fotos, modelo, nome_j, j, nome_caso, nome_tam, celular, tamanho):
    ok = placar.ok
    largura, altura = tamanho
    rotulo = f"{nome_j} · {nome_caso} · {nome_tam}"
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-janelinhas-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    nav.por_fora(pasta, alt.SQL_BASE)
    nome_arq = f"{nome_j}-{nome_caso}-{nome_tam}".replace(" ", "_").replace("·", "").replace("__", "_")
    r = Rustinho(pasta, j["tela"], j["props"], fotos, nome_arq, celular, tamanho)
    placar.logs.append(r.saida)
    try:
        r.entrar("Ana")
        if not ok(f"{rotulo}: a tela abriu", r.esperar(lambda: r.tem(j["abrir"]), 20), r.foto("-0")):
            return
        r.clicar(j["abrir"])
        if not ok(f"{rotulo}: a janelinha abriu", r.esperar(lambda: r.tem(j["sinal"]), 10), r.foto("-1")):
            return
        time.sleep(0.8)
        if nome_caso == "12 parcelas":
            for rot, txt in j["campos"]:
                preencher(r, rot, txt)
            time.sleep(1.0)
            # R6: o caso só vale se a prévia mostrou as 12 parcelas (senão a medida seria da janelinha quase vazia)
            if not ok(f"{rotulo}: a prévia mostra as 12 parcelas (o caso aconteceu)", any(t.strip().startswith("12ª") for _x, _y, t in r.textos()), r.foto("-p")):
                return
        f = r.foto("-2")
        b = alt.botoes(r, "Lançar")
        if not ok(f"{rotulo}: a janelinha tem o botão Lançar", bool(b), f):
            return
        ret = b[-1]
        a_vista = alt.dentro(ret, largura, altura)
        rolou = None
        if not a_vista:
            alt.rolar_miolo(r, largura, altura, celular)
            b2 = alt.botoes(r, "Lançar")
            rolou = b2[-1] if b2 else None
            a_vista = bool(rolou) and alt.dentro(rolou, largura, altura)
        f2 = r.foto("-3")
        ok(f"{rotulo}: o botão Lançar fica inteiro dentro da janela (à vista ou rolando o miolo)", a_vista,
           f"{f2} janela {largura}×{altura}; botão {tuple(round(v) for v in ret)}"
           + ("" if rolou is None else f" → depois de rolar {tuple(round(v) for v in rolou)}"))
        cs = campos_toque(r)
        fora = [tuple(round(v) for v in c) for c in cs if c[0] < -1 or c[0] + c[2] > largura + 1]
        estreitos = [tuple(round(v) for v in c) for c in cs if c[2] < 40]
        ok(f"{rotulo}: os campos à vista cabem na largura e têm pelo menos 40 de largura", not fora and not estreitos,
           f"{f2} fora da largura: {fora}; estreitos: {estreitos}")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(nav.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({nav.PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-janelinhas-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    print("BINÁRIO:", nav.PROGRAMA, flush=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = nav.Placar()
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-janelinhas-modelo-")
    try:
        nav.montar_loja(modelo)
        for nome_j, j in JANELINHAS.items():
            for nome_caso in CASOS:
                for nome_tam, celular, tamanho in TAMANHOS:
                    print(f"--- {nome_j} · {nome_caso} · {nome_tam}", flush=True)
                    try:
                        caso(placar, fotos, modelo, nome_j, j, nome_caso, nome_tam, celular, tamanho)
                    except Exception as e:
                        placar.ok(f"{nome_j} · {nome_caso} · {nome_tam} chegou ao fim ({e!r})", False)
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
