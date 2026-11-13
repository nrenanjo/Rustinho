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
    // N04 (dev.3.3, passo 7.5): a lista é contínua, sem páginas. As peças vêm do
    // núcleo em partes, na ordem da lista inteira, e a próxima parte vem sozinha
    // quando a rolagem (ou o teclado) chega perto do fim. Nenhum botão de página
    // nem de "carregar mais". Uma peça que já está na lista não entra de novo.
    ListModel { id: pecas }
    property int total: 0
    property bool temMais: false
    readonly property int tamanhoParte: 100
    // Quantas linhas o núcleo já mandou nesta busca (é o deslocamento da
    // próxima parte) e os ids que já estão na lista.
    property int lidas: 0
    property var vistas: ({})
    property int diferentes: 0
    property bool carregando: false
    property int pendenciasXml: 0
    property bool buscou: false

    function atualizarPendenciasXml() {
        const p = Api.chamar("xml_pendencias_listar")
        if (p.ok && Array.isArray(p.dados))
            pendenciasXml = p.dados.length
    }

    function novaBusca() {
        buscar(false)
    }

    // A lista do começo. Com manterSelecao (ao voltar de uma peça), lê de novo
    // até onde a pessoa já tinha rolado e fica na mesma peça, se ela ainda
    // estiver na lista.
    function buscar(manterSelecao) {
        const anterior = manterSelecao && lista.currentIndex >= 0 && lista.currentIndex < pecas.count
                         ? pecas.get(lista.currentIndex).id : ""
        const ate = manterSelecao ? Math.max(pecas.count, tamanhoParte) : tamanhoParte
        const r = parte(0, Math.min(ate, 1000))
        if (r.ok) {
            pecas.clear()
            lidas = 0
            vistas = {}
            diferentes = 0
            total = r.dados.total || 0
            acrescentar(r.dados)
            while (temMais && pecas.count < ate) {
                if (!maisUmaParte(Math.min(ate - pecas.count, 1000)))
                    break
            }
            let indice = pecas.count > 0 ? 0 : -1
            if (anterior !== "") {
                for (let i = 0; i < pecas.count; i++) {
                    if (pecas.get(i).id === anterior) {
                        indice = i
                        break
                    }
                }
            }
            lista.currentIndex = indice
            if (indice <= 0)
                lista.positionViewAtBeginning()
        } else {
            janela.avisar(r.erro, "erro")
        }
        buscou = true
    }

    // Uma parte da lista, a partir de `desde` (na ordem da lista inteira).
    function parte(desde, quantas) {
        return Api.chamar("produtos_buscar", {
            texto: busca.texto,
            inativas: inativas.checked,
            limite: quantas,
            deslocamento: desde
        })
    }

    function acrescentar(dados) {
        const chegaram = dados.itens || []
        const novas = []
        for (const p of chegaram) {
            if (!vistas[p.id]) {
                vistas[p.id] = true
                novas.push(p)
            }
        }
        lidas += chegaram.length
        diferentes += novas.length
        if (novas.length > 0)
            pecas.append(novas)
        temMais = !!dados.temMais && chegaram.length > 0
    }

    // A próxima parte, no fim da lista. Devolve se veio.
    function maisUmaParte(quantas) {
        if (!temMais || carregando)
            return false
        carregando = true
        const r = parte(lidas, quantas || tamanhoParte)
        if (r.ok)
            acrescentar(r.dados)
        else {
            temMais = false
            janela.avisar(r.erro, "erro")
        }
        carregando = false
        return r.ok
    }

    // Perto do fim (a última linha à vista a 30 do fim, ou a linha atual nas
    // 12 últimas, pelo teclado): a próxima parte vem sozinha. Antes de a lista
    // se arrumar na tela não há linha à vista (-1), e nada vem antes da hora.
    function talvezMais() {
        if (!temMais || carregando || lista.count === 0)
            return
        const ultimaAVista = lista.indexAt(lista.width / 2, lista.contentY + lista.height - 1)
        if (ultimaAVista >= lista.count - 30 || lista.currentIndex >= lista.count - 12)
            Qt.callLater(maisUmaParte)
    }

    function abrirPeca(indice) {
        if (indice >= 0 && indice < pecas.count)
            janela.abrir("TelaPeca", { produtoId: pecas.get(indice).id })
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

    // Setas e Page Up/Down mexem na lista enquanto se digita na busca, pela
    // lista inteira (N04): na última linha, a próxima parte vem antes de andar.
    Keys.onDownPressed: {
        if (lista.currentIndex >= lista.count - 1)
            tela.maisUmaParte()
        lista.currentIndex = Math.min(lista.currentIndex + 1, lista.count - 1)
    }
    Keys.onUpPressed: lista.currentIndex = Math.max(lista.currentIndex - 1, 0)
    Keys.onPressed: (evento) => {
        if (evento.key === Qt.Key_PageDown) {
            evento.accepted = true
            if (lista.currentIndex + 8 > lista.count - 1)
                tela.maisUmaParte()
            lista.currentIndex = Math.min(lista.currentIndex + 8, lista.count - 1)
        } else if (evento.key === Qt.Key_PageUp) {
            evento.accepted = true
            lista.currentIndex = Math.max(lista.currentIndex - 8, 0)
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
            visible: pecas.count > 0 && !Tema.estreito
            spacing: 18
            Layout.fillWidth: true
            Layout.leftMargin: 18
            Layout.rightMargin: 20
            Layout.topMargin: 4
            Rotulo { text: "Código"; Layout.preferredWidth: 84 }
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
            model: pecas
            currentIndex: -1
            highlightMoveDuration: 0
            highlightFollowsCurrentItem: true
            keyNavigationEnabled: false
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            onCurrentIndexChanged: {
                if (currentIndex >= 0)
                    positionViewAtIndex(currentIndex, ListView.Contain)
                tela.talvezMais()
            }
            onContentYChanged: tela.talvezMais()
            onCountChanged: tela.talvezMais()
            onHeightChanged: tela.talvezMais()
            delegate: LinhaPeca {
                required property int index
                width: lista.width
                // Ao recomeçar a lista, a linha que está saindo não tem mais peça.
                item: pecas.get(index) || ({})
                indice: index
                atual: lista.currentIndex === index
                onAbrir: tela.abrirPeca(index)
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(560, parent.width - 40)
                visible: tela.buscou && pecas.count === 0
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
    }

    // Para os testes das telas (--textos-da-tela): quantas linhas a lista tem,
    // quantas peças diferentes, se ainda há mais (1/0) e a linha atual. Sem
    // tamanho: não aparece e não se toca.
    Item {
        readonly property string nomeAcessivel: "TESTE_LISTA:" + pecas.count + ":" + tela.diferentes + ":"
                                                + (tela.temMais ? 1 : 0) + ":" + lista.currentIndex
    }

    StackView.onActivated: {
        buscar(true)
        atualizarPendenciasXml()
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
        onImportou: {
            tela.atualizarPendenciasXml()
            tela.buscar()
        }
        onClosed: {
            tela.atualizarPendenciasXml()
            if (tela.StackView.status === StackView.Active)
                busca.focarCursor()
        }
    }
}
