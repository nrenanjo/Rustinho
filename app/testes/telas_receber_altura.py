#!/usr/bin/env python3
"""A janelinha "Receber de …" (TelaReceber) em telas pequenas: o botão "Receber" fica sempre ao alcance (16/11/2026).

Achado na validação da integ8 (lacuna 14, caso 7 no celular): com a mensagem longa de registro inconsistente, o botão
"Receber" aparecia cortado no pé da tela. O `Painel.qml` limita a altura da janelinha à janela e diz que "a que pode
ficar mais alta rola o miolo e deixa os botões à vista" (H06); a do Receber da venda faz isso, a do Receber de cliente
não fazia.

Para cada caso e cada tamanho, abre a tela de receber do Zé da Oficina, pede "Receber tudo" e confere:
  - o botão "Receber" da janelinha está inteiro dentro da janela, ou fica, depois de rolar o miolo da janelinha;
  - no caso do erro longo, a mensagem do recebimento recusado fica à vista, dentro do miolo que rola.
Casos: normal (uma conta); erro longo (a conta de registro inconsistente, depois de tentar receber); caixa fechado
(sem caixa aberto: a recusa comum, duas vezes, com o miolo levado ao começo entre elas: R8 da revisão da T040); muitas
parcelas (12 contas). Tamanhos: celular 360×780 e 320×640, celular deitado 780×360, computador 1280×760 e 1024×600.
Mede pelo que o programa informa (--medir-toque: as linhas TOQUE: com o retângulo de cada botão).

    RUSTINHO_TELA=:68 python3 app/testes/telas_receber_altura.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o de antes da correção)
    RUSTINHO_CASOS_CELULAR=1               (os tamanhos de celular; sem ela, os de computador)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402

TELA = nav.TELA
xdo = nav.xdo

TAMANHOS = [("celular 360x780", True, (360, 780)), ("celular 320x640", True, (320, 640)),
            ("celular deitado 780x360", True, (780, 360)), ("computador 1280x760", False, (1280, 760)),
            ("computador 1024x600", False, (1024, 600))]
# O jeito de digitar e de tocar dos auxiliares (nav) vale para a execução inteira (RUSTINHO_CASOS_CELULAR): uma execução
# faz os tamanhos de celular, outra os de computador.
TAMANHOS = [t for t in TAMANHOS if t[1] == bool(nav.CELULAR)]

SQL_BASE = """
    INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, aberto_por, valor_inicial)
      SELECT 'sessao-alt', valor, 'Caixa 1', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
             (SELECT id FROM usuario WHERE nome = 'Ana'), 0
        FROM config WHERE chave = 'dispositivo_id';
