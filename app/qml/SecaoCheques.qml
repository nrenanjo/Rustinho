import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Cheques recebidos: em carteira (por "bom para"), depositar, repassar,
// devolvido.
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var painel: null
    property string quais: "carteira"
    property var contas: []
    property bool buscou: false

    function carregar() {
        const r = Api.chamar("cheques_painel", { texto: busca.texto, quais: quais })
        if (r.ok)
            painel = r.dados
        const c = Api.chamar("contas_loja", {})
        if (c.ok)
            contas = c.dados
        buscou = true
    }
    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            carregar()
            return true
        }
        return false
    }

    Timer { id: espera; interval: 200; onTriggered: secao.carregar() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        RowLayout {
            spacing: 12
            Layout.fillWidth: true
            CartaoNumero {
                rotulo: "Em carteira"
                valor: secao.painel ? secao.painel.emCarteira : ""
                nota: secao.painel ? secao.painel.quantosEmCarteira + (secao.painel.quantosEmCarteira === 1 ? " cheque" : " cheques") : ""
            }
            CartaoNumero {
                rotulo: "Pode depositar"
                valor: secao.painel ? secao.painel.bonsAteHoje : ""
                cor: Tema.verde
                corValor: Tema.verdeEscuro
            }
            CartaoNumero {
                rotulo: "Próximos 30 dias"
                valor: secao.painel ? secao.painel.proximos30Dias : ""
                cor: Tema.azul
            }
            CartaoNumero {
                rotulo: "Devolvidos"
                valor: secao.painel ? secao.painel.devolvidos : ""
                cor: Tema.rosa
                corValor: secao.painel && secao.painel.devolvidos !== "0,00" ? Tema.vermelho : Tema.tinta
            }
        }

        RowLayout {
            spacing: 14
            Layout.fillWidth: true
            Campo {
                id: busca
                icone: "busca"
                altura: 46
                tamanhoFonte: 16
                dica: "Procurar: número, quem assinou, cliente ou banco"
                Layout.fillWidth: true
                onEditado: espera.restart()
            }
            Pilulas {
                pequenas: true
                modelo: [{ id: "carteira", nome: "Em carteira" }, { id: "todos", nome: "Todos" }]
                atual: secao.quais
                onEscolhido: id => {
                    secao.quais = id
                    secao.carregar()
                }
            }
        }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: secao.painel ? secao.painel.cheques : []
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: linha
                required property var modelData
                required property int index
                readonly property var ch: modelData
                width: lista.width
                implicitHeight: 64
                radius: Tema.raio
                color: (index + 1) % 2 === 0 ? Tema.azul : Tema.pergaminho
                readonly property string chip: ch.situacao !== "carteira" ? ch.nomeSituacao
                                               : ch.atrasado ? "Passou do dia"
                                               : ch.podeDepositar ? "Pode depositar"
                                               : ch.diasPara === 1 ? "Amanhã" : "Em " + ch.diasPara + " dias"
                readonly property color corChip: ch.situacao === "devolvido" || ch.atrasado ? Tema.rosa
                                                 : ch.podeDepositar || ch.situacao === "depositado" ? Tema.verde : Tema.papel
                HoverHandler { id: sobreLinha }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 12
                    spacing: 14
                    opacity: linha.ch.situacao === "carteira" ? 1 : 0.75
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: 96
                        Text {
                            text: linha.ch.bomPara
                            color: linha.ch.atrasado ? Tema.vermelho : Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.DemiBold
                            font.pixelSize: 15
                        }
                        Text {
                            text: "bom para"
                            color: Tema.tintaSuave
                            font.family: Tema.texto
                            font.pixelSize: 12
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Text {
                            text: "Nº " + linha.ch.numero + "  ·  " + (linha.ch.emitente || "sem nome")
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: [linha.ch.banco, linha.ch.cliente && linha.ch.cliente !== linha.ch.emitente ? "cliente " + linha.ch.cliente : "",
                                   linha.ch.conta ? "em " + linha.ch.conta : "", linha.ch.situacaoEm, linha.ch.observacoes].filter(x => !!x).join("  ·  ")
                            visible: text !== ""
                            color: Tema.tintaSuave
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Chip {
                        texto: linha.chip
                        color: linha.corChip
                        corTexto: linha.ch.situacao === "devolvido" || linha.ch.atrasado ? Tema.vermelho : Tema.tinta
                    }
                    Text {
                        text: "R$ " + linha.ch.valor
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 17
                        horizontalAlignment: Text.AlignRight
                        Layout.preferredWidth: 110
                    }
                    Row {
                        spacing: 4
                        Layout.preferredWidth: 300
                        layoutDirection: Qt.RightToLeft
                        Botao {
                            visible: linha.ch.situacao === "carteira"
                            text: "Depositar"
                            tipo: "secundario"
                            implicitHeight: 36
                            onClicked: deposito.abrirCom(linha.ch)
                        }
                        Botao {
                            visible: (linha.ch.situacao === "carteira" || linha.ch.situacao === "depositado") && sobreLinha.hovered
                            text: "Devolvido"
                            tipo: "discreto"
                            implicitHeight: 36
                            onClicked: devolucao.abrirCom(linha.ch)
                        }
                        Botao {
                            visible: linha.ch.situacao === "carteira" && sobreLinha.hovered
                            text: "Repassar"
                            tipo: "discreto"
                            implicitHeight: 36
                            onClicked: motivo.perguntar("Repassar o cheque nº " + linha.ch.numero + "?",
                                                        "R$ " + linha.ch.valor + ". Ex.: pagou o fornecedor com ele.", "Repassar",
                                                        m => Api.chamar("cheque_repassar", { id: linha.ch.id, motivo: m }), "Para quem", "principal")
                        }
                        Botao {
                            visible: (linha.ch.situacao === "depositado" || linha.ch.situacao === "repassado") && sobreLinha.hovered
                            text: "Desfazer"
                            tipo: "discreto"
                            implicitHeight: 36
                            onClicked: janela.confirmar("Voltar para a carteira?", "O cheque nº " + linha.ch.numero + " volta a ficar em carteira.",
                                                        "Voltar", () => {
                                                            const r = Api.chamar("cheque_desfazer", { id: linha.ch.id })
                                                            if (!r.ok)
                                                                janela.avisar(r.erro, "erro")
                                                            secao.carregar()
                                                        })
                        }
                    }
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: secao.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : "Nenhum cheque em carteira"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    visible: busca.texto === ""
                    text: "Cheque recebido na venda ou no crediário (forma “Cheque”) aparece aqui até ser depositado."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    // ---------- Depositar ----------
    Painel {
        id: deposito
        property var cheque: null
        property string erro: ""
        readonly property var bancos: secao.contas
        width: Math.min(500, Tema.largura - 24)
        function abrirCom(c) {
            cheque = c
            erro = ""
            const i = bancos.findIndex(x => x.tipo === "banco")
            conta.indice = i >= 0 ? i : 0
            open()
        }
        function salvar() {
            const c = bancos[conta.indice]
            const r = Api.chamar("cheque_depositar", { id: cheque.id, conta: c ? c.id : "" })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            janela.avisar("Cheque depositado.", "ok")
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: deposito.cheque ? "Depositar o nº " + deposito.cheque.numero : ""; font.pixelSize: 30; Layout.fillWidth: true }
            Nota {
                text: deposito.cheque ? "R$ " + deposito.cheque.valor + " · bom para " + deposito.cheque.bomPara
                                        + (deposito.cheque.diasPara > 0 ? " (ainda faltam " + deposito.cheque.diasPara + " dias)" : "") : ""
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Selecao { id: conta; rotulo: "Em qual conta"; modelo: deposito.bancos.map(c => c.nome); Layout.fillWidth: true }
            TextoErro { text: deposito.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: deposito.close() }
                Botao { text: "Depositar"; icone: "check"; onClicked: deposito.salvar() }
            }
        }
    }

    // ---------- Devolvido ----------
    Painel {
        id: devolucao
        property var cheque: null
        property string erro: ""
        width: Math.min(520, Tema.largura - 24)
        function abrirCom(c) {
            cheque = c
            erro = ""
            dMotivo.texto = ""
            divida.checked = c.clienteId !== ""
            open()
            dMotivo.focar()
        }
        function salvar() {
            const r = Api.chamar("cheque_devolver", { id: cheque.id, motivo: dMotivo.texto, divida: divida.checked })
            if (!r.ok) {
                erro = r.erro
                return
            }
            close()
            secao.carregar()
        }
        contentItem: ColumnLayout {
            spacing: 12
            Titulo { text: devolucao.cheque ? "Cheque nº " + devolucao.cheque.numero + " devolvido" : ""; font.pixelSize: 30; Layout.fillWidth: true }
            Nota {
                text: devolucao.cheque ? "R$ " + devolucao.cheque.valor + (devolucao.cheque.situacao === "depositado" ? ". O valor sai de " + devolucao.cheque.conta + "." : ".") : ""
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Campo { id: dMotivo; rotulo: "Motivo"; dica: "Ex.: sem fundos"; maximo: 120; Layout.fillWidth: true; onConfirmado: devolucao.salvar() }
            Marcar {
                id: divida
                visible: !!devolucao.cheque && devolucao.cheque.clienteId !== ""
                text: devolucao.cheque ? "Lançar como dívida de " + devolucao.cheque.cliente + " no Receber" : ""
            }
            TextoErro { text: devolucao.erro }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: devolucao.close() }
                Botao { text: "Registrar"; tipo: "perigo"; onClicked: devolucao.salvar() }
            }
        }
    }

    PerguntaMotivo { id: motivo; onFeito: secao.carregar() }

    Component.onCompleted: {
        carregar()
        busca.focar()
    }
}
