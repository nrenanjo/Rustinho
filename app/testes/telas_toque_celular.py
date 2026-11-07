#!/usr/bin/env python3
"""Teste das telas no desenho do celular (0.9.18): A15 e PEND-17.

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros testes das telas (`telas_d09.ambiente_neutro`), com as opções dos
testes `--celular` (o QML desenha como no Android: `Tema.celular`) e
`--medir-toque` (cada controle escreve onde o dedo o aciona). A janela fica
do tamanho de um celular em pé (padrão 360 × 780 dp) e com a densidade de um
celular (padrão 2: cada dp vale 2 pixels, como num aparelho xhdpi).

  A15.  Todo controle que se toca (botões, caixas de marcar, listas, campos,
        MouseArea e TapHandler, com a margem deles) tem área de toque de pelo
        menos 48 × 48 dp, em cada tela da lista. As medidas vêm do próprio
        programa, em pixels lógicos (no Android, 1 pixel lógico = 1 dp).
  PEND-17. Na tela da compra (item ainda sem peça), os botões da linha do
        item (Escolher peça, Cadastrar nova, Não lançar) não ficam por cima
        do texto "Que peça é esta?" nem o espremem.
  A12.  Nenhum TypeError/ReferenceError nos registros.

O aparelho de verdade não é usado aqui (sem SDK nem aparelho): isto mede o
desenho do celular no computador. Com densidade maior que 1, o programa usa o
desenho padrão (OpenGL do Mesa na tela virtual), e não o "software" dos
outros roteiros, que não desenha os ícones (Shape com layer) nessa escala.

    python3 app/testes/telas_toque_celular.py [pasta para as fotos]
    RUSTINHO_TELA=:60 RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_toque_celular.py
    RUSTINHO_TEMA=roxo   (tema das telas; padrão cinza)
    RUSTINHO_DENSIDADE=2 RUSTINHO_CELULAR_DP=360x780
    RUSTINHO_TOQUE_TELAS=pecas,compra-pendente   (só essas)
    RUSTINHO_TOQUE_SO_MEDIR=1   (só o inventário, sem falhar: para a base)

Precisa de: Xvfb, xdotool, ImageMagick (import) e Pillow. Sai com 0 se tudo
conferiu.
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402  (importar já deixa o ambiente neutro)
import telas_tema_0918 as tema  # noqa: E402  (o Rustinho que confere o ambiente)

TELA = os.environ.get("RUSTINHO_TELA", ":60")
PROGRAMA = os.environ.get("RUSTINHO_PROGRAMA", base.PROGRAMA)
base.TELA = tema.TELA = TELA
base.PROGRAMA = tema.PROGRAMA = PROGRAMA
DENSIDADE = float(os.environ.get("RUSTINHO_DENSIDADE", "2"))
LARGURA_DP, ALTURA_DP = (int(v) for v in os.environ.get("RUSTINHO_CELULAR_DP", "360x780").split("x"))
TEMA = os.environ.get("RUSTINHO_TEMA", "cinza")
MINIMO = 48.0
# Folga de arredondamento (as medidas vêm com uma casa decimal).
FOLGA = 0.05


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


class Celular(tema.Rustinho):
    """O Rustinho com o desenho do celular, conferindo o ambiente antes de
    tocar nele (DISPLAY da tela virtual, sem Wayland, xcb)."""

    def __init__(self, pasta, tela, props, fotos, nome):
        self.fotos = fotos
        self.nome = nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true",
                   QT_SCALE_FACTOR=str(DENSIDADE))
        env.pop("WAYLAND_DISPLAY", None)
        # Desenho: o "software" dos outros roteiros não desenha os ícones
        # (Shape com layer) com escala diferente de 1; com densidade > 1 fica o
        # desenho padrão (OpenGL do Mesa na tela virtual).
        if DENSIDADE == 1 or os.environ.get("RUSTINHO_DESENHO") == "software":
            env["QT_QUICK_BACKEND"] = "software"
        else:
            env.pop("QT_QUICK_BACKEND", None)
        self.log = open(self.saida, "w")
        self.p = subprocess.Popen(
            [PROGRAMA, "--textos-da-tela", "--celular", "--medir-toque", "--abrir", tela, "--props", json.dumps(props)],
            env=env, stdout=self.log, stderr=self.log,
        )
        self.conferir_ambiente()
        self.esperar_janela()
        # Em pé, do tamanho de um celular (a janela abre com 1280 × 760 dp).
        self.redimensionar(round(LARGURA_DP * DENSIDADE), round(ALTURA_DP * DENSIDADE))

    def esperar_janela(self):
        for _ in range(150):
            s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            if s.stdout.split():
                time.sleep(0.5)
                return
            time.sleep(0.2)
        raise RuntimeError(f"a janela do Rustinho não apareceu (veja {self.saida})")

    def ponteiro_dentro(self):
        """Sem gerenciador de janelas, o teclado vai para a janela embaixo
        do ponteiro: antes de cada tecla, o ponteiro no canto de baixo, à
        direita, da janela (fundo da tela, sem nada embaixo)."""
        wx, wy, w, h = tema.janela_do_rustinho()
        xdo("mousemove", str(wx + w - 4), str(wy + h - 4))
        time.sleep(0.15)

    def textos(self):
        self.ponteiro_dentro()
        return super().textos()

    def tela_fisica(self, x, y):
        wx, wy, _, _ = tema.janela_do_rustinho()
        return round(wx + x * DENSIDADE), round(wy + y * DENSIDADE)

    def clicar(self, texto, comeca=False):
        for x, y, t in self.textos():
            if base.simples(t) == base.simples(texto) or (comeca and base.simples(t).startswith(base.simples(texto))):
                px, py = self.tela_fisica(x, y)
                xdo("mousemove", str(px), str(py), "click", "1")
                return True
        return False

    def foto_janela(self, sufixo=""):
        """Foto só da janela (o mouse fora dela)."""
        xdo("mousemove", "1590", "5")
        time.sleep(0.6)
        inteira = self.foto(sufixo + "-tela")
        x, y, w, h = tema.janela_do_rustinho()
        from PIL import Image
        caminho = os.path.join(self.fotos, self.nome + sufixo + ".png")
        Image.open(inteira).crop((x, y, x + w, y + h)).save(caminho)
        os.remove(inteira)
        return caminho

    def medidas(self):
        """(toques, retangulos) do que está na tela agora."""
        with open(self.saida) as f:
            antes = f.read().count("TEXTOS-DA-TELA-FIM")
        self.ponteiro_dentro()
        xdo("key", "ctrl+shift+F12")
        for _ in range(60):
            time.sleep(0.2)
            with open(self.saida) as f:
                tudo = f.read()
            if tudo.count("TEXTOS-DA-TELA-FIM") > antes:
                break
        else:
            raise RuntimeError(f"o Rustinho não escreveu os textos da tela (veja {self.saida})")
        bloco = tudo.split("TEXTOS-DA-TELA-INICIO")[-1].split("TEXTOS-DA-TELA-FIM")[0]
        toques, rets = [], []
        for linha in bloco.splitlines():
            p = linha.split("\t", 2)
            if len(p) != 3:
                continue
            if p[2].startswith("TOQUE:"):
                tipo, comp, classe, nome, ret, ativo = p[2][6:].split("|")
                x, y, w, h = (float(v) for v in ret.split(","))
                toques.append(dict(tipo=tipo, componente=comp, classe=classe, nome=nome, x=x, y=y, w=w, h=h,
                                   ativo=ativo == "1"))
            elif p[2].startswith("RET:"):
                ret, alinhamento, comp, texto = p[2][4:].split("|", 3)
                v = [float(a) for a in ret.split(",")]
                rets.append(dict(x=v[0], y=v[1], w=v[2], h=v[3], cw=v[4], ch=v[5], natural=v[6],
                                 alinhamento=int(alinhamento), componente=comp, texto=texto))
        return toques, rets


# ---- A loja de teste ----

def compra_pendente(pasta):
    """Uma nota de compra ainda em rascunho, com os itens sem peça ("Que peça
    é esta?"): cópia da compra da loja de teste, pelo núcleo (por fora)."""
    sql = """
    INSERT INTO compra (id, fornecedor_id, chave, numero, serie, emissao, natureza, total_produtos, total,
                        situacao, destinatario, criado_em, criado_por, dispositivo)
      SELECT 'compra-pend17', fornecedor_id, '', numero || '7', serie, emissao, natureza, total_produtos, total,
             'rascunho', destinatario, criado_em, criado_por, dispositivo
      FROM compra ORDER BY rowid LIMIT 1;
    INSERT INTO compra_item (id, compra_id, ordem, codigo_fornecedor, gtin, descricao, ncm, cest, cfop, unidade,
                             quantidade, valor_produtos, acao, produto_id, achado_por, fator)
      SELECT id || '-p17', 'compra-pend17', ordem, codigo_fornecedor, gtin, descricao, ncm, cest, cfop, unidade,
             quantidade, valor_produtos, '', NULL, '', fator
      FROM compra_item WHERE compra_id = (SELECT id FROM compra WHERE id <> 'compra-pend17' ORDER BY rowid LIMIT 1);
    """
    tema.por_fora(pasta, sql)
    return "compra-pend17"


def ids_da_loja(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        um = lambda sql: (c.execute(sql).fetchone() or [""])[0]  # noqa: E731
        return {
            "FILTRO": um("SELECT id FROM produto WHERE nome = 'Filtro de óleo'"),
            "ZE": um("SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'"),
            "VENDA": um("SELECT id FROM venda WHERE situacao = 'concluida' ORDER BY rowid LIMIT 1"),
            "COMPRA": um("SELECT id FROM compra WHERE id <> 'compra-pend17' ORDER BY rowid LIMIT 1"),
        }
    finally:
        c.close()


# (nome, tela, propriedades). "Nenhuma" = Início; "Entrada" = a tela de
# entrada, medida antes de entrar.
TELAS = [
    ("entrada", "Entrada", {}),
    ("inicio", "Nenhuma", {}),
    ("vender", "TelaVender", {}),
    ("receber", "TelaReceber", {}),
    ("orcamentos", "TelaOrcamentos", {}),
    ("caixa", "TelaCaixa", {}),
    ("trocas", "TelaTrocas", {}),
    ("venda", "TelaVenda", {"vendaId": "VENDA"}),
    ("devolucao", "TelaDevolucao", {"vendaId": "VENDA"}),
    ("pecas", "TelaPecas", {}),
    ("peca-ficha", "TelaPeca", {"produtoId": "FILTRO"}),
    ("peca-nova", "TelaPeca", {}),
    ("clientes", "TelaPessoas", {"papel": "cliente"}),
    ("pessoa-ficha", "TelaPessoa", {"pessoaId": "ZE"}),
    ("fornecedores", "TelaPessoas", {"papel": "fornecedor"}),
    ("compras", "TelaCompras", {}),
    ("compra-lancada", "TelaCompra", {"compraId": "COMPRA"}),
    ("compra-pendente", "TelaCompra", {"compraId": "compra-pend17"}),
    ("etiquetas", "TelaEtiquetas", {}),
    ("estoque-comprar", "TelaEstoque", {"secao": "comprar"}),
    ("estoque-encomendas", "TelaEstoque", {"secao": "encomendas"}),
    ("estoque-vendidos", "TelaEstoque", {"secao": "vendidos"}),
    ("estoque-parados", "TelaEstoque", {"secao": "parados"}),
    ("ordens", "TelaOrdens", {}),
    ("ordem-nova", "TelaOrdem", {}),
    ("financeiro-pagar", "TelaFinanceiro", {"secao": "pagar"}),
    ("financeiro-contas", "TelaFinanceiro", {"secao": "contas"}),
    ("financeiro-cheques", "TelaFinanceiro", {"secao": "cheques"}),
    ("financeiro-fluxo", "TelaFinanceiro", {"secao": "fluxo"}),
    ("financeiro-comissoes", "TelaFinanceiro", {"secao": "comissoes"}),
    ("painel-resumo", "TelaPainel", {"secao": "resumo"}),
    ("painel-dre", "TelaPainel", {"secao": "dre"}),
    ("painel-pecas", "TelaPainel", {"secao": "pecas"}),
    ("painel-pagamentos", "TelaPainel", {"secao": "pagamentos"}),
    ("usuarios", "TelaUsuarios", {}),
    ("config-loja", "TelaConfiguracoes", {"secao": "loja"}),
    ("config-vendas", "TelaConfiguracoes", {"secao": "vendas"}),
    ("config-crediario", "TelaConfiguracoes", {"secao": "receber"}),
    ("config-impressao", "TelaConfiguracoes", {"secao": "impressao"}),
    ("config-caixas", "TelaConfiguracoes", {"secao": "caixas"}),
    ("config-historico", "TelaConfiguracoes", {"secao": "historico"}),
    ("config-backup", "TelaConfiguracoes", {"secao": "backup"}),
    ("config-fiscal", "TelaConfiguracoes", {"secao": "fiscal"}),
    ("config-gerente", "TelaConfiguracoes", {"secao": "gerente"}),
    ("config-sobre", "TelaConfiguracoes", {"secao": "sobre"}),
]

# Depois de abrir: o que fazer na tela antes de medir (uma janelinha aberta).
ACOES = {
    # A seta da ficha alterada abre "Descartar alterações?" (Painel com a seta).
    "peca-nova-descartar": ("TelaPeca", {}, "descartar"),
    # Sangria (Painel que era Popup) no Caixa.
    "caixa-sangria": ("TelaCaixa", {}, "sangria"),
}


def acao(r, qual):
    if qual == "descartar":
        # Digita no primeiro campo e aciona a seta (como o Esc).
        toques, _ = r.medidas()
        campos = [t for t in toques if t["tipo"] == "campo" and t["ativo"]]
        if not campos:
            raise RuntimeError("sem campo na ficha")
        c = campos[0]
        px, py = r.tela_fisica(c["x"] + c["w"] / 2, c["y"] + c["h"] / 2)
        xdo("mousemove", str(px), str(py), "click", "1")
        time.sleep(0.4)
        xdo("type", "--delay", "60", "Teste")
        time.sleep(0.4)
        seta = [t for t in toques if t["classe"] == "Seta"]
        if not seta:
            raise RuntimeError("sem seta na ficha")
        s = seta[0]
        px, py = r.tela_fisica(s["x"] + s["w"] / 2, s["y"] + s["h"] / 2)
        xdo("mousemove", str(px), str(py), "click", "1")
        time.sleep(1)
    elif qual == "sangria":
        if not (r.clicar("Sangria") or r.clicar("Sangria / suprimento", comeca=True)):
            raise RuntimeError("não achei a Sangria no Caixa")
        time.sleep(1)


def pequeno(t):
    return t["w"] + FOLGA < MINIMO or t["h"] + FOLGA < MINIMO


def sobrepoe(a, b):
    """Os retângulos se cruzam (mais que meio pixel)?"""
    return (min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0]) > 0.5
            and min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1]) > 0.5)


