#!/usr/bin/env python3
"""OS-2 (2ª re-revisão da C8): o relógio da guia (`esperaDaGuia`) da TelaConfiguracoes não pode tocar o destaque
depois que a tela saiu. Antes da correção, desistir da guia (ou a tela sair da pilha por outro caminho) durante a
espera deixava o relógio rodando; o toque seguinte chamava `janela.guiaDestacar` com a janela já nula:
    TelaConfiguracoes.qml:252: TypeError: Cannot call method 'guiaDestacar' of null
Na tela inteira isso é uma disputa de tempo (apareceu 1 vez em 9 rodadas); aqui o momento é provocado de propósito.

Teste de QML (qmltestrunner, sem janela). Usa o próprio código da tela: o Timer `esperaDaGuia` e as funções
`destacarCampoDaGuia` e `sair` são tirados de `app/qml/TelaConfiguracoes.qml` e de `android/rust/qml/…` (as duas
cópias), com uma janela simulada que, como a pilha de verdade, deixa `janela` nula quando a tela sai. Qualquer
TypeError/ReferenceError reprova (failOnWarning). Confere:
  1. desistir durante a espera: o relógio para e nenhum erro acontece;
  2. a tela sai da pilha por outro caminho durante a espera: nenhum erro acontece;
  3. a guia já acabou: o destaque não acontece;
  4. o caminho normal continua: o relógio espera a parte assentar e destaca o campo uma vez, com o foco nele;
  5. sair continua: com guia, conclui a guia sem marcar; sem guia, volta.

    python3 app/testes/qml_os2_relogio_da_guia.py [raiz do projeto]
"""

import hashlib
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
    name: "OS2_%COPIA%"
    width: 400
    height: 400
    Item {
        id: tela
        width: 400
        height: 400
        property var janela: null
        property var guia: null
        Flickable {
            id: rolagem
            width: 400
            height: 400
            contentHeight: 900
            Item { id: campoDaGuia; width: 100; height: 40; property int focos: 0; function focarSempre() { focos++ } }
            Item { id: linhaDocumentos; width: 100; height: 40 }
            Item { id: cnpjLoja; width: 100; height: 40; function focarSempre() {} }
        }
        QtObject { id: ufLoja; property int indice: 1 }
        function alvoDaGuia(campo) { return [campoDaGuia, "loja"] }
%BLOCOS%
    }
    QtObject {
        id: janelaFalsa
        property int destaques: 0
        property int concluidas: 0
        property int voltas: 0
        property var ultimoCampo: null
        function guiaDestacar(campo, area) { destaques++; ultimoCampo = campo }
        // Como a pilha de verdade: ao concluir a guia ou voltar, a tela sai e "janela" passa a valer null.
        function concluirGuia(ok) { concluidas++; tela.janela = null; return true }
        function voltar() { voltas++; tela.janela = null }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError/)
        esperaDaGuia.stop()
        janelaFalsa.destaques = 0
        janelaFalsa.concluidas = 0
        janelaFalsa.voltas = 0
        janelaFalsa.ultimoCampo = null
        campoDaGuia.focos = 0
        tela.janela = janelaFalsa
        tela.guia = { requisito: { campos: ["loja.uf_cnpj"] }, alvo: null }
    }
    function cleanup() { esperaDaGuia.stop() }
    // O que comecarGuia() faz no fim: zera a espera e liga o relógio.
    function comecarEspera() {
        esperaDaGuia.tentativas = 0
        esperaDaGuia.alturaAntes = -1
        esperaDaGuia.restart()
        verify(esperaDaGuia.running, "o relógio da guia não começou")
    }
    // Espera o relógio terminar sozinho (no máximo 25 toques de 120 ms) e mais um pouco.
    function esperarORelogio() {
        tryVerify(function() { return !esperaDaGuia.running }, 6000, "o relógio da guia não parou")
        wait(300)
    }

    function test_1_desistir_durante_a_espera() {
        comecarEspera()
        tela.sair()
        verify(!esperaDaGuia.running, "desistir não parou o relógio da guia")
        compare(janelaFalsa.concluidas, 1, "desistir não concluiu a guia")
        esperarORelogio()
        compare(janelaFalsa.destaques, 0, "destacou depois de a tela sair")
    }
    function test_2_tela_sai_por_outro_caminho_durante_a_espera() {
        comecarEspera()
        tela.janela = null   // a pilha tirou a tela (sessão encerrada, backup restaurado), sem passar por sair()
        esperarORelogio()
        compare(janelaFalsa.destaques, 0)
    }
    function test_3_guia_ja_acabou() {
        tela.guia = null
        tela.destacarCampoDaGuia(campoDaGuia)
        compare(janelaFalsa.destaques, 0, "destacou sem guia")
    }
    function test_4_caminho_normal_continua() {
        comecarEspera()
        esperarORelogio()
        compare(janelaFalsa.destaques, 1, "o destaque não aconteceu uma vez")
        verify(janelaFalsa.ultimoCampo === campoDaGuia, "destacou outro campo")
        compare(campoDaGuia.focos, 1, "o foco não foi para o campo")
    }
    function test_5_sair_continua() {
        tela.sair()
        compare(janelaFalsa.concluidas, 1)
        compare(janelaFalsa.voltas, 0)
        tela.janela = janelaFalsa
        tela.guia = null
        tela.sair()
        compare(janelaFalsa.voltas, 1)
        compare(janelaFalsa.concluidas, 1)
    }
}
"""

PADROES = [
    ("o relógio esperaDaGuia", r"^    Timer \{\n        id: esperaDaGuia\n.*?^    \}\n"),
    ("destacarCampoDaGuia", r"^    function destacarCampoDaGuia\(campo\) \{\n.*?^    \}\n"),
    ("sair", r"^    function sair\(\) \{\n.*?^    \}\n"),
]


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


def blocos(caminho):
    texto = open(caminho, encoding="utf-8").read()
    partes = []
    for nome, padrao in PADROES:
        m = re.search(padrao, texto, re.S | re.M)
        if not m:
            sys.exit(f"não achei {nome} em {caminho}")
        partes.append("\n".join(("    " + ln) if ln.strip() else ln for ln in m.group(0).splitlines()))
    print(f"{os.path.relpath(caminho, RAIZ)}: sha256 {hashlib.sha256(texto.encode()).hexdigest()[:16]}")
    return "\n".join(partes)


def main():
    qmltestrunner = runner()
    falhou = []
    for copia, rel in (("app", "app/qml/TelaConfiguracoes.qml"), ("android", "android/rust/qml/TelaConfiguracoes.qml")):
        pasta = tempfile.mkdtemp(prefix="rustinho-qml-os2-")
        try:
            arquivo = os.path.join(pasta, f"tst_os2_{copia}.qml")
            with open(arquivo, "w", encoding="utf-8") as f:
                f.write(TESTE.replace("%COPIA%", copia).replace("%BLOCOS%", blocos(os.path.join(RAIZ, rel))))
            r = subprocess.run([qmltestrunner, "-input", arquivo], env=dict(os.environ, QT_QPA_PLATFORM="offscreen"),
                               capture_output=True, text=True, errors="replace")
            print(r.stdout + r.stderr)
            if r.returncode != 0:
                falhou.append(copia)
        finally:
            shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "reprovou em " + ", ".join(falhou) if falhou else "as duas cópias passaram")
    sys.exit(1 if falhou else 0)


if __name__ == "__main__":
    main()
