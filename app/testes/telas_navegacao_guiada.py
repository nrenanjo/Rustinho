#!/usr/bin/env python3
"""Teste das telas da navegação guiada (piloto 0.9.16).

Peças › Importar XML sem o CNPJ da loja › Configurações › Loja › volta a Peças.
Abre o Rustinho de verdade numa tela virtual (Xvfb), escolhe os arquivos na
janela de arquivos de verdade (a pasta pessoal do teste é temporária, com os
XMLs em Downloads) e confere, pelos textos da tela, pelo banco e pela cor dos
pixels (o escurecimento):

A janela de arquivos do Qt 6.4 usada aqui (sem o seletor do sistema) marca
um arquivo por vez; o caso "um dos arquivos some e os outros ficam" é
conferido no núcleo (testes_nav::nav01_arquivos_conferir_separa_os_que_faltam).

  A. gerente: leva a Configurações › Loja ("voltar" diz Peças), orientação e
     destaque; CNPJ inválido não volta e mantém o que foi digitado; CNPJ
     válido volta a Peças, reabre a importação com os arquivos, lê de novo e
     avisa "Requisito satisfeito — conclua a operação."; nada é cadastrado
     sozinho; só uma tela de Configurações na pilha.
  B. desistir (Esc): volta a Peças com os arquivos escolhidos, sem marcar o
     requisito; "Tentar de novo" leva outra vez (uma só).
  C. a pasta escolhida some enquanto a pessoa está nas Configurações: avisa,
     sem ler nem cadastrar nada sozinho.
  D. Configurações aberta à mão: "voltar" diz Início, sem orientação nem
     escurecimento.
  E. caixa, janela pequena: pede a senha do gerente antes; com ela, o campo
     fica à vista (rolagem) e o fluxo termina igual.

    python3 app/testes/telas_navegacao_guiada.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, conferir, simples  # noqa: E402

base.TELA = ":83"
TELA = base.TELA
CNPJ_BOM = "11.444.777/0001-61"
CNPJ_RUIM = "11.444.777/0001-00"
EXEMPLO = os.path.join(RAIZ, "nucleo", "tests", "nfe_exemplo.xml")


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def banco(pasta, sql, *p):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        r = c.execute(sql, p).fetchall()
        c.commit()
        return r
    finally:
        c.close()


def cnpj_gravado(pasta):
    r = banco(pasta, "SELECT valor FROM config WHERE chave = 'loja_cnpj'")
    return r[0][0] if r else ""


def pixel(foto, x, y):
    """(r, g, b) de um ponto da foto."""
    s = subprocess.run(["convert", foto, "-crop", f"1x1+{x}+{y}", "-depth", "8", "txt:-"],
                       capture_output=True, text=True, check=True).stdout
    cor = s.splitlines()[-1].split("#")[1][:6]
    return tuple(int(cor[i:i + 2], 16) for i in (0, 2, 4))


def brilho(c):
    return sum(c) / 3


class Rustinho(base.Rustinho):
    def __init__(self, pasta, tela, props, fotos, nome, casa):
        # A pasta pessoal do teste: a janela de arquivos abre em Downloads.
        antes = os.environ.get("HOME")
        os.environ["HOME"] = casa
        try:
            super().__init__(pasta, tela, props, fotos, nome)
        finally:
            os.environ["HOME"] = antes

    def tem(self, texto):
        return any(simples(texto) in simples(t) for _, _, t in self.textos())

    def esperar(self, texto, segundos=15):
        for _ in range(int(segundos / 0.5)):
            try:
                if self.tem(texto):
                    return True
            except RuntimeError:
                pass
            time.sleep(0.5)
        return False

    def posicao(self, texto):
        for x, y, t in self.textos():
            if simples(t) == simples(texto):
                return x, y
        return None

    def abrir_janela(self, botao):
        if not self.clicar("Importar XML"):
            raise RuntimeError(f"não achei Importar XML (veja {self.saida})")
        time.sleep(1.2)
        if not self.clicar(botao):
            raise RuntimeError(f"não achei {botao} (veja {self.saida})")
        time.sleep(2)
        aberto = self.posicao("Open")
        if not aberto:
            raise RuntimeError(f"a janela de arquivos não abriu (veja {self.saida})")
        return aberto

    def escolher_arquivo(self):
        """Importar XML › Escolher arquivos › o primeiro da lista › Open."""
        aberto = self.abrir_janela("Escolher arquivos")
        # A lista começa 235 px acima do botão Open.
        xdo("mousemove", str(aberto[0] - 257), str(aberto[1] - 235), "click", "1")
        time.sleep(0.5)
        xdo("mousemove", str(aberto[0]), str(aberto[1]), "click", "1")
        time.sleep(3)

    def escolher_pasta(self):
        """Importar XML › Escolher uma pasta › sobe uma pasta › Downloads › Open."""
        aberto = self.abrir_janela("Escolher uma pasta")
        # A seta de subir fica 384 px à esquerda e 292 px acima do Open; a
        # primeira linha da lista, 265 px à esquerda e 230 px acima.
        xdo("mousemove", str(aberto[0] - 384), str(aberto[1] - 292), "click", "1")
        time.sleep(1)
        xdo("mousemove", str(aberto[0] - 265), str(aberto[1] - 230), "click", "1")
        time.sleep(0.5)
        xdo("mousemove", str(aberto[0]), str(aberto[1]), "click", "1")
        time.sleep(3)

    def encolher(self, largura, altura):
        ids = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                             env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True).stdout.split()
        for i in ids:
            xdo("windowsize", i, str(largura), str(altura))
        time.sleep(1.5)

    def digitar_cnpj(self, cnpj):
        xdo("key", "ctrl+a")
        xdo("type", "--delay", "40", cnpj)
        time.sleep(0.3)
        xdo("key", "F2")
        time.sleep(3)


def casa_com_xmls(raiz, nomes):
    casa = tempfile.mkdtemp(prefix="casa-", dir=raiz)
    os.makedirs(os.path.join(casa, "Downloads"))
    for n in nomes:
        shutil.copy(EXEMPLO, os.path.join(casa, "Downloads", n))
    return casa


def main():
    for p in ("Xvfb", "xdotool", "import", "convert"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-nav-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-nav-")
    pasta = os.path.join(raiz, "loja")
    modelo = os.path.join(raiz, "loja-modelo")
    os.makedirs(modelo)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"))
        certo &= bool(condicao)

    try:
        # A loja de teste vem sem o CNPJ. Cada cenário começa de uma cópia dela.
        base.montar_loja(modelo)

        def loja_nova():
            shutil.rmtree(pasta, ignore_errors=True)
            shutil.copytree(modelo, pasta)

        loja_nova()
        ok("loja de teste sem CNPJ", cnpj_gravado(pasta) == "")
        pessoas_antes = banco(pasta, "SELECT COUNT(*) FROM pessoa")[0][0]

        # ---------------- A. gerente: o fluxo completo ----------------
        casa = casa_com_xmls(raiz, ["a-nota.xml", "b-nota.xml"])
        r = Rustinho(pasta, "TelaPecas", {}, fotos, "A-gerente", casa)
        try:
            r.entrar("Ana")
            sem_guia = r.foto("-0-pecas")
            r.escolher_arquivo()
            ok("A: abriu Configurações › Loja", r.esperar("CNPJ da loja"), r.foto("-1"))
            f = r.foto("-1-guia")
            t = r.textos()
            certo &= conferir("A: orientação e origem no voltar", f, t,
                              tem=["Falta o CNPJ da loja", "volta para Peças"],
                              igual=["Peças", "Configurações"], sem_igual=["Início"])
            ok("A: uma orientação só", sum(1 for _, _, x in t if simples(x) == simples("Falta o CNPJ da loja")) == 1, f)
            # Escurecimento: um ponto do fundo longe do campo fica mais escuro
            # que o mesmo ponto sem guia.
            ok("A: o resto da tela escurece", brilho(pixel(f, 1200, 740)) < brilho(pixel(sem_guia, 1200, 740)) - 15, f)
            ok("A: nada gravado ainda", cnpj_gravado(pasta) == "")
            # CNPJ inválido: fica, mostra o erro, mantém o digitado.
            r.digitar_cnpj(CNPJ_RUIM)
            f = r.foto("-2-invalido")
            certo &= conferir("A: CNPJ inválido não volta", f, r.textos(),
                              tem=["CNPJ inválido", "Não foi salvo", CNPJ_RUIM], igual=["Configurações"])
            ok("A: inválido não foi gravado", cnpj_gravado(pasta) == "")
            # CNPJ válido: volta a Peças, reabre a importação, lê de novo.
            r.digitar_cnpj(CNPJ_BOM)
            ok("A: voltou a Peças", r.esperar("Trazer cadastros pelo XML"), r.foto("-3"))
            time.sleep(2)
            f = r.foto("-3-voltou")
            t = r.textos()
            certo &= conferir("A: requisito satisfeito e importação restaurada", f, t,
                              tem=["Requisito satisfeito — conclua a operação.", "Trazer cadastros pelo XML",
                                   "notas lidas"],
                              igual=["Peças"], nao_tem=["Falta o CNPJ da loja", "Cadastrados:", "Aguarde"])
            ok("A: o CNPJ válido foi gravado", cnpj_gravado(pasta) == "11444777000161")
            ok("A: nada cadastrado sozinho", banco(pasta, "SELECT COUNT(*) FROM pessoa")[0][0] == pessoas_antes)
            # Sem telas a mais na pilha: Esc fecha a importação e o próximo
            # volta ao Início (não a outra Configurações).
            xdo("key", "Escape")
            time.sleep(1)
            xdo("key", "Escape")
            time.sleep(1.5)
            ok("A: pilha sem telas repetidas (Esc vai ao Início)", r.tem("Trocar usuário") and not r.tem("CNPJ da loja"),
               r.foto("-4-inicio"))
        finally:
            r.fechar()

        # ---------------- B. desistir ----------------
        loja_nova()
        casa = casa_com_xmls(raiz, ["a-nota.xml", "b-nota.xml"])
        r = Rustinho(pasta, "TelaPecas", {}, fotos, "B-desistir", casa)
        try:
            r.entrar("Ana")
            r.escolher_arquivo()
            ok("B: abriu Configurações", r.esperar("Falta o CNPJ da loja"), r.foto("-1"))
            xdo("key", "Escape")
            ok("B: voltou a Peças", r.esperar("Trazer cadastros pelo XML"), r.foto("-2"))
            time.sleep(1)
            f = r.foto("-2-voltou")
            certo &= conferir("B: arquivos mantidos, requisito não marcado", f, r.textos(),
                              tem=["1 arquivo escolhido: a-nota.xml", "Tentar de novo",
                                   "A operação ainda não foi concluída", "CNPJ da loja"],
                              nao_tem=["Requisito satisfeito"])
            ok("B: nada gravado", cnpj_gravado(pasta) == "")
            r.clicar("Tentar de novo")
            ok("B: tentar de novo leva outra vez", r.esperar("Falta o CNPJ da loja"), r.foto("-3"))
            t = r.textos()
            ok("B: uma orientação só", sum(1 for _, _, x in t if simples(x) == simples("Falta o CNPJ da loja")) == 1,
               r.foto("-3b"))
            xdo("key", "Escape")
            ok("B: e volta de novo a Peças", r.esperar("Tentar de novo"), r.foto("-4"))
        finally:
            r.fechar()

        # ---------------- C. um arquivo some no meio ----------------
        loja_nova()
        casa = casa_com_xmls(raiz, ["a-nota.xml", "b-nota.xml"])
        r = Rustinho(pasta, "TelaPecas", {}, fotos, "C-pasta-sumiu", casa)
        try:
            r.entrar("Ana")
            r.escolher_pasta()
            ok("C: abriu Configurações", r.esperar("Falta o CNPJ da loja"), r.foto("-1"))
            os.rename(os.path.join(casa, "Downloads"), os.path.join(casa, "Downloads-movida"))
            r.digitar_cnpj(CNPJ_BOM)
            ok("C: voltou a Peças", r.esperar("Trazer cadastros pelo XML"), r.foto("-2"))
            time.sleep(2)
            certo &= conferir("C: avisa que a pasta sumiu, sem ler nada sozinho", r.foto("-2-voltou"), r.textos(),
                              tem=["não estão mais no lugar (Downloads)", "Escolha de novo", "Requisito satisfeito"],
                              nao_tem=["notas lidas", "nota lida", "Cadastrados:", "Aguarde"])
        finally:
            r.fechar()

        # ---------------- D. Configurações aberta à mão ----------------
        r = Rustinho(pasta, "TelaConfiguracoes", {}, fotos, "D-manual", casa)
        try:
            r.entrar("Ana")
            time.sleep(1)
            f = r.foto("-1")
            certo &= conferir("D: à mão, como sempre", f, r.textos(), igual=["Início", "Configurações"],
                              nao_tem=["Falta o CNPJ", "Requisito"], sem_igual=["Peças"])
        finally:
            r.fechar()

        # ---------------- E. caixa e janela pequena ----------------
        loja_nova()
        casa = casa_com_xmls(raiz, ["a-nota.xml"])
        r = Rustinho(pasta, "TelaPecas", {}, fotos, "E-caixa-pequena", casa)
        try:
            r.entrar("Bia")
            r.escolher_arquivo()
            ok("E: pede a senha do gerente", r.esperar("Configurações da loja: só o gerente pode"), r.foto("-1"))
            certo &= conferir("E: orientação da senha", r.foto("-1b"), r.textos(),
                              tem=["pede a senha do gerente"], igual=["Peças"])
            # Janela pequena antes de liberar: o campo tem de ficar à vista.
            r.encolher(760, 480)
            xdo("type", "--delay", "80", base.SENHA["Ana"])
            xdo("key", "Return")
            time.sleep(2.5)
            p = r.posicao("CNPJ da loja")
            f = r.foto("-2-pequena")
            ok("E: com a senha, o campo do CNPJ fica à vista", p is not None and 0 < p[1] < 480, f)
            certo &= conferir("E: orientação do CNPJ", f, r.textos(), tem=["Falta o CNPJ da loja"])
            r.digitar_cnpj(CNPJ_BOM)
            # D-09: cada gravação do caixa pede a senha do gerente (a guia não
            # pula isso).
            ok("E: salvar pede a senha do gerente de novo", r.esperar("Só o gerente pode: configurações da loja"),
               r.foto("-3a"))
            ok("E: sem a senha, nada gravado", cnpj_gravado(pasta) == "")
            xdo("type", "--delay", "80", base.SENHA["Ana"])
            xdo("key", "Return")
            ok("E: voltou a Peças", r.esperar("Requisito satisfeito"), r.foto("-3"))
            ok("E: CNPJ gravado", cnpj_gravado(pasta) == "11444777000161")
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
