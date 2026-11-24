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
/// de dados (47810/TCP e 47811/UDP), preserva o BOM UTF-8 do `.iss` e remove as regras
/// antes de recriar (`[Run]`) e na desinstalação (`[UninstallRun]`).
#[test]
fn dp45_firewall_do_instalador_libera_apenas_portas_da_troca() {
    let caminho = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("instalador").join("rustinho.iss");
    let bytes = std::fs::read(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
    assert!(
        bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
        "instalador/rustinho.iss deve preservar BOM UTF-8 para acentos no Inno Setup"
    );
    let iss = String::from_utf8(bytes[3..].to_vec()).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));

    let mut secao = String::new();
    let mut run_linhas = Vec::new();
    let mut uninstall_linhas = Vec::new();
    for linha in iss.lines() {
        let limpa = linha.trim();
        if limpa.is_empty() || limpa.starts_with(';') {
            continue;
        }
        if limpa.starts_with('[') && limpa.ends_with(']') {
            secao = limpa.to_ascii_lowercase();
            continue;
        }
        match secao.as_str() {
            "[run]" => run_linhas.push(limpa),
            "[uninstallrun]" => uninstall_linhas.push(limpa),
            _ => {}
        }
    }

    // N-03 (0.9.19-dev.2): "apagar os dados" na desinstalação apaga também a pasta da
    // identidade do caixa (protocolo 4), que fica fora da pasta do banco.
    for pasta in [r"{localappdata}\Rustinho'", r"{localappdata}\Rustinho-identidade'"] {
        assert!(
            iss.lines().any(|l| l.trim_start().starts_with("DelTree(") && l.contains(pasta)),
            "a desinstalação com \"apagar os dados\" tem de apagar {pasta}"
        );
    }

    let regras_add: Vec<&str> =
        run_linhas.iter().copied().filter(|l| l.contains("advfirewall firewall add rule")).collect();
    assert_eq!(
        regras_add.len(),
        2,
        "esperadas exatamente 2 regras de firewall em [Run] (TCP 47810 e UDP 47811): {regras_add:#?}"
    );
    assert!(
        regras_add.iter().any(|l| l.contains("protocol=TCP")
            && l.contains("localport=47810")
            && l.contains("profile=private,domain")),
        "falta regra restrita TCP 47810 em [Run]: {regras_add:#?}"
    );
    assert!(
        regras_add.iter().any(|l| l.contains("protocol=UDP")
            && l.contains("localport=47811")
            && l.contains("profile=private,domain")),
        "falta regra restrita UDP 47811 em [Run]: {regras_add:#?}"
    );
    assert!(
        run_linhas.iter().any(|l| l.contains("advfirewall firewall delete rule name=\"\"Rustinho\"\"")),
        "falta remoção prévia da regra em [Run]: {run_linhas:#?}"
    );
    assert!(
        uninstall_linhas.iter().any(|l| l.contains("advfirewall firewall delete rule name=\"\"Rustinho\"\"")),
        "falta remoção da regra em [UninstallRun]: {uninstall_linhas:#?}"
    );
}
