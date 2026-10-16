import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Primeira vez num caixa novo: em vez de começar do zero, recebe tudo de
// outro caixa da loja (usuários, peças, clientes, estoque...).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var sync: null
    property bool procurando: false

    function juntar() {
        for (const c of [caixa, senha, enderecos])
            c.erro = ""
        const r = Api.chamar("sync_juntar", { caixa: caixa.texto, senha: senha.texto, enderecos: enderecos.texto })
        if (r.ok) {
            sync = r.dados
            procurando = true
        } else {
            const alvo = { caixa: caixa, senha: senha, enderecos: enderecos }[r.campo] || senha
            alvo.erro = r.erro
            alvo.focarSempre()
        }
    }

    Timer {
        interval: 2000
        repeat: true
        running: tela.procurando
        onTriggered: {
            const r = Api.chamar("sync_ler")
            if (!r.ok)
                return
            tela.sync = r.dados
            if (r.dados.usuarios > 0) {
                tela.procurando = false
                tela.janela.atualizarEstado()
                tela.janela.avisar("Pronto: os dados da loja chegaram.", "ok")
                tela.janela.trocarPara("TelaEntrar")
            }
        }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: Math.max(height, coluna.implicitHeight + 120)
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: coluna
            width: Math.min(620, parent.width - 2 * Tema.margem)
            x: (parent.width - width) / 2
            y: Math.max(48, (tela.height - implicitHeight) / 2 - 20)
            spacing: 18

            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Titulo {
                    text: "Juntar este caixa"
                    font.pixelSize: Tema.estreito ? 42 : 56
                    Layout.fillWidth: true
                }
                Nota {
                    text: "Este " + Tema.aparelho + " vai receber tudo do outro caixa: usuários, peças, clientes e estoque. Os dois precisam estar no mesmo Wi-Fi, com o Rustinho aberto."
                    font.pixelSize: 17
                    Layout.fillWidth: true
                }
            }

            Faixa {
                titulo: "Este " + Tema.aparelho
                color: Tema.pergaminho
                Layout.fillWidth: true
                Campo {
                    id: caixa
                    rotulo: "Nome deste " + Tema.aparelho
                    texto: Tema.celular ? "Celular 1" : "Caixa 2"
                    maximo: 30
                    Layout.fillWidth: true
                    onConfirmado: senha.focar()
                }
                Campo {
                    id: senha
                    rotulo: "Senha da rede da loja"
                    dica: "A mesma que está nas Configurações do outro caixa"
                    senha: true
                    maximo: 128
                    Layout.fillWidth: true
                    onConfirmado: tela.juntar()
                }
                Campo {
                    id: enderecos
                    rotulo: "Endereço do outro caixa (só se não achar sozinho)"
                    dica: "Ex.: 192.168.0.12 (aparece em Configurações do outro caixa)"
                    maximo: 300
                    Layout.fillWidth: true
                    onConfirmado: tela.juntar()
                }
            }

            Faixa {
                visible: tela.procurando
                titulo: "Procurando os outros caixas"
                color: Tema.azul
                Layout.fillWidth: true
                Nota {
                    text: !tela.sync || tela.sync.pares.length === 0
                          ? "Ainda não encontrei nenhum. Confira se o outro caixa está com a sincronização ligada e a mesma senha. Endereço deste " + Tema.aparelho + ": " + (tela.sync ? tela.sync.meuEndereco : "")
                          : "Recebendo os dados…"
                    Layout.fillWidth: true
                }
                Repeater {
                    model: tela.sync ? tela.sync.pares : []
                    delegate: Text {
                        required property var modelData
                        text: (modelData.nome || "Caixa") + " (" + modelData.endereco + ")" + (modelData.erro ? " · " + modelData.erro : "")
                        color: modelData.erro ? Tema.vermelho : Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 15
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
            }

            Linha {
                spacing: 12
                Layout.fillWidth: true
                Botao {
                    text: "Voltar"
                    tipo: "discreto"
                    onClicked: tela.janela.trocarPara("TelaBoasVindas")
                }
                Item { Layout.fillWidth: true }
                Botao {
                    text: tela.procurando ? "Procurar de novo" : "Procurar os outros caixas"
                    icone: "busca"
                    onClicked: tela.juntar()
                }
            }
        }
    }

    StackView.onActivated: senha.focarSempre()
}
