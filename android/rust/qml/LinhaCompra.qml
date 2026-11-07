import QtQuick
import QtQuick.Layouts
import rustinho

// Uma compra na lista.
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool atual: false

    signal abrir()

    readonly property bool par: (indice + 1) % 2 === 0
    // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
    readonly property color corBase: Tema.faixa(par)
    readonly property color corTexto: atual ? Tema.tintaAtual : Tema.tintaDaFaixa(par)
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuaveDaFaixa(par)

    radius: Tema.raio
    color: atual ? Tema.atual : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    palette.window: Tema.superficie(color)
    implicitHeight: Math.max(64, conteudo.implicitHeight + 22)

    RowLayout {
        id: conteudo
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 20
        spacing: 18
        opacity: linha.item.situacao === "estornada" ? 0.55 : 1

        Text {
            text: linha.item.emissao || ""
            color: linha.corSuave
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 15
            visible: !Tema.estreito
            Layout.preferredWidth: 96
        }

        ColumnLayout {
            spacing: 1
            Layout.fillWidth: true
            Text {
                text: linha.item.fornecedor || "Fornecedor não identificado"
                color: linha.corTexto
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 22
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: (Tema.estreito && linha.item.emissao ? linha.item.emissao + "  ·  " : "") + "NF " + (linha.item.numero || "—") + "  ·  " + linha.item.itens + (linha.item.itens === 1 ? " item" : " itens")
                color: linha.corSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 14
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Item {
            Layout.preferredWidth: Tema.estreito ? chip.width : 170
            implicitHeight: chip.height
            Rectangle {
                id: chip
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: textoChip.implicitWidth + 20
                height: textoChip.implicitHeight + 8
                radius: height / 2
                color: {
                    if (linha.atual)
                        return Tema.chipAtual
                    switch (linha.item.situacao) {
                    case "rascunho": return Tema.rosa
                    case "lancada": return Tema.verde
                    default: return Tema.roxo ? Tema.papel : Tema.sombra(0.08)
                    }
                }
                Text {
                    id: textoChip
                    anchors.centerIn: parent
                    text: {
                        switch (linha.item.situacao) {
                        case "rascunho": return linha.item.pendentes > 0 ? linha.item.pendentes + (linha.item.pendentes === 1 ? " item a decidir" : " itens a decidir") : "Pronta para lançar"
                        case "lancada": return "Lançada"
                        default: return "Estornada"
                        }
                    }
                    color: linha.atual ? Tema.tintaAtual : linha.item.situacao === "rascunho" ? Tema.vermelho : linha.item.situacao === "lancada" ? Tema.verdeEscuro : Tema.tintaSuave
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: 13
                }
            }
        }

        Text {
            text: "R$ " + Tema.valor(linha.item.total || "0,00")
            color: linha.corTexto
            font.family: Tema.texto
            font.weight: Font.Bold
            font.pixelSize: 19
            horizontalAlignment: Text.AlignRight
            Layout.preferredWidth: Tema.estreito ? -1 : 140
        }
    }

    HoverHandler {
        id: sobre
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler { onTapped: linha.abrir() }
}
