import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Entrada: cada pessoa clica no próprio nome e digita a senha.
FocusScope {
    id: tela
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

    ColumnLayout {
        width: Math.min(980, parent.width - 2 * Tema.margem)
        anchors.centerIn: parent
        spacing: 26

        ColumnLayout {
            spacing: 0
            Titulo {
                text: "Rustinho"
                font.pixelSize: 76
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
                    width: 210
                    height: 128
                    radius: Tema.raio
                    color: marcado ? Tema.tinta : (index % 2 === 0 ? Tema.pergaminho : Tema.azul)
                    border.width: foco.activeFocus ? 2 : 0
                    border.color: Tema.azulEscuro

                    Text {
                        x: 18
                        y: 6
                        text: cartao.modelData.nome.charAt(0).toUpperCase()
                        font.family: Tema.titulo
                        font.italic: true
                        font.weight: Font.DemiBold
                        font.pixelSize: 64
                        color: cartao.marcado ? Tema.dourado : Tema.dourado
                        opacity: 0.85
                    }
                    Text {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: 18
                        text: cartao.modelData.nome
                        font.family: Tema.titulo
                        font.weight: Font.Bold
                        font.pixelSize: 26
                        color: cartao.marcado ? Tema.claro : Tema.tinta
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
            visible: tela.escolhido >= 0
            spacing: 14
            Layout.maximumWidth: 640
            Layout.fillWidth: true
            Campo {
                id: senha
                rotulo: tela.escolhido >= 0 ? "Senha de " + tela.usuarios[tela.escolhido].nome : "Senha"
                senha: true
                maximo: 128
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

    Component.onCompleted: carregar()
    StackView.onActivated: {
        if (escolhido >= 0)
            senha.focarSempre()
    }

}
