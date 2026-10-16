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

    function focar() {
        if (!Tema.celular || Qt.inputMethod.visible)
            entrada.forceActiveFocus()
    }

    Rotulo {
        text: area.rotulo
        visible: text !== ""
        color: area.erro !== "" ? Tema.vermelho : Tema.apagado
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
        placeholderTextColor: "#9A9086"
        selectionColor: Tema.azulEscuro
        selectedTextColor: "white"
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
        visible: text !== ""
        color: Tema.vermelho
        font.family: Tema.texto
        font.pixelSize: 14
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
        Layout.preferredWidth: 1
    }
}
