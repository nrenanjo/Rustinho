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
    // A superfície embaixo (0.9.18; veja Tema.superficie), como no Campo.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo

    Rotulo {
        text: selecao.rotulo
        visible: text !== ""
        // H05 (7.2): pela superfície, como o Campo (no roxo escuro, claro).
        color: selecao.erro !== "" ? Tema.vermelhoNo(selecao.superficie) : Tema.apagadoNo(selecao.superficie)
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }

    ComboBox {
        id: combo
        Layout.fillWidth: true
        implicitWidth: 120
        // A15 (0.9.18): no celular, a área de toque tem pelo menos 48 dp de
        // altura; o desenho da lista fica com 40, no meio. No computador, igual.
        implicitHeight: Math.max(40, Tema.toqueMinimo)
        topInset: (implicitHeight - 40) / 2
        bottomInset: topInset
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
            textFormat: Text.PlainText
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
            // A15: no celular, cada opção com 48 dp de altura.
            height: Math.max(36, Tema.toqueMinimo)
            text: modelData
            font: combo.font
            highlighted: combo.highlightedIndex === index
            // Texto sempre em tinta: o "realçado" do estilo usava letra clara
            // sobre o azul claro, sem contraste (Nova OS › Serviço, 0.9.15).
            contentItem: Text {
                text: parent.text
                textFormat: Text.PlainText
                font: parent.font
                color: Tema.tinta
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                leftPadding: 4
            }
            background: Rectangle {
                radius: 6
                color: parent.highlighted ? Tema.azul : "transparent"
            }
        }
        popup: Popup {
            // Logo abaixo do desenho da lista (no celular, a área de toque é maior).
            y: (combo.height + 40) / 2 + 4
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
        textFormat: Text.PlainText
        visible: text !== ""
        color: Tema.vermelho
        font.family: Tema.texto
        font.pixelSize: 14
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }
}
