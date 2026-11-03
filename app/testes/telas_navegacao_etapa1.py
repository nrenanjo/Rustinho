#!/usr/bin/env python3
"""Teste das telas da etapa 1 da navegação guiada (0.9.17).

Abre o Rustinho de verdade numa tela virtual (Xvfb), escolhe os XMLs na
janela de arquivos de verdade e confere pelos textos da tela, pelo banco e
pela cor dos pixels (o escurecimento). Usa as peças do teste do piloto
(telas_navegacao_guiada.py).

  A. Compras sem CNPJ (gerente): leva a Configurações › Loja ("voltar" diz
     Compras), orientação e escurecimento; CNPJ inválido fica e não grava;
     válido volta a Compras com o arquivo esperando, sem importar sozinho;
     "Importar agora" importa (uma compra).
  B. Compras, desistir (Esc): volta com o arquivo esperando, sem marcar o
     requisito; "Importar agora" leva de novo (uma orientação só); "Desistir"
     limpa.
  C. Compras com um CNPJ inválido gravado (banco antigo): leva do mesmo jeito.
  D. Nota de outro CNPJ: janela de conflito com os dois documentos (com o olho
     fechado, sem os números); Conferir mostra os itens; Cancelar guarda em
     "Não importada"; Ver reabre; Descartar tira. Nada gravado, o CNPJ da
     loja não muda.
  E. Compras como caixa: pede a senha do gerente antes e de novo ao salvar.
  F. Fiscal sem UF e CNPJ: "Ir para Loja" leva aos dois campos ("voltar" diz
     Fiscal); só o estado não basta (fica, com o que falta); com os dois,
     volta ao Fiscal e a faixa some.

    python3 app/testes/telas_navegacao_etapa1.py [pasta para as fotos]
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_navegacao_guiada as nav  # noqa: E402
from telas_d09 import PROGRAMA, conferir, simples  # noqa: E402

base.TELA = ":84"
nav.TELA = base.TELA
TELA = base.TELA
CNPJ_BOM = nav.CNPJ_BOM
CNPJ_RUIM = nav.CNPJ_RUIM
CNPJ_OUTRO = "11.222.333/0001-81"
banco, cnpj_gravado, pixel, brilho, casa_com_xmls = nav.banco, nav.cnpj_gravado, nav.pixel, nav.brilho, nav.casa_com_xmls


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def por_fora(pasta, sql):
    """Muda o banco por fora (como um banco antigo ou restaurado), pelo núcleo
    (os gatilhos da troca de dados precisam das funções do Rustinho)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta, RUSTINHO_SQL=sql)
    subprocess.run(["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
                    "--ignored", "--exact", "executar_por_fora"],
                   cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True)


def config_por_fora(pasta, pares):
    por_fora(pasta, "".join(f"INSERT INTO config (chave, valor) VALUES ('{k}', '{v}') "
                            "ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor;" for k, v in pares))


def uf_gravada(pasta):
    r = banco(pasta, "SELECT valor FROM config WHERE chave = 'loja_uf'")
    return r[0][0] if r else ""


def compras(pasta):
    return banco(pasta, "SELECT COUNT(*) FROM compra")[0][0]


class Rustinho(nav.Rustinho):
    def escolher_xml_de_compra(self):
        """Compras › Importar XML › o primeiro da lista › Open."""
        if not self.clicar("Importar XML"):
            raise RuntimeError(f"não achei Importar XML (veja {self.saida})")
        time.sleep(2)
        aberto = self.posicao("Open")
        if not aberto:
            raise RuntimeError(f"a janela de arquivos não abriu (veja {self.saida})")
        xdo("mousemove", str(aberto[0] - 257), str(aberto[1] - 235), "click", "1")
        time.sleep(0.5)
        xdo("mousemove", str(aberto[0]), str(aberto[1]), "click", "1")
        time.sleep(3)

    def clicar_campo(self, rotulo):
        """O campo fica logo abaixo do rótulo."""
        p = self.posicao(rotulo)
        if not p:
            raise RuntimeError(f"não achei o campo {rotulo} (veja {self.saida})")
        xdo("mousemove", str(p[0]), str(p[1] + 32), "click", "1")
        time.sleep(0.4)

    def quantas(self, texto):
        return sum(1 for _, _, x in self.textos() if simples(x) == simples(texto))


