#!/usr/bin/env python3
"""Roteiro da T107 (texto Q do proprietário, 21/11/2026): "Escolher uma pasta" em Compras › Importar XML, no mesmo
padrão de Peças › Trazer cadastros pelo XML: uma pasta inteira, com as subpastas, vai à importação das notas de
compra. A escolha de arquivos (o "Importar XML", F2) continua como era.

  ESTÁTICA  no código: o botão "Escolher uma pasta" abre um FolderDialog que manda a pasta à mesma importação
            (`tela.importar([selectedFolder])`); o "Importar XML" continua com o F2 e a janela de arquivos.
  TELA      Compras, com a nota guardada numa subpasta de Downloads (notas/2026/): o botão está ao lado do
            "Importar XML"; ele abre a janela de pasta; escolhida a Downloads, a importação chega à nota da subpasta
            (o aviso cita o arquivo dela); o F2 continua abrindo a janela de arquivos.

O programa dos roteiros confere a assinatura com as autoridades de verdade: a nota de exemplo (assinada pela PKI de
teste) é recusada com o nome do arquivo no aviso. É a prova de que a pasta e a subpasta foram lidas.

    python3 app/testes/telas_compras_pasta_t107.py [pasta para as fotos]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_importar_cadastros as imp  # noqa: E402
import telas_navegacao_guiada as nav  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":96")
nav.TELA = base.TELA
imp.TELA = base.TELA
TELA = base.TELA
QML = os.path.join(RAIZ, "app", "qml", "TelaCompras.qml")
TITULO_PASTA = "Escolha a pasta com os XMLs das notas de compra"
TITULO_ARQUIVOS = "Escolha o XML da nota de compra"
NOTA = "nota-da-subpasta.xml"


def estatica(caminho=QML):
    """[(o que falta)] no código."""
    t = open(caminho, encoding="utf-8").read()
    faltas = []
    if '"Escolher uma pasta"' not in t or "seletorPasta.open()" not in t:
        faltas.append("não há o botão Escolher uma pasta")
    if not re.search(r"FolderDialog\s*\{[^}]*onAccepted:\s*tela\.importar\(\[selectedFolder\]\)", t, re.S):
        faltas.append("a pasta não vai à mesma importação")
    if not re.search(r'text: "Importar XML"\s*\n\s*icone: "mais"\s*\n\s*atalho: "F2"\s*\n\s*onClicked: seletor\.open\(\)', t):
        faltas.append("o Importar XML (F2, janela de arquivos) mudou")
    if "fileMode: FileDialog.OpenFiles" not in t:
        faltas.append("a janela de arquivos não escolhe mais vários arquivos")
    return faltas


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def botao(tx, nome):
    return any(x.startswith("TOQUE:") and x.split("|")[3] == nome and x.split("|")[-1] == "1" for x in tx)


def main():
    certo = True

    def ok(nome, condicao, detalhe=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" ({detalhe})"))
        certo &= bool(condicao)

    faltas = estatica()
    ok("ESTÁTICA: o Escolher uma pasta manda a pasta à mesma importação; o Importar XML (F2) como era", not faltas,
       str(faltas))

    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-t107-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-t107-")
    pasta = os.path.join(raiz, "loja")
    casa = tempfile.mkdtemp(prefix="casa-", dir=raiz)
    sub = os.path.join(casa, "Downloads", "notas", "2026")
    os.makedirs(sub)
    shutil.copy(nav.EXEMPLO, os.path.join(sub, NOTA))
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    antes = os.environ.get("RUSTINHO_ARGS_TESTE")
    os.environ["RUSTINHO_ARGS_TESTE"] = ((antes or "") + " --medir-toque").strip()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)
        imp.por_fora(pasta, "UPDATE config SET valor = '11444777000161' WHERE chave = 'loja_cnpj';")
        r = nav.Rustinho(pasta, "TelaCompras", {}, fotos, "compras", casa)
        try:
            r.entrar("Ana")
            time.sleep(1)
            tx = [t for _, _, t in r.textos()]
            if not botao(tx, "Importar XML"):
                raise RuntimeError(f"INSTRUMENTAÇÃO: Compras não abriu (foto {r.foto('-instr')})")
            f = r.foto("-1-tela")
            ok("TELA: o Escolher uma pasta está ao lado do Importar XML", botao(tx, "Escolher uma pasta"), f"foto {f}")
            if botao(tx, "Escolher uma pasta"):
                r.clicar("Escolher uma pasta")
                time.sleep(2)
                janela = nav.janela_de_arquivos(TITULO_PASTA)
                ok("TELA: abre a janela de pasta", janela is not None, f"foto {r.foto('-2-janela')}")
                if janela:
                    # Na janela à parte (Qt 6.8 e 6.12), subir marca a pasta de onde se veio (Downloads); Abrir.
                    x, y, largura, altura, _ = janela
                    xdo("mousemove", str(x + 41), str(y + 41), "click", "1")
                    time.sleep(1.5)
                    xdo("mousemove", str(x + largura - 180), str(y + altura - 30), "click", "1")
                    time.sleep(4)
                    nav.devolver_teclado()
                    fim = time.time() + 20
                    chegou = False
                    while time.time() < fim and not chegou:
                        tx = [t for _, _, t in r.textos()]
                        chegou = any(NOTA in t for t in tx) or any("DISTRIBUIDORA DE AUTOPECAS EXEMPLO" in t for t in tx)
                        if not chegou:
                            time.sleep(1)
                    ok("TELA: escolhida a Downloads, a importação chega à nota da subpasta (notas/2026)", chegou,
                       f"foto {r.foto('-3-depois')}")
            # O Importar XML (F2) continua abrindo a janela de arquivos. (O Esc só se a importação abriu outra tela:
            # na tela de Compras, ele voltaria ao Início.)
            nav.devolver_teclado()
            if not botao([t for _, _, t in r.textos()], "Importar XML"):
                xdo("key", "Escape")
                time.sleep(1.5)
            r.clicar("Importar XML")
            time.sleep(2)
            arquivos = nav.janela_de_arquivos(TITULO_ARQUIVOS)
            ok("TELA: o Importar XML continua abrindo a janela de arquivos", arquivos is not None,
               f"foto {r.foto('-4-arquivos')}")
            if arquivos:
                xdo("windowfocus", "--sync", arquivos[4])
                xdo("key", "Escape")
                time.sleep(1)
            with open(r.saida, encoding="utf-8", errors="replace") as reg:
                erros = [linha.strip() for linha in reg if "TypeError" in linha or "ReferenceError" in linha]
            ok("nenhum TypeError/ReferenceError no registro", not erros, str(erros[:3]))
        finally:
            r.fechar()
    finally:
        if antes is None:
            os.environ.pop("RUSTINHO_ARGS_TESTE", None)
        else:
            os.environ["RUSTINHO_ARGS_TESTE"] = antes
        shutil.rmtree(raiz, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()
    print(f"\nfotos em {fotos}")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
