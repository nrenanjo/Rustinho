import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Usuários: quem usa o Rustinho. Todos têm acesso a todas as funções.
FocusScope {
    id: tela
    // Se um gerente liberou esta tela para um caixa, acaba ao sair dela.
    Component.onDestruction: Api.chamar("gerente_encerrar")
    readonly property var janela: ApplicationWindow.window
    property var usuarios: []
    property var editando: null      // usuário aberto à direita (null = novo)
    // Celular: a lista e a ficha aparecem uma de cada vez.
    property bool mostrarFicha: false

    function carregar() {
        const r = Api.chamar("usuarios_listar")
        if (r.ok)
            usuarios = r.dados
    }

    function novo() {
        mostrarFicha = true
        editando = null
        nome.texto = ""
        comissao.texto = ""
        ativo.checked = true
        senha.texto = ""
        senha2.texto = ""
        limparErros()
        nome.focar()
    }

    function editar(u) {
        mostrarFicha = true
        editando = u
        nome.texto = u.nome
        comissao.texto = u.comissao
        ativo.checked = u.ativo
        senha.texto = ""
        senha2.texto = ""
        limparErros()
        nome.focar()
    }

    function limparErros() {
        nome.erro = ""
        senha.erro = ""
        senha2.erro = ""
        comissao.erro = ""
    }

    function salvar() {
        limparErros()
        const r = Api.chamar("usuario_salvar", {
            id: editando ? editando.id : "",
            nome: nome.texto,
            ativo: ativo.checked,
            senha: senha.texto,
            senha2: senha2.texto,
            comissao: comissao.texto
        })
        if (r.ok) {
            janela.avisar(editando ? "Usuário alterado." : "Usuário criado.", "ok")
            carregar()
            janela.atualizarEstado()
            const salvo = usuarios.find(u => u.id === r.dados.id)
            if (salvo)
                editar(salvo)
            if (Tema.estreito)
                mostrarFicha = false
        } else {
            const campos = { nome: nome, senha: senha, senha2: senha2, comissao: comissao }
            const alvo = campos[r.campo]
            if (alvo) {
                alvo.erro = r.erro
                alvo.focar()
            } else {
                janela.avisar(r.erro, "erro")
            }
        }
    }

    function quando(iso) {
        if (!iso)
            return "nunca entrou"
        return "último acesso " + Qt.formatDateTime(new Date(iso), "dd/MM/yyyy 'às' HH:mm")
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.salvar()
    }

    Linha {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 22

        ColumnLayout {
            visible: !Tema.estreito || !tela.mostrarFicha
            spacing: 14
            Layout.fillWidth: true
            Layout.fillHeight: true

            Cabecalho {
                sensivel: true
                titulo: "Usuários"
                subtitulo: "Todos acessam todas as funções. Cada um entra com o próprio nome e senha."
                Layout.fillWidth: true
                onVoltar: tela.janela.voltar()
                Botao {
                    text: "Novo usuário"
                    icone: "mais"
                    onClicked: tela.novo()
                }
            }

            ListView {
                id: lista
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: tela.usuarios
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle {
                    id: linha
                    required property var modelData
                    required property int index
                    readonly property bool aberto: tela.editando && tela.editando.id === modelData.id
                    width: lista.width
                    height: 76
                    radius: Tema.raio
                    color: aberto ? Tema.tinta : (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                    opacity: modelData.ativo ? 1 : 0.6

                    Text {
                        x: 16
                        y: -2
                        text: linha.modelData.nome.charAt(0).toUpperCase()
                        font.family: Tema.titulo
                        font.italic: true
                        font.weight: Font.DemiBold
                        font.pixelSize: 56
                        color: Tema.dourado
                    }
                    ColumnLayout {
                        anchors.left: parent.left
                        anchors.leftMargin: 70
                        anchors.right: parent.right
                        anchors.rightMargin: 20
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 0
                        Text {
                            text: linha.modelData.nome + (linha.modelData.voce ? "  (você)" : "") + (linha.modelData.ativo ? "" : "  · desativado")
                            color: linha.aberto ? Tema.claro : Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 24
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: tela.quando(linha.modelData.ultimoAcesso)
                            color: linha.aberto ? Tema.claroSuave(0.75) : Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }
                    HoverHandler { cursorShape: Qt.PointingHandCursor }
                    TapHandler { onTapped: tela.editar(linha.modelData) }
                }
            }
        }

        Faixa {
            visible: !Tema.estreito || tela.mostrarFicha
            titulo: tela.editando ? "Alterar usuário" : "Novo usuário"
            color: Tema.pergaminho
            Layout.preferredWidth: 400
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: Tema.estreito ? 0 : 92
            Botao {
                visible: Tema.estreito
                tipo: "discreto"
                icone: "voltar"
                text: "Voltar à lista"
                Layout.leftMargin: -10
                onClicked: tela.mostrarFicha = false
            }

            Campo {
                id: nome
                rotulo: "Nome"
                dica: "Como aparece na tela de entrada"
                maximo: 40
                Layout.fillWidth: true
                onConfirmado: senha.focar()
            }
            Campo {
                id: comissao
                sensivel: true
                rotulo: "Comissão nas vendas"
                dica: "Sem comissão"
                sufixo: "%"
                numerico: true
                maximo: 6
                Layout.preferredWidth: 200
                onConfirmado: senha.focar()
            }
            Marcar {
                id: ativo
                text: "Ativo (pode entrar no Rustinho)"
            }
            Rotulo {
                text: tela.editando ? "Trocar a senha (deixe em branco para manter)" : "Senha"
                wrapMode: Text.WordWrap
                elide: Text.ElideNone
                Layout.fillWidth: true
                Layout.topMargin: 6
            }
            Campo {
                id: senha
                senha: true
                dica: tela.editando ? "Nova senha" : "Pelo menos 4 caracteres"
                maximo: 128
                Layout.fillWidth: true
                onConfirmado: senha2.focar()
            }
            Campo {
                id: senha2
                senha: true
                dica: "Repita a senha"
                maximo: 128
                Layout.fillWidth: true
                onConfirmado: tela.salvar()
            }
            Linha {
                spacing: 10
                Layout.fillWidth: true
                Layout.topMargin: 4
                Botao {
                    visible: !!tela.editando
                    text: "Novo"
                    tipo: "discreto"
                    icone: "mais"
                    onClicked: tela.novo()
                }
                Item { Layout.fillWidth: true }
                Botao {
                    text: "Salvar"
                    icone: "check"
                    atalho: "F2"
                    onClicked: tela.salvar()
                }
            }
        }
    }

    Component.onCompleted: {
        carregar()
        novo()
        mostrarFicha = false
    }
    StackView.onActivated: if (!Tema.estreito) nome.focar()
}
