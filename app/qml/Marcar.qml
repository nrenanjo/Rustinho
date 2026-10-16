import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Caixa de marcar.
CheckBox {
    id: caixa
    font.family: Tema.texto
    font.pixelSize: 15
    spacing: 10
    // No celular, o texto comprido quebra a linha em vez de passar da tela.
    Layout.fillWidth: Tema.estreito

    indicator: Rectangle {
        implicitWidth: 22
        implicitHeight: 22
        x: caixa.leftPadding
        y: (caixa.height - height) / 2
        radius: 5
        color: caixa.checked ? Tema.tinta : Tema.papel
        border.width: caixa.visualFocus ? 2 : 0
        border.color: Tema.azulEscuro
        Icone {
            anchors.centerIn: parent
            nome: "check"
            cor: Tema.claro
            tamanho: 16
            espessura: 2.2
            visible: caixa.checked
        }
    }

    contentItem: Text {
        leftPadding: caixa.indicator.width + caixa.spacing
        text: caixa.text
        font: caixa.font
        color: Tema.tinta
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
    }

    HoverHandler { cursorShape: Qt.PointingHandCursor }
}
