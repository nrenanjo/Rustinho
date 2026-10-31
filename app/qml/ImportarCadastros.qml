import QtCore
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import rustinho

// Trazer clientes, fornecedores e peças dos XMLs das notas: as da loja
// (exportadas do sistema antigo) e as dos fornecedores. Mostra antes o que vai ser
// cadastrado; nada que já existe é mudado.
Painel {
    id: painel
    property string chave: ""
    readonly property var janela: ApplicationWindow.window
    width: Math.min(640, Tema.largura - 24)
    property var arquivos: []
    property var previa: null
    property string erro: ""
    property bool trabalhando: false
    // Escolhas guardadas para quando os arquivos são lidos de novo.
    property var escolhas: null
    // Arquivos que sumiram enquanto a pessoa resolvia um requisito.
    property string avisoArquivos: ""
    // O que cadastrar (marcado na prévia).
    property bool qClientes: false
    property bool qFornecedores: false
    property bool qPecasLoja: false
    property bool qPecasFornecedor: false

    signal importou()

    function abrirCom() {
        chave = Api.novaChave()
        arquivos = []
        previa = null
        erro = ""
        escolhas = null
        avisoArquivos = ""
        open()
    }

    function ler(lista) {
        arquivos = lista.map(u => Api.caminho(u.toString()))
        escolhas = null
        avisoArquivos = ""
        lerArquivos()
    }
    // Lê (ou lê de novo) os arquivos já escolhidos: só mostra a prévia, não
    // cadastra nada.
    function lerArquivos() {
        erro = ""
        previa = null
        trabalhando = true
        // Deixa a tela mostrar "Lendo..." antes de começar.
        Api.chamarDemorado("importar_previa", { arquivos: painel.arquivos }, "Lendo os arquivos…", r => {
            painel.trabalhando = false
            if (r.ok) {
                painel.previa = r.dados
                const e = painel.escolhas
                painel.qClientes = r.dados.clientesNovos > 0 && (!e || e.qClientes)
                painel.qFornecedores = r.dados.fornecedoresNovos > 0 && (!e || e.qFornecedores)
                painel.qPecasLoja = r.dados.pecasLojaNovas > 0 && (!e || e.qPecasLoja)
                painel.qPecasFornecedor = r.dados.pecasFornecedorNovas > 0 && (!e || e.qPecasFornecedor)
                return
            }
            painel.erro = r.erro
            // Falta um requisito (ex.: o CNPJ da loja): a navegação guiada
            // leva ao lugar de resolver. Os arquivos e as escolhas ficam aqui;
            // a janela volta igual quando a pessoa voltar.
            if (painel.janela.guiar(r.campo, r.erro, ok => painel.voltouDaGuia(ok)))
                painel.close()
        })
    }
    // Volta da navegação guiada: reabre como estava. Com o requisito
    // satisfeito, confere se os arquivos continuam no lugar e lê de novo
    // (a pessoa decide se cadastra).
    function voltouDaGuia(satisfeito) {
        open()
        if (!satisfeito)
            return
        const c = Api.chamar("arquivos_conferir", { arquivos: painel.arquivos })
        if (c.ok && c.dados.faltam.length > 0) {
            const nomes = c.dados.faltam.map(a => a.split(/[\\/]/).pop()).join(", ")
            painel.arquivos = c.dados.existem
            if (c.dados.existem.length === 0) {
                painel.erro = "Os arquivos escolhidos não estão mais no lugar (" + nomes + "). Escolha de novo."
                return
            }
            painel.lerArquivos()
            painel.avisoArquivos = "Não estão mais no lugar e ficaram de fora: " + nomes + ". Os outros continuam escolhidos."
            return
        }
        painel.lerArquivos()
    }

    function cadastrar() {
        cadastrarAgora()
    }
    function cadastrarAgora() {
        trabalhando = true
        Qt.callLater(() => {
            // D-09: a senha do gerente vale para esta importação, com estes arquivos.
            painel.janela.chamarGerenteDemorado("importar_cadastros", {
                arquivos: painel.arquivos, clientes: painel.qClientes, fornecedores: painel.qFornecedores,
                pecasLoja: painel.qPecasLoja, pecasFornecedor: painel.qPecasFornecedor
            }, "Cadastrando…", r => painel.depoisDeImportar(r), painel.chave)
        })
    }
    function depoisDeImportar(r) {
        Qt.callLater(() => {
            painel.trabalhando = false
            if (!r.ok) {
                painel.erro = r.erro
                return
            }
            const d = r.dados
            const partes = []
            if (d.clientes > 0) partes.push(d.clientes + (d.clientes === 1 ? " cliente" : " clientes"))
            if (d.fornecedores > 0) partes.push(d.fornecedores + (d.fornecedores === 1 ? " fornecedor" : " fornecedores"))
            if (d.pecas > 0) partes.push(d.pecas + (d.pecas === 1 ? " peça" : " peças"))
            let msg = partes.length > 0 ? "Cadastrados: " + partes.join(", ") + "." : "Nada novo para cadastrar."
            if (d.codigosLigados > 0)
                msg += " " + d.codigosLigados + " códigos de fornecedor ligados às peças."
            painel.janela.avisar(msg, "ok")
            painel.close()
            painel.importou()
        })
    }

    function texto(n, um, varios) {
        return n + " " + (n === 1 ? um : varios)
    }

    FileDialog {
        id: seletorArquivos
        title: "Escolha os XMLs das notas (ou um .zip / .rar)"
        fileMode: FileDialog.OpenFiles
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        nameFilters: ["XML, ZIP ou RAR (*.xml *.XML *.zip *.ZIP *.rar *.RAR)", "Todos os arquivos (*)"]
        onAccepted: painel.ler(selectedFiles)
    }
    FolderDialog {
        id: seletorPasta
        title: "Escolha a pasta com os XMLs"
        currentFolder: StandardPaths.standardLocations(StandardPaths.DownloadLocation)[0]
        onAccepted: painel.ler([selectedFolder])
    }

    contentItem: Item {
        implicitWidth: coluna.implicitWidth
        implicitHeight: coluna.implicitHeight

        DropArea {
            anchors.fill: parent
            keys: ["text/uri-list"]
            onDropped: (evento) => {
                if (evento.hasUrls)
                    painel.ler(evento.urls)
            }
        }

        ColumnLayout {
            id: coluna
            width: parent.width
            spacing: 14

            Titulo { text: "Trazer cadastros pelo XML"; font.pixelSize: 32; Layout.fillWidth: true }
            Nota {
                text: "Use os XMLs das notas emitidas pela loja (exportados do sistema antigo) para trazer clientes e peças com o código e o preço da loja, "
                      + "e os das notas de fornecedores para trazer fornecedores e peças com o custo. O que já está cadastrado não muda. "
                      + "Aqui as notas servem só de cadastro: não entram no estoque nem no financeiro (para isso, use Compras)."
                font.pixelSize: 14
                Layout.fillWidth: true
            }

            Linha {
                spacing: 10
                Layout.fillWidth: true
                Botao {
                    text: "Escolher arquivos"
                    icone: "mais"
                    tipo: painel.previa ? "secundario" : "principal"
                    enabled: !painel.trabalhando
                    onClicked: seletorArquivos.open()
                }
                Botao {
                    text: "Escolher uma pasta"
                    visible: !Tema.celular
                    tipo: "secundario"
                    enabled: !painel.trabalhando
                    onClicked: seletorPasta.open()
                }
                Item { Layout.fillWidth: true }
            }
            Nota {
                visible: !painel.previa && !painel.trabalhando && painel.erro === ""
                text: "Dá para escolher vários .xml, um .zip ou .rar com os XMLs dentro ou uma pasta inteira (com subpastas). Também dá para arrastar para cá."
                font.pixelSize: 13
                Layout.fillWidth: true
            }

            Text {
                visible: painel.trabalhando
                text: "Lendo as notas..."
                color: Tema.tintaSuave
                font.family: Tema.serifa
                font.italic: true
                font.pixelSize: 16
            }
            TextoErro { text: painel.erro; Layout.fillWidth: true }
            // Os arquivos já escolhidos (continuam escolhidos quando a leitura
            // não deu: dá para tentar de novo sem escolher outra vez).
            Linha {
                visible: painel.arquivos.length > 0 && !painel.previa && !painel.trabalhando
                spacing: 10
                Layout.fillWidth: true
                Nota {
                    text: painel.texto(painel.arquivos.length, "arquivo escolhido", "arquivos escolhidos") + ": "
                          + painel.arquivos.map(a => a.split(/[\\/]/).pop()).join(", ")
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    maximumLineCount: 2
                    Layout.fillWidth: true
                }
                Botao {
                    text: "Tentar de novo"
                    tipo: "secundario"
                    onClicked: painel.lerArquivos()
                }
            }
            Nota {
                visible: painel.avisoArquivos !== ""
                text: painel.avisoArquivos
                color: Tema.vermelho
                font.pixelSize: 13
                Layout.fillWidth: true
            }

            // Prévia
            ColumnLayout {
                visible: !!painel.previa && !painel.trabalhando
                spacing: 8
                Layout.fillWidth: true

                Rotulo {
                    text: painel.previa ? painel.texto(painel.previa.notas, "nota lida", "notas lidas")
                                          + "  ·  " + painel.previa.notasLoja + " da loja  ·  " + painel.previa.notasFornecedor + " de fornecedores"
                                          + (painel.previa.ignorados > 0 ? "  ·  " + painel.texto(painel.previa.ignorados, "arquivo ignorado", "arquivos ignorados") : "") : ""
                    Layout.fillWidth: true
                }

                Repeater {
                    model: painel.previa ? [
                        { chave: "qClientes", titulo: "Clientes", novos: painel.previa.clientesNovos, ja: painel.previa.clientesJa,
                          exemplos: painel.previa.exemplosClientes, cor: Tema.verde },
                        { chave: "qFornecedores", titulo: "Fornecedores", novos: painel.previa.fornecedoresNovos, ja: painel.previa.fornecedoresJa,
                          exemplos: painel.previa.exemplosFornecedores, cor: Tema.azul },
                        { chave: "qPecasLoja", titulo: "Peças das notas da loja", novos: painel.previa.pecasLojaNovas, ja: -1,
                          exemplos: [], cor: Tema.papel },
                        { chave: "qPecasFornecedor", titulo: "Peças das notas de fornecedores", novos: painel.previa.pecasFornecedorNovas, ja: -1,
                          exemplos: [], cor: Tema.papel }
                    ] : []
                    delegate: Rectangle {
                        id: linha
                        required property var modelData
                        visible: modelData.novos > 0 || modelData.ja > 0
                        Layout.fillWidth: true
                        implicitHeight: coluna.implicitHeight + 20
                        radius: Tema.raio
                        color: modelData.novos > 0 ? modelData.cor : Tema.vela(0.35)
                        ColumnLayout {
                            id: coluna
                            x: 14
                            y: 10
                            width: parent.width - 28
                            spacing: 2
                            RowLayout {
                                spacing: 6
                                Layout.fillWidth: true
                                Marcar {
                                    visible: linha.modelData.novos > 0
                                    checked: painel[linha.modelData.chave]
                                    onToggled: {
                                        painel[linha.modelData.chave] = checked
                                        painel.escolhas = { qClientes: painel.qClientes, qFornecedores: painel.qFornecedores,
                                                            qPecasLoja: painel.qPecasLoja, qPecasFornecedor: painel.qPecasFornecedor }
                                    }
                                    leftPadding: 0
                                }
                                Text {
                                    text: linha.modelData.titulo
                                    color: Tema.tinta
                                    font.family: Tema.titulo
                                    font.weight: Font.Bold
                                    font.pixelSize: 20
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                }
                                Text {
                                    text: linha.modelData.novos > 0 ? painel.texto(linha.modelData.novos, "novo", "novos") : "nada novo"
                                    color: linha.modelData.novos > 0 ? Tema.verdeEscuro : Tema.apagado
                                    font.family: Tema.texto
                                    font.weight: Font.Bold
                                    font.pixelSize: 15
                                }
                            }
                            Text {
                                visible: text !== ""
                                text: [linha.modelData.ja > 0 ? linha.modelData.ja + " já cadastrados" : "",
                                       linha.modelData.exemplos.length > 0 ? linha.modelData.exemplos.join(", ") + (linha.modelData.novos > linha.modelData.exemplos.length ? "..." : "") : ""]
                                      .filter(x => !!x).join("  ·  ")
                                color: Tema.tintaSuave
                                font.family: Tema.serifa
                                font.italic: true
                                font.pixelSize: 13
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
                    }
                }

                Nota {
                    visible: !!painel.previa && painel.previa.pecasLojaNovas > 0 && painel.previa.pecasFornecedorNovas > 0
                    text: "Peça sem código de barras pode ficar repetida (uma com o código da loja, outra do fornecedor). Se for migrar de outro sistema, prefira só as notas da loja."
                    font.pixelSize: 13
                    Layout.fillWidth: true
                }
                Repeater {
                    model: painel.previa ? painel.previa.avisos : []
                    delegate: Text {
                        required property var modelData
                        text: modelData
                        color: Tema.apagado
                        font.family: Tema.texto
                        font.pixelSize: 12
                        elide: Text.ElideMiddle
                        Layout.fillWidth: true
                    }
                }
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true
                Layout.topMargin: 6
                Item { Layout.fillWidth: true }
                Botao { text: "Fechar"; tipo: "discreto"; onClicked: painel.close() }
                Botao {
                    visible: !!painel.previa
                    text: "Cadastrar"
                    icone: "check"
                    enabled: !painel.trabalhando && (painel.qClientes || painel.qFornecedores || painel.qPecasLoja || painel.qPecasFornecedor)
                    onClicked: painel.cadastrar()
                }
            }
        }
    }
}