"""


def conta(id_, descricao, valor, dias=0):
    return f"""
    INSERT INTO conta_receber (id, cliente_id, descricao, valor, vencimento, criado_em, dispositivo)
      SELECT '{id_}', id, '{descricao}', {valor}, date('now', '{dias} day'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
             (SELECT valor FROM config WHERE chave = 'dispositivo_id')
        FROM pessoa WHERE nome = 'Zé da Oficina';"""


CASOS = {
    "normal": SQL_BASE + conta("cr-alt-1", "Caderno", 5000),
    "erro longo": SQL_BASE + conta("cr-alt-1", "Caderno", 5000) + """
    INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
      VALUES ('conta_receber', '["cr-alt-1"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""",
    # R8 da revisão da T040: uma recusa comum (sem a janela "Registro inconsistente" por cima, que no "erro longo"
    # pega o segundo Enter e bloqueia a rolagem): sem caixa aberto, "Abra o caixa antes de receber.", duas vezes.
    # A loja de ensaio já vem com um caixa aberto (1ª rodada, v4: o recebimento deu certo): fecha todos.
    "caixa fechado": conta("cr-alt-1", "Caderno", 5000) + """
    UPDATE caixa_sessao SET fechado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), contado = 0, esperado = 0
     WHERE fechado_em IS NULL;""",
    "muitas parcelas": SQL_BASE + "".join(conta(f"cr-alt-{i}", f"Venda a prazo {i:02d}", 1000 + 100 * i, -i)
                                          for i in range(1, 13)),
}


class Rustinho(nav.Rustinho):
    """O Rustinho na tela de receber, com a medição dos toques (--medir-toque) no celular e no computador, numa janela
    do tamanho pedido, no canto (os pontos dos textos valem como pontos da tela)."""

    def __init__(self, pasta, props, fotos, nome, celular, tamanho):
        self.fotos, self.nome = fotos, nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true", QT_SCALE_FACTOR="1")
        env.pop("WAYLAND_DISPLAY", None)
        self.log = open(self.saida, "w")
        # Os auxiliares (arrastar, tocar no campo) usam o tamanho do celular do módulo: o desta janela.
        if celular:
            nav.CELULAR_DP = tamanho
        args = [nav.PROGRAMA, "--textos-da-tela", "--medir-toque"] + (["--celular"] if celular else [])
        self.p = subprocess.Popen(args + ["--abrir", "TelaReceber", "--props", json.dumps(props)],
                                  env=env, stdout=self.log, stderr=self.log)
        if not nav.conferir_ambiente(self.p.pid):
            self.fechar()
            raise RuntimeError("o Rustinho do teste não está na tela virtual (ou tem Wayland): parei antes de clicar")
        self._preparar_janela_celular(*tamanho)
        self.ambiente_ok = True


def medidas(r):
    """As linhas TOQUE:, RET: e GEO: de agora (no computador, o auxiliar não as separa: lê direto do programa)."""
    if nav.CELULAR:
        geo = [t for _, _, t in r.textos() if t.startswith("GEO:")]
        return [t for _, _, t in getattr(r, "_medidas", [])] + geo
    return [t for _, _, t in nav.base.Rustinho.textos(r) if t.startswith(("TOQUE:", "RET:", "GEO:"))]


def botoes(r, nome):
    """Os retângulos (x, y, w, h) dos BOTÕES com esse nome (as pílulas "Vender / Receber" ficam de fora), das linhas
    TOQUE: (o último é o mais por cima)."""
    achados = []
    for t in medidas(r):
        if t.startswith("TOQUE:"):
            p = t[6:].split("|")
            if len(p) == 6 and p[0] == "botao" and p[3] == nome:
                achados.append(tuple(float(v) for v in p[4].split(",")))
    return achados


MENSAGEM = {"erro longo": "Este registro (contas a receber) ficou inconsistente",
            "caixa fechado": "Abra o caixa antes de receber"}


def erro_a_vista(r, trecho=MENSAGEM["erro longo"]):
    """A mensagem do recebimento recusado está inteira dentro do miolo que rola (GEO:rolagemDoReceber)?
    Devolve (sim ou não, detalhe)."""
    m = medidas(r)
    area = None
    for t in m:
        if t.startswith("GEO:rolagemDoReceber:"):
            x, y, w, h = (int(v) for v in t.rsplit(":", 1)[1].split(","))
            area = (y, y + h)
    texto = None
    for t in m:
        if t.startswith("RET:") and trecho in t:
            v = [float(a) for a in t[4:].split("|", 1)[0].split(",")]
            texto = (v[1], v[1] + v[3])
    if area is None or texto is None:
        return False, f"área do miolo={area} mensagem={texto}"
    return area[0] - 2 <= texto[0] and texto[1] <= area[1] + 2, f"área do miolo={area} mensagem={texto}"


def dentro(ret, largura, altura):
    x, y, w, h = ret
    return x >= 0 and y >= 0 and x + w <= largura + 1 and y + h <= altura + 1


def rolar_miolo(r, largura, altura, celular):
    """Tenta rolar o miolo da janelinha para baixo (mostrar o que está embaixo): arrastar para cima no meio dela e,
    no computador, também a roda do mouse."""
    x, y0 = largura // 2, int(altura * 0.6)
    for _ in range(4):
        xdo("mousemove", str(x), str(y0))
        xdo("mousedown", "1")
        for i in range(1, 9):
            xdo("mousemove", str(x), str(y0 - int(altura * 0.35) * i // 8))
            time.sleep(0.03)
        time.sleep(0.15)
        xdo("mouseup", "1")
        time.sleep(0.4)
        if not celular:
            xdo("mousemove", str(x), str(y0))
            for _ in range(5):
                xdo("click", "5")
            time.sleep(0.4)


def rolar_miolo_ao_topo(r):
    """Leva o miolo da janelinha de volta ao começo (arrasta para baixo dentro dele, GEO:rolagemDoReceber), como a
    pessoa que subiu para conferir o valor depois da recusa."""
    area = None
    for t in medidas(r):
        if t.startswith("GEO:rolagemDoReceber:"):
            area = tuple(int(v) for v in t.rsplit(":", 1)[1].split(","))
    if area is None:
        return
    x, y, w, h = area
    cx = x + w // 2
    for _ in range(4):
        y0, y1 = y + int(h * 0.15), y + int(h * 0.9)
        xdo("mousemove", str(cx), str(y0))
        xdo("mousedown", "1")
        for i in range(1, 9):
            xdo("mousemove", str(cx), str(y0 + (y1 - y0) * i // 8))
            time.sleep(0.03)
        time.sleep(0.15)
        xdo("mouseup", "1")
        time.sleep(0.4)


def tentar_receber(r, largura, altura, celular):
    """Tenta receber: no celular, o toque no botão (se estiver à vista); senão, o Enter."""
    b = botoes(r, "Receber")
    if celular and b and dentro(b[-1], largura, altura):
        x, y, w, h = b[-1]
        xdo("mousemove", str(int(x + w / 2)), str(int(y + h / 2)), "click", "1")
    else:
        xdo("key", "Return")


def caso(placar, fotos, modelo, nome_caso, nome_tam, celular, tamanho):
    ok = placar.ok
    largura, altura = tamanho
    rotulo = f"{nome_caso} · {nome_tam}"
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-altura-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    nav.por_fora(pasta, CASOS[nome_caso])
    ze = nav.ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
    nome_arq = f"{nome_caso}-{nome_tam}".replace(" ", "_")
    r = Rustinho(pasta, {"clienteInicial": ze}, fotos, nome_arq, celular, tamanho)
    placar.logs.append(r.saida)
    try:
        r.entrar("Ana")
        if not ok(f"{rotulo}: a tela de receber abriu", r.esperar(lambda: r.tem("Receber tudo"), 20), r.foto("-0")):
            return
        r.clicar("Receber tudo")
        if not ok(f"{rotulo}: a janelinha abriu", r.esperar(lambda: r.tem("Receber de"), 10), r.foto("-1")):
            return
        time.sleep(0.8)
        if nome_caso in MENSAGEM:
            # Tenta receber (o Enter, como a pessoa faz no teclado; no celular o toque no botão, se ele estiver à vista):
            # a conta é de registro inconsistente (a mensagem longa) ou não há caixa aberto.
            tentar_receber(r, largura, altura, celular)
            r.esperar(lambda: r.tem(MENSAGEM[nome_caso][:20]), 8)
            time.sleep(0.8)
        f = r.foto("-2")
        erro_visto = erro_a_vista(r, MENSAGEM[nome_caso]) if nome_caso in MENSAGEM else (True, "")
        b = botoes(r, "Receber")
        if not ok(f"{rotulo}: a janelinha tem o botão Receber", bool(b), f):
            return
        ret = b[-1]
        a_vista = dentro(ret, largura, altura)
        rolou = None
        if not a_vista:
            rolar_miolo(r, largura, altura, celular)
            b2 = botoes(r, "Receber")
            rolou = b2[-1] if b2 else None
            a_vista = bool(rolou) and dentro(rolou, largura, altura)
        f2 = r.foto("-3")
        ok(f"{rotulo}: o botão Receber fica inteiro dentro da janela (à vista ou rolando o miolo da janelinha)", a_vista,
           f"{f2} janela {largura}×{altura}; botão {tuple(round(v) for v in ret)}"
           + ("" if rolou is None else f" → depois de rolar {tuple(round(v) for v in rolou)}"))
        if nome_caso in MENSAGEM:
            # A mensagem do recebimento recusado tem de estar à vista (o miolo rolou até ela), e não escondida abaixo
            # da dobra do miolo. Medido logo depois da recusa, antes de qualquer rolagem do roteiro.
            ok(f"{rotulo}: a mensagem do recebimento recusado fica à vista, dentro do miolo", erro_visto[0],
               f"{f} {erro_visto[1]}")
        if nome_caso == "caixa fechado":
            # R8 da revisão da T040: a pessoa sobe o miolo e tenta de novo; a MESMA recusa (o texto não muda) também
            # tem de levar o miolo até a mensagem. (No "erro longo" não: a janela "Registro inconsistente" fica por
            # cima, pega o Enter e bloqueia a rolagem; ela mesma avisa.)
            rolar_miolo_ao_topo(r)
            escondida = not erro_a_vista(r, MENSAGEM[nome_caso])[0]
            f3 = r.foto("-4")
            tentar_receber(r, largura, altura, celular)
            time.sleep(1.5)
            de_novo = erro_a_vista(r, MENSAGEM[nome_caso])
            f4 = r.foto("-5")
            ok(f"{rotulo}: na segunda tentativa (mesma recusa), a mensagem volta à vista", de_novo[0],
               f"{f4} {de_novo[1]}; depois de subir o miolo a mensagem estava {'escondida' if escondida else 'ainda à vista'} ({f3})")
    finally:
        r.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(nav.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({nav.PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-altura-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    print("BINÁRIO:", nav.PROGRAMA, flush=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = nav.Placar()
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-altura-modelo-")
    try:
        nav.montar_loja(modelo)
        for nome_caso in CASOS:
            for nome_tam, celular, tamanho in TAMANHOS:
                print(f"--- {nome_caso} · {nome_tam}", flush=True)
                try:
                    caso(placar, fotos, modelo, nome_caso, nome_tam, celular, tamanho)
                except Exception as e:
                    placar.ok(f"{nome_caso} · {nome_tam} chegou ao fim ({e!r})", False)
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
