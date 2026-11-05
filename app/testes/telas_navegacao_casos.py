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

Abre o Rustinho com o ambiente neutro de telas_d09.py (só X11, sem Wayland,
sem o tema do desktop) e confere, em /proc, que ele está na tela virtual e
sem Wayland antes de qualquer clique. Nada abre na área de trabalho.

    RUSTINHO_TELA=:88 python3 app/testes/telas_navegacao_casos.py [pasta das fotos] [--casos 8,5]
"""

import hashlib
import os
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


class Rustinho(base.Rustinho):
    def __init__(self, pasta, tela, props, fotos, nome):
        super().__init__(pasta, tela, props, fotos, nome)
        if not conferir_ambiente(self.p.pid):
            self.fechar()
            raise RuntimeError("o Rustinho do teste não está na tela virtual (ou tem Wayland): parei antes de clicar")
        self.ambiente_ok = True

    def pilha(self):
        for _, _, t in self.textos():
            if t.startswith("TESTE_PILHA:"):
                return int(t.split(":")[1])
        return None

    def tem(self, pedaco):
        return any(simples(pedaco) in simples(t) for _, _, t in self.textos())

    def tem_igual(self, texto):
        return any(simples(texto) == simples(t) for _, _, t in self.textos())

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
        for _ in range(vezes):
            pontos = self.onde(texto)
            if any(60 <= p[1] < self.altura() - 30 for p in pontos):
                return True
            xdo("mousemove", str(x), str(y))
            for _ in range(4):
                xdo("click", "5")
            time.sleep(0.4)
        return any(60 <= p[1] < self.altura() - 30 for p in self.onde(texto))

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

    def digitar(self, texto):
        xdo("type", "--delay", "70", texto)


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
            ok("8b: o \"voltar\" diz Início", r.tem_igual("Início"), f)
            ok("8b: o backup bom está na lista", r.rolar_ate("Restaurar") and len(r.onde("Restaurar")) >= 2, r.foto("-2"))
            r.clicar_ultimo("Restaurar")
            f = r.foto("-3")
            ok("8b: restaurar pede confirmação antes", r.esperar(lambda: r.tem("Restaurar o backup de")), f)
            ok("8b: ainda nada restaurado", sha(db) == antes, f)
            r.clicar("Cancelar")
            time.sleep(1)
            f = r.foto("-4")
            ok("8b: cancelar a confirmação não restaura", not r.tem("Restaurar o backup de") and sha(db) == antes, f)
            r.rolar_ate("Restaurar")
            r.clicar_ultimo("Restaurar")
            r.esperar(lambda: r.tem("Restaurar o backup de"))
            r.clicar("Continuar")
            f = r.foto("-5")
            ok("8b: depois da confirmação, pede a senha de um gerente",
               r.esperar(lambda: r.tem("Senha de um gerente (backup de antes")), f)
            ok("8b: ainda nada restaurado sem a senha", sha(db) == antes, f)
            time.sleep(0.5)
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


CASOS = {"8": caso8}


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    if TELA not in (":88", ":89"):
        sys.exit(f"use RUSTINHO_TELA=:88 ou :89 (pedida: {TELA})")
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
