//! Testes de estabilidade (antes da versão estável): o que a loja precisa
//! poder confiar — backup, estoque, crediário, DRE, permissões, recuperação,
//! importação e troca de dados. O plano e o que cada teste prova estão em
//! `docs/TESTES-estabilidade.md`; os defeitos achados, em `docs/DEFEITOS.md`.

mod a1_auditoria;
#[path = "../apoio/chave.rs"]
mod apoio_chave;
mod comum;
mod d09_custos;
mod t1_backup;
mod t2_estoque;
mod t3_crediario;
mod t4_dre;
mod t5_permissoes;
mod t6_recuperacao;
mod t7_importacao;
mod t8_sincronizacao;

/// Só à mão: monta a loja de teste (a mesma de `povoar`) numa pasta, para
/// abrir o Rustinho nela e conferir as telas.
/// `RUSTINHO_LOJA_TESTE=/pasta cargo test -p rustinho-nucleo --test estavel -- --ignored montar_loja_de_teste`
#[test]
#[ignore]
fn montar_loja_de_teste() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    let mut l = comum::Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        serde_json::json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                            "senha": comum::SENHA_ANA, "senha2": comum::SENHA_ANA }),
    );
    let povo = comum::povoar(&mut l);
    // RVI-02 (app/testes/telas_rvi02.py): a mesma loja com a troca de dados ligada.
    if std::env::var_os("RUSTINHO_LOJA_TROCA").is_some() {
        l.ok("sync_salvar", serde_json::json!({ "ligado": true, "senha": "rede-da-loja-teste", "enderecos": "" }));
    }
    // Notificações (app/testes/telas_notificacoes.py) no Início: informativas
    // (parcela vencida, conta a pagar vencida, encomenda em aberto), atenção
    // (diferença entre caixas) e duas críticas (computador de outra loja
    // recusado; banco que veio de outro computador — o Rustinho confere ao
    // abrir e bloqueia os códigos).
    if std::env::var_os("RUSTINHO_LOJA_AVISOS").is_some() {
        l.ok(
            "receber_lancar",
            serde_json::json!({ "clienteId": povo.ze, "descricao": "Caderno", "valor": "50", "parcelas": 1,
                                "primeiroVencimento": "" }),
        );
        l.ok(
            "pagar_lancar",
            serde_json::json!({ "categoriaId": "cat-aluguel", "descricao": "Aluguel de novembro", "valor": "300",
                                "parcelas": 1, "competencia": "", "fornecedorId": povo.fornecedor }),
        );
        l.ok(
            "encomenda_criar",
            serde_json::json!({ "clienteNome": "Maria Souza", "telefone": "(15) 99808-0000",
                                "descricao": "Pastilha dianteira", "quantidade": "1" }),
        );
        let recusa = serde_json::json!({ "nome": "Caixa da outra loja", "endereco": "192.168.0.9",
                                         "quando": rustinho_nucleo::tempo::agora() });
        l.executar(&format!(
            "UPDATE conta_receber SET vencimento = date('now', '-3 days');
             UPDATE conta_pagar SET vencimento = date('now', '-2 days');
             UPDATE config SET valor = 'outro-computador' WHERE chave = 'computador_id';
             INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando)
             VALUES ('produto', '{}', 'v1', 'v2', 'caixa-2', datetime('now'));
             INSERT OR REPLACE INTO config (chave, valor) VALUES ('{}', '{}');",
            povo.filtro,
            rustinho_nucleo::loja::OUTRA_LOJA,
            recusa
        ));
    }
}

/// Consultas da validação da busca na tela (app/testes/validacao_visual_0918.py)
/// e as peças que cada uma tem de achar, nesta ordem de exibição, na loja de
/// `montar_loja_de_validacao`. O roteiro lê o mesmo arquivo gerado
/// (`busca_esperada.json`) para conferir a tela.
const CONSULTAS_DA_VALIDACAO: &[(&str, &str, &[&str])] = &[
    ("código", "PF-100", &["Pastilha de freio dianteira"]),
    ("código original", "GDB1330", &["Pastilha de freio dianteira"]),
    ("nome", "correia dentada", &["Correia dentada"]),
    ("nome (palavras fora de ordem)", "traseiro amortecedor", &["Amortecedor traseiro"]),
    ("marca", "gates", &["Correia dentada"]),
    ("modelo do carro", "onix", &["Correia dentada"]),
    ("carro e ano", "amortecedor celta 2010", &["Amortecedor traseiro"]),
    ("carro, motor e ano", "pastilha gol 1.6 2015", &["Pastilha de freio dianteira"]),
    ("equivalente", "N-560", &["Pastilha de freio dianteira"]),
    ("equivalente (outra marca)", "PSL55", &["Filtro de óleo do motor"]),
    // Os dois rolamentos FAG: o do catálogo pela marca e o da nota de teste
    // pelo código do fornecedor (FAG-32206).
    ("marca e nome", "fag rolamento", &["Rolamento de roda dianteiro", "ROLAMENTO RODA TRASEIRA 32206"]),
    ("código do fornecedor", "FAG-32206", &["ROLAMENTO RODA TRASEIRA 32206"]),
    ("nada", "xyzzy", &[]),
];

