import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Janela para acertar o estoque com a quantidade contada na prateleira.
Popup {
    id: ajuste
    property string produtoId: ""
    property string estoqueAtual: "0"
    property string unidade: "UN"

    signal ajustado(var ficha)

    modal: true
    focus: true
    anchors.centerIn: T.Overlay.overlay
    width: Math.min(520, Tema.largura - 24)
    padding: Tema.estreito ? 18 : 30
    closePolicy: Popup.CloseOnEscape

    T.Overlay.modal: Rectangle { color: Tema.veu }
    background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }

    function abrirCom(id, atual, unidadePeca) {
        produtoId = id
        estoqueAtual = atual
        unidade = unidadePeca
        quantidade.texto = ""
        quantidade.erro = ""
        motivo.texto = ""
        motivo.erro = ""
        open()
    }

    function confirmar() {
        const r = Api.chamar("estoque_ajustar", { produtoId: produtoId, quantidade: quantidade.texto, motivo: motivo.texto })
        if (r.ok) {
            close()
            ajustado(r.dados)
        } else if (r.campo === "motivo") {
            motivo.erro = r.erro
            motivo.focar()
        } else {
            quantidade.erro = r.erro
            quantidade.focar()
        }
    }

    contentItem: ColumnLayout {
        spacing: 16
        Titulo { text: "Ajustar estoque"; font.pixelSize: 32 }
        Nota {
            text: "O sistema mostra " + ajuste.estoqueAtual + " " + ajuste.unidade + ". Digite quanto tem na prateleira agora; fica registrado quem ajustou e por quê."
            font.pixelSize: 16
            Layout.fillWidth: true
        }
        Campo {
            id: quantidade
            rotulo: "Quantidade contada agora"
            sufixo: ajuste.unidade
            numerico: true
            maximo: 14
            tamanhoFonte: 20
            altura: 46
            Layout.preferredWidth: 220
            onConfirmado: motivo.focar()
        }
        Campo {
            id: motivo
            rotulo: "Motivo"
            dica: "Ex.: contagem do mês"
            maximo: 120
            Layout.fillWidth: true
            onConfirmado: ajuste.confirmar()
        }
        Flow {
            spacing: 8
            Layout.fillWidth: true
            Repeater {
                model: ["Contagem", "Avaria ou perda", "Correção de cadastro", "Devolução ao fornecedor"]
                delegate: Botao {
                    required property string modelData
                    text: modelData
                    tipo: "secundario"
                    implicitHeight: 32
                    font.pixelSize: 14
                    onClicked: {
                        motivo.texto = modelData
                        motivo.erro = ""
                        motivo.focar()
                    }
                }
            }
        }
        Linha {
            spacing: 10
            Layout.alignment: Qt.AlignRight
            Layout.topMargin: 6
            Botao {
                text: "Cancelar"
                tipo: "discreto"
                onClicked: ajuste.close()
            }
            Botao {
                text: "Ajustar"
                icone: "check"
                onClicked: ajuste.confirmar()
            }
        }
    }

    onOpened: quantidade.focar()
}
