import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Os lançamentos que compõem um total da DRE (rastreabilidade): de onde veio
// cada valor. Uso: detalhe.abrir("dre_detalhe", { ...filtro, linha: "cmv" })
Painel {
    id: janelinha
    property var d: null
    property string erro: ""
    width: Math.min(820, Tema.largura - 24)
    height: Math.min(640, Tema.altura - 40)

    function abrir(comando, dados) {
        const r = Api.chamar(comando, dados)
        d = r.ok ? r.dados : null
        erro = r.ok ? "" : r.erro
        open()
    }

    contentItem: ColumnLayout {
        spacing: 12
        Titulo {
            text: janelinha.d ? (janelinha.d.nome || "Lançamentos").replace(/^\(.\) |^= /, "") : "Lançamentos"
            font.pixelSize: 28
            Layout.fillWidth: true
        }
        Nota {
            visible: !!janelinha.d
            text: janelinha.d ? janelinha.d.quantidade + (janelinha.d.quantidade === 1 ? " lançamento" : " lançamentos")
                                + "  ·  total R$ " + Tema.valor(janelinha.d.total) : ""
            font.pixelSize: 15
            Layout.fillWidth: true
        }
        TextoErro { text: janelinha.erro }
        ListView {
            id: lista
            clip: true
            spacing: 2
            model: janelinha.d ? (janelinha.d.itens || janelinha.d.lancamentos || []) : []
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: registro
                required property var modelData
                required property int index
                width: lista.width - 12
                implicitHeight: linha.implicitHeight + 14
                radius: Tema.raioCampo
                color: registro.index % 2 === 0 ? Tema.papel : "transparent"
                RowLayout {
                    id: linha
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12
                    Text {
                        text: registro.modelData.data
                        color: Tema.apagado
                        font.family: Tema.texto
                        font.pixelSize: 13
                        Layout.preferredWidth: 130
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: registro.modelData.descricao
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 14
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Text {
                            readonly property var m: registro.modelData
                            visible: text !== ""
                            text: [m.categoria, m.fornecedor, m.semCompetencia ? "sem mês de referência (usado o vencimento)" : ""]
                                  .filter(x => !!x).join("  ·  ")
                            color: m.semCompetencia ? Tema.vermelho : Tema.apagado
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 12
                            Layout.fillWidth: true
                        }
                    }
                    Text {
                        text: "R$ " + Tema.valor(registro.modelData.valor)
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 14
                        horizontalAlignment: Text.AlignRight
                        Layout.minimumWidth: 100
                    }
                }
            }
        }
        Nota {
            visible: !!janelinha.d && !!janelinha.d.cortado
            text: "A lista mostra os 500 primeiros. O total vale para todos."
            font.pixelSize: 13
            Layout.fillWidth: true
        }
        Botao {
            text: "Fechar"
            tipo: "discreto"
            Layout.alignment: Qt.AlignRight
            onClicked: janelinha.close()
        }
    }
}
