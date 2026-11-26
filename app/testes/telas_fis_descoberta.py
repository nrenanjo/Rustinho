#!/usr/bin/env python3
"""T010b e R1 (C-M4, C-M8) da 0.9.19, etapa 3: a descoberta dos dados fiscais nas notas que a loja já emitiu.

Em Documentos/notas da pasta pessoal do teste ficam duas NFC-e autorizadas da loja (série 1, números 41 e 42,
produção, Simples Nacional, QR Code v2 com o identificador do CSC 1) e duas notas de compra de um fornecedor. A loja
de teste não tem CNPJ informado. O roteiro confere, pela janela de pasta do sistema (o seletor nativo):
- com dois emitentes, o Rustinho não escolhe sozinho: pergunta "Qual destes CNPJ é o da loja?" e lista os dois;
- escolhido o da loja, aparecem o regime, o identificador do CSC (e o aviso de que o CSC não está nas notas) e a
  série 1 com o último número 42 e a frase de que a próxima nota do Rustinho sai depois da 42;
- "Usar esta série" grava a série da NFC-e e o piso da numeração (a 42) neste computador, com o registro.

    python3 app/testes/telas_fis_descoberta.py [pasta para as fotos]
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
import telas_importar_cadastros as imp  # noqa: E402
import telas_navegacao_guiada as nav  # noqa: E402
from telas_d09 import PROGRAMA, conferir  # noqa: E402
from telas_fis_p5 import clicar_rolando  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", base.TELA)
nav.TELA = base.TELA
imp.TELA = base.TELA
TELA = base.TELA
TITULO = "Escolha a pasta com os XML das notas que a loja já emitiu"
LOJA = "11222333000181"
FORNECEDOR = "99888777000166"


def nfce(cnpj, numero, crt):
    chave = "35261" + "1" + cnpj + "65001" + "%09d" % numero + "1" + "00000019"
    return (
        '<nfeProc xmlns="http://www.portalfiscal.inf.br/nfe" versao="4.00"><NFe>'
        f'<infNFe Id="NFe{chave}" versao="4.00">'
        f"<ide><cUF>35</cUF><mod>65</mod><serie>1</serie><nNF>{numero}</nNF>"
        f"<dhEmi>2026-11-2{numero % 9}T10:00:00-03:00</dhEmi><tpAmb>1</tpAmb></ide>"
        f"<emit><CNPJ>{cnpj}</CNPJ><enderEmit><UF>SP</UF></enderEmit><IE>123456789110</IE><CRT>{crt}</CRT></emit>"
        "</infNFe><infNFeSupl><qrCode>https://www.nfce.fazenda.sp.gov.br/qrcode?"
        f"p={chave}|2|1|000001|ABCDEF</qrCode></infNFeSupl></NFe>"
        '<protNFe versao="4.00"><infProt><cStat>100</cStat></infProt></protNFe></nfeProc>'
    )


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def clicar_na_linha(r, texto_da_linha, botao):
    """Clica o botão que fica na mesma linha (mesma altura) do texto."""
    alvos = r.textos_para_clicar()
    linha = [y for _, y, t in alvos if base.simples(texto_da_linha) in base.simples(t)]
    if not linha:
        return False
    candidatos = [(x, y) for x, y, t in alvos if t == botao and abs(y - linha[0]) < 30]
    if not candidatos:
        return False
    xdo("mousemove", str(candidatos[0][0]), str(candidatos[0][1]), "click", "1")
    return True


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-descoberta-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-descoberta-")
    pasta = os.path.join(raiz, "loja")
    casa = tempfile.mkdtemp(prefix="casa-", dir=raiz)
    notas = os.path.join(casa, "Documents", "notas")
    os.makedirs(notas)
    for nome, conteudo in [
        ("loja-41.xml", nfce(LOJA, 41, 1)),
        ("loja-42.xml", nfce(LOJA, 42, 1)),
        ("compra-7.xml", nfce(FORNECEDOR, 7, 3)),
        ("compra-8.xml", nfce(FORNECEDOR, 8, 3)),
    ]:
        with open(os.path.join(notas, nome), "w", encoding="utf-8") as f:
            f.write(conteudo)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)
        imp.por_fora(pasta, "DELETE FROM config WHERE chave = 'loja_cnpj';")
        r = nav.Rustinho(pasta, "TelaConfiguracoes", {"secao": "fiscal"}, fotos, "fis-descoberta", casa)
        try:
            r.entrar("Ana")
            time.sleep(3)
            if not clicar_rolando(r, "Trazer os dados das notas que a loja já emitiu"):
                raise RuntimeError(f"não achei o botão da descoberta (veja {r.saida})")
            time.sleep(2)
            janela = nav.janela_de_arquivos(TITULO)
            if janela is None:
                raise RuntimeError(f"a janela de pasta não abriu (foto {r.foto('-instr')})")
            # A janela abre em Documentos; subir marca a pasta de onde se veio (Documents); Abrir lê ela e as subpastas.
            x, y, largura, altura, _ = janela
            xdo("mousemove", str(x + 41), str(y + 41), "click", "1")
            time.sleep(1.5)
            xdo("mousemove", str(x + largura - 180), str(y + altura - 30), "click", "1")
            time.sleep(3)
            nav.devolver_teclado()
            if not r.esperar("Qual destes CNPJ é o da loja?", 20):
                raise RuntimeError(f"a descoberta não perguntou qual é a loja (foto {r.foto('-instr2')})")
            tudo_certo &= conferir("C-M8: com dois emitentes, pergunta qual é a loja", r.foto("-1"), r.textos(),
                                   tem=["Qual destes CNPJ é o da loja?", f"{LOJA} · 2 notas", f"{FORNECEDOR} · 2 notas",
                                        "4 notas autorizadas lidas, de 2 emitentes"],
                                   nao_tem=["Regime (CRT)", "Séries usadas", "0 notas autorizadas lidas"])
            if not clicar_na_linha(r, f"{LOJA} · 2 notas", "É o da loja"):
                raise RuntimeError(f"não achei o É o da loja do CNPJ da loja (foto {r.foto('-instr3')})")
            if not r.esperar("Séries usadas", 15):
                raise RuntimeError(f"a descoberta não leu de novo com a loja escolhida (foto {r.foto('-instr4')})")
            tudo_certo &= conferir("T010b: os dados da loja, com a origem", r.foto("-2"), r.textos(),
                                   tem=["Regime (CRT): ", "Identificador do CSC: 1", "o CSC (o código secreto) não aparece",
                                        "NFC-e série 1 (produção): último número 42 · 2 notas",
                                        "a próxima nota do Rustinho sai depois da 42"],
                                   nao_tem=["Qual destes CNPJ é o da loja?", FORNECEDOR])
            if not clicar_rolando(r, "Usar esta série"):
                raise RuntimeError(f"não achei o Usar esta série (foto {r.foto('-instr5')})")
            if not r.esperar("Gravado: Série da NFC-e.", 15):
                tudo_certo = False
                print(f"✘ C-M4: o Usar esta série não confirmou (foto {r.foto('-3')})")
        finally:
            r.fechar()
        c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
        valor = dict(c.execute("SELECT chave, valor FROM config WHERE chave IN "
                               "('fiscal_serie_nfce', 'fiscal_ultimo_de_antes:1:65:1')").fetchall())
        registros = c.execute("SELECT COUNT(*) FROM auditoria WHERE acao = 'fiscal.descoberta'").fetchone()[0]
        c.close()
        certo = valor == {"fiscal_serie_nfce": "1", "fiscal_ultimo_de_antes:1:65:1": "42"} and registros == 1
        print(("✔" if certo else "✘") + f" C-M4: a série e o piso gravados neste computador, com o registro: {valor}, {registros}")
        tudo_certo &= certo
    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