def desenho_do_texto(r):
    """O retângulo onde o texto é desenhado: a largura do desenho
    (contentWidth: a do texto cortado com "…", ou maior que o item quando o
    texto passa dele), pelo alinhamento horizontal (1 esquerda, 2 direita,
    4 centro)."""
    w = r["cw"]
    if r["alinhamento"] == 2:
        x = r["x"] + r["w"] - w
    elif r["alinhamento"] == 4:
        x = r["x"] + (r["w"] - w) / 2
    else:
        x = r["x"]
    return (x, r["y"], w, min(r["ch"], r["h"]) if r["h"] > 0 else r["ch"])


BOTOES_DA_DECISAO = {"escolher peca", "trocar peca", "cadastrar nova", "nao lancar"}


def pend17(res, toques, rets, foto):
    """Na linha do item da compra: os botões da decisão não cobrem os textos
    da linha e o "Que peça é esta?" não fica espremido."""
    botoes = [t for t in toques if t["tipo"] == "botao" and base.simples(t["nome"]) in BOTOES_DA_DECISAO]
    textos = [r for r in rets if r["componente"] == "LinhaItemCompra"
              and base.simples(r["texto"]) not in BOTOES_DA_DECISAO]
    pergunta = [r for r in textos if base.simples(r["texto"]) == "que peca e esta?"]
    res.conferir("PEND-17 achou a linha do item sem peça (\"Que peça é esta?\") e os botões",
                 bool(pergunta) and len(botoes) >= 3, f"{len(pergunta)} perguntas, {len(botoes)} botões; foto {foto}")
    cobertos = []
    for b in botoes:
        rb = (b["x"], b["y"], b["w"], b["h"])
        for t in textos:
            if sobrepoe(rb, desenho_do_texto(t)):
                cobertos.append(f"\"{b['nome']}\" sobre \"{t['texto'][:40]}\"")
    res.conferir("PEND-17 nenhum botão da decisão por cima de um texto da linha", not cobertos,
                 "; ".join(sorted(set(cobertos))[:6]) + (f" (foto {foto})" if cobertos else ""))
    espremidos = [f"{p['texto']}: {p['w']:.0f} de {p['natural']:.0f} dp" for p in pergunta if p["w"] + 0.5 < p["natural"]]
    res.conferir("PEND-17 \"Que peça é esta?\" inteiro (não espremido nem cortado)", not espremidos,
                 "; ".join(espremidos))
    fora = [f"{b['nome']} até x = {b['x'] + b['w']:.0f}" for b in botoes if b["x"] < -0.5 or b["x"] + b["w"] > LARGURA_DP + 0.5]
    res.conferir(f"PEND-17 os botões da decisão inteiros dentro da tela ({LARGURA_DP} dp)", not fora, "; ".join(fora[:6]))


