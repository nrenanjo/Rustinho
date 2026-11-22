#!/usr/bin/env python3
"""T108 (texto R do proprietário, 21/11/2026): a lógica das telas da importação compartilhada, sem o núcleo.

Teste de QML (qmltestrunner, sem janela). As funções e as expressões saem do próprio QML:
- de `app/qml/TelaCompras.qml`, o `importar`;
- de `app/qml/ImportarCadastros.qml`, o `lerArquivos`, o `cat` e o `texto`, o modelo da prévia, a visibilidade da
  linha, o texto do "já" e o `onToggled` do Marcar.
O núcleo (`Api`), a janela e o tema são simulados. O programa dos roteiros de tela não chega a uma prévia com nota de
verdade (PEND-09: ele só confia nas autoridades de verdade); por isso esta prova lê o QML.

Confere:
  N1  (2ª conferência): UMA mensagem só no fim da importação de Compras; com erro, ela é de erro e leva também as
      notas importadas e as que já estavam em Compras.
  A3  (FI-1 da opção nova): "Notas de compra em Compras" prende "Fornecedores", na 1ª leitura e ao marcar, com o porquê.
  A8  a linha das notas de compra aparece também só com as que não servem para Compras, e diz quantas.
  N5  o singular: "1 já estava em Compras".

    python3 app/testes/qml_t108_importacao_compartilhada.py [raiz do projeto]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", ".."))

API = """pragma Singleton
import QtQuick
QtObject {
    property var respostas: []
    property bool repetidaAgora: false
    function caminho(a) { return a }
    function novaChave() { return "chave" }
    function gravarDemorado(comando, dados, texto, depois, chave) { depois(respostas.shift()) }
    function chamarDemorado(comando, dados, texto, depois) { depois(respostas.shift()) }
}
"""

TEMA = """pragma Singleton
import QtQuick
QtObject {
    property string verde: "verde"
    property string azul: "azul"
    property string papel: "papel"
}
"""

TESTE = """import QtQuick
import QtTest
import falsos

