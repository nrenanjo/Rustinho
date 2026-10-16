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
    color: dia.hoje ? Tema.papel : (indice + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
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
                color: Tema.tinta
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 14
                Layout.preferredWidth: 82
            }
            Text {
                text: linha.dia.hoje ? "hoje" : linha.dia.diaSemana
                color: Tema.apagado
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
                    color: Tema.verdeEscuro
                    opacity: 0.55
                }
                Rectangle {
                    visible: linha.dia.saidas !== "0,00"
                    y: 9
                    radius: 3
                    height: 6
                    width: Math.max(6, barras.width * linha.dia.barraSaida)
                    color: Tema.vermelho
                    opacity: 0.5
                }
            }
            Text {
                text: linha.dia.entradas !== "0,00" ? "+ " + Tema.valor(linha.dia.entradas) : ""
                color: Tema.verdeEscuro
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 14
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 96
            }
            Text {
                text: linha.dia.saidas !== "0,00" ? "− " + Tema.valor(linha.dia.saidas) : ""
                color: Tema.vermelho
                font.family: Tema.texto
                font.weight: Font.DemiBold
                font.pixelSize: 14
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 96
            }
            Text {
                visible: linha.mostrarSaldo
                text: "R$ " + Tema.valor(linha.dia.saldo)
                color: linha.dia.negativo ? Tema.vermelho : Tema.tinta
                font.family: Tema.texto
                font.weight: Font.Bold
                font.pixelSize: 14
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 104
            }
        }
        Text {
            text: (linha.dia.detalhes || []).join("  ·  ")
            visible: text !== ""
            color: Tema.tintaSuave
            font.family: Tema.serifa
            font.italic: true
            font.pixelSize: 12
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.leftMargin: 128
        }
    }
}
