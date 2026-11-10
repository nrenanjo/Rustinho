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
    // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
    readonly property color corBase: Tema.faixa(par)
    readonly property color corTexto: atual ? Tema.tintaAtual : Tema.tintaDaFaixa(par)
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuaveDaFaixa(par)

    // Código do fornecedor como identificação principal da primeira coluna (dev.3.2):
    // quando não houver código de fornecedor associado, exibe um traço ("—"), nunca o código interno.
    readonly property string codigoFornecedorExibido: (linha.item && linha.item.codigoFornecedor) ? linha.item.codigoFornecedor : "—"

    // Texto do balão de identificação (Seção 5.2):
    // Quando a busca encontra a peça por código de fornecedor, formata:
    // "Fornecedor · CÓDIGO · NOME DO FORNECEDOR"
    // Mantém os demais rótulos inalterados ("Cód. barras", "Cód. original", "Cód. antigo", "Similar").
    readonly property string textoAchou: {
        if (!linha.item || !linha.item.achou)
            return ""
        if (linha.item.achou.indexOf("Fornecedor") === 0) {
            const cod = linha.item.codigoAchado || linha.item.codigoFornecedor || ""
            const forn = linha.item.fornecedorNome || ""
            if (cod !== "" && forn !== "")
                return "Fornecedor · " + cod + " · " + forn
            if (cod !== "")
                return "Fornecedor · " + cod
        }
        return linha.item.achou
    }

    radius: Tema.raio
    color: atual ? Tema.atual : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    palette.window: Tema.superficie(color)
    implicitHeight: Tema.estreito ? compacto.implicitHeight + 20 : Math.max(64, conteudo.implicitHeight + 22)

    // Celular: nome e preço em cima; código do fornecedor, local e estoque embaixo.
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
                font.family: Tema.fonteLinha
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
                text: ["Cód. forn.: " + linha.codigoFornecedorExibido, linha.item.local].filter(x => !!x).join("  ·  ")
                color: linha.corSuave
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 13
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Balao {
                texto: linha.textoAchou
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
                    color: linha.item.estoqueBaixo ? (linha.atual ? Tema.vermelhoSobre : Tema.vermelho) : linha.corTexto
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

        // Coluna 1: CÓD. FORNECEDOR em uma única linha, largura 84 px (idêntico à dev.2)
        Text {
            text: linha.codigoFornecedorExibido
            color: linha.corSuave
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 15
            elide: Text.ElideRight
            Layout.preferredWidth: 84
        }

        // Coluna 2: Descrição da Peça e Balão de Identificação
        ColumnLayout {
            spacing: 1
            Layout.fillWidth: true
            Item {
                Layout.fillWidth: true
                implicitHeight: Math.max(nomePeca.implicitHeight, achou.implicitHeight)
                implicitWidth: nomePeca.implicitWidth + (achou.visible ? achou.implicitWidth + 10 : 0)
                Text {
                    id: nomePeca
                    objectName: "nomePeca"
                    width: Math.min(implicitWidth, parent.width - (achou.visible ? achou.implicitWidth + 10 : 0))
                    text: (linha.item.nome || "") + (linha.item.ativo ? "" : "  (inativa)")
                    color: linha.corTexto
                    font.family: Tema.fonteLinha
                    font.weight: Font.Bold
                    font.pixelSize: 22
                    elide: Text.ElideRight
                }
                Balao {
                    id: achou
                    x: nomePeca.width + 10
                    anchors.verticalCenter: nomePeca.verticalCenter
                    larguraMaxima: 360
                    texto: linha.textoAchou
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
                    color: linha.item.estoqueBaixo ? (linha.atual ? Tema.vermelhoSobre : Tema.vermelho) : linha.corTexto
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
