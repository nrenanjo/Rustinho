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
    property var trabalhos: ({
        meus: [],
        deOutroCaixa: [],
        deUsuariosDesativados: [],
        outrosAtivos: { usuariosComTrabalho: 0, totalTrabalhos: 0 },
        houveInterrupcao: false,
        relogioRecuado: false
    })
    readonly property bool temTrabalhosParaMostrar: {
        const t = trabalhos || {}
        return ((t.meus && t.meus.length > 0)
                || (t.deOutroCaixa && t.deOutroCaixa.length > 0)
                || (t.deUsuariosDesativados && t.deUsuariosDesativados.length > 0)
                || (t.outrosAtivos && t.outrosAtivos.totalTrabalhos > 0))
    }
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
        const t = Api.chamar("trabalho_listar")
        if (t.ok && t.dados)
            trabalhos = t.dados
    }

    function descartarMeuTrabalho(item) {
        janela.confirmar(
            "Descartar este rascunho?",
            "O trabalho guardado “" + item.titulo + "” (" + item.tipoNome + ") será descartado deste caixa e ficará anotado no histórico.",
            "Descartar rascunho",
            () => {
                const r = Api.chamar("trabalho_descartar", { id: item.id, motivo: "Descartado pelo usuário no Início" })
                if (r.ok) {
                    janela.avisar("Rascunho descartado.", "ok")
                    carregar()
                } else {
                    janela.avisar(r.erro, "erro")
                }
            },
            "perigo"
        )
    }

    function abrirTrabalhoDeDesativado(item) {
        motivoTrabalhoGerente.perguntar(
            "Abrir trabalho de " + item.usuarioNome + "?",
            "Este usuário está desativado. Como exceção de recuperação (7b), o gerente pode abrir o rascunho “"
            + item.titulo + "” para conferir e concluir. O acesso fica registrado no histórico com o seu nome, o autor original e o motivo.",
            "Abrir rascunho",
            (m, pronto) => {
                janela.retomarTrabalhoDesativado(item, m, r => pronto(r))
            },
            "Motivo da abertura pelo gerente",
            "principal"
        )
    }

    function descartarTrabalhoDeDesativado(item) {
        motivoTrabalhoGerente.perguntar(
            "Descartar trabalho de " + item.usuarioNome + "?",
            "O rascunho “" + item.titulo + "” deixado pelo usuário desativado será descartado e ficará registrado no histórico com o seu nome e o motivo.",
            "Descartar rascunho",
            (m, pronto) => {
                janela.chamarGerente("trabalho_gerente_descartar", { id: item.id, motivo: m }, r => {
                    pronto(r)
                    if (r.ok) {
                        janela.avisar("Rascunho do usuário desativado descartado.", "ok")
                        carregar()
                    } else {
                        janela.avisar(r.erro, "erro")
                    }
                })
            },
            "Motivo do descarte pelo gerente",
            "perigo"
        )
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
        } else if (aviso.id === "fiscal_notas") {
            Api.chamar("menu_usar", { id: f.id })
            if (aviso.alvo)
                janela.abrir("TelaCompra", { compraId: aviso.alvo, abrirConferenciaAoAbrir: true })
            else
                janela.abrir("TelaCompras", { filtro: "aguardando_autorizacao" })
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

                // REC-1 a REC-5: "Continuar de onde parou?" — só aparece quando há trabalhos guardados
                // (próprios, de outro caixa vindos em backup, de usuários desativados para o gerente 7b,
                // ou contagem de outros usuários ativos DR-3).
                Faixa {
                    id: faixaContinuarTrabalho
                    objectName: "faixaContinuarTrabalho"
                    visible: tela.temTrabalhosParaMostrar
                    titulo: "Continuar de onde parou?"
                    subtitulo: "rascunhos guardados neste caixa · nada definitivo é concluído sozinho"
                    color: Tema.pergaminho
                    Layout.fillWidth: true

                    Nota {
                        visible: !!(tela.trabalhos && tela.trabalhos.relogioRecuado)
                        text: "Atenção: o relógio deste computador parece ter voltado para trás em relação às gravações anteriores. Nenhum rascunho foi rebaixado para abandonado."
                        color: Tema.vermelho
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }

                    Repeater {
                        model: tela.trabalhos && tela.trabalhos.meus ? tela.trabalhos.meus : []
                        delegate: Rectangle {
                            id: linhaMeuTrabalho
                            required property var modelData
                            required property int index
                            readonly property bool par: (index + 1) % 2 === 0
                            Layout.fillWidth: true
                            radius: Tema.raioCampo
                            color: Tema.faixa(par, Tema.vela(0.45), Tema.vela(0.25))
                            palette.window: Tema.superficie(color)
                            implicitHeight: mioloMeuTrabalho.implicitHeight + 16
                            GridLayout {
                                id: mioloMeuTrabalho
                                x: 12
                                y: 8
                                width: linhaMeuTrabalho.width - 24
                                columns: Tema.estreito ? 1 : 2
                                columnSpacing: 12
                                rowSpacing: 8
                                ColumnLayout {
                                    spacing: 2
                                    Layout.fillWidth: true
                                    Text {
                                        text: modelData.tipoNome + " · " + modelData.titulo
                                        textFormat: Text.PlainText
                                        color: Tema.tintaDaFaixa(linhaMeuTrabalho.par)
                                        font.family: Tema.titulo
                                        font.weight: Font.Bold
                                        font.pixelSize: 17
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: "Guardado em " + modelData.atualizadoTexto
                                              + (modelData.situacao === "abandonado" ? " · Guardado há mais de 30 dias (abandonado)" : "")
                                              + (modelData.situacao === "sem_conserto" ? " · Estado ilegível (sem conserto)" : "")
                                              + (modelData.usouVersaoAnterior ? " · Recuperado da versão anterior íntegra" : "")
                                              + (modelData.conflitoBase ? " · " + modelData.motivoConflito : "")
                                              + (modelData.jaConcluida ? " · Operação já concluída" + (modelData.resumoConcluida ? " (" + modelData.resumoConcluida + ")" : "") : "")
                                        textFormat: Text.PlainText
                                        color: (modelData.conflitoBase || modelData.situacao === "sem_conserto")
                                               ? Tema.vermelhoDaFaixa(linhaMeuTrabalho.par)
                                               : Tema.tintaSuaveDaFaixa(linhaMeuTrabalho.par)
                                        font.family: Tema.serifa
                                        font.italic: true
                                        font.pixelSize: 13
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                }
                                Flow {
                                    spacing: 8
                                    Layout.alignment: Tema.estreito ? Qt.AlignLeft : Qt.AlignRight | Qt.AlignVCenter
                                    Botao {
                                        text: modelData.jaConcluida ? "Ver conclusão" : "Continuar"
                                        tipo: "principal"
                                        altura: 38
                                        enabled: modelData.situacao !== "sem_conserto" && !modelData.formatoIncompativel
                                        onClicked: tela.janela.retomarTrabalho(modelData)
                                    }
                                    Botao {
                                        text: "Descartar"
                                        tipo: "secundario"
                                        altura: 38
                                        onClicked: tela.descartarMeuTrabalho(modelData)
                                    }
                                }
                            }
                        }
                    }

                    // DR-6 / REC-4: trabalhos vindos num backup restaurado de outro caixa.
                    ColumnLayout {
                        visible: !!(tela.trabalhos && tela.trabalhos.deOutroCaixa && tela.trabalhos.deOutroCaixa.length > 0)
                        spacing: 6
                        Layout.fillWidth: true
                        Nota {
                            text: "Rascunhos vindos no backup de outro caixa (preservados para conferência; o trabalho não muda de caixa sozinho):"
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Repeater {
                            model: tela.trabalhos && tela.trabalhos.deOutroCaixa ? tela.trabalhos.deOutroCaixa : []
                            delegate: Nota {
                                required property var modelData
                                text: "• " + modelData.tipoNome + " · " + modelData.titulo + " (caixa de origem: " + modelData.dispositivoOrigem + " · guardado em " + modelData.atualizadoTexto + ")"
                                color: Tema.vermelho
                                font.pixelSize: 13
                                Layout.fillWidth: true
                            }
                        }
                    }

                    // DR-3: para o gerente, existência de trabalhos de outros usuários ativos (sem ver nem abrir o conteúdo).
                    Nota {
                        visible: !!(tela.trabalhos && tela.trabalhos.outrosAtivos && tela.trabalhos.outrosAtivos.totalTrabalhos > 0)
                        text: {
                            const o = tela.trabalhos && tela.trabalhos.outrosAtivos ? tela.trabalhos.outrosAtivos : { totalTrabalhos: 0, usuariosComTrabalho: 0 }
                            return "Há " + o.totalTrabalhos + (o.totalTrabalhos === 1 ? " trabalho guardado de " : " trabalhos guardados de ")
                                   + o.usuariosComTrabalho + (o.usuariosComTrabalho === 1 ? " outro usuário ativo" : " outros usuários ativos")
                                   + " neste caixa (pelo sigilo do trabalho em andamento, só cada usuário abre o seu)."
                        }
                        font.pixelSize: 13
                        Layout.fillWidth: true
                    }

                    // 7b / REC-2: trabalhos deixados por usuários desativados (exceção restrita e auditável para o gerente).
                    ColumnLayout {
                        visible: !!(tela.trabalhos && tela.trabalhos.deUsuariosDesativados && tela.trabalhos.deUsuariosDesativados.length > 0)
                        spacing: 6
                        Layout.fillWidth: true
                        Nota {
                            text: "Trabalhos deixados por usuários desativados (acesso do gerente com senha e registro no histórico):"
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                        Repeater {
                            model: tela.trabalhos && tela.trabalhos.deUsuariosDesativados ? tela.trabalhos.deUsuariosDesativados : []
                            delegate: Rectangle {
                                id: linhaDesativado
                                required property var modelData
                                required property int index
                                readonly property bool par: (index + 1) % 2 === 0
                                Layout.fillWidth: true
                                radius: Tema.raioCampo
                                color: Tema.faixa(par, Tema.vela(0.45), Tema.vela(0.25))
                                palette.window: Tema.superficie(color)
                                implicitHeight: mioloDesativado.implicitHeight + 14
                                GridLayout {
                                    id: mioloDesativado
                                    x: 12
                                    y: 7
                                    width: linhaDesativado.width - 24
                                    columns: Tema.estreito ? 1 : 2
                                    columnSpacing: 12
                                    rowSpacing: 6
                                    ColumnLayout {
                                        spacing: 2
                                        Layout.fillWidth: true
                                        Text {
                                            text: modelData.usuarioNome + " (desativado) · " + modelData.tipoNome + " · " + modelData.titulo
                                            textFormat: Text.PlainText
                                            color: Tema.tintaDaFaixa(linhaDesativado.par)
                                            font.family: Tema.titulo
                                            font.weight: Font.Bold
                                            font.pixelSize: 16
                                            wrapMode: Text.WordWrap
                                            Layout.fillWidth: true
                                        }
                                        Text {
                                            text: "Guardado em " + modelData.atualizadoTexto
                                            textFormat: Text.PlainText
                                            color: Tema.tintaSuaveDaFaixa(linhaDesativado.par)
                                            font.family: Tema.serifa
                                            font.italic: true
                                            font.pixelSize: 13
                                            wrapMode: Text.WordWrap
                                            Layout.fillWidth: true
                                        }
                                    }
                                    Flow {
                                        spacing: 8
                                        Layout.alignment: Tema.estreito ? Qt.AlignLeft : Qt.AlignRight | Qt.AlignVCenter
                                        Botao {
                                            text: "Abrir como gerente"
                                            tipo: "principal"
                                            altura: 36
                                            onClicked: tela.abrirTrabalhoDeDesativado(modelData)
                                        }
                                        Botao {
                                            text: "Descartar como gerente"
                                            tipo: "secundario"
                                            altura: 36
                                            onClicked: tela.descartarTrabalhoDeDesativado(modelData)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

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

    PerguntaMotivo {
        id: motivoTrabalhoGerente
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
