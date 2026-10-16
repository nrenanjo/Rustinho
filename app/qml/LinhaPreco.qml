import QtQuick
import QtQuick.Layouts
import rustinho

// Uma tabela de preço: percentual de lucro sobre o custo e preço de venda.
// Mexer no percentual calcula o preço; mexer no preço calcula o percentual.
RowLayout {
    id: linha
    property int tabela: 1
    property string nome: ""
    property string lucroInicial: ""
    property string precoInicial: ""
    property string custo: ""
    property bool destaque: false
    property alias campoLucro: lucro
    property alias campoPreco: preco
    readonly property string lucroTexto: lucro.texto
    readonly property string precoTexto: preco.texto

    signal editado()

    spacing: 12

    // `custoAtual`: quem mexeu no custo passa o valor na hora, porque a
    // ligação `custo` só é atualizada depois do sinal de edição.
    function recalcular(de, custoAtual) {
        const valorCusto = custoAtual !== undefined ? custoAtual : linha.custo
        const r = Api.chamar("preco_calcular", { custo: valorCusto, lucro: lucro.texto, preco: preco.texto, de: de })
        if (!r.ok)
            return
        if (de !== "lucro" && lucro.texto !== r.dados.lucro)
            lucro.texto = r.dados.lucro
        if (de !== "preco" && preco.texto !== r.dados.preco)
            preco.texto = r.dados.preco
    }

    function formatar(campo, tipo) {
        const r = Api.chamar("numero_formatar", { tipo: tipo, texto: campo.texto })
        if (r.ok && r.dados.texto !== campo.texto)
            campo.texto = r.dados.texto
    }

    Campo {
        id: lucro
        sensivel: true
        rotulo: linha.destaque ? "% sobre o custo" : linha.nome
        sufixo: "%"
        numerico: true
        maximo: 12
        texto: linha.lucroInicial
        tamanhoFonte: linha.destaque ? 18 : 16
        altura: linha.destaque ? 46 : 40
        Layout.preferredWidth: linha.destaque ? 150 : 120
        onEditado: {
            linha.recalcular("lucro")
            linha.editado()
        }
        onSaiu: linha.formatar(lucro, "percentual")
    }

    Campo {
        id: preco
        rotulo: linha.destaque ? "Preço de " + linha.nome.toLowerCase() : "Preço"
        prefixo: "R$"
        numerico: true
        maximo: 18
        texto: linha.precoInicial
        tamanhoFonte: linha.destaque ? 22 : 16
        altura: linha.destaque ? 46 : 40
        entrada.font.weight: linha.destaque ? Font.Bold : Font.Normal
        Layout.fillWidth: true
        onEditado: {
            linha.recalcular("preco")
            linha.editado()
        }
        onSaiu: linha.formatar(preco, "dinheiro")
    }
}
