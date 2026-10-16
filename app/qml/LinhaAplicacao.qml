import QtQuick
import QtQuick.Layouts
import rustinho

// Um carro em que a peça serve: montadora, modelo, motor e anos.
RowLayout {
    id: linha
    required property int index
    required property string montadora
    required property string modelo
    required property string motor
    required property string anoInicio
    required property string anoFim
    property var sugestoesMontadora: []
    property var sugestoesModelo: []

    signal mudou(string campo, string valor)
    signal remover()

    spacing: 10

    function campo(nome) {
        const campos = { montadora: campoMontadora, modelo: campoModelo, motor: campoMotor, anoInicio: campoAnoInicio, anoFim: campoAnoFim, observacao: campoModelo }
        return campos[nome] || null
    }

    function focar() {
        campoMontadora.focar()
    }

    Campo {
        id: campoMontadora
        dica: "Montadora"
        maximo: 40
        sugestoes: linha.sugestoesMontadora
        Layout.preferredWidth: 150
        onEditado: linha.mudou("montadora", texto)
    }
    Campo {
        id: campoModelo
        dica: "Modelo (ex.: Gol G5)"
        maximo: 40
        sugestoes: linha.sugestoesModelo
        Layout.fillWidth: true
        onEditado: linha.mudou("modelo", texto)
    }
    Campo {
        id: campoMotor
        dica: "Motor"
        maximo: 30
        Layout.preferredWidth: 104
        onEditado: linha.mudou("motor", texto)
    }
    Campo {
        id: campoAnoInicio
        dica: "De"
        maximo: 4
        numerico: true
        Layout.preferredWidth: 74
        onEditado: linha.mudou("anoInicio", texto)
    }
    Campo {
        id: campoAnoFim
        dica: "Até"
        maximo: 4
        numerico: true
        Layout.preferredWidth: 74
        onEditado: linha.mudou("anoFim", texto)
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
        campoMontadora.texto = montadora
        campoModelo.texto = modelo
        campoMotor.texto = motor
        campoAnoInicio.texto = anoInicio
        campoAnoFim.texto = anoFim
    }
}
