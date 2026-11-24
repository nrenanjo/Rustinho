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
    // T094 (informativo da 2ª conferência): o próprio computador retirado também volta, com uma pasta de dados nova.
    assert!(t.contains("ou com este mesmo"), "a pergunta fala só de outro computador");
}

/// DP-45 (0.9.19-dev.2): o instalador Windows libera exclusivamente as portas da troca
/// de dados (47810/TCP e 47811/UDP) e remove as regras na instalação prévia e na desinstalação.
#[test]
fn dp45_firewall_do_instalador_libera_apenas_portas_da_troca() {
    let caminho = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("instalador").join("rustinho.iss");
    let iss = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
    let regras_add: Vec<&str> = iss
        .lines()
        .filter(|l| l.contains("advfirewall firewall add rule") && !l.trim_start().starts_with(';'))
        .collect();
    assert_eq!(
        regras_add.len(),
        2,
        "esperadas exatamente 2 regras de firewall (TCP 47810 e UDP 47811): {regras_add:#?}"
    );
    assert!(
        regras_add.iter().any(|l| l.contains("protocol=TCP")
            && l.contains("localport=47810")
            && l.contains("profile=private,domain")),
        "falta regra restrita TCP 47810: {regras_add:#?}"
    );
    assert!(
        regras_add.iter().any(|l| l.contains("protocol=UDP")
            && l.contains("localport=47811")
            && l.contains("profile=private,domain")),
        "falta regra restrita UDP 47811: {regras_add:#?}"
    );
    let deletes = iss
        .lines()
        .filter(|l| {
            l.contains("advfirewall firewall delete rule name=\"\"Rustinho\"\"") && !l.trim_start().starts_with(';')
        })
        .count();
    assert!(deletes >= 2, "esperada remoção da regra antes de recriar e na desinstalação (achadas: {deletes})");
}
