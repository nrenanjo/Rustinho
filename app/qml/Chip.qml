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
    // 0.9.18: no roxo, um selo de cor transparente (véu) fica sobre o papel,
    // não sobre a faixa escura da lista: o texto escuro continua legível.
    Rectangle {
        visible: Tema.roxo && chip.color.a < 1
        anchors.fill: parent
        radius: chip.radius
        color: Tema.papel
        z: -1
    }
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
