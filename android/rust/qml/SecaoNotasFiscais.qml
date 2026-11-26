import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// FIS-P1 e FIS-P2 (0.9.19, etapa 3, pacote E; texto A §12 e §13): o painel das
// notas emitidas pelo Rustinho neste computador e o detalhe de cada uma.
// - O ambiente (homologação ou produção) fica sempre à vista.
// - O certificado aparece em 4 estados: configurado, conectado, válido, testado.
// - Os 4 indicadores do texto A §12 (Autorizadas, Pendentes, Rejeitadas e
//   Canceladas) filtram, sem repetir pílulas de situação; as sem resposta
//   conclusiva têm um aviso à parte (pedem consulta). A pesquisa, o período
//   (no dia deste computador) e o tipo ficam simples, e "Limpar filtros" volta
//   a tudo.
// - CPF e CNPJ ficam escondidos pelo olho ("Mostrar valores").
// - O detalhe mostra primeiro a explicação humana; o código e a mensagem da
//   SEFAZ vêm na íntegra, para copiar; "sem resposta" fica separado de
//   "rejeitada", e não há reenvio automático.
// Nesta etapa o Rustinho ainda não emite (o emissor é a T008): a lista fica vazia
// até lá, e nada aqui finge nota.
ColumnLayout {
    id: secao
    readonly property var janela: ApplicationWindow.window
    property var painel: null
    property var liga: null
    property string indicador: ""
    property string texto: ""
    property int periodo: 0
    property int modelo: 0
    property var detalhe: null
    // FIS-P3: os impedimentos da troca de ambiente.
    property var troca: null
    spacing: 12

    readonly property var periodos: ["Todo o período", "Hoje", "Últimos 7 dias", "Últimos 30 dias"]
    readonly property var modelos: ["NFC-e e NF-e", "Só NFC-e (modelo 65)", "Só NF-e (modelo 55)"]
    readonly property var indicadores: [
        { id: "autorizadas", nome: "Autorizadas" },
        { id: "pendentes", nome: "Pendentes" },
        { id: "rejeitadas", nome: "Rejeitadas" },
        { id: "canceladas", nome: "Canceladas" }
    ]
    // B2 da R1: o período vai em dias; o núcleo conta no dia deste computador.
    readonly property var diasDoPeriodo: [0, 1, 7, 30]

    function carregar() {
        const r = Api.chamar("nota_painel", { indicador: indicador, texto: texto, dias: diasDoPeriodo[periodo],
                                              modelo: modelo === 1 ? 65 : modelo === 2 ? 55 : 0 })
        if (r.ok) {
            painel = r.dados.painel
            liga = r.dados.liga
        } else if (janela) {
            janela.avisar(r.erro, "erro")
        }
    }

    function limpar() {
        indicador = ""
        texto = ""
        campoBusca.texto = ""
        periodo = 0
        modelo = 0
        carregar()
    }

    function abrir(id) {
        const r = Api.chamar("nota_detalhe", { id: id })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        detalhe = r.dados
        painelDetalhe.open()
    }

    function abrirTroca() {
        const r = Api.chamar("fiscal_ambiente_impedimentos")
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        troca = r.dados
        confirmaProducao.texto = ""
        senhaTroca.texto = ""
        painelTroca.open()
    }

    function trocar() {
        const para = troca.ambiente === 1 ? 2 : 1
        const r = Api.chamar("fiscal_ambiente_trocar", { para: para, confirmacao: confirmaProducao.texto, senha: senhaTroca.texto })
        senhaTroca.texto = ""
        if (!r.ok) {
            if (r.campo === "confirmacao")
                confirmaProducao.erro = r.erro
            else if (r.campo === "senha")
                senhaTroca.erro = r.erro
            janela.avisar(r.erro, "erro")
            return
        }
        painelTroca.close()
        janela.avisar("Ambiente fiscal trocado.", "ok")
        carregar()
    }

    function nomeDoEstado(e) {
        return ({ rascunho: "Rascunho", enviada: "Enviada, sem resposta ainda",
                  autorizada: "Autorizada", rejeitada: "Rejeitada", denegada: "Denegada",
                  desconhecida: "Sem resposta (consultar)", nao_recebida: "Não chegou à SEFAZ",
                  cancelada: "Cancelada", descartada: "Descartada" })[e] || e
    }

    function descartar() {
        const id = detalhe.nota.id
        const n = detalhe.nota.numero
        janela.confirmar("Descartar esta nota?",
                         n > 0 ? "Ela não chegou a valer na SEFAZ. O número " + n + " fica para a próxima nota."
                               : "Ela ainda não foi transmitida.",
                         "Descartar", () => janela.chamarGerente("nota_descartar", { id: id }, r => {
                             if (r.ok) {
                                 detalhe = r.dados
                                 carregar()
                             } else {
                                 // K11 da R2: o erro do descarte aparece (a nota pode ter mudado de estado).
                                 janela.avisar(r.erro, "erro")
                             }
                         }), "perigo")
    }

    function documento(doc) {
        if (!doc)
            return "consumidor sem documento"
        return Tema.oculto ? "documento escondido" : doc
    }

    function numero(n) {
        return n > 0 ? String(n) : "sem número"
    }

    // O histórico é gravado em UTC; a tela mostra a hora deste computador.
    function hora(utc) {
        const d = new Date(utc)
        return isNaN(d.getTime()) ? utc : d.toLocaleString(Qt.locale("pt_BR"), "dd/MM/yyyy HH:mm")
    }

    function reais(centavos) {
        return "R$ " + (centavos / 100).toLocaleString(Qt.locale("pt_BR"), "f", 2)
    }

    Component.onCompleted: carregar()

    Faixa {
        titulo: "Notas emitidas pelo Rustinho"
        color: Tema.pergaminho
        Layout.fillWidth: true
        espacamento: 10

        Flow {
            spacing: 10
            Layout.fillWidth: true
            Rectangle {
                radius: Tema.raioCampo
                color: secao.painel && secao.painel.ambiente === 1 ? Tema.rosa : Tema.papel
                implicitWidth: textoAmbiente.implicitWidth + 28
                implicitHeight: 36
                Text {
                    id: textoAmbiente
                    anchors.centerIn: parent
                    text: !secao.painel ? "" : secao.painel.ambiente === 1 ? "Ambiente: PRODUÇÃO (notas com valor fiscal)"
                                                                         : "Ambiente: homologação (teste, sem valor fiscal)"
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.weight: Font.Bold
                    font.pixelSize: 14
                }
            }
            Botao {
                visible: !!secao.liga && secao.liga.podeMudar
                text: "Trocar de ambiente"
                tipo: "discreto"
                onClicked: secao.abrirTroca()
            }
        }
        Text {
            // B4 da R1: os 4 estados do certificado numa frase só, sem repetir "ainda não".
            text: "Certificado: "
                  + (secao.liga && secao.liga.ligado ? "liberado neste computador" : "desligado neste computador")
                  + ". Configurado, conectado, válido e testado: os quatro chegam junto com o emissor."
            textFormat: Text.PlainText
            color: Tema.tinta
            font.family: Tema.texto
            font.pixelSize: 14
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
        Flow {
            spacing: 10
            Layout.fillWidth: true
            Repeater {
                model: secao.indicadores
                delegate: Botao {
                    required property var modelData
                    readonly property int n: !secao.painel ? 0 : secao.painel[modelData.id]
                    text: modelData.nome + ": " + n
                    tipo: secao.indicador === modelData.id ? "principal" : "secundario"
                    onClicked: {
                        secao.indicador = secao.indicador === modelData.id ? "" : modelData.id
                        secao.carregar()
                    }
                }
            }
        }
        Botao {
            visible: !!secao.painel && secao.painel.semResposta > 0
            // K10 da R2: a consulta na SEFAZ chega com o emissor; até lá, o aviso só mostra quais são.
            text: !secao.painel ? "" : (secao.painel.semResposta === 1 ? "1 nota sem resposta da SEFAZ: ver qual é"
                                                                         : secao.painel.semResposta + " notas sem resposta da SEFAZ: ver quais são")
            tipo: secao.indicador === "sem_resposta" ? "principal" : "secundario"
            onClicked: {
                secao.indicador = secao.indicador === "sem_resposta" ? "" : "sem_resposta"
                secao.carregar()
            }
        }
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Campo {
                id: campoBusca
                rotulo: "Procurar (número, nome ou chave)"
                maximo: 60
                Layout.fillWidth: true
                onEditado: {
                    secao.texto = texto
                    secao.carregar()
                }
            }
            Selecao {
                rotulo: "Período"
                modelo: secao.periodos
                indice: secao.periodo
                Layout.preferredWidth: 180
                onEscolhido: {
                    secao.periodo = indice
                    secao.carregar()
                }
            }
            Selecao {
                rotulo: "Tipo"
                modelo: secao.modelos
                indice: secao.modelo
                Layout.preferredWidth: 220
                onEscolhido: {
                    secao.modelo = indice
                    secao.carregar()
                }
            }
        }
        RowLayout {
            spacing: 10
            Layout.fillWidth: true
            Nota {
                text: !secao.painel ? "" : (secao.painel.total === 1 ? "1 nota" : secao.painel.total + " notas")
                                          + " · " + secao.periodos[secao.periodo].toLowerCase()
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Botao {
                visible: secao.indicador !== "" || secao.texto !== "" || secao.periodo !== 0 || secao.modelo !== 0
                text: "Limpar filtros"
                tipo: "discreto"
                onClicked: secao.limpar()
            }
        }
        Nota {
            visible: !!secao.painel && secao.painel.total === 0
            text: "Nenhuma nota emitida pelo Rustinho aqui. Por enquanto as notas continuam no sistema de hoje."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
        Repeater {
            model: secao.painel ? secao.painel.notas : []
            delegate: Rectangle {
                id: linhaNota
                required property var modelData
                required property int index
                readonly property bool par: (index + 1) % 2 === 0
                Layout.fillWidth: true
                implicitHeight: Math.max(48, colunaNota.implicitHeight + 16)
                radius: Tema.raioCampo
                color: sobreNota.hovered ? Tema.vela(0.6) : Tema.faixa(linhaNota.par, Tema.vela(0.35), "transparent")
                palette.window: Tema.superficie(color)
                ColumnLayout {
                    id: colunaNota
                    x: 12
                    y: 8
                    width: parent.width - 24
                    spacing: 2
                    Text {
                        text: (linhaNota.modelData.modelo === 65 ? "NFC-e" : "NF-e") + " " + secao.numero(linhaNota.modelData.numero)
                              + " (série " + linhaNota.modelData.serie + ") · " + secao.nomeDoEstado(linhaNota.modelData.estado)
                              + " · " + secao.reais(linhaNota.modelData.valor)
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(linhaNota.par)
                        font.family: Tema.texto
                        font.weight: Font.DemiBold
                        font.pixelSize: 15
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Text {
                        text: (linhaNota.modelData.destinatarioNome || "Consumidor") + " · " + secao.documento(linhaNota.modelData.destinatarioDoc)
                        textFormat: Text.PlainText
                        color: Tema.tintaSuaveDaFaixa(linhaNota.par)
                        font.family: Tema.texto
                        font.pixelSize: 13
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
                HoverHandler { id: sobreNota; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: secao.abrir(linhaNota.modelData.id) }
            }
        }
    }

    // FIS-P2: o detalhe da nota.
    Painel {
        id: painelDetalhe
        width: Math.min(680, Tema.largura - 24)
        contentItem: Flickable {
            implicitHeight: Math.min(colunaDetalhe.implicitHeight, painelDetalhe.alturaMaxima - 40)
            contentHeight: colunaDetalhe.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ColumnLayout {
                id: colunaDetalhe
                width: parent.width
                spacing: 12
                Titulo {
                    text: !secao.detalhe ? "" : (secao.detalhe.nota.modelo === 65 ? "NFC-e " : "NF-e ") + secao.numero(secao.detalhe.nota.numero)
                                                + " · " + secao.nomeDoEstado(secao.detalhe.nota.estado)
                    font.pixelSize: 26
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Text {
                    text: secao.detalhe ? secao.detalhe.explicacao : ""
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.pixelSize: 17
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Nota {
                    visible: !!secao.detalhe && secao.detalhe.pedeConsulta
                    text: "A consulta na SEFAZ chega junto com o emissor. Até lá, não emita esta nota de novo por outro sistema sem conferir no portal da SEFAZ."
                    font.pixelSize: 14
                    Layout.fillWidth: true
                }
                Rotulo { visible: !!secao.detalhe && secao.detalhe.cstat !== ""; text: "Código e mensagem da SEFAZ" }
                Rectangle {
                    visible: !!secao.detalhe && secao.detalhe.cstat !== ""
                    radius: Tema.raioCampo
                    color: Tema.papel
                    Layout.fillWidth: true
                    implicitHeight: mensagemSefaz.implicitHeight + 20
                    TextEdit {
                        id: mensagemSefaz
                        x: 10
                        y: 10
                        width: parent.width - 20
                        text: secao.detalhe ? secao.detalhe.cstat + " - " + secao.detalhe.motivo : ""
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 15
                    }
                }
                Botao {
                    visible: !!secao.detalhe && secao.detalhe.cstat !== ""
                    text: "Copiar a mensagem"
                    tipo: "secundario"
                    onClicked: {
                        mensagemSefaz.selectAll()
                        mensagemSefaz.copy()
                        mensagemSefaz.deselect()
                        secao.janela.avisar("Copiado.", "ok")
                    }
                }
                Text {
                    visible: !!secao.detalhe && secao.detalhe.xmlNome !== ""
                    text: secao.detalhe ? "Arquivo XML: " + secao.detalhe.xmlNome : ""
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.pixelSize: 13
                    wrapMode: Text.WrapAnywhere
                    Layout.fillWidth: true
                }
                Text {
                    visible: !!secao.detalhe && secao.detalhe.chave !== ""
                    text: secao.detalhe ? "Chave: " + secao.detalhe.chave + (secao.detalhe.protocolo ? " · protocolo " + secao.detalhe.protocolo : "") : ""
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.pixelSize: 13
                    wrapMode: Text.WrapAnywhere
                    Layout.fillWidth: true
                }
                Rotulo { text: "Histórico" }
                Repeater {
                    model: secao.detalhe ? secao.detalhe.eventos : []
                    delegate: Rectangle {
                        id: linhaEvento
                        required property var modelData
                        required property int index
                        readonly property bool par: (index + 1) % 2 === 0
                        Layout.fillWidth: true
                        implicitHeight: textoEvento.implicitHeight + 12
                        radius: Tema.raioCampo
                        color: Tema.faixa(linhaEvento.par, Tema.vela(0.35), "transparent")
                        palette.window: Tema.superficie(color)
                        Text {
                            id: textoEvento
                            x: 10
                            y: 6
                            width: parent.width - 20
                            text: secao.hora(linhaEvento.modelData.quando) + " · "
                                  + secao.nomeDoEstado(linhaEvento.modelData.para) + " · " + linhaEvento.modelData.detalhe
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linhaEvento.par)
                            font.family: Tema.texto
                            font.pixelSize: 13
                            wrapMode: Text.WordWrap
                        }
                    }
                }
                Botao {
                    visible: !!secao.detalhe && secao.detalhe.podeDescartar
                    text: "Descartar esta nota"
                    tipo: "perigo"
                    Layout.topMargin: 8
                    onClicked: secao.descartar()
                }
            }
        }
    }

    // FIS-P3: a troca de ambiente. Com qualquer impedimento, a confirmação e a
    // senha ficam desligadas: digitar PRODUÇÃO e a senha nunca contornam um
    // bloqueio (texto A §14).
    Painel {
        id: painelTroca
        width: Math.min(560, Tema.largura - 24)
        readonly property bool livre: !!secao.troca && secao.troca.impedimentos.length === 0
        readonly property bool paraProducao: !!secao.troca && secao.troca.ambiente === 2
        contentItem: ColumnLayout {
            spacing: 12
            Titulo {
                text: painelTroca.paraProducao ? "Ir para a produção" : "Voltar para a homologação"
                font.pixelSize: 26
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Nota {
                text: painelTroca.paraProducao
                      ? "Na produção, as notas têm valor fiscal. Filas, números e histórico de cada ambiente ficam separados."
                      : "Na homologação, as notas são de teste, sem valor fiscal. Filas, números e histórico de cada ambiente ficam separados."
                font.pixelSize: 14
                Layout.fillWidth: true
            }
            Rotulo { visible: !painelTroca.livre; text: "Ainda não dá para trocar" }
            Repeater {
                model: secao.troca ? secao.troca.impedimentos : []
                delegate: Text {
                    required property string modelData
                    text: "• " + modelData
                    textFormat: Text.PlainText
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.pixelSize: 15
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }
            Campo {
                id: confirmaProducao
                visible: painelTroca.paraProducao
                enabled: painelTroca.livre
                rotulo: "Para confirmar, digite PRODUÇÃO"
                maximo: 20
                Layout.fillWidth: true
            }
            Campo {
                id: senhaTroca
                visible: painelTroca.paraProducao
                enabled: painelTroca.livre
                rotulo: "Senha do gerente"
                senha: true
                maximo: 256
                Layout.fillWidth: true
                onConfirmado: secao.trocar()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 10
                Botao {
                    text: "Trocar"
                    icone: "check"
                    enabled: painelTroca.livre
                    onClicked: secao.trocar()
                }
            }
        }
    }
}
