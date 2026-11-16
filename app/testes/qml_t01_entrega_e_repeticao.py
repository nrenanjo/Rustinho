#!/usr/bin/env python3
"""QT-1b (dev.3.3): T-01 e T-05 da 2ª re-revisão independente (Fable, 16/11/2026;
`revisao_independente/fable/RELATORIO_REREVISAO2_QT1.md`). Teste de QML (qmltestrunner, sem janela), tirado da sonda do
revisor (`qml_fable_s1_i6.py`).

Usa o `Api.qml` de verdade (app/base) e as funções `chamarGerente`, `chamarGerenteDemorado`, `formaErradaGerente` e
`avisar` tiradas do próprio `app/qml/Main.qml`, com um núcleo simulado (`Sistema`) que anota cada chamada.
  S1: toda gravação com a chave da operação, pelo gravar ou pelo chamarGerente (os 27 comandos do gerente), pede a
      confirmação de entrega no quadro seguinte; leitura e gravação recusada não pedem; núcleo ocupado tenta de novo.
  I6: a repetição pelo gravar e pelo chamarGerente mostra "Já estava feito"; a marca some no fim do tratamento.
Fora deste teste (registrados no relatório e na consolidação): os casos i6_c e i6_d da sonda (OT-01, vazamentos que
nenhuma tela de hoje alcança) e o i6_e (tratamento adiado genérico; o ImportarCadastros lê a marca da resposta).

    python3 app/testes/qml_t01_entrega_e_repeticao.py <raiz do projeto>
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.abspath(sys.argv[1])

SISTEMA = """pragma Singleton
import QtQuick
QtObject {
    property string proxima: '{"ok": true, "dados": null}'
    property string entrega: '{"ok": true, "dados": null}'
    property var chamados: []
    property int pedidos: 0
    signal terminou(int pedido, string resposta)
    function chamar(comando, dados) { chamados.push(comando); return comando === "entrega_confirmar" ? entrega : proxima }
    function chamar_demorado(comando, dados) { chamados.push("demorado:" + comando); pedidos += 1; return pedidos }
    function limpar() { chamados = [] }
    function quantos(c) { return chamados.filter(x => x === c).length }
}
"""

TESTE = """import QtQuick
import QtTest
import rustinho

