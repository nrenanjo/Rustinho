pragma Singleton
import QtQuick
import rustinho

// Fala com o núcleo em Rust. Resposta: {ok, dados} ou {ok: false, erro, campo}.
QtObject {
    function chamar(comando, dados) {
        const texto = Sistema.chamar(comando, JSON.stringify(dados === undefined ? {} : dados))
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
        return r
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
            return Sistema.arquivoLocal(String(endereco))
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
