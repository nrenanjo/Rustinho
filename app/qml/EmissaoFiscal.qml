import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Configurações › Simultaneidade › Fiscal (0.9.15): a emissão de notas pelo
// Rustinho, como está de verdade hoje. A emissão e a integração com um
// emissor fiscal ainda não existem: nada aqui finge estado, documento ou
// pendência. Quando existirem, estes blocos mostram os dados reais.
ColumnLayout {
    id: emissao
    readonly property var janela: ApplicationWindow.window
    spacing: 12

    // Ícone grande num círculo (os blocos vazios).
    component Selo: Rectangle {
        property string icone: ""
        implicitWidth: 92
        implicitHeight: 92
        radius: 46
        color: Tema.sombra(0.07)
        Layout.alignment: Qt.AlignHCenter
        Icone { anchors.centerIn: parent; nome: parent.icone; tamanho: 62; espessura: 1.6; cor: Tema.tinta }
    }

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

    Faixa {
        color: Tema.azul
        Layout.fillWidth: true
        espacamento: 8
        Selo { icone: "documentoElo" }
        Titulo {
            text: "Integração fiscal ainda não configurada"
            font.pixelSize: Tema.estreito ? 24 : 30
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            Layout.topMargin: 6
        }
        Nota {
            text: "Configure um emissor fiscal compatível para habilitar a emissão de documentos."
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
        Botao {
            text: "Ver opções de integração"
            icone: "engrenagem"
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 6
            onClicked: emissao.janela.avisar("A integração com um emissor fiscal ainda está em planejamento e chega numa próxima versão. "
                                             + "Por enquanto o Rustinho não emite notas.", "info")
        }
    }

    Faixa {
        titulo: "Contingência e pendências"
        color: Tema.pergaminho
        Layout.fillWidth: true
        espacamento: 8
        Selo { icone: "documentoRelogio"; Layout.topMargin: -14 }
        Text {
            text: "Nenhuma pendência fiscal"
            color: Tema.tinta
            font.family: Tema.titulo
            font.weight: Font.DemiBold
            font.pixelSize: Tema.estreito ? 21 : 25
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
        Nota {
            text: "Documentos em contingência e transmissões pendentes aparecerão aqui."
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
    }

    Faixa {
        titulo: "Estado da integração"
        color: Tema.azul
        Layout.fillWidth: true
        Linha {
            spacing: 18
            Layout.fillWidth: true
            Rectangle {
                radius: Tema.raioCampo
                color: Tema.papel
                implicitWidth: estado.implicitWidth + 40
                implicitHeight: 44
                Row {
                    id: estado
                    anchors.centerIn: parent
                    spacing: 10
                    Rectangle { width: 14; height: 14; radius: 7; color: Tema.apagado; anchors.verticalCenter: parent.verticalCenter }
                    Text {
                        text: "Não configurada"
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 16
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }
            Rectangle {
                visible: !Tema.fino
                implicitWidth: 1
                implicitHeight: 34
                color: Tema.sombra(0.25)
                Layout.alignment: Qt.AlignVCenter
            }
            Nota {
                text: "A emissão fiscal ainda não está habilitada neste caixa."
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }
}
