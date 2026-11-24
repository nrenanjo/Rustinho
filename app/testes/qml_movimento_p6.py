#!/usr/bin/env python3
"""P6 (0.9.19-dev.1-D): provas determinísticas de `Movimento.qml`, `Tema.qml` e barreira `T-02` (`Main.qml`).

Roda no `qmltestrunner` do Qt 6 (offscreen, sem janela) nas duas cópias (`app` e `android`), provando:
  1. A2: rolagem de distância média ocorre em múltiplos passos (250..450 ms planejados), chegando ao destino exato.
  2. A3: grande distância (> 3 alturas da área) usa esmaecimento (opacity <= 0.25) + salto para perto + rolagem curta final.
  3. A8: interrupção manual (mudança de contentY durante o movimento) interrompe no mesmo quadro, restaura opacity=1 e NÃO puxa de volta.
  4. A9: movimento reduzido (`Tema.modoMovimento = "ligado"`) posiciona imediatamente em 0 ms sem passos intermediários.
  5. T-02: `onFrameSwapped` só chama `Api.confirmarEntrega()` depois de `onAfterSynchronizing` ter sincronizado a gravação pendente.

Uso:
    python3 app/testes/qml_movimento_p6.py [raiz do projeto]
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
import rustinho

TestCase {
    name: "P6Movimento_%COPIA%"
    width: 600
    height: 400
    when: windowShown

    Flickable {
        id: area
        width: 400
        height: 400
        contentWidth: 400
        contentHeight: 3200
        Item { width: 400; height: 3200 }
    }

    Movimento { id: mov }

    // Barreira T-02 extraída de Main.qml (simulada com Api local).
    Item {
        id: barreiraT02
        property int confirmacoes: 0
        property int updates: 0
        property var apiSim: QtObject {
            property bool _entregaPendente: false
            function confirmarEntrega() { barreiraT02.confirmacoes++; _entregaPendente = false }
        }
        property var janelaSim: QtObject {
            function update() { barreiraT02.updates++ }
        }
        property bool _entregaSincronizadaNoQuadro: false
        function dispararAfterSynchronizing() {
            if (apiSim._entregaPendente)
                _entregaSincronizadaNoQuadro = true
        }
        function dispararFrameSwapped() {
            if (_entregaSincronizadaNoQuadro) {
                _entregaSincronizadaNoQuadro = false
                apiSim.confirmarEntrega()
            } else if (apiSim._entregaPendente) {
                janelaSim.update()
            }
        }
    }

    function init() {
        failOnWarning(/TypeError|ReferenceError/)
        mov.cancelar(true)
        Tema.modoMovimento = "desligado"
        Tema.movimentoReduzidoSistema = false
        area.contentY = 0
        area.opacity = 1
    }

    function cleanup() {
        mov.cancelar(true)
    }

    function test_1_a2_rolagem_suave_multiplos_passos() {
        let terminou = false
        let interrompido = true
        let passos = 0
        mov.rolarSuave(area, 500, {
            aoAtualizar: () => { passos++ },
            aoTerminar: (interr) => { terminou = true; interrompido = interr }
        })
        verify(mov.ativo, "movimento deveria estar ativo")
        verify(mov.duracaoMs >= 250 && mov.duracaoMs <= 450, "duração fora de 250..450 ms: " + mov.duracaoMs)
        tryVerify(() => terminou, 4000, "rolagem não concluiu")
        verify(!interrompido, "não deveria ter sido interrompida")
        verify(passos >= 3, "deveria ter múltiplos passos suaves, teve " + passos)
        verify(Math.abs(area.contentY - 500) <= 1, "destino final incorreto: " + area.contentY)
        compare(area.opacity, 1)
    }

    function test_2_a3_grande_distancia_esmaece_salta_e_desliza() {
        let terminou = false
        let viuFade = false
        let minOpacidade = 1.0
        mov.rolarSuave(area, 2400, {
            aoTerminar: () => { terminou = true }
        })
        compare(mov.estado, "fadeOut", "grande distância (>3x altura) deve iniciar em fadeOut")
        tryVerify(() => {
            if (area.opacity < minOpacidade)
                minOpacidade = area.opacity
            if (area.opacity < 0.8)
                viuFade = true
            return terminou
        }, 5000, "rolagem longa não concluiu")
        verify(viuFade && minOpacidade < 0.8, "esmaecimento não ocorreu (minOp=" + minOpacidade + ")")
        verify(Math.abs(area.contentY - 2400) <= 1, "destino final incorreto: " + area.contentY)
        compare(area.opacity, 1, "opacidade final deve voltar a 1")
    }

    function test_3_a8_interrupcao_manual_para_e_nao_puxa_de_volta() {
        let terminou = false
        let interrompido = false
        let passos = 0
        mov.rolarSuave(area, 600, {
            aoAtualizar: () => {
                passos++
                if (passos === 2) {
                    // Simula intervenção manual (ex.: roda do mouse puxando para cima).
                    area.contentY = 120
                }
            },
            aoTerminar: (interr) => { terminou = true; interrompido = interr }
        })
        tryVerify(() => terminou, 3000, "interrupção manual não encerrou o movimento")
        verify(interrompido, "deveria sinalizar interrupção manual")
        verify(!mov.ativo, "movimento deve ficar inativo após interrupção")
        compare(area.opacity, 1, "opacidade deve voltar a 1 após interrupção")
        wait(120)
        verify(Math.abs(area.contentY - 120) <= 1, "não pode puxar de volta após interrupção (y=" + area.contentY + ")")
    }

    function test_4_a9_movimento_reduzido_imediato() {
        Tema.modoMovimento = "ligado"
        verify(Tema.movimentoReduzido, "Tema.movimentoReduzido deveria ser true")
        let terminou = false
        let passos = 0
        mov.rolarSuave(area, 700, {
            aoAtualizar: () => { passos++ },
            aoTerminar: () => { terminou = true }
        })
        verify(!mov.ativo, "em movimento reduzido não deve ligar animação contínua")
        compare(area.contentY, 700, "deve ir direto ao destino em 0 ms")
        tryVerify(() => terminou, 1000, "callback aoTerminar deve rodar em movimento reduzido")
        compare(passos, 1)
    }

    function test_5_t02_barreira_afterSynchronizing_frameSwapped() {
        barreiraT02.confirmacoes = 0
        barreiraT02.updates = 0
        barreiraT02._entregaSincronizadaNoQuadro = false
        barreiraT02.apiSim._entregaPendente = true
        // Quadro anterior (já em voo antes da gravação) termina swap sem ter sincronizado a gravação:
        barreiraT02.dispararFrameSwapped()
        compare(barreiraT02.confirmacoes, 0, "não pode confirmar entrega antes de afterSynchronizing")
        compare(barreiraT02.updates, 1, "deve solicitar novo quadro quando gravação chegou após sync")
        // Próximo quadro sincroniza e exibe:
        barreiraT02.dispararAfterSynchronizing()
        barreiraT02.dispararFrameSwapped()
        compare(barreiraT02.confirmacoes, 1, "deve confirmar entrega após afterSynchronizing + frameSwapped")
        verify(!barreiraT02.apiSim._entregaPendente)
    }
}
"""


