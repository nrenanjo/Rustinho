import QtQuick
import QtQuick.Layouts
import rustinho

// Uma compra na lista.
Rectangle {
    id: linha
    property var item: ({})
    property int indice: 0
    property bool atual: false
    property bool temAcaoConferir: false

    signal abrir()
    signal conferir()

    readonly property bool rascunho: item.situacao === "rascunho"
    readonly property bool bloqueada: rascunho && item.autorizacao === "negada"
    readonly property bool aguardandoConferencia: rascunho && item.podeLancar === false && !bloqueada
    readonly property bool par: (indice + 1) % 2 === 0
    // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
    readonly property color corBase: Tema.faixa(par)
    readonly property color corTexto: atual ? Tema.tintaAtual : Tema.tintaDaFaixa(par)
    readonly property color corSuave: atual ? Tema.claroSuave(0.78) : Tema.tintaSuaveDaFaixa(par)
    readonly property string textoSituacao: {
        switch (item.situacao) {
        case "rascunho":
            if (bloqueada)
                return "Nota bloqueada"
            if (aguardandoConferencia)
                return "Aguardando conferência"
            return item.pendentes > 0
                   ? item.pendentes + (item.pendentes === 1 ? " item a decidir" : " itens a decidir")
                   : "Pronta para lançar"
        case "lancada": return "Lançada"
        default: return "Estornada"
        }
    }
    readonly property color corFundoChip: {
        if (atual)
            return Tema.chipAtual
        if (rascunho) {
            if (bloqueada || item.pendentes > 0 && !aguardandoConferencia)
                return Tema.rosa
            if (aguardandoConferencia)
                return Tema.amarelo
            return Tema.verde
        }
        return item.situacao === "lancada" ? Tema.verde : Tema.roxo ? Tema.papel : Tema.sombra(0.08)
    }
    readonly property color corTextoChip: {
        if (atual)
            return Tema.tintaAtual
        if (rascunho) {
            if (bloqueada || item.pendentes > 0 && !aguardandoConferencia)
                return Tema.vermelho
            if (aguardandoConferencia)
                return Tema.tinta
            return Tema.verdeEscuro
        }
        return item.situacao === "lancada" ? Tema.verdeEscuro : Tema.tintaSuave
    }

    radius: Tema.raio
    color: atual ? Tema.atual : sobre.hovered ? Qt.darker(corBase, 1.04) : corBase
    palette.window: Tema.superficie(color)
    implicitHeight: Math.max(64, conteudo.implicitHeight + 22)

    ColumnLayout {
        id: conteudo
        anchors.fill: parent
        anchors.leftMargin: Tema.estreito ? 14 : 18
        anchors.rightMargin: Tema.estreito ? 14 : 20
        anchors.topMargin: 11
        anchors.bottomMargin: 11
        spacing: 8
        opacity: linha.item.situacao === "estornada" ? 0.55 : 1

        RowLayout {
            Layout.fillWidth: true
            spacing: Tema.estreito ? 10 : 18

            Text {
                text: linha.item.emissao || ""
                textFormat: Text.PlainText
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
                    textFormat: Text.PlainText
                    color: linha.corTexto
                    font.family: Tema.fonteLinha
                    font.weight: Font.Bold
                    font.pixelSize: Tema.estreito ? 18 : 22
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    text: (Tema.estreito && linha.item.emissao ? linha.item.emissao + "  ·  " : "")
                          + "NF " + (linha.item.numero || "—")
                          + "  ·  " + linha.item.itens + (linha.item.itens === 1 ? " item" : " itens")
                          + (linha.aguardandoConferencia && linha.item.pendentes > 0
                             ? "  ·  " + linha.item.pendentes + (linha.item.pendentes === 1 ? " item a decidir" : " itens a decidir")
                             : "")
                    textFormat: Text.PlainText
                    color: linha.corSuave
                    font.family: Tema.serifa
                    font.italic: true
                    font.pixelSize: 14
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            Item {
                visible: !Tema.estreito
                Layout.preferredWidth: 190
                implicitHeight: chipDesktop.height
                Rectangle {
                    id: chipDesktop
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: textoChipDesktop.implicitWidth + 20
                    height: textoChipDesktop.implicitHeight + 8
                    radius: height / 2
                    color: linha.corFundoChip
                    Text {
                        id: textoChipDesktop
                        anchors.centerIn: parent
                        text: linha.textoSituacao
                        textFormat: Text.PlainText
                        color: linha.corTextoChip
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 13
                    }
                }
            }

            Text {
                text: "R$ " + Tema.valor(linha.item.total || "0,00")
                textFormat: Text.PlainText
                color: linha.corTexto
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: Tema.estreito ? 17 : 19
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: Tema.estreito ? -1 : 130
            }

            Item {
                visible: !Tema.estreito && linha.temAcaoConferir
                Layout.preferredWidth: Math.max(108, btnConferirDesktop.implicitWidth)
                implicitHeight: btnConferirDesktop.implicitHeight
                Botao {
                    id: btnConferirDesktop
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    visible: linha.aguardandoConferencia
                    text: "Conferir"
                    tipo: "secundario"
                    onClicked: linha.conferir()
                }
            }
        }

        RowLayout {
            visible: Tema.estreito
            Layout.fillWidth: true
            spacing: 10
            Rectangle {
                width: textoChipCelular.implicitWidth + 20
                height: textoChipCelular.implicitHeight + 8
                radius: height / 2
                color: linha.corFundoChip
                Text {
                    id: textoChipCelular
                    anchors.centerIn: parent
                    text: linha.textoSituacao
                    textFormat: Text.PlainText
                    color: linha.corTextoChip
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: 13
                }
            }
            Item { Layout.fillWidth: true }
            Botao {
                visible: linha.aguardandoConferencia
                text: "Conferir"
                tipo: "secundario"
                onClicked: linha.conferir()
            }
        }
    }

    HoverHandler {
        id: sobre
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler { onTapped: linha.abrir() }
}
