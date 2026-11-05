pragma Singleton
import QtQuick

// Cores, fontes e medidas do Rustinho, tiradas do livro do Baralho Cigano.
QtObject {
    // O id não pode ser "tema": no QML o id vence a propriedade "tema" (0.9.18).
    id: raizDoTema

    // ---- Tema (0.9.18): sol = cinza de sempre, lua = preto AMOLED,
    // planeta = Planeta Roxo ----
    // Vale só para este aparelho; a janela principal lê e guarda a escolha.
    property string tema: "cinza"
    // O "escuro" de antes continua (só leitura): é o AMOLED.
    readonly property bool escuro: tema === "amoled"
    readonly property bool roxo: tema === "roxo"
    // Planeta Roxo: as cores exatas das imagens aprovadas dão texto branco
    // sobre magenta com 3,3:1 (botão principal e linha escolhida), abaixo dos
    // 4,5:1 do texto comum. Por padrão o magenta é um pouco mais fechado
    // (4,6:1); true = as cores exatas do print (decisão do dono, D-V2).
    readonly property bool roxoFiel: false

    // ---- Cores ----
    // No escuro: fundo preto absoluto, faixas com as mesmas cores bem
    // escuras e letras claras, sempre com muito contraste para ler sem esforço.
    // No roxo: fundo roxo com letras claras; painéis, campos e faixas claras
    // com letras escuras (as cores abaixo são as dos painéis; o texto direto
    // no fundo roxo usa as funções "...No(superfície)", mais abaixo).
    readonly property color fundo: roxo ? "#770273" : escuro ? "#000000" : "#D6D3CD"        // cinza da página do livro
    readonly property color pergaminho: roxo ? "#DDDCDF" : escuro ? "#28231D" : "#EFE5D1"   // faixa ímpar
    readonly property color azul: roxo ? "#D6D0DC" : escuro ? "#1C2629" : "#D9E2E4"         // faixa par
    readonly property color verde: roxo ? "#D9DED8" : escuro ? "#1E271B" : "#DFE4D6"        // faixa das palavras positivas
    readonly property color rosa: roxo ? "#EDD5E3" : escuro ? "#33221E" : "#EBDCD6"         // faixa das palavras negativas
    readonly property color amarelo: roxo ? "#F1E2BE" : escuro ? "#332B14" : "#EFE0B8"      // "vence hoje"
    readonly property color papel: roxo ? "#F5F5F5" : escuro ? "#0F0D0B" : "#F8F4EB"        // dentro dos campos
    readonly property color tinta: roxo ? "#231A26" : escuro ? "#F4EFE6" : "#2A2622"        // texto principal
    readonly property color tintaSuave: roxo ? "#4A3F4E" : escuro ? "#D6CEC2" : "#5C534A"   // texto secundário
    readonly property color apagado: roxo ? "#5A2D63" : escuro ? "#ADA397" : "#857B70"      // rótulos
    readonly property color dourado: roxo ? "#744679" : escuro ? "#A8977F" : "#A3927E"      // capitulares e detalhes
    readonly property color verdeEscuro: roxo ? "#3E5A39" : escuro ? "#A6CC9B" : "#4F6B4A"  // deu certo
    readonly property color vermelho: roxo ? "#8C3530" : escuro ? "#F4A498" : "#94443A"     // erro
    readonly property color azulEscuro: roxo ? "#8A1679" : escuro ? "#9CC6D0" : "#3E5F67"   // foco e destaques
    readonly property color claro: roxo ? "#FFFFFF" : escuro ? "#0F0D0B" : "#F3EDE1"        // texto sobre fundo "tinta"
    // Vermelho sobre fundo "tinta" (linha escolhida com estoque baixo).
    readonly property color vermelhoSobre: roxo ? "#FFFFFF" : escuro ? "#8E2B20" : "#F2B8AE"
    readonly property color rosaSobre: roxo ? "#E5C6D8" : escuro ? "#432C27" : "#E4CFC7"
    readonly property color dica: roxo ? "#6E6472" : escuro ? "#958B7F" : "#9A9086"          // texto de exemplo nos campos
    // Pílulas da tela inicial e notificações (0.9.15).
    readonly property color cartao: roxo ? "#DDDBDE" : escuro ? "#1A1814" : "#F2EFE9"       // fundo da pílula
    readonly property color bordaCartao: roxo ? "#5A0257" : escuro ? "#3A342D" : "#D9D3C8"  // contorno fino
    readonly property color atencao: escuro ? "#2E2714" : "#FCF1DA"      // notificação de atenção
    readonly property color bordaAtencao: escuro ? "#5A4A22" : "#EAD3A2"
    readonly property color critico: escuro ? "#3A1F1B" : "#F6DDD7"      // notificação crítica
    readonly property color bordaCritico: escuro ? "#6B3A33" : "#E2B3A9"
    readonly property color informativo: escuro ? "#1C2629" : "#E6ECEC"  // notificação informativa
    readonly property color bordaInformativo: escuro ? "#34474C" : "#C9D6D8"

    // Botão secundário (normal, mouse em cima, apertado) e o texto dele.
    readonly property color botao: roxo ? "#FACBE3" : escuro ? "#3A332B" : "#E6DAC2"
    readonly property color botaoSobre: roxo ? "#F3BCD9" : escuro ? "#463E34" : "#DDD0B5"
    readonly property color botaoBaixo: roxo ? "#EBADCF" : escuro ? "#51483D" : "#D3C5A8"
    readonly property color tintaBotao: roxo ? "#3A0B45" : tinta
    // Botão principal (cor da tinta; no roxo, magenta) com mouse em cima e
    // apertado, e o texto dele.
    readonly property color principal: roxo ? (roxoFiel ? "#FB2ACD" : "#D123AB") : tinta
    readonly property color sobrePrincipal: roxo ? "#FFFFFF" : claro
    readonly property color principalSobre: roxo ? (roxoFiel ? "#F04BCF" : "#C21E9E") : escuro ? "#E0D9CE" : "#3B352F"
    readonly property color principalBaixo: roxo ? "#A8198A" : escuro ? "#C9C1B5" : "#1A1714"
    readonly property color perigoSobre: escuro ? "#F7B8AE" : "#A34D42"
    readonly property color perigoBaixo: escuro ? "#E58F82" : "#7E3930"

    // Linha escolhida numa lista (cor da tinta; no roxo, magenta) e o texto.
    readonly property color atual: roxo ? (roxoFiel ? "#FA26C9" : "#D123AB") : tinta
    readonly property color tintaAtual: roxo ? "#FFFFFF" : claro
    // Selo (chip) dentro da linha escolhida: no roxo, um véu escuro deixa o
    // texto branco legível sobre o magenta.
    readonly property color chipAtual: roxo ? Qt.rgba(0.2, 0, 0.2, 0.35) : vela(0.12)
    // Pílulas: a escolhida no fundo da tela e a escolhida dentro de uma faixa
    // (subpílulas, pílulas num painel), com o texto de cada uma.
    readonly property color pilulaAtiva: roxo ? "#F94BBF" : tinta
    readonly property color sobrePilulaAtiva: roxo ? "#1E0B1D" : claro
    readonly property color subpilulaAtiva: roxo ? "#610161" : tinta
    readonly property color sobreSubpilulaAtiva: roxo ? "#FFFFFF" : claro
    // A faixa das subpílulas (Configurações › Simultaneidade).
    readonly property color faixaSubpilulas: roxo ? "#F22EB6" : sombra(0.06)
    // Busca das telas (campo com a lupa): no roxo, escura com borda magenta.
    readonly property color buscaFundo: roxo ? "#4E114B" : papel
    readonly property color buscaBorda: roxo ? "#F038D0" : azulEscuro
    // Pílula do olho ("Mostrar valores") e do tema (sol, lua, planeta).
    readonly property color pilulaClara: roxo ? "#E4D1E5" : vela(0.45)
    readonly property color pilulaClaraSobre: roxo ? "#ECDDED" : vela(0.7)
    readonly property color seletorFundo: roxo ? "#3C223B" : vela(0.45)
    readonly property color seletorAtivo: roxo ? "#FD2BD1" : tinta
    // Campo só de leitura.
    readonly property color campoFixo: roxo ? "#E6E1E8" : vela(0.28)

    // ---- Planeta Roxo: as faixas das listas e o texto claro ----
    readonly property color roxoFaixaClara: "#DDDBDE"
    readonly property color roxoFaixaEscura: "#2C272C"
    readonly property color roxoTintaClara: "#FBF7FB"
    readonly property color roxoTintaSuaveClara: "#EBDDE9"
    readonly property color roxoApagadoClaro: "#E2CCDF"
    readonly property color roxoVermelhoClaro: "#FFB0C4"
    readonly property color roxoVerdeClaro: "#A6CC9B"
    readonly property color roxoAzulClaro: "#F9B6EC"
    readonly property color roxoDouradoClaro: "#E9CFA9"

    // Branco por cima da faixa (deixa mais claro): no escuro, bem mais leve.
    function vela(a) { return escuro ? Qt.rgba(1, 1, 1, a * 0.16) : Qt.rgba(1, 1, 1, a) }
    // Tinta por cima (bordas, mouse em cima): no escuro, vira claro.
    function sombra(a) { return escuro ? Qt.rgba(0.96, 0.94, 0.9, a * 1.4) : roxo ? Qt.rgba(0.14, 0.1, 0.15, a) : Qt.rgba(0.16, 0.15, 0.13, a) }
    // Texto "claro" um pouco apagado (sobre fundo "tinta"). No roxo, o texto
    // (a ≥ 0,5) fica inteiro: sobre o magenta, qualquer transparência fica
    // abaixo de 4,5:1; o fundo leve (a < 0,5) continua transparente.
    function claroSuave(a) { return roxo && a >= 0.5 ? claro : Qt.rgba(claro.r, claro.g, claro.b, a) }
    // Véu atrás das janelas que abrem por cima.
    readonly property color veu: roxo ? Qt.rgba(0.12, 0, 0.12, 0.55) : escuro ? Qt.rgba(0, 0, 0, 0.7) : Qt.rgba(0.16, 0.15, 0.13, 0.35)

    // ---- Superfícies (0.9.18) ----
    // No roxo há fundo escuro (a janela, a faixa escura das listas) e
    // superfícies claras (painéis, campos, faixa clara): o texto troca de cor
    // conforme a superfície embaixo dele. Cada superfície diz a sua cor em
    // `palette.window` (os itens de dentro herdam); os componentes de texto
    // (Titulo, Nota, Rotulo, Marcar, Botao discreto, Pilulas) leem dali.
    // No cinza e no AMOLED tudo continua como antes: as funções devolvem a
    // cor de sempre e a superfície não importa.
    function _linear(c) { return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4) }
    function luminancia(c) {
        const k = typeof c === "string" ? Qt.lighter(c, 1.0) : c
        return 0.2126 * _linear(k.r) + 0.7152 * _linear(k.g) + 0.0722 * _linear(k.b)
    }
    // A superfície pede texto claro? (só no roxo)
    function escura(sup) { return roxo && luminancia(sup) < 0.18 }
    // Cor a pôr em `palette.window` de uma superfície (no cinza e no AMOLED,
    // a mesma da janela: nada muda para os controles de dentro).
    function superficie(cor) { return roxo ? cor : fundo }
    function tintaNo(sup) { return escura(sup) ? roxoTintaClara : tinta }
    function tintaSuaveNo(sup) { return escura(sup) ? roxoTintaSuaveClara : tintaSuave }
    function apagadoNo(sup) { return escura(sup) ? roxoApagadoClaro : apagado }
    function vermelhoNo(sup) { return escura(sup) ? roxoVermelhoClaro : vermelho }
    function verdeEscuroNo(sup) { return escura(sup) ? roxoVerdeClaro : verdeEscuro }
    function azulEscuroNo(sup) { return escura(sup) ? roxoAzulClaro : azulEscuro }
    function douradoNo(sup) { return escura(sup) ? roxoDouradoClaro : dourado }
    // Mouse em cima / apertado de algo sem fundo próprio sobre a superfície.
    function sombraNo(a, sup) { return escura(sup) ? Qt.rgba(1, 1, 1, a * 1.4) : sombra(a) }

    // ---- Linhas alternadas das listas (V-6, 0.9.18) ----
    // par = (índice + 1) % 2 === 0, como nas listas de sempre: a primeira
    // linha é a "ímpar". a/b: as cores desta lista no cinza e no AMOLED
    // (padrão: pergaminho e azul). No roxo: faixa clara e faixa escura, com o
    // texto invertido (escuro na clara, claro na escura).
    function faixa(par, a, b) {
        if (roxo)
            return par ? roxoFaixaEscura : roxoFaixaClara
        return par ? (b === undefined ? azul : b) : (a === undefined ? pergaminho : a)
    }
    function tintaDaFaixa(par) { return roxo && par ? roxoTintaClara : tinta }
    function tintaSuaveDaFaixa(par) { return roxo && par ? roxoTintaSuaveClara : tintaSuave }
    function apagadoDaFaixa(par) { return roxo && par ? roxoApagadoClaro : apagado }
    function vermelhoDaFaixa(par) { return roxo && par ? roxoVermelhoClaro : vermelho }
    function verdeEscuroDaFaixa(par) { return roxo && par ? roxoVerdeClaro : verdeEscuro }
    function azulEscuroDaFaixa(par) { return roxo && par ? roxoAzulClaro : azulEscuro }
    function douradoDaFaixa(par) { return roxo && par ? roxoDouradoClaro : dourado }

    // ---- Seta de retorno (V-2 e V-4, 0.9.18; componente Seta.qml) ----
    // Área de toque: 40 × 40 px no computador, 48 × 48 dp no celular.
    readonly property int toqueSeta: celular ? 48 : 40
    // D-V1 (decisão em aberto; padrão provisório): o nome do destino ao lado
    // da seta ("← Início", "← Compras"), como hoje. A navegação guiada usa
    // esse texto para dizer para onde a seta volta, e os testes conferem.
    // false = só a seta, como em comoficará.png (o nome do destino continua
    // como nome acessível do botão).
    readonly property bool setaComTexto: true
    // D-V2 (decisão em aberto; padrão provisório): a cor da seta é a de
    // maior contraste em cada tema, a do texto sobre a superfície: tinta
    // escura no cinza, clara no AMOLED, clara no fundo roxo (como no print
    // das Configurações) e escura nos painéis claros do roxo. A seta magenta
    // do print de Peças daria 1,9:1 sobre o roxo (abaixo de 3:1). Trocar a
    // cor da seta é só nesta função.
    function setaNo(sup) { return tintaNo(sup) }

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
    property FontLoader _titulo500: FontLoader { source: raizDoTema.caminho + "CormorantGaramondAjustada-Medium.ttf" }
    property FontLoader _titulo600: FontLoader { source: raizDoTema.caminho + "CormorantGaramondAjustada-SemiBold.ttf" }
    property FontLoader _titulo700: FontLoader { source: raizDoTema.caminho + "CormorantGaramondAjustada-Bold.ttf" }
    property FontLoader _titulo500i: FontLoader { source: raizDoTema.caminho + "CormorantGaramondAjustada-MediumItalic.ttf" }
    property FontLoader _titulo600i: FontLoader { source: raizDoTema.caminho + "CormorantGaramondAjustada-SemiBoldItalic.ttf" }
    property FontLoader _sans400: FontLoader { source: raizDoTema.caminho + "source-sans-3-latin-400-normal.ttf" }
    property FontLoader _sans400i: FontLoader { source: raizDoTema.caminho + "source-sans-3-latin-400-italic.ttf" }
    property FontLoader _sans600: FontLoader { source: raizDoTema.caminho + "source-sans-3-latin-600-normal.ttf" }
    property FontLoader _sans700: FontLoader { source: raizDoTema.caminho + "source-sans-3-latin-700-normal.ttf" }
    property FontLoader _serif400: FontLoader { source: raizDoTema.caminho + "source-serif-4-latin-400-normal.ttf" }
    property FontLoader _serif400i: FontLoader { source: raizDoTema.caminho + "source-serif-4-latin-400-italic.ttf" }
    property FontLoader _serif600: FontLoader { source: raizDoTema.caminho + "source-serif-4-latin-600-normal.ttf" }
    property FontLoader _serif600i: FontLoader { source: raizDoTema.caminho + "source-serif-4-latin-600-italic.ttf" }

    // Títulos (Cormorant Garamond Ajustada): use com font.weight 500, 600 ou 700.
    readonly property string titulo: _titulo600.name

    // Texto (Source Sans 3): use com font.weight (Normal, DemiBold, Bold).
    readonly property string texto: _sans400.name

    // Texto com serifa (Source Serif 4), para descrições e notas.
    readonly property string serifa: _serif400.name
}
