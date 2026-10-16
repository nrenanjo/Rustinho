import QtQuick
import QtQuick.Layouts
import rustinho

// Um carro do cliente: placa e descrição.
RowLayout {
    id: linha
    required property int index
    required property string placa
    required property string descricao

    signal mudou(string campo, string valor)
    signal remover()

    spacing: 10

    function campo(nome) {
        return nome === "placa" ? campoPlaca : campoDescricao
    }

    function focar() {
        campoPlaca.focar()
    }

    Campo {
        id: campoPlaca
        dica: "Placa"
        maximo: 8
        maiusculas: true
        Layout.preferredWidth: 130
        onEditado: linha.mudou("placa", texto)
    }
    Campo {
        id: campoDescricao
        dica: "Carro (ex.: Gol G5 1.6 2012 prata)"
        maximo: 60
        Layout.fillWidth: true
        onEditado: linha.mudou("descricao", texto)
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
        campoPlaca.texto = placa
        campoDescricao.texto = descricao
    }
}