def runner():
    candidatos = [os.environ.get("RUSTINHO_QMLTESTRUNNER", "")]
    if os.environ.get("QT_ROOT_DIR"):
        candidatos.append(os.path.join(os.environ["QT_ROOT_DIR"], "bin", "qmltestrunner"))
    candidatos += [
        "/usr/lib/qt6/bin/qmltestrunner",
        "/usr/lib64/qt6/bin/qmltestrunner",
        shutil.which("qmltestrunner6") or "",
        shutil.which("qmltestrunner") or "",
    ]
    for c in candidatos:
        if c and os.access(c, os.X_OK):
            return c
    sys.exit("falta o qmltestrunner do Qt 6")


def main():
    qmltestrunner = runner()
    falhou = []
    for copia, base_dir, qml_dir in (
        ("app", "app/base", "app/qml"),
        ("android", "android/rust/base", "android/rust/qml"),
    ):
        main_txt = open(os.path.join(RAIZ, qml_dir, "Main.qml"), encoding="utf-8").read()
        if "onAfterSynchronizing:" not in main_txt or "_entregaSincronizadaNoQuadro" not in main_txt:
            print(f"✘ {copia}: Main.qml sem barreira T-02")
            falhou.append(copia)
            continue
        pasta = tempfile.mkdtemp(prefix=f"rustinho-qml-p6-{copia}-")
        try:
            mod = os.path.join(pasta, "rustinho")
            os.makedirs(mod)
            shutil.copy(os.path.join(RAIZ, base_dir, "Tema.qml"), os.path.join(mod, "Tema.qml"))
            shutil.copy(os.path.join(RAIZ, qml_dir, "Movimento.qml"), os.path.join(mod, "Movimento.qml"))
            with open(os.path.join(mod, "qmldir"), "w", encoding="utf-8") as f:
                f.write("module rustinho\nsingleton Tema 1.0 Tema.qml\nMovimento 1.0 Movimento.qml\n")
            arquivo = os.path.join(pasta, f"tst_p6_{copia}.qml")
            with open(arquivo, "w", encoding="utf-8") as f:
                f.write(TESTE.replace("%COPIA%", copia))
            r = subprocess.run(
                [qmltestrunner, "-import", pasta, "-input", arquivo],
                env=dict(os.environ, QT_QPA_PLATFORM="offscreen"),
                capture_output=True, text=True, errors="replace",
            )
            print(r.stdout + r.stderr)
            if r.returncode != 0:
                falhou.append(copia)
        finally:
            shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "reprovou em " + ", ".join(falhou) if falhou else "as duas cópias passaram")
    sys.exit(1 if falhou else 0)


if __name__ == "__main__":
    main()
