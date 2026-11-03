#!/usr/bin/env python3
"""Teste automático das telas da D-09 (o caixa e os custos).

Abre o Rustinho de verdade numa tela virtual (Xvfb), entra como caixa e como
gerente e confere o que cada um vê: o Rustinho, aberto com
`--textos-da-tela`, escreve cada texto da tela e onde ele está quando recebe
Ctrl+Shift+F12. No fim, o caixa muda o preço de uma peça com custo: a tela
tem de pedir a senha do gerente e, com ela, salvar uma vez só.

Precisa de: Xvfb, xdotool, ImageMagick (import, para as fotos) e o Rustinho
compilado (cargo build -p rustinho). Usa uma loja de teste numa pasta
temporária (a mesma dos testes de estabilidade); a loja de verdade não é
tocada.

    python3 app/testes/telas_d09.py [pasta para as fotos]

Sai com 0 se tudo conferiu; senão diz o que faltou e onde está a foto.
"""

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

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PROGRAMA = os.environ.get("RUSTINHO_PROGRAMA", os.path.join(RAIZ, "target", "debug", "rustinho"))
TELA = ":79"
SENHA = {"Ana": "1234", "Bia": "1111"}


def simples(t):
    """Sem acento, minúsculo e com um espaço só."""
    t = unicodedata.normalize("NFKD", t)
    t = "".join(c for c in t if not unicodedata.combining(c))
    return re.sub(r"\s+", " ", t).strip().lower()


