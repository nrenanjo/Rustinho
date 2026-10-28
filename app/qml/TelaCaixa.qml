import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Templates as T
import QtQuick.Layouts
import rustinho

// Caixa deste computador: abrir, sangria, suprimento, fechar e as vendas.
FocusScope {
    id: tela
    property string parte: "resumo"
    readonly property var janela: ApplicationWindow.window
    property var estado: null
    property var vendas: []
    // FALHA-04: chave de cada operação do caixa (abrir; o movimento e o
    // fechamento ganham chave nova ao abrir a janela deles).
    property string chaveAbrir: Api.novaChave()
    readonly property bool aberto: !!estado && estado.aberto
    readonly property var r: aberto ? estado.resumo : null

    function carregar() {
        chaveAbrir = Api.novaChave()
        const e = Api.chamar("caixa_estado")
        if (!e.ok) {
            janela.avisar(e.erro, "erro")
            return
        }
        estado = e.dados
        if (aberto) {
            const v = Api.chamar("vendas_listar", { tipo: "venda", sessao: estado.resumo.sessao.id, limite: 500 })
            vendas = v.ok ? v.dados : []
        } else {
            vendas = []
            Qt.callLater(() => trocoInicial.focar())
        }
    }

    function imprimir(id) {
        const x = JSON.parse(Sistema.imprimir("caixa", id))
        if (!x.ok)
            janela.avisar(x.erro, "erro")
    }

    function hora(iso) {
        return Qt.formatDateTime(new Date(iso), "dd/MM 'às' HH:mm")
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 12

        Cabecalho {
            titulo: "Caixa"
            subtitulo: !tela.estado ? "" : tela.aberto
                       ? tela.estado.caixa + " · aberto " + tela.hora(tela.r.sessao.abertoEm) + " por " + tela.r.sessao.abertoPor
                       : tela.estado.caixa + " · fechado"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                visible: tela.aberto
                text: "Sangria"
                tipo: "secundario"
                onClicked: movimento.abrirCom("sangria")
            }
            Botao {
                visible: tela.aberto
                text: "Suprimento"
                tipo: "secundario"
                onClicked: movimento.abrirCom("suprimento")
            }
            Botao {
                visible: tela.aberto
                text: "Imprimir parcial"
                tipo: "discreto"
                onClicked: tela.imprimir(tela.r.sessao.id)
            }
            Botao {
                visible: tela.aberto
                text: "Fechar caixa"
                icone: "check"
                onClicked: fechar.abrirCom()
            }
        }

        // Celular: resumo e vendas em pílulas (uma parte de cada vez).
        Pilulas {
            visible: Tema.estreito && tela.aberto
            modelo: [{ id: "resumo", nome: "Resumo" }, { id: "vendas", nome: "Vendas (" + tela.vendas.length + ")" }]
            atual: tela.parte
            onEscolhido: id => tela.parte = id
        }

        RowLayout {
            spacing: 18
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---------- Resumo ou abertura ----------
            Flickable {
                visible: !Tema.estreito || tela.parte === "resumo" || !tela.aberto
                Layout.fillWidth: Tema.estreito
                Layout.preferredWidth: Tema.estreito ? -1 : 400
                Layout.fillHeight: true
                clip: true
                contentHeight: lado.implicitHeight + 20
                boundsBehavior: Flickable.StopAtBounds

                ColumnLayout {
                    id: lado
                    width: parent.width
                    spacing: 14

                    Faixa {
                        visible: !!tela.estado && !tela.aberto
                        titulo: "Abrir o caixa"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true
                        Nota {
                            text: "Conte o dinheiro que está na gaveta para o troco e abra o caixa."
                            Layout.fillWidth: true
                        }
                        Campo {
                            id: trocoInicial
                            rotulo: "Troco na gaveta"
                            prefixo: "R$"
                            numerico: true
                            dica: "0,00"
                            Layout.fillWidth: true
                            onConfirmado: botaoAbrir.clicked()
                        }
                        Botao {
                            id: botaoAbrir
                            text: "Abrir caixa"
                            icone: "check"
                            onClicked: {
                                const x = Api.gravar("caixa_abrir", { valor: trocoInicial.texto }, tela.chaveAbrir)
                                if (x.ok) {
                                    tela.carregar()
                                    tela.janela.avisar("Caixa aberto.", "ok")
                                } else {
                                    trocoInicial.erro = x.erro
                                }
                            }
                        }
                    }

                    Faixa {
                        visible: tela.aberto
                        titulo: "Dinheiro na gaveta"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true
                        Text {
                            text: tela.r ? "R$ " + tela.r.esperado : ""
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.pixelSize: 44
                        }
                        Nota {
                            text: "Troco inicial + vendas e crediário recebidos em dinheiro − troco dado + suprimentos − sangrias − devoluções em dinheiro."
                            font.pixelSize: 13
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        visible: tela.aberto
                        titulo: "Hoje neste caixa"
                        color: Tema.pergaminho
                        espacamento: 7
                        Layout.fillWidth: true
                        Repeater {
                            model: !tela.r ? [] : [["Troco inicial", "R$ " + tela.r.valorInicial],
                                                   ["Vendas (" + tela.r.vendas + ")", "R$ " + tela.r.totalVendido]]
                                                  .concat(tela.r.porForma.map(f => ["    " + f.nome, "R$ " + f.valor]))
                                                  .concat(tela.r.recebimentos > 0 ? [["Recebido do crediário (" + tela.r.recebimentos + ")", "R$ " + tela.r.totalRecebido]] : [])
                                                  .concat(tela.r.recebidoPorForma.map(f => ["    " + f.nome, "R$ " + f.valor]))
                                                  .concat([["Suprimentos", "R$ " + tela.r.suprimentos], ["Sangrias", "− R$ " + tela.r.sangrias]])
                                                  .concat(tela.r.devolvido !== "0,00" ? [["Devolvido a clientes", "− R$ " + tela.r.devolvido]] : [])
                                                  .concat([["Vendas canceladas", String(tela.r.canceladas)]])
                            delegate: RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                Text {
                                    text: modelData[0]
                                    color: Tema.tintaSuave
                                    font.family: Tema.texto
                                    font.pixelSize: 15
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: modelData[1]
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 15
                                }
                            }
                        }
                    }

                    Faixa {
                        visible: tela.aberto && tela.r.movimentos.length > 0
                        titulo: "Sangrias e suprimentos"
                        color: Tema.azul
                        espacamento: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: tela.r ? tela.r.movimentos : []
                            delegate: Nota {
                                required property var modelData
                                text: Qt.formatTime(new Date(modelData.quando), "HH:mm") + " · " + (modelData.tipo === "sangria" ? "Sangria" : "Suprimento")
                                      + " R$ " + modelData.valor + " · " + modelData.motivo + " · " + modelData.usuario
                                font.pixelSize: 14
                                Layout.fillWidth: true
                            }
                        }
                    }

                    Faixa {
                        visible: !!tela.estado && !tela.aberto && tela.estado.historico.length > 0
                        titulo: "Caixas anteriores"
                        color: Tema.azul
                        espacamento: 6
                        Layout.fillWidth: true
                        Repeater {
                            model: tela.estado ? tela.estado.historico.slice(0, 12) : []
                            delegate: RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                Text {
                                    text: tela.hora(modelData.abertoEm) + " · " + modelData.abertoPor
                                    color: Tema.tinta
                                    font.family: Tema.texto
                                    font.pixelSize: 14
                                    Layout.fillWidth: true
                                }
                                Botao {
                                    text: "Imprimir"
                                    tipo: "discreto"
                                    implicitHeight: 30
                                    font.pixelSize: 13
                                    onClicked: tela.imprimir(modelData.id)
                                }
                            }
                        }
                    }
                }
            }

            // ---------- Vendas deste caixa ----------
            ColumnLayout {
                visible: !Tema.estreito || (tela.parte === "vendas" && tela.aberto)
                spacing: 8
                Layout.fillWidth: true
                Layout.fillHeight: true
                Rotulo { text: tela.aberto ? "Vendas deste caixa" : "" }
                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
                    model: tela.vendas
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    delegate: LinhaVenda {
                        required property var modelData
                        required property int index
                        width: lista.width
                        item: modelData
                        indice: index
                        onAbrir: tela.janela.abrir("TelaVenda", { vendaId: modelData.id })
                    }
                    Nota {
                        anchors.centerIn: parent
                        visible: tela.aberto && tela.vendas.length === 0
                        text: "Nenhuma venda neste caixa ainda."
                        font.pixelSize: 16
                    }
                }
            }
        }
    }

    // Sangria / suprimento
    Popup {
        id: movimento
        property string tipo: "sangria"
        property var contas: []
        property string chave: ""
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(480, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        T.Overlay.modal: Rectangle { color: Tema.veu }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }
        function abrirCom(t) {
            chave = Api.novaChave()
            tipo = t
            valorMov.texto = ""
            valorMov.erro = ""
            motivoMov.texto = ""
            motivoMov.erro = ""
            const c = Api.chamar("contas_loja", {})
            contas = c.ok ? c.dados : []
            contaMov.indice = 0
            open()
            valorMov.focar()
        }
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: movimento.tipo === "sangria" ? "Sangria" : "Suprimento"; font.pixelSize: 32 }
            Nota {
                text: movimento.tipo === "sangria" ? "Dinheiro que sai da gaveta (ex.: levar para o cofre ou o banco)."
                                                   : "Dinheiro que entra na gaveta (ex.: mais troco)."
                Layout.fillWidth: true
            }
            Campo { id: valorMov; rotulo: "Valor"; prefixo: "R$"; numerico: true; Layout.fillWidth: true; onConfirmado: motivoMov.focar() }
            Campo { id: motivoMov; rotulo: "Motivo"; maximo: 120; Layout.fillWidth: true; onConfirmado: botaoMov.clicked() }
            Selecao {
                id: contaMov
                rotulo: movimento.tipo === "sangria" ? "O dinheiro vai para" : "O dinheiro vem de"
                modelo: movimento.contas.map(c => c.nome).concat([movimento.tipo === "sangria" ? "Nenhuma conta (saiu da loja)" : "Nenhuma conta"])
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: movimento.close() }
                Botao {
                    id: botaoMov
                    text: "Confirmar"
                    icone: "check"
                    onClicked: {
                        const x = Api.gravar("caixa_movimentar", { tipo: movimento.tipo, valor: valorMov.texto, motivo: motivoMov.texto,
                                                                conta: (movimento.contas[contaMov.indice] || {}).id || "" },
                                             movimento.chave)
                        if (x.ok) {
                            movimento.close()
                            tela.carregar()
                            tela.janela.avisar(movimento.tipo === "sangria" ? "Sangria registrada." : "Suprimento registrado.", "ok")
                        } else if (x.campo === "motivo") {
                            motivoMov.erro = x.erro
                        } else {
                            valorMov.erro = x.erro
                        }
                    }
                }
            }
        }
    }

    // Fechar
    Popup {
        id: fechar
        modal: true
        focus: true
        anchors.centerIn: T.Overlay.overlay
        width: Math.min(520, Tema.largura - 24)
        padding: Tema.estreito ? 18 : 30
        T.Overlay.modal: Rectangle { color: Tema.veu }
        background: Rectangle { color: Tema.pergaminho; radius: Tema.raio + 3 }
        property string chave: ""
        function abrirCom() {
            chave = Api.novaChave()
            contado.texto = ""
            contado.erro = ""
            obs.texto = ""
            open()
            contado.focar()
        }
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: "Fechar o caixa"; font.pixelSize: 32 }
            Nota {
                text: "Conte o dinheiro da gaveta e digite o total. O Rustinho mostra a diferença e imprime o fechamento."
                Layout.fillWidth: true
            }
            Campo { id: contado; rotulo: "Dinheiro contado"; prefixo: "R$"; numerico: true; Layout.fillWidth: true; onConfirmado: botaoFechar.clicked() }
            AreaTexto { id: obs; rotulo: "Observações"; Layout.fillWidth: true }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Botao { text: "Cancelar"; tipo: "discreto"; onClicked: fechar.close() }
                Botao {
                    id: botaoFechar
                    text: "Fechar e imprimir"
                    icone: "check"
                    onClicked: {
                        const x = Api.gravar("caixa_fechar", { valor: contado.texto, motivo: obs.texto }, fechar.chave)
                        if (!x.ok) {
                            contado.erro = x.erro
                            return
                        }
                        fechar.close()
                        const d = x.dados.diferencaCentavos
                        const valor = x.dados.diferenca.replace("-", "")
                        tela.janela.avisar(d === 0 ? "Caixa fechado. O dinheiro bateu." :
                                           "Caixa fechado. " + (d > 0 ? "Sobraram R$ " : "Faltaram R$ ") + valor + " na gaveta.",
                                           d === 0 ? "ok" : "info")
                        tela.imprimir(x.dados.sessao.id)
                        tela.carregar()
                    }
                }
            }
        }
    }

    StackView.onActivated: carregar()
}
