import QtQuick
import QtQuick.Layouts
import rustinho

// O "aperto": o gerente digita a senha na hora, sem trocar de usuário.
// Uso: pedir.pedir("cancelar_venda", "Cancelar venda", curta, () => { ... })
// A senha vale só para a ação pedida; a de uma ação só acaba quando é usada.
Painel {
    id: pedido
    property string acao: ""
    property string oQue: ""
    property bool curta: true
    property var depois: null
    width: Math.min(460, Tema.largura - 24)

    function pedir(acao, oQue, curta, depois) {
        pedido.acao = acao
        pedido.oQue = oQue
        pedido.curta = curta
        pedido.depois = depois
        senha.texto = ""
        senha.erro = ""
        open()
        senha.focar()
    }
    function liberar() {
        const r = Api.chamar("gerente_liberar", { senha: senha.texto, curta: pedido.curta, acao: pedido.acao })
        if (!r.ok) {
            senha.erro = r.erro
            senha.focar()
            senha.entrada.selectAll()
            return
        }
        const f = depois
        depois = null
        close()
        if (f)
            f()
    }

    contentItem: ColumnLayout {
        spacing: 14
        Titulo { text: "Senha do gerente"; font.pixelSize: 30; wrapMode: Text.WordWrap; Layout.fillWidth: true }
        Nota {
            text: (pedido.oQue ? pedido.oQue + ": s" : "S") + "ó o gerente pode. Peça para um gerente digitar a senha aqui; você continua no seu usuário."
            font.pixelSize: 15
            Layout.fillWidth: true
        }
        Campo {
            id: senha
            rotulo: "Senha do gerente"
            senha: true
            maximo: 256
            Layout.fillWidth: true
            onConfirmado: pedido.liberar()
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: 10
            Botao { text: "Voltar"; tipo: "discreto"; onClicked: pedido.close() }
            Botao { text: "Liberar"; icone: "check"; onClicked: pedido.liberar() }
        }
    }
}
