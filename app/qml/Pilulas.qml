import QtQuick
import QtQuick.Layouts
import rustinho

// Escolha entre poucas opções, em pílulas arredondadas (sem abas nem linhas).
// modelo: [{ id: "abertas", nome: "Em aberto" }, ...]
// Na tela estreita, as pílulas que não cabem passam para a linha de baixo.
Flow {
    id: pilulas
    // Dentro de um Layout, ocupa a largura toda (para poder quebrar a linha).
    Layout.fillWidth: true
    property var modelo: []
    property string atual: ""
    property bool pequenas: false
    signal escolhido(string id)

    spacing: 6
    // Fora de um Layout: largura do que couber na linha.
    implicitWidth: _soma
    implicitHeight: childrenRect.height
    readonly property real _soma: {
        let t = 0
        for (let i = 0; i < children.length; i++)
            t += children[i].width + (i > 0 ? spacing : 0)
        return t
    }

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
