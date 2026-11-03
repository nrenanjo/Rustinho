import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Fluxo de caixa: o que entrou e saiu, dia a dia, e o que vem pela frente.
FocusScope {
    id: secao
    property string parte: "aconteceu"
    readonly property var janela: ApplicationWindow.window
    property var fluxo: null
    property string periodo: "mes"
    property string erro: ""

    function datas(p) {
        const h = new Date()
        const f = d => Qt.formatDate(d, "dd/MM/yyyy")
        if (p === "mes")
            return [f(new Date(h.getFullYear(), h.getMonth(), 1)), f(h)]
        if (p === "passado")
            return [f(new Date(h.getFullYear(), h.getMonth() - 1, 1)), f(new Date(h.getFullYear(), h.getMonth(), 0))]
        if (p === "semana")
            return [f(new Date(h.getFullYear(), h.getMonth(), h.getDate() - 6)), f(h)]
        return [de.texto, ate.texto]
    }
    function usar(p) {
        periodo = p
        const d = datas(p)
        de.texto = d[0]
        ate.texto = d[1]
        carregar()
    }
    function carregar() {
        const r = Api.chamar("fluxo_caixa", { de: de.texto, ate: ate.texto, dias: 30 })
        erro = r.ok ? "" : r.erro
        if (r.ok)
            fluxo = r.dados
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Linha {
            spacing: 12
            Layout.fillWidth: true
            Pilulas {
                pequenas: true
                modelo: [{ id: "mes", nome: "Este mês" }, { id: "passado", nome: "Mês passado" }, { id: "semana", nome: "7 dias" }]
                atual: secao.periodo
                onEscolhido: id => secao.usar(id)
            }
            Item { Layout.fillWidth: true }
            // Datas: juntas numa linha também no celular.
            RowLayout {
                spacing: 8
                Campo {
                    id: de
                    dica: "de"
                    maximo: 10
                    Layout.preferredWidth: Tema.estreito ? -1 : 140
                    Layout.fillWidth: Tema.estreito
                    onConfirmado: { secao.periodo = ""; secao.carregar() }
                }
                Text { text: "a"; color: Tema.tintaSuave; font.family: Tema.serifa; font.italic: true; font.pixelSize: 15 }
                Campo {
                    id: ate
                    dica: "até"
                    maximo: 10
                    Layout.preferredWidth: Tema.estreito ? -1 : 140
                    Layout.fillWidth: Tema.estreito
                    onConfirmado: { secao.periodo = ""; secao.carregar() }
                }
                Botao { text: "Ver"; tipo: "secundario"; onClicked: { secao.periodo = ""; secao.carregar() } }
            }
        }
        TextoErro { text: secao.erro }

        Linha {
            colunasEstreito: 2
            spacing: 12
            Layout.fillWidth: true
            CartaoNumero { rotulo: "Entrou"; sensivel: true; valor: secao.fluxo ? secao.fluxo.entradas : ""; cor: Tema.verde; corValor: Tema.verdeEscuro }
            CartaoNumero { rotulo: "Saiu"; sensivel: true; valor: secao.fluxo ? secao.fluxo.saidas : ""; cor: Tema.rosa }
            CartaoNumero {
                rotulo: "Resultado"
                sensivel: true
                valor: secao.fluxo ? secao.fluxo.resultado : ""
                cor: Tema.pergaminho
                corValor: secao.fluxo && secao.fluxo.resultadoNegativo ? Tema.vermelho : Tema.tinta
            }
            CartaoNumero { rotulo: "Nas contas hoje"; sensivel: true; valor: secao.fluxo ? secao.fluxo.saldoHoje : ""; cor: Tema.azul }
        }

        // Celular: uma parte de cada vez, escolhida nas pílulas.
        Pilulas {
            visible: Tema.estreito && (true)
            modelo: [{ id: "aconteceu", nome: "O que aconteceu" }, { id: "frente", nome: "Pela frente" }]
            atual: secao.parte
            onEscolhido: id => secao.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // O que aconteceu
            ColumnLayout {
                visible: !Tema.estreito || secao.parte === "aconteceu"
                spacing: 8
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: Tema.estreito ? -1 : 1
                Rotulo { text: "O que aconteceu"; Layout.topMargin: 2 }
                ListView {
                    id: realizado
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 5
                    model: secao.fluxo ? secao.fluxo.realizado : []
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: LinhaFluxo {
                        required property var modelData
                        required property int index
                        width: realizado.width - 10
                        dia: modelData
                        indice: index
                        mostrarSaldo: false
                    }
                    Nota {
                        anchors.centerIn: parent
                        visible: realizado.count === 0
                        text: "Nada neste período."
                        font.pixelSize: 16
                    }
                }
                Nota {
                    text: "Vendas em crediário e em cheque entram quando o dinheiro chega (recebimento ou depósito)."
                    font.pixelSize: 12
                    Layout.fillWidth: true
                }
            }

            // O que vem pela frente
            Rectangle {
                visible: !Tema.estreito || secao.parte === "frente"
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: Tema.estreito ? -1 : 1
                radius: Tema.raio
                color: Tema.vela(0.28)
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        Rotulo { text: "Próximos 30 dias"; Layout.fillWidth: true }
                        Text {
                            text: secao.fluxo ? "termina com R$ " + Tema.valor(secao.fluxo.saldoPrevisto) : ""
                            color: secao.fluxo && secao.fluxo.saldoPrevistoNegativo ? Tema.vermelho : Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 15
                        }
                    }
                    Text {
                        visible: !!secao.fluxo && (secao.fluxo.atrasadoReceber !== "0,00" || secao.fluxo.atrasadoPagar !== "0,00")
                        text: secao.fluxo ? "Atrasado (fora da conta): a receber R$ " + secao.fluxo.atrasadoReceber + "  ·  a pagar R$ " + secao.fluxo.atrasadoPagar : ""
                        color: Tema.vermelho
                        font.family: Tema.texto
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    ListView {
                        id: previsto
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 5
                        model: secao.fluxo ? secao.fluxo.previsto : []
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        delegate: LinhaFluxo {
                            required property var modelData
                            required property int index
                            width: previsto.width - 10
                            dia: modelData
                            indice: index
                            mostrarSaldo: true
                        }
                        Nota {
                            anchors.centerIn: parent
                            width: parent.width - 40
                            visible: previsto.count === 0
                            text: "Nada a receber nem a pagar nos próximos 30 dias."
                            horizontalAlignment: Text.AlignHCenter
                            font.pixelSize: 16
                        }
                    }
                    Nota {
                        text: "Começa com o saldo das contas da loja hoje; soma o crediário e os cheques a receber e tira as contas a pagar."
                        font.pixelSize: 12
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    Component.onCompleted: usar("mes")
}
