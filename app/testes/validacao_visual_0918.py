#!/usr/bin/env python3
"""Validação visual e funcional da 0.9.18 no programa MONTADO (o AppImage no
Linux, o Rustinho que o instalador pôs no Windows), antes de publicar. Uma
foto da tela de verdade para cada passo, com o resultado em resultado.txt.

  B. Busca: as consultas de busca_esperada.json (código, código original,
     nome, palavras fora de ordem, marca, modelo do carro, carro e ano, carro,
     motor e ano, equivalentes, marca e nome, código do fornecedor, nada),
     a primeira pela busca da Início e as outras pela de Peças. A lista na
     tela tem de ter exatamente as peças esperadas, na ordem. Com a busca
     preenchida, o Esc e a seta voltam direto à Início (D-V3, decidida pelo
     dono em 07/11/2026).
  T. Temas: claro (sol), escuro (lua) e Planeta Roxo, trocados como uma
     pessoa troca, na Início. Em cada um: Início, Configurações, dois
     formulários (peça nova e cliente novo) e as telas de lista; nas listas,
     pelos pixels da foto, as linhas vizinhas têm fundos diferentes e as
     alternadas, o mesmo fundo.
  N. Navegação: Peças › a seta volta à Início; Vender › o Esc volta; peça
     nova com o nome digitado: o Esc pergunta "Descartar alterações?" (a
     janelinha tem a sua seta, sem "Cancelar"), o Esc de novo continua no
     formulário, a seta da tela pergunta de novo, a seta da janelinha
     desiste igual ao Esc e "Descartar" volta sem gravar; a navegação guiada
     Configurações › Simultaneidade › Fiscal › "Ir para Loja": o Esc desiste
     e volta ao Fiscal; uma mudança não salva pede "Descartar alterações?" e
     "Descartar" volta ao Fiscal sem gravar; com a UF e o CNPJ salvos, volta
     ao Fiscal com o requisito satisfeito e a faixa some.
  S. Seta de retorno em todas as telas (correção definitiva do dono,
     07/11/2026): cada tela da Início, as de dentro (peça nova e cadastrada,
     etiquetas, cliente novo e cadastrado, nota de compra, orçamento, OS
     nova) e as trocadas pelas pílulas (Receber, Fornecedores): só a seta,
     sem texto, no canto de cima à esquerda (cerca de 1 cm); o Esc e a seta
     voltam ao mesmo lugar. Uma foto de cada tela.

Uso:
  python3 validacao_visual_0918.py <programa> <pasta da loja montada> <pasta das fotos>

A loja (no mesmo computador do programa, para a identidade do computador
bater):
  RUSTINHO_LOJA_TESTE=<pasta> cargo test -p rustinho-nucleo --test estavel \\
      -- --ignored --exact montar_loja_de_validacao
O roteiro trabalha numa cópia da loja; a pasta montada não muda.

Linux: Xvfb, xdotool, ImageMagick (import) e Pillow. Só numa tela virtual
(RUSTINHO_TELA, padrão :70), com uma pasta pessoal temporária e sem o
barramento da sessão: nada abre na área de trabalho de verdade.
Windows: pyautogui, pywin32 e Pillow, na área de trabalho da máquina (CI).
RUSTINHO_PARTES=BTNS escolhe as partes (padrão: as quatro).
"""
import collections
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import unicodedata

WINDOWS = os.name == "nt"
USUARIO, SENHA = "Ana", "1234"
CNPJ_BOM = "11.444.777/0001-61"
# O escuro primeiro: a loja nova já abre no claro, e escolher o tema que já
# está valendo não grava nada.
TEMAS = [("Tema escuro", "escuro", "amoled"), ("Tema claro", "claro", "cinza"), ("Planeta Roxo", "roxo", "roxo")]
# Os cartões da Início que abrem as telas de lista.
TELAS = ["Peças", "Clientes", "Orçamentos", "Caixa", "Compras", "Estoque / Fornecedores", "Financeiro",
         "Ordens de serviço", "Painel", "Usuários", "Trocas e devoluções", "Vender / Receber"]
FORMULARIOS = [("Peças", "Nova peça", "peca-nova"), ("Clientes", "Novo cliente", "cliente-novo")]
# (cartão da Início, pílula): listas de dentro das telas, para as faixas.
LISTAS_POR_PILULA = [("Estoque / Fornecedores", "Fornecedores"), ("Estoque / Fornecedores", "Parados"),
                     ("Financeiro", "Contas da loja"), ("Painel", "Peças e vendedores")]


def simples(t):
    t = unicodedata.normalize("NFKD", t)
    t = "".join(c for c in t if not unicodedata.combining(c))
    return re.sub(r"\s+", " ", t).strip().lower()


def arquivo(nome):
    return re.sub(r"[^a-z0-9]+", "-", simples(nome)).strip("-")


