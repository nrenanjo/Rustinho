pragma Singleton
import QtQuick

// Cores, fontes e medidas do Rustinho, tiradas do livro do Baralho Cigano.
QtObject {
    id: tema

    // ---- Cores ----
    readonly property color fundo: "#D6D3CD"        // cinza da página do livro
    readonly property color pergaminho: "#EFE5D1"   // faixa ímpar
    readonly property color azul: "#D9E2E4"         // faixa par
    readonly property color verde: "#DFE4D6"        // faixa das palavras positivas
    readonly property color rosa: "#EBDCD6"         // faixa das palavras negativas
    readonly property color papel: "#F8F4EB"        // dentro dos campos
    readonly property color tinta: "#2A2622"        // texto principal
    readonly property color tintaSuave: "#5C534A"   // texto secundário
    readonly property color apagado: "#857B70"      // rótulos
    readonly property color dourado: "#A3927E"      // capitulares e detalhes
    readonly property color verdeEscuro: "#4F6B4A"  // deu certo
    readonly property color vermelho: "#94443A"     // erro
    readonly property color azulEscuro: "#3E5F67"   // foco e destaques
    readonly property color claro: "#F3EDE1"        // texto sobre fundo escuro

    // ---- Olho: esconde CPF/CNPJ, custo, lucro, comissões e saldos ----
    // Começa sempre escondido e vale para todas as telas.
    property bool oculto: true

    // CPF/CNPJ escondido, mantendo o formato ("•••.•••.•••-••").
    function sigilo(t) {
        return oculto && t ? String(t).replace(/[0-9A-Za-z]/g, "•") : (t || "")
    }
    // Valor em dinheiro escondido (sem dar ideia do tamanho).
    function valor(t) {
        return oculto && t !== undefined && t !== null && String(t) !== "" ? "•••,••" : (t === undefined || t === null ? "" : String(t))
    }
    // Percentual escondido.
    function pct(t) {
        return oculto && t ? "••" : (t || "")
    }

    // ---- Medidas ----
    readonly property real raio: 11                 // cantos das faixas (3 mm no livro)
    readonly property real raioCampo: 7
    readonly property int margem: 32
    readonly property int espaco: 16

    // ---- Fontes (dentro do programa, não precisam estar instaladas) ----
    readonly property string caminho: "qrc:/qt/qml/rustinho/fontes/"
    property FontLoader _titulo500: FontLoader { source: tema.caminho + "CormorantGaramondAjustada-Medium.ttf" }
    property FontLoader _titulo600: FontLoader { source: tema.caminho + "CormorantGaramondAjustada-SemiBold.ttf" }
    property FontLoader _titulo700: FontLoader { source: tema.caminho + "CormorantGaramondAjustada-Bold.ttf" }
    property FontLoader _titulo500i: FontLoader { source: tema.caminho + "CormorantGaramondAjustada-MediumItalic.ttf" }
    property FontLoader _titulo600i: FontLoader { source: tema.caminho + "CormorantGaramondAjustada-SemiBoldItalic.ttf" }
    property FontLoader _sans400: FontLoader { source: tema.caminho + "source-sans-3-latin-400-normal.ttf" }
    property FontLoader _sans400i: FontLoader { source: tema.caminho + "source-sans-3-latin-400-italic.ttf" }
    property FontLoader _sans600: FontLoader { source: tema.caminho + "source-sans-3-latin-600-normal.ttf" }
    property FontLoader _sans700: FontLoader { source: tema.caminho + "source-sans-3-latin-700-normal.ttf" }
    property FontLoader _serif400: FontLoader { source: tema.caminho + "source-serif-4-latin-400-normal.ttf" }
    property FontLoader _serif400i: FontLoader { source: tema.caminho + "source-serif-4-latin-400-italic.ttf" }
    property FontLoader _serif600: FontLoader { source: tema.caminho + "source-serif-4-latin-600-normal.ttf" }
    property FontLoader _serif600i: FontLoader { source: tema.caminho + "source-serif-4-latin-600-italic.ttf" }

    // Títulos (Cormorant Garamond Ajustada): use com font.weight 500, 600 ou 700.
    readonly property string titulo: _titulo600.name

    // Texto (Source Sans 3): use com font.weight (Normal, DemiBold, Bold).
    readonly property string texto: _sans400.name

    // Texto com serifa (Source Serif 4), para descrições e notas.
    readonly property string serifa: _serif400.name
}
