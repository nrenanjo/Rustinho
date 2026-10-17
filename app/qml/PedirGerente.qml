import QtQuick
import QtQuick.Layouts
import rustinho

// O "aperto": o gerente digita a senha na hora, sem trocar de usuário.
// Uso: pedir.pedir("Cancelar venda", curta, () => { ... })
Painel {
    id: pedido
    property string oQue: ""
    property bool curta: true
    property var depois: null
    property string erro: ""
    width: Math.min(460, Tema.largura - 24)

    function pedir(oQue, curta, depois) {
        pedido.oQue = oQue
        pedido.curta = curta
        pedido.depois = depois
        pedido.erro = ""
        senha.texto = ""
        open()
        senha.focar()
    }
    function liberar() {
        const r = Api.chamar("gerente_liberar", { senha: senha.texto, curta: pedido.curta })
        if (!r.ok) {
            erro = r.erro
            senha.texto = ""
            senha.focar()
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
            maximo: 128
            erro: pedido.erro
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
