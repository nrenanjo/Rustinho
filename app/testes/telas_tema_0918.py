#!/usr/bin/env python3
"""Teste das telas dos três temas (0.9.18): cinza, preto AMOLED e Planeta Roxo.

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros testes das telas (`telas_d09.ambiente_neutro`: só X11, sem o tema do
desktop), e confere pelos textos da tela, pelo banco e pela cor dos pixels.

  A1.  Trocar o tema na Início (sol, lua, planeta) muda a tela na hora e a
       escolha fica gravada só neste computador (`tela_tema`); fechar e abrir
       de novo volta no mesmo tema. Um banco antigo com `tela_escura = "1"`
       abre no AMOLED (A2, também no teste do núcleo).
  A3.  No roxo, as cores medidas (média 5×5 px) na Início, em Peças e em
       Configurações › Caixas ficam a ±12 por canal das cores do tema
       (as do `Tema.qml`, conferidas no próprio arquivo).
  A10. Início no roxo: os cartões em xadrez (claro/escuro por
       (linha + coluna) % 2) com 4, 3, 2 e 1 colunas (quatro larguras).
  A12. Nenhum TypeError/ReferenceError nos registros de todas as aberturas.

Modo capturas (A4: comparar antes × depois no cinza e no AMOLED): abre as
mesmas telas com o programa indicado e guarda as fotos da janela, sem
conferir nada.

    python3 app/testes/telas_tema_0918.py [pasta para as fotos]
    RUSTINHO_TELA=:92 python3 app/testes/telas_tema_0918.py        (outra tela virtual)
    RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_tema_0918.py
    RUSTINHO_TEMA_MODO=capturas RUSTINHO_TEMA_PREFIXO=antes RUSTINHO_TEMAS=cinza,amoled \\
        python3 app/testes/telas_tema_0918.py pasta

Precisa de: Xvfb, xdotool, ImageMagick (import) e Pillow (cores das fotos).
Sai com 0 se tudo conferiu.
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
import telas_d09 as base  # noqa: E402  (importar já deixa o ambiente neutro)

base.TELA = os.environ.get("RUSTINHO_TELA", ":91")
TELA = base.TELA
base.PROGRAMA = os.environ.get("RUSTINHO_PROGRAMA", base.PROGRAMA)
PROGRAMA = base.PROGRAMA
TEMA_QML = os.path.join(base.RAIZ, "app", "base", "Tema.qml")
TOLERANCIA = 12


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def por_fora(pasta, sql):
    """Muda o banco por fora (como um banco antigo), pelo núcleo."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_SQL=sql)
    subprocess.run(["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
                    "--ignored", "--exact", "executar_por_fora"],
                   cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True)


def config(pasta, chave):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        r = c.execute("SELECT valor FROM config WHERE chave = ?", (chave,)).fetchone()
        return r[0] if r else None
    finally:
        c.close()


def gravar_tema(pasta, chave, valor):
    """Deixa a loja num tema antes de abrir: `tela_tema` (0.9.18) ou o
    `tela_escura` antigo; valor None apaga as duas chaves."""
    sql = "DELETE FROM config WHERE chave IN ('tela_tema', 'tela_escura');"
    if valor is not None:
        sql += f"INSERT INTO config (chave, valor) VALUES ('{chave}', '{valor}');"
    por_fora(pasta, sql)


# ---- Cores do tema, lidas do próprio Tema.qml ----

