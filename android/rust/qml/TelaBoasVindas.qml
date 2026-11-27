import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import rustinho

// Primeira vez que o Rustinho abre neste computador.
FocusScope {
    id: tela
    // Como esta tela aparece na orientação da navegação guiada ("Depois o
    // Rustinho volta para …").
    readonly property string nomeNaNavegacao: "Boas-vindas"
    readonly property var janela: ApplicationWindow.window
    // E7-7 (PEND-30 (a), decisão do proprietário de 26/11/2026): com o banco vazio e cópias da loja na pasta de backup
    // deste computador (o Rustinho reinstalado), oferece restaurar antes de criar uma loja nova.
    property var copias: []
    // B7 da R1 da 008: a cópia escolhida (a mais nova; dá para escolher outra se ela não abrir).
    property int escolhida: 0
    // B5 da R2 da 008: sem o Google Drive na Beta, o pen drive é a recuperação de um computador perdido. A pasta
    // escolhida pelo seletor do sistema ("" = a pasta de backup padrão deste computador).
    property string pastaDasCopias: ""

    function procurarEmOutraPasta(url) {
        const pasta = Api.caminho(url.toString())
        const r = Api.chamar("inicio_copias", { pasta: pasta })
        if (!r.ok || !Array.isArray(r.dados) || r.dados.length === 0) {
            janela.avisar("Nenhuma cópia do Rustinho nessa pasta.", "erro")
            return
        }
        copias = r.dados
        pastaDasCopias = pasta
        escolhida = 0
        painelRestaurar.open()
    }
    Component.onCompleted: {
        const r = Api.chamar("inicio_copias", {})
        copias = r.ok && Array.isArray(r.dados) ? r.dados : []
    }

    function restaurarCopia() {
        senhaCopia.erro = ""
        const c = copias[escolhida]
        Api.chamarDemorado("inicio_restaurar", { arquivo: c.caminho, senha: senhaCopia.texto, pasta: pastaDasCopias }, "Restaurando a loja…", r => {
            if (r.ok) {
                painelRestaurar.close()
                janela.avisar("A loja foi restaurada da cópia de " + c.quando + ". Entre com o seu usuário.", "ok")
                janela.atualizarEstado()
                janela.trocarPara("TelaEntrar")
                return
            }
            senhaCopia.erro = r.erro
            senhaCopia.focarSempre()
        })
    }

    function comecar() {
        for (const c of [loja, caixa, nome, senha, senha2])
            c.erro = ""
        // No registro (no Android, no logcat): o teste do emulador sabe se o
        // cadastro foi pedido e por que foi recusado. Nunca as senhas.
        console.info("Rustinho: cadastro inicial pedido")
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
        console.warn("Rustinho: cadastro inicial recusado (" + (r.campo || "geral") + "): " + r.erro)
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
                visible: tela.copias.length > 0
                titulo: "Restaurar a loja"
                color: Tema.verde
                Layout.fillWidth: true
                Layout.topMargin: 8
                Nota {
                    text: tela.copias.length === 0 ? ""
                          : (tela.pastaDasCopias ? "Encontrei cópias da loja na pasta escolhida"
                                                 : "Encontrei cópias da loja neste " + Tema.aparelho)
                            + ". A mais nova é de " + tela.copias[0].quando
                            + ". Se o Rustinho foi reinstalado, restaure a loja a partir da cópia: ela volta como estava."
                    font.pixelSize: 15
                    Layout.fillWidth: true
                }
                Botao {
                    text: "Restaurar a loja a partir da cópia"
                    onClicked: painelRestaurar.open()
                }
            }

            // B5 da R2 da 008: a cópia num pen drive ou noutra pasta (o seletor do sistema).
            Botao {
                objectName: "restaurarDeOutraPasta"
                text: "Restaurar de um pen drive ou outra pasta"
                tipo: "discreto"
                onClicked: escolherPastaDasCopias.open()
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
                        dica: "Pelo menos 8 caracteres"
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
                altura: 48
                onClicked: tela.comecar()
            }

        }
    }

    StackView.onActivated: loja.focarSempre()

    FolderDialog {
        id: escolherPastaDasCopias
        title: "Onde está a cópia da loja (pen drive ou outra pasta)"
        onAccepted: tela.procurarEmOutraPasta(selectedFolder)
    }

    // PEND-30: a senha de um gerente da loja da cópia (a cópia sai trancada com a chave da loja).
    Painel {
        id: painelRestaurar
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: "Restaurar a loja"; font.pixelSize: 30; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Nota {
                text: tela.copias.length === 0 ? ""
                      : "A loja volta como estava na cópia de " + tela.copias[tela.escolhida].quando
                        + ". Digite a senha de um gerente da loja (a que ele usava quando a cópia foi feita)."
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            // B7: se a mais nova não abrir, outra das mais novas (até 5, sem paginação).
            Nota {
                visible: tela.copias.length > 1
                text: "Outras cópias:"
                font.pixelSize: 15
            }
            Flow {
                visible: tela.copias.length > 1
                spacing: 8
                Layout.fillWidth: true
                Repeater {
                    model: tela.copias.slice(0, 5)
                    delegate: Botao {
                        required property var modelData
                        required property int index
                        objectName: "copia_" + index
                        text: modelData.quando
                        tipo: index === tela.escolhida ? "principal" : "secundario"
                        onClicked: {
                            tela.escolhida = index
                            senhaCopia.erro = ""
                            senhaCopia.focarSempre()
                        }
                    }
                }
            }
            Campo {
                id: senhaCopia
                rotulo: "Senha de um gerente"
                senha: true
                maximo: 256
                Layout.fillWidth: true
                onConfirmado: tela.restaurarCopia()
            }
            Botao {
                text: "Restaurar"
                tipo: "principal"
                Layout.alignment: Qt.AlignRight
                onClicked: tela.restaurarCopia()
            }
        }
        onOpened: senhaCopia.focarSempre()
    }
}
