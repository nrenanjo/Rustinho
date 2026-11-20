import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Topo das telas: a seta de retorno (só a seta, no vidro roxo), título
// grande e, à direita, os botões.
ColumnLayout {
    id: cabecalho
    // Os testes das telas acham o topo da tela por este nome (no celular, sem
    // a seta, ele é a referência do lugar da tela). Não muda nada na tela.
    objectName: "cabecalhoDaTela"
    // T042: o cabeçalho fica por cima do que vem logo abaixo dele (em quase
    // todas as telas, a barra de pesquisa): o halo da barra passa ATRÁS dos
    // botões e do título, como na referência, e não por cima deles.
    z: 1
    property string titulo: ""
    property string subtitulo: ""
    property bool mostrarVoltar: true
    // Pedido do proprietário (18/11/2026): no celular (Android), sem a seta.
    // O "voltar" do Android (botão e gesto) faz o mesmo, pelo mesmo caminho
    // (Keys.onBackPressed → janela.retornar, no Main). O computador continua
    // com ela.
    readonly property bool comSeta: mostrarVoltar && !Tema.celular
    // Tela com CPF/CNPJ, custo, lucro ou saldos: mostra o olho no canto.
    property bool sensivel: false
    default property alias acoes: linhaAcoes.data

    spacing: 2

    RowLayout {
        spacing: 10
        Layout.fillWidth: true
        // Sem a seta, a linha só fica se houver o olho; senão, o título sobe.
        visible: cabecalho.comSeta || cabecalho.sensivel
        // V-2/V-3 (0.9.18): a seta faz o mesmo que o Esc, pelo mesmo caminho
        // (janela.retornar): a tela decide com aoEsc, se tiver; senão, volta.
        // Nenhuma tela trata a seta à parte.
        Seta {
            id: seta
            visible: cabecalho.comSeta
            // O vidro alinhado com o conteúdo e um pouco mais abaixo (Seta.descida).
            Layout.topMargin: seta.descida
            onClicked: ApplicationWindow.window.retornar(true)
        }
        // A altura da seta só quando ela está lá (com o olho sozinho, a linha
        // fica da altura dele).
        Item { Layout.fillWidth: true; implicitHeight: cabecalho.comSeta ? Tema.toqueSeta : 0 }
        // T054 (pedido do proprietário, 20/11/2026): o lugar do olho fica na
        // linha; o olho em si sobe para cima do véu enquanto uma janelinha que
        // pediu (levarOlho) está aberta, no mesmo lugar e com a mesma cara. É o
        // mesmo controle: nada de outro olho.
        Item {
            id: lugarDoOlho
            visible: cabecalho.sensivel
            implicitWidth: olho.implicitWidth
            implicitHeight: olho.implicitHeight
            Olho { id: olho }
            // A camada do olho: do tamanho dele, exatamente no lugar dele, por
            // cima da janelinha (aberta depois dela), sem véu, sem pegar o
            // teclado e sem fechar sozinha. Fora da área de uma janelinha, o
            // toque só chega ao que está dentro de uma camada; por isso o olho
            // vai para esta, e não para a janelinha.
            Popup {
                id: camadaDoOlho
                x: 0
                y: 0
                width: lugarDoOlho.width
                height: lugarDoOlho.height
                padding: 0
                margins: 0
                modal: false
                dim: false
                focus: false
                closePolicy: Popup.NoAutoClose
                background: null
                contentItem: Item { id: casaDoOlho }
            }
        }
    }

    property var _janelinhaDoOlho: null
    Binding {
        target: olho
        property: "parent"
        when: cabecalho._janelinhaDoOlho !== null && cabecalho.sensivel
        value: casaDoOlho
    }
    // A janelinha (um Painel aberto) que mostra valores chama ao abrir e ao
    // fechar. Sem ela, o olho fica na linha, como sempre.
    function levarOlho(janelinha) {
        if (!cabecalho.sensivel)
            return
        cabecalho._janelinhaDoOlho = janelinha
        camadaDoOlho.open()
    }
    function devolverOlho() {
        cabecalho._janelinhaDoOlho = null
        camadaDoOlho.close()
    }

    // Título e botões lado a lado; na tela estreita, os botões vão para
    // baixo do título e quebram linha.
    GridLayout {
        columns: Tema.fino ? 1 : 2
        columnSpacing: 16
        rowSpacing: 10
        Layout.fillWidth: true
        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            Titulo {
                text: cabecalho.titulo
                font.pixelSize: Tema.baixo ? 30 : Tema.estreito ? 34 : 44
                Layout.fillWidth: true
            }
            Nota {
                text: cabecalho.subtitulo
                visible: text !== ""
                font.pixelSize: Tema.estreito ? 15 : 16
                Layout.fillWidth: true
            }
        }
        Flow {
            id: linhaAcoes
            spacing: 10
            visible: children.length > 0
            Layout.alignment: Tema.fino ? Qt.AlignLeft : Qt.AlignBottom | Qt.AlignRight
            Layout.fillWidth: Tema.fino
            Layout.preferredWidth: Tema.fino ? -1 : implicitLargura
            // Flow não calcula a própria largura: soma a dos botões.
            readonly property real implicitLargura: {
                let t = 0
                let n = 0
                for (let i = 0; i < children.length; i++)
                    if (children[i].visible) {
                        t += children[i].width
                        n++
                    }
                return t + Math.max(0, n - 1) * spacing
            }
        }
    }
}
