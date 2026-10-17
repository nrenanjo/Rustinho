import QtQuick
import QtQuick.Layouts
import rustinho

// Um cliente ou fornecedor na lista (faixas alternadas).
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool atual: false

    signal abrir()

    readonly property bool par: (indice + 1) % 2 === 0
    readonly property color corBase: par ? Tema.azul : Tema.pergaminho
    readonly property color corTexto: atual ? Tema.claro : Tema.tinta
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuave

    radius: Tema.raio
    color: atual ? Tema.tinta : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    implicitHeight: Math.max(64, conteudo.implicitHeight + 22)

    RowLayout {
        id: conteudo
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 14 : 18
        anchors.rightMargin: Tema.estreito ? 14 : 20
        spacing: Tema.estreito ? 10 : 18
        opacity: linha.item.ativo ? 1 : 0.6

        Text {
            text: linha.item.codigo || ""
            color: linha.corSuave
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 15
            elide: Text.ElideRight
            visible: !Tema.estreito
            Layout.preferredWidth: 64
        }

        ColumnLayout {
            spacing: 1
            Layout.fillWidth: true
            Text {
                text: (linha.item.nome || "") + (linha.item.ativo ? "" : "  (inativo)")
                color: linha.corTexto
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: Tema.estreito ? 20 : 22
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: {
                    const partes = [linha.item.fantasia, linha.item.cidade, linha.item.veiculos]
                    if (Tema.estreito)
                        partes.unshift(linha.item.telefone)
                    if (linha.item.cliente && linha.item.fornecedor)
                        partes.push("cliente e fornecedor")
                    return partes.filter(x => !!x).join("  ·  ")
                }
                visible: text !== ""
                color: linha.corSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 14
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Text {
            text: Tema.sigilo(linha.item.documento)
            color: linha.corSuave
            font.family: Tema.texto
            font.pixelSize: 15
            elide: Text.ElideRight
            visible: !Tema.estreito
            Layout.preferredWidth: 190
        }

        Text {
            text: linha.item.telefone || ""
            color: linha.corTexto
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 15
            horizontalAlignment: Text.AlignRight
            elide: Text.ElideRight
            visible: !Tema.estreito
            Layout.preferredWidth: 150
        }
    }

    HoverHandler {
        id: sobre
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler { onTapped: linha.abrir() }
}
