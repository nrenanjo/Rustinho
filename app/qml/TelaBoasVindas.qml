import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Primeira vez que o Rustinho abre neste computador.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window

    function comecar() {
        for (const c of [loja, caixa, nome, senha, senha2])
            c.erro = ""
        const r = Api.chamar("configurar_inicio", {
            loja: loja.texto,
            caixa: caixa.texto,
            nome: nome.texto,
            senha: senha.texto,
            senha2: senha2.texto
        })
        if (r.ok) {
            janela.entrou()
            return
        }
        const campos = { loja: loja, caixa: caixa, nome: nome, senha: senha, senha2: senha2 }
        const alvo = campos[r.campo]
        if (alvo) {
            alvo.erro = r.erro
            alvo.focarSempre()
        } else {
            janela.avisar(r.erro, "erro")
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
                    text: "Rustinho"
                    font.pixelSize: Tema.estreito ? 60 : 76
                    font.weight: Font.Bold
                    font.italic: true
                }
                Nota {
                    text: "Bem-vindo. Vamos preparar este " + Tema.aparelho + " para a loja."
                    font.pixelSize: 18
                    Layout.fillWidth: true
                }
                Botao {
                    text: Tema.estreito ? "Já uso o Rustinho na loja: juntar" : "Já uso o Rustinho em outro caixa da loja: juntar este " + Tema.aparelho
                    tipo: "discreto"
                    icone: "direita"
                    Layout.leftMargin: -10
                    Layout.topMargin: 6
                    onClicked: tela.janela.trocarPara("TelaJuntar")
                }
            }

            Faixa {
                titulo: "A loja"
                color: Tema.pergaminho
                Layout.fillWidth: true
                Layout.topMargin: 8
                Campo {
                    id: loja
                    rotulo: "Nome da loja"
                    dica: "Ex.: Auto Peças Central"
                    maximo: 80
                    Layout.fillWidth: true
                    onConfirmado: caixa.focar()
                }
                Campo {
                    id: caixa
                    rotulo: "Nome deste " + Tema.aparelho
                    texto: Tema.celular ? "Celular 1" : "Caixa 1"
                    maximo: 30
                    Layout.fillWidth: true
                    onConfirmado: nome.focar()
                }
            }

            Faixa {
                titulo: "Seu usuário"
                subtitulo: "Depois você cadastra os outros."
                color: Tema.azul
                Layout.fillWidth: true
                Campo {
                    id: nome
                    rotulo: "Seu nome"
                    dica: "Como vai aparecer na tela de entrada"
                    maximo: 40
                    Layout.fillWidth: true
                    onConfirmado: senha.focar()
                }
                Linha {
                    spacing: 14
                    Layout.fillWidth: true
                    Campo {
                        id: senha
                        rotulo: "Senha"
                        senha: true
                        dica: "Pelo menos 4 caracteres"
                        maximo: 256
                        Layout.fillWidth: true
                        onConfirmado: senha2.focar()
                    }
                    Campo {
                        id: senha2
                        rotulo: "Repita a senha"
                        senha: true
                        maximo: 256
                        Layout.fillWidth: true
                        onConfirmado: tela.comecar()
                    }
                }
            }

            Botao {
                text: "Começar"
                atalho: "Enter"
                Layout.fillWidth: true
                implicitHeight: 48
                onClicked: tela.comecar()
            }

        }
    }

    StackView.onActivated: loja.focarSempre()
}
