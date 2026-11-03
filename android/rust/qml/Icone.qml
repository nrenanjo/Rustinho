import QtQuick
import QtQuick.Shapes
import rustinho

// Ícones simples, desenhados com linhas numa grade de 24 x 24.
Item {
    id: icone
    property string nome: "busca"
    property color cor: Tema.tinta
    property real tamanho: 20
    property real espessura: 1.9

    implicitWidth: tamanho
    implicitHeight: tamanho

    readonly property var desenhos: ({
        "sol": "M 16 12 A 4 4 0 1 1 8 12 A 4 4 0 1 1 16 12 M 12 2.5 L 12 4.5 M 12 19.5 L 12 21.5 M 2.5 12 L 4.5 12 M 19.5 12 L 21.5 12 M 5.3 5.3 L 6.7 6.7 M 17.3 17.3 L 18.7 18.7 M 5.3 18.7 L 6.7 17.3 M 17.3 6.7 L 18.7 5.3",
        "lua": "M 19.5 14.5 A 8 8 0 1 1 9.5 4.5 A 6.5 6.5 0 0 0 19.5 14.5 Z",
        "camera": "M 3.5 8 L 7.5 8 L 9 5.5 L 15 5.5 L 16.5 8 L 20.5 8 L 20.5 18.5 L 3.5 18.5 Z M 15.5 13 A 3.5 3.5 0 1 1 8.5 13 A 3.5 3.5 0 1 1 15.5 13",
        "busca": "M 17 10.5 A 6.5 6.5 0 1 1 4 10.5 A 6.5 6.5 0 1 1 17 10.5 M 15.4 15.4 L 20 20",
        "voltar": "M 19 12 L 5 12 M 11 6 L 5 12 L 11 18",
        "mais": "M 12 5 L 12 19 M 5 12 L 19 12",
        "fechar": "M 6.5 6.5 L 17.5 17.5 M 17.5 6.5 L 6.5 17.5",
        "lixeira": "M 4.5 7 L 19.5 7 M 9.5 7 L 9.5 4.5 L 14.5 4.5 L 14.5 7 M 6.5 7 L 7.5 19.5 L 16.5 19.5 L 17.5 7 M 10.5 10.5 L 10.5 16 M 13.5 10.5 L 13.5 16",
        "check": "M 5 12.5 L 10 17.5 L 19 7",
        "baixo": "M 6 9.5 L 12 15.5 L 18 9.5",
        "cima": "M 6 14.5 L 12 8.5 L 18 14.5",
        "direita": "M 9.5 6 L 15.5 12 L 9.5 18",
        "pino": "M 12 15.5 L 12 21 M 7.5 3.5 L 16.5 3.5 M 9 3.5 L 9 9 L 6 13 L 6 15.5 L 18 15.5 L 18 13 L 15 9 L 15 3.5",
        "sair": "M 13.5 5 L 13.5 4 L 4.5 4 L 4.5 20 L 13.5 20 L 13.5 19 M 9.5 12 L 20.5 12 M 16.5 8 L 20.5 12 L 16.5 16",
        "caixa": "M 4 8 L 12 4 L 20 8 L 20 16 L 12 20 L 4 16 Z M 4 8 L 12 12 L 20 8 M 12 12 L 12 20",
        "pessoa": "M 16 8 A 4 4 0 1 1 8 8 A 4 4 0 1 1 16 8 M 4.5 20 C 5.5 15 18.5 15 19.5 20",
        "lapis": "M 4.5 19.5 L 5 16 L 16 5 L 19 8 L 8 19 Z M 14 7 L 17 10",
        "ajuste": "M 4 8 L 20 8 M 4 16 L 20 16 M 9 5.5 L 9 10.5 M 15 13.5 L 15 18.5",
        "relogio": "M 20 12 A 8 8 0 1 1 4 12 A 8 8 0 1 1 20 12 M 12 7.5 L 12 12 L 15 14",
        "mensagem": "M 4.5 5.5 L 19.5 5.5 L 19.5 16 L 11 16 L 7 19.5 L 7 16 L 4.5 16 Z",
        "olho": "M 2.5 12 C 6 6 18 6 21.5 12 C 18 18 6 18 2.5 12 Z M 15 12 A 3 3 0 1 1 9 12 A 3 3 0 1 1 15 12",
        "olhoFechado": "M 3 10 C 7 15.5 17 15.5 21 10 M 6.4 13.4 L 4.8 15.6 M 12 15 L 12 17.8 M 17.6 13.4 L 19.2 15.6",
        "impressora": "M 7.5 9 L 7.5 4.5 L 16.5 4.5 L 16.5 9 M 7.5 16.5 L 4.5 16.5 L 4.5 9 L 19.5 9 L 19.5 16.5 L 16.5 16.5 M 7.5 13 L 16.5 13 L 16.5 20 L 7.5 20 Z",
        // Pílulas da tela inicial (0.9.15).
        "carrinho": "M 2.5 4.5 L 5.5 4.5 L 7.5 15 L 18 15 L 20 7.5 L 6.3 7.5 M 10.5 19 A 1.4 1.4 0 1 1 7.7 19 A 1.4 1.4 0 1 1 10.5 19 M 18 19 A 1.4 1.4 0 1 1 15.2 19 A 1.4 1.4 0 1 1 18 19",
        "engrenagem": "M 18.75 10.13 L 21.08 10.52 L 21.08 13.48 L 18.75 13.87 L 18.09 15.45 L 19.47 17.37 L 17.37 19.47 L 15.45 18.09 L 13.87 18.75 L 13.48 21.08 L 10.52 21.08 L 10.13 18.75 L 8.55 18.09 L 6.63 19.47 L 4.53 17.37 L 5.91 15.45 L 5.25 13.87 L 2.92 13.48 L 2.92 10.52 L 5.25 10.13 L 5.91 8.55 L 4.53 6.63 L 6.63 4.53 L 8.55 5.91 L 10.13 5.25 L 10.52 2.92 L 13.48 2.92 L 13.87 5.25 L 15.45 5.91 L 17.37 4.53 L 19.47 6.63 L 18.09 8.55 Z M 15 12 A 3 3 0 1 1 9 12 A 3 3 0 1 1 15 12",
        "documento": "M 6 3.5 L 14 3.5 L 18.5 8 L 18.5 20.5 L 6 20.5 Z M 14 3.5 L 14 8 L 18.5 8 M 9 12 L 15.5 12 M 9 15 L 15.5 15 M 9 18 L 13 18",
        "registradora": "M 4 20 L 20 20 L 20 13.5 L 4 13.5 Z M 6 13.5 L 7 8.5 L 17 8.5 L 18 13.5 M 9.5 8.5 L 9.5 4.5 L 15 4.5 L 15 8.5 M 7.5 17 L 9 17 M 11.25 17 L 12.75 17 M 15 17 L 16.5 17 M 8.5 11 L 15.5 11",
        "trocas": "M 4 8.5 L 18.5 8.5 M 14.5 4.5 L 18.5 8.5 L 14.5 12.5 M 20 15.5 L 5.5 15.5 M 9.5 11.5 L 5.5 15.5 L 9.5 19.5",
        "nota": "M 6 3.5 L 18 3.5 L 18 20.5 L 15.6 19 L 13.2 20.5 L 10.8 19 L 8.4 20.5 L 6 19 Z M 9 8 L 15 8 M 9 11.5 L 15 11.5 M 9 15 L 12.5 15",
        "clientes": "M 15 8 A 3 3 0 1 1 9 8 A 3 3 0 1 1 15 8 M 6.5 19.5 C 7 14.5 17 14.5 17.5 19.5 M 21 9.5 A 2.2 2.2 0 1 1 16.6 9.5 M 3 9.5 A 2.2 2.2 0 1 0 7.4 9.5 M 18.5 14 C 20.5 14.3 21.5 15.8 21.8 18 M 5.5 14 C 3.5 14.3 2.5 15.8 2.2 18",
        "cesta": "M 3 10 L 21 10 L 19 19.5 L 5 19.5 Z M 7.5 10 L 11 4 M 16.5 10 L 13 4 M 9 13 L 9.5 16.5 M 12 13 L 12 16.5 M 15 13 L 14.5 16.5",
        "barras": "M 4 20 L 20 20 M 6.5 20 L 6.5 13 L 9.5 13 L 9.5 20 M 10.5 20 L 10.5 8 L 13.5 8 L 13.5 20 M 14.5 20 L 14.5 4.5 L 17.5 4.5 L 17.5 20",
        "chave": "M 14.5 3.8 A 5 5 0 0 0 9.8 10.6 L 3.8 16.6 A 2.3 2.3 0 0 0 7.4 20.2 L 13.4 14.2 A 5 5 0 0 0 20.2 9.5 L 17 12.7 L 13.3 12.4 L 13 8.7 L 16.2 5.5 A 5 5 0 0 0 14.5 3.8 Z",
        "pizza": "M 11 4 A 8.5 8.5 0 1 0 20 13 L 11 13 Z M 13.5 2.5 A 8.5 8.5 0 0 1 21.5 10.5 L 13.5 10.5 Z",
        // Configurações › Simultaneidade › Fiscal (0.9.15).
        "documentoElo": "M 11 20.5 L 5.5 20.5 L 5.5 3.5 L 13.5 3.5 L 18 8 L 18 12 M 13.5 3.5 L 13.5 8 L 18 8 M 8.5 11 L 14.5 11 M 8.5 14 L 12 14 M 15.4 15.6 L 16.8 14.2 A 2 2 0 0 1 19.6 17 L 18.2 18.4 M 17.6 19 L 16.2 20.4 A 2 2 0 0 1 13.4 17.6 L 14.8 16.2 M 15.6 18.2 L 17.4 16.4",
        "documentoRelogio": "M 11.5 20.5 L 5.5 20.5 L 5.5 3.5 L 13.5 3.5 L 18 8 L 18 11 M 13.5 3.5 L 13.5 8 L 18 8 M 8.5 11 L 14.5 11 M 8.5 14 L 12 14 M 21.5 17 A 4 4 0 1 1 13.5 17 A 4 4 0 1 1 21.5 17 M 17.5 15 L 17.5 17 L 19 18"
    })

    Shape {
        anchors.fill: parent
        layer.enabled: true
        layer.samples: 4
        layer.smooth: true

        ShapePath {
            scale: Qt.size(icone.tamanho / 24, icone.tamanho / 24)
            strokeColor: icone.cor
            strokeWidth: icone.espessura
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg { path: icone.desenhos[icone.nome] || "" }
        }
    }
}
