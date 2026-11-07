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
  F. Fiscal sem UF e CNPJ: testa desistência, descarte de alterações da Loja,
     atualização do requisito após salvar só a UF e preservação de regra de ST
     pendente; com UF e CNPJ, volta ao Fiscal e a faixa some.

    python3 app/testes/telas_navegacao_etapa1.py [pasta para as fotos]
    RUSTINHO_TELA=:88 python3 app/testes/telas_navegacao_etapa1.py   (se a :84 estiver ocupada)
    RUSTINHO_CASOS=F python3 app/testes/telas_navegacao_etapa1.py    (só alguns casos)
    RUSTINHO_TEMA=roxo python3 app/testes/telas_navegacao_etapa1.py  (no Planeta Roxo; 0.9.18)
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

# A tela virtual pode ser trocada (RUSTINHO_TELA=:88) quando a :84 estiver
# ocupada por um Xvfb que ficou aberto.
base.TELA = os.environ.get("RUSTINHO_TELA", ":84")
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


def janela_de_arquivos(titulo, segundos=10):
    """(x, y, id) da janela de arquivos do Qt com esse título, ou None.

    Desde o Qt 6.8, no X11, a janela de arquivos do próprio Qt abre numa
    janela à parte (QQuickFileDialogImpl usa Popup.Window), fora da lista de
    textos da tela do Rustinho. O título é o da tela (FileDialog.title)."""
    fim = time.time() + segundos
    while True:
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", titulo],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        for janela in s.stdout.split():
            g = subprocess.run(["xdotool", "getwindowgeometry", "--shell", janela],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            d = dict(x.split("=", 1) for x in g.stdout.split() if "=" in x)
            if "X" in d and "Y" in d:
                return int(d["X"]), int(d["Y"]), janela
        if time.time() >= fim:
            return None
        time.sleep(0.5)


def primeiro_xml(casa):
    """O primeiro arquivo que a janela de arquivos lista em Downloads (pelo
    filtro de Compras: XML, ZIP ou RAR, em ordem alfabética)."""
    pasta = os.path.join(casa, "Downloads")
    nomes = sorted((n for n in os.listdir(pasta) if n.lower().endswith((".xml", ".zip", ".rar"))), key=str.lower)
    if not nomes:
        raise RuntimeError(f"nenhum XML em {pasta}")
    return os.path.join(pasta, nomes[0])


class Rustinho(nav.Rustinho):
    def __init__(self, pasta, tela, props, fotos, nome, casa):
        self.casa = casa
        super().__init__(pasta, tela, props, fotos, nome, casa)

    def escolher_xml_de_compra(self):
        """Compras › Importar XML › o primeiro da lista."""
        if not self.clicar("Importar XML"):
            raise RuntimeError(f"não achei Importar XML (veja {self.saida})")
        time.sleep(2)
        aberto = self.posicao("Open")
        if aberto:
            # Qt anterior ao 6.8: a janela de arquivos fica dentro da tela.
            xdo("mousemove", str(aberto[0] - 257), str(aberto[1] - 235), "click", "1")
            time.sleep(0.5)
            xdo("mousemove", str(aberto[0]), str(aberto[1]), "click", "1")
            time.sleep(3)
            return
        titulo = "Escolha o XML da nota de compra"
        janela = janela_de_arquivos(titulo)
        if not janela:
            raise RuntimeError(f"a janela de arquivos não abriu (veja {self.saida})")
        # Ctrl+L abre o campo do caminho; com o caminho de um arquivo, Enter
        # escolhe e fecha, como o Abrir (QQuickFolderBreadcrumbBar, igual no
        # Qt 6.8 e no 6.12; a posição dos arquivos na lista muda entre eles).
        xdo("windowfocus", "--sync", janela[2])
        time.sleep(0.5)
        xdo("key", "ctrl+l")
        time.sleep(0.5)
        xdo("type", "--delay", "20", primeiro_xml(self.casa))
        time.sleep(0.3)
        xdo("key", "Return")
        time.sleep(3)
        if janela_de_arquivos(titulo, segundos=0):
            raise RuntimeError(f"a janela de arquivos não fechou com o caminho do arquivo (veja {self.saida})")
        # Sem gerenciador de janelas (tela virtual), o teclado não volta
        # sozinho para a janela do Rustinho quando a de arquivos fecha: as
        # teclas (CNPJ, Esc, F2) se perderiam. Na área de trabalho, quem
        # devolve é o gerenciador de janelas.
        for busca in (["--class", "rustinho"], ["--name", "Rustinho"]):
            s = subprocess.run(["xdotool", "search", "--onlyvisible", *busca],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            if s.stdout.split():
                xdo("windowfocus", "--sync", s.stdout.split()[0])
                time.sleep(0.3)
                break

    def clicar_campo(self, rotulo):
        """O campo fica logo abaixo do rótulo."""
        p = self.posicao(rotulo)
        if not p:
            raise RuntimeError(f"não achei o campo {rotulo} (veja {self.saida})")
        xdo("mousemove", str(p[0]), str(p[1] + 32), "click", "1")
        time.sleep(0.4)

    def quantas(self, texto):
        return sum(1 for _, _, x in self.textos() if simples(x) == simples(texto))

    def profundidade_pilha(self):
        for _, _, texto in self.textos():
            if texto.startswith("TESTE_PILHA:"):
                return int(texto.split(":", 1)[1])
        raise RuntimeError(f"profundidade da pilha ausente (veja {self.saida})")

    def altura_da_janela(self):
        """A altura da janela do Rustinho (a lista de textos inclui o que está
        fora dela, rolado para cima ou para baixo)."""
        # Sem gerenciador de janelas a janela pode não ter título: tenta a
        # classe e o nome; senão, o tamanho padrão da janela (1280 x 760).
        for busca in (["--class", "rustinho"], ["--name", "Rustinho"]):
            s = subprocess.run(["xdotool", "search", "--onlyvisible", *busca, "getwindowgeometry", "--shell"],
                               env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            alturas = [int(x.split("=")[1]) for x in s.stdout.split() if x.startswith("HEIGHT=")]
            if alturas:
                return min(max(alturas), 1000)
        return 760

    def clicar_visivel(self, texto, topo=330, tentativas=40):
        """Rola com a roda do mouse até o texto ficar dentro da área que rola e
        clica. Na seção Fiscal, "Adicionar regra" fica bem abaixo e "Ir para
        Loja" no alto: clicar pela posição sem rolar cairia fora da janela ou
        atrás do cabeçalho fixo das Configurações (título e abas, até uns
        300 px; "topo" com folga: na 0.9.18 a linha da seta ficou 6 px mais
        alta e um botão em y = 301 ficava escondido). Só clica com a tela
        parada: um clique durante o deslize só para a rolagem."""
        altura = self.altura_da_janela()
        anterior = None
        for _ in range(tentativas):
            p = self.posicao(texto)
            if not p:
                return False
            if topo < p[1] < altura - 40:
                if p == anterior:
                    xdo("mousemove", str(p[0]), str(p[1]), "click", "1")
                    time.sleep(0.6)
                    return True
                anterior = p
                time.sleep(0.5)
                continue
            anterior = None
            # 4 = roda para cima, 5 = para baixo; o mouse fica no meio da tela.
            xdo("mousemove", "640", str(altura // 2), "click", "4" if p[1] <= topo else "5")
            time.sleep(0.3)
        return False


def main():
    for p in ("Xvfb", "xdotool", "import", "convert"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    # O Rustinho só aparece na tela virtual pelo X11. Numa sessão Wayland o Qt
    # escolheria o Wayland, a janela iria para a área de trabalho de verdade e
    # o teste ficaria esperando a tela de entrada.
    os.environ.pop("WAYLAND_DISPLAY", None)
    os.environ["QT_QPA_PLATFORM"] = "xcb"
    # O teste lê os textos da tela na saída de erro. O Qt do sistema, quando
    # tem journald, manda as mensagens para o journal se a saída não é um
    # terminal; isto as mantém na saída de erro.
    os.environ["QT_FORCE_STDERR_LOGGING"] = "1"
    # Ambiente neutro: sem o tema do desktop (KDE, GNOME...), a janela de
    # arquivos é a do próprio Qt, que o teste sabe usar.
    for v in ("XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP", "DESKTOP_SESSION", "KDE_FULL_SESSION",
              "KDE_SESSION_VERSION", "KDE_SESSION_UID", "GNOME_DESKTOP_SESSION_ID",
              "QT_QPA_PLATFORMTHEME", "QT_STYLE_OVERRIDE"):
        os.environ.pop(v, None)
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-etapa1-")
    os.makedirs(fotos, exist_ok=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    if xvfb.poll() is not None:
        sys.exit(f"a tela virtual {TELA} já está em uso (um Xvfb ficou aberto?); "
                 "escolha outra com RUSTINHO_TELA=:NN")
    raiz = tempfile.mkdtemp(prefix="rustinho-etapa1-")
    pasta = os.path.join(raiz, "loja")
    modelo = os.path.join(raiz, "loja-modelo")
    os.makedirs(modelo)
    certo = True
    # Casos a rodar: RUSTINHO_CASOS=F roda só o F.
    casos = os.environ.get("RUSTINHO_CASOS", "ABCDEF").upper()

    def quer(caso):
        return caso in casos

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"))
        certo &= bool(condicao)

    try:
        base.montar_loja(modelo)
        nav.tema_por_fora(modelo)

        def loja_nova(**config):
            shutil.rmtree(pasta, ignore_errors=True)
            shutil.copytree(modelo, pasta)
            if config:
                config_por_fora(pasta, [("loja_" + k, v) for k, v in config.items()])

        loja_nova()
        ok("loja de teste sem CNPJ", cnpj_gravado(pasta) == "")
        antes = compras(pasta)
        # Uma pasta pessoal com o XML para todos os casos (o D e o F usam a
        # última; assim cada caso pode rodar sozinho).
        casa = casa_com_xmls(raiz, ["a-nota.xml"])

        # ---------------- A. Compras sem CNPJ, gerente ----------------
        if quer("A"):
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
                passou, detalhe = nav.escurecimento(f, sem_guia)
                ok(f"A: o resto da tela escurece ({detalhe})", passou, f)
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
        if quer("B"):
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
        if quer("C"):
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
        if quer("D"):
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
                                       "A nota não foi importada e nada foi gravado", "a-nota.xml",
                                       "DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA", "AUTO PECAS EXEMPLO"],
                                  nao_tem=["11.222.333", "11.444.777", "1.245,67"])
                # 0.9.18 (V-4): a janela não tem mais "Cancelar"; a seta dela
                # (nome "Voltar") fecha, igual ao Esc.
                certo &= conferir("D: a janela tem a seta e não tem Cancelar", f, r.textos(),
                                  igual=["Voltar"], sem_igual=["Cancelar"])
                # O olho fica atrás da janela (modal): fecha, abre o olho, reabre.
                r.clicar("Voltar")
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
                r.clicar("Voltar")
                time.sleep(1)
                certo &= conferir("D: a seta só fecha; a nota continua em Não importada", r.foto("-3-cancelar"),
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
        if quer("E"):
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
        if quer("F"):
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
                ok("F: adicionar uma regra de ST pendente", r.clicar_visivel("Adicionar regra"), r.foto("-1c-regra"))
                ok("F: a regra de ST ficou pendente", r.tem("Salvar regras"), r.foto("-1d-regra-pendente"))
                profundidade = r.profundidade_pilha()
                r.clicar_visivel("Ir para Loja")
                ok("F: levou à Loja", r.esperar("volta para Fiscal"), r.foto("-2"))
                ok("F: reutilizou a Configurações atual", r.profundidade_pilha() == profundidade, r.foto("-2-pilha"))
                certo &= conferir("F: orientação e origem", r.foto("-2b"), r.textos(),
                                  tem=["Para continuar, informe a UF e o CNPJ da loja", "Estado", "CNPJ da loja"],
                                  igual=["Fiscal"])
                xdo("key", "Escape")
                ok("F: Esc desistiu e restaurou Fiscal", r.esperar("Ir para Loja"),
                   r.foto("-2c-desistiu"))
                ok("F: desistir não empilhou outra tela", r.profundidade_pilha() == profundidade, r.foto("-2c-pilha"))
                ok("F: desistir preservou a regra de ST pendente", r.tem("Salvar regras"), r.foto("-2c-regra"))
                r.clicar_visivel("Ir para Loja")
                ok("F: reabriu Loja na mesma instância", r.esperar("volta para Fiscal"), r.foto("-2d-reabriu"))
                ok("F: nova tentativa mantém a pilha", r.profundidade_pilha() == profundidade, r.foto("-2d-pilha"))
                r.clicar_campo("Nome da loja")
                xdo("key", "ctrl+a")
                xdo("type", "TEMP-NAO-SALVAR")
                ok("F: alteração local da Loja aparece", r.esperar("TEMP-NAO-SALVAR"), r.foto("-2e-alterada"))
                xdo("key", "Escape")
                ok("F: ESC pede confirmação para descartar Loja", r.esperar("Descartar alterações?"), r.foto("-2f-confirmar-descarte"))
                ok("F: confirmou o descarte", r.clicar("Descartar"), r.foto("-2g-descartar"))
                ok("F: descarte retorna ao Fiscal", r.esperar("Ir para Loja"),
                   r.foto("-2h-retorno-descarte"))
                ok("F: descarte limpa o estado Não salvo", not r.tem("Não salvo"), r.foto("-2i-nao-salvo"))
                ok("F: descarte preservou a regra de ST pendente", r.tem("Salvar regras"), r.foto("-2j-regra"))
                r.clicar_visivel("Ir para Loja")
                ok("F: voltou à Loja depois do descarte", r.esperar("volta para Fiscal"), r.foto("-2k-reabriu"))
                ok("F: descarte recarregou o nome persistido", not r.tem("TEMP-NAO-SALVAR"), r.foto("-2l-nome-recarregado"))
                # O cursor está no estado (o que falta primeiro): SP.
                for _ in range(26):
                    xdo("key", "Down")
                time.sleep(0.3)
                xdo("key", "F2")
                time.sleep(3)
                certo &= conferir("F: só o estado não basta (fica, diz o que falta)", r.foto("-3"), r.textos(),
                                  tem=["O CNPJ da loja ainda não está salvo."], igual=["Fiscal"])
                ok("F: o estado foi gravado", uf_gravada(pasta) == "SP")
                xdo("key", "Escape")
                ok("F: desistir depois de salvar só a UF retorna ao Fiscal",
                   r.esperar("Ir para Loja"), r.foto("-3b-desistiu"))
                certo &= conferir("F: desistência atualiza o requisito pelo núcleo", r.foto("-3c-requisito-atualizado"),
                                  r.textos(), tem=["O CNPJ da loja ainda não está salvo."],
                                  nao_tem=["O estado (UF) da loja ainda não está salvo."], igual=["Fiscal"])
                ok("F: regra de ST continua pendente após atualizar o Fiscal",
                   r.tem("Salvar regras"), r.foto("-3d-regra-preservada"))
                ok("F: desistência após salvar UF não empilhou tela",
                   r.profundidade_pilha() == profundidade, r.foto("-3e-pilha"))
                r.clicar_visivel("Ir para Loja")
                ok("F: voltou à Loja para preencher o CNPJ", r.esperar("volta para Fiscal"), r.foto("-3f-reabriu"))
                r.clicar_campo("CNPJ da loja")
                r.digitar_cnpj(CNPJ_BOM)
                ok("F: voltou ao Fiscal", r.esperar("Requisito satisfeito"), r.foto("-4"))
                ok("F: sucesso não deixou tela duplicada", r.profundidade_pilha() == profundidade, r.foto("-4-pilha"))
                time.sleep(1.5)
                certo &= conferir("F: a faixa some", r.foto("-4b"), r.textos(),
                                  tem=["CSOSN"], nao_tem=["Para continuar, informe a UF e o CNPJ da loja"])
                ok("F: requisito satisfeito também preserva regra ST pendente",
                   r.tem("Salvar regras"), r.foto("-4c-regra-preservada"))
            finally:
                r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
