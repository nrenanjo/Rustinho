pragma Singleton
import QtQuick
import rustinho

// Fala com o núcleo em Rust. Resposta: {ok, dados} ou {ok: false, erro, campo}.
QtObject {
    function chamar(comando, dados) {
        const texto = Sistema.chamar(comando, JSON.stringify(dados === undefined ? {} : dados))
        try {
            return JSON.parse(texto)
        } catch (e) {
            return { ok: false, erro: "Resposta inválida do sistema.", campo: null }
        }
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
