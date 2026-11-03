pragma Singleton
import QtQuick

// Cores, fontes e medidas do Rustinho, tiradas do livro do Baralho Cigano.
QtObject {
    id: tema

    // ---- Modo escuro (sol = cinza de sempre, lua = preto AMOLED) ----
    // Vale só para este aparelho; a janela principal lê e guarda a escolha.
    property bool escuro: false

    // ---- Cores ----
    // No escuro: fundo preto absoluto, faixas com as mesmas cores bem
    // escuras e letras claras, sempre com muito contraste para ler sem esforço.
    readonly property color fundo: escuro ? "#000000" : "#D6D3CD"        // cinza da página do livro
    readonly property color pergaminho: escuro ? "#28231D" : "#EFE5D1"   // faixa ímpar
    readonly property color azul: escuro ? "#1C2629" : "#D9E2E4"         // faixa par
    readonly property color verde: escuro ? "#1E271B" : "#DFE4D6"        // faixa das palavras positivas
    readonly property color rosa: escuro ? "#33221E" : "#EBDCD6"         // faixa das palavras negativas
    readonly property color amarelo: escuro ? "#332B14" : "#EFE0B8"      // "vence hoje"
    readonly property color papel: escuro ? "#0F0D0B" : "#F8F4EB"        // dentro dos campos
    readonly property color tinta: escuro ? "#F4EFE6" : "#2A2622"        // texto principal
    readonly property color tintaSuave: escuro ? "#D6CEC2" : "#5C534A"   // texto secundário
    readonly property color apagado: escuro ? "#ADA397" : "#857B70"      // rótulos
    readonly property color dourado: escuro ? "#A8977F" : "#A3927E"      // capitulares e detalhes
    readonly property color verdeEscuro: escuro ? "#A6CC9B" : "#4F6B4A"  // deu certo
    readonly property color vermelho: escuro ? "#F4A498" : "#94443A"     // erro
    readonly property color azulEscuro: escuro ? "#9CC6D0" : "#3E5F67"   // foco e destaques
    readonly property color claro: escuro ? "#0F0D0B" : "#F3EDE1"        // texto sobre fundo "tinta"
    // Vermelho sobre fundo "tinta" (linha escolhida com estoque baixo).
    readonly property color vermelhoSobre: escuro ? "#8E2B20" : "#F2B8AE"
    readonly property color rosaSobre: escuro ? "#432C27" : "#E4CFC7"
    readonly property color dica: escuro ? "#958B7F" : "#9A9086"          // texto de exemplo nos campos
    // Pílulas da tela inicial e notificações (0.9.15).
    readonly property color cartao: escuro ? "#1A1814" : "#F2EFE9"       // fundo da pílula
    readonly property color bordaCartao: escuro ? "#3A342D" : "#D9D3C8"  // contorno fino
    readonly property color atencao: escuro ? "#2E2714" : "#FCF1DA"      // notificação de atenção
    readonly property color bordaAtencao: escuro ? "#5A4A22" : "#EAD3A2"
    readonly property color critico: escuro ? "#3A1F1B" : "#F6DDD7"      // notificação crítica
    readonly property color bordaCritico: escuro ? "#6B3A33" : "#E2B3A9"
    readonly property color informativo: escuro ? "#1C2629" : "#E6ECEC"  // notificação informativa
    readonly property color bordaInformativo: escuro ? "#34474C" : "#C9D6D8"

    // Botão secundário (normal, mouse em cima, apertado).
    readonly property color botao: escuro ? "#3A332B" : "#E6DAC2"
    readonly property color botaoSobre: escuro ? "#463E34" : "#DDD0B5"
    readonly property color botaoBaixo: escuro ? "#51483D" : "#D3C5A8"
    // Botão principal (cor da tinta) com mouse em cima e apertado.
    readonly property color principalSobre: escuro ? "#E0D9CE" : "#3B352F"
    readonly property color principalBaixo: escuro ? "#C9C1B5" : "#1A1714"
    readonly property color perigoSobre: escuro ? "#F7B8AE" : "#A34D42"
    readonly property color perigoBaixo: escuro ? "#E58F82" : "#7E3930"

    // Branco por cima da faixa (deixa mais claro): no escuro, bem mais leve.
    function vela(a) { return escuro ? Qt.rgba(1, 1, 1, a * 0.16) : Qt.rgba(1, 1, 1, a) }
    // Tinta por cima (bordas, mouse em cima): no escuro, vira claro.
    function sombra(a) { return escuro ? Qt.rgba(0.96, 0.94, 0.9, a * 1.4) : Qt.rgba(0.16, 0.15, 0.13, a) }
    // Texto "claro" um pouco apagado (sobre fundo "tinta").
    function claroSuave(a) { return Qt.rgba(claro.r, claro.g, claro.b, a) }
    // Véu atrás das janelas que abrem por cima.
    readonly property color veu: escuro ? Qt.rgba(0, 0, 0, 0.7) : Qt.rgba(0.16, 0.15, 0.13, 0.35)

    // ---- Olho: esconde CPF/CNPJ, custo, lucro, comissões e saldos ----
    // Começa sempre escondido e vale para todas as telas.
    property bool oculto: true

    // CPF/CNPJ escondido, mantendo o formato ("•••.•••.•••-••").
    function sigilo(t) {
        return oculto && t ? String(t).replace(/[0-9A-Za-z]/g, "•") : (t || "")
    }
    // Texto com valores em reais dentro ("lucro R$ 12,00"): com o olho
    // fechado, os valores somem e o resto do texto fica.
    function sigiloTexto(t) {
        return oculto && t ? String(t).replace(/R\$ ?-?[\d.]+,\d\d/g, "R$ •••") : (t || "")
    }
    // Valor em dinheiro escondido (sem dar ideia do tamanho).
    function valor(t) {
        return oculto && t !== undefined && t !== null && String(t) !== "" ? "•••,••" : (t === undefined || t === null ? "" : String(t))
    }
    // Percentual escondido.
    function pct(t) {
        return oculto && t ? "••" : (t || "")
    }

    // ---- Tamanho da janela (a janela principal atualiza) ----
    // Estreito = celular em pé ou janela pequena: as telas viram uma coluna
    // só, que rola como um feed.
    property real largura: 1280
    property real altura: 760
    // Celular em pé (estreito) ou deitado (baixo): telas em uma coluna, com pílulas.
    readonly property bool baixo: altura < 560
    readonly property bool estreito: largura < 760 || (baixo && largura < 1100)
    // Só a largura: em pé é fino; deitado não (linhas de campos cabem lado a lado).
    readonly property bool fino: largura < 760
    readonly property bool celular: Qt.platform.os === "android" || Qt.platform.os === "ios"

    // ---- Medidas ----
    readonly property real raio: 11                 // cantos das faixas (3 mm no livro)
    readonly property real raioCampo: 7
    readonly property int margem: estreito ? 14 : 32
    readonly property int espaco: 16
    // Altura mínima de um botão ou campo para o dedo.
    readonly property int toque: celular ? 44 : 40
    // "computador" no PC, "aparelho" no celular (nos textos).
    readonly property string aparelho: celular ? "aparelho" : "computador"

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