def cores_do_tema(tema):
    """{nome: "#RRGGBB"} das cores de um tema ("cinza", "amoled" ou "roxo")
    no Tema.qml: cada `readonly property color nome: <expressão>` é avaliada
    com roxo/escuro/roxoFiel do tema (o operador ?: do QML, &&, ||, !,
    parênteses, cores "#..." e outros nomes de cor do próprio Tema). As que
    dependem de funções (Qt.rgba, sombra...) ficam de fora."""
    texto = open(TEMA_QML, encoding="utf-8").read()
    exprs = dict(re.findall(r"readonly property color (\w+):\s*(.+?)\s*(?://.*)?$", texto, re.M))
    roxo_fiel = re.search(r"readonly property bool roxoFiel:\s*(true|false)", texto).group(1) == "true"
    valores = {"roxo": tema == "roxo", "escuro": tema == "amoled", "roxoFiel": roxo_fiel}
    cores = {}

    def valor(nome, pilha=()):
        if nome in valores:
            return valores[nome]
        if nome in cores:
            return cores[nome]
        if nome not in exprs or nome in pilha:
            raise ValueError(nome)
        cores[nome] = avaliar(exprs[nome], pilha + (nome,))
        return cores[nome]

    def avaliar(expr, pilha):
        fichas = re.findall(r'"#[0-9A-Fa-f]{6}"|&&|\|\||[?:()!]|\w+', expr)
        if len("".join(fichas)) != len(re.sub(r"\s", "", expr)):
            raise ValueError(expr)   # função (Qt.rgba, sombra...): fora
        pos = [0]

        def olhar():
            return fichas[pos[0]] if pos[0] < len(fichas) else None

        def comer():
            pos[0] += 1
            return fichas[pos[0] - 1]

        def condicional():
            c = ou()
            if olhar() == "?":
                comer()
                a = condicional()
                assert comer() == ":"
                b = condicional()
                return a if c else b
            return c

        def ou():
            v = e()
            while olhar() == "||":
                comer()
                w = e()
                v = v or w
            return v

        def e():
            v = nao()
            while olhar() == "&&":
                comer()
                w = nao()
                v = v and w
            return v

        def nao():
            if olhar() == "!":
                comer()
                return not nao()
            return atomo()

        def atomo():
            f = comer()
            if f == "(":
                v = condicional()
                assert comer() == ")"
                return v
            if f.startswith('"#'):
                return f.strip('"').upper()
            return valor(f, pilha)

        v = condicional()
        if olhar() is not None:
            raise ValueError(expr)
        return v

    for nome in exprs:
        try:
            valor(nome)
        except (ValueError, AssertionError, IndexError, TypeError):
            cores.pop(nome, None)
    return {k: v for k, v in cores.items() if isinstance(v, str)}


def tokens_do_roxo():
    """{nome: "#RRGGBB"} das cores do Planeta Roxo no Tema.qml."""
    return cores_do_tema("roxo")


def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def media(foto, x, y, r=2):
    from PIL import Image
    img = Image.open(foto).convert("RGB")
    px = [img.getpixel((i, j)) for i in range(x - r, x + r + 1) for j in range(y - r, y + r + 1)]
    return tuple(round(sum(p[k] for p in px) / len(px)) for k in range(3))


def hx(c):
    return "#%02X%02X%02X" % tuple(c)


def perto(medida, alvo, tol=TOLERANCIA):
    return all(abs(a - b) <= tol for a, b in zip(medida, rgb(alvo)))


# ---- O programa numa tela virtual ----

