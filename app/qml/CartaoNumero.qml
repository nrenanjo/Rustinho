import QtQuick
import QtQuick.Layouts
import rustinho

// Cartão com um número grande (totais do topo das telas).
Rectangle {
    id: cartao
    property string rotulo: ""
    property string valor: ""
    property string prefixo: "R$ "
    property color cor: Tema.pergaminho
    property color corValor: Tema.tinta
    property string nota: ""
    property color corNota: Tema.apagado
    // A nota vai embaixo do número (para notas compridas).
    property bool notaEmbaixo: false
    // Valor escondido enquanto o olho estiver fechado.
    property bool sensivel: false

    radius: Tema.raio
    color: cor
    implicitHeight: 72
    Layout.fillWidth: true

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 14
        anchors.topMargin: 10
        anchors.bottomMargin: 10
        spacing: 0
        RowLayout {
            spacing: 8
            Layout.fillWidth: true
            Rotulo { text: cartao.rotulo }
            Item { Layout.fillWidth: true }
            Text {
                visible: !cartao.notaEmbaixo
                text: cartao.nota
                color: cartao.corNota
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 12
                elide: Text.ElideRight
                Layout.maximumWidth: cartao.width * 0.6
            }
        }
        Text {
            text: cartao.prefixo + (cartao.sensivel ? Tema.valor(cartao.valor) : cartao.valor)
            color: cartao.corValor
            font.family: Tema.titulo
            font.weight: Font.Bold
            font.pixelSize: 28
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        Text {
            visible: cartao.notaEmbaixo && cartao.nota !== ""
            text: cartao.nota
            color: cartao.corNota
            font.family: Tema.serifa
            font.italic: true
            font.pixelSize: 13
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
}
