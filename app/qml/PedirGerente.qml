import QtQuick
import QtQuick.Layouts
import rustinho

// O "aperto": o gerente digita a senha na hora, sem trocar de usuário.
// Duas formas (D-09):
//   consultar(acao, nome, depois): só para ler a tela da ação, até sair dela;
//   paraChamada(comando, dados, mensagem, depois, desistiu): para esta
//   gravação, com estes dados exatos, uma vez. A senha não fica guardada.
Painel {
    id: pedido
    property string acao: ""
    property string comando: ""
    property var dados: null
    property string oQue: ""
    property string mensagem: ""
    property var depois: null
    property var desistiu: null
    width: Math.min(460, Tema.largura - 24)

    function abrirPedido() {
        senha.texto = ""
        senha.erro = ""
        open()
        senha.focar()
    }
    function consultar(acao, oQue, depois) {
        pedido.acao = acao
        pedido.comando = ""
        pedido.dados = null
        pedido.oQue = oQue
        pedido.mensagem = ""
        pedido.depois = depois
        pedido.desistiu = null
        abrirPedido()
    }
    function paraChamada(comando, dados, mensagem, depois, desistiu) {
        pedido.acao = ""
        pedido.comando = comando
        pedido.dados = dados
        pedido.oQue = ""
        pedido.mensagem = mensagem
        pedido.depois = depois
        pedido.desistiu = desistiu
        abrirPedido()
    }
    function liberar() {
        const r = Api.chamar("gerente_liberar", pedido.comando !== ""
                             ? { senha: senha.texto, comando: pedido.comando, dados: pedido.dados }
                             : { senha: senha.texto, acao: pedido.acao })
        senha.texto = ""
        if (!r.ok) {
            senha.erro = r.erro
            senha.focar()
            return
        }
        const f = depois
        depois = null
        desistiu = null
        close()
        if (f)
            f()
    }
    onClosed: {
        senha.texto = ""
        const f = desistiu
        desistiu = null
        depois = null
        if (f)
            f()
    }

    contentItem: ColumnLayout {
        spacing: 14
        Titulo { text: "Senha do gerente"; font.pixelSize: 30; wrapMode: Text.WordWrap; Layout.fillWidth: true }
        Nota {
            text: pedido.comando !== ""
                  ? (pedido.mensagem || "Só o gerente pode.") + " A senha vale só para isto, uma vez; você continua no seu usuário."
                  : (pedido.oQue ? pedido.oQue + ": s" : "S") + "ó o gerente pode. Com a senha você só consulta esta tela; para gravar, ela é pedida de novo."
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
