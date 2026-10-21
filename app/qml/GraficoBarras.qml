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
                    visible: col.modelData.destaque || col.index % grafico.rotuloCada === 0
                    text: col.modelData.rotulo
                    color: col.modelData.destaque ? Tema.tinta : Tema.apagado
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
