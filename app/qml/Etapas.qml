import QtQuick
import QtQuick.Layouts
import rustinho

// Caminho em pílulas (ex.: Orçamento › Em serviço › Pronta › Entregue).
// As que já passaram ficam verdes; a atual, escura. No celular, quebra a linha.
Item {
    id: etapas
    property var modelo: []      // [{ id, nome }]
    property string atual: ""
    readonly property int indice: modelo.findIndex(m => m.id === atual)

    Layout.fillWidth: true
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
        width: etapas.width
        spacing: 4

        Repeater {
            id: repetidor
            model: etapas.modelo
            delegate: Row {
                required property var modelData
                required property int index
                spacing: 4
                Rectangle {
                    readonly property bool feita: etapas.indice > index
                    readonly property bool agora: etapas.indice === index
                    radius: height / 2
                    height: 30
                    width: rotulo.implicitWidth + 26
                    color: agora ? Tema.tinta : feita ? Tema.verde : Qt.rgba(1, 1, 1, 0.35)
                    anchors.verticalCenter: parent.verticalCenter
                    Text {
                        id: rotulo
                        anchors.centerIn: parent
                        text: parent.feita ? "✓ " + modelData.nome : modelData.nome
                        color: parent.agora ? Tema.claro : parent.feita ? Tema.verdeEscuro : Tema.apagado
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 13
                    }
                }
                Text {
                    visible: index < etapas.modelo.length - 1
                    text: "›"
                    color: Tema.apagado
                    font.family: Tema.texto
                    font.pixelSize: 18
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
        }
    }
}
