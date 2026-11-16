import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Gráfico de barras em pílulas (uma cor só). Cada barra:
// { rotulo, dica, valor, parte (0 a 1), destaque, futuro }
// Passando o mouse aparece o valor (em R$, escondido pelo olho se `dinheiro`).
Item {
    id: grafico
    property var barras: []
    property bool dinheiro: true
    property string unidade: ""
    property color cor: Tema.azulEscuro
    property color corDestaque: Tema.verdeEscuro
    property real alturaBarras: 150
    // Mostra só 1 rótulo a cada N (para não embolar).
    property int rotuloCada: 1

    implicitHeight: alturaBarras + 24
    Layout.fillWidth: true

    readonly property real coluna: barras.length > 0 ? width / barras.length : width

    // I8 (VIS-C8-02, 15/11/2026): no celular, as colunas ficam estreitas (30 dias a 360 dp: uns 10 dp cada; 12 meses
    // no DRE: uns 23 dp) e os rótulos se sobrepunham ("10" × "11", "dez/25" × "jan/26"). O passo dos rótulos é o
    // `rotuloCada` ou o necessário para o maior rótulo caber com folga, o que for maior; o destacado (hoje, o mês
    // atual) sempre aparece, e os vizinhos que encostariam nele, não. Onde já cabe (no computador), nada muda.
    FontMetrics { id: medida; font.family: Tema.texto; font.pixelSize: 11; font.bold: true }
    readonly property real larguraRotulo: {
        let m = 0
        for (let i = 0; i < barras.length; i++)
            m = Math.max(m, medida.advanceWidth(String(barras[i].rotulo)))
        return m
    }
    // Quantas colunas o maior rótulo ocupa (1 quando cabe na própria coluna).
    readonly property int ocupa: coluna > 0 ? Math.max(1, Math.ceil((larguraRotulo + 4) / coluna)) : 1
    readonly property int passo: Math.max(rotuloCada, ocupa)
    // B-5 da revisão do VIS2: com várias colunas destacadas (horas empatadas no pico), os vizinhos de CADA uma saem, e
    // duas destacadas que se cruzariam não aparecem juntas (fica a primeira). Onde o rótulo cabe na coluna (ocupa 1),
    // nada muda.
    readonly property var destaquesVisiveis: {
        const v = []
        for (let i = 0; i < barras.length; i++)
            if (barras[i].destaque && (v.length === 0 || i - v[v.length - 1] >= ocupa))
                v.push(i)
        return v
    }
    function mostraRotulo(i) {
        if (barras[i].destaque)
            return destaquesVisiveis.indexOf(i) >= 0
        for (let k = 0; k < destaquesVisiveis.length; k++)
            if (Math.abs(i - destaquesVisiveis[k]) < ocupa)
                return false
        return i % passo === 0
    }

    Row {
        anchors.fill: parent
        Repeater {
            model: grafico.barras
            delegate: Item {
                id: col
                required property var modelData
                required property int index
                width: grafico.coluna
                height: grafico.height

                // Trilho claro em pílula, e a barra por dentro.
                Rectangle {
                    id: trilho
                    width: Math.max(6, Math.min(grafico.coluna * 0.58, 26))
                    height: grafico.alturaBarras
                    anchors.horizontalCenter: parent.horizontalCenter
                    radius: width / 2
                    color: col.modelData.futuro ? Tema.vela(0.18) : sobre.hovered ? Tema.vela(0.75) : Tema.vela(0.45)
                    Behavior on color { ColorAnimation { duration: 100 } }

                    Rectangle {
                        visible: col.modelData.parte > 0
                        width: parent.width
                        height: Math.max(width, parent.height * col.modelData.parte)
                        anchors.bottom: parent.bottom
                        radius: width / 2
                        color: col.modelData.destaque ? grafico.corDestaque : grafico.cor
                        opacity: col.modelData.destaque ? 0.9 : sobre.hovered ? 0.75 : 0.5
                        Behavior on height { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
                    }
                }
                Text {
                    anchors.top: trilho.bottom
                    anchors.topMargin: 5
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: grafico.mostraRotulo(col.index)
                    text: col.modelData.rotulo
                    textFormat: Text.PlainText
                    color: col.modelData.destaque ? Tema.tintaNo(palette.window) : Tema.apagadoNo(palette.window)
                    font.family: Tema.texto
                    font.weight: col.modelData.destaque ? Font.Bold : Font.Normal
                    font.pixelSize: 11
                }

                HoverHandler { id: sobre }
                ToolTip.visible: sobre.hovered && !col.modelData.futuro
                ToolTip.delay: 150
                ToolTip.text: (grafico.dinheiro ? "R$ " + Tema.valor(col.modelData.valor) : col.modelData.valor + grafico.unidade)
                              + "\n" + Tema.sigiloTexto(col.modelData.dica)
            }
        }
    }
}