def medir_tela(res, fotos, pasta, nome, tela, props, ids, inventario, qual=None):
    props = {k: ids.get(v, v) if isinstance(v, str) else v for k, v in props.items()}
    r = Celular(pasta, "Nenhuma" if tela == "Entrada" else tela, props, fotos, f"{TEMA}-{nome}")
    res.logs.append(r.saida)
    try:
        if tela == "Entrada":
            for _ in range(10):
                time.sleep(1)
                try:
                    if any(base.simples(t) == "quem esta usando?" for _, _, t in r.textos()):
                        break
                except RuntimeError:
                    pass
            else:
                raise RuntimeError(f"a tela de entrada não apareceu (veja {r.saida})")
            time.sleep(0.5)
        else:
            r.entrar("Ana")
            time.sleep(1.5)
        if qual:
            acao(r, qual)
        foto = r.foto_janela()
        toques, rets = r.medidas()
        largura = max((t["x"] + t["w"] for t in toques), default=0)
        res.conferir(f"{nome}: {len(toques)} controles medidos", len(toques) > 0, f"foto {foto}")
        for t in toques:
            inventario.append(dict(t, tela=nome))
        if nome == "compra-pendente":
            pend17(res, toques, rets, foto)
        return largura
    finally:
        r.fechar()


def escrever_inventario(fotos, inventario):
    """TSV com cada controle (tela, componente, tipo, nome, tamanho) e um
    resumo por componente dos que ficam abaixo de 48 dp."""
    caminho = os.path.join(fotos, f"inventario-toque-{TEMA}.tsv")
    with open(caminho, "w", encoding="utf-8") as f:
        f.write("tela\tcomponente\tclasse\ttipo\tnome\tlargura\taltura\tativo\tabaixo_de_48\n")
        for t in inventario:
            f.write(f"{t['tela']}\t{t['componente']}\t{t['classe']}\t{t['tipo']}\t{t['nome']}\t{t['w']:.1f}\t{t['h']:.1f}\t"
                    f"{int(t['ativo'])}\t{int(pequeno(t))}\n")
    grupos = {}
    for t in inventario:
        if not pequeno(t):
            continue
        chave = (t["componente"], t["classe"], t["tipo"], round(t["w"]) if t["w"] < MINIMO else "≥48",
                 round(t["h"]) if t["h"] < MINIMO else "≥48")
        g = grupos.setdefault(chave, {"telas": set(), "nomes": set(), "n": 0})
        g["telas"].add(t["tela"])
        g["nomes"].add(t["nome"][:30])
        g["n"] += 1
    resumo = os.path.join(fotos, f"inventario-toque-{TEMA}-abaixo-de-48.md")
    with open(resumo, "w", encoding="utf-8") as f:
        f.write(f"# Controles com área de toque < 48 dp (tema {TEMA}, {LARGURA_DP}×{ALTURA_DP} dp, densidade {DENSIDADE:g})\n\n")
        f.write("| Componente | Classe | Tipo | Largura | Altura | Ocorrências | Telas | Exemplos |\n|---|---|---|---|---|---|---|---|\n")
        for chave, g in sorted(grupos.items(), key=lambda kv: (kv[0][0], kv[0][1], str(kv[0][3]), str(kv[0][4]))):
            comp, cls, tipo, w, h = chave
            f.write(f"| {comp} | {cls} | {tipo} | {w} | {h} | {g['n']} | {len(g['telas'])}: {', '.join(sorted(g['telas'])[:6])}"
                    f"{' …' if len(g['telas']) > 6 else ''} | {'; '.join(sorted(g['nomes'])[:4])} |\n")
    return caminho, resumo, grupos


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual (a :0 é a área de trabalho de verdade)")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-toque-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-toque-modelo-")
    lado = (max(1600, round(LARGURA_DP * DENSIDADE) + 200), max(1700, round(ALTURA_DP * DENSIDADE) + 100))
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", f"{lado[0]}x{lado[1]}x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    res = tema.Resultado()
    inventario = []
    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(modelo)
        compra_pendente(modelo)
        if TEMA != "cinza":
            chave, valor = tema.TEMA_NO_BANCO[TEMA]
            tema.gravar_tema(modelo, chave, valor)
        ids = ids_da_loja(modelo)
        so = [t for t in os.environ.get("RUSTINHO_TOQUE_TELAS", "").split(",") if t]
        lista = [(n, t, p, None) for n, t, p in TELAS] + [(n, t, p, q) for n, (t, p, q) in ACOES.items()]
        for nome, tela, props, qual in lista:
            if so and nome not in so:
                continue
            pasta = tempfile.mkdtemp(prefix="rustinho-loja-toque-")
            shutil.rmtree(pasta)
            shutil.copytree(modelo, pasta)
            try:
                medir_tela(res, fotos, pasta, nome, tela, props, ids, inventario, qual)
            except RuntimeError as e:
                res.conferir(f"{nome}: abriu e mediu", False, str(e))
            finally:
                shutil.rmtree(pasta, ignore_errors=True)
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(modelo, ignore_errors=True)
    tsv, resumo, grupos = escrever_inventario(fotos, inventario)
    pequenos = [t for t in inventario if pequeno(t)]
    print(f"\ninventário: {len(inventario)} controles em {len({t['tela'] for t in inventario})} telas; "
          f"{len(pequenos)} abaixo de 48 dp em {len(grupos)} grupos ({tsv}; {resumo})")
    for (comp, cls, tipo, w, h), g in sorted(grupos.items(), key=lambda kv: -kv[1]["n"]):
        print(f"   {comp} / {cls} ({tipo}) {w} × {h}: {g['n']} em {len(g['telas'])} telas — {'; '.join(sorted(g['nomes'])[:3])}")
    if os.environ.get("RUSTINHO_TOQUE_SO_MEDIR") != "1":
        res.conferir(f"A15 todos os {len(inventario)} controles com área de toque ≥ 48 × 48 dp", not pequenos,
                     f"{len(pequenos)} abaixo (veja {resumo})")
    erros = [e for caminho in res.logs for e in tema.erros_de_script(caminho)]
    res.conferir(f"A12 zero TypeError/ReferenceError em {len(res.logs)} registros", not erros, "; ".join(erros[:5]))
    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha")
    for f in res.falhas:
        print("  ✘", f)
    print("fotos em", fotos)
    sys.exit(0 if not res.falhas else 1)


if __name__ == "__main__":
    main()
