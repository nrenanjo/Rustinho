#!/usr/bin/env python3
"""T002 e T003 da 009 (R3 M1): as duas Notas de Configurações › Simultaneidade que mostram o que a rede deste caixa diz:
`vpnsDesteComputador` (as VPNs ligadas) e `nomesResolvidos` (o IP que cada nome digitado deu).

Teste de QML (qmltestrunner, sem janela): tira do próprio `app/qml/TelaConfiguracoes.qml` as expressões `visible` e
`text` das duas Notas e as roda com um `tela.sync` igual ao que o `sync_ler` devolve. Confere:
  1. sem dados (ou com as listas vazias), as duas ficam escondidas;
  2. com uma VPN, a Nota diz "VPN neste computador: wg0 (10.8.0.2). …";
  3. com dois nomes, a Nota diz "caixa2: 10.0.0.2 · loja2: não respondeu a tempo".
Assim, um nome de chave errado no QML (`nomesResolvido`, `vpn`, `resultados`) reprova.

    python3 app/testes/qml_t002_notas_da_rede.py [raiz do projeto]
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", ".."))

TESTE = """import QtQuick
import QtTest

TestCase {
    name: "NotasDaRede"
    property var tela: ({ sync: null })

    // QtObject, e não Text: dentro do TestCase um Text herdaria o "invisível" do pai.
    QtObject { id: vpns; property bool visible: %VPNS_VISIVEL%; property string text: %VPNS_TEXTO% }
    QtObject { id: nomes; property bool visible: %NOMES_VISIVEL%; property string text: %NOMES_TEXTO% }

    function test_1_sem_dados_escondidas() {
        tela = { sync: null }
        verify(!vpns.visible && !nomes.visible)
        tela = { sync: { vpns: [], nomesResolvidos: [] } }
        verify(!vpns.visible && !nomes.visible)
    }
    function test_2_a_vpn() {
        tela = { sync: { vpns: [{ nome: "wg0", ip: "10.8.0.2" }], nomesResolvidos: [] } }
        verify(vpns.visible)
        verify(!nomes.visible)
        verify(vpns.text.indexOf("VPN neste computador: wg0 (10.8.0.2)") === 0, vpns.text)
    }
    function test_3_os_nomes() {
        tela = { sync: { vpns: [], nomesResolvidos: [
            { nome: "caixa2", resultado: "10.0.0.2" },
            { nome: "loja2", resultado: "não respondeu a tempo" }
        ] } }
        verify(nomes.visible)
        compare(nomes.text, "caixa2: 10.0.0.2 · loja2: não respondeu a tempo")
    }
}
"""


def runner():
    """O qmltestrunner do Qt 6 (o /usr/bin/qmltestrunner pode ser o do Qt 5)."""
    candidatos = [os.environ.get("RUSTINHO_QMLTESTRUNNER", "")]
    if os.environ.get("QT_ROOT_DIR"):
        candidatos.append(os.path.join(os.environ["QT_ROOT_DIR"], "bin", "qmltestrunner"))
    candidatos += ["/usr/lib/qt6/bin/qmltestrunner", shutil.which("qmltestrunner6") or "", shutil.which("qmltestrunner") or ""]
    for c in candidatos:
        if c and os.access(c, os.X_OK):
            return c
    sys.exit("falta o qmltestrunner do Qt 6")


def expressoes(qml, objeto):
    """As expressões `visible` e `text` da Nota com esse objectName (cada uma pode ocupar várias linhas)."""
    m = re.search(r'objectName: "' + re.escape(objeto) + r'"\n(.*?)\n\s*textFormat:', qml, re.S)
    if not m:
        sys.exit(f"não achei a Nota {objeto} no TelaConfiguracoes.qml")
    bloco = m.group(1)
    vis = re.search(r"\bvisible: (.*?)\n\s*text:", bloco, re.S)
    txt = re.search(r"\btext: (.*)", bloco, re.S)
    if not vis or not txt:
        sys.exit(f"a Nota {objeto} mudou de forma: {bloco}")
    return " ".join(vis.group(1).split()), " ".join(txt.group(1).split())


def main():
    qmltestrunner = runner()
    qml = open(os.path.join(RAIZ, "app", "qml", "TelaConfiguracoes.qml"), encoding="utf-8").read()
    vv, vt = expressoes(qml, "vpnsDesteComputador")
    nv, nt = expressoes(qml, "nomesResolvidos")
    teste = (TESTE.replace("%VPNS_VISIVEL%", vv).replace("%VPNS_TEXTO%", vt)
             .replace("%NOMES_VISIVEL%", nv).replace("%NOMES_TEXTO%", nt))
    pasta = tempfile.mkdtemp(prefix="rustinho-qml-t002-")
    try:
        with open(os.path.join(pasta, "tst_t002.qml"), "w", encoding="utf-8") as f:
            f.write(teste)
        r = subprocess.run([qmltestrunner, "-input", os.path.join(pasta, "tst_t002.qml")],
                           env=dict(os.environ, QT_QPA_PLATFORM="offscreen"), capture_output=True, text=True,
                           errors="replace")
        print(r.stdout + r.stderr)
        sys.exit(r.returncode)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


if __name__ == "__main__":
    main()