# ---------------------------------------------------------------- sistemas
class Linux:
    """Tela virtual (Xvfb), xdotool e import. A janela ocupa a tela toda."""
    LARGURA, ALTURA = 1440, 1000

    def __init__(self):
        self.tela = os.environ.get("RUSTINHO_TELA", ":70")
        n = self.tela.lstrip(":")
        if not n.isdigit() or int(n) < 10:
            sys.exit(f"tela {self.tela} recusada: use uma tela virtual (:10 ou mais)")
        if os.path.exists(f"/tmp/.X{n}-lock") or os.path.exists(f"/tmp/.X11-unix/X{n}"):
            sys.exit(f"a tela {self.tela} já está em uso; escolha outra com RUSTINHO_TELA=:NN")
        self.xvfb = subprocess.Popen(["Xvfb", self.tela, "-screen", "0", f"{self.LARGURA}x{self.ALTURA}x24"],
                                     stderr=subprocess.DEVNULL)
        time.sleep(1.5)
        if self.xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu a tela {self.tela}")
        self.id = None
        self.marca = None

    def ambiente(self, casa, marca):
        """O ambiente do programa: só a tela virtual, pasta pessoal
        temporária, sem Wayland, sem o tema do desktop e sem o barramento da
        sessão (nenhuma janela do sistema na área de trabalho de verdade)."""
        fora = ("WAYLAND_DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP", "DESKTOP_SESSION",
                "KDE_FULL_SESSION", "KDE_SESSION_VERSION", "KDE_SESSION_UID", "GNOME_DESKTOP_SESSION_ID",
                "QT_QPA_PLATFORMTHEME", "QT_STYLE_OVERRIDE", "APPIMAGE", "APPDIR", "OWD", "ARGV0",
                "DBUS_SESSION_BUS_ADDRESS", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME",
                "XDG_STATE_HOME", "XAUTHORITY", "SESSION_MANAGER")
        e = {k: v for k, v in os.environ.items() if k not in fora}
        e.update(DISPLAY=self.tela, QT_QPA_PLATFORM="xcb", HOME=casa, XDG_RUNTIME_DIR=casa,
                 RUSTINHO_VALIDACAO=marca)
        self.marca = marca
        return e

    def _x(self, *a, saida=False):
        r = subprocess.run(["xdotool", *a], env=dict(os.environ, DISPLAY=self.tela),
                           capture_output=saida, text=saida, check=not saida)
        return r.stdout if saida else None

    def preparar(self, pid):
        for _ in range(90):
            for busca in (["--class", "rustinho"], ["--name", "Rustinho"]):
                for j in self._x("search", "--onlyvisible", *busca, saida=True).split():
                    g = self._x("getwindowgeometry", "--shell", j, saida=True)
                    d = dict(x.split("=", 1) for x in g.split() if "=" in x)
                    if int(d.get("WIDTH", 0)) > 200:
                        self.id = j
                        self._x("windowmove", j, "0", "0")
                        self._x("windowsize", j, str(self.LARGURA), str(self.ALTURA))
                        time.sleep(2)
                        self.neutro()
                        return True
            time.sleep(1)
        return False

    def janela(self):
        g = self._x("getwindowgeometry", "--shell", self.id, saida=True)
        d = dict(x.split("=", 1) for x in g.split() if "=" in x)
        return int(d["X"]), int(d["Y"]), int(d["WIDTH"]), int(d["HEIGHT"])

    def neutro(self):
        """O mouse num canto da janela, sem nada embaixo (sem "mouse em
        cima"), e ainda dentro dela: sem gerenciador de janelas, o teclado vai
        para a janela que está sob o mouse."""
        x, y, w, h = self.janela()
        self._x("mousemove", str(x + w - 3), str(y + h - 3))

    def tecla(self, k):
        self._x("key", k)

    def digitar(self, t):
        self._x("type", "--delay", "70", t)

    def clicar(self, px, py):
        x, y, _, _ = self.janela()
        self._x("mousemove", str(x + px), str(y + py), "click", "1")

    def rolar(self, px, py, para_baixo):
        x, y, _, _ = self.janela()
        self._x("mousemove", str(x + px), str(y + py), "click", "5" if para_baixo else "4")

    def foto(self, caminho):
        subprocess.run(["import", "-window", "root", caminho], env=dict(os.environ, DISPLAY=self.tela), check=True)

    def encerrar_programa(self):
        """O AppImage pode deixar processos filhos: fecha os que herdaram a
        marca deste teste (nunca pelo nome, para não fechar outro Rustinho)."""
        alvo = f"RUSTINHO_VALIDACAO={self.marca}".encode()
        for d in os.listdir("/proc"):
            if not d.isdigit():
                continue
            try:
                with open(f"/proc/{d}/environ", "rb") as f:
                    if alvo in f.read().split(b"\0"):
                        os.kill(int(d), 9)
            except (OSError, PermissionError):
                pass

    def sistema(self):
        try:
            with open("/etc/os-release") as f:
                nome = dict(l.rstrip().split("=", 1) for l in f if "=" in l).get("PRETTY_NAME", "Linux")
        except OSError:
            nome = "Linux"
        return f"{nome.strip(chr(34))}, tela virtual {self.tela}"

    def fim(self):
        self.xvfb.terminate()


