import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Peças: busca única (código, nome, marca, carro...) e lista de resultados.
FocusScope {
    id: tela
    readonly property var janela: ApplicationWindow.window
    // Nome desta tela no "voltar" de uma tela aberta pela navegação guiada.
    readonly property string nomeNaNavegacao: "Peças"
    property string buscaInicial: ""
    property var itens: []
    property int total: 0
    property int totalCatalogo: 0
    property bool buscou: false
    property var avisoVinculosXml: null
    property int buscaVersao: 0
    property bool buscaEmAndamento: false
    property var buscaSeguinte: null
    property var pedidosBusca: ({})

    function buscar(manterSelecao, adicionar) {
        const anterior = manterSelecao && lista.currentIndex >= 0 && lista.currentIndex < itens.length
                         ? itens[lista.currentIndex].id : ""
        const consulta = {
            versao: ++buscaVersao,
            anterior: anterior,
            adicionar: adicionar === true,
            dados: {
                texto: busca.texto,
                inativas: inativas.checked,
                limite: 100,
                deslocamento: adicionar ? itens.length : 0
            }
        }
        if (buscaEmAndamento) {
            buscaSeguinte = consulta
            return
        }
        enviarBusca(consulta)
    }

    function invalidarBusca() {
        buscaVersao++
        buscaSeguinte = null
    }

    function enviarBusca(consulta) {
        buscaEmAndamento = true
        const pedido = Sistema.buscar_produtos(JSON.stringify(consulta.dados))
        if (pedido === 0) {
            buscaEmAndamento = false
            janela.avisar("Não foi possível iniciar a busca agora. Tente novamente.", "erro")
            return
        }
        const pendentes = Object.assign({}, pedidosBusca)
        pendentes[pedido] = consulta
        pedidosBusca = pendentes
    }

    function aplicarResultado(consulta, resposta) {
        if (consulta.versao !== buscaVersao)
            return
        const r = Api.interpretar("produtos_buscar", resposta)
        if (r.ok) {
            itens = consulta.adicionar ? itens.concat(r.dados.itens) : r.dados.itens
            total = r.dados.total
            totalCatalogo = r.dados.catalogo
            let indice = itens.length > 0 ? 0 : -1
            if (consulta.anterior !== "") {
                const achou = itens.findIndex(p => p.id === consulta.anterior)
                if (achou >= 0)
                    indice = achou
            }
            lista.currentIndex = indice
            if (!consulta.adicionar && indice <= 0)
                lista.positionViewAtBeginning()
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }

    function carregarAvisoVinculos() {
        const alertas = Api.chamar("avisos_do_dia")
        avisoVinculosXml = alertas.ok ? alertas.dados.find(a => a.id === "vinculos_xml") || null : null
    }

    Connections {
        target: Sistema
        function onBuscaTerminou(pedido, resposta) {
            const pendentes = Object.assign({}, tela.pedidosBusca)
            const consulta = pendentes[pedido]
            delete pendentes[pedido]
            tela.pedidosBusca = pendentes
            tela.buscaEmAndamento = false
            if (tela.buscaSeguinte) {
                const seguinte = tela.buscaSeguinte
                tela.buscaSeguinte = null
                tela.enviarBusca(seguinte)
                return
            }
            if (consulta)
                tela.aplicarResultado(consulta, resposta)
        }
    }

    function abrirXmlPendente() {
        const r = Api.chamar("compras_listar", { texto: "", limite: 1000 })
        if (!r.ok) {
            janela.avisar(r.erro, "erro")
            return
        }
        const compra = r.dados.find(c => c.situacao === "rascunho" && c.pendentes > 0)
        if (!compra) {
            buscar()
            return
        }
        Api.chamar("menu_usar", { id: "compras" })
        janela.abrir("TelaCompra", { compraId: compra.id, focarPendenteInicial: true })
    }

    function abrirPeca(indice) {
        if (indice >= 0 && indice < itens.length)
            janela.abrir("TelaPeca", { produtoId: itens[indice].id })
    }

    function novaPeca() {
        janela.abrir("TelaPeca", { produtoId: "" })
    }

    function focarBusca() {
        busca.focar()
    }

    function aoEsc() {
        if (busca.texto !== "") {
            busca.texto = ""
            buscar()
        } else {
            janela.voltar()
        }
    }

    Timer {
        id: espera
        interval: 160
        onTriggered: tela.buscar()
    }

    Shortcut {
        sequence: "F2"
        enabled: tela.StackView.status === StackView.Active
        onActivated: tela.novaPeca()
    }

    // Setas e Page Up/Down mexem na lista enquanto se digita na busca.
    Keys.onDownPressed: lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
    Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)
    Keys.onPressed: (evento) => {
        if (evento.key === Qt.Key_PageDown) {
            lista.currentIndex = Math.min(lista.currentIndex + 8, lista.count - 1)
            evento.accepted = true
        } else if (evento.key === Qt.Key_PageUp) {
            lista.currentIndex = Math.max(lista.currentIndex - 8, 0)
            evento.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Tema.margem
        anchors.topMargin: Tema.margem - 12
        spacing: 14

        Cabecalho {
            titulo: "Peças"
            subtitulo: busca.texto.trim() !== ""
                       ? tela.total + (tela.total === 1 ? " resultado" : " resultados")
                       : tela.totalCatalogo === 0 ? "Nenhuma peça cadastrada"
                         : tela.totalCatalogo === 1 ? "1 peça cadastrada" : tela.totalCatalogo + " peças cadastradas"
            Layout.fillWidth: true
            onVoltar: tela.janela.voltar()
            Botao {
                text: "Etiquetas"
                icone: "impressora"
                tipo: "discreto"
                onClicked: tela.janela.abrir("TelaEtiquetas", {})
            }
            Botao {
                text: "Importar XML"
                tipo: "secundario"
                onClicked: importador.abrirCom()
            }
            Botao {
                text: "Nova peça"
                icone: "mais"
                atalho: "F2"
                onClicked: tela.novaPeca()
            }
        }

        Rectangle {
            visible: !!tela.avisoVinculosXml
            radius: Tema.raio
            color: Tema.critico
            border.width: 1
            border.color: Tema.bordaCritico
            Layout.fillWidth: true
            implicitHeight: conteudoAlerta.implicitHeight + 24
            RowLayout {
                id: conteudoAlerta
                anchors.fill: parent
                anchors.margins: 12
                spacing: 12
                Text {
                    text: tela.avisoVinculosXml ? tela.avisoVinculosXml.texto : ""
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                Botao {
                    text: "Revisar"
                    tipo: "principal"
                    onClicked: tela.abrirXmlPendente()
                }
            }
        }

        Linha {
            spacing: 18
            Layout.fillWidth: true
            Campo {
                id: busca
                icone: "busca"
                atalho: "F11"
                camera: true
                altura: 54
                tamanhoFonte: 18
                dica: Tema.estreito ? "Código, nome ou carro" : "Qualquer código, nome, modelo, marca ou carro (ex.: 8PK1515, pastilha gol 2015)"
                Layout.fillWidth: true
                onEditado: { tela.invalidarBusca(); espera.restart() }
                onConfirmado: tela.abrirPeca(lista.currentIndex)
            }
            Marcar {
                id: inativas
                text: "Mostrar inativas"
                onToggled: tela.buscar()
            }
        }

        RowLayout {
            visible: tela.itens.length > 0 && !Tema.estreito
            spacing: 18
            Layout.fillWidth: true
            Layout.leftMargin: 18
            Layout.rightMargin: 20
            Layout.topMargin: 4
            Rotulo { text: "Cód. fornecedor"; Layout.preferredWidth: 140 }
            Rotulo { text: busca.texto.trim() === "" ? "Peça · alteradas por último" : "Peça"; Layout.fillWidth: true }
            Rotulo { text: "Local"; Layout.preferredWidth: 90 }
            Rotulo { text: "Estoque"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 104 }
            Rotulo { text: "Revenda"; horizontalAlignment: Text.AlignRight; Layout.preferredWidth: 128 }
        }

        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 6
            model: tela.itens
            currentIndex: -1
            highlightMoveDuration: 0
            highlightFollowsCurrentItem: true
            keyNavigationEnabled: false
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            footer: Component {
                Item {
                    width: lista.width
                    height: tela.itens.length < tela.total ? 58 : 0
                    Botao {
                        anchors.centerIn: parent
                        visible: parent.height > 0
                        text: "Carregar mais resultados"
                        tipo: "secundario"
                        onClicked: tela.buscar(true, true)
                    }
                }
            }
            onCurrentIndexChanged: if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
            delegate: LinhaPeca {
                required property var modelData
                required property int index
                width: lista.width
                item: modelData
                indice: index
                atual: lista.currentIndex === index
                onAbrir: tela.abrirPeca(index)
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(560, parent.width - 40)
                visible: tela.buscou && tela.itens.length === 0
                spacing: 14
                Titulo {
                    text: tela.totalCatalogo === 0 ? "Nenhuma peça cadastrada ainda" : "Nada encontrado"
                    font.pixelSize: 32
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: tela.totalCatalogo === 0
                          ? "Cadastre as peças com os carros em que servem: assim o balcão acha tudo por um campo só."
                          : "Nenhuma peça combina com “" + busca.texto + "”. Tente só uma parte do código ou do nome."
                    font.pixelSize: 16
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Botao {
                    visible: tela.totalCatalogo === 0
                    text: "Cadastrar a primeira peça"
                    icone: "mais"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: tela.novaPeca()
                }
            }
        }
    }

    StackView.onActivated: {
        buscar(true)
        carregarAvisoVinculos()
        busca.focarCursor()
    }
    Component.onCompleted: {
        busca.texto = buscaInicial
    }

    ImportarCadastros {
        id: importador
        onImportou: tela.buscar()
        // O cursor volta à busca só se Peças for a tela de cima (a importação
        // também fecha quando a navegação guiada leva a outra tela).
        onClosed: if (tela.StackView.status === StackView.Active) busca.focarCursor()
    }
}
