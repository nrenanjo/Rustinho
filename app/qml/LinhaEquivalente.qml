import QtQuick
import QtQuick.Layouts
import rustinho

// A mesma peça em outra marca: marca e código.
RowLayout {
    id: linha
    required property int index
    required property string marca
    required property string codigo
    property var sugestoesMarca: []

    signal mudou(string campo, string valor)
    signal remover()

    spacing: 10

    function campo(nome) {
        return nome === "marca" ? campoMarca : campoCodigo
    }

    function focar() {
        campoMarca.focar()
    }

    Campo {
        id: campoMarca
        dica: "Marca (ex.: SKF)"
        maximo: 40
        sugestoes: linha.sugestoesMarca
        Layout.preferredWidth: Tema.estreito ? 154 : 220
        onEditado: linha.mudou("marca", texto)
    }
    Campo {
        id: campoCodigo
        dica: "Código nessa marca"
        maximo: 60
        Layout.fillWidth: true
        onEditado: linha.mudou("codigo", texto)
    }
    Botao {
        tipo: "discreto"
        icone: "lixeira"
        implicitWidth: 40
        leftPadding: 0
        rightPadding: 0
        Layout.alignment: Qt.AlignTop
        onClicked: linha.remover()
    }

    Component.onCompleted: {
        campoMarca.texto = marca
        campoCodigo.texto = codigo
    }
}