class Windows:
    """A área de trabalho do Windows, pyautogui e win32. A janela é
    maximizada; as posições do Rustinho são da área de dentro da janela."""

    TECLAS = {"Escape": ("esc",), "Return": ("enter",), "ctrl+a": ("ctrl", "a"), "F2": ("f2",), "Down": ("down",),
              "ctrl+shift+F12": ("ctrl", "shift", "f12")}

    def __init__(self):
        import ctypes
        try:
            ctypes.windll.user32.SetProcessDPIAware()
        except Exception:
            pass
        import pyautogui
        self.pa = pyautogui
        self.pa.FAILSAFE = False
        self.pa.PAUSE = 0.05
        self.hwnd = None

    def ambiente(self, casa, marca):
        return dict(os.environ, RUSTINHO_VALIDACAO=marca)

    def preparar(self, pid):
        import win32con
        import win32gui
        import win32process
        for _ in range(90):
            achadas = []

            def ver(h, _):
                if win32gui.IsWindowVisible(h) and win32process.GetWindowThreadProcessId(h)[1] == pid \
                        and win32gui.GetWindowText(h):
                    achadas.append(h)
            win32gui.EnumWindows(ver, None)
            if achadas:
                self.hwnd = achadas[0]
                win32gui.ShowWindow(self.hwnd, win32con.SW_MAXIMIZE)
                time.sleep(2)
                self.frente()
                self.neutro()
                return True
            time.sleep(1)
        return False

    def frente(self):
        import win32gui
        if win32gui.GetForegroundWindow() != self.hwnd:
            try:
                self.pa.press("alt")
                win32gui.SetForegroundWindow(self.hwnd)
                time.sleep(0.3)
            except Exception:
                pass

    def janela(self):
        import win32gui
        ox, oy = win32gui.ClientToScreen(self.hwnd, (0, 0))
        l, t, r, b = win32gui.GetClientRect(self.hwnd)
        return ox, oy, r - l, b - t

    def neutro(self):
        x, y, w, h = self.janela()
        self.pa.moveTo(x + w - 3, y + h - 3)

    def tecla(self, k):
        self.frente()
        t = self.TECLAS[k]
        if len(t) > 1:
            self.pa.hotkey(*t)
        else:
            self.pa.press(t[0])

    def digitar(self, t):
        self.frente()
        self.pa.write(t, interval=0.05)

    def clicar(self, px, py):
        self.frente()
        x, y, _, _ = self.janela()
        self.pa.click(x + px, y + py)

    def rolar(self, px, py, para_baixo):
        self.frente()
        x, y, _, _ = self.janela()
        self.pa.moveTo(x + px, y + py)
        self.pa.scroll(-3 if para_baixo else 3)

    def foto(self, caminho):
        self.pa.screenshot(caminho)

    def encerrar_programa(self):
        pass

    def sistema(self):
        import platform
        x, y, w, h = self.janela()
        return f"{platform.platform()}, janela {w}x{h}"

    def fim(self):
        pass


