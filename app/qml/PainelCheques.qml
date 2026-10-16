import QtQuick
import QtQuick.Layouts
import rustinho

// Dados dos cheques (na venda e no Receber): quantos, bom para, banco e
// número. Os seguintes vêm um mês depois do outro, com o número em sequência.
Rectangle {
    id: painel
    property var calc: null          // o "cheques" que volta do cálculo
    signal mudou()

    function dados() {
        return { quantidade: parseInt(quantidade.texto) || 1, primeiroBomPara: bomPara.texto,
                 banco: banco.texto, numero: numero.texto, emitente: "" }
    }
    function limpar() {
        quantidade.texto = "1"
        bomPara.texto = ""
        banco.texto = ""
        numero.texto = ""
        prazo = "vista"
    }
    property string prazo: "vista"
    function usarPrazo(id) {
        prazo = id
        const n = { vista: 1, "30": 1, "60": 2, "90": 3 }[id]
        quantidade.texto = String(n)
        if (id === "vista") {
            bomPara.texto = ""
        } else {
            const d = new Date()
            d.setDate(d.getDate() + 30)
            bomPara.texto = Qt.formatDate(d, "dd/MM/yyyy")
        }
        mudou()
    }

    visible: !!calc && calc.valor !== "0,00"
    radius: Tema.raioCampo
    color: Tema.azul
    implicitHeight: coluna.implicitHeight + 24

    ColumnLayout {
        id: coluna
        x: 14
        y: 12
        width: parent.width - 28
        spacing: 8
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Rotulo { text: "Cheque"; Layout.fillWidth: true }
            Pilulas {
                pequenas: true
                modelo: [{ id: "vista", nome: "À vista" }, { id: "30", nome: "30" }, { id: "60", nome: "30/60" }, { id: "90", nome: "30/60/90" }]
                atual: painel.prazo
                onEscolhido: id => painel.usarPrazo(id)
            }
        }
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Campo {
                id: numero
                rotulo: "Nº do 1º cheque"
                maximo: 20
                Layout.fillWidth: true
                onEditado: painel.mudou()
            }
            Campo {
                id: banco
                rotulo: "Banco"
                dica: "opcional"
                maximo: 40
                Layout.fillWidth: true
                onEditado: painel.mudou()
            }
            Campo {
                id: quantidade
                rotulo: "Cheques"
                sufixo: "×"
                numerico: true
                maximo: 2
                texto: "1"
                Layout.preferredWidth: 90
                onEditado: { painel.prazo = ""; painel.mudou() }
            }
            Campo {
                id: bomPara
                rotulo: "Bom para (1º)"
                dica: "hoje"
                maximo: 10
                Layout.preferredWidth: 130
                onEditado: { painel.prazo = ""; painel.mudou() }
            }
        }
        Flow {
            spacing: 14
            visible: !!painel.calc && painel.calc.cheques.length > 1
            Layout.fillWidth: true
            Repeater {
                model: painel.calc ? painel.calc.cheques : []
                delegate: Text {
                    required property var modelData
                    text: (modelData.numero ? "nº " + modelData.numero + "  " : "") + modelData.bomPara + "  R$ " + modelData.valor
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.pixelSize: 14
                }
            }
        }
        Text {
            visible: text !== ""
            text: painel.calc && painel.calc.erro && painel.calc.campo !== "chequeNumero" ? painel.calc.erro : ""
            color: Tema.vermelho
            font.family: Tema.texto
            font.weight: Font.DemiBold
            font.pixelSize: 14
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }
}
