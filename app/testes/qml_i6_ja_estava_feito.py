#!/usr/bin/env python3
"""I6 (decisão de 15/11/2026): quando a mesma operação repetida volta "ok" com a marca `repetida` (o núcleo não
gravou de novo), o aviso verde que a tela mostra em seguida começa com "Já estava feito".

Teste de QML (qmltestrunner, sem janela): usa o `Api.qml` de verdade (app/base) e a função `avisar` tirada do
próprio `app/qml/Main.qml`, com um núcleo simulado (`Sistema`) que devolve a resposta escolhida. Confere:
  1. gravação normal: o aviso verde fica como a tela mandou;
  2. repetição: o aviso verde começa com "Já estava feito.", e a marca é consumida;
  3. repetição seguida de aviso de erro: o erro fica como está, e a marca não vaza para um aviso verde depois;
  4. repetição sem aviso no mesmo tratamento: a marca some no fim dele (não vai parar no aviso de outra ação);
  5. o mesmo pelo caminho demorado (gravarDemorado).

    python3 app/testes/qml_i6_ja_estava_feito.py [raiz do projeto]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", ".."))

SISTEMA = """pragma Singleton
import QtQuick
QtObject {
    property string proxima: '{"ok": true, "dados": null}'
    property var chamados: []
    property int pedidos: 0
    signal terminou(int pedido, string resposta)
    function chamar(comando, dados) { chamados.push(comando); return proxima }
    function chamar_demorado(comando, dados) { pedidos += 1; return pedidos }
}
"""

TESTE = """import QtQuick
import QtTest
import rustinho

TestCase {
    name: "I6"
    Item {
        id: janela
        QtObject {
            id: aviso
            property string ultimo: ""
            property string tipo: ""
            function mostrar(texto, tipoAviso) { ultimo = texto; tipo = tipoAviso }
        }
%AVISAR%
    }
    readonly property string normal: '{"ok": true, "dados": {"numero": 7}}'
    readonly property string repetida: '{"ok": true, "dados": {"numero": 7}, "repetida": true}'

    function init() { aviso.ultimo = ""; aviso.tipo = ""; wait(0) }

    function test_1_gravacao_normal_nao_muda_o_aviso() {
        Sistema.proxima = normal
        const r = Api.gravar("venda_concluir", {}, "c1")
        verify(r.ok)
        janela.avisar("Venda nº 7 concluída.", "ok")
        compare(aviso.ultimo, "Venda nº 7 concluída.")
    }
    function test_2_repeticao_mostra_ja_estava_feito() {
        Sistema.proxima = repetida
        const r = Api.gravar("venda_concluir", {}, "c2")
        verify(r.ok && r.repetida === true)
        janela.avisar("Venda nº 7 concluída.", "ok")
        compare(aviso.ultimo, "Já estava feito. Venda nº 7 concluída.")
        compare(aviso.tipo, "ok")
        janela.avisar("Outro aviso verde.", "ok")
        compare(aviso.ultimo, "Outro aviso verde.", "a marca vale para um aviso só")
    }
    function test_3_erro_nao_leva_a_marca_e_ela_nao_vaza() {
        Sistema.proxima = repetida
        Api.gravar("venda_concluir", {}, "c3")
        janela.avisar("Não deu para imprimir.", "erro")
        compare(aviso.ultimo, "Não deu para imprimir.")
        wait(0)
        janela.avisar("Código lido: 123", "ok")
        compare(aviso.ultimo, "Código lido: 123", "a marca vazou para o aviso de outra ação")
    }
    function test_4_sem_aviso_no_mesmo_tratamento_a_marca_some() {
        Sistema.proxima = repetida
        Api.gravar("venda_concluir", {}, "c4")
        verify(Api.repetidaAgora === true)
        wait(0)
        verify(Api.repetidaAgora === false, "a marca ficou depois do tratamento")
        janela.avisar("Observação guardada.", "ok")
        compare(aviso.ultimo, "Observação guardada.")
    }
    function test_5_caminho_demorado() {
        let visto = ""
        Api.gravarDemorado("restaurar", {}, "Espere…", r => { janela.avisar("Backup restaurado.", "ok"); visto = aviso.ultimo }, "c5")
        Sistema.terminou(Sistema.pedidos, repetida)
        compare(visto, "Já estava feito. Backup restaurado.")
        wait(0)
        verify(Api.repetidaAgora === false)
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


def main():
    qmltestrunner = runner()
    main_qml = open(os.path.join(RAIZ, "app", "qml", "Main.qml"), encoding="utf-8").read()
    m = re.search(r"^    function avisar\(texto, tipo\) \{\n.*?^    \}\n", main_qml, re.S | re.M)
    if not m:
        sys.exit("não achei a função avisar no Main.qml")
    pasta = tempfile.mkdtemp(prefix="rustinho-qml-i6-")
    try:
        mod = os.path.join(pasta, "rustinho")
        os.makedirs(mod)
        shutil.copy(os.path.join(RAIZ, "app", "base", "Api.qml"), os.path.join(mod, "Api.qml"))
        with open(os.path.join(mod, "Sistema.qml"), "w", encoding="utf-8") as f:
            f.write(SISTEMA)
        with open(os.path.join(mod, "qmldir"), "w", encoding="utf-8") as f:
            f.write("module rustinho\nsingleton Sistema 1.0 Sistema.qml\nsingleton Api 1.0 Api.qml\n")
        avisar = "\n".join("        " + ln[4:] if ln.strip() else ln for ln in m.group(0).splitlines())
        with open(os.path.join(pasta, "tst_i6.qml"), "w", encoding="utf-8") as f:
            f.write(TESTE.replace("%AVISAR%", avisar))
        r = subprocess.run([qmltestrunner, "-import", pasta, "-input", os.path.join(pasta, "tst_i6.qml")],
                           env=dict(os.environ, QT_QPA_PLATFORM="offscreen"), capture_output=True, text=True,
                           errors="replace")
        print(r.stdout + r.stderr)
        sys.exit(r.returncode)
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


if __name__ == "__main__":
    main()
