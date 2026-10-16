import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import rustinho

// Lista das unidades de medida para escolher (e criar uma nova).
Painel {
    id: escolher
    property var lista: []
    property string atual: ""
    signal escolhida(string sigla)
    width: Math.min(620, Tema.largura - 24)

    function abrirCom(sigla) {
        atual = sigla
        carregar()
        sigla_.texto = ""
        descricao.texto = ""
        fracionada.checked = false
        sigla_.erro = ""
        descricao.erro = ""
        open()
    }
    function carregar() {
        const r = Api.chamar("unidades_listar")
        if (r.ok)
            lista = r.dados
    }
    function criar() {
        sigla_.erro = ""
        descricao.erro = ""
        const r = Api.chamar("unidade_salvar", { sigla: sigla_.texto, descricao: descricao.texto, fracionada: fracionada.checked })
        if (!r.ok) {
            (r.campo === "descricao" ? descricao : sigla_).erro = r.erro
            return
        }
        lista = r.dados.lista
        escolher.escolhida(r.dados.sigla)
        close()
    }

    contentItem: ColumnLayout {
        spacing: 14
        Titulo { text: "Unidade"; font.pixelSize: 30 }
        Nota {
            text: "As que vendem quebrado (como 0,350 kg ou 1,5 m) estão marcadas com “quebrado”."
            font.pixelSize: 14
            Layout.fillWidth: true
        }
        Flickable {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(fluxo.implicitHeight, Tema.altura * 0.42)
            contentHeight: fluxo.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Flow {
                id: fluxo
                width: parent.width
                spacing: 6
                Repeater {
                    model: escolher.lista
                    delegate: Rectangle {
                        id: pilula
                        required property var modelData
                        readonly property bool ativa: modelData.sigla === escolher.atual
                        radius: height / 2
                        height: Tema.celular ? 44 : 38
                        width: textoPilula.implicitWidth + 30
                        color: ativa ? Tema.tinta : sobre.hovered ? Qt.rgba(0.16, 0.15, 0.13, 0.07) : Tema.papel
                        Text {
                            id: textoPilula
                            anchors.centerIn: parent
                            text: "<b>" + pilula.modelData.sigla + "</b>  " + pilula.modelData.descricao
                                  + (pilula.modelData.fracionada ? "  · quebrado" : "")
                            textFormat: Text.StyledText
                            color: pilula.ativa ? Tema.claro : Tema.tinta
                            font.family: Tema.texto
                            font.pixelSize: 15
                        }
                        HoverHandler { id: sobre; cursorShape: Qt.PointingHandCursor }
                        TapHandler {
                            onTapped: {
                                escolher.escolhida(pilula.modelData.sigla)
                                escolher.close()
                            }
                        }
                    }
                }
            }
        }
        Rotulo { text: "Nova unidade"; Layout.topMargin: 4 }
        Linha {
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
            Layout.alignment: Qt.AlignRight
            spacing: 10
            Botao { text: "Voltar"; tipo: "discreto"; onClicked: escolher.close() }
            Botao { text: "Criar e usar"; icone: "mais"; onClicked: escolher.criar() }
        }
    }
}
