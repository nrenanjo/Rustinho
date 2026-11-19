#!/usr/bin/env python3
"""T047 (pedido do proprietário, 18/11/2026): no Android, as telas não têm a seta de voltar. O "voltar" do Android (o
botão e o gesto, que chegam ao Qt como a tecla Voltar, Qt.Key_Back) faz a mesma navegação, pelo mesmo caminho do Esc e
da seta (Main.qml: Keys.onBackPressed → janela.retornar → aoEsc da tela). O espaço da seta fica para a tela. No
computador, nada muda.

  S   O levantamento, no código (as duas cópias do QML): cada seta (Cabecalho, TelaVender, TelaJuntar e, pela decisão
      1a de 18/11/2026, a das janelinhas, Painel) só aparece fora do celular; as cópias são iguais.
  V1  Em cada tela com a seta: no computador, a seta está lá (só o desenho, quadrada); no celular, não está.
  V2  No celular, nas telas com o cabeçalho comum e sem o olho ("Mostrar valores"): o título começa no alto do
      cabeçalho (sem a linha da seta, o título sobe).
  V3  O voltar: no computador, a seta; no celular, a tecla Voltar. Os dois tiram a tela de cima (volta ao Início).
  C1  O voltar contextual no celular: na ficha de uma peça nova com algo digitado, a tecla Voltar pergunta "Descartar
      alterações?" (como a seta e o Esc), e a tela continua. A janelinha da pergunta: no celular sem a seta (1a), no
      computador com ela. O voltar seguinte fecha a janelinha, e a ficha continua.
Roda no computador e, num 2º processo, no desenho do celular (360 × 780). A tecla e o gesto no Android de verdade
ficam no emulador da CI (android/testar.sh) e no POCO.

    RUSTINHO_TELA=:88 python3 app/testes/telas_voltar_android.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o de antes, para a prova)
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, montar_loja, xdo  # noqa: E402

RAIZ = os.path.dirname(os.path.dirname(AQUI))
JANELA = "celular" if nav.CELULAR else "computador"
# As telas com a seta (o levantamento da T047), abertas pela abertura direta dos testes (por cima do Início). A
# TelaJuntar fica só no levantamento do código: o voltar dela desiste de juntar e troca a pilha inteira.
TELAS = ["TelaPecas", "TelaEstoque", "TelaPessoas", "TelaCompras", "TelaOrcamentos", "TelaOrdens", "TelaTrocas",
         "TelaFinanceiro", "TelaEtiquetas", "TelaConfiguracoes", "TelaFiscal", "TelaUsuarios", "TelaPainel",
         "TelaReceber", "TelaVender", "TelaCaixa", "TelaPeca", "TelaPessoa", "TelaOrdem", "TelaDevolucao",
         ]
# (A TelaVenda e a TelaCompra precisam de uma venda ou compra escolhida: abertas sozinhas, mostram um erro. Ficam de
# fora; usam o mesmo Cabecalho, conferido no levantamento.)
OLHO = ("Mostrar valores", "Esconder")


def escrito(tx):
    return not tx.startswith(("GEO:", "TOQUE:", "RET:", "TESTE_"))


def geo(t, nome):
    for _, _, tx in t:
        if tx.startswith(f"GEO:{nome}:"):
            return tuple(int(v) for v in tx.rsplit(":", 1)[1].split(","))
    return None


def setas(t):
    return [tuple(int(v) for v in tx.rsplit(":", 1)[1].split(",")) for _, _, tx in t
            if tx.startswith("GEO:setaDeRetorno:")]


def levantamento():
    """S: as setas das telas no código, nas duas cópias."""
    problemas = []
    for nome in ("Cabecalho.qml", "TelaVender.qml", "TelaJuntar.qml", "Painel.qml"):
        a = os.path.join(RAIZ, "app", "qml", nome)
        b = os.path.join(RAIZ, "android", "rust", "qml", nome)
        if open(a, encoding="utf-8").read() != open(b, encoding="utf-8").read():
            problemas.append(f"{nome}: as cópias do computador e do Android são diferentes")
    pasta = os.path.join(RAIZ, "app", "qml")
    for nome in sorted(os.listdir(pasta)):
        if not nome.endswith(".qml") or nome == "Seta.qml":
            continue
        texto = open(os.path.join(pasta, nome), encoding="utf-8").read()
        for m in re.finditer(r"^(\s*)Seta \{\n((?:\1    .*\n|\s*\n)*?)\1\}", texto, re.M):
            bloco = m.group(2)
            vis = re.search(r"^\s*visible: (.*)$", bloco, re.M)
            regra = vis.group(1) if vis else ""
            if "!Tema.celular" in regra:
                continue
            if nome == "Cabecalho.qml" and regra.strip() == "cabecalho.comSeta" and re.search(
                    r"readonly property bool comSeta: mostrarVoltar && !Tema\.celular", texto):
                continue
            if nome == "Painel.qml" and regra.strip() == "painel.mostraSeta" and re.search(
                    r"readonly property bool mostraSeta: comSeta && !Tema\.celular", texto):
                continue
            problemas.append(f"{nome}: uma seta sem a regra do celular (visible: {regra or 'nenhum'})")
    # S1-R5 da revisão Sonnet: o espaço da seta segue a regra (Tema.celular), e não a visibilidade efetiva da seta
    # (`seta.visible`), que fica falsa quando a tela é coberta na pilha e mexia no layout no computador também.
    for nome in sorted(os.listdir(pasta)):
        if nome.endswith(".qml") and re.search(r"\bseta\.visible\b", open(os.path.join(pasta, nome), encoding="utf-8").read()):
            problemas.append(f"{nome}: o espaço depende de seta.visible (a visibilidade efetiva), e não da regra do celular")
    contagem = sum(len(re.findall(r"^\s*Seta \{", open(os.path.join(pasta, n), encoding="utf-8").read(), re.M))
                   for n in os.listdir(pasta) if n.endswith(".qml") and n != "Seta.qml")
    return problemas, contagem


def esperar(condicao, segundos=15):
    fim = time.time() + segundos
    while time.time() < fim:
        v = condicao()
        if v:
            return v
        time.sleep(0.3)
    return None


def cenario_tela(placar, fotos, pasta, nome):
    ok = placar.ok
    r = placar.abrir(pasta, nome, {}, fotos, f"V-{nome}-{JANELA}")
    try:
        r.entrar("Ana")
        aberta = esperar(lambda: (r.pilha() or 0) >= 2, 20)
        f = r.foto("-0")
        if not ok(f"{nome}: abriu por cima do Início", aberta, f):
            return
        time.sleep(1.0)
        t = r.textos()
        s = setas(t)
        if nav.CELULAR:
            ok(f"V1 {nome}: no celular, sem a seta", not s, f"{f} setas={s}")
        else:
            ok(f"V1 {nome}: no computador, a seta está lá (só o desenho)", bool(s) and s[0][2] == s[0][3],
               f"{f} setas={s}")
        # A contagem que o programa dá no registro do Android (Main.setasVisiveis, a mesma função; o emulador lê por
        # ela, sem depender da árvore de acessibilidade): aqui, na leitura dos textos, tem de bater com as setas
        # vistas. No computador, acha a seta (o controle positivo da contagem); no celular, 0.
        contagem = next((int(tx.split(":")[1]) for _, _, tx in t if tx.startswith("TESTE_SETAS_VISIVEIS:")), None)
        if contagem is not None:
            ok(f"V1 {nome}: a contagem de setas do programa ({contagem}) bate com as setas vistas ({len(s)})",
               contagem == len(s) and (contagem == 0 if nav.CELULAR else contagem >= 1), f"{f} contagem={contagem}")
        cab = geo(t, "cabecalhoDaTela")
        if cab is None and s and nav.CELULAR:
            # Um programa de antes da T047 não dá o nome do cabeçalho: o alto dele é o da seta (o 1º item do
            # cabeçalho), e ele vai até 220 px abaixo (o título e o subtítulo). Assim a prova "antes" também mede.
            cab = (0, s[0][1], nav.CELULAR_DP[0], 220)
        if nav.CELULAR and cab:
            x0, y0, w0, h0 = cab
            dentro = [(y, tx) for x, y, tx in t if escrito(tx) and tx != "Voltar"
                      and x0 <= x <= x0 + w0 and y0 <= y <= y0 + h0]
            com_olho = any(tx in OLHO for _, tx in dentro)
            if dentro and not com_olho:
                y1, primeiro = min(dentro)
                print(f"MEDIDA V2 {nome}: o cabeçalho começa em y={y0}; o 1º texto (\"{primeiro}\") no meio em "
                      f"y={y1}", flush=True)
                ok(f"V2 {nome}: no celular, o título começa no alto do cabeçalho (sem a linha da seta)",
                   y1 - y0 <= 40, f"{f} cabeçalho y={y0}, \"{primeiro}\" y={y1}")
            elif com_olho:
                print(f"MEDIDA V2 {nome}: a linha de cima fica pelo olho", flush=True)
        # Antes de voltar, a tela parada, sem aviso no alto (algumas telas abrem com um aviso; quando ele some, a
        # tela volta ao lugar e o clique desse instante é descartado pela N6, como deve).
        esperar(lambda: (geo(r.textos(), "cliqueDescartadoNoMovimento") or (0, 0))[1] == 0, 8)
        time.sleep(0.5)
        antes = r.pilha()
        if nav.CELULAR:
            r.voltar_android()
        else:
            r.clicar_seta_de_cima()
        voltou = esperar(lambda: r.pilha() == antes - 1, 10)
        f = r.foto("-1-depois-do-voltar")
        jeito = "a tecla Voltar do Android" if nav.CELULAR else "a seta"
        ok(f"V3 {nome}: {jeito} tira a tela de cima (de {antes} para {antes - 1})", voltou,
           f"{f} pilha={r.pilha()}")
    finally:
        r.fechar()


def cenario_contexto(placar, fotos, pasta):
    """C1: na ficha de uma peça nova com algo digitado, o voltar pergunta antes de descartar."""
    ok = placar.ok
    r = placar.abrir(pasta, "TelaPeca", {}, fotos, f"C1-{JANELA}")
    try:
        r.entrar("Ana")
        if not ok("C1: a ficha da peça nova abriu", esperar(lambda: (r.pilha() or 0) >= 2, 20), r.foto("-0")):
            return
        time.sleep(1.0)
        if nav.CELULAR:
            # As linhas TOQUE: da medição do celular (r._medidas, guardadas pela leitura): o meio de cada campo ativo.
            r.textos()
            campos = sorted((y, x) for x, y, tx in getattr(r, "_medidas", [])
                            if tx.startswith("TOQUE:campo|") and tx.endswith("|1") and 60 <= y < 740)
            if not ok("C1: um campo da ficha à vista", campos, r.foto("-0")):
                return
            y, x = campos[0]
            xdo("mousemove", str(x), str(y), "click", "1")
        else:
            xdo("key", "Tab")
        time.sleep(0.4)
        xdo("type", "--delay", "60", "Teste")
        time.sleep(0.6)
        antes = r.pilha()
        if nav.CELULAR:
            r.voltar_android()
        else:
            r.clicar_seta_de_cima()
        perguntou = esperar(lambda: r.tem("Descartar"), 8)
        f = r.foto("-1")
        jeito = "a tecla Voltar do Android" if nav.CELULAR else "a seta"
        ok(f"C1: com algo digitado, {jeito} pergunta antes de descartar, e a ficha continua",
           perguntou and r.pilha() == antes, f"{f} pilha={r.pilha()}")
        if not perguntou:
            return
        time.sleep(0.6)
        t = r.textos()
        s = setas(t)
        if nav.CELULAR:
            ok("C1: no celular, a janelinha da pergunta sem a seta (decisão 1a)", not s and r.janelinha_aberta(t),
               f"{f} setas={s}")
            r.voltar_android()
        else:
            ok("C1: no computador, a janelinha da pergunta com a seta dela (e a da tela)", len(s) == 2, f"{f} setas={s}")
            r.clicar_seta_de_cima()
        fechou = esperar(lambda: not r.tem("Descartar") and not r.janelinha_aberta(), 8)
        f = r.foto("-2")
        ok(f"C1: {jeito} de novo fecha a janelinha, e a ficha continua (nada descartado)",
           fechou and r.pilha() == antes and r.tem("Teste"), f"{f} pilha={r.pilha()}")
    finally:
        r.fechar()


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-voltar-")
    os.makedirs(fotos, exist_ok=True)
    placar = Placar()
    print(f"janela: {'celular 360 × 780' if nav.CELULAR else 'computador'}", flush=True)
    if not nav.CELULAR:
        problemas, contagem = levantamento()
        print(f"MEDIDA S: {contagem} seta(s) no código (telas e janelinhas)", flush=True)
        placar.ok("S: no código, cada seta (telas e janelinhas) só aparece fora do celular, e as duas cópias são iguais",
                  not problemas and contagem >= 4, "; ".join(problemas) or f"{contagem} setas")
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-voltar-")
    try:
        montar_loja(pasta)
        for nome in TELAS:
            try:
                cenario_tela(placar, fotos, pasta, nome)
            except Exception as e:
                placar.ok(f"{nome} ({JANELA}) chegou ao fim ({e})", False)
        try:
            cenario_contexto(placar, fotos, pasta)
        except Exception as e:
            placar.ok(f"C1 ({JANELA}) chegou ao fim ({e})", False)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        placar.ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        shutil.rmtree(pasta, ignore_errors=True)
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO ({JANELA}): {placar.certos} certos, {placar.errados} errados")
    saida = 0 if placar.errados == 0 else 1
    if not nav.CELULAR and not os.environ.get("RUSTINHO_VOLTAR_SO_ESTE"):
        env = dict(os.environ, RUSTINHO_CASOS_CELULAR="1", RUSTINHO_VOLTAR_SO_ESTE="1")
        s = subprocess.run([sys.executable, "-B", os.path.abspath(__file__), os.path.join(fotos, "celular")], env=env)
        saida = saida or s.returncode
    if not os.environ.get("RUSTINHO_VOLTAR_SO_ESTE"):
        print("TUDO CERTO" if saida == 0 else "HOUVE FALHAS")
    sys.exit(saida)


if __name__ == "__main__":
    main()