TestCase {
    name: "QT1T01EntregaERepeticao"
    Item {
        id: janela
        property var estado: ({ usuario: { gerente: true } })
        QtObject {
            id: pedirGerente
            function paraChamada(comando, dados, erro, sim, nao) {}
        }
        QtObject {
            id: aviso
            property string ultimo: ""
            property string tipo: ""
            property var todos: []
            function mostrar(texto, tipoAviso) { ultimo = texto; tipo = tipoAviso; todos.push(tipoAviso + ": " + texto) }
        }
%FUNCOES%
    }
    readonly property string normal: '{"ok": true, "dados": {"numero": 7}}'
    readonly property string repetida: '{"ok": true, "dados": {"numero": 7}, "repetida": true}'
    readonly property string aguarde: '{"ok": false, "erro": "Aguarde: o Rustinho ainda está terminando a operação anterior.", "campo": null}'

    function init() {
        aviso.ultimo = ""; aviso.tipo = ""; aviso.todos = []
        Sistema.proxima = normal; Sistema.entrega = '{"ok": true, "dados": null}'
        // Zera o que ficou do teste anterior (um quadro "desenhado").
        Api.confirmarEntrega(); wait(0); Sistema.limpar()
    }

    // ---- S1 ----
    function test_s1_a_gravar_e_confirmado_no_quadro() {
        const r = Api.gravar("pessoa_salvar", {}, "k1")
        verify(r.ok)
        compare(Sistema.quantos("entrega_confirmar"), 0, "antes do quadro, nada é confirmado")
        Api.confirmarEntrega()   // o quadro foi desenhado (Main.qml, onFrameSwapped)
        compare(Sistema.quantos("entrega_confirmar"), 1)
        Api.confirmarEntrega(); Api.confirmarEntrega()
        compare(Sistema.quantos("entrega_confirmar"), 1, "uma vez por gravação, não a cada quadro")
    }
    function test_s1_b_leitura_nao_pede_confirmacao() {
        Api.chamar("caixa_estado", {})
        Api.confirmarEntrega()
        compare(Sistema.quantos("entrega_confirmar"), 0)
    }
    function test_s1_c_chamarGerente_tambem_confirma_no_quadro() {
        let resposta = null
        janela.chamarGerente("pagar_pagar", { id: "p1", forma: "dinheiro" }, r => { resposta = r }, "k2")
        verify(resposta && resposta.ok, "a gravação pelo chamarGerente deu ok")
        verify(Sistema.quantos("pagar_pagar") === 1)
        Api.confirmarEntrega(); Api.confirmarEntrega(); Api.confirmarEntrega()   // três quadros desenhados
        console.log("QT1-T01 chamados depois de chamarGerente + 3 quadros: " + JSON.stringify(Sistema.chamados))
        compare(Sistema.quantos("entrega_confirmar"), 1, "T-01: a gravação pelo chamarGerente é confirmada depois do quadro")
    }
    function test_s1_d_chamarGerenteDemorado_tambem_confirma() {
        let resposta = null
        janela.chamarGerenteDemorado("backup_restaurar", { arquivo: "x" }, "Espere…", r => { resposta = r }, "k3")
        Sistema.terminou(Sistema.pedidos, normal)
        verify(resposta && resposta.ok)
        Api.confirmarEntrega()
        compare(Sistema.quantos("entrega_confirmar"), 1, "a gravação demorada pelo chamarGerenteDemorado não é confirmada")
    }
    function test_s1_e_nucleo_ocupado_tenta_de_novo_a_cada_quadro() {
        Api.gravar("venda_concluir", {}, "k4")
        Sistema.entrega = aguarde
        for (let i = 0; i < 5; i++) Api.confirmarEntrega()
        compare(Sistema.quantos("entrega_confirmar"), 5, "ocupado: uma tentativa por quadro")
        compare(aviso.todos.length, 0, "a tentativa recusada não vira aviso na tela")
        Sistema.entrega = '{"ok": true, "dados": null}'
        Api.confirmarEntrega(); Api.confirmarEntrega()
        compare(Sistema.quantos("entrega_confirmar"), 6)
    }
    function test_s1_f_gravacao_recusada_nao_pede_confirmacao() {
        Sistema.proxima = '{"ok": false, "erro": "Falta o cliente.", "campo": "cliente"}'
        Api.gravar("venda_concluir", {}, "k5")
        Api.confirmarEntrega()
        compare(Sistema.quantos("entrega_confirmar"), 0)
    }

    // ---- I6 ----
    function test_i6_a_repeticao_pelo_gravar() {
        Sistema.proxima = repetida
        Api.gravar("receber_lancar", {}, "c1")
        janela.avisar("Dívida lançada.", "ok")
        compare(aviso.ultimo, "Já estava feito. Dívida lançada.")
    }
    function test_i6_b_repeticao_pelo_chamarGerente() {
        Sistema.proxima = repetida
        janela.chamarGerente("pagar_pagar", { id: "p1" }, r => { if (r.ok) janela.avisar("Conta paga.", "ok") }, "c2")
        console.log("QT1-T01 aviso da repetição pelo chamarGerente: " + aviso.ultimo)
        compare(aviso.ultimo, "Já estava feito. Conta paga.", "T-05: a repetição pelo chamarGerente mostra 'Já estava feito'")
    }
    function test_i6_f_a_marca_some_no_fim_do_tratamento() {
        Sistema.proxima = repetida
        Api.gravar("venda_concluir", {}, "c7")
        wait(0)
        janela.avisar("Código lido: 123", "ok")
        compare(aviso.ultimo, "Código lido: 123")
    }
}
"""


def runner():
    for c in [os.environ.get("RUSTINHO_QMLTESTRUNNER", ""), "/usr/lib/qt6/bin/qmltestrunner",
              shutil.which("qmltestrunner6") or "", shutil.which("qmltestrunner") or ""]:
        if c and os.access(c, os.X_OK):
            return c
    sys.exit("falta o qmltestrunner do Qt 6")


def funcao(main_qml, nome):
    m = re.search(r"^    function " + nome + r"\([^)]*\) \{\n.*?^    \}\n", main_qml, re.S | re.M)
    if not m:
        sys.exit(f"não achei a função {nome} no Main.qml")
    return "\n".join("        " + ln[4:] if ln.strip() else ln for ln in m.group(0).splitlines())


def main():
    main_qml = open(os.path.join(RAIZ, "app", "qml", "Main.qml"), encoding="utf-8").read()
    funcoes = "\n".join(funcao(main_qml, n) for n in
                        ("formaErradaGerente", "chamarGerente", "chamarGerenteDemorado", "avisar"))
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-qml-")
    try:
        mod = os.path.join(pasta, "rustinho")
        os.makedirs(mod)
        shutil.copy(os.path.join(RAIZ, "app", "base", "Api.qml"), os.path.join(mod, "Api.qml"))
        open(os.path.join(mod, "Sistema.qml"), "w", encoding="utf-8").write(SISTEMA)
        open(os.path.join(mod, "qmldir"), "w", encoding="utf-8").write(
            "module rustinho\nsingleton Sistema 1.0 Sistema.qml\nsingleton Api 1.0 Api.qml\n")
        open(os.path.join(pasta, "tst_fable.qml"), "w", encoding="utf-8").write(TESTE.replace("%FUNCOES%", funcoes))
        r = subprocess.run([runner(), "-import", pasta, "-input", os.path.join(pasta, "tst_fable.qml")],
                           env=dict(os.environ, QT_QPA_PLATFORM="offscreen"), capture_output=True, text=True,
                           errors="replace")
        print(r.stdout + r.stderr)
        sys.exit(r.returncode)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


if __name__ == "__main__":
    main()
