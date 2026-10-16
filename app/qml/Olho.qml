import QtQuick
import QtQuick.Controls.Basic
import rustinho

// Pílula do olho: mostra ou esconde CPF/CNPJ, custo, lucro, comissões e
// saldos em todas as telas de uma vez.
Rectangle {
    id: olho
    radius: height / 2
    implicitHeight: 34
    implicitWidth: linha.implicitWidth + 28
    color: Tema.oculto ? (sobre.hovered ? Qt.rgba(1, 1, 1, 0.7) : Qt.rgba(1, 1, 1, 0.45)) : (sobre.hovered ? "#E4CFC7" : Tema.rosa)
    Behavior on color { ColorAnimation { duration: 120 } }

    Row {
        id: linha
        anchors.centerIn: parent
        spacing: 8
        Icone {
            nome: Tema.oculto ? "olhoFechado" : "olho"
            tamanho: 20
            cor: Tema.oculto ? Tema.tintaSuave : Tema.vermelho
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            text: Tema.oculto ? "Mostrar valores" : "Esconder"
            color: Tema.oculto ? Tema.tintaSuave : Tema.vermelho
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 14
            anchors.verticalCenter: parent.verticalCenter
        }
    }

    HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: Tema.oculto = !Tema.oculto }

    ToolTip.visible: sobre.hovered
    ToolTip.delay: 600
    ToolTip.text: Tema.oculto ? "Mostrar CPF/CNPJ, custo, lucro e saldos (F12)" : "Esconder CPF/CNPJ, custo, lucro e saldos (F12)"
}
