import QtQuick
import rustinho

// Etiqueta arredondada de situação ("Vencida", "Paga", "Em carteira").
Rectangle {
    id: chip
    property string texto: ""
    property color corTexto: Tema.tinta
    visible: texto !== ""
    radius: height / 2
    color: Tema.papel
    implicitWidth: rotulo.implicitWidth + 18
    implicitHeight: rotulo.implicitHeight + 6
    Text {
        id: rotulo
        anchors.centerIn: parent
        text: chip.texto
        color: chip.corTexto
        font.family: Tema.texto
        font.weight: Font.DemiBold
        font.pixelSize: 12
    }
}
