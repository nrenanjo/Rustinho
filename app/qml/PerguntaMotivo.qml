import QtQuick
import QtQuick.Layouts
import rustinho

// Pergunta que pede um motivo (estornar, tirar, devolver...).
// acao(motivo) devolve a resposta da Api; se der certo, avisa com `feito`.
Painel {
    id: motivo
    property string titulo: ""
    property string mensagem: ""
    property string textoBotao: ""
    property string rotulo: "Motivo"
    property string tipoBotao: "perigo"
    property var acao: null
    property string erro: ""
    signal feito()
    width: 500

    function perguntar(titulo, mensagem, textoBotao, acao, rotulo, tipoBotao) {
        motivo.titulo = titulo
        motivo.mensagem = mensagem
        motivo.textoBotao = textoBotao
        motivo.acao = acao
        motivo.rotulo = rotulo || "Motivo"
        motivo.tipoBotao = tipoBotao || "perigo"
        motivo.erro = ""
        campo.texto = ""
        open()
        campo.focar()
    }
    function confirmar() {
        const r = acao(campo.texto)
        if (!r.ok) {
            erro = r.erro
            return
        }
        close()
        feito()
    }

    contentItem: ColumnLayout {
        spacing: 14
        Titulo { text: motivo.titulo; font.pixelSize: 30; wrapMode: Text.WordWrap; Layout.fillWidth: true }
        Nota { text: motivo.mensagem; visible: text !== ""; font.pixelSize: 15; Layout.fillWidth: true }
        Campo {
            id: campo
            rotulo: motivo.rotulo
            erro: motivo.erro
            maximo: 200
            Layout.fillWidth: true
            onConfirmado: motivo.confirmar()
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 10
            Botao { text: "Voltar"; tipo: "discreto"; onClicked: motivo.close() }
            Botao { text: motivo.textoBotao; tipo: motivo.tipoBotao; onClicked: motivo.confirmar() }
        }
    }
}
