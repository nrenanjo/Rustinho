pragma Singleton
import QtQuick
import rustinho

// Fala com o núcleo em Rust. Resposta: {ok, dados} ou {ok: false, erro, campo}.
QtObject {
    // D-07: a sessão acabou (usuário desativado, ou ninguém conectado): a
    // janela volta para a tela de entrada.
    signal sessaoEncerrada(string motivo)

    // Uma função demorada começou ou terminou (a janela mostra a espera e
    // não deixa clicar em nada enquanto isso: também evita o clique repetido).
    signal esperaComecou(string texto)
    signal esperaTerminou()
    // Decisão 2c do proprietário (18/11/2026, bug YVZ4): uma operação gravou
    // (a lista da tela pode mudar no lugar). O Main descarta o clique nas
    // telas por 0,4 s.
    signal gravou()

    // Pedidos demorados em andamento: número → { comando, depois }.
    property var _pendentes: ({})
    function temOperacaoEmCurso() {
        return Object.keys(_pendentes).length > 0
    }
    function comandoAtivo() {
        const chaves = Object.keys(_pendentes)
        if (chaves.length === 0)
            return ""
        const u = _pendentes[chaves[chaves.length - 1]]
        return u && u.comando ? String(u.comando) : ""
    }
    // Consultas fora da linha da janela (7.6): o último pedido de cada canal
    // → { pedido, comando, depois }.
    property var _consultas: ({})
    property Connections _respostas: Connections {
        target: Sistema
        function onTerminou(pedido, resposta) {
            const p = _pendentes[pedido]
            delete _pendentes[pedido]
            esperaTerminou()
            // Como antes: só interpreta quando há quem use a resposta (o gravarDemorado sempre usava).
            if (!p || (!p.depois && !p.comChave))
                return
            const r = interpretar(p.comando, resposta)
            // T-01/T-05 da 2ª re-revisão: a gravação com a chave da operação, venha de onde vier.
            if (p.comChave)
                _depoisDeGravar(r)
            if (p.depois)
                p.depois(r)
        }
        function onConsultado(canal, pedido, resposta) {
            const p = _consultas[canal]
            // A última vence: a resposta de um pedido que já não é o último
            // do canal (ou de um canal esquecido) não chega à tela.
            if (!p || p.pedido !== pedido)
                return
            delete _consultas[canal]
            if (p.depois)
                p.depois(interpretar(p.comando, resposta))
        }
    }

    // H09 (7.6): uma leitura fora da linha da janela. Devolve na hora; a
    // resposta chega depois em depois(r), sem a espera com a barra e sem
    // parar a janela. Um canal por tela: no mesmo canal, a última vence (a
    // resposta de um pedido mais velho nunca chega ao depois). Só as leituras
    // da lista do núcleo (CONSULTAS_FORA_DA_JANELA); o resto é recusado. Gravar
    // continua em chamar e gravar.
    function consultar(canal, comando, dados, depois) {
        const n = Sistema.consultar(canal, comando, JSON.stringify(dados === undefined ? {} : dados))
        _consultas[canal] = { pedido: n, comando: comando, depois: depois }
    }

    // A tela não quer mais a resposta do canal (fechou, ou leu de novo pelo
    // caminho de sempre).
    function esquecer(canal) {
        delete _consultas[canal]
    }

    // Roda um comando demorado (restaurar backup, importar planilhas e XML)
    // numa tarefa à parte; depois(r) recebe a resposta. Enquanto roda, a
    // janela mostra "texto" com a barra andando.
    function chamarDemorado(comando, dados, texto, depois) {
        const n = Sistema.chamar_demorado(comando, JSON.stringify(dados === undefined ? {} : dados))
        if (n === 0) {
            if (depois)
                depois({ ok: false, erro: "Aguarde: o Rustinho ainda está terminando a operação anterior.", campo: "ocupado" })
            return
        }
        _pendentes[n] = { comando: comando, depois: depois, comChave: !!(dados && dados.chaveOperacao) }
        esperaComecou(texto || "Espere um pouco…")
    }

    // Gravação com a chave da operação (FALHA-04). A chave identifica UMA
    // tentativa lógica: `chave` é a da operação que a tela está montando (a
    // mesma em cada nova tentativa dela, como o duplo clique ou o Enter
    // repetido; a tela troca por uma nova quando começa outra operação). Assim
    // a repetição não grava duas vezes, e duas operações iguais legítimas
    // (duas vendas com os mesmos itens) são gravadas as duas. Sem `chave`: uma
    // chave só para esta chamada (ações sobre um registro que já existe, como
    // estornar ou cancelar, que o núcleo não deixa fazer duas vezes).
    function gravar(comando, dados, chave) {
        return chamar(comando, comChave(dados, chave))
    }

    // O mesmo, para um comando demorado.
    function gravarDemorado(comando, dados, texto, depois, chave) {
        chamarDemorado(comando, comChave(dados, chave), texto, depois)
    }

    // T-01 e T-05 da 2ª re-revisão do QT-1 (16/11/2026): a confirmação de entrega (S1) e a marca da repetição (I6)
    // num lugar só, por onde passa toda chamada com a chave da operação: o gravar, o gravarDemorado, o
    // chamarGerente e a repetição com a senha do gerente (antes, só o gravar e o gravarDemorado; 27 comandos do
    // gerente ficavam sem confirmação e viravam "Gravada" no aviso depois de uma queda).
    function _depoisDeGravar(r) {
        if (r.ok)
            _entregaPendente = true
        _marcarRepetida(r)
        if (r.ok)
            gravou()
    }

    // I6 (decisão de 15/11/2026): a mesma operação repetida (duplo clique, Enter repetido) volta "ok" com a marca
    // `repetida`, sem gravar de novo. O aviso verde que a tela mostra em seguida, no mesmo tratamento, começa com
    // "Já estava feito" (Main.qml, avisar). A marca vale só até o fim desse tratamento, para não ir parar no aviso
    // de outra ação.
    property bool repetidaAgora: false
    function _marcarRepetida(r) {
        if (!r.ok || r.repetida !== true)
            return
        repetidaAgora = true
        Qt.callLater(() => { repetidaAgora = false })
    }


    // QT-1b, N-01 da re-revisão (parte 2): a gravação só conta como "a pessoa viu" depois que a janela desenhou um
    // quadro com o resultado. A janela (Main.qml) chama confirmarEntrega() a cada quadro desenhado; aqui, só quando
    // houve gravação. Se o núcleo estiver ocupado (comando demorado), tenta de novo no próximo quadro.
    property bool _entregaPendente: false
    function confirmarEntrega() {
        if (!_entregaPendente)
            return
        const r = interpretar("entrega_confirmar", Sistema.chamar("entrega_confirmar", "{}"))
        if (r.ok)
            _entregaPendente = false
    }

    // Os dados com a chave da operação (a que já vier neles fica).
    function comChave(dados, chave) {
        const d = Object.assign({}, dados === undefined ? {} : dados)
        if (!d.chaveOperacao)
            d.chaveOperacao = chave || novaChave()
        return d
    }

    function chamar(comando, dados) {
        const texto = Sistema.chamar(comando, JSON.stringify(dados === undefined ? {} : dados))
        const r = interpretar(comando, texto)
        if (dados && dados.chaveOperacao && comando !== "trabalho_guardar")
            _depoisDeGravar(r)
        return r
    }

    function interpretar(comando, texto) {
        let r
        try {
            r = JSON.parse(texto)
        } catch (e) {
            return { ok: false, erro: "Resposta inválida do sistema.", campo: null }
        }
        // Só "ok: true" de verdade conta como sucesso (C-14).
        if (r === null || typeof r !== "object" || typeof r.ok !== "boolean")
            return { ok: false, erro: "Resposta inválida do sistema.", campo: null }
        if (!r.ok && typeof r.erro !== "string")
            r.erro = "Erro desconhecido."
        if (r.ok && r.dados === undefined)
            r.dados = null
        if (!r.ok && r.campo === "sessao" && comando !== "entrar")
            sessaoEncerrada(r.erro)
        return r
    }

    // Chave de uma operação (venda, recebimento, OS...): vai junto da
    // gravação; o núcleo recusa a mesma chave chegando de novo (duplo clique).
    function novaChave() {
        return Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 12) + "-" + Math.random().toString(36).slice(2, 12)
    }

    // Caminho do computador → endereço "file://" (também "C:\\..." do Windows).
    function url(caminho) {
        const c = String(caminho || "").replace(/\\/g, "/")
        return /^[A-Za-z]:/.test(c) ? "file:///" + c : "file://" + c
    }

    // Endereço "file://" → caminho do computador.
    function caminho(endereco) {
        // Android: o arquivo vem como "content://..."; vira uma cópia local.
        if (String(endereco).startsWith("content:"))
            return Sistema.arquivo_local(String(endereco))
        let c = decodeURIComponent(String(endereco || "").replace(/^file:\/\//, ""))
        if (/^\/[A-Za-z]:/.test(c))
            c = c.substring(1)
        return c
    }

    // Pasta onde fica um arquivo ("/a/b/c.db" → "/a/b").
    function pastaDe(caminho) {
        const c = String(caminho || "")
        return c.substring(0, Math.max(c.lastIndexOf("/"), c.lastIndexOf("\\")))
    }
}