def main():
    for p in ("Xvfb", "xdotool", "import", "convert"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-etapa1-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-etapa1-")
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
        base.montar_loja(modelo)

        def loja_nova(**config):
            shutil.rmtree(pasta, ignore_errors=True)
            shutil.copytree(modelo, pasta)
            if config:
                config_por_fora(pasta, [("loja_" + k, v) for k, v in config.items()])

        loja_nova()
        ok("loja de teste sem CNPJ", cnpj_gravado(pasta) == "")
        antes = compras(pasta)

        # ---------------- A. Compras sem CNPJ, gerente ----------------
        # A loja de teste já lançou a nota de exemplo: estornada, ela pode
        # entrar de novo (para "Importar agora" mostrar a compra nova).
        por_fora(pasta, "UPDATE compra SET situacao = 'estornada';")
        casa = casa_com_xmls(raiz, ["a-nota.xml"])
        r = Rustinho(pasta, "TelaCompras", {}, fotos, "A-compras", casa)
        try:
            r.entrar("Ana")
            sem_guia = r.foto("-0-compras")
            r.escolher_xml_de_compra()
            ok("A: abriu Configurações › Loja", r.esperar("Falta o CNPJ da loja"), r.foto("-1"))
            f = r.foto("-1-guia")
            certo &= conferir("A: orientação e origem no voltar", f, r.textos(),
                              tem=["Falta o CNPJ da loja", "volta para Compras", "CNPJ da loja"],
                              igual=["Compras", "Configurações"], sem_igual=["Início"])
            ok("A: o resto da tela escurece", brilho(pixel(f, 1200, 740)) < brilho(pixel(sem_guia, 1200, 740)) - 15, f)
            ok("A: nada importado", compras(pasta) == antes)
            r.digitar_cnpj(CNPJ_RUIM)
            certo &= conferir("A: CNPJ inválido fica", r.foto("-2-invalido"), r.textos(),
                              tem=["CNPJ inválido", "Não foi salvo", CNPJ_RUIM], igual=["Configurações"])
            ok("A: inválido não gravado", cnpj_gravado(pasta) == "")
            r.digitar_cnpj(CNPJ_BOM)
            ok("A: voltou a Compras", r.esperar("Esperando para importar"), r.foto("-3"))
            time.sleep(1.5)
            certo &= conferir("A: arquivo esperando, nada importado sozinho", r.foto("-3-voltou"), r.textos(),
                              tem=["Requisito satisfeito — conclua a operação.", "a-nota.xml", "Importar agora"],
                              igual=["Compras"], nao_tem=["Falta o CNPJ da loja", "Aguarde"])
            ok("A: o CNPJ válido foi gravado", cnpj_gravado(pasta) == "11444777000161")
            ok("A: nada importado sozinho", compras(pasta) == antes)
            # "Importar agora" manda o arquivo de novo e o CNPJ já não segura:
            # o núcleo segue conferindo a nota. (O programa de verdade só
            # aceita certificado da ICP-Brasil e a nota de exemplo é assinada
            # pela raiz de teste: aqui ela para na assinatura, sem gravar.)
            r.clicar("Importar agora")
            time.sleep(4)
            certo &= conferir("A: Importar agora passa do requisito e o núcleo confere a nota", r.foto("-4"),
                              r.textos(), tem=["a-nota.xml: O certificado do emitente"],
                              nao_tem=["Falta o CNPJ da loja", "Esperando para importar"])
            ok("A: nota sem certificado válido não grava", compras(pasta) == antes)
        finally:
            r.fechar()

        # ---------------- B. Compras, desistir ----------------
        loja_nova()
        casa = casa_com_xmls(raiz, ["a-nota.xml"])
        r = Rustinho(pasta, "TelaCompras", {}, fotos, "B-desistir", casa)
        try:
            r.entrar("Ana")
            r.escolher_xml_de_compra()
            ok("B: abriu Configurações", r.esperar("Falta o CNPJ da loja"), r.foto("-1"))
            xdo("key", "Escape")
            ok("B: voltou a Compras", r.esperar("Esperando para importar"), r.foto("-2"))
            time.sleep(1)
            certo &= conferir("B: arquivo mantido, requisito não marcado", r.foto("-2-voltou"), r.textos(),
                              tem=["a-nota.xml", "Importar agora", "A operação ainda não foi concluída"],
                              nao_tem=["Requisito satisfeito"])
            ok("B: nada gravado", cnpj_gravado(pasta) == "" and compras(pasta) == antes)
            r.clicar("Importar agora")
            ok("B: tentar de novo leva outra vez", r.esperar("Falta o CNPJ da loja"), r.foto("-3"))
            ok("B: uma orientação só", r.quantas("Falta o CNPJ da loja") == 1, r.foto("-3b"))
            xdo("key", "Escape")
            ok("B: e volta de novo", r.esperar("Esperando para importar"), r.foto("-4"))
            r.clicar("Desistir")
            time.sleep(1)
            ok("B: Desistir limpa a espera", not r.tem("Esperando para importar"), r.foto("-5"))
            ok("B: nada gravado no fim", cnpj_gravado(pasta) == "" and compras(pasta) == antes)
        finally:
            r.fechar()

        # ---------------- C. CNPJ inválido gravado (banco antigo) ----------------
        loja_nova(cnpj="11444777000100")
        casa = casa_com_xmls(raiz, ["a-nota.xml"])
        r = Rustinho(pasta, "TelaCompras", {}, fotos, "C-invalido-gravado", casa)
        try:
            r.entrar("Ana")
            r.escolher_xml_de_compra()
            ok("C: CNPJ inválido gravado também leva a Configurações", r.esperar("Falta o CNPJ da loja"), r.foto("-1"))
            ok("C: nada importado", compras(pasta) == antes)
        finally:
            r.fechar()

        # ---------------- D. nota de outro CNPJ ----------------
        # O programa de verdade só aceita nota assinada com certificado da
        # ICP-Brasil, e aqui não há uma: a nota de outro CNPJ (o que o núcleo
        # devolve em "conflitos", conferido em testes_nav::nav04) entra como a
        # tela a guarda depois da importação, e o teste começa em "Ver".
        loja_nova(cnpj="11222333000181")
        conflito = {
            "arquivo": "a-nota.xml", "lojaNome": "Loja Teste Auto Peças", "lojaDocumento": "11.222.333/0001-81",
            "emitenteNome": "DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA", "emitenteDocumento": "12.345.678/0001-95",
            "destinatarioNome": "AUTO PECAS EXEMPLO", "destinatarioDocumento": "11.444.777/0001-61",
            "numero": "12345", "serie": "1", "emissao": "08/10/2026", "total": "1.245,67",
            "itens": [{"codigo": "8PK1515", "descricao": "CORREIA POLY V", "quantidade": "2", "unidade": "UN",
                       "total": "90,00"}],
        }
        r = Rustinho(pasta, "TelaCompras", {"naoImportadas": [conflito]}, fotos, "D-outro-cnpj", casa)
        try:
            r.entrar("Ana")
            time.sleep(1)
            certo &= conferir("D: a nota fica em Não importada", r.foto("-0"), r.textos(),
                              tem=["Não importada: nota de outro CNPJ", "a-nota.xml", "Ver", "Descartar"])
            r.clicar("Ver")
            ok("D: Ver abre a janela de conflito", r.esperar("CNPJ da nota é diferente"), r.foto("-1"))
            f = r.foto("-1-janela")
            certo &= conferir("D: os dois documentos, sem os números com o olho fechado", f, r.textos(),
                              tem=["CNPJ da loja", "CNPJ no XML (destinatário)", "Conferir nota", "Descartar nota",
                                   "Cancelar", "A nota não foi importada e nada foi gravado", "a-nota.xml",
                                   "DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA", "AUTO PECAS EXEMPLO"],
                              nao_tem=["11.222.333", "11.444.777", "1.245,67"])
            # O olho fica atrás da janela (modal): fecha, abre o olho, reabre.
            r.clicar("Cancelar")
            time.sleep(0.8)
            r.clicar("Mostrar valores")
            time.sleep(0.8)
            r.clicar("Ver")
            r.esperar("CNPJ da nota é diferente")
            certo &= conferir("D: com o olho aberto, os números para comparar", r.foto("-1b"), r.textos(),
                              tem=["11.222.333/0001-81", "11.444.777/0001-61", "R$ 1.245,67"])
            r.clicar("Conferir nota")
            time.sleep(1)
            certo &= conferir("D: Conferir mostra os itens da nota", r.foto("-2-conferir"), r.textos(),
                              tem=["Itens da nota (1)", "CORREIA POLY V", "Esconder os itens"])
            r.clicar("Cancelar")
            time.sleep(1)
            certo &= conferir("D: Cancelar só fecha; a nota continua em Não importada", r.foto("-3-cancelar"),
                              r.textos(), tem=["Não importada: nota de outro CNPJ"],
                              nao_tem=["CNPJ da nota é diferente"])
            r.clicar("Ver")
            ok("D: Ver reabre a janela", r.esperar("CNPJ da nota é diferente"), r.foto("-4"))
            xdo("key", "Escape")
            time.sleep(1)
            ok("D: Esc também só fecha", r.tem("Não importada") and not r.tem("CNPJ da nota é diferente"), r.foto("-4b"))
            r.clicar("Ver")
            r.esperar("CNPJ da nota é diferente")
            r.clicar("Descartar nota")
            time.sleep(1)
            ok("D: Descartar tira a nota", not r.tem("Não importada") and not r.tem("CNPJ da nota é diferente"),
               r.foto("-5"))
            ok("D: nada gravado, o CNPJ da loja não mudou",
               compras(pasta) == antes and cnpj_gravado(pasta) == "11222333000181")
        finally:
            r.fechar()

        # ---------------- E. Compras como caixa ----------------
        loja_nova()
        casa = casa_com_xmls(raiz, ["a-nota.xml"])
        r = Rustinho(pasta, "TelaCompras", {}, fotos, "E-caixa", casa)
        try:
            r.entrar("Bia")
            r.escolher_xml_de_compra()
            ok("E: pede a senha do gerente", r.esperar("Configurações da loja: só o gerente pode"), r.foto("-1"))
            xdo("type", "--delay", "80", base.SENHA["Ana"])
            xdo("key", "Return")
            time.sleep(2.5)
            certo &= conferir("E: com a senha, a orientação do CNPJ", r.foto("-2"), r.textos(),
                              tem=["Falta o CNPJ da loja", "volta para Compras"])
            r.digitar_cnpj(CNPJ_BOM)
            ok("E: salvar pede a senha de novo", r.esperar("Só o gerente pode: configurações da loja"), r.foto("-3"))
            xdo("type", "--delay", "80", base.SENHA["Ana"])
            xdo("key", "Return")
            ok("E: voltou a Compras", r.esperar("Esperando para importar"), r.foto("-4"))
            ok("E: gravado, nada importado", cnpj_gravado(pasta) == "11444777000161" and compras(pasta) == antes)
        finally:
            r.fechar()

        # ---------------- F. Fiscal sem UF e CNPJ ----------------
        loja_nova(uf="")
        ok("F: loja sem UF e sem CNPJ", uf_gravada(pasta) == "" and cnpj_gravado(pasta) == "")
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "F-fiscal", casa)
        try:
            r.entrar("Ana")
            time.sleep(1)
            # Simultaneidade pede a senha do gerente uma vez para consultar.
            if r.esperar("senha do gerente", 4) and not r.tem("Para continuar, informe a UF e o CNPJ da loja"):
                xdo("type", "--delay", "80", base.SENHA["Ana"])
                xdo("key", "Return")
            ok("F: a faixa do Fiscal pede UF e CNPJ", r.esperar("Para continuar, informe a UF e o CNPJ da loja"),
               r.foto("-1"))
            certo &= conferir("F: o que falta", r.foto("-1b"), r.textos(), tem=["Ir para Loja", "UF", "CNPJ"])
            r.clicar("Ir para Loja")
            ok("F: levou à Loja", r.esperar("volta para Fiscal"), r.foto("-2"))
            certo &= conferir("F: orientação e origem", r.foto("-2b"), r.textos(),
                              tem=["Para continuar, informe a UF e o CNPJ da loja", "Estado", "CNPJ da loja"],
                              igual=["Fiscal"])
            # O cursor está no estado (o que falta primeiro): SP.
            for _ in range(26):
                xdo("key", "Down")
            time.sleep(0.3)
            xdo("key", "F2")
            time.sleep(3)
            certo &= conferir("F: só o estado não basta (fica, diz o que falta)", r.foto("-3"), r.textos(),
                              tem=["O CNPJ da loja ainda não está salvo."], igual=["Fiscal"])
            ok("F: o estado foi gravado", uf_gravada(pasta) == "SP")
            r.clicar_campo("CNPJ da loja")
            r.digitar_cnpj(CNPJ_BOM)
            ok("F: voltou ao Fiscal", r.esperar("Requisito satisfeito"), r.foto("-4"))
            time.sleep(1.5)
            certo &= conferir("F: a faixa some", r.foto("-4b"), r.textos(),
                              tem=["CSOSN"], nao_tem=["Para continuar, informe a UF e o CNPJ da loja"])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