# ---------------------------------------------------------------- o programa
class Rustinho:
    def __init__(self, programa, pasta, fotos, sistema):
        self.s = sistema
        self.fotos = fotos
        self.pasta = pasta
        self.saida = os.path.join(fotos, "rustinho.log")
        self.casa = tempfile.mkdtemp(prefix="rustinho-casa-")
        env = self.s.ambiente(self.casa, os.path.basename(self.casa))
        env.update(RUSTINHO_DADOS=pasta, QT_FORCE_STDERR_LOGGING="1",
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true")
        self.log = open(self.saida, "wb")
        # --medir-toque: as linhas das listas (áreas de toque), para conferir as faixas.
        self.p = subprocess.Popen([programa, "--textos-da-tela", "--medir-toque"], env=env, stdout=self.log,
                                  stderr=self.log)
        if not self.s.preparar(self.p.pid):
            raise RuntimeError(f"a janela do Rustinho não apareceu (veja {self.saida})")
        time.sleep(3)

    def _ler(self):
        with open(self.saida, "rb") as f:
            return f.read()

    def textos(self):
        """[(x, y, texto)] do que está na janela agora (posições de dentro
        da janela). No Windows o Qt escreve na página de código do sistema."""
        antes = self._ler().count(b"TEXTOS-DA-TELA-FIM")
        self.s.tecla("ctrl+shift+F12")
        for _ in range(80):
            time.sleep(0.25)
            tudo = self._ler()
            if tudo.count(b"TEXTOS-DA-TELA-FIM") > antes:
                bruto = tudo.split(b"TEXTOS-DA-TELA-INICIO")[-1].split(b"TEXTOS-DA-TELA-FIM")[0]
                try:
                    bloco = bruto.decode("utf-8")
                except UnicodeDecodeError:
                    bloco = bruto.decode("cp1252", errors="replace")
                lista = []
                for linha in bloco.splitlines():
                    p = linha.split("\t")
                    if len(p) >= 3 and p[0].lstrip("-").isdigit() and p[1].lstrip("-").isdigit():
                        lista.append((int(p[0]), int(p[1]), "\t".join(p[2:]).strip()))
                return lista
        raise RuntimeError(f"o Rustinho não escreveu os textos da tela (veja {self.saida})")

    def pilha(self, t=None):
        for _, _, x in (t if t is not None else self.textos()):
            if x.startswith("TESTE_PILHA:"):
                return int(x.split(":")[1])
        return -1

    def tem(self, texto, t=None):
        alvo = simples(texto)
        return any(alvo in simples(x) for _, _, x in (t if t is not None else self.textos()))

    def esperar(self, texto, segundos=15):
        fim = time.time() + segundos
        while True:
            if self.tem(texto):
                return True
            if time.time() > fim:
                return False
            time.sleep(0.5)

    def esperar_pilha(self, n, segundos=10):
        fim = time.time() + segundos
        while True:
            if self.pilha() == n:
                return True
            if time.time() > fim:
                return False
            time.sleep(0.4)

    def posicao(self, texto, comeca=False, t=None):
        alvo = simples(texto)
        for x, y, tx in (t if t is not None else self.textos()):
            s = simples(tx)
            if s == alvo or (comeca and s.startswith(alvo)):
                return x, y
        return None

    def geos(self, nome, t=None):
        """[(x, y, largura, altura)] dos itens com este objectName, na ordem da
        tela (o da tela antes do das janelinhas)."""
        achados = []
        for _, _, tx in (t if t is not None else self.textos()):
            if tx.startswith(f"GEO:{nome}:"):
                achados.append(tuple(int(float(v)) for v in tx.split(":")[2].split(",")))
        return achados

    def geo(self, nome, t=None, ultimo=False):
        """O centro do item com este objectName (o de cima de tudo com
        ultimo=True: a seta da janelinha aberta)."""
        g = self.geos(nome, t)
        if not g:
            return None
        x, y, w, h = g[-1] if ultimo else g[0]
        return x + w // 2, y + h // 2

    def clicar(self, texto, comeca=False, topo=0, tentativas=30):
        """Clica no texto; se ele estiver fora da área visível (acima de
        "topo", atrás de um cabeçalho fixo, ou abaixo da janela), rola com a
        roda do mouse e só clica com a tela parada."""
        _, _, w, h = self.s.janela()
        anterior = None
        for _ in range(tentativas):
            p = self.posicao(texto, comeca)
            if not p:
                return False
            if topo < p[1] < h - 30 and 0 < p[0] < w:
                if p == anterior or topo == 0:
                    self.s.clicar(*p)
                    time.sleep(1.2)
                    return True
                anterior = p
                time.sleep(0.4)
                continue
            anterior = None
            self.s.rolar(w // 2, h // 2, p[1] >= h - 30)
            time.sleep(0.4)
        return False

    def clicar_seta(self, ultimo=False):
        p = self.geo("setaDeRetorno", ultimo=ultimo)
        if not p:
            return False
        self.s.clicar(*p)
        time.sleep(1.2)
        return True

    def campo(self, rotulo, abaixo=32):
        """Clica no campo logo abaixo do rótulo."""
        p = self.posicao(rotulo)
        if not p:
            return False
        self.s.clicar(p[0], p[1] + abaixo)
        time.sleep(0.5)
        return True

    def senha_se_pedir(self):
        """Algumas telas pedem a senha do gerente para abrir."""
        if self.esperar("senha do gerente", 1.5):
            self.s.digitar(SENHA)
            self.s.tecla("Return")
            time.sleep(1.5)

    def foto(self, nome):
        self.s.neutro()
        time.sleep(0.6)
        caminho = os.path.join(self.fotos, nome + ".png")
        self.s.foto(caminho)
        return caminho

    def erros_de_script(self):
        texto = self._ler().decode("utf-8", errors="replace")
        return [l.strip() for l in texto.splitlines()
                if re.search(r"TypeError|ReferenceError|is not a type|module .* is not installed", l)]

    def fechar(self):
        self.p.terminate()
        try:
            self.p.wait(10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        time.sleep(2)
        self.s.encerrar_programa()
        self.log.close()
        shutil.rmtree(self.casa, ignore_errors=True)


# ---------------------------------------------------------------- conferências
class Resultado:
    def __init__(self, pasta):
        self.linhas = []
        self.falhas = 0
        self.arquivo = os.path.join(pasta, "resultado.txt")
        open(self.arquivo, "w", encoding="utf-8").close()

    def escrever(self, linha):
        print(linha, flush=True)
        self.linhas.append(linha)
        with open(self.arquivo, "a", encoding="utf-8") as f:
            f.write(linha + "\n")

    def conferir(self, nome, certo, foto="", detalhe=""):
        if not certo:
            self.falhas += 1
        self.escrever(("✔ " if certo else "✘ ") + nome + (f" — {detalhe}" if detalhe else "")
                      + (f" [{os.path.basename(foto)}]" if foto else ""))
        return certo


def listas_da_tela(textos):
    """As linhas das listas da tela, pelo que o próprio Rustinho informa com
    --medir-toque (linhas TOQUE: a área de toque de cada controle): as linhas
    de uma lista são áreas de toque (TapHandler ou MouseArea) do mesmo
    componente, na mesma coluna e com a mesma largura, uma logo abaixo da
    outra (vão de até 16 px). Devolve [(componente, [(x, y, w, h)])], só as
    listas com 3 linhas ou mais."""
    grupos = collections.defaultdict(list)
    for _, _, t in textos:
        if t.startswith(("TOQUE:tap|", "TOQUE:mousearea|")):
            partes = t.split("|")
            x, y, w, h = (float(v) for v in partes[4].split(","))
            grupos[(partes[1], partes[2], round(x), round(w))].append((x, y, w, h))
    listas = []
    for (componente, _, _, _), rets in grupos.items():
        rets.sort(key=lambda r: r[1])
        seq = [rets[0]]
        for r in rets[1:]:
            if 0 <= r[1] - (seq[-1][1] + seq[-1][3]) <= 16:
                seq.append(r)
                continue
            if len(seq) >= 3:
                listas.append((componente, seq))
            seq = [r]
        if len(seq) >= 3:
            listas.append((componente, seq))
    return listas


def alternancia(foto, textos, janela):
    """Confere as faixas claras e escuras das listas da tela. As linhas vêm do
    próprio Rustinho (listas_da_tela); a cor de cada linha é a do fundo dela
    na foto, na margem da esquerda, a meia altura (a cor que mais aparece de
    x+3 a x+12), onde não há texto. Só as linhas inteiras dentro da janela.
    Devolve (estado, detalhe): "alterna" (em cada lista com 3 linhas ou mais,
    da 2ª em diante, vizinhas com fundos diferentes e alternadas iguais: A, B,
    A...; a 1ª, a escolhida, pode ter a cor de destaque), "nao" (alguma lista
    sem isso) ou "sem" (nenhuma lista com 3 linhas inteiras à vista)."""
    from PIL import Image
    img = Image.open(foto).convert("RGB")
    ox, oy, w, h = janela
    vistas, falhas = [], []
    for componente, linhas in listas_da_tela(textos):
        inteiras = [r for r in linhas if r[1] >= 0 and r[1] + r[3] <= h and r[0] >= 0 and r[0] + 14 <= w]
        if len(inteiras) < 3:
            continue
        cores = []
        for x, y, _, alt in inteiras:
            amostra = [img.getpixel((round(ox + x + dx), round(oy + y + alt / 2))) for dx in range(3, 13)]
            cores.append(collections.Counter(amostra).most_common(1)[0][0])
        descricao = f"{componente}: {len(inteiras)} linhas, fundos {' '.join(hexa(c) for c in cores[:6])}"
        # Sem a 1ª (a escolhida pode ter a cor de destaque): vizinhas
        # diferentes e alternadas iguais.
        c = cores[1:]
        if all(c[i] != c[i + 1] for i in range(len(c) - 1)) and all(c[i] == c[i + 2] for i in range(len(c) - 2)):
            vistas.append(descricao)
        else:
            falhas.append(descricao)
    if falhas:
        return "nao", "sem faixas alternadas: " + "; ".join(falhas)
    if vistas:
        return "alterna", "; ".join(vistas)
    return "sem", "nenhuma lista com 3 linhas inteiras à vista nesta tela (não dá para conferir as faixas)"


def hexa(c):
    return "#%02X%02X%02X" % tuple(c)


def voltar_inicio(r, res, nome):
    for _ in range(5):
        if r.pilha() == 1:
            return True
        r.s.tecla("Escape")
        time.sleep(1.2)
    certo = r.pilha() == 1
    if not certo:
        res.conferir(f"{nome}: voltou à Início", False, r.foto(f"erro-{arquivo(nome)}"), "a pilha não voltou a 1")
    return certo


JANELINHAS = ["Descartar alterações?", "Descartar as regras alteradas?", "Sair da OS?", "Sair sem salvar?",
              "Limpar esta venda?"]


def janelinha(r, t):
    for j in JANELINHAS:
        if any(simples(x) == simples(j) for _, _, x in t):
            return j
    return ""


def seta_so_desenho(r, res, nome, t, foto):
    """Correção definitiva do botão de retorno (dono, 07/11/2026): uma seta
    só, no vidro roxo, no canto de cima à esquerda (cerca de 1 cm: 28 a 40 px
    numa tela de 96 dpi), sem texto, legenda ou nome de destino ao lado (a
    área de toque quadrada: com texto ela fica mais larga)."""
    setas = r.geos("setaDeRetorno", t)
    if not setas:
        return res.conferir(f"{nome}: a seta de retorno na tela", False, foto, "sem seta")
    x, y, w, h = setas[0]
    return res.conferir(f"{nome}: só a seta (sem texto), no canto de cima à esquerda, ~1 cm",
                        w == h and w >= 40 and 28 <= x <= 40 and 24 <= y <= 40, foto,
                        f"seta em ({x}, {y}), {w}×{h} px")


def voltar_ate(r, n):
    for _ in range(5):
        p = r.pilha()
        if p <= n:
            return
        r.s.tecla("Escape")
        time.sleep(1.0)
        if janelinha(r, r.textos()):
            r.clicar("Descartar")
            time.sleep(1.0)


def ida_e_volta(r, res, nome, abrir):
    """abrir() leva da tela de agora até a tela a conferir. Na 1ª ida: a seta
    (só o desenho, no canto) e a foto; o Esc volta. Na 2ª ida, a seta volta.
    Os dois têm de dar na mesma tela (a de antes, pela pilha), sem janelinha."""
    antes = r.pilha()
    fim = {}
    for como in ("esc", "seta"):
        abrir()
        r.senha_se_pedir()
        if not r.esperar_pilha(antes + 1, 8):
            res.conferir(f"S {nome}: abriu", False, r.foto(f"S-erro-{arquivo(nome)}-{como}"),
                         f"pilha {r.pilha()}, esperado {antes + 1}")
            voltar_ate(r, antes)
            return
        time.sleep(1.0)
        if como == "esc":
            t = r.textos()
            seta_so_desenho(r, res, f"S {nome}", t, r.foto(f"S-{arquivo(nome)}"))
            r.s.tecla("Escape")
        else:
            r.clicar_seta()
        time.sleep(1.3)
        t = r.textos()
        fim[como] = (r.pilha(t), janelinha(r, t))
        voltar_ate(r, antes)
    e, s = fim["esc"], fim["seta"]
    res.conferir(f"S {nome}: o Esc e a seta voltam ao mesmo lugar", e == s and e[0] == antes and not e[1],
                 detalhe=f"pilha {antes + 1} → Esc {e[0]}, seta {s[0]}" + (f"; janelinha {e[1] or s[1]}" if e[1] or s[1] else ""))


def tema_gravado(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        v = c.execute("SELECT valor FROM config WHERE chave = 'tela_tema'").fetchone()
    finally:
        c.close()
    return v[0] if v else ""


def produtos_com_nome(pasta, nome):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute("SELECT count(*) FROM produto WHERE nome = ?", (nome,)).fetchone()[0]
    finally:
        c.close()


def config_loja(pasta, chave):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        v = c.execute("SELECT valor FROM config WHERE chave = ?", (chave,)).fetchone()
    finally:
        c.close()
    return v[0] if v else ""


# ---------------------------------------------------------------- partes
def busca(r, res, consultas, todas):
    res.escrever("== B. Busca de peças")
    res.conferir("B: na Início", r.esperar_pilha(1))
    for i, q in enumerate(consultas, 1):
        nome = f"B{i:02d}-{arquivo(q['tipo'])}"
        if i == 1:
            p = r.posicao("Buscar peça", comeca=True)
            if p:
                r.s.clicar(*p)
                time.sleep(0.4)
            r.s.digitar(q["consulta"])
            time.sleep(0.4)
            foto_inicio = r.foto("B00-busca-na-inicio")
            r.s.tecla("Return")
            res.conferir(f"B: a busca da Início leva a Peças com \"{q['consulta']}\"", r.esperar_pilha(2),
                         foto_inicio)
        else:
            r.s.tecla("ctrl+a")
            r.s.digitar(q["consulta"])
        time.sleep(2.2)
        t = r.textos()
        # A peça aparece na lista (o texto igual ao nome dela). Quando o nome
        # é igual à consulta, o texto do campo de busca não conta.
        achadas = []
        for n in todas:
            ys = sorted(y for _, y, x in t if simples(x) == simples(n))
            if simples(n) == simples(q["consulta"]):
                ys = ys[1:]
            if ys:
                achadas.append((ys[0], n))
        na_tela = [n for _, n in sorted(achadas)]
        esperadas = list(q["esperado"])
        foto = r.foto(nome)
        res.conferir(f"B: {q['tipo']} — \"{q['consulta']}\"", na_tela == esperadas, foto,
                     f"na tela: {na_tela or 'nenhuma peça'}; esperado: {esperadas or 'nenhuma peça'}")
    # D-V3 (correção definitiva do dono, 07/11/2026): com a busca preenchida, o
    # Esc faz o mesmo que a seta, os dois voltam direto à Início.
    r.s.tecla("Escape")
    res.conferir("B: com a busca preenchida, o Esc volta direto à Início (igual à seta)", r.esperar_pilha(1),
                 r.foto("B90-esc-volta"))
    p = r.posicao("Buscar peça", comeca=True)
    if p:
        r.s.clicar(*p)
        time.sleep(0.4)
    r.s.digitar(consultas[0]["consulta"])
    r.s.tecla("Return")
    r.esperar_pilha(2)
    time.sleep(1.5)
    r.clicar_seta()
    res.conferir("B: com a busca preenchida, a seta volta direto à Início", r.esperar_pilha(1), r.foto("B91-seta-volta"))


def temas(r, res, pasta):
    res.escrever("== T. Temas")
    from PIL import Image
    fundos = {}
    for dica, nome, chave in TEMAS:
        voltar_inicio(r, res, f"T {nome}")
        ok = r.clicar(dica)
        time.sleep(1.0)
        foto = r.foto(f"T-{nome}-01-inicio")
        ox, oy, w, h = r.s.janela()
        fundos[nome] = Image.open(foto).convert("RGB").getpixel((ox + w - 12, oy + h - 12))
        res.conferir(f"T {nome}: \"{dica}\" na Início troca o tema (gravado: {tema_gravado(pasta)!r})",
                     ok and tema_gravado(pasta) == chave, foto, "fundo #%02X%02X%02X" % fundos[nome])
        r.clicar("Configurações")
        r.senha_se_pedir()
        res.conferir(f"T {nome}: Configurações", r.esperar_pilha(2), r.foto(f"T-{nome}-02-configuracoes"))
        voltar_inicio(r, res, f"T {nome} configurações")
        for j, (lista, botao, arq) in enumerate(FORMULARIOS, 3):
            r.clicar(lista)
            r.esperar_pilha(2)
            r.clicar(botao)
            res.conferir(f"T {nome}: formulário ({botao})", r.esperar_pilha(3), r.foto(f"T-{nome}-{j:02d}-{arq}"))
            voltar_inicio(r, res, f"T {nome} {botao}")
        # As telas da Início e, pelas pílulas, listas que têm linhas na loja de
        # validação (as primeiras partes de Estoque, Financeiro e Painel ficam
        # vazias ou sem lista).
        listas = [(tela, None) for tela in TELAS] + [(m, p) for m, p in LISTAS_POR_PILULA]
        for i, (tela, pilula) in enumerate(listas, 3 + len(FORMULARIOS)):
            rotulo = f"{tela} › {pilula}" if pilula else tela
            if not r.clicar(tela):
                res.conferir(f"T {nome}: abrir {tela}", False, r.foto(f"T-{nome}-erro-{i:02d}"), "cartão não achado")
                continue
            r.senha_se_pedir()
            r.esperar_pilha(2)
            time.sleep(1.2)
            if pilula:
                r.clicar(pilula)
                time.sleep(1.5)
            foto = r.foto(f"T-{nome}-{i:02d}-{arquivo(rotulo)}")
            t = r.textos()
            estado, detalhe = alternancia(foto, t, r.s.janela())
            if estado == "sem":
                res.conferir(f"T {nome}: {rotulo}", r.pilha(t) == 2, foto, detalhe)
            else:
                res.conferir(f"T {nome}: {rotulo} — linhas alternadas", estado == "alterna" and r.pilha(t) == 2, foto,
                             detalhe)
            voltar_inicio(r, res, f"T {nome} {rotulo}")
    res.conferir("T: os três temas têm fundos diferentes", len(set(fundos.values())) == 3,
                 detalhe=", ".join(f"{k} #%02X%02X%02X" % v for k, v in fundos.items()))


def navegacao(r, res, pasta):
    res.escrever("== N. Navegação")
    voltar_inicio(r, res, "N")
    # 1. A seta.
    r.clicar("Peças")
    t = r.textos() if r.esperar_pilha(2) else []
    foto = r.foto("N1-a-pecas")
    res.conferir("N1: Início › Peças", r.pilha(t) == 2, foto)
    seta_so_desenho(r, res, "N1: Peças", t, foto)
    res.conferir("N1: a seta volta à Início", r.clicar_seta() and r.esperar_pilha(1), r.foto("N1-b-seta-voltou"))
    # 2. O Esc.
    r.clicar("Vender / Receber")
    res.conferir("N2: Início › Vender / Receber", r.esperar_pilha(2), r.foto("N2-a-vender"))
    r.s.tecla("Escape")
    res.conferir("N2: o Esc volta à Início", r.esperar_pilha(1), r.foto("N2-b-esc-voltou"))
    # 3. Formulário não salvo.
    r.clicar("Peças")
    r.esperar_pilha(2)
    r.clicar("Nova peça")
    r.esperar_pilha(3)
    time.sleep(1)
    r.campo("Nome da peça")
    r.s.digitar("TESTE NAO SALVAR")
    res.conferir("N3: peça nova com o nome digitado (sem salvar)", r.esperar("TESTE NAO SALVAR", 5),
                 r.foto("N3-a-digitado"))
    r.s.tecla("Escape")
    certo = r.esperar("Descartar alterações?", 5)
    foto = r.foto("N3-b-esc-pergunta")
    res.conferir("N3: o Esc pergunta \"Descartar alterações?\"", certo, foto)
    # A janelinha tem a sua seta (a de cima de tudo): só o desenho, sem texto.
    setas = r.geos("setaDeRetorno")
    res.conferir("N3: a janelinha tem a seta (só o desenho, sem \"Cancelar\")",
                 len(setas) >= 2 and setas[-1][2] == setas[-1][3] and not r.tem("Cancelar"), foto,
                 f"setas {setas}")
    r.s.tecla("Escape")
    time.sleep(1.2)
    t = r.textos()
    res.conferir("N3: o Esc na pergunta continua no formulário, com o nome",
                 not r.tem("Descartar alterações?", t) and r.tem("TESTE NAO SALVAR", t) and r.pilha(t) == 3,
                 r.foto("N3-c-continua"))
    r.clicar_seta()
    res.conferir("N3: a seta também pergunta", r.esperar("Descartar alterações?", 5),
                 r.foto("N3-d-seta-pergunta"))
    # A seta da janelinha desiste, como o Esc: continua no formulário.
    r.clicar_seta(ultimo=True)
    time.sleep(1.2)
    t = r.textos()
    res.conferir("N3: a seta da janelinha desiste (igual ao Esc): continua no formulário, com o nome",
                 not r.tem("Descartar alterações?", t) and r.tem("TESTE NAO SALVAR", t) and r.pilha(t) == 3,
                 r.foto("N3-d2-seta-da-janelinha"))
    r.s.tecla("Escape")
    r.esperar("Descartar alterações?", 5)
    r.clicar("Descartar")
    res.conferir("N3: \"Descartar\" volta a Peças", r.esperar_pilha(2), r.foto("N3-e-descartou"))
    n = produtos_com_nome(pasta, "TESTE NAO SALVAR")
    res.conferir("N3: nada foi gravado", n == 0, detalhe=f"peças com esse nome no banco: {n}")
    voltar_inicio(r, res, "N3")
    # 4. Navegação guiada: Configurações › Simultaneidade › Fiscal › Loja.
    r.clicar("Configurações")
    r.senha_se_pedir()
    r.esperar_pilha(2)
    r.clicar("Simultaneidade")
    # A Simultaneidade pede a senha do gerente uma vez para consultar.
    r.senha_se_pedir()
    r.clicar("Fiscal")
    if r.esperar("senha do gerente", 3) and not r.tem("Para continuar, informe a UF e o CNPJ da loja"):
        r.s.digitar(SENHA)
        r.s.tecla("Return")
    res.conferir("N4: Configurações › Simultaneidade › Fiscal pede a UF e o CNPJ da loja (\"Ir para Loja\")",
                 r.esperar("Para continuar, informe a UF e o CNPJ da loja") and r.tem("Ir para Loja"),
                 r.foto("N4-a-fiscal"))
    profundidade = r.pilha()
    r.clicar("Ir para Loja", topo=330)
    t = r.textos() if r.esperar("volta para Fiscal") else []
    res.conferir("N4: \"Ir para Loja\" leva à Loja, com \"volta para Fiscal\" e a orientação",
                 r.tem("volta para Fiscal", t) and r.tem("CNPJ da loja", t) and r.pilha(t) == profundidade,
                 r.foto("N4-b-loja-guiada"))
    r.s.tecla("Escape")
    certo = r.esperar("Ir para Loja")
    res.conferir("N4: o Esc desiste e volta ao Fiscal, sem empilhar tela", certo and r.pilha() == profundidade,
                 r.foto("N4-c-esc-voltou"))
    r.clicar("Ir para Loja", topo=330)
    r.esperar("volta para Fiscal")
    r.campo("Nome da loja")
    r.s.tecla("ctrl+a")
    r.s.digitar("TEMP-NAO-SALVAR")
    res.conferir("N4: mudança não salva no nome da loja", r.esperar("TEMP-NAO-SALVAR", 5), r.foto("N4-d-alterada"))
    r.s.tecla("Escape")
    res.conferir("N4: o Esc pergunta \"Descartar alterações?\"", r.esperar("Descartar alterações?", 5),
                 r.foto("N4-e-pergunta"))
    r.clicar("Descartar")
    certo = r.esperar("Ir para Loja")
    res.conferir("N4: \"Descartar\" volta ao Fiscal", certo and r.pilha() == profundidade, r.foto("N4-f-descartou"))
    nome = config_loja(pasta, "loja_nome")
    res.conferir("N4: o nome da loja não mudou", nome == "Loja Teste Auto Peças", detalhe=f"no banco: {nome!r}")
    r.clicar("Ir para Loja", topo=330)
    r.esperar("volta para Fiscal")
    time.sleep(0.8)
    for _ in range(26):  # o cursor começa no estado (o que falta primeiro): 26 para baixo = SP
        r.s.tecla("Down")
    time.sleep(0.3)
    r.s.tecla("F2")
    time.sleep(3)
    t = r.textos()
    res.conferir("N4: só a UF não basta: fica na Loja e diz o que falta",
                 r.tem("O CNPJ da loja ainda não está salvo.", t) and config_loja(pasta, "loja_uf") == "SP",
                 r.foto("N4-g-so-uf"), f"UF gravada: {config_loja(pasta, 'loja_uf')!r}")
    r.campo("CNPJ da loja")
    r.s.tecla("ctrl+a")
    r.s.digitar(CNPJ_BOM)
    time.sleep(0.3)
    r.s.tecla("F2")
    res.conferir("N4: com a UF e o CNPJ, volta ao Fiscal com o requisito satisfeito",
                 r.esperar("Requisito satisfeito", 10) and r.pilha() == profundidade, r.foto("N4-h-satisfeito"))
    time.sleep(1.5)
    res.conferir("N4: a faixa do que falta some", not r.tem("Para continuar, informe a UF e o CNPJ da loja"),
                 r.foto("N4-i-sem-faixa"), f"CNPJ gravado: {config_loja(pasta, 'loja_cnpj')!r}")
    voltar_inicio(r, res, "N4")


MODULOS = TELAS + ["Configurações"]
# (módulo na Início, o que abre a tela de dentro, como): "clicar" no texto ou
# "enter" (a lista abre a 1ª linha, a que já vem escolhida).
SUBTELAS = [("Peças", "Nova peça", "clicar"), ("Peças", "1ª peça da lista", "enter"), ("Peças", "Etiquetas", "clicar"),
            ("Clientes", "Novo cliente", "clicar"), ("Clientes", "1º cliente da lista", "enter"),
            ("Compras", "1ª nota de compra", "enter"), ("Orçamentos", "1º orçamento", "enter"),
            ("Ordens de serviço", "Nova OS", "clicar")]
# Pílulas que trocam a tela (Vender / Receber, Estoque / Fornecedores).
PILULAS = [("Vender / Receber", "Receber"), ("Estoque / Fornecedores", "Fornecedores")]


def setas(r, res):
    """S. Correção definitiva do botão de retorno, em todas as telas: cada
    tela da Início, as telas de dentro (peça, cliente, nota, orçamento, OS,
    etiquetas) e as trocadas pelas pílulas: só a seta no canto, e o Esc e a
    seta voltando ao mesmo lugar. Uma foto de cada tela."""
    res.escrever("== S. Seta de retorno em todas as telas")
    voltar_inicio(r, res, "S")
    for m in MODULOS:
        ida_e_volta(r, res, m, lambda m=m: r.clicar(m))
    for m, sub, como in SUBTELAS:
        voltar_inicio(r, res, "S")
        r.clicar(m)
        r.senha_se_pedir()
        if not r.esperar_pilha(2):
            res.conferir(f"S {m} › {sub}: abriu {m}", False, r.foto(f"S-erro-{arquivo(m)}"))
            continue
        time.sleep(1.0)
        if como == "enter":
            ida_e_volta(r, res, f"{m} › {sub}", lambda: r.s.tecla("Return"))
        else:
            ida_e_volta(r, res, f"{m} › {sub}", lambda sub=sub: r.clicar(sub))
    for m, pilula in PILULAS:
        fim = {}
        for como in ("esc", "seta"):
            voltar_inicio(r, res, "S")
            r.clicar(m)
            r.senha_se_pedir()
            r.esperar_pilha(2)
            time.sleep(0.8)
            r.clicar(pilula)
            time.sleep(1.5)
            t = r.textos()
            if como == "esc":
                seta_so_desenho(r, res, f"S {m} › {pilula}", t, r.foto(f"S-{arquivo(m)}-{arquivo(pilula)}"))
                r.s.tecla("Escape")
            else:
                r.clicar_seta()
            time.sleep(1.3)
            t = r.textos()
            fim[como] = (r.pilha(t), janelinha(r, t))
        e, s = fim["esc"], fim["seta"]
        res.conferir(f"S {m} › {pilula}: o Esc e a seta voltam à Início", e == s and e[0] == 1 and not e[1],
                     detalhe=f"Esc {e}, seta {s}")
    voltar_inicio(r, res, "S")


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    programa, modelo, fotos = sys.argv[1:]
    partes = os.environ.get("RUSTINHO_PARTES", "BTNS").upper()
    os.makedirs(fotos, exist_ok=True)
    with open(os.path.join(modelo, "busca_esperada.json"), encoding="utf-8") as f:
        consultas = json.load(f)
    pasta = tempfile.mkdtemp(prefix="rustinho-validacao-")
    for nome in os.listdir(modelo):
        if nome.startswith("rustinho.db") and not nome.endswith(".trava"):
            shutil.copy2(os.path.join(modelo, nome), pasta)
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    todas = [n for (n,) in c.execute("SELECT nome FROM produto")]
    c.close()
    res = Resultado(fotos)
    sistema = Windows() if WINDOWS else Linux()
    try:
        r = Rustinho(programa, pasta, fotos, sistema)
        try:
            res.escrever(f"programa: {programa}")
            res.escrever(f"sistema: {sistema.sistema()}")
            res.conferir("abriu: tela de entrada", r.esperar(USUARIO, 40), r.foto("00-entrada"))
            r.clicar(USUARIO)
            time.sleep(0.8)
            r.s.digitar(SENHA)
            r.s.tecla("Return")
            res.conferir("entrou como Ana: Início", r.esperar_pilha(1, 20) and r.tem("Vender / Receber"),
                         r.foto("01-inicio"))
            for letra, parte in (("B", lambda: busca(r, res, consultas, todas)), ("T", lambda: temas(r, res, pasta)),
                                 ("N", lambda: navegacao(r, res, pasta)), ("S", lambda: setas(r, res))):
                if letra not in partes:
                    continue
                try:
                    parte()
                except Exception as e:  # uma parte que quebra não esconde as outras
                    res.conferir(f"{letra}: rodou até o fim", False, r.foto(f"erro-{letra}"), repr(e))
                    voltar_inicio(r, res, letra)
            erros = r.erros_de_script()
            res.conferir("nenhum erro de script nas telas (QML)", not erros, detalhe="; ".join(erros[:3]))
        finally:
            r.fechar()
    finally:
        sistema.fim()
        shutil.rmtree(pasta, ignore_errors=True)
    certas = sum(1 for l in res.linhas if l.startswith("✔"))
    res.escrever(f"{certas} conferências certas, {res.falhas} com falha. Fotos em {fotos}")
    sys.exit(1 if res.falhas else 0)


if __name__ == "__main__":
    main()
