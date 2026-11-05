import QtQuick
import QtQuick.Layouts
import rustinho

// Um dia do fluxo de caixa: entradas e saídas, com barrinhas.
Rectangle {
    id: linha
    property var dia: ({})
    property int indice: 0
    property bool mostrarSaldo: false

    radius: Tema.raioCampo
    // Faixas alternadas (V-6); "hoje" fica destacado no papel (claro).
    readonly property bool par: (indice + 1) % 2 === 0
    readonly property bool escura: !dia.hoje && par
    color: dia.hoje ? Tema.papel : Tema.faixa(par)
    palette.window: Tema.superficie(color)
    implicitHeight: coluna.implicitHeight + 16

    ColumnLayout {
        id: coluna
        x: 14
        y: 8
        width: parent.width - 28
        spacing: 4
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Text {
                text: linha.dia.data
                color: Tema.tintaDaFaixa(linha.escura)
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 14
                Layout.preferredWidth: Tema.estreito ? 57 : 82
            }
            Text {
                text: linha.dia.hoje ? "hoje" : linha.dia.diaSemana
                color: Tema.apagadoDaFaixa(linha.escura)
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 13
                Layout.preferredWidth: 36
            }
            // barrinhas
            Item {
                id: barras
                Layout.fillWidth: true
                implicitHeight: 15
                Rectangle {
                    visible: linha.dia.entradas !== "0,00"
                    y: 0
                    radius: 3
                    height: 6
                    width: Math.max(6, barras.width * linha.dia.barraEntrada)
                    color: Tema.verdeEscuroDaFaixa(linha.escura)
                    opacity: 0.55
                }
                Rectangle {
                    visible: linha.dia.saidas !== "0,00"
                    y: 9
                    radius: 3
                    height: 6
                    width: Math.max(6, barras.width * linha.dia.barraSaida)
                    color: Tema.vermelhoDaFaixa(linha.escura)
                    opacity: 0.5
                }
            }
            Text {
                text: linha.dia.entradas !== "0,00" ? "+ " + Tema.valor(linha.dia.entradas) : ""
                color: Tema.verdeEscuroDaFaixa(linha.escura)
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 14
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: Tema.estreito ? 67 : 96
            }
            Text {
                text: linha.dia.saidas !== "0,00" ? "− " + Tema.valor(linha.dia.saidas) : ""
                color: Tema.vermelhoDaFaixa(linha.escura)
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 14
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: Tema.estreito ? 67 : 96
            }
            Text {
                visible: linha.mostrarSaldo
                text: "R$ " + Tema.valor(linha.dia.saldo)
                color: linha.dia.negativo ? Tema.vermelhoDaFaixa(linha.escura) : Tema.tintaDaFaixa(linha.escura)
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 14
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: Tema.estreito ? 73 : 104
            }
        }
        Text {
            text: (linha.dia.detalhes || []).join("  ·  ")
            visible: text !== ""
            color: Tema.tintaSuaveDaFaixa(linha.escura)
            font.family: Tema.serifa
            font.italic: true
            font.pixelSize: 12
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.leftMargin: 128
        }
    }
}
