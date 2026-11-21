//! T076 (OPUS-D-3 = AUD-D-02 da auditoria da 0.9.18-dev.3.3.3): o texto das telas não promete uma proteção que
//! a troca de dados não dá. A retirada de um caixa só marca o identificador que ele declara: quem souber a senha
//! da rede entra de novo com outro computador (ou com uma pasta de dados vazia). Até o protocolo 4, a tela tem de
//! dizer isso e mandar trocar a senha da rede.

fn tela(nome: &str) -> String {
    let caminho = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("app").join("qml").join(nome);
    std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()))
}

#[test]
fn retirar_um_caixa_nao_promete_que_ele_nao_volta() {
    let t = tela("TelaConfiguracoes.qml");
    assert!(
        !t.contains("não entra sozinho"),
        "a pergunta de Retirar da loja promete que o caixa não volta sozinho, e a senha da rede deixa voltar"
    );
    assert!(t.contains("troque a senha da rede"), "a retirada não orienta a trocar a senha da rede");
}
