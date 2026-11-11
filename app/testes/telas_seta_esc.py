#!/usr/bin/env python3
"""Seta de retorno, Esc, janelinhas e Início (0.9.18: V-2, V-3, V-4, V-5, V-7).

  ESTATICO  No código: as 21 telas com Cabecalho usam a seta dele e nenhuma
            trata a seta à parte (onVoltar); a TelaVender usa a mesma seta;
            a seta e o Esc vão pelo mesmo caminho (janela.retornar); todas as
            janelinhas são Painel (com a seta); nenhum botão de texto que só
            volta ou só fecha fora da lista fechada (V-5, a tabela final);
            ícones da Início (Peças = chave inglesa, Ordens = prancheta);
            cópias do Android iguais.
  A6   Em cada tema: a seta no mesmo lugar (x, y) e com a mesma altura nas 21
       telas e na TelaVender, a cerca de 1 cm do canto (28 a 40 px), só a
       seta (área de toque quadrada, ≥ 40 × 40 px, sem texto ao lado: D-V1,
       correção definitiva do dono, 07/11/2026); nenhum "Cancelar", "Voltar"
       ou "Fechar" de texto na tela (fora a própria seta); uma foto de cada tela.
  A7   Seta × Esc: para cada tela, abre e aciona a seta; abre de novo e aperta
       Esc; os dois dão no mesmo (quantas telas na pilha e qual janelinha
       abriu). Também com alteração por salvar (Peça, Pessoa, Configurações,
       OS nova): os dois abrem a mesma pergunta e ficam na tela. D-V3
       (decidida pelo dono, 07/11/2026): com a busca preenchida, o Esc faz o
       mesmo que a seta, os dois voltam direto (conferido à parte).
  A8   Janelinhas: na peça alterada, a seta e o Esc da tela abrem "Descartar
       alterações?" (sem "Cancelar"); a seta da janelinha e o Esc fecham sem
       efeito (continua alterada, nada gravado); "Descartar" descarta. Mais
       três: senha do gerente (Painel), ajuste de estoque e sangria (eram
       Popup direto).
  A10  Início no roxo: xadrez com 4, 3, 2 e 1 colunas (o caso do roteiro do
       tema, telas_tema_0918.a10_xadrez).
  INICIO Em cada tema: colar à esquerda e copiar à direita dentro da busca, F11
       fora do campo; copiar e colar funcionam.
  A12  Nenhum TypeError/ReferenceError nos registros.

Abre o Rustinho de verdade numa tela virtual (Xvfb), com o ambiente neutro de
telas_d09.py (só X11, sem Wayland, sem o tema do desktop) e confere em /proc,
antes de qualquer clique, que ele está na tela virtual e sem Wayland
(telas_tema_0918.Rustinho). Nada abre na área de trabalho.

    RUSTINHO_TELA=:91 RUSTINHO_PROGRAMA=/caminho/rustinho \\
        python3 app/testes/telas_seta_esc.py [pasta das fotos]
    RUSTINHO_CASOS=ESTATICO,A6,A7,A8,A10,INICIO   (padrão: todos)
    RUSTINHO_TEMAS=cinza,amoled,roxo              (A6, INICIO e a foto do A8)
    RUSTINHO_TEMA_A7=cinza                        (tema das rodadas do A7)

Precisa de: Xvfb, xdotool, ImageMagick (import) e Pillow. Sai com 0 se tudo
conferiu.
"""

import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault("RUSTINHO_TELA", ":91")
import telas_d09 as base  # noqa: E402  (importar já deixa o ambiente neutro)
import telas_tema_0918 as tema  # noqa: E402

TELA = tema.TELA
PROGRAMA = tema.PROGRAMA
RAIZ = base.RAIZ
QML = os.path.join(RAIZ, "app", "qml")
simples = base.simples


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


# ---------------------------------------------------------------------------
# As telas
# ---------------------------------------------------------------------------

# As 21 telas com Cabecalho (espec §7) e a TelaVender (seta própria, a 22ª).
TELAS_CABECALHO = ["TelaCaixa", "TelaCompra", "TelaCompras", "TelaConfiguracoes", "TelaDevolucao", "TelaEstoque",
                   "TelaEtiquetas", "TelaFinanceiro", "TelaFiscal", "TelaOrcamentos", "TelaOrdem", "TelaOrdens",
                   "TelaPainel", "TelaPeca", "TelaPecas", "TelaPessoa", "TelaPessoas", "TelaReceber", "TelaTrocas",
                   "TelaUsuarios", "TelaVenda"]

# (tela, propriedades); FILTRO, ZE, COMPRA e VENDA viram os ids da loja de teste.
TELAS = [
    ("TelaCaixa", {}), ("TelaCompra", {"compraId": "COMPRA"}), ("TelaCompras", {}),
    ("TelaConfiguracoes", {"secao": "loja"}), ("TelaDevolucao", {"vendaId": "VENDA"}), ("TelaEstoque", {"secao": "comprar"}),
    ("TelaEtiquetas", {}), ("TelaFinanceiro", {"secao": "pagar"}), ("TelaFiscal", {}), ("TelaOrcamentos", {}),
    ("TelaOrdem", {}), ("TelaOrdens", {}), ("TelaPainel", {"secao": "resumo"}), ("TelaPeca", {"produtoId": "FILTRO"}),
    ("TelaPecas", {}), ("TelaPessoa", {"pessoaId": "ZE"}), ("TelaPessoas", {"papel": "cliente"}), ("TelaReceber", {}),
    ("TelaTrocas", {}), ("TelaUsuarios", {}), ("TelaVenda", {"vendaId": "VENDA"}), ("TelaVender", {}),
]

