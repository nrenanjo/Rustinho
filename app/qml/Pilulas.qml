import QtQuick
import QtQuick.Layouts
import rustinho

// Escolha entre poucas opções, em pílulas arredondadas (sem abas nem linhas).
// modelo: [{ id: "abertas", nome: "Em aberto" }, ...]
// Quando não cabem numa linha (celular), as pílulas passam para a de baixo.
Item {
    id: pilulas
    property var modelo: []
    property string atual: ""
    property bool pequenas: false
    signal escolhido(string id)

    // Dentro de um Layout, ocupa a largura toda (para poder quebrar a linha).
    Layout.fillWidth: true
    // Largura de tudo numa linha só (fora de um Layout).
    implicitWidth: _soma
    implicitHeight: fluxo.implicitHeight
    readonly property real _soma: {
        let t = 0
        for (let i = 0; i < repetidor.count; i++) {
            const c = repetidor.itemAt(i)
            if (c)
                t += c.width + (i > 0 ? fluxo.spacing : 0)
        }
        return t
    }

    Flow {
        id: fluxo
        width: pilulas.width
        spacing: 6

        Repeater {
            id: repetidor
            model: pilulas.modelo
            delegate: Rectangle {
                id: pilula
                required property var modelData
                readonly property bool ativa: pilulas.atual === modelData.id
                radius: height / 2
                height: pilulas.pequenas ? 32 : (Tema.celular ? 42 : 40)
                width: rotulo.implicitWidth + (pilulas.pequenas ? 26 : 36)
                color: ativa ? Tema.tinta : sobre.hovered ? Tema.sombra(0.07) : "transparent"
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
}
