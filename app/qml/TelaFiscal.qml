import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Conferência fiscal: CSOSN que não combina com a regra de ST, peças sem
// NCM ou sem CEST, e as regras de ST (que o contador pode mudar).
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var dados: null
    property var ufs: []
    property bool regrasAlteradas: false

    ListModel { id: regras }

    function carregar() {
        const c = Api.chamar("config_ler")
        if (c.ok)
            ufs = c.dados.ufs
        const r = Api.chamar("fiscal_conferir")
        if (r.ok)
            preencher(r.dados)
        else
            janela.avisar(r.erro, "erro")
    }

    function preencher(d) {
        dados = d
        regras.clear()
        for (const g of d.regras)
            regras.append({ uf: g.uf, cestInicio: g.cestInicio, vigenteDe: g.vigenteDe, vigenteAte: g.vigenteAte, comSt: g.comSt, descricao: g.descricao })
        regrasAlteradas = false
    }

    function corrigirTodas() {
        const ids = dados.conflitos.map(p => p.id)
        janela.confirmar("Passar " + ids.length + (ids.length === 1 ? " peça" : " peças") + " para automático?",
                         "O CSOSN dessas peças passa a seguir a regra do estado (hoje: " + (dados.conflitos[0] ? dados.conflitos[0].sugerido : "") + "). "
                         + "Quando a regra mudar, elas acompanham sozinhas. Confirme com o contador antes.",
                         "Passar para automático", () => {
            const r = Api.chamar("fiscal_automatico", { ids: ids })
            if (r.ok) {
                preencher(r.dados.conferencia)
                janela.avisar(r.dados.alteradas + (r.dados.alteradas === 1 ? " peça alterada." : " peças alteradas."), "ok")
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function salvarRegras() {
        salvarRegrasAgora()
    }
    function salvarRegrasAgora() {
        const lista = []
        for (let i = 0; i < regras.count; i++) {
            const g = regras.get(i)
            lista.push({ uf: g.uf, cestInicio: g.cestInicio, vigenteDe: g.vigenteDe, vigenteAte: g.vigenteAte, comSt: g.comSt, descricao: g.descricao })
        }
        janela.chamarGerente("fiscal_regras_salvar", { regras: lista }, r => {
            if (r.ok) {
                preencher(r.dados)
                janela.avisar("Regras de ST salvas.", "ok")
            } else {
                const m = r.campo ? r.campo.match(/^regras\.(\d+)\.(\w+)$/) : null
                if (m) {
                    const linha = repRegras.itemAt(parseInt(m[1]))
                    if (linha)
                        linha.campo(m[2]).erro = r.erro
                }
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function aoEsc() {
        if (regrasAlteradas)
            janela.confirmar("Descartar as regras alteradas?", "As mudanças nas regras de ST ainda não foram salvas.",
                             "Descartar", () => janela.voltar(), "perigo")
        else
            janela.voltar()
    }

    component ListaPecas: ColumnLayout {
        id: listaPecas
        property var pecas: []
        property bool mostrarCsosn: false
        property int limite: 8
        property bool tudo: false
        spacing: 4
        Layout.fillWidth: true
        Repeater {
            model: listaPecas.tudo ? listaPecas.pecas : listaPecas.pecas.slice(0, listaPecas.limite)
            delegate: Rectangle {
                required property var modelData
                required property int index
                Layout.fillWidth: true
                implicitHeight: 40
                radius: Tema.raioCampo
                color: sobre.hovered ? Tema.vela(0.6) : Tema.vela(0.35)
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12
                    Text {
                        text: modelData.codigo
                        color: Tema.tintaSuave
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                        visible: !Tema.estreito
                        Layout.preferredWidth: 60
                    }
                    Text {
                        text: modelData.nome + (modelData.marca ? "  ·  " + modelData.marca : "")
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 15
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Text {
                        text: listaPecas.mostrarCsosn ? "CSOSN " + modelData.csosn + "  →  " + modelData.sugerido
                                                      : (modelData.ncm ? "NCM " + modelData.ncm : "")
                        color: listaPecas.mostrarCsosn ? Tema.vermelho : Tema.tintaSuave
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                    }
                    Icone { nome: "direita"; tamanho: 16; cor: Tema.apagado }
                }
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: tela.janela.abrir("TelaPeca", { produtoId: modelData.id }) }
            }
        }
        Botao {
            visible: listaPecas.pecas.length > listaPecas.limite
            text: listaPecas.tudo ? "Mostrar menos" : "Ver todas (" + listaPecas.pecas.length + ")"
            tipo: "discreto"
            Layout.leftMargin: -10
            onClicked: listaPecas.tudo = !listaPecas.tudo
        }
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active && tela.regrasAlteradas
        onActivated: tela.salvarRegras()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 12

        Cabecalho {
            titulo: "Fiscal"
            subtitulo: !tela.dados ? "" : (tela.dados.uf ? "Loja em " + tela.dados.uf : "Estado da loja não informado")
                                         + " · regras válidas em " + tela.dados.data.split("-").reverse().join("/")
                                         + " · " + tela.dados.totalPecas + " peças ativas"
            Layout.fillWidth: true
            onVoltar: tela.aoEsc()
        }

        Flickable {
            id: rolagem
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: corpo.implicitHeight + Tema.margem
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            Linha {
                id: corpo
                width: rolagem.width - 14
                y: 6
                spacing: 18

                ColumnLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: "Notas fiscais"
                        subtitulo: "emitir pelo Rustinho"
                        color: Tema.verde
                        corTitulo: Tema.verdeEscuro
                        Layout.fillWidth: true
                        Flow {
                            spacing: 8
                            Layout.fillWidth: true
                            Repeater {
                                model: ["NF-e · modelo 55", "NFC-e · cupom, modelo 65", "NFS-e · serviço", "MDF-e · transporte"]
                                delegate: Rectangle {
                                    required property string modelData
                                    radius: height / 2
                                    color: Tema.papel
                                    implicitWidth: textoNota.implicitWidth + 30
                                    implicitHeight: textoNota.implicitHeight + 16
                                    Text {
                                        id: textoNota
                                        anchors.centerIn: parent
                                        text: "<b>" + parent.modelData + "</b>  · em breve"
                                        textFormat: Text.StyledText
                                        color: Tema.tinta
                                        font.family: Tema.texto
                                        font.pixelSize: 15
                                    }
                                }
                            }
                        }
                        Nota {
                            text: "Por enquanto, as notas continuam no sistema de hoje. O cadastro das peças já guarda o que a nota vai pedir (NCM, CEST, CSOSN, PIS/Cofins, IPI, ANP e ISS)."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }

                    Faixa {
                        visible: !!tela.dados && tela.dados.uf === ""
                        titulo: "Falta o estado da loja"
                        color: Tema.rosa
                        corTitulo: Tema.vermelho
                        Layout.fillWidth: true
                        Nota {
                            text: "A substituição tributária muda de estado para estado. Informe o estado (e o CNPJ) da loja em Configurações."
                            Layout.fillWidth: true
                        }
                        Botao {
                            text: "Abrir configurações"
                            tipo: "secundario"
                            onClicked: tela.janela.abrir("TelaConfiguracoes")
                        }
                    }

                    Faixa {
                        id: faixaConflitos
                        readonly property int n: tela.dados ? tela.dados.conflitos.length : 0
                        titulo: "CSOSN que não combina com a regra"
                        color: n > 0 ? Tema.rosa : Tema.verde
                        corTitulo: n > 0 ? Tema.vermelho : Tema.verdeEscuro
                        Layout.fillWidth: true
                        Text {
                            text: faixaConflitos.n === 0 ? "Nenhuma peça com CSOSN errado" : faixaConflitos.n + (faixaConflitos.n === 1 ? " peça com CSOSN para conferir" : " peças com CSOSN para conferir")
                            color: Tema.tinta
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 28
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                        }
                        Nota {
                            text: "Em São Paulo, autopeças (CEST 01) e pneus (CEST 16) saíram da substituição tributária em 01/10/2026 "
                                  + "(Portaria SRE 34/2026). Desde então a venda dentro do estado deixa de ser CSOSN 500 / CFOP 5405 e passa a ser "
                                  + "CSOSN 102 / CFOP 5102. Peças com o CSOSN fixado à mão (como vêm do sistema antigo) não mudam sozinhas."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        ListaPecas {
                            pecas: tela.dados ? tela.dados.conflitos : []
                            mostrarCsosn: true
                        }
                        Botao {
                            visible: faixaConflitos.n > 0
                            text: "Passar todas para automático"
                            icone: "check"
                            onClicked: tela.corrigirTodas()
                        }
                    }

                    Faixa {
                        readonly property int n: tela.dados ? tela.dados.semNcm.length : 0
                        visible: n > 0
                        titulo: "Peças sem NCM"
                        subtitulo: "a nota fiscal não sai sem NCM"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        ListaPecas { pecas: tela.dados ? tela.dados.semNcm : [] }
                    }

                    Faixa {
                        readonly property int n: tela.dados ? tela.dados.semCest.length : 0
                        visible: n > 0
                        titulo: "NCM de autopeça sem CEST"
                        subtitulo: "sem CEST o Rustinho não sabe se a peça tem ST"
                        color: Tema.azul
                        Layout.fillWidth: true
                        ListaPecas { pecas: tela.dados ? tela.dados.semCest : [] }
                    }

                    Faixa {
                        titulo: "PIS/Cofins monofásico"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        Nota {
                            text: !tela.dados ? "" : (tela.dados.monofasicas === 0 ? "Nenhuma peça marcada como monofásica."
                                  : tela.dados.monofasicas + (tela.dados.monofasicas === 1 ? " peça marcada" : " peças marcadas") + " como monofásica.")
                                  + " As vendas dessas peças saem separadas no relatório para o contador (PGDAS-D). "
                                  + "Nas compras pelo XML, peças novas com PIS CST 04 já vêm marcadas."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }
                }

                ColumnLayout {
                    spacing: 14
                    Layout.preferredWidth: Math.min(500, Math.max(420, corpo.width * 0.36))
                    Layout.minimumWidth: 420
                    Layout.maximumWidth: 500
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        titulo: "Regras de substituição tributária"
                        color: Tema.azul
                        Layout.fillWidth: true
                        Nota {
                            text: "Quando o CEST da peça começa com o número da regra, no estado e no período dela, a peça tem (ou não) ST. "
                                  + "Vale a regra com o começo de CEST mais longo. Mude só com o contador."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Repeater {
                            id: repRegras
                            model: regras
                            delegate: LinhaRegraSt {
                                ufs: tela.ufs
                                Layout.fillWidth: true
                                Layout.topMargin: 6
                                onMudou: (campo, valor) => {
                                    regras.setProperty(index, campo, valor)
                                    tela.regrasAlteradas = true
                                }
                                onRemover: {
                                    regras.remove(index)
                                    tela.regrasAlteradas = true
                                }
                            }
                        }
                        Linha {
                            spacing: 10
                            Layout.fillWidth: true
                            Botao {
                                text: "Adicionar regra"
                                icone: "mais"
                                tipo: "secundario"
                                onClicked: {
                                    regras.append({ uf: tela.dados && tela.dados.uf ? tela.dados.uf : "SP", cestInicio: "", vigenteDe: "", vigenteAte: "", comSt: false, descricao: "" })
                                    tela.regrasAlteradas = true
                                }
                            }
                            Item { Layout.fillWidth: true }
                            Botao {
                                visible: tela.regrasAlteradas
                                text: "Salvar regras"
                                icone: "check"
                                atalho: "F2"
                                onClicked: tela.salvarRegras()
                            }
                        }
                    }
                }
            }
        }
    }

    StackView.onActivated: carregar()
}
