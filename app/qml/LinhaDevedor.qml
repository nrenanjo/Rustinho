import QtQuick
import QtQuick.Layouts
import rustinho

// Um cliente que deve, na lista da tela Receber.
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool atual: false

    signal abrir()

    readonly property bool atrasado: item.vencido !== "0,00"
    readonly property color corBase: (indice + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
    readonly property color corTexto: atual ? Tema.claro : Tema.tinta
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuave

    radius: Tema.raio
    color: atual ? Tema.tinta : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    implicitHeight: 64

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 12 : 18
        anchors.rightMargin: Tema.estreito ? 12 : 18
        spacing: 12

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Text {
                text: linha.item.nome
                color: linha.corTexto
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 20
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: [linha.item.parcelas + (linha.item.parcelas === 1 ? " parcela" : " parcelas"),
                       (linha.atrasado ? "venceu " : "vence ") + linha.item.proximoVencimento,
                       linha.item.telefone].filter(x => !!x).join("  ·  ")
                color: linha.corSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 13
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
        ColumnLayout {
            spacing: 0
            Text {
                text: Tema.sigiloTexto("R$ " + linha.item.emAberto)
                color: linha.corTexto
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 17
                Layout.alignment: Qt.AlignRight
            }
            Text {
                visible: linha.atrasado
                text: linha.item.diasAtraso + (linha.item.diasAtraso === 1 ? " dia" : " dias") + " de atraso"
                color: linha.atual ? Tema.vermelhoSobre : Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 13
                Layout.alignment: Qt.AlignRight
            }
        }
    }

    HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: linha.abrir() }
}
