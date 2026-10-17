import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Lista de escolha (ComboBox) com rótulo.
ColumnLayout {
    id: selecao
    property string rotulo: ""
    property alias modelo: combo.model
    property alias indice: combo.currentIndex
    property alias combo: combo
    property string erro: ""

    signal escolhido()

    spacing: 5
    Layout.alignment: Qt.AlignTop
    // Por padrão não estica; quem usar decide (Layout.fillWidth: true).
    Layout.fillWidth: false

    Rotulo {
        text: selecao.rotulo
        visible: text !== ""
        color: selecao.erro !== "" ? Tema.vermelho : Tema.apagado
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }

    ComboBox {
        id: combo
        Layout.fillWidth: true
        implicitWidth: 120
        implicitHeight: 40
        font.family: Tema.texto
        font.pixelSize: 15
        onActivated: {
            selecao.erro = ""
            selecao.escolhido()
        }

        background: Rectangle {
            radius: Tema.raioCampo
            color: Tema.papel
            border.width: combo.visualFocus || selecao.erro !== "" ? 2 : 0
            border.color: selecao.erro !== "" ? Tema.vermelho : Tema.azulEscuro
        }
        contentItem: Text {
            leftPadding: 12
            rightPadding: 34
            text: combo.displayText
            font: combo.font
            color: Tema.tinta
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
        indicator: Icone {
            nome: "baixo"
            tamanho: 16
            cor: Tema.apagado
            x: combo.width - width - 12
            y: (combo.height - height) / 2
        }
        delegate: ItemDelegate {
            required property var modelData
            required property int index
            width: combo.width - 8
            height: 36
            text: modelData
            font: combo.font
            highlighted: combo.highlightedIndex === index
            background: Rectangle {
                radius: 6
                color: parent.highlighted ? Tema.azul : "transparent"
            }
        }
        popup: Popup {
            y: combo.height + 4
            width: combo.width
            padding: 4
            implicitHeight: Math.min(contentItem.implicitHeight + 8, 340)
            contentItem: ListView {
                clip: true
                implicitHeight: contentHeight
                model: combo.popup.visible ? combo.delegateModel : null
                currentIndex: combo.highlightedIndex
                boundsBehavior: Flickable.StopAtBounds
            }
            background: Rectangle {
                color: Tema.papel
                radius: 8
                border.width: 1
                border.color: Tema.sombra(0.12)
            }
        }
        HoverHandler { cursorShape: Qt.PointingHandCursor }
    }

    Text {
        text: selecao.erro
        visible: text !== ""
        color: Tema.vermelho
        font.family: Tema.texto
        font.pixelSize: 14
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }
}
