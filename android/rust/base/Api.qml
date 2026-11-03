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

    // Pedidos demorados em andamento: número → { comando, depois }.
    property var _pendentes: ({})
    property Connections _respostas: Connections {
        target: Sistema
        function onTerminou(pedido, resposta) {
            const p = _pendentes[pedido]
            delete _pendentes[pedido]
            esperaTerminou()
            if (p && p.depois)
                p.depois(interpretar(p.comando, resposta))
        }
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
        _pendentes[n] = { comando: comando, depois: depois }
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

    // Os dados com a chave da operação (a que já vier neles fica).
    function comChave(dados, chave) {
        const d = Object.assign({}, dados === undefined ? {} : dados)
        if (!d.chaveOperacao)
            d.chaveOperacao = chave || novaChave()
        return d
    }

    function chamar(comando, dados) {
        const texto = Sistema.chamar(comando, JSON.stringify(dados === undefined ? {} : dados))
        return interpretar(comando, texto)
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
