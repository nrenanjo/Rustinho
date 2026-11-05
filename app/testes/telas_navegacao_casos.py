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
            ok("4: o \"voltar\" diz Etiquetas", r.tem_igual("Etiquetas"), f)
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
            ok("4: o \"voltar\" da peça diz Etiquetas", r.tem_igual("Etiquetas"), f)
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


CASOS = {"8": caso8, "5": caso5, "4": caso4}


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
