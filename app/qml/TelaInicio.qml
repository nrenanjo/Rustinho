import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Início: saudação, busca rápida e as funções (mais usadas primeiro).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var menu: ({ maisUsados: [], todos: [] })
    property var avisos: []

    function carregar() {
        const r = Api.chamar("menu")
        if (r.ok)
            menu = r.dados
        const a = Api.chamar("avisos_do_dia")
        if (a.ok)
            avisos = a.dados.filter(x => funcao(x.funcao))
    }

    function funcao(id) {
        return (menu.todos || []).find(f => f.id === id && f.disponivel)
    }

    function esconderAviso(aviso) {
        Api.chamar("aviso_esconder", { id: aviso.id })
    }

    function fixar(item) {
        const r = Api.chamar("menu_fixar", { id: item.id, fixo: !item.fixo })
        if (r.ok)
            menu = r.dados
        else
            janela.avisar(r.erro, "erro")
    }

    function saudacao() {
        const h = new Date().getHours()
        return h < 12 ? "Bom dia" : h < 18 ? "Boa tarde" : "Boa noite"
    }

    function focarBusca() {
        busca.focar()
    }

    function buscar() {
        const texto = busca.texto.trim()
        Api.chamar("menu_usar", { id: "pecas" })
        busca.texto = ""
        janela.abrir("TelaPecas", { buscaInicial: texto })
    }

    readonly property int colunas: Tema.fino ? 1 : Tema.estreito ? 2 : width >= 1100 ? 4 : 3
    readonly property int colunasMaisUsados: Tema.fino ? 2 : colunas

    Flickable {
        anchors.fill: parent
        contentHeight: coluna.implicitHeight + 2 * Tema.margem
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: coluna
            x: Tema.margem
            y: Tema.margem - 4
            width: parent.width - 2 * Tema.margem
            spacing: 14

            GridLayout {
                columns: Tema.fino ? 1 : 2
                columnSpacing: 16
                rowSpacing: 6
                Layout.fillWidth: true
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Titulo {
                        text: tela.saudacao() + ", " + (tela.janela && tela.janela.estado.usuario ? tela.janela.estado.usuario.nome : "")
                        font.pixelSize: Tema.fino ? 36 : Tema.baixo ? 38 : 50
                        wrapMode: Tema.fino ? Text.WordWrap : Text.NoWrap
                        Layout.fillWidth: true
                    }
                    Nota {
                        text: new Date().toLocaleDateString(Qt.locale("pt_BR"), "dddd, d 'de' MMMM")
                        font.pixelSize: 17
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    spacing: 2
                    Layout.alignment: Tema.fino ? Qt.AlignLeft : Qt.AlignTop
                    Rotulo {
                        text: tela.janela ? [tela.janela.estado.loja, tela.janela.estado.caixa].filter(x => !!x).join(" · ") : ""
                        Layout.alignment: Tema.fino ? Qt.AlignLeft : Qt.AlignRight
                    }
                    RowLayout {
                        spacing: 10
                        Layout.alignment: Tema.fino ? Qt.AlignLeft : Qt.AlignRight
                        Layout.topMargin: 4
                        Aparencia { Layout.alignment: Qt.AlignVCenter }
                        Botao {
                            tipo: "discreto"
                            icone: "sair"
                            text: "Trocar usuário"
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: tela.janela.trocarUsuario()
                        }
                    }
                }
            }

            ColumnLayout {
                visible: tela.avisos.length > 0
                spacing: 10
                Layout.fillWidth: true
                Layout.topMargin: 6
                Repeater {
                    model: tela.avisos
                    delegate: FaixaAviso {
                        required property var modelData
                        aviso: modelData
                        Layout.fillWidth: true
                        onAbrir: {
                            const f = tela.funcao(modelData.funcao)
                            if (!f)
                                return
                            if (modelData.id === "backup" || modelData.id === "backup_nuvem") {
                                tela.janela.abrir("TelaConfiguracoes", { secao: "backup" })
                            } else if (modelData.id === "cheques") {
                                Api.chamar("menu_usar", { id: f.id })
                                tela.janela.abrir("TelaFinanceiro", { secao: "cheques" })
                            } else if (modelData.id === "oficina") {
                                Api.chamar("menu_usar", { id: f.id })
                                tela.janela.abrir("TelaOrdens", { quais: modelData.texto !== "" ? "prontas" : "abertas" })
                            } else if (modelData.id === "encomendas") {
                                Api.chamar("menu_usar", { id: f.id })
                                tela.janela.abrir("TelaEstoque", { secao: "encomendas" })
                            } else {
                                tela.janela.abrirFuncao(f)
                            }
                        }
                        onEsconder: tela.esconderAviso(modelData)
                    }
                }
            }

            Campo {
                id: busca
                dica: Tema.estreito ? "Buscar peça: código, nome ou carro" : "Buscar peça: qualquer código, nome, modelo ou carro (ex.: óleo 5w30, 8PK1515)"
                icone: "busca"
                atalho: "F11"
                tamanhoFonte: 19
                altura: 56
                Layout.fillWidth: true
                Layout.topMargin: 8
                onConfirmado: tela.buscar()
            }

            Rotulo { text: "Mais usados"; Layout.topMargin: 14 }

            GridLayout {
                columns: tela.colunasMaisUsados
                columnSpacing: Tema.estreito ? 10 : 14
                rowSpacing: Tema.estreito ? 10 : 14
                Layout.fillWidth: true
                Repeater {
                    model: tela.menu.maisUsados
                    delegate: CartaoFuncao {
                        required property var modelData
                        required property int index
                        item: modelData
                        indice: index
                        grande: !Tema.estreito
                        Layout.fillWidth: true
                        Layout.preferredWidth: 100
                        onAbrir: tela.janela.abrirFuncao(modelData)
                        onFixar: tela.fixar(modelData)
                    }
                }
            }

            Rotulo { text: "Todas as funções"; Layout.topMargin: 18 }

            GridLayout {
                columns: tela.colunas
                columnSpacing: 12
                rowSpacing: Tema.estreito ? 10 : 12
                Layout.fillWidth: true
                Repeater {
                    model: tela.menu.todos
                    delegate: CartaoFuncao {
                        required property var modelData
                        required property int index
                        item: modelData
                        indice: index
                        Layout.fillWidth: true
                        Layout.preferredWidth: 100
                        onAbrir: tela.janela.abrirFuncao(modelData)
                        onFixar: tela.fixar(modelData)
                    }
                }
            }
        }
    }

    StackView.onActivated: {
        carregar()
        busca.focar()
    }
    Component.onCompleted: carregar()
}
