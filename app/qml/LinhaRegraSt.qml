import QtQuick
import QtQuick.Layouts
import rustinho

// Uma regra de ST: estado, começo do CEST, período e se tem ST.
Rectangle {
    id: linha
    required property int index
    required property string uf
    required property string cestInicio
    required property string vigenteDe
    required property string vigenteAte
    required property bool comSt
    required property string descricao
    property var ufs: []

    signal mudou(string campo, var valor)
    signal remover()

    radius: Tema.raioCampo
    color: Qt.rgba(1, 1, 1, 0.35)
    implicitHeight: coluna.implicitHeight + 24

    function campo(nome) {
        return { uf: campoUf, cestInicio: campoCest, vigenteDe: campoDe, vigenteAte: campoAte }[nome] || campoCest
    }

    ColumnLayout {
        id: coluna
        x: 12
        y: 12
        width: parent.width - 24
        spacing: 8

        RowLayout {
            spacing: 8
            Layout.fillWidth: true
            Selecao {
                id: campoUf
                rotulo: "UF"
                modelo: linha.ufs
                Layout.preferredWidth: Tema.estreito ? 64 : 92
                onEscolhido: linha.mudou("uf", linha.ufs[indice])
            }
            Campo {
                id: campoCest
                rotulo: "CEST"
                dica: "01"
                maximo: 9
                Layout.preferredWidth: Tema.estreito ? 56 : 80
                onEditado: linha.mudou("cestInicio", texto)
            }
            Marcar {
                id: campoSt
                text: "Com ST"
                Layout.alignment: Qt.AlignBottom
                Layout.bottomMargin: 6
                Layout.fillWidth: true
                onToggled: linha.mudou("comSt", checked)
            }
        }
        RowLayout {
            spacing: 8
            Layout.fillWidth: true
            Campo {
                id: campoDe
                rotulo: "Vale de"
                dica: "dd/mm/aaaa"
                maximo: 10
                Layout.fillWidth: true
                onEditado: linha.mudou("vigenteDe", texto)
            }
            Campo {
                id: campoAte
                rotulo: "Até"
                dica: "sem fim"
                maximo: 10
                Layout.fillWidth: true
                onEditado: linha.mudou("vigenteAte", texto)
            }
            Botao {
                tipo: "discreto"
                icone: "lixeira"
                implicitWidth: 36
                leftPadding: 0
                rightPadding: 0
                Layout.alignment: Qt.AlignBottom
                onClicked: linha.remover()
            }
        }
        Campo {
            id: campoDescricao
            dica: "Descrição (ex.: Portaria SRE 34/2026)"
            maximo: 120
            altura: 34
            tamanhoFonte: 14
            Layout.fillWidth: true
            onEditado: linha.mudou("descricao", texto)
        }
    }

    Component.onCompleted: {
        campoUf.indice = Math.max(0, linha.ufs.indexOf(uf))
        campoCest.texto = cestInicio
        campoDe.texto = vigenteDe
        campoAte.texto = vigenteAte
        campoSt.checked = comSt
        campoDescricao.texto = descricao
        campoDescricao.entrada.cursorPosition = 0
    }
}