TestCase {
    name: "T108"

    QtObject {
        id: janelaFalsa
        property var avisos: []
        property var abertas: []
        function avisar(texto, tipo) { avisos.push({ texto: texto, tipo: tipo }) }
        function abrir(nome, propriedades) { abertas.push(nome) }
        function guiar(campo, erro, depois) { return false }
    }

    QtObject {
        id: tela
        property var janela: janelaFalsa
        property var pendentes: []
        property string avisoPendentes: ""
        function buscar() {}
        function guardarNaoImportadas(conflitos) {}
        function nomeArquivo(a) { return a }
%IMPORTAR%
    }

    QtObject {
        id: painel
        property var janela: janelaFalsa
        property var arquivos: ["pasta"]
        property string chaveTela: ""
        property var previa: null
        property string erro: ""
        property bool trabalhando: false
        property var escolhas: null
        property bool qClientes: false
        property bool qFornecedores: false
        property bool qPecasLoja: false
        property bool qPecasFornecedor: false
        property bool qCompras: false
        property int refeitas: 0
        function refazerPrevia(x) { refeitas += 1 }
        function dadosDaPrevia(comOpcoes) { return {} }
%CAT%
%TEXTO%
%LER%
        function linhas() { return %MODELO% }
        function alternar(linha, checked) {
%ALTERNAR%
        }
    }

    function visivel(modelData) { return %VISIVEL% }
    function textoJa(linha) { return %TEXTOJA% }

    function previa(campos) {
        const p = { clientesNovos: 0, clientesJa: 0, fornecedoresNovos: 0, fornecedoresJa: 0, pecasLojaNovas: 0,
                    pecasFornecedorNovas: 0, comprasNovas: 0, comprasJa: 0, comprasNaoServe: 0, categorias: [],
                    exemplosClientes: [], exemplosFornecedores: [] }
        for (const k in campos)
            p[k] = campos[k]
        return p
    }
    function linha(chave) { return painel.linhas().find(l => l.chave === chave) }

    function init() {
        janelaFalsa.avisos = []
        janelaFalsa.abertas = []
        painel.escolhas = null
        painel.qClientes = false
        painel.qFornecedores = false
        painel.qPecasLoja = false
        painel.qPecasFornecedor = false
        painel.qCompras = false
    }

    function test_n1_com_erro_uma_mensagem_so_e_de_erro() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }, { id: "b" }], erros: ["x.xml: XML alterado"],
                                              conflitos: [], jaEmCompras: 1 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        const a = janelaFalsa.avisos[0]
        compare(a.tipo, "erro")
        verify(a.texto.indexOf("x.xml: XML alterado") >= 0, a.texto)
        verify(a.texto.indexOf("2 notas importadas") >= 0, a.texto)
        verify(a.texto.indexOf("1 nota já estava em Compras") >= 0, a.texto)
    }

    function test_n1_uma_nota_e_as_que_ja_estavam() {
        Api.respostas = [{ ok: true, dados: { compras: [{ id: "a" }], erros: [], conflitos: [], jaEmCompras: 2 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.abertas, ["TelaCompra"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        compare(janelaFalsa.avisos[0].tipo, "ok")
        verify(janelaFalsa.avisos[0].texto.indexOf("2 notas já estavam em Compras") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_n1_so_as_que_ja_estavam() {
        Api.respostas = [{ ok: true, dados: { compras: [], erros: [], conflitos: [], jaEmCompras: 3 } }]
        tela.importar(["pasta"])
        compare(janelaFalsa.avisos.length, 1, JSON.stringify(janelaFalsa.avisos))
        verify(janelaFalsa.avisos[0].texto.indexOf("3 notas já estavam em Compras") >= 0, janelaFalsa.avisos[0].texto)
    }

    function test_a3_primeira_leitura_prende_fornecedores() {
        Api.respostas = [{ ok: true, dados: previa({ comprasNovas: 2 }) }]
        painel.lerArquivos()
        verify(painel.qCompras, "as notas de compra marcadas")
        verify(painel.qFornecedores, "Fornecedores marcada junto (FI-1)")
        const f = linha("qFornecedores")
        verify(f.presa, "Fornecedores travada")
        verify(f.porque.indexOf("Notas de compra em Compras") >= 0, f.porque)
    }

    function test_a3_marcar_as_notas_marca_fornecedores() {
        painel.previa = previa({ comprasNovas: 1 })
        painel.alternar({ modelData: { chave: "qCompras" } }, true)
        verify(painel.qCompras)
        verify(painel.qFornecedores, "Fornecedores marcada junto (FI-1)")
        verify(painel.escolhas.qCompras, "a escolha guardada")
    }

    function test_a8_linha_so_com_as_que_nao_servem() {
        painel.previa = previa({ comprasNaoServe: 2 })
        const l = linha("qCompras")
        verify(visivel(l), "a linha aparece")
        verify(l.extra.indexOf("2 notas de fornecedor não servem para Compras") >= 0, l.extra)
    }

    function test_n5_singular_do_ja() {
        painel.previa = previa({ comprasJa: 1 })
        const t = textoJa({ modelData: linha("qCompras") })
        verify(t.indexOf("1 já estava em Compras") >= 0, t)
    }
}
"""


def runner():
    """O qmltestrunner do Qt 6 (o /usr/bin/qmltestrunner pode ser o do Qt 5)."""
    candidatos = []
    if os.environ.get("QT_ROOT_DIR"):
        candidatos.append(os.path.join(os.environ["QT_ROOT_DIR"], "bin", "qmltestrunner"))
    candidatos += ["/usr/lib/qt6/bin/qmltestrunner", shutil.which("qmltestrunner6") or "", shutil.which("qmltestrunner") or ""]
    for c in candidatos:
        if c and os.path.exists(c):
            return c
    sys.exit("falta o qmltestrunner do Qt 6")


def achar(padrao, texto, nome, grupo=0):
    """O trecho do QML; sem ele, o teste reprova (a tela não tem o que se confere)."""
    m = re.search(padrao, texto, re.S | re.M)
    return m.group(grupo) if m else f"/* FALTA: {nome} */ undefined"


def main():
    compras = open(os.path.join(RAIZ, "app", "qml", "TelaCompras.qml"), encoding="utf-8").read()
    cad = open(os.path.join(RAIZ, "app", "qml", "ImportarCadastros.qml"), encoding="utf-8").read()
    trocas = {
        "%IMPORTAR%": achar(r"^    function importar\(arquivos\) \{\n.*?^    \}\n", compras, "importar"),
        "%CAT%": achar(r"^    function cat\(codigo\) \{\n.*?^    \}\n", cad, "cat"),
        "%TEXTO%": achar(r"^    function texto\(n, um, varios\) \{\n.*?^    \}\n", cad, "texto"),
        "%LER%": achar(r"^    function lerArquivos\(\) \{\n.*?^    \}\n", cad, "lerArquivos"),
        "%MODELO%": achar(r"model: (painel\.previa \? \[.*?\] : \[\])\n", cad, "o modelo da prévia", 1),
        "%VISIVEL%": achar(r"delegate: Rectangle \{\n\s*id: linha\n.*?\n\s*visible: ([^\n]*)\n", cad, "visible", 1),
        "%TEXTOJA%": achar(r"text: (\[linha\.modelData\.ja > 0 \?.*?\.join\(\"  ·  \"\))", cad, "o texto do já", 1),
        "%ALTERNAR%": achar(r"onToggled: \{\n(.*?)\n\s*\}\n\s*leftPadding", cad, "onToggled", 1),
    }
    teste = TESTE
    for k, v in trocas.items():
        teste = teste.replace(k, v)
    pasta = tempfile.mkdtemp(prefix="rustinho-qml-t108-")
    try:
        mod = os.path.join(pasta, "falsos")
        os.makedirs(mod)
        with open(os.path.join(mod, "Api.qml"), "w", encoding="utf-8") as f:
            f.write(API)
        with open(os.path.join(mod, "Tema.qml"), "w", encoding="utf-8") as f:
            f.write(TEMA)
        with open(os.path.join(mod, "qmldir"), "w", encoding="utf-8") as f:
            f.write("module falsos\nsingleton Api 1.0 Api.qml\nsingleton Tema 1.0 Tema.qml\n")
        with open(os.path.join(pasta, "tst_t108.qml"), "w", encoding="utf-8") as f:
            f.write(teste)
        r = subprocess.run([runner(), "-import", pasta, "-input", os.path.join(pasta, "tst_t108.qml")],
                           env=dict(os.environ, QT_QPA_PLATFORM="offscreen"), capture_output=True, text=True,
                           errors="replace")
        print(r.stdout + r.stderr)
        sys.exit(r.returncode)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


if __name__ == "__main__":
    main()
