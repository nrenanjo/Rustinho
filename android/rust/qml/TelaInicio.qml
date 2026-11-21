import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Início: saudação e busca rápida (fixas em cima); embaixo, rolando quando
// não cabe, as notificações e as pílulas das funções (as fixadas primeiro).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    // Como esta tela aparece no "voltar" da tela que ela abriu.
    readonly property string nomeNaNavegacao: "Início"
    property var menu: ({ todos: [] })
    property var avisos: []
    // Faixas (as críticas primeiro, depois a expandida) e quadrados (as
    // informativas e de atenção minimizadas), na ordem que vem do núcleo.
    readonly property var faixas: avisos.filter(a => !(a.minimizado && a.categoria !== "critica"))
    readonly property var quadrados: avisos.filter(a => a.minimizado && a.categoria !== "critica")

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
        const r = Api.chamar("aviso_esconder", { id: aviso.id })
        if (!r.ok) {
            janela.avisar(r.erro, "info")
            carregar()
        }
    }

    // QT-1b: "Conferi" (a pessoa conferiu o que foi e o que não foi gravado). Só resolve a notificação: não
    // grava, não refaz e não desfaz nenhuma operação.
    function resolverAviso(aviso) {
        const r = Api.chamar("aviso_resolver", { id: aviso.id })
        if (!r.ok)
            janela.avisar(r.erro, "info")
        carregar()
    }

    // Minimizar/expandir é só apresentação (fica anotado neste computador).
    // Uma faixa não crítica aberta por vez: abrir uma fecha a outra.
    // O núcleo grava tudo de uma vez: as outras não críticas ficam minimizadas.
    function minimizarAviso(aviso, minimizado) {
        const r = Api.chamar("aviso_minimizar", { id: aviso.id, minimizado: minimizado })
        if (!r.ok) {
            janela.avisar(r.erro, "info")
            carregar()
            return
        }
        avisos = avisos.map(a => a.id === aviso.id ? Object.assign({}, a, { minimizado: minimizado })
                                 : a.categoria !== "critica" && !a.minimizado
                                   ? Object.assign({}, a, { minimizado: true }) : a)
    }

    // Linkagem: leva direto ao lugar de resolver.
    function abrirAviso(aviso) {
        const f = funcao(aviso.funcao)
        if (!f)
            return
        // 9e (C8, dev3.3): UF ou CNPJ da loja faltando. Auto: a guia leva
        // direto ao campo, e a volta ao Início só acontece quando o núcleo
        // confirma o que foi salvo (a notificação some sozinha).
        if (aviso.id === "fiscal_loja"
                && janela.guiar("requisito:loja.uf_cnpj", "", ok => tela.carregar(), "UF e CNPJ da loja salvos."))
            return
        if (aviso.id.indexOf("senha.") === 0) {
            janela.abrir("TelaConfiguracoes", { secao: "loja" })
        } else if (aviso.id === "xml_pendencias") {
            Api.chamar("menu_usar", { id: f.id })
            janela.abrir("TelaPecas", { abrirPendenciasXml: true })
        } else if (aviso.secao) {
            // 9a/9c (C8): com um registro só, ele vem destacado (alvo).
            janela.abrir("TelaConfiguracoes", { secao: aviso.secao, alvo: aviso.alvo || "" })
        } else if (aviso.id === "receber") {
            Api.chamar("menu_usar", { id: f.id })
            janela.abrir("TelaReceber", {})
        } else if (aviso.id === "cheques") {
            Api.chamar("menu_usar", { id: f.id })
            janela.abrir("TelaFinanceiro", { secao: "cheques" })
        } else if (aviso.id === "oficina") {
            Api.chamar("menu_usar", { id: f.id })
            janela.abrir("TelaOrdens", { quais: aviso.texto !== "" ? "prontas" : "abertas" })
        } else if (aviso.id === "encomendas" || aviso.id === "encomendas_numero_repetido") {
            Api.chamar("menu_usar", { id: f.id })
            // 10a (C8): com uma encomenda só, a linha dela vem destacada.
            janela.abrir("TelaEstoque", { secao: "encomendas", alvo: aviso.alvo || "" })
        } else {
            janela.abrirFuncao(f)
        }
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

    readonly property int colunas: Tema.fino ? 1 : Tema.estreito ? 2 : width >= 1000 ? 4 : 3

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- Parte de cima, fixa: saudação, loja e caixa, tema, trocar usuário e busca ----
        ColumnLayout {
            spacing: 10
            Layout.fillWidth: true
            Layout.leftMargin: Tema.margem
            Layout.rightMargin: Tema.margem
            Layout.topMargin: Tema.margem - 4

            GridLayout {
                // T042: por cima da barra de pesquisa logo abaixo: o halo dela passa atrás, não por cima.
                z: 1
                columns: Tema.fino ? 1 : 2
                columnSpacing: 16
                rowSpacing: 6
                Layout.fillWidth: true
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    // Ajuste visual de 15/11/2026: a saudação e a data sem serifa (a Source Sans 3 do resto do
                    // Início), só nesta tela; tamanho, peso e estilo de antes. A altura de cada uma é a de antes,
                    // para o cabeçalho não crescer (a Source Sans tem a linha mais alta que a Cormorant e a Source Serif).
                    // As mesmas duas linhas com as fontes de antes, invisíveis (fora do layout): a altura delas é a altura
                    // de antes, medida pelo próprio Qt.
                    Titulo {
                        id: saudacaoAntes
                        visible: false
                        text: saudacao.text
                        font.pixelSize: saudacao.font.pixelSize
                        wrapMode: saudacao.wrapMode
                        width: saudacao.width
                    }
                    Nota {
                        id: dataAntes
                        visible: false
                        text: data.text
                        font.pixelSize: data.font.pixelSize
                        width: data.width
                    }
                    Titulo {
                        id: saudacao
                        font.family: Tema.texto
                        // A altura de antes (a da cópia invisível), com o texto centrado nela.
                        Layout.preferredHeight: saudacaoAntes.implicitHeight
                        verticalAlignment: Text.AlignVCenter
                        text: tela.saudacao() + ", " + (tela.janela && tela.janela.estado.usuario ? tela.janela.estado.usuario.nome : "")
                        font.pixelSize: Tema.fino ? 36 : Tema.baixo ? 38 : 50
                        wrapMode: Tema.fino ? Text.WordWrap : Text.NoWrap
                        Layout.fillWidth: true
                    }
                    Nota {
                        id: data
                        font.family: Tema.texto
                        Layout.preferredHeight: dataAntes.implicitHeight
                        verticalAlignment: Text.AlignVCenter
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
                            // T084 (texto O do proprietário, 21/11): abre a tela de Usuários pelo mesmo caminho do
                            // cartão Usuários do menu (a mesma função, a mesma senha do gerente para o caixa).
                            onClicked: {
                                const usuarios = tela.funcao("usuarios")
                                if (usuarios)
                                    tela.janela.abrirFuncao(usuarios)
                            }
                        }
                    }
                }
            }

            // A busca como nas imagens aprovadas (D-V6, 0.9.18): colar à
            // esquerda e copiar à direita, dentro do campo; o atalho F11 fora
            // do campo, à direita.
            RowLayout {
                spacing: 18
                Layout.fillWidth: true
                Layout.topMargin: 2
                Layout.bottomMargin: 12
                Campo {
                    id: busca
                    dica: Tema.estreito ? "Buscar peça: código, nome ou carro" : "Buscar peça: qualquer código, nome, modelo ou carro (ex.: óleo 5w30, 8PK1515)"
                    colarCopiar: true
                    tamanhoFonte: 19
                    altura: 56
                    Layout.fillWidth: true
                    onConfirmado: tela.buscar()
                }
                Text {
                    visible: !Tema.celular && !Tema.estreito
                    text: "F11"
                    color: Tema.tintaSuaveNo(palette.window)
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 17
                    font.letterSpacing: 1
                    Layout.alignment: Qt.AlignVCenter
                    Layout.rightMargin: 4
                }
            }
        }

        // ---- Abaixo da busca: rola quando não cabe (notificações e pílulas) ----
        Flickable {
            id: rolagem
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: coluna.implicitHeight + Tema.margem
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: rolagem.contentHeight > rolagem.height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff }

            ColumnLayout {
                id: coluna
                x: Tema.margem
                width: rolagem.width - 2 * Tema.margem
                spacing: 14

                // Notificações: as faixas (largura toda, uma embaixo da outra) e,
                // depois, os quadrados das minimizadas, em fila que quebra linha.
                ColumnLayout {
                    visible: tela.avisos.length > 0
                    spacing: 8
                    Layout.fillWidth: true
                    Layout.bottomMargin: 6
                    Repeater {
                        model: tela.faixas
                        delegate: Notificacao {
                            required property var modelData
                            aviso: modelData
                            funcao: modelData.funcao
                            icone: tela.funcao(modelData.funcao) ? tela.funcao(modelData.funcao).icone : ""
                            Layout.fillWidth: true
                            onAbrir: tela.abrirAviso(modelData)
                            onMinimizar: (m) => tela.minimizarAviso(modelData, m)
                            onEsconder: tela.esconderAviso(modelData)
                            onResolver: tela.resolverAviso(modelData)
                        }
                    }
                    Flow {
                        visible: tela.quadrados.length > 0
                        spacing: Tema.estreito ? 10 : 14
                        Layout.fillWidth: true
                        Repeater {
                            model: tela.quadrados
                            delegate: Notificacao {
                                required property var modelData
                                aviso: modelData
                                funcao: modelData.funcao
                                icone: tela.funcao(modelData.funcao) ? tela.funcao(modelData.funcao).icone : ""
                                width: lado
                                onAbrir: tela.abrirAviso(modelData)
                                onMinimizar: (m) => tela.minimizarAviso(modelData, m)
                                onEsconder: tela.esconderAviso(modelData)
                                onResolver: tela.resolverAviso(modelData)
                            }
                        }
                    }
                }

                // Uma grade só: as fixadas primeiro (na ordem em que foram
                // fixadas), depois as outras na ordem fixa.
                GridLayout {
                    columns: tela.colunas
                    columnSpacing: Tema.estreito ? 10 : 14
                    rowSpacing: Tema.estreito ? 10 : 14
                    Layout.fillWidth: true
                    Repeater {
                        model: tela.menu.todos
                        delegate: CartaoFuncao {
                            required property var modelData
                            required property int index
                            item: modelData
                            indice: index
                            colunas: tela.colunas
                            Layout.fillWidth: true
                            Layout.preferredWidth: 100
                            onAbrir: tela.janela.abrirFuncao(modelData)
                            onFixar: tela.fixar(modelData)
                        }
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

    // RVI-02: a situação da troca de dados (e os outros avisos) muda com o
    // Início aberto; ele confere de novo a cada 20 s, para não continuar
    // mostrando "tentando de novo" quando a troca já parou (ou o contrário).
    Timer {
        interval: 20000
        repeat: true
        running: tela.StackView.status === StackView.Active
        onTriggered: {
            const a = Api.chamar("avisos_do_dia")
            if (a.ok)
                tela.avisos = a.dados.filter(x => tela.funcao(x.funcao))
        }
    }
}
