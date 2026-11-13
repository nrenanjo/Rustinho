pragma Singleton
import QtQuick

// Cores, fontes e medidas do Rustinho, tiradas do livro do Baralho Cigano.
QtObject {
    // O id não pode ser "tema": no QML o id vence a propriedade "tema" (0.9.18).
    id: raizDoTema

    // ---- Tema (0.9.18): sol = claro, lua = escuro (AMOLED), planeta =
    // Planeta Roxo. Os três na mesma identidade (ordem visual da 0.9.18, §10):
    // o claro com painéis claros e detalhes roxos e magenta; o escuro com
    // fundo profundo, grafite e destaques magenta. Os nomes guardados no banco
    // continuam "cinza" e "amoled" (a escolha de cada aparelho não se perde).
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
    // No escuro: fundo quase preto (bom para AMOLED), faixas em grafite e
    // letras claras, sempre com muito contraste para ler sem esforço.
    // No roxo: fundo roxo com letras claras; painéis, campos e faixas claras
    // com letras escuras (as cores abaixo são as dos painéis; o texto direto
    // no fundo roxo usa as funções "...No(superfície)", mais abaixo).
    // No claro: fundo lilás bem claro, painéis e campos claros, letras escuras.
    // Contraste conferido (texto 4,5:1; bordas 3:1) em
    // Desenvolvimento_0.9.18/estabilizacao_0.9.18/paletas/conferir_contraste.py.
    readonly property color fundo: roxo ? "#770273" : escuro ? "#0A070C" : "#E9E3ED"
    readonly property color pergaminho: roxo ? "#DDDCDF" : escuro ? "#1C1820" : "#F8F5FA"   // faixa ímpar e painéis
    readonly property color azul: roxo ? "#D6D0DC" : escuro ? "#28232B" : "#ECE5F0"         // faixa par
    readonly property color verde: roxo ? "#D9DED8" : escuro ? "#1C2619" : "#E3EBE1"        // faixa das palavras positivas
    readonly property color rosa: roxo ? "#EDD5E3" : escuro ? "#33212C" : "#F5E4EE"         // faixa das palavras negativas
    readonly property color amarelo: roxo ? "#F1E2BE" : escuro ? "#332B14" : "#F6EBCB"      // "vence hoje"
    readonly property color papel: roxo ? "#F5F5F5" : escuro ? "#141017" : "#FFFFFF"        // dentro dos campos
    readonly property color tinta: roxo ? "#231A26" : escuro ? "#F4EEF6" : "#231A26"        // texto principal
    readonly property color tintaSuave: roxo ? "#4A3F4E" : escuro ? "#DCD2E0" : "#4A3F4E"   // texto secundário
    readonly property color apagado: roxo ? "#5A2D63" : escuro ? "#C9AECF" : "#674A6D"      // rótulos
    readonly property color dourado: roxo ? "#744679" : escuro ? "#D9BBDE" : "#744679"      // capitulares e detalhes
    readonly property color verdeEscuro: roxo ? "#3E5A39" : escuro ? "#A6CC9B" : "#3E5A39"  // deu certo
    readonly property color vermelho: roxo ? "#8C3530" : escuro ? "#FFA7B9" : "#8C3530"     // erro
    readonly property color azulEscuro: roxo ? "#8A1679" : escuro ? "#F38BDA" : "#8A1679"   // foco e destaques
    readonly property color claro: roxo ? "#FFFFFF" : escuro ? "#141017" : "#FFFFFF"        // texto sobre fundo "tinta"
    // Vermelho sobre a linha escolhida (magenta nos três temas: branco).
    readonly property color vermelhoSobre: "#FFFFFF"
    readonly property color rosaSobre: roxo ? "#E5C6D8" : escuro ? "#4A2D3F" : "#F0DCE8"
    readonly property color dica: roxo ? "#6E6472" : escuro ? "#A396A8" : "#6E6472"          // texto de exemplo nos campos
    // Pílulas da tela inicial e notificações (0.9.15).
    readonly property color cartao: roxo ? "#DDDBDE" : escuro ? "#1C1820" : "#FFFFFF"       // fundo da pílula
    readonly property color bordaCartao: roxo ? "#5A0257" : escuro ? "#3B2F40" : "#D3C4DA"  // contorno fino
    readonly property color atencao: escuro ? "#2E2714" : "#FCF1DA"      // notificação de atenção
    readonly property color bordaAtencao: escuro ? "#5A4A22" : "#EAD3A2"
    readonly property color critico: escuro ? "#3A1F1B" : "#F6DDD7"      // notificação crítica
    readonly property color bordaCritico: escuro ? "#6B3A33" : "#E2B3A9"
    readonly property color informativo: escuro ? "#1C2629" : "#E6ECEC"  // notificação informativa
    readonly property color bordaInformativo: escuro ? "#34474C" : "#C9D6D8"

    // Botão secundário (normal, mouse em cima, apertado) e o texto dele.
    readonly property color botao: roxo ? "#FACBE3" : escuro ? "#3A2A3E" : "#F3DCEB"
    readonly property color botaoSobre: roxo ? "#F3BCD9" : escuro ? "#47344C" : "#EDCDE2"
    readonly property color botaoBaixo: roxo ? "#EBADCF" : escuro ? "#553F5A" : "#E4BDD6"
    readonly property color tintaBotao: roxo ? "#3A0B45" : escuro ? tinta : "#3A0B45"
    // Botão principal (magenta, a cor da identidade, nos três temas) com
    // mouse em cima e apertado, e o texto dele.
    readonly property color principal: roxo && roxoFiel ? "#FB2ACD" : "#D123AB"
    readonly property color sobrePrincipal: "#FFFFFF"
    readonly property color principalSobre: roxo && roxoFiel ? "#F04BCF" : "#C21E9E"
    readonly property color principalBaixo: "#A8198A"
    readonly property color perigoSobre: escuro ? "#FFC1CE" : "#A34D42"
    readonly property color perigoBaixo: escuro ? "#F58FA6" : "#7E3930"

    // Linha escolhida numa lista (magenta nos três temas) e o texto.
    readonly property color atual: roxo && roxoFiel ? "#FA26C9" : "#D123AB"
    readonly property color tintaAtual: "#FFFFFF"
    // Selo (chip) dentro da linha escolhida: um véu escuro deixa o texto
    // branco legível sobre o magenta.
    readonly property color chipAtual: Qt.rgba(0.2, 0, 0.2, 0.35)
    // Pílulas: a escolhida no fundo da tela e a escolhida dentro de uma faixa
    // (subpílulas, pílulas num painel), com o texto de cada uma.
    readonly property color pilulaAtiva: roxo ? "#F94BBF" : "#D123AB"
    readonly property color sobrePilulaAtiva: roxo ? "#1E0B1D" : "#FFFFFF"
    readonly property color subpilulaAtiva: roxo ? "#610161" : escuro ? "#F38BDA" : "#610161"
    readonly property color sobreSubpilulaAtiva: roxo ? "#FFFFFF" : escuro ? "#1E0B1D" : "#FFFFFF"
    // A faixa das subpílulas (Configurações › Simultaneidade).
    readonly property color faixaSubpilulas: roxo ? "#F22EB6" : escuro ? "#2C1F31" : "#F1DDEC"
    // Busca das telas (campo com a lupa): borda magenta nos três temas; no
    // roxo, o campo escuro.
    readonly property color buscaFundo: roxo ? "#4E114B" : papel
    readonly property color buscaBorda: escuro || roxo ? "#F038D0" : "#D123AB"
    // Pílula do olho ("Mostrar valores") e do tema (sol, lua, planeta).
    readonly property color pilulaClara: roxo ? "#E4D1E5" : escuro ? "#28232B" : "#F1E6F2"
    readonly property color pilulaClaraSobre: roxo ? "#ECDDED" : escuro ? "#352E39" : "#F7EFF8"
    readonly property color seletorFundo: roxo ? "#3C223B" : escuro ? "#28232B" : "#E4DCE8"
    readonly property color seletorAtivo: roxo ? "#FD2BD1" : escuro ? "#F038D0" : "#D123AB"
    // Campo só de leitura.
    readonly property color campoFixo: roxo ? "#E6E1E8" : escuro ? "#19151C" : "#EFEAF2"

    // ---- Estados dos campos (0.9.18, ordem visual §6) ----
    // Normal (contorno fino), focado (contorno magenta com brilho), preenchido
    // (contorno um pouco mais forte), inválido (vermelho com brilho),
    // desabilitado (apagado) e validado (verde). Contraste das bordas: 3:1.
    readonly property color foco: escuro || roxo && roxoFiel ? "#F038D0" : "#D123AB"
    readonly property color focoBrilho: Qt.rgba(foco.r, foco.g, foco.b, 0.28)
    function erroBrilhoNo(sup) { const c = vermelhoNo(sup); return Qt.rgba(c.r, c.g, c.b, 0.24) }
    readonly property color bordaCampo: sombra(0.16)
    readonly property color bordaPreenchido: sombra(0.34)
    readonly property real opacidadeDesabilitado: 0.5

    // ---- Vidro roxo (0.9.18, ordem visual §9: a seta de retorno) ----
    // Um roxo translúcido, com contorno fino claro e um brilho em cima: sobre o
    // fundo roxo ou escuro, um vidro claro; sobre um painel claro, um vidro
    // roxo. A cor da seta continua a de maior contraste (setaNo).
    function _vidroEscuro(sup) { return luminancia(sup) < 0.18 }
    function vidroNo(sup) { return _vidroEscuro(sup) ? Qt.rgba(0.98, 0.78, 0.96, 0.16) : Qt.rgba(0.47, 0.01, 0.45, 0.10) }
    function vidroSobreNo(sup) { return _vidroEscuro(sup) ? Qt.rgba(0.98, 0.78, 0.96, 0.26) : Qt.rgba(0.47, 0.01, 0.45, 0.17) }
    function vidroBaixoNo(sup) { return _vidroEscuro(sup) ? Qt.rgba(0.98, 0.78, 0.96, 0.34) : Qt.rgba(0.47, 0.01, 0.45, 0.24) }
    function vidroBordaNo(sup) { return _vidroEscuro(sup) ? Qt.rgba(1, 0.86, 0.98, 0.42) : Qt.rgba(0.47, 0.01, 0.45, 0.32) }
    function vidroBrilhoNo(sup) { return _vidroEscuro(sup) ? Qt.rgba(1, 1, 1, 0.20) : Qt.rgba(1, 1, 1, 0.55) }

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
    // Texto secundário na linha escolhida (o magenta, nos três temas): o
    // texto (a ≥ 0,5) fica inteiro, porque sobre o magenta qualquer
    // transparência fica abaixo de 4,5:1; o fundo leve (a < 0,5) continua
    // transparente.
    function claroSuave(a) { return a >= 0.5 ? tintaAtual : Qt.rgba(tintaAtual.r, tintaAtual.g, tintaAtual.b, a) }
    // Véu atrás das janelas que abrem por cima.
    readonly property color veu: roxo ? Qt.rgba(0.12, 0, 0.12, 0.55) : escuro ? Qt.rgba(0, 0, 0, 0.7) : Qt.rgba(0.16, 0.04, 0.18, 0.35)

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
    // D-V1, decidida pelo dono na correção definitiva do botão de retorno
    // (07/11/2026): só a seta, como em comoficará.png, sem texto, legenda ou
    // nome de destino ao lado (Seta.qml).
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

    // H10 (7.2): o dado que entra num texto formatado (Text.StyledText) vai
    // escapado: "<", ">" e "&" aparecem como foram digitados, sem virar
    // marcação (uma variação "<i>P</i>" não deixa o resto em itálico).
    function escapar(t) {
        return String(t === undefined || t === null ? "" : t)
            .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    }

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
    readonly property bool celular: Qt.platform.os === "android" || Qt.platform.os === "ios" || simularCelular
    // Só para os testes das telas (A15, 0.9.18): o desenho do celular no
    // computador, numa tela virtual. A janela principal liga só com as opções
    // dos testes (--textos-da-tela --celular); no uso normal é sempre false.
    property bool simularCelular: false

    // ---- Medidas ----
    readonly property real raio: 11                 // cantos das faixas (3 mm no livro)
    readonly property real raioCampo: 7
    readonly property int margem: estreito ? 14 : 32
    readonly property int espaco: 16
    // Altura mínima de um botão ou campo para o dedo.
    readonly property int toque: celular ? 44 : 40
    // A15 (0.9.18): área de toque mínima de tudo o que se toca no celular,
    // 48 × 48 dp. Os componentes (Botao, Campo, Selecao, Marcar, Pilulas,
    // Olho, Aparencia) crescem a área de toque até aqui e deixam o desenho do
    // tamanho de sempre, no meio. No computador é 0: nada muda.
    readonly property int toqueMinimo: celular ? 48 : 0
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

    // Nome do registro nas linhas das listas (0.9.18, ordem visual §3: texto
    // funcional sem serifa, como na imagem aprovada de Peças): Source Sans 3
    // em negrito, nos três temas. Os números de ordem (as capitulares
    // douradas: nº da venda, posição no carrinho e no ranking) continuam com
    // a fonte dos títulos.
    readonly property string fonteLinha: texto
}
