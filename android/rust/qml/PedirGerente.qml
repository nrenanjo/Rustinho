import QtQuick
import QtQuick.Layouts
import rustinho

// O "aperto": o gerente digita a senha na hora, sem trocar de usuário.
// Três formas (D-09 e DP-52 (b)):
//   consultar(acao, nome, depois): só para ler a tela da ação, até sair dela;
//   paraChamada(comando, dados, mensagem, depois, desistiu): para esta
//   gravação, com estes dados exatos, uma vez;
//   paraTela(tela, chave, nome, depois, desistiu): para usar esta tela do
//   gerente ("Trazer cadastros") até fechá-la, com a chave que ela sorteou.
// A senha não fica guardada.
Painel {
    id: pedido
    property string acao: ""
    property var acoes: []
    property string comando: ""
    property var dados: null
    property string tela: ""
    property string chaveTela: ""
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
    // Várias ações de uma vez (Configurações › Simultaneidade): uma senha
    // abre a consulta de todas; gravar continua pedindo a senha.
    function consultarVarias(acoes, oQue, depois) {
        consultar("", oQue, depois)
        pedido.acoes = acoes
    }
    function consultar(acao, oQue, depois) {
        pedido.acoes = []
        pedido.acao = acao
        pedido.comando = ""
        pedido.dados = null
        pedido.tela = ""
        pedido.chaveTela = ""
        pedido.oQue = oQue
        pedido.mensagem = ""
        pedido.depois = depois
        pedido.desistiu = null
        abrirPedido()
    }
    function paraChamada(comando, dados, mensagem, depois, desistiu) {
        pedido.acoes = []
        pedido.acao = ""
        pedido.comando = comando
        pedido.dados = dados
        pedido.tela = ""
        pedido.chaveTela = ""
        pedido.oQue = ""
        pedido.mensagem = mensagem
        pedido.depois = depois
        pedido.desistiu = desistiu
        abrirPedido()
    }
    function paraTela(tela, chave, oQue, depois, desistiu) {
        pedido.acoes = []
        pedido.acao = ""
        pedido.comando = ""
        pedido.dados = null
        pedido.tela = tela
        pedido.chaveTela = chave
        pedido.oQue = oQue
        pedido.mensagem = ""
        pedido.depois = depois
        pedido.desistiu = desistiu
        abrirPedido()
    }
    function liberar() {
        const r = Api.chamar("gerente_liberar", pedido.tela !== ""
                             ? { senha: senha.texto, tela: pedido.tela, chave: pedido.chaveTela }
                             : pedido.comando !== ""
                             ? { senha: senha.texto, comando: pedido.comando, dados: pedido.dados }
                             : pedido.acoes.length > 0 ? { senha: senha.texto, acoes: pedido.acoes }
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
            text: pedido.tela !== ""
                  ? (pedido.oQue || "Esta tela") + ": só o gerente pode. Com a senha, você usa esta tela até fechá-la; o cadastro não pede a senha de novo."
                  : pedido.comando !== ""
                  ? (pedido.mensagem || "Só o gerente pode.") + " A senha vale só para isto, uma vez; você continua no seu usuário."
                  : (pedido.oQue ? pedido.oQue + ": s" : "S") + "ó o gerente pode. Com a senha você só consulta "
                    + (pedido.acoes.length > 0 ? "estas telas" : "esta tela") + "; para gravar, ela é pedida de novo."
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
            Botao { text: "Liberar"; icone: "check"; onClicked: pedido.liberar() }
        }
    }
}
