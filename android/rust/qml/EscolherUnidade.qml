import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Lista das unidades de medida para escolher (e criar uma nova).
// Ordem (Seção 6 do padrão, 0.9.18; a regra fica no núcleo, unidades.rs):
// 1ª linha = as 5 com mais peças ativas; 2ª linha = 6º ao 10º do cadastro e,
// entrando pela direita, as até 5 mais vendidas nas últimas 24 horas (T = a
// abertura da pílula Peças); depois, todas as outras pela sigla. Até 5
// botões por linha no computador e 3 no celular: cada grupo começa numa
// linha nova e a sequência não muda.
Painel {
    id: escolher
    property var lista: []
    property string atual: ""
    // T da janela de 24 horas (RFC 3339). Vazio: o núcleo usa agora.
    property string referencia: ""
    // FALHA-04: chave da unidade nova (nova a cada vez que a janela abre).
    property string chave: ""
    signal escolhida(string sigla)
    width: Math.min(620, Tema.largura - 24)
    readonly property int colunas: Tema.celular ? 3 : 5
    readonly property real larguraBotao: Math.floor((rolagem.width - 6 * (colunas - 1)) / colunas)
    readonly property var linha1: lista.filter(u => u.linha === 1)
    readonly property var linha2: lista.filter(u => u.linha === 2)
    readonly property var demais: lista.filter(u => u.linha !== 1 && u.linha !== 2)

    function abrirCom(sigla, instante) {
        chave = Api.novaChave()
        atual = sigla
        referencia = instante || ""
        carregar()
        sigla_.texto = ""
        descricao.texto = ""
        fracionada.checked = false
        sigla_.erro = ""
        descricao.erro = ""
        open()
    }
    function carregar() {
        const r = Api.chamar("unidades_listar", { referencia: escolher.referencia })
        if (r.ok)
            lista = r.dados
    }
    function criar() {
        sigla_.erro = ""
        descricao.erro = ""
        const r = Api.gravar("unidade_salvar", { sigla: sigla_.texto, descricao: descricao.texto, fracionada: fracionada.checked }, chave)
        if (!r.ok) {
            (r.campo === "descricao" ? descricao : sigla_).erro = r.erro
            return
        }
        lista = r.dados.lista
        escolher.escolhida(r.dados.sigla)
        close()
    }

    contentItem: ColumnLayout {
        id: coluna
        spacing: 14
        // A janelinha cabe na tela (no celular, em pé, a tela é baixa): a lista
        // das unidades fica com a altura que sobra e rola; o resto (título,
        // nova unidade, "Criar e usar") aparece sempre inteiro.
        readonly property real alturaDoResto: tituloUnidade.implicitHeight + notaUnidade.implicitHeight
            + rotuloNova.implicitHeight + camposNova.implicitHeight + fracionada.implicitHeight
            + acoesNova.implicitHeight + 6 * spacing + escolher.topPadding + escolher.bottomPadding
        Titulo { id: tituloUnidade; text: "Unidade"; font.pixelSize: 30 }
        Nota {
            id: notaUnidade
            text: "As que vendem quebrado (como 0,350 kg ou 1,5 m) estão marcadas com “quebrado”."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
        Flickable {
            id: rolagem
            objectName: "unidadesGrupos"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(grupos.implicitHeight, Tema.altura * 0.42,
                                             Math.max(110, Tema.altura - coluna.alturaDoResto - 24))
            contentHeight: grupos.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: grupos
                width: rolagem.width
                spacing: 10
                Repeater {
                    model: [escolher.linha1, escolher.linha2, escolher.demais]
                    delegate: GridLayout {
                        id: grade
                        required property var modelData
                        required property int index
                        objectName: "unidadesGrupo" + (index + 1)
                        visible: modelData.length > 0
                        columns: escolher.colunas
                        columnSpacing: 6
                        rowSpacing: 6
                        Repeater {
                            model: grade.modelData
                            delegate: AbstractButton {
                                id: botaoUnidade
                                required property var modelData
                                readonly property bool ativa: modelData.sigla === escolher.atual
                                objectName: "unidade_" + modelData.sigla
                                Layout.preferredWidth: escolher.larguraBotao
                                Layout.fillHeight: true
                                padding: 6
                                implicitHeight: Math.max(Tema.toqueMinimo, 52, implicitContentHeight + topPadding + bottomPadding)
                                focusPolicy: Qt.StrongFocus
                                hoverEnabled: true
                                Accessible.role: Accessible.Button
                                Accessible.name: modelData.sigla + ", " + modelData.descricao
                                                 + (modelData.fracionada ? ", vende quebrado" : "")
                                background: Rectangle {
                                    radius: Tema.raioCampo
                                    color: botaoUnidade.ativa ? Tema.atual
                                         : botaoUnidade.down ? Tema.botaoBaixo
                                         : botaoUnidade.hovered ? Tema.botaoSobre : Tema.papel
                                    border.width: botaoUnidade.visualFocus ? 2 : 1
                                    border.color: botaoUnidade.visualFocus ? Tema.azulEscuro : Tema.sombra(0.18)
                                }
                                // A descrição inteira, sem cortar: quebra em quantas linhas precisar.
                                contentItem: Column {
                                    spacing: 1
                                    Text {
                                        width: parent.width
                                        text: botaoUnidade.modelData.sigla
                                        horizontalAlignment: Text.AlignHCenter
                                        wrapMode: Text.WrapAnywhere
                                        color: botaoUnidade.ativa ? Tema.tintaAtual : Tema.tinta
                                        font.family: Tema.texto
                                        font.pixelSize: 16
                                        font.bold: true
                                    }
                                    Text {
                                        width: parent.width
                                        text: botaoUnidade.modelData.descricao
                                        horizontalAlignment: Text.AlignHCenter
                                        wrapMode: Text.Wrap
                                        color: botaoUnidade.ativa ? Tema.tintaAtual : Tema.tinta
                                        font.family: Tema.texto
                                        font.pixelSize: 13
                                    }
                                    Text {
                                        width: parent.width
                                        visible: botaoUnidade.modelData.fracionada
                                        text: "quebrado"
                                        horizontalAlignment: Text.AlignHCenter
                                        color: botaoUnidade.ativa ? Tema.tintaAtual : Tema.tintaSuave
                                        font.family: Tema.texto
                                        font.pixelSize: 12
                                        font.italic: true
                                    }
                                }
                                onClicked: {
                                    escolher.escolhida(modelData.sigla)
                                    escolher.close()
                                }
                                Keys.onReturnPressed: clicked()
                                Keys.onEnterPressed: clicked()
                            }
                        }
                    }
                }
            }
        }
        Rotulo { id: rotuloNova; text: "Nova unidade"; Layout.topMargin: 4 }
        Linha {
            id: camposNova
            spacing: 10
            Layout.fillWidth: true
            Campo {
                id: sigla_
                rotulo: "Sigla"
                dica: "Ex.: QD"
                maximo: 6
                maiusculas: true
                Layout.preferredWidth: 110
                onConfirmado: descricao.focarSempre()
            }
            Campo {
                id: descricao
                rotulo: "Nome"
                dica: "Ex.: Quadrado de 10x10"
                maximo: 40
                Layout.fillWidth: true
                onConfirmado: escolher.criar()
            }
        }
        Marcar {
            id: fracionada
            text: "Vende quebrado (com vírgula)"
        }
        RowLayout {
            id: acoesNova
            Layout.alignment: Qt.AlignRight
            spacing: 10
            Botao { objectName: "criarUnidade"; text: "Criar e usar"; icone: "mais"; onClicked: escolher.criar() }
        }
    }
}