# D-V3: telas com busca; com ela preenchida, o Esc e a seta voltam direto (iguais).
TELAS_BUSCA = [("TelaPecas", {}), ("TelaPessoas", {"papel": "cliente"}), ("TelaCompras", {}), ("TelaOrcamentos", {}),
               ("TelaOrdens", {}), ("TelaTrocas", {}), ("TelaReceber", {}), ("TelaFinanceiro", {"secao": "pagar"}),
               ("TelaEstoque", {"secao": "comprar"}), ("TelaVender", {})]

# Janelinhas que a seta ou o Esc de uma tela podem abrir.
JANELINHAS = ["Descartar alterações?", "Descartar as regras alteradas?", "Sair da OS?", "Sair sem salvar?",
              "Limpar esta venda?"]


def ids_da_loja(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return {"FILTRO": c.execute("SELECT id FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0],
                "ZE": c.execute("SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'").fetchone()[0],
                "COMPRA": c.execute("SELECT id FROM compra ORDER BY rowid LIMIT 1").fetchone()[0],
                "VENDA": c.execute("SELECT id FROM venda ORDER BY rowid LIMIT 1").fetchone()[0]}
    finally:
        c.close()


def consulta(pasta, sql, *p):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute(sql, p).fetchall()
    finally:
        c.close()


class Rustinho(tema.Rustinho):
    def geo(self, nome, textos=None):
        """[(x, y, largura, altura)] dos itens com este objectName."""
        achados = []
        for _, _, t in (textos if textos is not None else self.textos()):
            if t.startswith("GEO:" + nome + ":"):
                achados.append(tuple(int(v) for v in t.rsplit(":", 1)[1].split(",")))
        return achados

    def pilha(self, textos=None):
        for _, _, t in (textos if textos is not None else self.textos()):
            if t.startswith("TESTE_PILHA:"):
                return int(t.split(":")[1])
        return None

    def tem_igual(self, texto, textos=None):
        return any(simples(texto) == simples(t) for _, _, t in (textos if textos is not None else self.textos()))

    def janelinha(self, textos=None):
        textos = textos if textos is not None else self.textos()
        for j in JANELINHAS:
            if self.tem_igual(j, textos):
                return j
        return ""

    def clicar_em(self, x, y):
        xdo("mousemove", str(x), str(y), "click", "1")

    def clicar_geo(self, g):
        self.clicar_em(g[0] + g[2] // 2, g[1] + g[3] // 2)

    def esperar(self, condicao, segundos=6):
        fim = time.time() + segundos
        while time.time() < fim:
            if condicao():
                return True
            time.sleep(0.4)
        return condicao()


def dentro(ponto, g, folga=0):
    x, y = ponto
    return g[0] - folga <= x <= g[0] + g[2] + folga and g[1] - folga <= y <= g[1] + g[3] + folga


def abrir(res, pasta, tela, props, fotos, nome, usuario="Ana"):
    r = Rustinho(pasta, tela, props, fotos, nome)
    res.logs.append(r.saida)
    r.entrar(usuario)
    time.sleep(1.5)
    return r


def resolver(props, ids):
    return {k: ids.get(v, v) if isinstance(v, str) else v for k, v in props.items()}


# ---------------------------------------------------------------------------
# Estático
# ---------------------------------------------------------------------------

# A tabela final dos botões de texto que ficam (V-5 e o resto do inventário):
# (arquivo, um texto do botão, um pedaço do onClicked, por quê).
FICAM = [
    # Correção definitiva do botão de retorno (dono, 07/11/2026): saíram o
    # "Cancelar" da restauração do backup, os dois das Configurações (caixa
    # novo, assumir as faixas), o "Voltar" do Juntar, os "Voltar à lista" e o
    # "Voltar ao carrinho" do celular; a seta e o Esc fazem esse retorno.
    ("SecaoEncomendas.qml", "Cancelar encomenda", "motivo.perguntar(\"Cancelar a encomenda", "cancela a encomenda (negócio)"),
    ("SecaoComissoes.qml", "Fechar", "fechamento.abrirCom(", "fecha a comissão (negócio)"),
    ("SecaoComissoes.qml", "Fechar e lançar", "fechamento.salvar()", "fecha e lança a comissão (negócio)"),
    ("TelaCompras.qml", "Desistir", "tela.pendentes = []", "descarta os arquivos esperando (descarta dados)"),
    ("TelaVenda.qml", "Cancelar venda", "tela.cancelar()", "abre o cancelamento da venda/orçamento (negócio)"),
    ("TelaVenda.qml", "Cancelar venda", "\"venda_cancelar\"", "confirma o cancelamento, com motivo e gerente (negócio)"),
    ("TelaOrdem.qml", "Cancelar OS", "pergunta.perguntar(", "cancela a OS (negócio)"),
    ("TelaOrdem.qml", "Voltar etapa", "tela.passar(", "volta a OS uma etapa (negócio)"),
    ("TelaOrdem.qml", "Sair sem salvar", "tela.alterado = false", "descarta a OS não salva (Descartar)"),
    ("TelaCaixa.qml", "Fechar caixa", "fechar.abrirCom()", "fecha o caixa (negócio)"),
    ("TelaCaixa.qml", "Fechar e imprimir", "\"caixa_fechar\"", "fecha o caixa e imprime (negócio)"),
    ("TelaReceber.qml", "Pronto", "feito.close()", "conclusão (Enter) do recibo; não é voltar nem cancelar"),
    ("TelaVender.qml", "Nova venda", "concluida.close()", "conclusão (Enter) da venda; não é voltar nem cancelar"),
    ("SecaoPagar.qml", "Pronto", "categorias.close()", "conclusão da edição das categorias (cada mudança já foi gravada)"),
]

ROTULO_DE_VOLTA = re.compile(r"^(Cancelar|Voltar|Fechar|Desistir|Sair|Continuar sem rascunho)")
SO_FECHA = re.compile(r"^\{?\s*((tela\.)?(janela\.)?(voltar|retornar)\([^)]*\)|tela\.aoEsc\(\)|\w+\.close\(\))"
                      r"(\s*[;\n]\s*((tela\.)?(janela\.)?(voltar|retornar)\([^)]*\)|tela\.aoEsc\(\)|\w+\.close\(\)))*\s*\}?$")


def bloco(texto, inicio):
    """O texto do bloco que abre em `inicio` (a posição do "{"), até o "}"."""
    nivel, i, aspas = 0, inicio, None
    while i < len(texto):
        c = texto[i]
        if aspas:
            if c == "\\":
                i += 2
                continue
            if c == aspas:
                aspas = None
        elif c in "\"'":
            aspas = c
        elif c == "{":
            nivel += 1
        elif c == "}":
            nivel -= 1
            if nivel == 0:
                return texto[inicio:i + 1]
        i += 1
    return texto[inicio:]


def botoes(arquivo):
    """[(linha, textos do botão, onClicked)] de cada Botao do arquivo."""
    texto = open(os.path.join(QML, arquivo), encoding="utf-8").read()
    lista = []
    for m in re.finditer(r"\bBotao\s*\{", texto):
        b = bloco(texto, m.end() - 1)
        corpo = b[1:-1]
        # Só as propriedades do próprio botão (blocos de dentro fora, menos o do onClicked).
        rotulo = re.search(r"(?:^|[;\n])\s*text:\s*([^\n;]*(?:\n\s*[?:][^\n]*)*)", corpo)
        textos = re.findall(r"\"([^\"]*)\"", rotulo.group(1)) if rotulo else []
        clique = ""
        k = re.search(r"onClicked:\s*", corpo)
        if k:
            resto = corpo[k.end():]
            if resto.startswith("{"):
                clique = bloco(resto, 0).strip()
            else:
                # Botao { text: "x"; onClicked: a.close() }: até o fim da linha.
                clique = resto.split("\n")[0].strip().rstrip("}").strip()
        lista.append((texto[:m.start()].count("\n") + 1, textos, clique))
    return lista


def estatico(res):
    arquivos = sorted(f for f in os.listdir(QML) if f.endswith(".qml"))
    conteudo = {f: open(os.path.join(QML, f), encoding="utf-8").read() for f in arquivos}

    com_cabecalho = sorted(f[:-4] for f, t in conteudo.items() if re.search(r"\bCabecalho\s*\{", t) and f != "Cabecalho.qml")
    res.conferir("ESTÁTICO: as 21 telas com Cabecalho são as da espec (§7)", com_cabecalho == sorted(TELAS_CABECALHO),
                 f"{len(com_cabecalho)}: {com_cabecalho}")
    trata = [f for f, t in conteudo.items() if "onVoltar" in t]
    res.conferir("ESTÁTICO: nenhuma tela trata a seta à parte (onVoltar)", not trata, str(trata))
    cab = conteudo["Cabecalho.qml"]
    res.conferir("ESTÁTICO: o Cabecalho usa a Seta e o caminho do Esc (retornar(true))",
                 "Seta {" in cab and "retornar(true)" in cab and "Botao" not in cab)
    vender = conteudo["TelaVender.qml"]
    res.conferir("ESTÁTICO: a TelaVender usa a mesma Seta, pelo mesmo caminho", "Seta {" in vender and "retornar(true)" in vender
                 and "icone: \"voltar\"\n                    text: \"Início\"" not in vender)
    main = conteudo["Main.qml"]
    esc = re.search(r"Keys\.onEscapePressed:[^}]*\}", main)
    volta = re.search(r"Keys\.onBackPressed:.*?\n        \}", main, re.S)
    res.conferir("ESTÁTICO: Esc e o voltar do Android vão por janela.retornar(false)",
                 bool(esc and "retornar(false)" in esc.group(0) and volta and "retornar(false)" in volta.group(0)))
    res.conferir("ESTÁTICO: D-V3 num lugar só (Main.escLimpaBuscaAntes), decidida: o Esc igual à seta",
                 len(re.findall(r"property bool escLimpaBuscaAntes", main)) == 1
                 and "readonly property bool escLimpaBuscaAntes: false" in main
                 and sum(t.count("janela.limparBuscaAntes(daSeta)") for t in conteudo.values()) == 10)
    tema_qml = open(os.path.join(RAIZ, "app", "base", "Tema.qml"), encoding="utf-8").read()
    # D-V1, decidida (correção definitiva do dono, 07/11/2026): só a seta, sem
    # texto, legenda ou nome de destino; D-V2 (a cor) continua em Tema.setaNo.
    seta_qml = conteudo["Seta.qml"]
    com_destino = sorted(f for f, t in conteudo.items()
                         if re.search(r"\b(textoVoltar|voltarPara|setaComTexto|destinoDaSeta)\b", t))
    res.conferir("ESTÁTICO: D-V1 decidida: a seta é só o desenho (sem texto nem destino) e D-V2 em Tema.setaNo",
                 "Text {" not in seta_qml and not re.search(r"^\s*text:", seta_qml, re.M)
                 and "property string destino" not in seta_qml and not com_destino
                 and "setaComTexto" not in tema_qml and "function setaNo(sup)" in tema_qml
                 and "readonly property int toqueSeta: celular ? 48 : 40" in tema_qml, str(com_destino))

    # Janelinhas: todas pelo Painel (que tem a seta); Popup só nas bases e menus.
    popups = {f: len(re.findall(r"\bPopup\s*\{", t)) for f, t in conteudo.items()}
    fora = {f: n for f, n in popups.items() if n and f not in ("Painel.qml", "Selecao.qml", "Campo.qml")}
    res.conferir("ESTÁTICO: nenhuma janelinha em Popup direto (todas pelo Painel, com a seta)", not fora, str(fora))
    res.conferir("ESTÁTICO: a seta está no Painel e o Dialogo é um Painel",
                 "Seta {" in conteudo["Painel.qml"] and re.search(r"^Painel\s*\{", conteudo["Dialogo.qml"], re.M) is not None)

    # Botões de texto que só voltam ou só fecham: só os da tabela final.
    achados, sobra = [], []
    usados = set()
    for f in arquivos:
        for linha, textos, clique in botoes(f):
            de_volta = any(ROTULO_DE_VOLTA.match(t) for t in textos)
            so_fecha = bool(clique) and bool(SO_FECHA.match(clique.replace("\n", ";")))
            if not (de_volta or so_fecha):
                continue
            ok = False
            for i, (arq, rotulo, pedaco, _) in enumerate(FICAM):
                if arq == f and rotulo in textos and pedaco in clique:
                    ok = True
                    usados.add(i)
            achados.append(f"{f}:{linha} {textos} → {clique[:60]!r}" + ("" if ok else "  ✘ FORA DA LISTA"))
            if not ok:
                sobra.append(f"{f}:{linha} {textos} ({clique[:50]})")
    print("   botões de texto de voltar/fechar encontrados:")
    for a in achados:
        print("     ", a)
    faltam = [f"{a} {r!r} ({p})" for i, (a, r, p, _) in enumerate(FICAM) if i not in usados]
    res.conferir(f"ESTÁTICO: nenhum botão de texto que só volta/fecha fora da lista fechada ({len(achados)} na lista)",
                 not sobra, "; ".join(sobra))
    res.conferir("ESTÁTICO: a tabela final confere com o código (nenhuma linha sobrando)", not faltam, "; ".join(faltam))

    # Início (D-V6) e ícones.
    inicio = conteudo["TelaInicio.qml"]
    res.conferir("ESTÁTICO: Início com colar/copiar dentro da busca e F11 fora",
                 "colarCopiar: true" in inicio and "atalho: \"F11\"" not in inicio and "text: \"F11\"" in inicio)
    icone = conteudo["Icone.qml"]
    res.conferir("ESTÁTICO: ícones novos no Icone.qml (colar, copiar, prancheta; chave inglesa = chave)",
                 all(f"\"{n}\":" in icone for n in ("colar", "copiar", "prancheta", "chave")))
    menu = open(os.path.join(RAIZ, "nucleo", "src", "servico", "menu.rs"), encoding="utf-8").read()
    pecas = re.search(r"id: \"pecas\",.*?icone: \"(\w+)\"", menu, re.S)
    oficina = re.search(r"id: \"oficina\",.*?icone: \"(\w+)\"", menu, re.S)
    res.conferir("ESTÁTICO: menu.rs: Peças = chave (inglesa), Ordens = prancheta",
                 pecas and pecas.group(1) == "chave" and oficina and oficina.group(1) == "prancheta",
                 f"Peças {pecas and pecas.group(1)}, Ordens {oficina and oficina.group(1)}")

    # Cópias do Android.
    diferentes = []
    for pasta in ("qml", "base"):
        a, b = os.path.join(RAIZ, "app", pasta), os.path.join(RAIZ, "android", "rust", pasta)
        if sorted(os.listdir(a)) != sorted(os.listdir(b)):
            diferentes.append(f"{pasta}: listas diferentes")
            continue
        for f in os.listdir(a):
            if open(os.path.join(a, f), "rb").read().replace(b"\r", b"") != open(os.path.join(b, f), "rb").read().replace(b"\r", b""):
                diferentes.append(f"{pasta}/{f}")
    res.conferir("ESTÁTICO: cópias do Android iguais (qml e base)", not diferentes, str(diferentes))


# ---------------------------------------------------------------------------
# A6 — a seta no mesmo lugar, em cada tema
# ---------------------------------------------------------------------------

def a6_seta_no_lugar(res, fotos, modelo, ids, temas):
    for nome_tema in temas:
        pasta = tema.nova_loja(modelo, nome_tema)
        lugares = {}
        try:
            for n, (tela, props) in enumerate(TELAS, 1):
                r = abrir(res, pasta, tela, resolver(props, ids), fotos, f"a6-{nome_tema}-{n:02d}-{tela}")
                try:
                    textos = r.textos()
                    setas = r.geo("setaDeRetorno", textos)
                    foto = r.foto_janela()
                    if len(setas) != 1:
                        res.conferir(f"A6 {nome_tema} {tela}: uma seta na tela", False, f"achei {len(setas)} (foto {foto})")
                        continue
                    g = setas[0]
                    lugares[tela] = g
                    # Texto que só volta: "Cancelar", "Voltar", "Fechar" fora da própria seta.
                    soltos = [t for x, y, t in textos if simples(t) in ("cancelar", "voltar", "fechar")
                              and not dentro((x, y), g, 2)]
                    # Só a seta (quadrada, sem texto ao lado), a cerca de 1 cm do canto.
                    res.conferir(f"A6 {nome_tema} {tela}: só a seta em ({g[0]}, {g[1]}), {g[2]}×{g[3]}; sem botão de texto que só volta",
                                 g[2] >= 40 and g[3] >= 40 and g[2] == g[3] and 28 <= g[0] <= 40 and 24 <= g[1] <= 40
                                 and not soltos, f"soltos {soltos}" if soltos else "")
                finally:
                    r.fechar()
        finally:
            shutil.rmtree(pasta, ignore_errors=True)
        if lugares:
            xs = {g[0] for g in lugares.values()}
            ys = {g[1] for g in lugares.values()}
            hs = {g[3] for g in lugares.values()}
            res.conferir(f"A6 {nome_tema}: a seta no mesmo lugar nas {len(lugares)} telas (x, y e altura iguais)",
                         len(lugares) == len(TELAS) and len(xs) == 1 and len(ys) == 1 and len(hs) == 1,
                         f"x {sorted(xs)}, y {sorted(ys)}, altura {sorted(hs)}")


# ---------------------------------------------------------------------------
# A7 — seta × Esc
# ---------------------------------------------------------------------------

def estado(r):
    textos = r.textos()
    return r.pilha(textos), r.janelinha(textos)


def a7_seta_e_esc(res, fotos, modelo, ids, nome_tema):
    pasta = tema.nova_loja(modelo, nome_tema)
    try:
        for n, (tela, props) in enumerate(TELAS, 1):
            props = resolver(props, ids)
            resultados = {}
            for como in ("seta", "esc"):
                r = abrir(res, pasta, tela, props, fotos, f"a7-{n:02d}-{tela}-{como}")
                try:
                    antes = r.pilha()
                    if como == "seta":
                        setas = r.geo("setaDeRetorno")
                        if len(setas) != 1:
                            res.conferir(f"A7 {tela}: achei a seta", False, f"{len(setas)} setas")
                            break
                        r.clicar_geo(setas[0])
                    else:
                        xdo("key", "Escape")
                    time.sleep(1.2)
                    resultados[como] = (antes,) + estado(r)
                    r.foto_janela()
                finally:
                    r.fechar()
            if len(resultados) == 2:
                s, e = resultados["seta"], resultados["esc"]
                res.conferir(f"A7 {tela}: seta → pilha {s[0]}→{s[1]}{' + ' + s[2] if s[2] else ''}; "
                             f"Esc → pilha {e[0]}→{e[1]}{' + ' + e[2] if e[2] else ''}", s == e)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# Seta × Esc com alteração por salvar: os dois têm de abrir a mesma pergunta.
# (tela, propriedades, texto do campo a mudar ou None = o foco já está num
# campo, a pergunta esperada)
TELAS_ALTERADAS = [
    ("TelaPeca", {"produtoId": "FILTRO"}, "Filtro de óleo", "Descartar alterações?"),
    ("TelaPessoa", {"pessoaId": "ZE"}, "Zé da Oficina", "Descartar alterações?"),
    ("TelaConfiguracoes", {"secao": "loja"}, "Loja Teste Auto Peças", "Descartar alterações?"),
    ("TelaOrdem", {}, None, "Sair da OS?"),
]


def a7_com_alteracao(res, fotos, modelo, ids, nome_tema):
    pasta = tema.nova_loja(modelo, nome_tema)
    try:
        for n, (tela, props, campo, pergunta) in enumerate(TELAS_ALTERADAS, 1):
            props = resolver(props, ids)
            resultados = {}
            for como in ("seta", "esc"):
                r = abrir(res, pasta, tela, props, fotos, f"a7-alterada-{n:02d}-{tela}-{como}")
                try:
                    if campo:
                        pos = [(x, y) for x, y, t in r.textos() if t == campo]
                        if not pos:
                            raise RuntimeError(f"{tela}: não achei o campo \"{campo}\" (veja {r.saida})")
                        r.clicar_em(*pos[-1])
                        xdo("key", "End")
                        xdo("type", "--delay", "60", " X")
                    else:
                        time.sleep(0.8)
                        xdo("type", "--delay", "60", "ABC1D23")
                    time.sleep(0.8)
                    antes = r.pilha()
                    if como == "seta":
                        r.clicar_geo(r.geo("setaDeRetorno")[0])
                    else:
                        xdo("key", "Escape")
                    time.sleep(1.2)
                    resultados[como] = (antes,) + estado(r)
                    r.foto_janela()
                finally:
                    r.fechar()
            s, e = resultados["seta"], resultados["esc"]
            res.conferir(f"A7 {tela} alterada: seta → pilha {s[0]}→{s[1]} + \"{s[2]}\"; Esc → pilha {e[0]}→{e[1]} + \"{e[2]}\"",
                         s == e and s[2] == pergunta and s[1] == s[0])
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def a7_dv3_busca(res, fotos, modelo, ids, nome_tema):
    """D-V3, decidida pelo dono (07/11/2026): com a busca preenchida, o Esc faz
    exatamente o mesmo que a seta: os dois voltam direto, sem janelinha."""
    pasta = tema.nova_loja(modelo, nome_tema)
    try:
        for n, (tela, props) in enumerate(TELAS_BUSCA, 1):
            props = resolver(props, ids)
            r1 = {}
            for como in ("esc", "seta"):
                r = abrir(res, pasta, tela, props, fotos, f"a7-dv3-{n:02d}-{tela}-{como}")
                try:
                    antes = r.pilha()
                    busca = r.geo("campoDeBusca")
                    if not busca:
                        res.conferir(f"A7/D-V3 {tela}: achei a busca", False)
                        break
                    r.clicar_geo(busca[0])
                    xdo("key", "ctrl+a")
                    xdo("type", "--delay", "60", "zzqx")
                    time.sleep(0.8)
                    digitou = r.tem_igual("zzqx")
                    if como == "esc":
                        xdo("key", "Escape")
                    else:
                        r.clicar_geo(r.geo("setaDeRetorno")[0])
                    time.sleep(1.2)
                    textos = r.textos()
                    r1[como] = (digitou, antes, r.pilha(textos), r.tem_igual("zzqx", textos), r.janelinha(textos))
                    r.foto_janela()
                finally:
                    r.fechar()
            if len(r1) == 2:
                e, s = r1["esc"], r1["seta"]
                res.conferir(f"A7/D-V3 {tela}: busca preenchida → o Esc (pilha {e[1]}→{e[2]}) e a seta "
                             f"(pilha {s[1]}→{s[2]}) voltam direto, iguais",
                             e[0] and s[0] and e[2] == e[1] - 1 and s[2] == s[1] - 1 and not e[4] and not s[4],
                             f"esc {e}, seta {s}")
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# A8 — janelinhas
# ---------------------------------------------------------------------------

def seta_da_janelinha(r, textos=None):
    """A seta que não é a do cabeçalho: a de cima de tudo (a janelinha)."""
    setas = r.geo("setaDeRetorno", textos)
    return setas[-1] if len(setas) >= 2 else None


def a8_peca(res, fotos, modelo, ids, nome_tema, completo=True):
    """O caso de comoficará.png: "Descartar alterações?" na peça alterada."""
    pasta = tema.nova_loja(modelo, nome_tema)
    filtro = ids["FILTRO"]
    nome_antes = consulta(pasta, "SELECT nome FROM produto WHERE id = ?", filtro)[0][0]
    try:
        r = abrir(res, pasta, "TelaPeca", {"produtoId": filtro}, fotos, f"a8-{nome_tema}-peca")
        try:
            pos = [(x, y) for x, y, t in r.textos() if t == nome_antes]
            if not pos:
                raise RuntimeError(f"não achei o campo com o nome da peça (veja {r.saida})")
            r.clicar_em(*pos[-1])
            xdo("key", "End")
            xdo("type", "--delay", "60", " X")
            time.sleep(0.8)
            res.conferir(f"A8 {nome_tema} peça: alterada (\"Não salvo\")", r.tem_igual("Não salvo"))
            pilha = r.pilha()
            cab = r.geo("setaDeRetorno")[0]
            r.clicar_geo(cab)
            time.sleep(1)
            textos = r.textos()
            foto = r.foto_janela("-1-descartar")
            res.conferir(f"A8 {nome_tema} peça: a seta abre \"Descartar alterações?\" com a seta no canto e sem \"Cancelar\"",
                         r.tem_igual("Descartar alterações?", textos) and r.tem_igual("Descartar", textos)
                         and not r.tem_igual("Cancelar", textos) and seta_da_janelinha(r, textos) is not None,
                         f"foto {foto}")
            if not completo:
                return
            j = seta_da_janelinha(r, textos)
            res.conferir("A8 peça: a seta da janelinha fica no canto de cima, à esquerda (antes do título)",
                         j is not None and [y for x, y, t in textos if t == "Descartar alterações?"][-1] > j[1] + j[3] // 2)
            r.clicar_geo(j)
            time.sleep(1)
            textos = r.textos()
            r.foto_janela("-2-seta-fechou")
            res.conferir("A8 peça: a seta da janelinha fecha sem efeito (continua alterada, na mesma tela, nada gravado)",
                         not r.tem_igual("Descartar alterações?", textos) and r.tem_igual("Não salvo", textos)
                         and r.pilha(textos) == pilha
                         and consulta(pasta, "SELECT nome FROM produto WHERE id = ?", filtro)[0][0] == nome_antes)
            xdo("key", "Escape")
            time.sleep(1)
            res.conferir("A8 peça: o Esc na tela abre a mesma pergunta (igual à seta)", r.tem_igual("Descartar alterações?"))
            xdo("key", "Escape")
            time.sleep(1)
            textos = r.textos()
            r.foto_janela("-3-esc-fechou")
            res.conferir("A8 peça: o Esc fecha a janelinha sem efeito (continua alterada, nada gravado)",
                         not r.tem_igual("Descartar alterações?", textos) and r.tem_igual("Não salvo", textos)
                         and r.pilha(textos) == pilha
                         and consulta(pasta, "SELECT nome FROM produto WHERE id = ?", filtro)[0][0] == nome_antes)
            r.clicar_geo(r.geo("setaDeRetorno")[0])
            time.sleep(1)
            r.clicar("Descartar")
            time.sleep(1.2)
            textos = r.textos()
            r.foto_janela("-4-descartou")
            res.conferir("A8 peça: \"Descartar\" descarta (sai da peça; nada gravado)",
                         r.pilha(textos) == pilha - 1 and not r.tem_igual("Descartar alterações?", textos)
                         and consulta(pasta, "SELECT nome FROM produto WHERE id = ?", filtro)[0][0] == nome_antes,
                         f"pilha {r.pilha(textos)}")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def a8_janelinha(res, fotos, pasta, nome, tela, props, usuario, abrir_com, marca, contar):
    """Abre a janelinha (clicando em `abrir_com`), fecha pela seta dela; abre de
    novo e fecha com Esc. Nos dois, nada muda (`contar` no banco) e a tela fica."""
    r = abrir(res, pasta, tela, props, fotos, f"a8-{nome}", usuario)
    try:
        antes = contar()
        pilha = r.pilha()
        for como in ("seta", "esc"):
            if not r.clicar(abrir_com):
                raise RuntimeError(f"não achei \"{abrir_com}\" (veja {r.saida})")
            aberta = r.esperar(lambda: any(simples(marca) in simples(t) for _, _, t in r.textos()))
            textos = r.textos()
            j = seta_da_janelinha(r, textos)
            foto = r.foto_janela(f"-{como}-1-aberta")
            res.conferir(f"A8 {nome}: abriu, com a seta no canto", aberta and j is not None, f"foto {foto}")
            if j is None:
                return
            if como == "seta":
                r.clicar_geo(j)
            else:
                xdo("key", "Escape")
            time.sleep(1)
            textos = r.textos()
            r.foto_janela(f"-{como}-2-fechou")
            res.conferir(f"A8 {nome}: {'a seta' if como == 'seta' else 'o Esc'} fecha sem efeito",
                         not any(simples(marca) in simples(t) for _, _, t in textos) and r.pilha(textos) == pilha
                         and contar() == antes, f"antes {antes}, depois {contar()}")
    finally:
        r.fechar()


def a8_outras(res, fotos, modelo, ids, nome_tema):
    pasta = tema.nova_loja(modelo, nome_tema)
    filtro = ids["FILTRO"]
    try:
        a8_janelinha(res, fotos, pasta, "senha-do-gerente", "TelaConfiguracoes", {}, "Bia", "Senha do gerente",
                     "você só consulta", lambda: consulta(pasta, "SELECT count(*) FROM config")[0][0])
        a8_janelinha(res, fotos, pasta, "ajuste-de-estoque", "TelaPeca", {"produtoId": filtro}, "Ana", "Ajustar",
                     "Quantidade contada agora",
                     lambda: consulta(pasta, "SELECT count(*), coalesce(sum(quantidade), 0) FROM estoque_movimento "
                                             "WHERE produto_id = ?", filtro)[0])
        a8_janelinha(res, fotos, pasta, "sangria", "TelaCaixa", {}, "Ana", "Sangria",
                     "Dinheiro que sai da gaveta",
                     lambda: consulta(pasta, "SELECT count(*) FROM caixa_movimento")[0][0])
    except (RuntimeError, sqlite3.Error) as e:
        res.conferir("A8 outras janelinhas terminaram", False, str(e))
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# Início (D-V6): colar, copiar e F11
# ---------------------------------------------------------------------------

def inicio(res, fotos, modelo, nome_tema, cores):
    pasta = tema.nova_loja(modelo, nome_tema)
    try:
        r = abrir(res, pasta, "Nenhuma", {}, fotos, f"inicio-{nome_tema}")
        try:
            textos = r.textos()
            foto = r.foto_janela("-1")
            campo = r.geo("buscaDaInicio", textos)
            pos = {t: (x, y) for x, y, t in textos if t in ("Colar na busca", "Copiar a busca", "F11")}
            if not campo or len(pos) < 3:
                res.conferir(f"INICIO {nome_tema}: achei a busca, colar, copiar e F11", False, f"{campo} {pos} (foto {foto})")
                return
            g = campo[0]
            cx, cy = pos["Colar na busca"]
            px, py = pos["Copiar a busca"]
            fx, fy = pos["F11"]
            res.conferir(f"INICIO {nome_tema}: colar dentro da busca, à esquerda", dentro((cx, cy), g) and cx < g[0] + 60,
                         f"colar ({cx}, {cy}); busca {g}")
            res.conferir(f"INICIO {nome_tema}: copiar dentro da busca, à direita", dentro((px, py), g) and px > g[0] + g[2] - 60,
                         f"copiar ({px}, {py})")
            res.conferir(f"INICIO {nome_tema}: F11 fora do campo, à direita, na mesma linha", fx > g[0] + g[2] and abs(fy - (g[1] + g[3] // 2)) <= 6,
                         f"F11 ({fx}, {fy})")
            if nome_tema == "roxo":
                tema.amostra(res, "INICIO roxo: fundo da busca (A3)", foto, px - 60, py, cores["buscaFundo"])
            # Copiar e colar.
            r.clicar_geo(g)
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "60", "8PK1515")
            time.sleep(0.5)
            r.clicar_em(px, py)
            time.sleep(0.8)
            r.clicar_geo(g)
            xdo("key", "ctrl+a")
            xdo("key", "BackSpace")
            time.sleep(0.5)
            vazio = not r.tem_igual("8PK1515")
            r.clicar_em(cx, cy)
            time.sleep(0.8)
            textos = r.textos()
            r.foto_janela("-2-colou")
            res.conferir(f"INICIO {nome_tema}: copiar e colar pelos botões da busca", vazio and r.tem_igual("8PK1515", textos))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------

def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    casos = os.environ.get("RUSTINHO_CASOS", "ESTATICO,A6,A7,A8,A10,INICIO").split(",")
    temas = os.environ.get("RUSTINHO_TEMAS", "cinza,amoled,roxo").split(",")
    tema_a7 = os.environ.get("RUSTINHO_TEMA_A7", "cinza")
    # RUSTINHO_SO_TELAS=TelaPecas,TelaPeca: só essas (rodadas curtas).
    so = [t for t in os.environ.get("RUSTINHO_SO_TELAS", "").split(",") if t]
    if so:
        TELAS[:] = [t for t in TELAS if t[0] in so]
        TELAS_BUSCA[:] = [t for t in TELAS_BUSCA if t[0] in so]
    res = tema.Resultado()
    if "ESTATICO" in casos:
        estatico(res)
    tela_casos = [c for c in casos if c != "ESTATICO"]
    if tela_casos:
        if not os.path.exists(PROGRAMA):
            sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
        fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-seta-")
        os.makedirs(fotos, exist_ok=True)
        modelo = tempfile.mkdtemp(prefix="rustinho-loja-seta-modelo-")
        xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
        time.sleep(1)
        try:
            if xvfb.poll() is not None:
                sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
            base.montar_loja(modelo)
            ids = ids_da_loja(modelo)
            cores = tema.tokens_do_roxo()
            for caso, funcao in [
                ("A6", lambda: a6_seta_no_lugar(res, fotos, modelo, ids, temas)),
                ("A7", lambda: (a7_seta_e_esc(res, fotos, modelo, ids, tema_a7), a7_com_alteracao(res, fotos, modelo, ids, tema_a7),
                                a7_dv3_busca(res, fotos, modelo, ids, tema_a7))),
                ("A7ALT", lambda: a7_com_alteracao(res, fotos, modelo, ids, tema_a7)),
                ("A8", lambda: ([a8_peca(res, fotos, modelo, ids, t, completo=(t == tema_a7)) for t in temas],
                                a8_outras(res, fotos, modelo, ids, tema_a7))),
                ("A10", lambda: tema.a10_xadrez(res, fotos, modelo, cores)),
                ("INICIO", lambda: [inicio(res, fotos, modelo, t, cores) for t in temas]),
            ]:
                if caso in casos:
                    try:
                        funcao()
                    except Exception as e:  # um caso que quebra não para os outros
                        res.conferir(f"{caso} terminou", False, f"{type(e).__name__}: {e}")
        finally:
            xvfb.terminate()
            xvfb.wait(10)
            shutil.rmtree(modelo, ignore_errors=True)
        print("fotos em", fotos)
        erros = [e for caminho in res.logs for e in tema.erros_de_script(caminho)]
        res.conferir(f"A12 zero TypeError/ReferenceError em {len(res.logs)} registros", not erros, "; ".join(erros[:5]))
    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha")
    for f in res.falhas:
        print("  ✘", f)
    sys.exit(0 if not res.falhas else 1)


if __name__ == "__main__":
    main()
