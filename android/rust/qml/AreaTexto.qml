import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Campo de várias linhas (observações).
ColumnLayout {
    id: area
    property string rotulo: ""
    property alias texto: entrada.text
    property alias dica: entrada.placeholderText
    property string erro: ""

    signal editado()

    spacing: 5
    Layout.alignment: Qt.AlignTop
    // Por padrão não estica; quem usar decide (Layout.fillWidth: true).
    Layout.fillWidth: false
    // A superfície embaixo (0.9.18; veja Tema.superficie), como no Campo.
    readonly property color superficie: Tema.roxo ? palette.window : Tema.fundo

    function focar() {
        if (!Tema.celular || Qt.inputMethod.visible)
            entrada.forceActiveFocus()
    }

    Rotulo {
        text: area.rotulo
        visible: text !== ""
        // H05 (7.2): pela superfície, como o Campo (no roxo escuro, claro).
        color: area.erro !== "" ? Tema.vermelhoNo(area.superficie) : Tema.apagadoNo(area.superficie)
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }
    TextArea {
        id: entrada
        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(84, implicitHeight)
        wrapMode: TextEdit.Wrap
        font.family: Tema.texto
        font.pixelSize: 16
        color: Tema.tinta
        placeholderTextColor: Tema.dica
        selectionColor: Tema.azulEscuro
        selectedTextColor: Tema.claro
        padding: 12
        selectByMouse: true
        background: Rectangle {
            radius: Tema.raioCampo
            color: Tema.papel
            border.width: area.erro !== "" || entrada.activeFocus ? 2 : 0
            border.color: area.erro !== "" ? Tema.vermelho : Tema.azulEscuro
        }
        onTextChanged: if (activeFocus) {
            area.erro = ""
            area.editado()
        }
    }
    Text {
        text: area.erro
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