def montar_loja(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
         "--ignored", "--exact", "montar_loja_de_teste"],
        cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    filtro = c.execute("SELECT id FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0]
    compra = c.execute("SELECT id FROM compra ORDER BY rowid LIMIT 1").fetchone()[0]
    oleo = c.execute("SELECT id FROM produto WHERE nome = 'Óleo 5W30'").fetchone()[0]
    c.close()
    return filtro, compra, oleo


def preco_gravado(pasta, peca):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    v = c.execute("SELECT preco FROM produto_preco WHERE produto_id = ? AND tabela = 1", (peca,)).fetchone()[0]
    c.close()
    return v


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


class Rustinho:
    def __init__(self, pasta, tela, props, fotos, nome):
        self.fotos = fotos
        self.nome = nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true")
        self.log = open(self.saida, "w")
        self.p = subprocess.Popen(
            [PROGRAMA, "--textos-da-tela", "--abrir", tela, "--props", json.dumps(props)],
            env=env, stdout=self.log, stderr=self.log,
        )

    def textos(self):
        """[(x, y, texto)] do que está na tela agora."""
        with open(self.saida) as f:
            antes = f.read().count("TEXTOS-DA-TELA-FIM")
        xdo("key", "ctrl+shift+F12")
        for _ in range(50):
            time.sleep(0.2)
            with open(self.saida) as f:
                tudo = f.read()
            if tudo.count("TEXTOS-DA-TELA-FIM") > antes:
                bloco = tudo.split("TEXTOS-DA-TELA-INICIO")[-1].split("TEXTOS-DA-TELA-FIM")[0]
                lista = []
                for linha in bloco.splitlines():
                    p = linha.split("\t")
                    if len(p) == 3 and p[0].lstrip("-").isdigit() and p[1].lstrip("-").isdigit():
                        lista.append((int(p[0]), int(p[1]), p[2]))
                return lista
        raise RuntimeError(f"o Rustinho não escreveu os textos da tela (veja {self.saida})")

    def clicar(self, texto, comeca=False):
        for x, y, t in self.textos():
            if simples(t) == simples(texto) or (comeca and simples(t).startswith(simples(texto))):
                xdo("mousemove", str(x), str(y), "click", "1")
                return True
        return False

    def entrar(self, usuario):
        for _ in range(40):
            time.sleep(1)
            try:
                if self.clicar(usuario):
                    break
            except RuntimeError:
                pass
        else:
            raise RuntimeError(f"a tela de entrada não apareceu (veja {self.saida})")
        time.sleep(0.8)
        xdo("type", "--delay", "80", SENHA[usuario])
        xdo("key", "Return")
        time.sleep(4)

    def foto(self, sufixo=""):
        caminho = os.path.join(self.fotos, self.nome + sufixo + ".png")
        subprocess.run(["import", "-window", "root", caminho], env=dict(os.environ, DISPLAY=TELA), check=True)
        return caminho

    def fechar(self):
        self.p.terminate()
        try:
            self.p.wait(10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        self.log.close()


def conferir(nome, foto, textos, tem=(), nao_tem=(), igual=(), sem_igual=(), sem_percentual=False):
    """tem/nao_tem: pedaço de algum texto; igual/sem_igual: o texto inteiro;
    sem_percentual: nenhum número com % (um % de lucro ou margem)."""
    todos = [simples(t) for _, _, t in textos]
    erros = []
    if sem_percentual:
        achados = [x for x in todos if re.search(r"\d\s*%", x)]
        if achados:
            erros.append(f"apareceu um percentual em {achados[:3]}")
    for t in tem:
        if not any(simples(t) in x for x in todos):
            erros.append(f"faltou \"{t}\"")
    for t in nao_tem:
        achados = [x for x in todos if simples(t) in x]
        if achados:
            erros.append(f"apareceu \"{t}\" em {achados[:3]}")
    for t in igual:
        if simples(t) not in todos:
            erros.append(f"faltou o texto \"{t}\"")
    for t in sem_igual:
        if simples(t) in todos:
            erros.append(f"apareceu o texto \"{t}\"")
    print(("✔ " if not erros else "✘ ") + nome + ("" if not erros else ": " + "; ".join(erros) + f" (foto: {foto})"))
    return not erros


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-d09-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-d09-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        filtro, compra, oleo = montar_loja(pasta)
        casos = [
            # (nome, usuário, tela, propriedades, conferências)
            ("caixa-peca", "Bia", "TelaPeca", {"produtoId": filtro},
             dict(tem=["ficam com o gerente", "Preço de revenda", "100,00"],
                  nao_tem=["sobre o custo"], sem_igual=["Custo"])),
            ("gerente-peca", "Ana", "TelaPeca", {"produtoId": filtro},
             dict(tem=["sobre o custo", "Preço de revenda"], nao_tem=["ficam com o gerente"], igual=["Custo"])),
            ("caixa-compra", "Bia", "TelaCompra", {"compraId": compra},
             # O nome da opção "pelo % de lucro de cada peça" pode aparecer;
             # nenhum valor de % nem margem.
             dict(tem=["Custo R$"], nao_tem=["margem", "sobre o custo"], sem_percentual=True)),
            ("caixa-parados", "Bia", "TelaEstoque", {"secao": "parados"},
             dict(tem=["Dinheiro parado", "só o gerente"], nao_tem=["pelo custo"])),
            ("gerente-parados", "Ana", "TelaEstoque", {"secao": "parados"},
             dict(tem=["Dinheiro parado", "pelo custo"], nao_tem=["só o gerente"])),
        ]
        for nome, usuario, tela, props, c in casos:
            r = Rustinho(pasta, tela, props, fotos, nome)
            try:
                r.entrar(usuario)
                tudo_certo &= conferir(nome, r.foto(), r.textos(), **c)
            finally:
                r.fechar()

        # O caixa muda o preço de uma peça com custo: a tela pede a senha do
        # gerente para a regra de preço; com ela, salva (uma vez só).
        antes = preco_gravado(pasta, filtro)
        r = Rustinho(pasta, "TelaPeca", {"produtoId": filtro}, fotos, "caixa-muda-preco")
        try:
            r.entrar("Bia")
            if not r.clicar("100,00"):
                raise RuntimeError(f"não achei o preço de revenda (veja {r.saida})")
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "60", "120")
            time.sleep(0.5)
            xdo("key", "F2")
            time.sleep(2)
            tudo_certo &= conferir("caixa-muda-preco: pede o gerente", r.foto("-1"), r.textos(),
                                   tem=["Liberar", "Só o gerente pode: mudar o preço de venda", "vale só para isto, uma vez"])
            if preco_gravado(pasta, filtro) != antes:
                print("✘ caixa-muda-preco: gravou sem a senha do gerente")
                tudo_certo = False
            xdo("type", "--delay", "80", SENHA["Ana"])
            xdo("key", "Return")
            time.sleep(3)
            foto = r.foto("-2")
            ok = preco_gravado(pasta, filtro) == 12000
            print(("✔ " if ok else "✘ ") + "caixa-muda-preco: com a senha, salvou R$ 120,00"
                  + ("" if ok else f" (foto: {foto})"))
            tudo_certo &= ok
            # A senha foi usada: mudar de novo pede outra vez.
            if not r.clicar("120,00"):
                raise RuntimeError(f"não achei o preço novo (veja {r.saida})")
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "60", "130")
            time.sleep(0.5)
            xdo("key", "F2")
            time.sleep(2)
            tudo_certo &= conferir("caixa-muda-preco: a senha não ficou aberta", r.foto("-3"), r.textos(),
                                   tem=["Liberar", "mudar o preço de venda"])
            if preco_gravado(pasta, filtro) != 12000:
                print("✘ caixa-muda-preco: gravou a segunda mudança sem a senha")
                tudo_certo = False
        finally:
            r.fechar()

        # Peça sem custo: mudar o preço também pede o gerente; desistir não grava.
        antes = preco_gravado(pasta, oleo)
        r = Rustinho(pasta, "TelaPeca", {"produtoId": oleo}, fotos, "caixa-preco-sem-custo")
        try:
            r.entrar("Bia")
            if not r.clicar("30,00"):
                raise RuntimeError(f"não achei o preço do óleo (veja {r.saida})")
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "60", "33")
            time.sleep(0.5)
            xdo("key", "F2")
            time.sleep(2)
            tudo_certo &= conferir("caixa-preco-sem-custo: pede o gerente", r.foto("-1"), r.textos(),
                                   tem=["Liberar", "mudar o preço de venda"])
            r.clicar("Voltar")
            time.sleep(1.5)
            ok = preco_gravado(pasta, oleo) == antes
            print(("✔ " if ok else "✘ ") + "caixa-preco-sem-custo: desistiu, nada gravado")
            tudo_certo &= ok
        finally:
            r.fechar()

        # Configurações: a senha abre só para consultar. 0.9.15: Backup fica em
        # Simultaneidade, que pede uma senha ao entrar (vale para Duplicatas,
        # Histórico, Caixas, Backup e Fiscal); a pílula Gerente pede a dela.
        r = Rustinho(pasta, "TelaConfiguracoes", {}, fotos, "caixa-configuracoes")
        try:
            r.entrar("Bia")
            if not r.clicar("Senha do gerente"):
                raise RuntimeError(f"não achei o botão Senha do gerente (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa-configuracoes: a senha é só para consultar", r.foto("-1"), r.textos(),
                                   tem=["Configurações da loja: só o gerente pode", "só consulta esta tela"])
            xdo("type", "--delay", "80", SENHA["Ana"])
            xdo("key", "Return")
            time.sleep(2)
            if not r.clicar("Gerente"):
                raise RuntimeError(f"a pílula Gerente não apareceu (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa-configuracoes: a pílula Gerente pede a senha dela", r.foto("-3"), r.textos(),
                                   tem=["Gerente: só o gerente pode", "Liberar"])
            r.clicar("Voltar")
            time.sleep(1)
            if not r.clicar("Simultaneidade"):
                raise RuntimeError(f"a pílula Simultaneidade não apareceu (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa-configuracoes: Simultaneidade pede a senha", r.foto("-2"), r.textos(),
                                   tem=["Simultaneidade: só o gerente pode", "estas telas", "Liberar"])
            xdo("type", "--delay", "80", SENHA["Ana"])
            xdo("key", "Return")
            time.sleep(2)
            if not r.clicar("Backup"):
                raise RuntimeError(f"a subpílula Backup não apareceu (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa-configuracoes: o Backup abre sem pedir de novo", r.foto("-2b"), r.textos(),
                                   tem=["Duplicatas", "Histórico", "Caixas", "Fiscal", "Fazer backup agora"],
                                   nao_tem=["Backup: só o gerente pode"])
            # FALHA-05: com a mesma senha, Duplicatas e Histórico abrem sem
            # pedir de novo; a pílula Gerente continua pedindo a dela.
            for parte, texto in [("Duplicatas", "Diferenças entre caixas"), ("Histórico", "Histórico")]:
                if not r.clicar(parte):
                    raise RuntimeError(f"a subpílula {parte} não apareceu (veja {r.saida})")
                time.sleep(1.5)
                tudo_certo &= conferir(f"caixa-configuracoes: {parte} abre sem pedir de novo", r.foto("-2" + parte[0]), r.textos(),
                                       tem=[texto], nao_tem=["só o gerente pode", "Liberar"])
            if not r.clicar("Gerente"):
                raise RuntimeError(f"a pílula Gerente não apareceu (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa-configuracoes: depois da Simultaneidade, a pílula Gerente ainda pede a senha dela",
                                   r.foto("-4"), r.textos(), tem=["Gerente: só o gerente pode", "Liberar"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if tudo_certo else 1)


if __name__ == "__main__":
    main()