def janela_do_rustinho():
    """(x, y, largura, altura) da janela do Rustinho na tela virtual."""
    for busca in (["--class", "rustinho"], ["--name", "Rustinho"]):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", *busca],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        for j in s.stdout.split():
            g = subprocess.run(["xdotool", "getwindowgeometry", "--shell", j],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            d = dict(x.split("=", 1) for x in g.stdout.split() if "=" in x)
            if int(d.get("WIDTH", 0)) > 200:
                return int(d["X"]), int(d["Y"]), int(d["WIDTH"]), int(d["HEIGHT"])
    return 0, 0, 1280, 760


class Rustinho(base.Rustinho):
    """O Rustinho aberto numa tela, conferindo o ambiente antes de tocar nele."""

    def __init__(self, pasta, tela, props, fotos, nome):
        super().__init__(pasta, tela, props, fotos, nome)
        self.conferir_ambiente()

    def conferir_ambiente(self):
        """Antes de qualquer clique ou tecla: o processo está na tela virtual
        (DISPLAY certo) e sem Wayland, senão a janela iria para a área de
        trabalho de verdade."""
        # Logo depois do fork o processo ainda é uma cópia do Python: espera o
        # programa de verdade (a linha de comando dele) antes de ler. N05
        # (dev.3.3): logo depois do exec (ou com o processo já terminando), o
        # /proc/<pid>/environ pode vir VAZIO; vazio não é "ambiente errado", é
        # "ainda não deu para ler": lê de novo. Um ambiente lido e errado
        # continua parando o roteiro antes de tocar no programa.
        for _ in range(100):
            if self.p.poll() is not None:
                raise RuntimeError(f"o Rustinho fechou antes de abrir a janela (código {self.p.returncode})")
            try:
                with open(f"/proc/{self.p.pid}/cmdline", "rb") as f:
                    carregado = f.read().split(b"\0")[0] == PROGRAMA.encode()
                if carregado:
                    with open(f"/proc/{self.p.pid}/environ", "rb") as f:
                        amb = dict(x.split(b"=", 1) for x in f.read().split(b"\0") if b"=" in x)
                    if amb:
                        break
            except (FileNotFoundError, ProcessLookupError):
                pass
            time.sleep(0.05)
        else:
            self.p.kill()
            raise RuntimeError("o Rustinho não abriu (o ambiente dele não pôde ser lido)")
        tela = amb.get(b"DISPLAY", b"").decode()
        if tela != TELA or b"WAYLAND_DISPLAY" in amb or amb.get(b"QT_QPA_PLATFORM") != b"xcb":
            self.p.kill()
            raise RuntimeError(f"ambiente errado no Rustinho (DISPLAY={tela!r}, "
                               f"WAYLAND_DISPLAY={'sim' if b'WAYLAND_DISPLAY' in amb else 'não'}): parei antes de tocar nele")
        with open(self.saida + ".ambiente", "w") as f:
            f.write(f"pid={self.p.pid} DISPLAY={tela} WAYLAND_DISPLAY={'presente' if b'WAYLAND_DISPLAY' in amb else 'ausente'} "
                    f"QT_QPA_PLATFORM={amb.get(b'QT_QPA_PLATFORM', b'').decode()}\n")

    def entrar(self, usuario):
        """Entra e confere que saiu da tela de entrada (com a máquina ocupada,
        o primeiro clique às vezes chega antes da tela estar pronta)."""
        super().entrar(usuario)
        for _ in range(3):
            if not any(base.simples(t) == "quem esta usando?" for _, _, t in self.textos()):
                return
            if self.clicar(usuario):
                time.sleep(1)
                xdo("type", "--delay", "80", base.SENHA[usuario])
                xdo("key", "Return")
                time.sleep(4)
        raise RuntimeError(f"não consegui entrar como {usuario} (veja {self.saida})")

    def foto_janela(self, sufixo=""):
        """Foto só da janela (sem o resto da tela virtual)."""
        xdo("mousemove", "5", "995")   # o mouse fora da janela: nada "com o mouse em cima"
        time.sleep(0.6)
        inteira = self.foto(sufixo + "-tela")
        x, y, w, h = janela_do_rustinho()
        from PIL import Image
        caminho = os.path.join(self.fotos, self.nome + sufixo + ".png")
        Image.open(inteira).crop((x, y, x + w, y + h)).save(caminho)
        os.remove(inteira)
        return caminho

    def posicao(self, texto):
        for x, y, t in self.textos():
            if base.simples(t) == base.simples(texto):
                return x, y
        return None

    def redimensionar(self, largura, altura):
        for busca in (["--class", "rustinho"], ["--name", "Rustinho"]):
            s = subprocess.run(["xdotool", "search", "--onlyvisible", *busca],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            for j in s.stdout.split():
                g = subprocess.run(["xdotool", "getwindowgeometry", "--shell", j],
                                   env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
                if "WIDTH=" in g.stdout and int(re.search(r"WIDTH=(\d+)", g.stdout).group(1)) > 200:
                    xdo("windowsize", j, str(largura), str(altura))
                    time.sleep(1.5)
                    return
        raise RuntimeError("não achei a janela do Rustinho para mudar o tamanho")


def erros_de_script(caminho):
    with open(caminho, encoding="utf-8", errors="replace") as f:
        return [l.strip() for l in f if "TypeError" in l or "ReferenceError" in l]


# ---- Telas fotografadas (A4) ----

TELAS_A4 = [
    # (nome, tela, propriedades)
    ("inicio", "Nenhuma", {}),
    ("pecas", "TelaPecas", {}),
    ("configuracoes-caixas", "TelaConfiguracoes", {"secao": "caixas"}),
    ("financeiro-pagar", "TelaFinanceiro", {"secao": "pagar"}),
    ("receber", "TelaReceber", {}),
    ("clientes", "TelaPessoas", {"papel": "cliente"}),
    # Mudança prevista (V-6): os rankings passam a alternar também no cinza e no AMOLED.
    ("painel-resumo", "TelaPainel", {"secao": "resumo"}),
    ("painel-pecas", "TelaPainel", {"secao": "pecas"}),
]

# Como deixar o banco em cada tema antes de abrir. O cinza e o AMOLED usam a
# chave antiga (`tela_escura`), que a versão de antes e a de agora entendem.
TEMA_NO_BANCO = {
    "cinza": ("tela_escura", None),
    "amoled": ("tela_escura", "1"),
    "roxo": ("tela_tema", "roxo"),
}


def modo_capturas(fotos, modelo):
    prefixo = os.environ.get("RUSTINHO_TEMA_PREFIXO", "captura")
    temas = os.environ.get("RUSTINHO_TEMAS", "cinza,amoled").split(",")
    # RUSTINHO_TEMA_TELAS=painel-pecas,inicio: só essas.
    so = [t for t in os.environ.get("RUSTINHO_TEMA_TELAS", "").split(",") if t]
    erros = []
    for tema in temas:
        for nome, tela, props in TELAS_A4:
            if so and nome not in so:
                continue
            pasta = tempfile.mkdtemp(prefix="rustinho-loja-tema-")
            shutil.rmtree(pasta)
            shutil.copytree(modelo, pasta)
            chave, valor = TEMA_NO_BANCO[tema]
            gravar_tema(pasta, chave, valor)
            r = Rustinho(pasta, tela, props, fotos, f"{prefixo}-{tema}-{nome}")
            try:
                r.entrar("Ana")
                time.sleep(1.5)
                print("foto:", r.foto_janela())
            finally:
                r.fechar()
                shutil.rmtree(pasta, ignore_errors=True)
            erros += erros_de_script(r.saida)
    print(f"{len(erros)} TypeError/ReferenceError nos registros")
    for e in erros:
        print("   ", e)
    return not erros


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-tema-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-tema-modelo-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(modelo)
        if os.environ.get("RUSTINHO_TEMA_MODO") == "capturas":
            ok = modo_capturas(fotos, modelo)
        else:
            ok = testar(fotos, modelo)
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(modelo, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if ok else 1)


# ---- Os testes ----

class Resultado:
    def __init__(self):
        self.ok = 0
        self.falhas = []
        self.logs = []

    def conferir(self, nome, certo, detalhe=""):
        print(("✔ " if certo else "✘ ") + nome + (f" ({detalhe})" if detalhe else ""))
        if certo:
            self.ok += 1
        else:
            self.falhas.append(nome + (f" ({detalhe})" if detalhe else ""))
        return certo


def nova_loja(modelo, tema=None):
    """Cópia da loja de teste; tema: None (nada gravado), "escura1" (banco
    antigo com tela_escura = "1") ou o nome do tema em tela_tema."""
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-tema-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    if tema == "escura1":
        gravar_tema(pasta, "tela_escura", "1")
    elif tema:
        gravar_tema(pasta, "tela_tema", tema)
    else:
        gravar_tema(pasta, "tela_tema", None)
    return pasta


def abrir(res, pasta, tela, props, fotos, nome):
    r = Rustinho(pasta, tela, props, fotos, nome)
    res.logs.append(r.saida)
    r.entrar("Ana")
    time.sleep(1.5)
    return r


def amostra(res, nome, foto, x, y, alvo, r=2):
    m = media(foto, x, y, r)
    return res.conferir(f"{nome}: {hx(m)} perto de {alvo}", perto(m, alvo), f"em ({x}, {y}), ±{TOLERANCIA}")


def onde(r, texto):
    p = r.posicao(texto)
    if not p:
        raise RuntimeError(f"não achei \"{texto}\" na tela (veja {r.saida})")
    return p


def a1_trocar_e_persistir(res, fotos, modelo, cores):
    """A1: sol/lua/planeta na Início mudam na hora e ficam gravados (só aqui)."""
    pasta = nova_loja(modelo)
    # 0.9.18: o claro (sol, "cinza" no banco) e o escuro (lua, "amoled") na
    # identidade roxa; as cores vêm do próprio Tema.qml.
    fundo_cinza, fundo_amoled = cores_do_tema("cinza")["fundo"], cores_do_tema("amoled")["fundo"]
    try:
        r = abrir(res, pasta, "Nenhuma", {}, fotos, "a1-inicio")
        try:
            foto = r.foto_janela("-1-cinza")
            amostra(res, "A1 começa no claro (nada gravado)", foto, 640, 740, fundo_cinza)
            for dica, tema, fundo in [("Planeta Roxo", "roxo", cores["fundo"]), ("Tema escuro", "amoled", fundo_amoled),
                                      ("Tema claro", "cinza", fundo_cinza), ("Planeta Roxo", "roxo", cores["fundo"])]:
                x, y = onde(r, dica)
                xdo("mousemove", str(x), str(y), "click", "1")
                time.sleep(0.8)
                foto = r.foto_janela(f"-2-{tema}")
                amostra(res, f"A1 {dica}: a Início muda na hora", foto, 640, 740, fundo)
                res.conferir(f"A1 {dica}: gravado tela_tema = {tema}", config(pasta, "tela_tema") == tema,
                             f"no banco: {config(pasta, 'tela_tema')!r}")
                res.conferir(f"A1 {dica}: chave antiga coerente (tela_escura)",
                             config(pasta, "tela_escura") == ("1" if tema == "amoled" else "0"))
        finally:
            r.fechar()
        # Fecha e abre: continua no roxo.
        r = abrir(res, pasta, "Nenhuma", {}, fotos, "a1-reaberto")
        try:
            foto = r.foto_janela()
            amostra(res, "A1 fechou e abriu: continua no roxo", foto, 640, 740, cores["fundo"])
        finally:
            r.fechar()
        # A2 na tela: banco antigo com tela_escura = "1" abre no AMOLED.
        antiga = nova_loja(modelo, "escura1")
        try:
            r = abrir(res, antiga, "Nenhuma", {}, fotos, "a2-banco-antigo")
            try:
                foto = r.foto_janela()
                amostra(res, "A2 banco antigo (tela_escura = 1) abre no AMOLED", foto, 640, 740, fundo_amoled)
            finally:
                r.fechar()
        finally:
            shutil.rmtree(antiga, ignore_errors=True)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def a3_cores_do_roxo(res, fotos, modelo, cores):
    """A3: as cores na tela, nos mesmos elementos das imagens aprovadas."""
    pasta = nova_loja(modelo, "roxo")
    try:
        # Início
        r = abrir(res, pasta, "Nenhuma", {}, fotos, "a3-roxo-inicio")
        try:
            foto = r.foto_janela()
            amostra(res, "A3 Início: fundo", foto, 640, 740, cores["fundo"])
            x, y = onde(r, "Vender / Receber")
            amostra(res, "A3 Início: cartão escuro (linha 0, coluna 0)", foto, x, y - 22, cores["roxoFaixaEscura"])
            x, y = onde(r, "Peças")
            amostra(res, "A3 Início: cartão claro (linha 0, coluna 1)", foto, x, y - 22, cores["roxoFaixaClara"])
            # 0.9.18 (D-V6): o F11 saiu do campo; o ponto de referência é o
            # botão de copiar, dentro da busca (o F11 no programa de antes).
            if r.posicao("Copiar a busca"):
                x, y = onde(r, "Copiar a busca")
                x += 10
            else:
                x, y = onde(r, "F11")
            amostra(res, "A3 Início: fundo da busca", foto, x - 70, y, cores["buscaFundo"])
            amostra(res, "A3 Início: borda da busca", foto, x - 70, y + 27, cores["buscaBorda"], r=0)
            x, y = onde(r, "Planeta Roxo")
            amostra(res, "A3 Início: planeta escolhido (anel)", foto, x, y - 14, cores["seletorAtivo"], r=0)
        finally:
            r.fechar()
        # Peças
        r = abrir(res, pasta, "TelaPecas", {}, fotos, "a3-roxo-pecas")
        try:
            foto = r.foto_janela()
            amostra(res, "A3 Peças: fundo", foto, 640, 752, cores["fundo"])
            precos = sorted((y, x) for x, y, t in r.textos() if t.startswith("R$ ") and x > 1000)
            if len(precos) < 3:
                raise RuntimeError(f"Peças: menos de 3 linhas (veja {r.saida})")
            amostra(res, "A3 Peças: linha escolhida (1ª)", foto, 40, precos[0][0], cores["atual"])
            amostra(res, "A3 Peças: faixa escura (2ª)", foto, 40, precos[1][0], cores["roxoFaixaEscura"])
            amostra(res, "A3 Peças: faixa clara (3ª)", foto, 40, precos[2][0], cores["roxoFaixaClara"])
            x, y = onde(r, "Nova peça")
            amostra(res, "A3 Peças: botão principal (Nova peça)", foto, x, y + 14, cores["principal"])
            x, y = onde(r, "Importar XML")
            amostra(res, "A3 Peças: botão secundário (Importar XML)", foto, x, y + 14, cores["botao"])
            x, y = onde(r, "F11")
            amostra(res, "A3 Peças: fundo da busca", foto, x - 70, y, cores["buscaFundo"])
        finally:
            r.fechar()
        # Configurações › Caixas
        r = abrir(res, pasta, "TelaConfiguracoes", {"secao": "caixas"}, fotos, "a3-roxo-configuracoes-caixas")
        try:
            foto = r.foto_janela()
            amostra(res, "A3 Config.: fundo", foto, 1150, 450, cores["fundo"])
            x, y = onde(r, "Simultaneidade")
            amostra(res, "A3 Config.: pílula ativa (Simultaneidade)", foto, x, y + 13, cores["pilulaAtiva"])
            x, y = onde(r, "Caixas")
            amostra(res, "A3 Config.: subpílula ativa (Caixas)", foto, x, y + 11, cores["subpilulaAtiva"])
            x, y = onde(r, "Duplicatas")
            amostra(res, "A3 Config.: faixa das subpílulas", foto, x, y + 13, cores["faixaSubpilulas"])
            x, y = onde(r, "Caixas da loja")
            amostra(res, "A3 Config.: painel", foto, x, y - 16, cores["pergaminho"])
            x, y = onde(r, "Senha da rede da loja")
            amostra(res, "A3 Config.: campo", foto, x + 200, y + 32, cores["papel"])
            x, y = onde(r, "Salvar sincronização")
            amostra(res, "A3 Config.: botão Salvar (secundário)", foto, x, y + 14, cores["botao"])
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


CARTOES = ["Vender / Receber", "Peças", "Orçamentos", "Caixa", "Trocas e devoluções", "Notas fiscais", "Clientes",
           "Compras", "Estoque / Fornecedores", "Financeiro", "Ordens de serviço", "Painel", "Usuários", "Configurações"]


INDISPONIVEIS = {"Notas fiscais"}


def agrupar(valores, tolerancia):
    """Os centros dos grupos de valores próximos (em ordem)."""
    grupos = []
    for v in sorted(valores):
        if grupos and v - grupos[-1][-1] <= tolerancia:
            grupos[-1].append(v)
        else:
            grupos.append([v])
    return [sum(g) / len(g) for g in grupos]


def a10_xadrez(res, fotos, modelo, cores):
    """A10: cartões da Início em xadrez com 4, 3, 2 e 1 colunas."""
    pasta = nova_loja(modelo, "roxo")
    try:
        r = abrir(res, pasta, "Nenhuma", {}, fotos, "a10-roxo-inicio")
        try:
            for colunas, (w, h) in [(4, (1280, 760)), (3, (900, 760)), (2, (900, 520)), (1, (600, 900))]:
                r.redimensionar(w, h)
                time.sleep(1)
                foto = r.foto_janela(f"-{colunas}-colunas")
                pos = [(x, y, t) for x, y, t in r.textos() if t in CARTOES]
                # Linhas e colunas da grade: agrupa as posições próximas (o
                # título de "Notas fiscais" fica mais alto, por causa do
                # "em breve" embaixo dele).
                xs = agrupar([x for x, _, _ in pos], 60)
                ys = agrupar([y for _, y, _ in pos], 30)
                res.conferir(f"A10 {colunas} coluna(s) na largura {w}", len(xs) == colunas, f"achei {len(xs)}")
                certos, vistos, errados = 0, 0, []
                for x, y, t in pos:
                    if y - 20 < 0 or y > h - 10:
                        continue   # fora da parte visível (rolada)
                    linha = min(range(len(ys)), key=lambda k: abs(ys[k] - y))
                    coluna = min(range(len(xs)), key=lambda k: abs(xs[k] - x))
                    esperado = cores["roxoFaixaEscura"] if (linha + coluna) % 2 == 0 else cores["roxoFaixaClara"]
                    if t in INDISPONIVEIS:
                        # Cartão "em breve": opacidade 0,78 sobre o fundo roxo.
                        e, f = rgb(esperado), rgb(cores["fundo"])
                        esperado = hx(tuple(round(0.78 * a + 0.22 * b) for a, b in zip(e, f)))
                    vistos += 1
                    # 16 px acima do centro do título: dentro do cartão também no
                    # celular (cartão de 64 px) e no "Notas fiscais" (título mais alto).
                    medida = media(foto, x, y - 16)
                    if perto(medida, esperado):
                        certos += 1
                    else:
                        errados.append(f"{t}: {hx(medida)} × {esperado}")
                res.conferir(f"A10 {colunas} coluna(s): xadrez (linha + coluna) % 2 nos {vistos} cartões visíveis",
                             vistos > 0 and certos == vistos, f"{certos} de {vistos} certos" + (f"; {errados}" if errados else ""))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# Volta pelas telas no roxo (fotos individuais para conferir o texto e as listas).
TELAS_ROXO = [
    ("inicio", "Nenhuma", {}),
    ("pecas", "TelaPecas", {}),
    ("peca-ficha", "TelaPeca", {"produtoId": "FILTRO"}),
    ("clientes", "TelaPessoas", {"papel": "cliente"}),
    ("fornecedores", "TelaPessoas", {"papel": "fornecedor"}),
    ("pessoa-ficha", "TelaPessoa", {"pessoaId": "ZE"}),
    ("vender", "TelaVender", {}),
    ("receber", "TelaReceber", {}),
    ("caixa", "TelaCaixa", {}),
    ("orcamentos", "TelaOrcamentos", {}),
    ("trocas", "TelaTrocas", {}),
    ("ordens", "TelaOrdens", {}),
    ("compras", "TelaCompras", {}),
    ("estoque-comprar", "TelaEstoque", {"secao": "comprar"}),
    ("estoque-encomendas", "TelaEstoque", {"secao": "encomendas"}),
    ("estoque-vendidos", "TelaEstoque", {"secao": "vendidos"}),
    ("financeiro-pagar", "TelaFinanceiro", {"secao": "pagar"}),
    ("financeiro-contas", "TelaFinanceiro", {"secao": "contas"}),
    ("financeiro-fluxo", "TelaFinanceiro", {"secao": "fluxo"}),
    ("financeiro-comissoes", "TelaFinanceiro", {"secao": "comissoes"}),
    ("painel-resumo", "TelaPainel", {"secao": "resumo"}),
    ("painel-dre", "TelaPainel", {"secao": "dre"}),
    ("painel-pecas", "TelaPainel", {"secao": "pecas"}),
    ("usuarios", "TelaUsuarios", {}),
    ("etiquetas", "TelaEtiquetas", {}),
    ("configuracoes-loja", "TelaConfiguracoes", {"secao": "loja"}),
    ("configuracoes-caixas", "TelaConfiguracoes", {"secao": "caixas"}),
    ("configuracoes-historico", "TelaConfiguracoes", {"secao": "historico"}),
    ("configuracoes-backup", "TelaConfiguracoes", {"secao": "backup"}),
]


def volta_no_roxo(res, fotos, modelo):
    pasta = nova_loja(modelo, "roxo")
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    ids = {"FILTRO": c.execute("SELECT id FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0],
           "ZE": c.execute("SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'").fetchone()[0]}
    c.close()
    try:
        for nome, tela, props in TELAS_ROXO:
            props = {k: ids.get(v, v) for k, v in props.items()}
            r = abrir(res, pasta, tela, props, fotos, f"roxo-{nome}")
            try:
                print("foto:", r.foto_janela())
            finally:
                r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def testar(fotos, modelo):
    cores = tokens_do_roxo()
    faltam = [n for n in ("fundo", "roxoFaixaClara", "roxoFaixaEscura", "atual", "principal", "botao", "buscaFundo",
                          "buscaBorda", "seletorAtivo", "pilulaAtiva", "faixaSubpilulas", "subpilulaAtiva",
                          "pergaminho", "papel") if n not in cores]
    if faltam:
        sys.exit(f"não achei no Tema.qml as cores do roxo: {faltam}")
    print("cores do roxo (Tema.qml):", ", ".join(f"{k} {v}" for k, v in sorted(cores.items())))
    res = Resultado()
    casos = os.environ.get("RUSTINHO_CASOS", "A1,A3,A10,VOLTA").split(",")
    for caso, funcao in [("A1", a1_trocar_e_persistir), ("A3", a3_cores_do_roxo), ("A10", a10_xadrez)]:
        if caso in casos:
            try:
                funcao(res, fotos, modelo, cores)
            except RuntimeError as e:
                res.conferir(f"{caso} terminou", False, str(e))
    if "VOLTA" in casos:
        volta_no_roxo(res, fotos, modelo)
    # A12: nenhum erro de script nos registros de todas as aberturas.
    erros = [e for caminho in res.logs for e in erros_de_script(caminho)]
    res.conferir(f"A12 zero TypeError/ReferenceError em {len(res.logs)} registros", not erros,
                 "; ".join(erros[:5]))
    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha")
    for f in res.falhas:
        print("  ✘", f)
    return not res.falhas


if __name__ == "__main__":
    main()
