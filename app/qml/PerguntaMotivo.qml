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
    width: Math.min(500, Tema.largura - 24)

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
    // A ação devolve a resposta, ou (gravação do gerente, D-09) chama
    // `pronto(r)` depois de pedir a senha e devolve nada.
    function confirmar() {
        const r = acao(campo.texto, resposta => motivo.terminar(resposta))
        if (r !== undefined)
            terminar(r)
    }
    function terminar(r) {
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
