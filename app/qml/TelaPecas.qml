import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Peças: busca única (código, nome, marca, carro...) e lista de resultados.
FocusScope {
    id: tela
    objectName: "telaPecas"
    readonly property var janela: ApplicationWindow.window
    // Nome desta tela no "voltar" de uma tela aberta pela navegação guiada.
    readonly property string nomeNaNavegacao: "Peças"
    // Unidades de medida (0.9.18): T da janela de 24 horas das vendas = a
    // abertura desta pílula (cada abertura empilha uma tela nova; enquanto
    // ela fica aberta, T não muda).
    readonly property string abertaEm: new Date().toISOString()
    property string buscaInicial: ""
    property bool abrirPendenciasXml: false
    property var itens: []
    property int total: 0
    property int tamanhoPagina: 50
    property int deslocamento: 0
    property int pendenciasXml: 0
    property bool buscou: false

    function atualizarPendenciasXml() {
        const p = Api.chamar("xml_pendencias_listar")
        if (p.ok && Array.isArray(p.dados))
            pendenciasXml = p.dados.length
    }

    function novaBusca() {
        deslocamento = 0
        buscar(false)
    }

    function irParaPagina(novoDeslocamento) {
        deslocamento = Math.max(0, novoDeslocamento)
        buscar(false)
    }

    function buscar(manterSelecao) {
        const anterior = manterSelecao && lista.currentIndex >= 0 && lista.currentIndex < itens.length
                         ? itens[lista.currentIndex].id : ""
        const r = Api.chamar("produtos_buscar", {
            texto: busca.texto,
            inativas: inativas.checked,
            limite: tamanhoPagina,
            deslocamento: deslocamento
        })
        if (r.ok) {
            itens = r.dados.itens
            total = r.dados.total
            deslocamento = r.dados.deslocamento || 0
            if (itens.length === 0 && total > 0 && deslocamento > 0) {
                deslocamento = Math.max(0, Math.floor((total - 1) / tamanhoPagina) * tamanhoPagina)
                return buscar(false)
            }
            let indice = itens.length > 0 ? 0 : -1
            if (anterior !== "") {
                const achou = itens.findIndex(p => p.id === anterior)
                if (achou >= 0)
                    indice = achou
            }
            lista.currentIndex = indice
            if (indice <= 0)
                lista.positionViewAtBeginning()
        } else {
            janela.avisar(r.erro, "erro")
        }
        atualizarPendenciasXml()
        buscou = true
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

    // O Esc e a seta fazem o mesmo (D-V3, decidida pelo dono em 07/11/2026:
    // com a busca preenchida, os dois voltam direto). limparBuscaAntes só
    // limparia a busca antes se a regra mudasse em Main.
    function aoEsc(daSeta) {
        if (busca.texto !== "" && janela.limparBuscaAntes(daSeta)) {
            busca.texto = ""
            novaBusca()
        } else {
            janela.voltar()
        }
    }

    Timer {
        id: espera
        interval: 160
        onTriggered: tela.novaBusca()
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
            subtitulo: tela.total === 0 ? "Nenhuma peça cadastrada" : tela.total === 1 ? "1 peça cadastrada" : tela.total + " peças cadastradas"
            Layout.fillWidth: true
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
                onEditado: espera.restart()
                onConfirmado: tela.abrirPeca(lista.currentIndex)
            }
            Marcar {
                id: inativas
                text: "Mostrar inativas"
                onToggled: tela.novaBusca()
            }
        }

        // Aviso contextual persistente de pendências de identificação XML (Fase 4.D):
        // não pode ser dispensado enquanto houver pendências.
        Rectangle {
            visible: tela.pendenciasXml > 0
            Layout.fillWidth: true
            implicitHeight: linhaAvisoXml.implicitHeight + 22
            radius: Tema.raio
            color: Tema.critico
            palette.window: Tema.superficie(color)
            border.width: 1
            border.color: Tema.bordaCritico
            RowLayout {
                id: linhaAvisoXml
                x: 18
                y: 11
                width: parent.width - 36
                spacing: 14
                ColumnLayout {
                    spacing: 2
                    Layout.fillWidth: true
                    Text {
                        text: "Pendências de identificação XML"
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.weight: Font.Bold
                        font.pixelSize: 16
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    Text {
                        text: (tela.pendenciasXml === 1
                               ? "1 item de XML aguardando associação com peça"
                               : tela.pendenciasXml + " itens de XML aguardando associação com peças")
                              + " · associe a uma peça existente ou cadastre como nova para concluir."
                        color: Tema.tinta
                        font.family: Tema.texto
                        font.pixelSize: 14
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                }
                Botao {
                    text: "Pendências de identificação"
                    tipo: "principal"
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: importador.abrirPendencias()
                }
            }
        }

        RowLayout {
            visible: tela.itens.length > 0 && !Tema.estreito
            spacing: 18
            Layout.fillWidth: true
            Layout.leftMargin: 18
            Layout.rightMargin: 20
            Layout.topMargin: 4
            Rotulo { text: "Cód. fornecedor"; Layout.preferredWidth: 136 }
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
                    text: tela.total === 0 ? "Nenhuma peça cadastrada ainda" : "Nada encontrado"
                    font.pixelSize: 32
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Nota {
                    text: tela.total === 0
                          ? "Cadastre as peças com os carros em que servem: assim o balcão acha tudo por um campo só."
                          : "Nenhuma peça combina com “" + busca.texto + "”. Tente só uma parte do código ou do nome."
                    font.pixelSize: 16
                    horizontalAlignment: Text.AlignHCenter
                    Layout.fillWidth: true
                }
                Botao {
                    visible: tela.total === 0
                    text: "Cadastrar a primeira peça"
                    icone: "mais"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: tela.novaPeca()
                }
            }
        }

        RowLayout {
            visible: tela.total > tela.tamanhoPagina || tela.deslocamento > 0
            spacing: 12
            Layout.fillWidth: true
            Botao {
                text: "Anterior"
                tipo: "secundario"
                enabled: tela.deslocamento > 0
                onClicked: tela.irParaPagina(Math.max(0, tela.deslocamento - tela.tamanhoPagina))
            }
            Item { Layout.fillWidth: true }
            Nota {
                text: "Página " + (Math.floor(tela.deslocamento / tela.tamanhoPagina) + 1)
                      + " de " + Math.max(1, Math.ceil(tela.total / tela.tamanhoPagina))
                      + (tela.itens.length > 0
                         ? "  ·  " + (tela.deslocamento + 1) + " a " + (tela.deslocamento + tela.itens.length) + " de " + tela.total
                         : "")
                font.pixelSize: 14
            }
            Item { Layout.fillWidth: true }
            Botao {
                text: "Próxima"
                tipo: "secundario"
                enabled: tela.deslocamento + tela.itens.length < tela.total
                onClicked: tela.irParaPagina(tela.deslocamento + tela.tamanhoPagina)
            }
        }
    }

    StackView.onActivated: {
        buscar(true)
        if (abrirPendenciasXml) {
            abrirPendenciasXml = false
            Qt.callLater(() => importador.abrirPendencias())
        } else {
            busca.focarCursor()
        }
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
