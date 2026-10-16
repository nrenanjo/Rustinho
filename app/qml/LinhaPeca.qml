import QtQuick
import QtQuick.Layouts
import rustinho

// Uma peça na lista de resultados (faixas alternadas, como no livro).
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool atual: false

    signal abrir()

    readonly property bool par: (indice + 1) % 2 === 0
    readonly property color corBase: par ? Tema.azul : Tema.pergaminho
    readonly property color corTexto: atual ? Tema.claro : Tema.tinta
    readonly property color corSuave: atual ? Qt.rgba(0.95, 0.93, 0.88, 0.78) : Tema.tintaSuave

    radius: Tema.raio
    color: atual ? Tema.tinta : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    implicitHeight: Tema.estreito ? compacto.implicitHeight + 20 : Math.max(64, conteudo.implicitHeight + 22)

    // Celular: nome e preço em cima; código, local e estoque embaixo.
    ColumnLayout {
        id: compacto
        visible: Tema.estreito
        x: 14
        width: parent.width - 28
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        opacity: linha.item.ativo ? 1 : 0.6
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Text {
                text: (linha.item.nome || "") + (linha.item.ativo ? "" : "  (inativa)")
                color: linha.corTexto
                font.family: Tema.titulo
                font.weight: Font.Bold
                font.pixelSize: 20
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: (linha.item.oferta ? "oferta  " : "") + (linha.item.preco ? "R$ " + linha.item.preco : "—")
                color: linha.corTexto
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 17
                Layout.alignment: Qt.AlignTop
            }
        }
        Text {
            text: [linha.item.marca, linha.item.codigoOriginal, linha.item.aplicacoes].filter(x => !!x).join("  ·  ")
            visible: text !== ""
            color: linha.corSuave
            font.family: Tema.serifa
            font.italic: true
            font.pixelSize: 13
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        RowLayout {
            spacing: 8
            Layout.fillWidth: true
            Text {
                text: [linha.item.codigo ? "Cód. " + linha.item.codigo : "", linha.item.local].filter(x => !!x).join("  ·  ")
                color: linha.corSuave
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 13
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Balao {
                texto: linha.item.achou || ""
                escuro: linha.atual
            }
            Rectangle {
                implicitWidth: estoqueC.implicitWidth + 16
                implicitHeight: estoqueC.implicitHeight + 6
                radius: height / 2
                color: linha.item.estoqueBaixo && !linha.atual ? Tema.rosa : "transparent"
                Text {
                    id: estoqueC
                    anchors.centerIn: parent
                    text: linha.item.servico ? "serviço" : (linha.item.estoque || "0") + " " + (linha.item.unidade || "")
                    color: linha.item.estoqueBaixo ? (linha.atual ? "#F2B8AE" : Tema.vermelho) : linha.corTexto
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: 14
                }
            }
        }
    }

    RowLayout {
        id: conteudo
        visible: !Tema.estreito
        anchors.fill: parent
        anchors.leftMargin: 18
        anchors.rightMargin: 20
        spacing: 18
        opacity: linha.item.ativo ? 1 : 0.6

        Text {
            text: linha.item.codigo || ""
            color: linha.corSuave
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 15
            elide: Text.ElideRight
            Layout.preferredWidth: 84
        }

        ColumnLayout {
            spacing: 1
            Layout.fillWidth: true
            Item {
                Layout.fillWidth: true
                implicitHeight: nomePeca.implicitHeight
                Text {
                    id: nomePeca
                    width: Math.min(implicitWidth, parent.width - (achou.visible ? achou.implicitWidth + 10 : 0))
                    text: (linha.item.nome || "") + (linha.item.ativo ? "" : "  (inativa)")
                    color: linha.corTexto
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: 22
                    elide: Text.ElideRight
                }
                Balao {
                    id: achou
                    x: nomePeca.width + 10
                    anchors.verticalCenter: nomePeca.verticalCenter
                    texto: linha.item.achou || ""
                    escuro: linha.atual
                }
            }
            Text {
                text: [linha.item.marca, linha.item.codigoOriginal, linha.item.aplicacoes].filter(x => !!x).join("  ·  ")
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
            text: linha.item.local || ""
            color: linha.corSuave
            font.family: Tema.texto
            font.pixelSize: 15
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignLeft
            Layout.preferredWidth: 90
        }

        Item {
            Layout.preferredWidth: 104
            implicitHeight: estoque.implicitHeight + 8
            Rectangle {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: estoque.implicitWidth + 16
                height: estoque.implicitHeight + 6
                radius: height / 2
                color: linha.item.estoqueBaixo && !linha.atual ? Tema.rosa : "transparent"
                Text {
                    id: estoque
                    anchors.centerIn: parent
                    text: linha.item.servico ? "serviço" : (linha.item.estoque || "0") + " " + (linha.item.unidade || "")
                    color: linha.item.estoqueBaixo ? (linha.atual ? "#F2B8AE" : Tema.vermelho) : linha.corTexto
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: 15
                }
            }
        }

        Text {
            text: (linha.item.oferta ? "oferta  " : "") + (linha.item.preco ? "R$ " + linha.item.preco : "—")
            color: linha.corTexto
            font.family: Tema.texto
            font.weight: Font.Bold
            font.pixelSize: 19
            horizontalAlignment: Text.AlignRight
            Layout.preferredWidth: 128
        }
    }

    HoverHandler {
        id: sobre
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler { onTapped: linha.abrir() }
}
