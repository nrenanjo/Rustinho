import QtQuick
import QtQuick.Layouts
import rustinho

// Escolha entre poucas opções, em pílulas arredondadas (sem abas nem linhas).
// modelo: [{ id: "abertas", nome: "Em aberto" }, ...]
Row {
    id: pilulas
    property var modelo: []
    property string atual: ""
    property bool pequenas: false
    signal escolhido(string id)

    spacing: 6

    Repeater {
        model: pilulas.modelo
        delegate: Rectangle {
            id: pilula
            required property var modelData
            readonly property bool ativa: pilulas.atual === modelData.id
            radius: height / 2
            height: pilulas.pequenas ? 32 : 40
            width: rotulo.implicitWidth + (pilulas.pequenas ? 26 : 36)
            color: ativa ? Tema.tinta : sobre.hovered ? Qt.rgba(0.16, 0.15, 0.13, 0.07) : "transparent"
            Behavior on color { ColorAnimation { duration: 120 } }
            Text {
                id: rotulo
                anchors.centerIn: parent
                text: pilula.modelData.nome
                color: pilula.ativa ? Tema.claro : Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: pilulas.pequenas ? 14 : 16
            }
            HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
            TapHandler {
                onTapped: pilulas.escolhido(pilula.modelData.id)
            }
        }
    }
}
