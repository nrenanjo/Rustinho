import QtQuick
import QtQuick.Controls.Basic
import rustinho

// Pílula do olho: mostra ou esconde CPF/CNPJ, custo, lucro, comissões e
// saldos em todas as telas de uma vez.
// A15 (0.9.18): o item é a área de toque; no celular, tem pelo menos 48 dp
// de altura (Tema.toqueMinimo) e a pílula fica com 34, no meio. No
// computador, item = pílula.
Item {
    id: olho
    implicitHeight: Math.max(34, Tema.toqueMinimo)
    implicitWidth: Math.max(linha.implicitWidth + 28, Tema.toqueMinimo)

    Rectangle {
        anchors.centerIn: parent
        width: linha.implicitWidth + 28
        height: 34
        radius: height / 2
        color: Tema.oculto ? (sobre.hovered ? Tema.pilulaClaraSobre : Tema.pilulaClara) : (sobre.hovered ? Tema.rosaSobre : Tema.rosa)
        Behavior on color { ColorAnimation { duration: 120 } }
    }

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
