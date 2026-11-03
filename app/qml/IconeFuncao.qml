import QtQuick
import rustinho

// O ícone de uma função num quadrado de fundo pastel (o mesmo das pílulas da
// tela inicial e dos quadrados das notificações minimizadas). As cores são
// por função (claro, escuro).
Rectangle {
    id: icone
    property string funcao: ""
    property string nome: "caixa"
    property real tamanhoIcone: Tema.estreito ? 24 : 27

    readonly property var cores: ({
        vender: ["#DCE7F8", "#2A6FD6", "#1D2A3D", "#8DB5F0"],
        pecas: ["#DDEBD8", "#4D8A3E", "#1E2A1B", "#9CCB8E"],
        orcamentos: ["#FBE0C7", "#D06D17", "#35261A", "#F0AE70"],
        caixa: ["#E6DDF5", "#7E52BF", "#2A2238", "#C1A6EE"],
        trocas: ["#F6D9D6", "#B84E43", "#3A2220", "#EE9F95"],
        notas: ["#F4E6C4", "#A67E25", "#342B17", "#E1C27A"],
        clientes: ["#DCE7F8", "#2A6FD6", "#1D2A3D", "#8DB5F0"],
        compras: ["#DDEBD8", "#4D8A3E", "#1E2A1B", "#9CCB8E"],
        estoque: ["#FBE0C7", "#D06D17", "#35261A", "#F0AE70"],
        financeiro: ["#E6DDF5", "#7E52BF", "#2A2238", "#C1A6EE"],
        oficina: ["#DDEBD8", "#4D8A3E", "#1E2A1B", "#9CCB8E"],
        painel: ["#DCE7F8", "#2A6FD6", "#1D2A3D", "#8DB5F0"],
        usuarios: ["#F6D9D6", "#B84E43", "#3A2220", "#EE9F95"],
        configuracoes: ["#E3E1DD", "#5F5A54", "#2A2826", "#BDB7AF"]
    })
    readonly property var cor: cores[funcao] || ["#E3E1DD", "#5F5A54", "#2A2826", "#BDB7AF"]

    implicitWidth: Tema.estreito ? 42 : 48
    implicitHeight: implicitWidth
    radius: 8
    color: Tema.escuro ? cor[2] : cor[0]
    Icone {
        anchors.centerIn: parent
        nome: icone.nome || "caixa"
        tamanho: icone.tamanhoIcone
        espessura: 2.1
        cor: Tema.escuro ? icone.cor[3] : icone.cor[1]
    }
}
