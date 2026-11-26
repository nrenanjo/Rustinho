import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Conferência fiscal: CSOSN que não combina com a regra de ST, peças sem
// NCM ou sem CEST, e as regras de ST (que o contador pode mudar). FIS-P5 e
// FIS-P6 (0.9.19, etapa 3): as regras aparecem para consulta, por estado,
// separadas em vigentes, futuras e encerradas, com a fonte de cada uma; mudar
// é num painel à parte, uma regra de cada vez.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    property var dados: null
    property var ufs: []
    // O estado cujas regras aparecem (começa no da loja).
    property string ufRegras: ""
    readonly property var regrasDaUf: !dados ? [] : dados.regras.filter(r => r.uf === ufRegras)
    readonly property var ordemDasSituacoes: ["vigente", "futura", "encerrada"]
    // A regra aberta no painel (vazia: regra nova).
    property var editando: null
    readonly property string nomeNaNavegacao: "Fiscal"
    // 0.9.17: o estado (UF) e o CNPJ da loja, conferidos pelo núcleo
    // (requisito "loja.uf_cnpj"); o que falta aparece na faixa.
    property var dadosDaLoja: null
    // 0.9.15: a conferência fiscal fica dentro de Configurações › Fiscal
    // (sem cabeçalho próprio); a tela sozinha continua existindo.
    property bool embutida: false
    // FIS-LIGA (decisão do proprietário, 25/11/2026): o seletor do certificado
    // A3 e da integração fiscal deste computador. Só o gerente vê; ligar pede
    // a senha de um gerente digitada na hora; qualquer gerente desliga.
    property var liga: null


    function carregarLiga() {
        const e = Api.chamar("fiscal_liga_estado")
        liga = e.ok ? e.dados : null
    }

    function desligarFiscal() {
        janela.confirmar("Desligar o fiscal neste computador?",
                         "O Rustinho deixa de usar o certificado A3 e o emissor fiscal deste computador. O resto continua igual: vender, comprar e trazer o XML das notas.",
                         "Desligar", () => {
            const r = Api.chamar("fiscal_desligar")
            if (r.ok) {
                liga = r.dados
                janela.avisar("Fiscal desligado neste computador.", "ok")
            } else {
                janela.avisar(r.erro, "erro")
            }
        })
    }

    function ligarFiscal() {
        const r = Api.chamar("fiscal_ligar", { senha: senhaLiga.texto })
        senhaLiga.texto = ""
        if (!r.ok) {
            senhaLiga.erro = r.erro
            senhaLiga.focar()
            return
        }
        liga = r.dados.estado
        painelLiga.close()
        janela.avisar("Fiscal ligado neste computador por " + r.dados.nome + ".", "ok")
    }

    function carregar() {
        carregarLiga()
        const c = Api.chamar("config_ler")
        if (c.ok)
            ufs = c.dados.ufs
        const q = Api.chamar("requisito_conferir", { id: "loja.uf_cnpj" })
        dadosDaLoja = q.ok ? q.dados : null
        const r = Api.chamar("fiscal_conferir")
        if (r.ok)
            preencher(r.dados)
        else
            janela.avisar(r.erro, "erro")
    }

    function preencher(d) {
        dados = d
        if (ufRegras === "")
            ufRegras = d.uf || "SP"
    }

    function corrigirTodas() {
        const ids = dados.conflitos.map(p => p.id)
        janela.confirmar("Passar " + ids.length + (ids.length === 1 ? " peça" : " peças") + " para automático?",
                         "O CSOSN dessas peças passa a seguir a regra do estado (hoje: " + (dados.conflitos[0] ? dados.conflitos[0].sugerido : "") + "). "
                         + "Quando a regra mudar, elas acompanham sozinhas. Confirme com o contador antes.",
                         "Passar para automático", () => {
            // R24 (DP-23, decisão do proprietário de 25/11/2026): mudar o
            // CSOSN das peças pede a senha do gerente.
            janela.chamarGerente("fiscal_automatico", { ids: ids }, r => {
                if (r.ok) {
                    preencher(r.dados.conferencia)
                    janela.avisar(r.dados.alteradas + (r.dados.alteradas === 1 ? " peça alterada." : " peças alteradas."), "ok")
                } else {
                    janela.avisar(r.erro, "erro")
                }
            })
        })
    }

    function nomeDaSituacao(sit) {
        return ({ vigente: "Valendo hoje", futura: "Vão valer", encerrada: "Encerradas" })[sit] || sit
    }

    function periodo(r) {
        return (r.vigenteAte ? r.vigenteDe + " a " + r.vigenteAte : "desde " + r.vigenteDe)
    }

    function abrirRegra(r) {
        editando = r
        formRegra.travada = !!(r && r.jaValeu)
        formRegra.preencher(r || { uf: ufRegras, comSt: false })
        painelRegra.open()
        formRegra.focar()
    }

    function respostaDasRegras(r, mensagem, depois) {
        if (r.ok) {
            preencher(r.dados.conferencia || r.dados)
            janela.avisar(mensagem, "ok")
            if (depois)
                depois()
            return
        }
        const alvo = r.campo ? formRegra.campo(r.campo) : null
        if (alvo && painelRegra.opened) {
            alvo.erro = r.erro
            if (alvo.focar)
                alvo.focar()
        }
        janela.avisar(r.erro, "erro")
    }

    function salvarRegra() {
        const dados = formRegra.valores()
        dados.id = editando ? editando.id : ""
        janela.chamarGerente("fiscal_regra_salvar", dados, r => {
            respostaDasRegras(r, "Regra de ST salva.", () => {
                tela.ufRegras = dados.uf
                painelRegra.close()
            })
        })
    }

    function encerrarRegra(r) {
        janela.confirmar("Encerrar esta regra?",
                         r.uf + ", CEST " + r.cestInicio + ": a regra passa a valer só até ontem e fica no histórico, como encerrada. "
                         + "As vendas de antes continuam com a regra que valia. Confirme com o contador antes.",
                         "Encerrar", () => {
            janela.chamarGerente("fiscal_regra_encerrar", { id: r.id }, x => respostaDasRegras(x, "Regra encerrada."))
        })
    }

    function excluirRegra(r) {
        janela.confirmar("Excluir esta regra futura?",
                         r.uf + ", CEST " + r.cestInicio + ", que começaria em " + r.vigenteDe + ". Ela ainda não valeu, então pode sair.",
                         "Excluir", () => {
            janela.chamarGerente("fiscal_regra_excluir", { id: r.id }, x => respostaDasRegras(x, "Regra excluída."))
        }, "perigo")
    }

    function aoEsc() {
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
                id: faixaDaLista
                required property var modelData
                required property int index
                Layout.fillWidth: true
                // A15 (0.9.18): no celular, cada linha com 48 dp (Tema.toqueMinimo).
                implicitHeight: Math.max(40, Tema.toqueMinimo)
                radius: Tema.raioCampo
                // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                readonly property bool par: (index + 1) % 2 === 0
                color: sobre.hovered ? (Tema.roxo ? Qt.darker(Tema.faixa(faixaDaLista.par), 1.04) : Tema.vela(0.6)) : Tema.faixa(faixaDaLista.par, Tema.vela(0.35), "transparent")
                palette.window: Tema.superficie(color)
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12
                    Text {
                        text: modelData.codigo
                        textFormat: Text.PlainText
                        color: Tema.tintaSuaveDaFaixa(faixaDaLista.par)
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                        visible: !Tema.estreito
                        Layout.preferredWidth: 60
                    }
                    Text {
                        text: modelData.nome + (modelData.marca ? "  ·  " + modelData.marca : "")
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(faixaDaLista.par)
                        font.family: Tema.texto
                        font.pixelSize: 15
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Text {
                        text: listaPecas.mostrarCsosn ? "CSOSN " + modelData.csosn + "  →  " + modelData.sugerido
                                                      : (modelData.ncm ? "NCM " + modelData.ncm : "")
                        textFormat: Text.PlainText
                        color: listaPecas.mostrarCsosn ? Tema.vermelhoDaFaixa(faixaDaLista.par) : Tema.tintaSuaveDaFaixa(faixaDaLista.par)
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 14
                    }
                    Icone { nome: "direita"; tamanho: 16; cor: Tema.apagadoDaFaixa(faixaDaLista.par) }
                }
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler {
                    onTapped: {
                        if (tela.janela)
                            tela.janela.encerrarValidacaoProgressiva(tela)
                        tela.janela.abrir("TelaPeca", { produtoId: modelData.id })
                    }
                }
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

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: tela.embutida ? 0 : Tema.margem
        anchors.topMargin: tela.embutida ? 0 : Tema.margem - 12
        anchors.bottomMargin: 0
        spacing: 12

        Nota {
            visible: tela.embutida && !!tela.dados
            text: !tela.dados ? "" : (tela.dados.uf ? "Loja em " + tela.dados.uf : "Estado da loja não informado")
                                     + " · regras válidas em " + tela.dados.data.split("-").reverse().join("/")
                                     + " · " + tela.dados.totalPecas + " peças ativas"
            Layout.fillWidth: true
        }

        Cabecalho {
            visible: !tela.embutida
            titulo: "Fiscal"
            subtitulo: !tela.dados ? "" : (tela.dados.uf ? "Loja em " + tela.dados.uf : "Estado da loja não informado")
                                         + " · regras válidas em " + tela.dados.data.split("-").reverse().join("/")
                                         + " · " + tela.dados.totalPecas + " peças ativas"
            Layout.fillWidth: true
        }

        Flickable {
            id: rolagem
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: tudo.implicitHeight + 6 + Tema.margem
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            ColumnLayout {
                id: tudo
                width: rolagem.width - 14
                y: 6
                spacing: 14

            // Navegação guiada (0.9.17): leva aos campos do estado e do
            // CNPJ em Configurações › Loja e volta aqui quando o
            // núcleo confirmar os dois salvos.
            Faixa {
                visible: !!tela.dados && !!tela.dadosDaLoja && !tela.dadosDaLoja.satisfeito
                titulo: "Para continuar, informe a UF e o CNPJ da loja"
                color: Tema.rosa
                corTitulo: Tema.vermelho
                Layout.fillWidth: true
                Nota {
                    text: (tela.dadosDaLoja ? tela.dadosDaLoja.falta + " " : "")
                          + "A substituição tributária muda de estado para estado, e o CNPJ confere se a nota de compra é mesmo para a loja."
                    Layout.fillWidth: true
                }
                Botao {
                    text: "Ir para Loja"
                    tipo: "secundario"
                    onClicked: tela.janela.guiar("requisito:loja.uf_cnpj", "", ok => tela.carregar())
                }
            }

            // FIS-LIGA: o seletor, só para o gerente.
            Faixa {
                id: faixaLiga
                visible: !!tela.liga && tela.liga.podeMudar
                titulo: "Certificado A3 e integração fiscal"
                color: tela.liga && tela.liga.ligado ? Tema.verde : Tema.papel
                corTitulo: tela.liga && tela.liga.ligado ? Tema.verdeEscuro : Tema.apagado
                Layout.fillWidth: true
                Text {
                    text: tela.liga && tela.liga.ligado ? "Ligado neste computador" : "Desligado neste computador"
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.titulo
                    font.weight: Font.Bold
                    font.pixelSize: 24
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Nota {
                    text: tela.liga && tela.liga.ligado
                          ? "O Rustinho pode usar o certificado A3 e o emissor fiscal deste computador. Qualquer gerente desliga."
                          : "Desligado, o Rustinho não usa o certificado A3 nem fala com o emissor fiscal. Vender, comprar e trazer o XML das notas funcionam igual."
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                Botao {
                    text: tela.liga && tela.liga.ligado ? "Desligar" : "Ligar"
                    icone: tela.liga && tela.liga.ligado ? "" : "check"
                    tipo: tela.liga && tela.liga.ligado ? "secundario" : "principal"
                    onClicked: {
                        if (tela.liga && tela.liga.ligado) {
                            tela.desligarFiscal()
                        } else {
                            senhaLiga.texto = ""
                            senhaLiga.erro = ""
                            painelLiga.open()
                            senhaLiga.focar()
                        }
                    }
                }
            }

            // A emissão de notas (0.9.15: o estado real, sem nada inventado).
            EmissaoFiscal {
                liga: tela.liga
                dadosDaLoja: tela.dadosDaLoja
                dados: tela.dados
                Layout.fillWidth: true
            }

            Linha {
                id: corpo
                Layout.fillWidth: true
                spacing: 18

                ColumnLayout {
                    spacing: 14
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop

                    Faixa {
                        id: faixaConflitos
                        readonly property int n: tela.dados ? tela.dados.conflitos.length : 0
                        titulo: "CSOSN que não combina com a regra"
                        color: n > 0 ? Tema.rosa : Tema.verde
                        corTitulo: n > 0 ? Tema.vermelho : Tema.verdeEscuro
                        Layout.fillWidth: true
                        Text {
                            text: faixaConflitos.n === 0 ? "Nenhuma peça com CSOSN errado" : faixaConflitos.n + (faixaConflitos.n === 1 ? " peça com CSOSN para conferir" : " peças com CSOSN para conferir")
                            textFormat: Text.PlainText
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
                        subtitulo: "pendência: a nota fiscal não sai sem NCM"
                        color: Tema.pergaminho
                        Layout.fillWidth: true
                        ListaPecas { pecas: tela.dados ? tela.dados.semNcm : [] }
                    }

                    Faixa {
                        readonly property int n: tela.dados ? tela.dados.semCest.length : 0
                        visible: n > 0
                        titulo: "Peças sem CEST: a analisar"
                        subtitulo: "o NCM está na lista oficial de autopeças ou pneus"
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
                                  + " É uma informação do cadastro, não um erro: as vendas dessas peças saem separadas no relatório para o contador (PGDAS-D). "
                                  + "Nas compras pelo XML, peças novas com PIS CST 04 já vêm marcadas. "
                                  + "O PIS/Cofins monofásico das autopeças e dos pneus vale até 31/12/2026 (Lei Complementar 214/2025, art. 542)."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                    }
                }

                ColumnLayout {
                    spacing: 14
                    // H21 (7.2): no celular, sem largura mínima maior que a tela.
                    Layout.preferredWidth: Tema.fino ? -1 : Math.min(500, Math.max(420, corpo.width * 0.36))
                    Layout.minimumWidth: Tema.fino ? 0 : 420
                    Layout.maximumWidth: Tema.fino ? Number.POSITIVE_INFINITY : 500
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
                        Selecao {
                            rotulo: "Estado"
                            modelo: tela.ufs
                            indice: Math.max(0, tela.ufs.indexOf(tela.ufRegras))
                            Layout.preferredWidth: 140
                            onEscolhido: tela.ufRegras = tela.ufs[indice]
                        }
                        Nota {
                            visible: tela.regrasDaUf.length === 0
                            text: "Nenhuma regra para " + tela.ufRegras + ". Sem regra, o Rustinho não presume se a peça tem ST: confirme com o contador."
                            font.pixelSize: 14
                            Layout.fillWidth: true
                        }
                        Repeater {
                            model: tela.ordemDasSituacoes
                            delegate: ColumnLayout {
                                id: grupoSituacao
                                required property string modelData
                                readonly property var lista: tela.regrasDaUf.filter(r => r.situacao === modelData)
                                visible: lista.length > 0
                                spacing: 4
                                Layout.fillWidth: true
                                Layout.topMargin: 6
                                Rotulo { text: tela.nomeDaSituacao(grupoSituacao.modelData) }
                                Repeater {
                                    model: grupoSituacao.lista
                                    delegate: Rectangle {
                                        id: cartaoRegra
                                        required property var modelData
                                        required property int index
                                        readonly property bool par: (index + 1) % 2 === 0
                                        Layout.fillWidth: true
                                        radius: Tema.raioCampo
                                        color: Tema.faixa(cartaoRegra.par, Tema.vela(0.35), "transparent")
                                        palette.window: Tema.superficie(color)
                                        implicitHeight: colunaRegra.implicitHeight + 20
                                        ColumnLayout {
                                            id: colunaRegra
                                            x: 12
                                            y: 10
                                            width: parent.width - 24
                                            spacing: 4
                                            Text {
                                                text: "CEST " + cartaoRegra.modelData.cestInicio + " · "
                                                      + (cartaoRegra.modelData.comSt ? "com ST" : "sem ST") + " · " + tela.periodo(cartaoRegra.modelData)
                                                textFormat: Text.PlainText
                                                color: Tema.tintaDaFaixa(cartaoRegra.par)
                                                font.family: Tema.texto
                                                font.weight: Font.DemiBold
                                                font.pixelSize: 15
                                                wrapMode: Text.WordWrap
                                                Layout.fillWidth: true
                                            }
                                            Text {
                                                visible: cartaoRegra.modelData.descricao !== ""
                                                text: cartaoRegra.modelData.descricao
                                                textFormat: Text.PlainText
                                                color: Tema.tintaDaFaixa(cartaoRegra.par)
                                                font.family: Tema.texto
                                                font.pixelSize: 14
                                                wrapMode: Text.WordWrap
                                                Layout.fillWidth: true
                                            }
                                            Text {
                                                text: cartaoRegra.modelData.fonte === ""
                                                      ? "Sem fonte informada"
                                                      : "Fonte: " + cartaoRegra.modelData.fonte
                                                        + (cartaoRegra.modelData.dispositivo ? " · " + cartaoRegra.modelData.dispositivo : "")
                                                        + (cartaoRegra.modelData.consultadaEm ? " · lida em " + cartaoRegra.modelData.consultadaEm : "")
                                                textFormat: Text.PlainText
                                                color: Tema.tintaSuaveDaFaixa(cartaoRegra.par)
                                                font.family: Tema.texto
                                                font.pixelSize: 13
                                                wrapMode: Text.WordWrap
                                                Layout.fillWidth: true
                                            }
                                            Text {
                                                visible: cartaoRegra.modelData.origem === "loja"
                                                text: "Mudada na loja"
                                                textFormat: Text.PlainText
                                                color: Tema.tintaSuaveDaFaixa(cartaoRegra.par)
                                                font.family: Tema.texto
                                                font.italic: true
                                                font.pixelSize: 13
                                            }
                                            Flow {
                                                spacing: 8
                                                Layout.fillWidth: true
                                                Botao {
                                                    text: "Mudar"
                                                    tipo: "discreto"
                                                    onClicked: tela.abrirRegra(cartaoRegra.modelData)
                                                }
                                                Botao {
                                                    visible: cartaoRegra.modelData.situacao === "vigente"
                                                    text: "Encerrar"
                                                    tipo: "discreto"
                                                    onClicked: tela.encerrarRegra(cartaoRegra.modelData)
                                                }
                                                Botao {
                                                    visible: cartaoRegra.modelData.situacao === "futura"
                                                    text: "Excluir"
                                                    tipo: "discreto"
                                                    onClicked: tela.excluirRegra(cartaoRegra.modelData)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Botao {
                            text: "Nova regra"
                            icone: "mais"
                            tipo: "secundario"
                            onClicked: tela.abrirRegra(null)
                        }
                    }
                }
            }
            }
        }
    }

    StackView.onActivated: {
        if (!janela || !janela.retornandoDeF11)
            carregar()
    }

    // FIS-LIGA: ligar pede a senha de um gerente, digitada agora (a senha
    // digitada antes, no "aperto", não vale), e diz o que muda.
    Painel {
        id: painelLiga
        width: Math.min(480, Tema.largura - 24)
        onClosed: senhaLiga.texto = ""
        contentItem: ColumnLayout {
            spacing: 14
            Titulo { text: "Ligar o fiscal neste computador"; font.pixelSize: 26; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Nota {
                text: "Com o fiscal ligado, o Rustinho passa a poder usar o certificado A3 e o emissor fiscal deste computador. Nos outros caixas, nada muda. Para ligar, digite a senha de um gerente."
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            Campo {
                id: senhaLiga
                rotulo: "Senha do gerente"
                senha: true
                maximo: 256
                Layout.fillWidth: true
                onConfirmado: tela.ligarFiscal()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Ligar"; icone: "check"; onClicked: tela.ligarFiscal() }
            }
        }
    }

    // FIS-P5: mudar uma regra (ou criar outra) num painel à parte. Mudar pede a
    // senha do gerente (janela.chamarGerente), como antes.
    Painel {
        id: painelRegra
        width: Math.min(620, Tema.largura - 24)
        contentItem: ColumnLayout {
            spacing: 14
            Titulo {
                text: tela.editando ? "Mudar a regra de ST" : "Nova regra de ST"
                font.pixelSize: 26
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Nota {
                text: tela.editando && tela.editando.jaValeu
                      ? "Esta regra já valeu: o estado, o CEST, o início e o \"com ST\" ficam como estão. Para mudar, encerre esta e crie outra. Mude só com o contador."
                      : "Vale a regra com o começo de CEST mais longo. Duas regras do mesmo estado e CEST não podem valer no mesmo dia. Mude só com o contador."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            LinhaRegraSt {
                id: formRegra
                ufs: tela.ufs
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao { text: "Salvar"; icone: "check"; onClicked: tela.salvarRegra() }
            }
        }
    }
}
