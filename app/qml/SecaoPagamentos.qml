import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Pagamentos no Painel: como os clientes pagam (o que entra na hora e o que
// entra depois).
FocusScope {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property int dias: 30
    property var dados: null

    function carregar() {
        const r = Api.chamar("painel_pagamentos", { dias: dias })
        if (r.ok)
            dados = r.dados
        else
            janela.avisar(r.erro, "erro")
    }
    onDiasChanged: carregar()

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        RowLayout {
            spacing: 12
            Layout.fillWidth: true
            CartaoNumero {
                rotulo: "Vendido"
                sensivel: true
                valor: secao.dados ? secao.dados.total : ""
                nota: secao.dados ? secao.dados.vendas + (secao.dados.vendas === 1 ? " venda" : " vendas") : ""
            }
            CartaoNumero {
                rotulo: "Entra na hora"
                sensivel: true
                valor: secao.dados ? secao.dados.naHora : ""
                nota: secao.dados && secao.dados.naHoraParte ? secao.dados.naHoraParte + " · dinheiro, Pix e débito" : "dinheiro, Pix e débito"
                cor: Tema.verde
                corValor: Tema.verdeEscuro
            }
            CartaoNumero {
                rotulo: "Entra depois"
                sensivel: true
                valor: secao.dados ? secao.dados.depois : ""
                nota: secao.dados && secao.dados.depoisParte ? secao.dados.depoisParte + " · crédito, cheque e crediário" : "crédito, cheque e crediário"
                cor: Tema.azul
            }
        }

        Faixa {
            titulo: "Formas de pagamento"
            color: Tema.pergaminho
            espacamento: 16
            Layout.fillWidth: true
            Repeater {
                model: secao.dados ? secao.dados.itens : []
                delegate: LinhaRanking {
                    required property var modelData
                    item: modelData
                }
            }
            Nota {
                visible: !!secao.dados && secao.dados.itens.length === 0
                text: "Nenhuma venda no período."
                font.pixelSize: 14
            }
        }

        Item { Layout.fillHeight: true }

        Nota {
            text: "Venda dividida entre duas formas conta nas duas. O dinheiro aparece já sem o troco."
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    Component.onCompleted: carregar()
}
