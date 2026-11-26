import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Configurações › Fiscal (0.9.15; FIS-CAT e FIS-P4 na 0.9.19): a emissão de notas pelo
// Rustinho, como está de verdade hoje. A emissão e a integração com um
// emissor fiscal ainda não existem: nada aqui finge estado, documento ou
// pendência. Quando existirem, estes blocos mostram os dados reais.
ColumnLayout {
    id: emissao
    readonly property var janela: ApplicationWindow.window
    // Vêm da tela Fiscal: o seletor do FIS-LIGA, a UF e o CNPJ da loja e a
    // conferência das peças.
    property var liga: null
    property var dadosDaLoja: null
    property var dados: null
    readonly property bool ligado: !!liga && liga.ligado
    readonly property int semNcm: dados ? dados.semNcm.length : 0
    readonly property bool lojaPronta: !!dadosDaLoja && dadosDaLoja.satisfeito
    readonly property var passos: [
        { titulo: "1. Emissor fiscal", situacao: "ainda não disponível", pronto: false,
          explicacao: "A ligação com o emissor (o ACBrMonitorPLUS deste computador) ainda não está pronta nesta versão." },
        { titulo: "2. Certificado A3", situacao: ligado ? "liberado neste computador" : "desligado neste computador", pronto: ligado,
          explicacao: ligado ? "Um gerente liberou o uso do certificado A3 aqui." : "Um gerente liga em \"Certificado A3 e integração fiscal\", acima." },
        { titulo: "3. Testar a conexão", situacao: "depois dos passos 1 e 2", pronto: false,
          explicacao: "O Rustinho confere a ligação com a SEFAZ em homologação, sem nota de verdade." },
        { titulo: "4. Condições para emitir", situacao: lojaPronta && semNcm === 0 ? "cadastro em dia" : "falta acertar", pronto: false,
          explicacao: (lojaPronta ? "UF e CNPJ da loja informados. " : "Falta informar a UF e o CNPJ da loja. ")
                      + (semNcm === 0 ? "Nenhuma peça sem NCM. " : semNcm + (semNcm === 1 ? " peça sem NCM. " : " peças sem NCM. "))
                      + "O regime da loja (Simples Nacional ou outro) fica com o contador." }
    ]
    spacing: 12

    Faixa {
        titulo: "Emissão fiscal"
        color: Tema.verde
        corTitulo: Tema.apagado
        Layout.fillWidth: true
        Nota {
            text: "Configure a integração fiscal para emitir documentos nesta loja."
            Layout.fillWidth: true
            Layout.topMargin: -8
        }
        Flow {
            spacing: 14
            Layout.fillWidth: true
            Repeater {
                model: ["NFC-e · modelo 65", "NF-e · modelo 55"]
                delegate: Rectangle {
                    required property string modelData
                    radius: Tema.raioCampo
                    color: Tema.papel
                    implicitWidth: linhaModelo.implicitWidth + 60
                    implicitHeight: 46
                    Row {
                        id: linhaModelo
                        anchors.centerIn: parent
                        spacing: 8
                        Text {
                            text: parent.parent.modelData
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 16
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            text: "em breve"
                            color: Tema.apagado
                            font.family: Tema.titulo
                            font.italic: true
                            font.pixelSize: 14
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                }
            }
        }
        Nota {
            text: "Por enquanto, as notas continuam no sistema de hoje. O cadastro das peças já guarda o que a nota vai pedir (NCM, CEST, CSOSN, PIS/Cofins, IPI, ANP e ISS)."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
    }

    // FIS-P4 (0.9.19, etapa 3; texto A §15): o que falta para emitir, numa
    // sequência só (emissor → certificado → testar → condições), cada passo
    // com a situação de verdade. Sem botão que não leva a lugar nenhum.
    Faixa {
        titulo: "Para emitir notas pelo Rustinho"
        color: Tema.azul
        Layout.fillWidth: true
        espacamento: 10
        Repeater {
            model: emissao.passos
            delegate: RowLayout {
                id: passo
                required property var modelData
                required property int index
                spacing: 14
                Layout.fillWidth: true
                Rectangle {
                    implicitWidth: 34
                    implicitHeight: 34
                    radius: 17
                    color: passo.modelData.pronto ? Tema.verde : Tema.papel
                    Layout.alignment: Qt.AlignTop
                    Text {
                        anchors.centerIn: parent
                        text: passo.modelData.pronto ? "✓" : String(passo.index + 1)
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 16
                    }
                }
                ColumnLayout {
                    spacing: 2
                    Layout.fillWidth: true
                    Text {
                        text: passo.modelData.titulo + " · " + passo.modelData.situacao
                        textFormat: Text.PlainText
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 16
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Nota {
                        text: passo.modelData.explicacao
                        font.pixelSize: 14
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    // As pendências (sem resposta, contingência) ficam no painel das notas,
    // logo abaixo (FIS-P1): um lugar só, sem dois cartões dizendo coisas diferentes.

}
