import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Ordens de serviço: as abertas (orçamento e em serviço), as prontas para
// entregar, as entregues e todas.
FocusScope {
    id: tela
    // Como esta tela aparece na orientação da navegação guiada ("Depois o
    // Rustinho volta para …").
    readonly property string nomeNaNavegacao: "Ordens de serviço"
    readonly property var janela: ApplicationWindow.window
    property string quais: "abertas"
    property var itens: []
    property var contagem: ({ abertas: 0, prontas: 0, atrasadas: 0 })
    property bool buscou: false

    function carregar() {
        const r = Api.chamar("os_listar", { quais: quais, texto: busca.texto })
        if (r.ok) {
            itens = r.dados.itens
            contagem = r.dados.contagem
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }

    // O Esc e a seta fazem o mesmo (D-V3, decidida pelo dono em 07/11/2026:
    // com a busca preenchida, os dois voltam direto). limparBuscaAntes só
    // limparia a busca antes se a regra mudasse em Main.
    function aoEsc(daSeta) {
        if (busca.texto !== "" && janela.limparBuscaAntes(daSeta)) {
            busca.texto = ""
            carregar()
        } else {
            janela.voltar()
        }
    }
    function focarBusca() {
        busca.focar()
    }

    onQuaisChanged: carregar()
    Timer { id: espera; interval: 200; onTriggered: tela.carregar() }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.janela.abrir("TelaOrdem", {})
    }

    readonly property var cores: ({
        rascunho: [Tema.amarelo, Tema.tinta],
        orcamento: [Tema.papel, Tema.tintaSuave],
        aprovada: [Tema.azul, Tema.azulEscuro],
        pronta: [Tema.verde, Tema.verdeEscuro],
        entregue: [Tema.vela(0.45), Tema.apagado],
        cancelada: [Tema.rosa, Tema.vermelho]
    })

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            titulo: "Ordens de serviço"
            subtitulo: tela.contagem.prontas > 0
                       ? tela.contagem.prontas + (tela.contagem.prontas === 1 ? " pronta para entregar" : " prontas para entregar")
                         + (tela.contagem.atrasadas > 0 ? " · " + tela.contagem.atrasadas + " passaram da previsão" : "")
                       : "O carro, as peças e a mão de obra, do orçamento à entrega"
            Layout.fillWidth: true
            Botao {
                text: "Nova OS"
                icone: "mais"
                atalho: "F2"
                onClicked: tela.janela.abrir("TelaOrdem", {})
            }
        }

        Linha {
            spacing: 14
            Layout.fillWidth: true
            Pilulas {
                modelo: [
                    { id: "abertas", nome: "Abertas" + (tela.contagem.abertas ? " · " + tela.contagem.abertas : "") },
                    { id: "prontas", nome: "Prontas" + (tela.contagem.prontas ? " · " + tela.contagem.prontas : "") },
                    { id: "entregues", nome: "Entregues" },
                    { id: "todas", nome: "Todas" }
                ]
                atual: tela.quais
                onEscolhido: id => tela.quais = id
            }
            Campo {
                id: busca
                icone: "busca"
                altura: 46
                tamanhoFonte: 16
                dica: "Placa, cliente, carro ou número da OS"
                Layout.fillWidth: true
                onEditado: espera.restart()
                onConfirmado: if (tela.itens.length === 1) tela.janela.abrir("TelaOrdem", { osId: tela.itens[0].id })
            }
        }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: tela.itens
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: linha
                required property var modelData
                required property int index
                readonly property var o: modelData
                readonly property var cor: tela.cores[o.situacao] || [Tema.pergaminho, Tema.tinta]
                width: lista.width
                implicitHeight: 70
                radius: Tema.raio
                // Faixas alternadas (V-6): no roxo, clara e escura com o texto invertido.
                readonly property bool par: (index + 1) % 2 === 0
                color: sobre.hovered ? Qt.darker(Tema.faixa(linha.par), 1.04) : Tema.faixa(linha.par)
                palette.window: Tema.superficie(color)
                opacity: o.situacao === "cancelada" ? 0.6 : 1
                HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                TapHandler { onTapped: tela.janela.abrir("TelaOrdem", { osId: linha.o.id }) }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Tema.estreito ? 10 : 16
                    anchors.rightMargin: Tema.estreito ? 12 : 18
                    spacing: Tema.estreito ? 8 : 14
                    // FALHA-02: o número é por caixa; o caixa vai junto (duas
                    // OS nº 1 de caixas diferentes não se confundem).
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Tema.estreito ? 44 : 64
                        Text {
                            // Rascunho ainda não tem número.
                            text: linha.o.situacao === "rascunho" ? "—" : linha.o.numero
                            textFormat: Text.PlainText
                            color: Tema.douradoDaFaixa(linha.par)
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: Tema.estreito ? 22 : 26
                            horizontalAlignment: Text.AlignRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: linha.o.caixa
                            textFormat: Text.PlainText
                            visible: text !== ""
                            color: Tema.tintaSuaveDaFaixa(linha.par)
                            font.family: Tema.texto
                            font.pixelSize: 11
                            horizontalAlignment: Text.AlignRight
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Rectangle {
                        visible: linha.o.placa !== ""
                        radius: 6
                        color: Tema.papel
                        border.width: 1
                        border.color: Tema.sombra(0.25)
                        implicitWidth: textoPlaca.implicitWidth + 16
                        implicitHeight: 28
                        Text {
                            id: textoPlaca
                            anchors.centerIn: parent
                            text: linha.o.placa
                            textFormat: Text.PlainText
                            color: Tema.tinta
                            font.family: Tema.texto
                            font.weight: Font.Bold
                            font.letterSpacing: 1.2
                            font.pixelSize: 14
                        }
                    }
                    ColumnLayout {
                        spacing: 1
                        Layout.fillWidth: true
                        Text {
                            text: [linha.o.veiculo, linha.o.cliente].filter(x => !!x).join(" · ")
                            textFormat: Text.PlainText
                            color: Tema.tintaDaFaixa(linha.par)
                            font.family: Tema.titulo
                            font.weight: Font.Bold
                            font.pixelSize: 20
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Text {
                            text: [Tema.estreito ? linha.o.nomeSituacao : "",
                                   Tema.estreito && linha.o.previsao !== "" && ["orcamento", "aprovada"].includes(linha.o.situacao) ? "previsão " + linha.o.previsao : "",
                                   linha.o.defeito, linha.o.mecanico ? "mecânico " + linha.o.mecanico : "", "aberta " + linha.o.quando].filter(x => !!x).join("  ·  ")
                            textFormat: Text.PlainText
                            color: Tema.tintaSuaveDaFaixa(linha.par)
                            font.family: Tema.serifa
                            font.italic: true
                            font.pixelSize: 13
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    Text {
                        visible: !Tema.estreito && linha.o.previsao !== "" && ["orcamento", "aprovada"].includes(linha.o.situacao)
                        text: "previsão " + linha.o.previsao
                        textFormat: Text.PlainText
                        color: linha.o.atrasada ? Tema.vermelhoDaFaixa(linha.par) : Tema.tintaSuaveDaFaixa(linha.par)
                        font.family: Tema.texto
                        font.weight: linha.o.atrasada ? Font.Bold : Font.Normal
                        font.pixelSize: 13
                    }
                    Chip {
                        visible: !Tema.estreito
                        texto: linha.o.nomeSituacao
                        color: linha.cor[0]
                        corTexto: linha.cor[1]
                    }
                    Text {
                        text: "R$ " + linha.o.total
                        textFormat: Text.PlainText
                        color: Tema.tintaDaFaixa(linha.par)
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: Tema.estreito ? 15 : 17
                        horizontalAlignment: Text.AlignRight
                        Layout.preferredWidth: Tema.estreito ? -1 : 120
                    }
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 40, 620)
                visible: tela.buscou && lista.count === 0
                spacing: 8
                Titulo {
                    text: busca.texto !== "" ? "Nada encontrado" : tela.quais === "prontas" ? "Nenhum carro pronto" : "Nenhuma OS aqui"
                    font.pixelSize: 30
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    visible: busca.texto === ""
                    text: (Tema.celular ? "Nova OS" : "Nova OS (F2)") + ": digite a placa, o que o cliente reclamou e vá pondo as peças e a mão de obra. "
                          + "As peças só saem do estoque quando o carro é entregue."
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
            }
        }
    }

    StackView.onActivated: {
        carregar()
        busca.focar()
    }
}
