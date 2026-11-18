#!/usr/bin/env python3
"""Navegação contextual (C8), casos 4 a 10 da matriz (0.9.18), pela tela.

Cada caso confere, com o Rustinho de verdade numa tela virtual (Xvfb):
o disparo (o evento certo leva ao lugar certo), a conclusão, a desistência
sem perder o que já estava feito, que não há ciclo (nenhuma tela empilhada a
mais) e, quando há dinheiro ou dados, que nada é gravado sem um clique.

  8. modo seguro: a faixa leva a Configurações › Backup (restaurar). Pede a
     senha do gerente ao caixa; desistir volta; restaurar continua pedindo a
     confirmação e a senha; nenhum backup é apagado; depois de restaurar, vai
     para a entrada (não retoma a operação de antes).
  5. transferência sem conta de destino: "Cadastrar conta" abre o cadastro
     por cima da transferência; desistir volta a ela com o que foi digitado;
     salvar volta com a conta nova escolhida, sem transferir; só o clique em
     Transferir transfere.
  4. etiquetas com código ambíguo: a peça bloqueada fica à parte ("Não saem
     agora"), com a causa e as opções "Ver duplicatas" e "Abrir a peça"; ir e
     voltar sem resolver não libera nada; resolvida em Duplicatas, volta para
     a lista com a quantidade pedida; nada é impresso sem clique.
  6. rascunho de OS bloqueado por um caixa desatualizado: a saída da OS
     mostra o motivo e as opções (Ver caixas / a seta, que continua sem
     rascunho, como o Esc — 0.9.18, V-4 / Sair
     sem salvar); "Ver caixas" leva a Simultaneidade › Caixas com a guia e o
     caixa destacado; desistir (Esc, ou a senha do gerente) volta à OS com
     tudo; retirar o caixa (decisão do gerente, com motivo) volta à OS só
     quando o núcleo confirma, e o rascunho só é salvo com o clique.
  7. recebimento de um registro inconsistente (dev3.3): a recusa mostra o
     motivo e "Ver o registro" (nada muda de tela sozinho); "Ver o registro"
     leva a Simultaneidade › Caixas com a guia e o registro em primeiro;
     desistir (Esc) volta ao Receber com a janelinha reaberta e o valor de
     antes; aceitar como está (decisão do gerente, com motivo) volta só quando
     o núcleo confirma, e o recebimento só acontece com o clique.
  9b. Duplicatas → "Ver no histórico" filtra pelo registro do conflito; a pílula volta.
  9e. Início → a notificação Fiscal leva à Loja pela guia; desistir volta.
  10b. Comissões: o fechamento pago → "Ver no A pagar" → volta; nada muda.
  9. conferência (9a, 9c, 9d, já feitos na 0.9.17): do Início, "Ver
     duplicatas", "Ver caixas" e "Ver backup" levam à parte certa da
     Simultaneidade, e o "voltar" (Esc) devolve ao Início; o caixa que
     desiste da senha volta ao Início, sem o "Só o gerente pode" espúrio do
     Backup por trás do pedido da senha.

Abre o Rustinho com o ambiente neutro de telas_d09.py (só X11, sem Wayland,
sem o tema do desktop) e confere, em /proc, que ele está na tela virtual e
sem Wayland antes de qualquer clique. Nada abre na área de trabalho.

    RUSTINHO_TELA=:88 python3 app/testes/telas_navegacao_casos.py [pasta das fotos] [--casos 8,5]
"""

import hashlib
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_modo_seguro as seguro  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":88")
TELA = base.TELA
SENHA = base.SENHA
# O pedido da senha do gerente para consultar (PedirGerente).
PEDIDO = "você só consulta"


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def cargo_teste(env_extra, *filtro):
    env = dict(os.environ, CARGO_NET_OFFLINE="true", **env_extra)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
         "--ignored", "--exact", *filtro],
        cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def montar_loja(pasta, avisos=False):
    extra = {"RUSTINHO_LOJA_TESTE": pasta}
    if avisos:
        extra["RUSTINHO_LOJA_AVISOS"] = "1"
    cargo_teste(extra, "montar_loja_de_teste")


def por_fora(pasta, sql):
    """Muda a loja de teste pelo núcleo (com os gatilhos), com o Rustinho fechado."""
    cargo_teste({"RUSTINHO_LOJA_TESTE": pasta, "RUSTINHO_SQL": sql}, "executar_por_fora")


def ler(pasta, sql, *p):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute(sql, p).fetchall()
    finally:
        c.close()


def sha(caminho):
    return hashlib.sha256(open(caminho, "rb").read()).hexdigest()


def conferir_ambiente(pid):
    """O Rustinho do teste tem de estar na tela virtual, sem Wayland."""
    for _ in range(20):
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(x.split(b"=", 1) for x in f.read().split(b"\0") if b"=" in x)
        except OSError:
            env = {}
        if b"DISPLAY" in env:
            return env.get(b"DISPLAY") == TELA.encode() and b"WAYLAND_DISPLAY" not in env
        time.sleep(0.25)
    return False


# Lacuna 14 da C8 (dev3.3): os mesmos casos no desenho do celular (janela estreita), com
# RUSTINHO_CASOS_CELULAR=1. A janela fica com RUSTINHO_CASOS_CELULAR_DP (padrão 360x780), densidade 1 (os pontos
# dos textos e os cliques valem o mesmo).
CELULAR = os.environ.get("RUSTINHO_CASOS_CELULAR") == "1"
CELULAR_DP = tuple(int(v) for v in os.environ.get("RUSTINHO_CASOS_CELULAR_DP", "360x780").split("x"))