/// Só à mão: a loja de teste (a de `montar_loja_de_teste`) com um catálogo
/// para a validação visual da 0.9.18: peças com código, código original,
/// marca, equivalentes de outras marcas e carros (montadora, modelo, motor e
/// anos). Confere no núcleo que cada consulta acha exatamente o esperado e
/// grava as consultas em `busca_esperada.json`, na pasta da loja.
/// `RUSTINHO_LOJA_TESTE=/pasta cargo test -p rustinho-nucleo --test estavel -- --ignored --exact montar_loja_de_validacao`
#[test]
#[ignore]
fn montar_loja_de_validacao() {
    use serde_json::json;
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    let mut l = comum::Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                "senha": comum::SENHA_ANA, "senha2": comum::SENHA_ANA }),
    );
    comum::povoar(&mut l);
    // (código, nome, marca, código original, equivalentes, carros)
    type Carro = (&'static str, &'static str, &'static str, &'static str, &'static str);
    let catalogo: &[(&str, &str, &str, &str, &[(&str, &str)], &[Carro])] = &[
        ("PF-100", "Pastilha de freio dianteira", "Fras-le", "GDB1330", &[("Cobreq", "N-560"), ("Bosch", "BB1330")],
         &[("Volkswagen", "Gol", "1.6", "2010", "2016"), ("Volkswagen", "Voyage", "1.6", "2012", "2016")]),
        ("FO-200", "Filtro de óleo do motor", "Mahle", "OC-263", &[("Tecfil", "PSL55"), ("Fram", "PH5548")],
         &[("Fiat", "Palio", "1.0", "2005", "2014"), ("Fiat", "Uno", "1.0", "2011", "2018")]),
        ("CD-400", "Correia dentada", "Gates", "5PK1234", &[("Dayco", "102RSF"), ("Contitech", "CT1028")],
         &[("Chevrolet", "Onix", "1.0", "2013", "2019")]),
        ("AM-500", "Amortecedor traseiro", "Cofap", "", &[],
         &[("Volkswagen", "Gol", "1.0", "2008", "2014"), ("Chevrolet", "Celta", "1.0", "2006", "2015")]),
        ("RD-600", "Rolamento de roda dianteiro", "FAG", "32206", &[("SKF", "VKBA3525")],
         &[("Renault", "Sandero", "1.6", "2012", "2020")]),
    ];
    for (codigo, nome, marca, original, equivalentes, carros) in catalogo {
        let mut f = l.ok("produto_novo", json!({}));
        f["codigo"] = json!(codigo);
        f["nome"] = json!(nome);
        f["marca"] = json!(marca);
        f["codigoOriginal"] = json!(original);
        f["custo"] = json!("20");
        f["estoqueInicial"] = json!("5");
        f["precos"][0]["preco"] = json!("50");
        f["precos"][0]["lucro"] = json!("");
        f["equivalentes"] = json!(equivalentes
            .iter()
            .map(|(m, c)| json!({ "marca": m, "codigo": c }))
            .collect::<Vec<_>>());
        f["aplicacoes"] = json!(carros
            .iter()
            .map(|(montadora, modelo, motor, inicio, fim)| json!({ "montadora": montadora, "modelo": modelo,
                "motor": motor, "anoInicio": inicio, "anoFim": fim, "observacao": "" }))
            .collect::<Vec<_>>());
        l.ok("produto_salvar", f);
    }
    let mut gravar = Vec::new();
    for (tipo, consulta, esperado) in CONSULTAS_DA_VALIDACAO {
        let r = l.ok("produtos_buscar", json!({ "texto": consulta, "inativas": false, "limite": 200 }));
        let achou: Vec<String> =
            r["itens"].as_array().unwrap().iter().map(|p| p["nome"].as_str().unwrap().to_string()).collect();
        assert_eq!(achou, *esperado, "consulta {tipo} {consulta:?}");
        gravar.push(json!({ "tipo": tipo, "consulta": consulta, "esperado": esperado }));
    }
    std::fs::write(pasta.join("busca_esperada.json"), serde_json::to_string_pretty(&gravar).unwrap()).unwrap();
}

/// Só à mão: muda a loja de teste por fora, como num banco antigo ou
/// restaurado (para as telas da navegação guiada, 0.9.17). Pelo núcleo, para
/// os gatilhos da troca de dados terem as funções do Rustinho.
/// `RUSTINHO_LOJA_TESTE=/pasta RUSTINHO_SQL="UPDATE config ..." cargo test ... -- --ignored executar_por_fora`
#[test]
#[ignore]
fn executar_por_fora() {
    let (Some(pasta), Ok(sql)) = (std::env::var_os("RUSTINHO_LOJA_TESTE"), std::env::var("RUSTINHO_SQL")) else {
        return;
    };
    let b = rustinho_nucleo::banco::Banco::abrir(&std::path::Path::new(&pasta).join("rustinho.db")).unwrap();
    b.conn.execute_batch(&sql).unwrap();
}

/// Só à mão: desativa um usuário da loja de teste por fora, como se a
/// mudança chegasse de outro caixa (para ver a tela voltar à entrada, D-07).
/// `RUSTINHO_LOJA_TESTE=/pasta RUSTINHO_DESATIVAR=Bia cargo test ... -- --ignored desativar_por_fora`
#[test]
#[ignore]
fn desativar_por_fora() {
    let (Some(pasta), Ok(nome)) = (std::env::var_os("RUSTINHO_LOJA_TESTE"), std::env::var("RUSTINHO_DESATIVAR")) else {
        return;
    };
    let b = rustinho_nucleo::banco::Banco::abrir(&std::path::Path::new(&pasta).join("rustinho.db")).unwrap();
    b.conn.execute("UPDATE usuario SET ativo = 0 WHERE nome = ?1", [&nome]).unwrap();
}
