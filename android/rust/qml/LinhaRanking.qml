import QtQuick
import QtQuick.Layouts
import rustinho

// Uma linha de ranking com barrinha: nome, detalhe, valor e percentual.
// item: { nome, detalhe, valor, parte (0 a 1), percentual }
ColumnLayout {
    id: linha
    property var item: ({})
    property bool dinheiro: true
    property color cor: Tema.azulEscuro
    property int posicao: 0
    // Valor que não é dinheiro mas também some com o olho fechado (margem).
    property bool sensivel: false
    signal abrir()

    spacing: 4
    Layout.fillWidth: true

    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        Text {
            visible: linha.posicao > 0
            text: linha.posicao
            color: Tema.dourado
            font.family: Tema.titulo
            font.weight: Font.Bold
            font.pixelSize: 18
            Layout.preferredWidth: 18
        }
        Text {
            text: linha.item.nome || ""
            color: Tema.tinta
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 15
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        Text {
            text: Tema.sigiloTexto(linha.item.detalhe || "")
            color: Tema.apagado
            font.family: Tema.serifa
            font.italic: true
            font.pixelSize: 12
        }
        Text {
            text: (linha.dinheiro ? "R$ " + Tema.valor(linha.item.valor)
                   : linha.sensivel && Tema.oculto ? "••" : linha.item.valor || "")
            color: Tema.tinta
            font.family: Tema.texto
            font.weight: Font.Bold
            font.pixelSize: 15
            horizontalAlignment: Text.AlignRight
            Layout.minimumWidth: 86
        }
    }
    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        Layout.leftMargin: linha.posicao > 0 ? 28 : 0
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 8
            radius: 4
            color: Tema.vela(0.5)
            Rectangle {
                height: parent.height
                radius: 4
                width: Math.max(linha.item.parte > 0 ? 8 : 0, parent.width * (linha.item.parte || 0))
                color: linha.cor
                opacity: 0.55
                Behavior on width { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
            }
        }
        Text {
            text: linha.item.percentual || ""
            color: Tema.tintaSuave
            font.family: Tema.texto
            font.pixelSize: 12
            horizontalAlignment: Text.AlignRight
            Layout.preferredWidth: 34
        }
    }
}