class Rustinho(base.Rustinho):
    def __init__(self, pasta, tela, props, fotos, nome):
        if CELULAR:
            self._abrir_celular(pasta, tela, props, fotos, nome)
        else:
            super().__init__(pasta, tela, props, fotos, nome)
        if CELULAR and conferir_ambiente(self.p.pid):
            self._preparar_janela_celular(*CELULAR_DP)
        if not conferir_ambiente(self.p.pid):
            self.fechar()
            raise RuntimeError("o Rustinho do teste não está na tela virtual (ou tem Wayland): parei antes de clicar")
        self.ambiente_ok = True

    def _abrir_celular(self, pasta, tela, props, fotos, nome):
        """Como base.Rustinho, com o desenho do celular (--celular)."""
        import json
        self.fotos, self.nome = fotos, nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true", QT_SCALE_FACTOR="1")
        env.pop("WAYLAND_DISPLAY", None)
        self.log = open(self.saida, "w")
        self.p = subprocess.Popen(
            # --medir-toque: as linhas TOQUE dizem onde está cada campo (para tocar nele antes de digitar).
            [PROGRAMA, "--textos-da-tela", "--celular", "--medir-toque", "--abrir", tela, "--props", json.dumps(props)],
            env=env, stdout=self.log, stderr=self.log,
        )

    # Instrumentação do modo celular (lacuna 14). Na tela virtual não há gerenciador de janelas: o teclado vai para a
    # janela que tem o foco (ou, sem foco definido, para a que está embaixo do ponteiro), e o Xvfb começa com o
    # ponteiro no meio da tela (x = 720), fora da janela estreita. Sem isto, o Ctrl+Shift+F12 dos textos da tela se
    # perdia e o roteiro nunca lia a tela (1ª rodada, c8_casos_celular_v1: "a tela de entrada não apareceu").
    def _geometria(self, janela):
        g = subprocess.run(["xdotool", "getwindowgeometry", "--shell", janela],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        d = dict(x.split("=", 1) for x in g.stdout.split() if "=" in x)
        return int(d.get("X", -1)), int(d.get("Y", -1)), int(d.get("WIDTH", 0)), int(d.get("HEIGHT", 0))

    def _esperar_janela(self):
        """A janela de verdade deste Rustinho (visível, de mais de 200 px), em até 30 s."""
        for _ in range(150):
            s = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.p.pid)],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            for j in s.stdout.split():
                if self._geometria(j)[2] > 200:
                    return j
            time.sleep(0.2)
        raise RuntimeError(f"INSTRUMENTAÇÃO: a janela do Rustinho não apareceu em 30 s (veja {self.saida})")

    def _preparar_janela_celular(self, largura, altura):
        """Espera a janela, põe no canto (os pontos dos textos valem como pontos da tela), dá o tamanho do
        celular e o foco do teclado, e confere lendo a geometria de volta."""
        janela = self._esperar_janela()
        for _ in range(20):
            xdo("windowmove", janela, "0", "0")
            xdo("windowsize", janela, str(largura), str(altura))
            time.sleep(0.5)
            x, y, w, h = self._geometria(janela)
            if (x, y, w, h) == (0, 0, largura, altura):
                break
        else:
            raise RuntimeError(f"INSTRUMENTAÇÃO: a janela ficou em {x},{y} {w}×{h}, e não em 0,0 {largura}×{altura}")
        self._janela = janela
        self._altura = altura
        xdo("windowfocus", "--sync", janela)
        self._ponteiro_dentro()
        time.sleep(1.5)  # o desenho assenta no tamanho novo
        print(f"INSTRUMENTAÇÃO {self.nome}: janela {janela} em 0,0 com {largura}×{altura}, foco e ponteiro nela",
              flush=True)

    def _ponteiro_dentro(self):
        """O ponteiro no canto de baixo, à direita, da janela (como no roteiro de toque), e o foco de volta nela:
        as teclas vão para o Rustinho mesmo depois de uma foto ou de um clique fora."""
        largura, altura = CELULAR_DP
        xdo("mousemove", str(largura - 4), str(altura - 4))
        if getattr(self, "_janela", None):
            xdo("windowfocus", "--sync", self._janela)
        time.sleep(0.15)

    def textos(self):
        """Os textos da tela. No celular, as linhas da medição de toque (TOQUE:, RET:) ficam à parte, em
        self._medidas: elas trazem nomes de botões e não podem contar como texto escrito na tela."""
        if not CELULAR:
            return super().textos()
        self._ponteiro_dentro()
        tudo = super().textos()
        self._medidas = [l for l in tudo if l[2].startswith(("TOQUE:", "RET:"))]
        return [l for l in tudo if not l[2].startswith(("TOQUE:", "RET:"))]

    def pilha(self):
        for _, _, t in self.textos():
            if t.startswith("TESTE_PILHA:"):
                return int(t.split(":")[1])
        return None

    def tem(self, pedaco):
        return any(simples(pedaco) in simples(t) for _, _, t in self.textos())

    def tem_igual(self, texto):
        return any(simples(texto) == simples(t) for _, _, t in self.textos())

    def setas(self):
        """[(x, y, largura, altura)] das setas de retorno: a da tela e, por
        cima, a de cada janelinha aberta."""
        return [tuple(int(v) for v in t.rsplit(":", 1)[1].split(",")) for _, _, t in self.textos()
                if t.startswith("GEO:setaDeRetorno:")]

    def seta_so_desenho(self):
        """A seta da tela é só o desenho, sem texto ao lado (D-V1, correção definitiva do dono, 07/11/2026):
        a área de toque fica quadrada."""
        s = self.setas()
        return bool(s) and s[0][2] == s[0][3]

    def onde(self, texto):
        """[(x, y)] de cada texto igual a este."""
        return [(x, y) for x, y, t in self.textos() if simples(t) == simples(texto)]

    def clicar_em(self, x, y):
        xdo("mousemove", str(x), str(y), "click", "1")

    def altura(self):
        """Altura da janela do Rustinho (na tela virtual, sem gerenciador de
        janelas, ela fica no tamanho de partida, não maximizada)."""
        if getattr(self, "_altura", None):
            return self._altura
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(self.p.pid)],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        for janela in s.stdout.split():
            g = subprocess.run(["xdotool", "getwindowgeometry", "--shell", janela],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            d = dict(x.split("=", 1) for x in g.stdout.split() if "=" in x)
            if int(d.get("HEIGHT", "0")) > 300:
                self._altura = int(d["HEIGHT"])
                return self._altura
        return 760

    def clicar(self, texto, comeca=False):
        """No celular (lacuna 14, 16/11/2026): o alvo pode estar fora da janela, abaixo (os quadrados das notificações
        no Início) ou acima. Arrasta até ele aparecer e só então toca; antes, o toque caía fora da janela. No
        computador, como sempre."""
        if not CELULAR:
            return super().clicar(texto, comeca)
        for _ in range(12):
            pontos = [(x, y) for x, y, t in self.textos()
                      if simples(t) == simples(texto) or (comeca and simples(t).startswith(simples(texto)))]
            if not pontos:
                return False
            x, y = pontos[0]
            if 60 <= y < self.altura() - 30:
                xdo("mousemove", str(x), str(y), "click", "1")
                return True
            self.arrastar(para_cima=y >= self.altura() - 30)
        return False

    def arrastar(self, para_cima=True, fracao=0.35):
        """No celular: um arrasto sem impulso (x = 20, dentro da rolagem). Para cima mostra o que está abaixo; para
        baixo, o que está acima."""
        largura, altura = CELULAR_DP
        y0, y1 = (int(altura * 0.75), int(altura * (0.75 - fracao))) if para_cima else \
                 (int(altura * 0.35), int(altura * (0.35 + fracao)))
        xdo("mousemove", "20", str(y0))
        xdo("mousedown", "1")
        for i in range(1, 9):
            xdo("mousemove", "20", str(y0 + (y1 - y0) * i // 8))
            time.sleep(0.03)
        time.sleep(0.15)
        xdo("mouseup", "1")
        time.sleep(0.8)

    def clicar_ultimo(self, texto):
        """O último (mais embaixo) texto igual a este, dentro da janela."""
        pontos = [p for p in self.onde(texto) if 0 <= p[1] < self.altura() - 10]
        if not pontos:
            return False
        x, y = max(pontos, key=lambda p: p[1])
        self.clicar_em(x, y)
        return True

    def rolar_ate(self, texto, x=600, y=500, vezes=25):
        """Rola (roda do mouse) até o texto ficar dentro da janela."""
        if CELULAR:
            return self._rolar_ate_arrastando(texto, vezes)
        for _ in range(vezes):
            pontos = self.onde(texto)
            if any(60 <= p[1] < self.altura() - 30 for p in pontos):
                return True
            xdo("mousemove", str(x), str(y))
            for _ in range(4):
                xdo("click", "5")
            time.sleep(0.4)
        return any(60 <= p[1] < self.altura() - 30 for p in self.onde(texto))

    def _rolar_ate_arrastando(self, texto, vezes=25):
        """No celular, rolar é arrastar o dedo (a roda do mouse não rolou a tela no desenho do celular, 3ª execução
        do caso 8). O arrasto começa perto da margem esquerda (x = 20, já dentro da rolagem), e sobe 40% da
        altura; nada é tocado no caminho."""
        largura, altura = CELULAR_DP
        def a_vista():
            return any(60 <= p[1] < self.altura() - 30 for p in self.onde(texto))
        for _ in range(vezes):
            if a_vista():
                return True
            # x = 20: dentro da área que rola (a margem da tela no celular é 14; em x = 6, o arrasto caía fora da
            # rolagem e nada rolava: a parte de instrumentação do 8b "inconclusivo", 15/11).
            x, y0, y1 = 20, int(altura * 0.75), int(altura * 0.35)
            xdo("mousemove", str(x), str(y0))
            xdo("mousedown", "1")
            for i in range(1, 9):
                xdo("mousemove", str(x), str(y0 - (y0 - y1) * i // 8))
                time.sleep(0.03)
            time.sleep(0.15)  # parado antes de soltar: sem o impulso que joga a lista longe
            xdo("mouseup", "1")
            time.sleep(0.6)
        return a_vista()

    def esperar(self, condicao, segundos=8):
        fim = time.time() + segundos
        while time.time() < fim:
            try:
                if condicao():
                    return True
            except RuntimeError:
                pass
            time.sleep(0.5)
        return condicao()

    def arrastar_para_cima(self, fracao=0.3):
        """No celular: um arrasto curto para cima (x = 20, dentro da rolagem), como a pessoa faz para ver inteiro o
        campo que apareceu no pé da tela antes de tocar nele."""
        largura, altura = CELULAR_DP
        y0 = int(altura * 0.75)
        y1 = int(y0 - altura * fracao)
        xdo("mousemove", "20", str(y0))
        xdo("mousedown", "1")
        for i in range(1, 9):
            xdo("mousemove", "20", str(y0 - (y0 - y1) * i // 8))
            time.sleep(0.03)
        xdo("mouseup", "1")
        time.sleep(0.8)

    def digitar(self, texto, rotulo=None):
        if CELULAR:
            self._tocar_no_campo(rotulo)
        xdo("type", "--delay", "70", texto)

    def _tocar_no_campo(self, rotulo=None):
        """No celular, de propósito, o campo não ganha o foco sozinho (Campo.qml, podeFocar: trocar de tela não abre
        o teclado). A pessoa toca no campo antes de digitar; o roteiro faz o mesmo: toca no centro do último campo de
        texto à vista (a janelinha aberta é desenhada por último, então o campo dela vem por último)."""
        campos = []
        self.textos()
        for _, _, t in self._medidas:
            if t.startswith("TOQUE:campo|"):
                partes = t.split("|")
                x, y, w, h = (float(v) for v in partes[4].split(","))
                if partes[5] == "1" and 0 <= y and y + h <= CELULAR_DP[1] and 0 <= x and x + w <= CELULAR_DP[0]:
                    campos.append((round(x + w / 2), round(y + h / 2)))
        if not campos:
            raise RuntimeError("INSTRUMENTAÇÃO: nenhum campo de texto à vista para tocar antes de digitar")
        cx, cy = campos[-1]
        # Lacuna 14 (16/11/2026): com o rótulo, o campo logo abaixo dele (o último campo à vista era o do km, e a placa
        # do caso 6 ia parar lá).
        if rotulo:
            r = [(x, y) for x, y, t in self.textos() if simples(t) == simples(rotulo)]
            if not r:
                raise RuntimeError(f"INSTRUMENTAÇÃO: o rótulo {rotulo!r} não está à vista para achar o campo dele")
            rx, ry = r[0]
            abaixo = sorted((cy2 - ry, abs(cx2 - rx), cx2, cy2) for cx2, cy2 in campos if 0 < cy2 - ry < 90)
            if not abaixo:
                raise RuntimeError(f"INSTRUMENTAÇÃO: nenhum campo logo abaixo do rótulo {rotulo!r}")
            cx, cy = abaixo[0][2], abaixo[0][3]
        xdo("mousemove", str(cx), str(cy), "click", "1")
        time.sleep(0.5)


def confirmar(r, botao):
    """Confirma a janelinha: no computador, o Enter (como a pessoa faz no teclado); no celular, o toque no botão dela
    (o campo não ganha o foco sozinho no celular, e o Enter não chega: lacuna 14, 16/11/2026)."""
    if CELULAR:
        if not r.clicar_ultimo(botao):
            raise RuntimeError(f"INSTRUMENTAÇÃO: o botão {botao!r} não está à vista para confirmar")
        time.sleep(0.4)
    else:
        xdo("key", "Return")


class Placar:
    def __init__(self):
        self.certos = 0
        self.errados = 0
        self.logs = []

    def ok(self, nome, condicao, foto=""):
        condicao = bool(condicao)
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"), flush=True)
        if condicao:
            self.certos += 1
        else:
            self.errados += 1
        return condicao

    def abrir(self, pasta, tela, props, fotos, nome):
        r = Rustinho(pasta, tela, props, fotos, nome)
        self.logs.append(r.saida)
        self.ok(f"{nome}: Rustinho na tela virtual {TELA}, sem Wayland", r.ambiente_ok)
        return r


# ---------------------------------------------------------------------------
# Caso 8: modo seguro → Configurações › Backup (restaurar).
# ---------------------------------------------------------------------------
BOTAO_8 = "Restaurar um backup"


def lista_backups(pasta):
    b = os.path.join(pasta, "backups")
    return sorted(n for n in os.listdir(b) if not n.startswith(".")) if os.path.isdir(b) else []


def caso8(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav8-")
    try:
        montar_loja(pasta)
        # Um backup bom, feito pela tela, antes do defeito.
        r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "backup"}, fotos, "8-0-backup-bom")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.tem("Fazer backup agora"))
            r.clicar("Fazer backup agora")
            ok("8: backup bom feito antes do defeito", r.esperar(lambda: len(lista_backups(pasta)) > 0, 20), r.foto())
        finally:
            r.fechar()
        db = os.path.join(pasta, "rustinho.db")
        seguro.estragar_lista_livre(db)
        antes = sha(db)
        backups_antes = lista_backups(pasta)

        # 8a. Na entrada, a faixa aparece sem o botão (ninguém dentro).
        r = placar.abrir(pasta, "TelaVender", {}, fotos, "8a-caixa")
        try:
            ok("8a: na entrada, a faixa do modo seguro aparece",
               r.esperar(lambda: r.tem("Modo seguro"), 30), r.foto("-0"))
            ok("8a: na entrada, sem o botão (ninguém dentro)", not r.tem_igual(BOTAO_8), r.foto("-0"))
            r.entrar("Bia")
            f = r.foto("-1")
            vender = r.pilha()
            ok("8a: dentro (caixa), a faixa tem o botão " + BOTAO_8, r.tem_igual(BOTAO_8), f)
            r.clicar(BOTAO_8)
            f = r.foto("-2")
            ok("8a: o caixa: pede a senha do gerente", r.esperar(lambda: r.tem(PEDIDO) or r.tem("Liberar")), f)
            ok("8a: Configurações por cima da tela de antes (uma só)", r.pilha() == vender + 1, f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("8a: desistir da senha (Esc) volta à tela de antes", r.esperar(lambda: r.pilha() == vender), f)
            ok("8a: a faixa continua lá", r.tem("Modo seguro") and r.tem_igual(BOTAO_8), f)
            r.clicar(BOTAO_8)
            r.esperar(lambda: r.tem("Liberar"))
            time.sleep(0.5)
            r.digitar(SENHA["Ana"])
            xdo("key", "Return")
            f = r.foto("-4")
            ok("8a: com a senha, abre Simultaneidade › Backup",
               r.esperar(lambda: r.tem("Fazer backup agora") and r.tem("Restaurar")), f)
            ok("8a: uma Configurações só", r.pilha() == vender + 1, f)
            r.clicar(BOTAO_8)
            time.sleep(1.5)
            f = r.foto("-5")
            ok("8a: o botão de novo, já no Backup, não empilha outra (sem ciclo)", r.pilha() == vender + 1, f)
            xdo("key", "Escape")
            f = r.foto("-6")
            ok("8a: Esc volta à tela de antes", r.esperar(lambda: r.pilha() == vender), f)
        finally:
            r.fechar()
        time.sleep(1)
        ok("8a: o banco não mudou (nada gravado)", sha(db) == antes)
        ok("8a: os backups continuam os mesmos", lista_backups(pasta) == backups_antes)

        # 8b. Gerente, pelo Início: o aviso diz "Ver backup"; o botão da faixa
        # leva ao Backup; restaurar pede confirmação e a senha; nada apagado.
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "8b-gerente")
        try:
            r.entrar("Ana")
            f = r.foto("-0")
            inicio = r.pilha()
            ok("8b: o aviso do modo seguro no Início leva ao Backup (\"Ver backup\")", r.tem_igual("Ver backup"), f)
            r.clicar(BOTAO_8)
            f = r.foto("-1")
            ok("8b: o gerente vai direto ao Backup (sem pedir senha)",
               r.esperar(lambda: r.tem("Fazer backup agora")) and not r.tem(PEDIDO), f)
            ok("8b: a seta é só o desenho e a tela está por cima do Início", r.seta_so_desenho() and r.pilha() == inicio + 1, f)
            ok("8b: o backup bom está na lista", r.rolar_ate("Restaurar") and len(r.onde("Restaurar")) >= 2, r.foto("-2"))
            r.clicar_ultimo("Restaurar")
            f = r.foto("-3")
            ok("8b: restaurar pede confirmação antes", r.esperar(lambda: r.tem("Restaurar o backup de")), f)
            ok("8b: ainda nada restaurado", sha(db) == antes, f)
            # 0.9.18 (V-4): a confirmação não tem mais "Cancelar"; a seta
            # dela (nome "Voltar", a de cima) desiste, igual ao Esc.
            r.clicar_seta_de_cima()
            time.sleep(1)
            f = r.foto("-4")
            ok("8b: desistir da confirmação (seta) não restaura", not r.tem("Restaurar o backup de") and sha(db) == antes, f)
            r.rolar_ate("Restaurar")
            r.clicar_ultimo("Restaurar")
            r.esperar(lambda: r.tem("Restaurar o backup de"))
            r.clicar("Continuar")
            f = r.foto("-5")
            ok("8b: depois da confirmação, pede a senha de um gerente",
               r.esperar(lambda: r.tem("Senha de um gerente (backup de antes")), f)
            ok("8b: ainda nada restaurado sem a senha", sha(db) == antes, f)
            time.sleep(0.5)
            if CELULAR:
                # O campo da senha aparece no pé da tela, cortado: a pessoa rola um pouco antes de tocar nele.
                r.arrastar_para_cima()
            r.digitar(SENHA["Ana"])
            xdo("key", "Return")
            f = r.foto("-6")
            ok("8b: restaurado, vai para a entrada (não retoma a operação de antes)",
               r.esperar(lambda: r.pilha() == 1 and r.tem("Ana") and not r.tem("Fazer backup agora"), 60), f)
            ok("8b: avisa que restaurou", r.tem("restaurado"), f)
            ok("8b: a pilha recomeçou (entrada)", inicio is not None and r.pilha() == 1, f)
        finally:
            r.fechar()
        depois = lista_backups(pasta)
        ok("8b: nenhum backup apagado", all(b in depois for b in backups_antes), str(sorted(set(backups_antes) - set(depois))))
        ok("8b: o banco foi trocado pelo backup (só depois da senha)", sha(db) != antes)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# Caso 5: transferência entre contas sem conta de destino.
# ---------------------------------------------------------------------------
def campo_com(r, texto):
    """O campo (texto mostrado) existe com este conteúdo."""
    return r.tem_igual(texto)


def caso5(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav5-")
    try:
        montar_loja(pasta)
        ativas = ler(pasta, "SELECT nome FROM conta_loja WHERE ativo = 1")
        if len(ativas) > 1:
            por_fora(pasta, "UPDATE conta_loja SET ativo = 0 WHERE id <> 'caixa-geral'")
        ok("5: a loja começa com uma conta só", len(ler(pasta, "SELECT 1 FROM conta_loja WHERE ativo = 1")) == 1)
        movimentos = lambda: ler(pasta, "SELECT COUNT(*) FROM movimento_conta")[0][0]  # noqa: E731
        antes = movimentos()
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "contas"}, fotos, "5-transferir")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.tem_igual("Transferir"))
            pilha = r.pilha()
            r.clicar("Transferir")
            f = r.foto("-1")
            ok("5: sem outra conta, a transferência mostra o botão Cadastrar conta",
               r.esperar(lambda: r.tem_igual("Cadastrar conta")), f)
            ok("5: e diz que o que foi digitado continua", r.tem("o que você já digitou aqui continua"), f)
            ok("5: o botão Transferir fica desligado (não há destino)", r.tem("Transferir de Caixa geral"), f)
            # O que a pessoa já digitou: valor e observação.
            r.digitar("123,45")
            xdo("key", "Tab")
            xdo("key", "Tab")
            time.sleep(0.3)
            r.digitar("Deposito do cofre")
            time.sleep(0.5)
            f = r.foto("-2")
            ok("5: valor e observação digitados", campo_com(r, "123,45") and campo_com(r, "Deposito do cofre"), f)
            # Desistir do cadastro (Esc): volta à transferência como estava.
            r.clicar("Cadastrar conta")
            f = r.foto("-3")
            ok("5: abre o cadastro da conta nova por cima da transferência",
               r.esperar(lambda: r.tem("Nova conta da loja")) and r.tem("Transferir de Caixa geral"), f)
            xdo("key", "Escape")
            time.sleep(1)
            f = r.foto("-4")
            ok("5: desistir do cadastro (Esc) volta à transferência, sem perder nada",
               not r.tem("Nova conta da loja") and r.tem("Transferir de Caixa geral")
               and campo_com(r, "123,45") and campo_com(r, "Deposito do cofre"), f)
            ok("5: nada cadastrado ao desistir", len(ler(pasta, "SELECT 1 FROM conta_loja")) == len(ler(pasta, "SELECT 1 FROM conta_loja WHERE id = 'caixa-geral'")) + max(0, len(ativas) - 1))
            # Cadastrar de verdade.
            r.clicar("Cadastrar conta")
            r.esperar(lambda: r.tem("Nova conta da loja"))
            time.sleep(0.5)
            r.digitar("Banco Teste")
            time.sleep(0.3)
            r.clicar("Salvar")
            f = r.foto("-5")
            ok("5: salvou a conta nova e voltou à transferência",
               r.esperar(lambda: not r.tem("Nova conta da loja") and r.tem("Transferir de Caixa geral")), f)
            ok("5: a conta nova já está escolhida como destino", r.tem_igual("Banco Teste"), f)
            ok("5: valor e observação continuam", campo_com(r, "123,45") and campo_com(r, "Deposito do cofre"), f)
            ok("5: a origem não mudou (Caixa geral)", r.tem("Transferir de Caixa geral"), f)
            ok("5: o botão Cadastrar conta sumiu (já há destino)", not r.tem_igual("Cadastrar conta"), f)
            ok("5: a conta nova está gravada", len(ler(pasta, "SELECT 1 FROM conta_loja WHERE nome = 'Banco Teste'")) == 1)
            ok("5: nada transferido sozinho depois de cadastrar", movimentos() == antes)
            ok("5: nenhuma tela empilhada (sem ciclo)", r.pilha() == pilha, f)
            r.clicar_ultimo("Transferir")
            f = r.foto("-6")
            ok("5: só o clique em Transferir transfere",
               r.esperar(lambda: movimentos() > antes and not r.tem("Transferir de Caixa geral")), f)
            destino = ler(pasta, "SELECT m.valor FROM movimento_conta m JOIN conta_loja c ON c.id = m.conta_id "
                                 "WHERE c.nome = 'Banco Teste'")
            ok("5: R$ 123,45 na conta nova", destino == [(12345,)], str(destino))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# Caso 4: etiquetas bloqueadas por código ambíguo.
# ---------------------------------------------------------------------------
def caso4(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav4-")
    try:
        montar_loja(pasta)
        compra = ler(pasta, "SELECT id FROM compra ORDER BY rowid LIMIT 1")[0][0]
        itens = ler(pasta, "SELECT DISTINCT i.produto_id, p.nome FROM compra_item i JOIN produto p ON p.id = i.produto_id "
                           "WHERE i.compra_id = ? ORDER BY i.ordem", compra)
        travada_id, travada = itens[0]
        filtro_codigo = ler(pasta, "SELECT codigo FROM produto WHERE nome = 'Filtro de óleo'")[0][0]
        # O código da primeira peça da compra passa a ser também o do filtro
        # (como dois caixas que deram o mesmo código).
        por_fora(pasta, f"UPDATE produto SET codigo = '{filtro_codigo}' WHERE id = '{travada_id}'")
        codigo = lambda pid: ler(pasta, "SELECT codigo FROM produto WHERE id = ?", pid)[0][0]  # noqa: E731
        impresso = lambda: ler(pasta, "SELECT COUNT(*) FROM config WHERE chave = 'etiqueta_modelo'")[0][0]  # noqa: E731
        ok("4: a peça da compra ficou com o código de outra", codigo(travada_id) == filtro_codigo)
        r = placar.abrir(pasta, "TelaEtiquetas", {"compraId": compra}, fotos, "4-etiquetas")
        try:
            r.entrar("Ana")
            f = r.foto("-0")
            r.esperar(lambda: r.tem("Não saem agora"))
            f = r.foto("-1")
            etiquetas = r.pilha()
            ok("4: a peça bloqueada fica à parte (Não saem agora), com o nome", r.tem("Não saem agora") and r.tem(travada), f)
            ok("4: com a causa (o código leva a outra peça)", r.tem("também é da peça") and r.tem("Filtro de óleo"), f)
            ok("4: e a parte certa (Simultaneidade › Duplicatas)", r.tem("Simultaneidade › Duplicatas"), f)
            ok("4: as opções Ver duplicatas e Abrir a peça", r.tem_igual("Ver duplicatas") and r.tem_igual("Abrir a peça"), f)
            outras = [n for _, n in itens[1:]]
            ok("4: as outras peças da compra estão na lista", all(r.tem(n) for n in outras), f)
            lista_antes = sorted(t for _, _, t in r.textos() if simples(t).endswith("etiq.") or "etiqueta" in simples(t))
            # Ver duplicatas e voltar sem resolver: continua bloqueada.
            r.clicar("Ver duplicatas")
            f = r.foto("-2")
            ok("4: Ver duplicatas abre Simultaneidade › Duplicatas (códigos das peças)",
               r.esperar(lambda: r.tem("Códigos das peças") and r.tem_igual("Dar código novo")), f)
            ok("4: Configurações por cima das Etiquetas (uma só)", r.pilha() == etiquetas + 1, f)
            ok("4: a seta é só o desenho", r.seta_so_desenho(), f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("4: Esc volta às Etiquetas", r.esperar(lambda: r.pilha() == etiquetas), f)
            ok("4: sem resolver, continua bloqueada (nada liberado)",
               r.tem("Não saem agora") and r.tem("também é da peça") and codigo(travada_id) == filtro_codigo, f)
            lista_depois = sorted(t for _, _, t in r.textos() if simples(t).endswith("etiq.") or "etiqueta" in simples(t))
            ok("4: a lista de etiquetas continua igual", lista_antes == lista_depois, f)
            # Abrir a peça e voltar.
            r.clicar("Abrir a peça")
            f = r.foto("-4")
            ok("4: Abrir a peça abre o cadastro da peça", r.esperar(lambda: r.pilha() == etiquetas + 1 and r.tem(travada)), f)
            ok("4: na peça, a seta é só o desenho", r.seta_so_desenho(), f)
            xdo("key", "Escape")
            f = r.foto("-5")
            ok("4: Esc volta às Etiquetas, ainda bloqueada",
               r.esperar(lambda: r.pilha() == etiquetas) and r.tem("Não saem agora"), f)
            # Resolver em Duplicatas: código novo para a peça da compra.
            r.clicar("Ver duplicatas")
            r.esperar(lambda: r.tem_igual("Dar código novo"))
            r.rolar_ate(travada)
            nome = [(x, y) for x, y, t in r.textos() if simples(t).startswith(simples(travada))]
            botoes = r.onde("Dar código novo")
            if nome and botoes:
                y = nome[0][1]
                bx, by = min(botoes, key=lambda b: abs(b[1] - y))
                r.clicar_em(bx, by)
            f = r.foto("-6")
            ok("4: o gerente deu código novo à peça (em Duplicatas)",
               r.esperar(lambda: codigo(travada_id) != filtro_codigo), f)
            ok("4: o código do filtro não mudou", codigo(ler(pasta, "SELECT id FROM produto WHERE nome = 'Filtro de óleo'")[0][0]) == filtro_codigo)
            xdo("key", "Escape")
            f = r.foto("-7")
            ok("4: de volta às Etiquetas", r.esperar(lambda: r.pilha() == etiquetas), f)
            ok("4: resolvida, sai das bloqueadas e volta para a lista",
               r.esperar(lambda: not r.tem("Não saem agora") and r.tem(travada)), f)
            ok("4: avisa que já pode sair", r.tem("já pode sair"), f)
            ok("4: as outras continuam na lista", all(r.tem(n) for n in outras), f)
            ok("4: nada impresso sem clique", impresso() == 0)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# Caso 6: rascunho de OS bloqueado por um caixa desatualizado.
# ---------------------------------------------------------------------------
TITULO_6 = "Rascunho de OS bloqueado por um caixa desatualizado"


# A placa do caso 6. Antes era "ABC1D23", o mesmo texto da dica do campo vazio: "com a placa digitada" passava mesmo
# sem nada digitado (achado na 1ª rodada do celular, 16/11/2026).
PLACA = "QRS4T56"


def caso6(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav6-")
    try:
        montar_loja(pasta)
        # Um caixa da loja que nunca confirmou a versão: bloqueia rascunho novo.
        por_fora(pasta, "INSERT INTO sync_par (dispositivo, nome, endereco) VALUES ('caixa-2', 'Caixa 2', '10.0.0.2:47810')")
        rascunhos = lambda: ler(pasta, "SELECT COUNT(*) FROM ordem_servico WHERE situacao = 'rascunho'")[0][0]  # noqa: E731

        def preencher_e_sair(r):
            r.esperar(lambda: r.tem("Nova OS"))
            time.sleep(0.8)
            r.digitar(PLACA, rotulo="Placa")
            time.sleep(0.5)
            xdo("key", "Escape")
            r.esperar(lambda: r.tem("Sair da OS?"))
            r.clicar("Salvar rascunho")
            return r.esperar(lambda: r.tem("Rascunho novo está bloqueado"))

        # 6b. Caixa (Bia): Ver caixas pede a senha; desistir volta à OS.
        r = placar.abrir(pasta, "TelaOrdem", {}, fotos, "6b-caixa")
        try:
            r.entrar("Bia")
            os_pilha = r.pilha()
            ok("6b: salvar rascunho bloqueado mostra o motivo na saída", preencher_e_sair(r), r.foto("-1"))
            r.clicar("Ver caixas")
            f = r.foto("-2")
            ok("6b: Ver caixas pede a senha do gerente (Simultaneidade)",
               r.esperar(lambda: r.tem(PEDIDO) or r.tem("Liberar")), f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("6b: desistir da senha volta à OS", r.esperar(lambda: r.pilha() == os_pilha and r.tem("Nova OS")), f)
            ok("6b: com a placa digitada", r.tem(PLACA), f)
            ok("6b: avisa que a operação não foi concluída", r.tem("ainda não foi concluída"), f)
            ok("6b: nenhum rascunho salvo", rascunhos() == 0)
        finally:
            r.fechar()

        # 6a. Gerente (Ana): opções, desistência, resolução e volta.
        r = placar.abrir(pasta, "TelaOrdem", {}, fotos, "6a-gerente")
        try:
            r.entrar("Ana")
            os_pilha = r.pilha()
            ok("6a: salvar rascunho bloqueado mostra o motivo na saída", preencher_e_sair(r), r.foto("-1"))
            f = r.foto("-1")
            # 0.9.18 (V-4): o "Continuar sem rascunho" (só fechava) virou a
            # seta da janelinha (nome "Voltar"), igual ao Esc.
            ok("6a: opções: Ver caixas, a seta (continuar sem rascunho), Sair sem salvar",
               r.tem_igual("Ver caixas") and len(r.setas()) == 2 and r.tem_igual("Sair sem salvar"), f)
            ok("6a: o motivo diz o caixa e o lugar", r.tem("Caixa 2") and r.tem("Simultaneidade › Caixas"), f)
            ok("6a: nada mudou de tela sozinho", r.pilha() == os_pilha, f)
            r.clicar_seta_de_cima()
            time.sleep(1)
            f = r.foto("-2")
            ok("6a: a seta da janelinha volta à OS, com o que foi digitado",
               not r.tem("Sair da OS?") and r.tem(PLACA) and r.pilha() == os_pilha, f)
            xdo("key", "Escape")
            r.esperar(lambda: r.tem("Sair da OS?"))
            r.clicar("Salvar rascunho")
            r.esperar(lambda: r.tem_igual("Ver caixas"))
            r.clicar("Ver caixas")
            f = r.foto("-3")
            ok("6a: Ver caixas leva a Simultaneidade › Caixas com a orientação",
               r.esperar(lambda: r.tem(TITULO_6) and r.tem("Versão dos caixas")), f)
            ok("6a: o caixa que bloqueia aparece (Caixa 2)", r.tem("Caixa 2"), f)
            ok("6a: a seta é só o desenho", r.seta_so_desenho(), f)
            ok("6a: uma tela por cima da OS (sem ciclo)", r.pilha() == os_pilha + 1, f)
            xdo("key", "Escape")
            f = r.foto("-4")
            ok("6a: desistir (Esc) volta à OS", r.esperar(lambda: r.pilha() == os_pilha and r.tem(PLACA)), f)
            ok("6a: avisa que a operação não foi concluída", r.tem("ainda não foi concluída"), f)
            ok("6a: nenhum rascunho salvo", rascunhos() == 0)
            # Resolver: o gerente retira o caixa da loja (com motivo).
            xdo("key", "Escape")
            r.esperar(lambda: r.tem("Sair da OS?"))
            r.clicar("Salvar rascunho")
            r.esperar(lambda: r.tem_igual("Ver caixas"))
            r.clicar("Ver caixas")
            r.esperar(lambda: r.tem(TITULO_6))
            r.rolar_ate("Retirar da loja")
            r.clicar_ultimo("Retirar da loja")
            r.esperar(lambda: r.tem("Retirar Caixa 2 da loja?"))
            time.sleep(0.5)
            r.digitar("Computador vendido")
            time.sleep(0.3)
            f = r.foto("-5")
            ok("6a: retirar pede o motivo (decisão do gerente)", r.tem("Computador vendido"), f)
            ok("6a: ainda nenhum rascunho", rascunhos() == 0)
            xdo("key", "Return")
            f = r.foto("-6")
            ok("6a: resolvido, volta à OS (o núcleo confirmou)",
               r.esperar(lambda: r.pilha() == os_pilha and r.tem("Sair da OS?"), 12), f)
            ok("6a: avisa \"Requisito satisfeito — conclua a operação.\"", r.tem("Requisito satisfeito"), f)
            ok("6a: a saída abre de novo com Salvar rascunho", r.tem_igual("Salvar rascunho") and not r.tem_igual("Ver caixas"), f)
            ok("6a: o rascunho NÃO foi salvo sozinho", rascunhos() == 0)
            ok("6a: o caixa foi retirado", len(ler(pasta, "SELECT 1 FROM config WHERE chave = 'caixa_retirado:caixa-2' AND valor <> ''")) == 1)
            r.clicar("Salvar rascunho")
            f = r.foto("-7")
            ok("6a: com o clique, o rascunho é salvo (com a placa)",
               r.esperar(lambda: rascunhos() == 1) and ler(pasta, "SELECT placa FROM ordem_servico WHERE situacao = 'rascunho'") == [(PLACA,)], f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# Caso 9 (conferência): os links das notificações do Início.
# ---------------------------------------------------------------------------
def caso9(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav9-")
    try:
        montar_loja(pasta, avisos=True)
        # Backup: começou há 10 dias e nunca teve um (notificação de atenção).
        por_fora(pasta, "INSERT OR REPLACE INTO config (chave, valor) VALUES "
                        "('backup_desde', strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-10 days'))")
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "9-gerente")
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            f = r.foto("-0")
            ok("9: Início com as notificações (Ver duplicatas, Ver caixas)",
               r.tem_igual("Ver duplicatas") and r.tem_igual("Ver caixas"), f)
            for botao, parte, texto in [("Ver duplicatas", "9a", "Diferenças entre caixas"),
                                        ("Ver caixas", "9c", "Caixas da loja")]:
                r.clicar(botao)
                f = r.foto("-" + parte)
                ok(f"{parte}: {botao} abre a parte certa ({texto})",
                   r.esperar(lambda: r.tem(texto) and r.pilha() == inicio + 1), f)
                ok(f"{parte}: a seta é só o desenho", r.seta_so_desenho(), f)
                xdo("key", "Escape")
                ok(f"{parte}: Esc volta ao Início", r.esperar(lambda: r.pilha() == inicio), r.foto("-" + parte + "v"))
            # A marca "aberta/minimizada" é deste computador: só abre o
            # quadrado se a faixa ainda não estiver aberta.
            if not r.tem_igual("Ver backup"):
                r.clicar("Atenção: Backup")
            r.esperar(lambda: r.tem_igual("Ver backup"))
            time.sleep(1.2)  # a faixa termina de abrir (animação) antes do clique
            r.clicar("Ver backup")
            f = r.foto("-9d")
            ok("9d: Ver backup abre a parte Backup",
               r.esperar(lambda: r.tem("Fazer backup agora") and r.pilha() == inicio + 1), f)
            xdo("key", "Escape")
            ok("9d: Esc volta ao Início", r.esperar(lambda: r.pilha() == inicio), r.foto("-9dv"))
        finally:
            r.fechar()
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "9-caixa")
        try:
            r.entrar("Bia")
            inicio = r.pilha()
            # A marca "aberta/minimizada" é deste computador: só abre o
            # quadrado se a faixa ainda não estiver aberta.
            if not r.tem_igual("Ver backup"):
                r.clicar("Atenção: Backup")
            r.esperar(lambda: r.tem_igual("Ver backup"))
            time.sleep(1.2)  # a faixa termina de abrir (animação) antes do clique
            r.clicar("Ver backup")
            f = r.foto("-1")
            ok("9d (caixa): pede a senha do gerente", r.esperar(lambda: r.tem(PEDIDO)), f)
            ok("9d (caixa): sem o \"Só o gerente pode: backup\" por trás da senha",
               not r.tem("Só o gerente pode: backup"), f)
            xdo("key", "Escape")
            ok("9d (caixa): desistir volta ao Início", r.esperar(lambda: r.pilha() == inicio), r.foto("-2"))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# Caso 7 (dev3.3): operação com dinheiro de um registro inconsistente.
# ---------------------------------------------------------------------------
TITULO_7 = "Registro inconsistente: o dinheiro dele está bloqueado"


def caso7(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav7-")
    try:
        montar_loja(pasta)
        # O caixa aberto, uma parcela do Zé e a prova de que ela ficou
        # inconsistente (sem troca de dados no teste: nada recalcula a lista).
        por_fora(pasta, """
            INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, aberto_por, valor_inicial)
              SELECT 'sessao-nav7', valor, 'Caixa 1', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT id FROM usuario WHERE nome = 'Ana'), 0
                FROM config WHERE chave = 'dispositivo_id';
            INSERT INTO conta_receber (id, cliente_id, descricao, valor, vencimento, criado_em, dispositivo)
              SELECT 'cr-nav7', id, 'Caderno', 5000, date('now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT valor FROM config WHERE chave = 'dispositivo_id')
                FROM pessoa WHERE nome = 'Zé da Oficina';
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_receber', '["cr-nav7"]', 'pessoa', '["sumiu"]', 'inconsistente',
                      '2026-01-01T00:00:00Z', 'log');""")
        ze = ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
        # A loja de teste já tem recebimentos: conta a partir do que existe.
        ja_havia = ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0]
        recebimentos = lambda: ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0] - ja_havia  # noqa: E731
        valores = {}

        def dinheiro(r):
            """Os valores em dinheiro à vista (os da janelinha do recebimento)."""
            return sorted(t for _, _, t in r.textos() if re.fullmatch(r"(R\$ )?[\d.]+,\d\d", t.strip()))

        def receber(r):
            r.clicar("Receber tudo")
            r.esperar(lambda: r.tem("Receber de"))
            time.sleep(0.6)
            valores["antes"] = dinheiro(r)
            confirmar(r, "Receber")
            return r.esperar(lambda: r.tem_igual("Ver o registro"))

        r = placar.abrir(pasta, "TelaReceber", {"clienteInicial": ze}, fotos, "7-gerente")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.tem("Receber tudo"))
            base_pilha = r.pilha()
            f = r.foto("-1")
            ok("7: receber o registro inconsistente mostra o motivo e Ver o registro", receber(r), f)
            f = r.foto("-2")
            ok("7: o motivo diz o lugar (Caixas da loja) e que a operação não foi feita",
               r.tem("Caixas da loja") and r.tem("não foi feita"), f)
            ok("7: nada mudou de tela sozinho", r.pilha() == base_pilha, f)
            ok("7: nada recebido", recebimentos() == 0)
            r.clicar("Ver o registro")
            f = r.foto("-3")
            ok("7: Ver o registro leva a Simultaneidade › Caixas com a orientação",
               r.esperar(lambda: r.tem(TITULO_7)), f)
            ok("7: o registro aparece, com Aceitar como está", r.rolar_ate("Aceitar como está"), f)
            ok("7: uma tela por cima do Receber (sem ciclo)", r.pilha() == base_pilha + 1, f)
            ok("7: a seta é só o desenho", r.seta_so_desenho(), f)
            ok("7: a janelinha do recebimento fechou", not r.tem("Receber de"), f)
            xdo("key", "Escape")
            f = r.foto("-4")
            ok("7: desistir (Esc) volta ao Receber com a janelinha reaberta",
               r.esperar(lambda: r.pilha() == base_pilha and r.tem("Receber de")), f)
            ok("7: com os valores de antes", bool(valores["antes"]) and dinheiro(r) == valores["antes"],
               f"{f} antes={valores['antes']} depois={dinheiro(r)}")
            ok("7: avisa que a operação não foi concluída", r.tem("ainda não foi concluída"), f)
            ok("7: nada recebido", recebimentos() == 0)
            # Resolver: o gerente aceita como está (com motivo).
            confirmar(r, "Receber")
            r.esperar(lambda: r.tem_igual("Ver o registro"))
            r.clicar("Ver o registro")
            r.esperar(lambda: r.tem(TITULO_7))
            r.rolar_ate("Aceitar como está")
            r.clicar_ultimo("Aceitar como está")
            r.esperar(lambda: r.tem("Aceitar como está?"))
            time.sleep(0.5)
            r.digitar("Caixa de origem perdido")
            time.sleep(0.3)
            f = r.foto("-5")
            ok("7: aceitar pede o motivo (decisão do gerente)", r.tem("Caixa de origem perdido"), f)
            ok("7: ainda nada recebido", recebimentos() == 0)
            confirmar(r, "Aceitar como está")
            f = r.foto("-6")
            ok("7: resolvido, volta ao Receber (o núcleo confirmou), com a janelinha",
               r.esperar(lambda: r.pilha() == base_pilha and r.tem("Receber de"), 12), f)
            ok("7: avisa \"Requisito satisfeito — conclua a operação.\"", r.tem("Requisito satisfeito"), f)
            ok("7: o recebimento NÃO foi feito sozinho", recebimentos() == 0)
            ok("7: o registro ficou aceito pelo gerente",
               ler(pasta, "SELECT estado FROM sync_referencia WHERE chave = '[\"cr-nav7\"]'") == [("aceito",)])
            confirmar(r, "Receber")
            f = r.foto("-7")
            ok("7: com o clique, recebe", r.esperar(lambda: recebimentos() == 1), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------
# 9b, 9e e 10b (dev3.3).
# ---------------------------------------------------------------------------
AGORA = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
DISPOSITIVO = "(SELECT valor FROM config WHERE chave = 'dispositivo_id')"


def caso9b(placar, fotos):
    """Duplicatas → "Ver no histórico" filtra pelo registro do conflito (ligação inequívoca); "Ver tudo"; a pílula
    Duplicatas volta (sem botão de texto que só volta); nada é gravado e nenhuma tela é empilhada."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav9b-")
    try:
        montar_loja(pasta)
        por_fora(pasta, "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando) "
                        "SELECT 'produto', json_array(id), 'v1', 'v2', 'caixa-2', datetime('now') FROM produto "
                        "ORDER BY criado_em LIMIT 1")
        conflitos = lambda: ler(pasta, "SELECT COUNT(*) FROM sync_conflito WHERE resolvido_em IS NULL")[0][0]  # noqa: E731
        antes = conflitos()
        r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "duplicatas"}, fotos, "9b-gerente")
        try:
            r.entrar("Ana")
            ok("9b: Duplicatas mostra o conflito com Ver no histórico",
               r.esperar(lambda: r.rolar_ate("Ver no histórico")), r.foto("-1"))
            base_pilha = r.pilha()
            r.clicar("Ver no histórico")
            f = r.foto("-2")
            ok("9b: o Histórico mostra só o registro do conflito",
               r.esperar(lambda: r.tem("Só o que aconteceu com este registro")), f)
            ok("9b: com o evento do cadastro da peça", r.tem("cadastrou a peça"), f)
            ok("9b: a mesma tela (sem empilhar)", r.pilha() == base_pilha, f)
            ok("9b: sem botão de texto que só volta (a seta e as pílulas fazem isso)", not r.tem("Voltar às"), f)
            if CELULAR:  # lacuna 14: volta ao alto da página, onde as pílulas ficam à vista
                for _ in range(4):
                    r.arrastar(para_cima=False)
            r.clicar("Duplicatas")
            f = r.foto("-3")
            ok("9b: a pílula Duplicatas volta à lista", r.esperar(lambda: r.tem_igual("Ver no histórico")), f)
            r.rolar_ate("Ver no histórico")
            r.clicar("Ver no histórico")
            r.esperar(lambda: r.tem_igual("Ver tudo"))
            r.clicar("Ver tudo")
            f = r.foto("-4")
            ok("9b: Ver tudo tira o filtro do registro",
               r.esperar(lambda: not r.tem("Só o que aconteceu com este registro")), f)
            ok("9b: nada foi resolvido nem gravado", conflitos() == antes)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def caso9e(placar, fotos):
    """Início → a notificação Fiscal (UF/CNPJ faltando numa loja com nota de compra) → "Informar UF e CNPJ" leva
    à Loja pela guia; desistir volta ao Início sem gravar. (Salvar e voltar é o mesmo requisito do caso 3,
    conferido em telas_navegacao_etapa1.py, F.)"""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav9e-")
    try:
        montar_loja(pasta)
        com_xml = ler(pasta, "SELECT COUNT(*) FROM compra WHERE xml <> ''")[0][0]
        if not com_xml:
            por_fora(pasta, "UPDATE compra SET xml = '<nfe/>' WHERE id = (SELECT id FROM compra LIMIT 1)")
        cnpj = lambda: ler(pasta, "SELECT COALESCE((SELECT valor FROM config WHERE chave = 'loja_cnpj'), '')")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "9e-gerente")
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            if not r.tem_igual("Informar UF e CNPJ"):
                r.clicar("Fiscal")
            f = r.foto("-1")
            ok("9e: a notificação Fiscal oferece Informar UF e CNPJ", r.esperar(lambda: r.tem_igual("Informar UF e CNPJ")), f)
            ok("9e: não sugere emissão", r.tem("não emite nota fiscal") or not r.tem("emitir"), f)
            time.sleep(1.2)
            r.clicar("Informar UF e CNPJ")
            f = r.foto("-2")
            ok("9e: leva à Loja com a orientação da guia",
               r.esperar(lambda: r.tem("Para continuar, informe a UF e o CNPJ da loja")), f)
            ok("9e: uma tela por cima do Início (sem ciclo)", r.pilha() == inicio + 1, f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("9e: desistir (Esc) volta ao Início", r.esperar(lambda: r.pilha() == inicio), f)
            # O aviso vem depois que a troca de tela termina (Main.depoisDaTroca): espera por ele, em vez de olhar uma
            # vez só logo depois da volta (com a máquina carregada, a troca demora e o aviso chega depois da leitura).
            ok("9e: avisa que não foi concluída", r.esperar(lambda: r.tem("ainda não foi concluída"), 8), f)
            ok("9e: nada gravado", cnpj() == "")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def caso10b(placar, fotos):
    """Comissões: o fechamento já pago oferece "Ver no A pagar" (Opções); abre a ficha da conta; ao fechar, volta a
    Comissões. Nada muda de valor (o pagamento não é estornado, o fechamento não é desfeito)."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav10b-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-nav10b', 'cat-comissoes', 'Comissão de Ana · teste', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO pagamento (id, conta_pagar_id, data, valor, forma, criado_em, dispositivo)
              VALUES ('pg-nav10b', 'cp-nav10b', date('now'), 1000, 'dinheiro', {AGORA}, {DISPOSITIVO});
            INSERT INTO comissao_fechamento (id, usuario_id, de, ate, vendas, total_vendido, valor, conta_pagar_id,
                                             criado_em, dispositivo)
              SELECT 'cf-nav10b', id, date('now', 'start of month'), date('now'), 1, 10000, 1000, 'cp-nav10b',
                     {AGORA}, {DISPOSITIVO} FROM usuario WHERE nome = 'Ana';""")
        intacto = lambda: ler(pasta, "SELECT (SELECT estornado_em FROM pagamento WHERE id = 'pg-nav10b'), "  # noqa: E731
                                     "(SELECT cancelado_em FROM comissao_fechamento WHERE id = 'cf-nav10b'), "
                                     "(SELECT valor FROM conta_pagar WHERE id = 'cp-nav10b')") == [(None, None, 1000)]
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "10b-gerente")
        try:
            r.entrar("Ana")
            f = r.foto("-1")
            ok("10b: o fechamento pago oferece Ver no A pagar (e não Desfazer)",
               r.esperar(lambda: r.rolar_ate("Ver no A pagar")) and not r.tem_igual("Desfazer"), f)
            base_pilha = r.pilha()
            r.clicar("Ver no A pagar")
            f = r.foto("-2")
            ok("10b: abre a ficha da conta da comissão", r.esperar(lambda: r.tem("Comissão de Ana · teste")), f)
            ok("10b: na mesma tela (sem empilhar)", r.pilha() == base_pilha, f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("10b: fechar a ficha volta a Comissões", r.esperar(lambda: r.tem_igual("Ver no A pagar")), f)
            time.sleep(1.0)
            r.foto("-d")  # a volta, com o fechamento destacado (conferência visual)
            ok("10b: nada mudou de valor", intacto())
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def caso_destaque(placar, fotos):
    """9a, 9c e 10a: com o registro exato (alvo), a parte abre com a linha à vista e destacada (as fotos "-d"
    mostram o véu e a luz em volta da linha; o roteiro não enxerga o destaque, a foto sim); nada é gravado."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-navdest-")
    try:
        montar_loja(pasta, avisos=True)
        por_fora(pasta, "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando) "
                        "SELECT 'produto', json_array(id), 'v1', 'v2', 'caixa-2', datetime('now') FROM produto "
                        "ORDER BY criado_em LIMIT 1")
        conflito = ler(pasta, "SELECT MAX(id) FROM sync_conflito")[0][0]
        encomenda = ler(pasta, "SELECT id FROM encomenda WHERE situacao = 'aberta' LIMIT 1")[0][0]
        antes = ler(pasta, "SELECT (SELECT COUNT(*) FROM sync_conflito WHERE resolvido_em IS NULL), "
                           "(SELECT COUNT(*) FROM encomenda WHERE situacao = 'aberta')")
        r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "duplicatas", "alvo": f"conflito:{conflito}"}, fotos,
                         "destaque-9a")
        try:
            r.entrar("Ana")
            ok("9a: Duplicatas abre com o conflito à vista", r.esperar(lambda: r.tem_igual("Ver no histórico")), r.foto("-1"))
            time.sleep(1.5)
            r.foto("-d")
        finally:
            r.fechar()
        r = placar.abrir(pasta, "TelaEstoque", {"secao": "encomendas", "alvo": f"encomenda:{encomenda}"}, fotos,
                         "destaque-10a")
        try:
            r.entrar("Ana")
            ok("10a: Encomendas abre com a encomenda à vista", r.esperar(lambda: r.tem("Pastilha dianteira")), r.foto("-1"))
            time.sleep(1.5)
            r.foto("-d")
        finally:
            r.fechar()
        ok("9a/10a: nada resolvido nem gravado", ler(pasta, "SELECT (SELECT COUNT(*) FROM sync_conflito WHERE resolvido_em "
                                                           "IS NULL), (SELECT COUNT(*) FROM encomenda WHERE situacao = "
                                                           "'aberta')") == antes)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def caso10a(placar, fotos):
    """10a pelo Início: "Ver encomendas" (uma encomenda só) leva à linha dela em Estoque › Encomendas; o Esc
    (cancelar) volta ao Início sem empilhar nada e sem mudar a encomenda."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav10a-")
    try:
        montar_loja(pasta, avisos=True)
        situacao = lambda: ler(pasta, "SELECT id, situacao FROM encomenda ORDER BY id")  # noqa: E731
        antes = situacao()
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "10a-gerente")
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            if not r.tem_igual("Ver encomendas"):
                r.clicar("Encomendas")
            ok("10a: a notificação oferece Ver encomendas", r.esperar(lambda: r.tem_igual("Ver encomendas")), r.foto("-1"))
            time.sleep(1.2)
            r.clicar("Ver encomendas")
            f = r.foto("-2")
            ok("10a: leva a Estoque › Encomendas com a encomenda à vista",
               r.esperar(lambda: r.tem("Pastilha dianteira") and r.pilha() == inicio + 1), f)
            ok("10a: com as ações dela à vista (a linha escolhida)", r.tem_igual("Cancelar encomenda"), f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("10a: cancelar (Esc) volta ao Início, sem empilhar", r.esperar(lambda: r.pilha() == inicio), f)
            ok("10a: a encomenda não mudou", situacao() == antes)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def caso10a_repetido(placar, fotos):
    """I1 (decisão do proprietário, 15/11/2026; 0915-M): o mesmo número de encomenda em dois caixas. O Início avisa
    ("Mesmo número em caixas diferentes"), e "Ver encomendas" leva à encomenda do OUTRO caixa, à vista e com as ações
    dela; o Esc volta ao Início sem empilhar; nenhuma encomenda muda."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav10a2-")
    try:
        montar_loja(pasta, avisos=True)
        # Como a troca de dados traria: a encomenda do Caixa 2, com o mesmo número de uma deste caixa.
        por_fora(pasta, "CREATE TEMP TABLE outra AS SELECT * FROM encomenda WHERE situacao = 'aberta' LIMIT 1; "
                        "UPDATE outra SET id = 'enc-caixa-2', dispositivo = 'caixa-2', cliente_nome = 'Cliente do Caixa 2', "
                        "descricao = 'Correia dentada'; INSERT INTO encomenda SELECT * FROM outra;")
        situacao = lambda: ler(pasta, "SELECT id, situacao FROM encomenda ORDER BY id")  # noqa: E731
        antes = situacao()
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "10a2-gerente")
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            aviso = "Mesmo número em caixas diferentes"
            if not r.tem(aviso):
                r.clicar("Atenção: Número de encomenda repetido")  # minimizada: o quadrado com o nome
            ok("10a (I1): o Início avisa o número repetido em caixas diferentes", r.esperar(lambda: r.tem(aviso)),
               r.foto("-1"))
            time.sleep(1.2)
            # O botão da notificação aberta (a das encomendas pendentes fica minimizada: uma aberta por vez).
            r.clicar("Ver encomendas")
            f = r.foto("-2")
            ok("10a (I1): leva a Estoque › Encomendas com a encomenda do outro caixa à vista",
               r.esperar(lambda: r.tem("Correia dentada") and r.pilha() == inicio + 1), f)
            ok("10a (I1): com as ações dela à vista (a linha escolhida)", r.tem_igual("Cancelar encomenda"), f)
            xdo("key", "Escape")
            f = r.foto("-3")
            ok("10a (I1): cancelar (Esc) volta ao Início, sem empilhar", r.esperar(lambda: r.pilha() == inicio), f)
            ok("10a (I1): nenhuma encomenda mudou", situacao() == antes)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def desativar(pasta, nome):
    """Como a troca de dados faria ao receber de outro caixa: o usuário fica inativo (o mesmo de
    telas_estab_sessao.py)."""
    # Ao ser importado, telas_estab_sessao troca base.TELA pela tela virtual dele (:65): sem devolver a nossa, as
    # teclas e os próximos Rustinhos iriam para uma tela que não existe (foi o que derrubou o 7i e os casos seguintes).
    tela = base.TELA
    import telas_estab_sessao
    base.TELA = tela
    telas_estab_sessao.desativar(pasta, nome)


def caso7i(placar, fotos):
    """Caso 7 com interrupção: um caixa (Bia) é recusado pelo registro inconsistente, pede "Ver o registro" (a guia
    pede a senha do gerente) e, no meio, é desativado em outro caixa. Ao tentar concluir, a sessão acaba: volta à
    entrada, **nada é recebido**, nenhuma janelinha nem faixa da guia sobra por cima, e a próxima pessoa entra
    normalmente. (A sessão encerrada descarta a tela de origem por segurança, de propósito: não é perda da
    navegação; a recuperação do trabalho é da 0.9.19.)"""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-nav7i-")
    try:
        montar_loja(pasta)
        por_fora(pasta, """
            INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, aberto_por, valor_inicial)
              SELECT 'sessao-nav7i', valor, 'Caixa 1', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT id FROM usuario WHERE nome = 'Ana'), 0
                FROM config WHERE chave = 'dispositivo_id';
            INSERT INTO conta_receber (id, cliente_id, descricao, valor, vencimento, criado_em, dispositivo)
              SELECT 'cr-nav7i', id, 'Caderno', 5000, date('now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT valor FROM config WHERE chave = 'dispositivo_id')
                FROM pessoa WHERE nome = 'Zé da Oficina';
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_receber', '["cr-nav7i"]', 'pessoa', '["sumiu"]', 'inconsistente',
                      '2026-01-01T00:00:00Z', 'log');""")
        ze = ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
        ja_havia = ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0]
        r = placar.abrir(pasta, "TelaReceber", {"clienteInicial": ze}, fotos, "7i-caixa")
        try:
            r.entrar("Bia")
            r.esperar(lambda: r.tem("Receber tudo"))
            r.clicar("Receber tudo")
            r.esperar(lambda: r.tem("Receber de"))
            time.sleep(0.6)
            confirmar(r, "Receber")
            ok("7i: a recusa oferece Ver o registro ao caixa", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1"))
            r.clicar("Ver o registro")
            ok("7i: a guia pede a senha do gerente ao caixa", r.esperar(lambda: r.tem(PEDIDO) or r.tem("Liberar")), r.foto("-2"))
            desativar(pasta, "Bia")
            xdo("key", "Escape")  # desiste da senha: volta ao Receber, com a janelinha reaberta
            r.esperar(lambda: r.tem("Receber de"))
            time.sleep(0.6)
            confirmar(r, "Receber")  # tenta concluir: a sessão já acabou
            f = r.foto("-3")
            ok("7i: a sessão encerrada volta à tela de entrada", r.esperar(lambda: r.tem("Quem está usando?"), 15), f)
            ok("7i: nenhuma janelinha do recebimento nem faixa da guia sobra por cima",
               not r.tem("Receber de") and not r.tem("Registro inconsistente") and not r.tem("conclua a operação"), f)
            ok("7i: nada foi recebido", ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0] == ja_havia)
            r.entrar("Ana")
            f = r.foto("-4")
            # Depois de entrar, o programa abre de novo a tela de partida do roteiro (Vender / Receber), onde o nome da
            # pessoa não aparece: a prova é a entrada ter saído e nada da guia ter sobrado.
            ok("7i: a próxima pessoa entra normalmente, sem resto da guia",
               r.esperar(lambda: not r.tem("Quem está usando?") and r.pilha() is not None)
               and not r.tem("Registro inconsistente") and not r.tem("conclua a operação"), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


CASOS = {"8": caso8, "5": caso5, "4": caso4, "6": caso6, "7": caso7, "7i": caso7i, "9": caso9, "9b": caso9b,
         "9e": caso9e, "10a": caso10a, "10a2": caso10a_repetido, "10b": caso10b, "destaque": caso_destaque}


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    # Só telas virtuais conhecidas (0.9.18: também :91 a :93, as das frentes
    # do visual, e :60 a :74, as reservadas às frentes de trabalho); nunca a
    # área de trabalho de verdade.
    aceitas = [":88", ":89", ":91", ":92", ":93"] + [f":{n}" for n in range(60, 75)]
    if TELA not in aceitas:
        sys.exit(f"use RUSTINHO_TELA=:88, :89, :91 a :93 ou :60 a :74 (pedida: {TELA})")
    args = [a for a in sys.argv[1:]]
    escolhidos = list(CASOS)
    if "--casos" in args:
        i = args.index("--casos")
        escolhidos = args[i + 1].split(",")
        del args[i:i + 2]
    fotos = args[0] if args else tempfile.mkdtemp(prefix="rustinho-fotos-navcasos-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        for c in escolhidos:
            print(f"--- caso {c}", flush=True)
            try:
                CASOS[c](placar, fotos)
            except Exception as e:  # o caso parou no meio: conta como falha
                placar.ok(f"caso {c} chegou ao fim ({e})", False)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        placar.ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("TUDO CERTO" if placar.errados == 0 else "HOUVE FALHAS")
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
