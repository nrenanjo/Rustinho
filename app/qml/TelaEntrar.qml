import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Entrada: cada pessoa clica no próprio nome e digita a senha.
FocusScope {
    id: tela
    // Como esta tela aparece na orientação da navegação guiada ("Depois o
    // Rustinho volta para …").
    readonly property string nomeNaNavegacao: "Entrada"
    readonly property var janela: ApplicationWindow.window
    property var usuarios: []
    property int escolhido: -1

    function carregar() {
        const r = Api.chamar("usuarios_entrada")
        usuarios = r.ok ? r.dados : []
        if (usuarios.length === 1)
            escolher(0)
    }

    function escolher(indice) {
        escolhido = indice
        senha.erro = ""
        senha.texto = ""
        senha.focarSempre()
        // H20 (7.2): com muitos usuários no celular, a senha e o Entrar
        // podem ficar abaixo da dobra: a tela rola até eles.
        Qt.callLater(mostrarSenha)
    }

    function mostrarSenha() {
        const p = linhaSenha.mapToItem(coluna, 0, 0)
        const fim = coluna.y + p.y + linhaSenha.height + Tema.margem
        if (fim > rolagem.contentY + rolagem.height)
            rolagem.contentY = Math.min(rolagem.contentHeight - rolagem.height, fim - rolagem.height)
    }

    function entrar() {
        if (escolhido < 0)
            return
        const r = Api.chamar("entrar", { id: usuarios[escolhido].id, senha: senha.texto })
        if (r.ok) {
            janela.entrou()
        } else {
            senha.erro = r.erro
            senha.focarSempre()
        }
    }

    function agoraPorExtenso() {
        const d = new Date()
        return d.toLocaleDateString(Qt.locale("pt_BR"), "dddd, d 'de' MMMM 'de' yyyy")
    }

    // H20 (7.2): a entrada rola (como as boas-vindas) e, no celular, usa
    // tamanhos menores: com 5 usuários, todos os nomes, a senha e o Entrar
    // cabem ou ficam ao alcance da rolagem.
    Flickable {
        id: rolagem
        anchors.fill: parent
        contentHeight: Math.max(height, coluna.implicitHeight + 2 * Tema.margem)
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: coluna
            width: Math.min(980, rolagem.width - 2 * Tema.margem)
            x: (rolagem.width - width) / 2
            y: Math.max(Tema.margem, (rolagem.height - implicitHeight) / 2)
            spacing: Tema.estreito ? 16 : 26

            ColumnLayout {
                spacing: 0
                Titulo {
                    text: "Rustinho"
                    font.pixelSize: Tema.estreito ? 56 : 76
                    font.weight: Font.Bold
                    font.italic: true
                }
                Nota {
                    text: (janela && janela.estado.loja ? janela.estado.loja + " · " : "")
                          + (janela ? janela.estado.caixa : "") + " · " + tela.agoraPorExtenso()
                    font.pixelSize: 18
                    Layout.fillWidth: true
                }
            }

            Rotulo { text: "Quem está usando?"; Layout.topMargin: 10 }

            Flow {
                id: cartoes
                spacing: 14
                Layout.fillWidth: true

                Repeater {
                    model: tela.usuarios
                    delegate: Rectangle {
                        id: cartao
                        required property var modelData
                        required property int index
                        readonly property bool marcado: tela.escolhido === index
                        // No celular, dois cartões por linha e mais baixos (H20).
                        width: Tema.estreito ? Math.floor((cartoes.width - cartoes.spacing) / 2) : 210
                        height: Tema.estreito ? 96 : 128
                        radius: Tema.raio
                        // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                        readonly property bool par: (index + 1) % 2 === 0
                        color: marcado ? Tema.atual : Tema.faixa(cartao.par)
                        palette.window: Tema.superficie(color)
                        border.width: foco.activeFocus ? 2 : 0
                        border.color: Tema.azulEscuroDaFaixa(cartao.par && !marcado)

                        Text {
                            x: 18
                            y: 6
                            text: cartao.modelData.nome.charAt(0).toUpperCase()
                            font.family: Tema.titulo
                            font.italic: true
                            font.weight: Font.DemiBold
                            font.pixelSize: Tema.estreito ? 48 : 64
                            color: cartao.marcado ? Tema.tintaAtual : Tema.douradoDaFaixa(cartao.par)
                            opacity: 0.85
                        }
                        Text {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: Tema.estreito ? 14 : 18
                            text: cartao.modelData.nome
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: Tema.estreito ? 22 : 26
                            color: cartao.marcado ? Tema.tintaAtual : Tema.tintaDaFaixa(cartao.par)
                            elide: Text.ElideRight
                        }
                        Item {
                            id: foco
                            anchors.fill: parent
                            activeFocusOnTab: true
                            Keys.onReturnPressed: tela.escolher(cartao.index)
                            Keys.onEnterPressed: tela.escolher(cartao.index)
                            Keys.onSpacePressed: tela.escolher(cartao.index)
                        }
                        TapHandler { onTapped: tela.escolher(cartao.index) }
                        HoverHandler { cursorShape: Qt.PointingHandCursor }
                    }
                }
            }

            Linha {
                id: linhaSenha
                visible: tela.escolhido >= 0
                spacing: 14
                Layout.maximumWidth: 640
                Layout.fillWidth: true
                Campo {
                    id: senha
                    rotulo: tela.escolhido >= 0 ? "Senha de " + tela.usuarios[tela.escolhido].nome : "Senha"
                    senha: true
                    maximo: 256
                    Layout.fillWidth: true
                    onConfirmado: tela.entrar()
                }
                Botao {
                    text: "Entrar"
                    atalho: "Enter"
                    Layout.alignment: Qt.AlignTop
                    Layout.topMargin: 22
                    onClicked: tela.entrar()
                }
            }
        }
    }

    Component.onCompleted: carregar()
    StackView.onActivated: {
        if (escolhido >= 0)
            senha.focarSempre()
    }

}
