import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// FIS-P1 e FIS-P2 (0.9.19, etapa 3, pacote E; texto A §12 e §13): o painel das
// notas emitidas pelo Rustinho neste computador e o detalhe de cada uma.
// - O ambiente (homologação ou produção) fica sempre à vista.
// - O certificado aparece em 4 estados: configurado, conectado, válido, testado.
// - Os 4 indicadores filtram (sem repetir pílulas de situação); a pesquisa, o
//   período e o tipo ficam simples, e "Limpar filtros" volta a tudo.
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
    spacing: 12

    readonly property var periodos: ["Todo o período", "Hoje", "Últimos 7 dias", "Últimos 30 dias"]
    readonly property var modelos: ["NFC-e e NF-e", "Só NFC-e (modelo 65)", "Só NF-e (modelo 55)"]
    readonly property var indicadores: [
        { id: "autorizadas", nome: "Autorizadas" },
        { id: "rejeitadas", nome: "Rejeitadas" },
        { id: "desconhecidas", nome: "Sem resposta" },
        { id: "pendentes", nome: "Pendentes" }
    ]

    function dia(deslocamento) {
        const d = new Date()
        d.setDate(d.getDate() + deslocamento)
        return d.toISOString().slice(0, 10)
    }

    function carregar() {
        const de = periodo === 1 ? dia(0) : periodo === 2 ? dia(-6) : periodo === 3 ? dia(-29) : ""
        const r = Api.chamar("nota_painel", { indicador: indicador, texto: texto, de: de, ate: "",
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

    function nomeDoEstado(e) {
        return ({ rascunho: "Rascunho", numerada: "Número reservado", enviada: "Enviada, sem resposta ainda",
                  autorizada: "Autorizada", rejeitada: "Rejeitada", denegada: "Denegada",
                  desconhecida: "Sem resposta (consultar)", cancelada: "Cancelada", inutilizada: "Inutilizada" })[e] || e
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
        }
        Text {
            text: "Certificado: "
                  + (secao.liga && secao.liga.ligado ? "liberado neste computador" : "desligado neste computador")
                  + " · configurado: ainda não · conectado: ainda não · válido: ainda não · testado: ainda não"
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
            }
        }
    }
}
