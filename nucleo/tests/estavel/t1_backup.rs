//! Prioridade 1 — Backup e restauração completa em outra pasta (como num
//! computador novo), conferindo todos os dados comerciais e financeiros, sem
//! mexer na origem.

use serde_json::{json, Value};

use crate::comum::*;

/// O que as telas mostram de dinheiro e estoque (para comparar antes e depois).
fn telas(l: &mut Loja, p: &Povo) -> Vec<(String, Value)> {
    let mut v = Vec::new();
    let filtro = json!({ "periodo": "mes", "de": "", "ate": "", "vendedorId": "" });
    let pedidos: Vec<(&str, Value)> = vec![
        ("dre_apurar", filtro.clone()),
        ("dre_indicadores", filtro),
        ("dre_metas", json!({})),
        ("receber_painel", json!({ "texto": "" })),
        ("receber_cliente", json!({ "id": p.ze, "todas": true })),
        ("financeiro_resumo", json!({})),
        ("pagar_painel", json!({})),
        ("painel_resumo", json!({})),
        ("vendas_listar", json!({ "tipo": "", "texto": "", "sessao": "" })),
        ("produtos_buscar", json!({ "texto": "", "limite": 100 })),
        ("compras_listar", json!({ "texto": "", "limite": 300 })),
        ("devolucoes_listar", json!({ "texto": "" })),
        ("contas_loja", json!({ "quais": "todas" })),
        ("pessoas_buscar", json!({ "texto": "", "papel": "", "inativos": true, "limite": 100 })),
        ("venda_abrir", json!({ "id": p.v1 })),
        ("venda_abrir", json!({ "id": p.v2 })),
        ("venda_abrir", json!({ "id": p.v3 })),
        ("venda_abrir", json!({ "id": p.v4 })),
        ("compra_abrir", json!({ "id": p.compra })),
    ];
    for (c, d) in pedidos {
        v.push((format!("{c} {d}"), l.ok(c, d)));
    }
    for peca in [&p.filtro, &p.pastilha, &p.oleo] {
        let mut f = l.ok("produto_abrir", json!({ "id": peca }));
        // O histórico da peça vem da auditoria, que ganha a linha da restauração.
        f["historico"] = Value::Null;
        v.push((format!("produto {peca}"), f));
    }
    v
}

/// T1.1 — Faz o backup na loja A, restaura numa loja nova B (outra pasta,
/// outro "computador") e confere tabela por tabela e tela por tela.
#[test]
fn t1_1_restaurar_em_outra_pasta_confere_tudo() {
    let _vez = vez_do_backup();
    let mut a = Loja::nova("t1-origem");
    let p = povoar(&mut a);
    let antes = foto_do_negocio(&a.conexao());
    // A comparação só vale se havia o que comparar.
    for t in [
        "venda",
        "venda_item",
        "venda_pagamento",
        "conta_receber",
        "recebimento",
        "conta_pagar",
        "pagamento",
        "compra",
        "compra_item",
        "devolucao",
        "estoque_movimento",
        "meta",
        "caixa_sessao",
        "caixa_movimento",
        "movimento_conta",
    ] {
        assert!(!antes[t].is_empty(), "a loja de teste não tem nada em {t}");
    }
    let telas_a = telas(&mut a, &p);
    let config_a = config_da_loja(&a);

    let feito = rustinho_nucleo::backup::fazer(&a.db, false).unwrap();
    let arquivo = std::path::PathBuf::from(&feito.arquivo);
    let marca_backup = marca_do_arquivo(&arquivo);

    // Computador novo: instala, configura com outro gerente e restaura.
    let mut b = Loja::nova("t1-destino");
    b.ok("caixa_abrir", json!({ "valor": "0" }));
    // Senha errada: recusa e não muda nada.
    let antes_b = foto_do_negocio(&b.conexao());
    b.erro("backup_restaurar", json!({ "arquivo": feito.arquivo, "nuvem": false, "senha": "errada" }));
    assert!(diferencas(&antes_b, &foto_do_negocio(&b.conexao())).is_empty(), "a senha errada mudou o banco");

    let info = b.ok("backup_restaurar", json!({ "arquivo": feito.arquivo, "nuvem": false, "senha": SENHA_ANA }));
    assert_eq!(info["loja"], "Loja Teste Auto Peças");
    // Depois de restaurar, entra de novo (os usuários agora são os do backup).
    assert!(b.chamar("produtos_buscar", json!({}))["erro"].as_str().unwrap().contains("Entre"));
    b.entrar("Ana", SENHA_ANA);

    let depois = foto_do_negocio(&b.conexao());
    let d = diferencas(&antes, &depois);
    assert!(d.is_empty(), "o restaurado não é igual à origem:\n{}", d.join("\n"));
    assert_eq!(config_da_loja(&b), config_a, "configurações da loja");
    let telas_b = telas(&mut b, &p);
    for ((nome, va), (_, vb)) in telas_a.iter().zip(telas_b.iter()) {
        assert_eq!(va, vb, "a tela {nome} mostra outra coisa depois de restaurar");
    }

    // A origem não mudou: o backup e os dados de A continuam os mesmos.
    assert_eq!(marca_do_arquivo(&arquivo), marca_backup, "o arquivo do backup foi alterado");
    let d = diferencas(&antes, &foto_do_negocio(&a.conexao()));
    assert!(d.is_empty(), "a loja de origem mudou: {d:?}");

    // B continua trabalhando: vende, e o estoque e o caixa seguem.
    let est = centavos(&b.estoque(&p.filtro));
    b.abrir_caixa();
    b.vender(&[(&p.filtro, "1")], &[("dinheiro", "100")]);
    assert_eq!(centavos(&b.estoque(&p.filtro)), est - 100);
    let nova = b.peca("Peça depois de restaurar", "10", "20", "1");
    assert!(!nova.is_empty());
    // Fechar e abrir de novo não perde nada.
    let foto = foto_do_negocio(&b.conexao());
    b.reabrir();
    b.entrar("Ana", SENHA_ANA);
    assert!(diferencas(&foto, &foto_do_negocio(&b.conexao())).is_empty());
}

/// T1.2 — O backup guarda uma cópia do que existia antes de restaurar, e
/// essa cópia volta tudo como era (desfazer uma restauração errada).
#[test]
fn t1_2_copia_de_antes_desfaz_a_restauracao() {
    let _vez = vez_do_backup();
    let mut a = Loja::nova("t1-2a");
    povoar(&mut a);
    let feito_a = rustinho_nucleo::backup::fazer(&a.db, false).unwrap();

    let mut b = Loja::nova("t1-2b");
    let x = b.peca("Só em B", "5", "10", "3");
    b.abrir_caixa();
    b.vender(&[(&x, "1")], &[("dinheiro", "10")]);
    let foto_b = foto_do_negocio(&b.conexao());

    b.ok("backup_restaurar", json!({ "arquivo": feito_a.arquivo, "nuvem": false, "senha": SENHA_ANA }));
    b.entrar("Ana", SENHA_ANA);
    let copias =
        rustinho_nucleo::backup::listar(&rustinho_nucleo::backup::ajustes(&b.conexao(), Some(&b.db)).unwrap().pasta);
    let antes = copias.iter().find(|c| c.antes).expect("a cópia de antes de restaurar");
    // Volta a cópia de antes: B fica como era (a senha é a do gerente de B, a mesma Ana/1234).
    b.ok("backup_restaurar", json!({ "arquivo": antes.caminho, "nuvem": false, "senha": SENHA_ANA }));
    b.entrar("Ana", SENHA_ANA);
    let d = diferencas(&foto_b, &foto_do_negocio(&b.conexao()));
    assert!(d.is_empty(), "a cópia de antes não voltou tudo: {d:?}");
}

/// T1.3 — Backup estragado ou de outro programa: recusa sem mudar nada.
#[test]
fn t1_3_backup_estragado_nao_muda_nada() {
    let _vez = vez_do_backup();
    let mut a = Loja::nova("t1-3a");
    povoar(&mut a);
    let feito = rustinho_nucleo::backup::fazer(&a.db, false).unwrap();
    let mut b = Loja::nova("t1-3b");
    b.peca("Só em B", "5", "10", "3");
    let foto = foto_do_negocio(&b.conexao());
    // Um byte trocado no meio do arquivo.
    let mut bytes = std::fs::read(&feito.arquivo).unwrap();
    let meio = bytes.len() / 2;
    bytes[meio] ^= 0xff;
    let estragado = b.pasta.join("estragado.rbk");
    std::fs::write(&estragado, &bytes).unwrap();
    b.erro(
        "backup_restaurar",
        json!({ "arquivo": estragado.display().to_string(), "nuvem": false, "senha": SENHA_ANA }),
    );
    // Um arquivo qualquer.
    let outro = b.pasta.join("qualquer.rbk");
    std::fs::write(&outro, b"isto nao e um backup").unwrap();
    b.erro("backup_restaurar", json!({ "arquivo": outro.display().to_string(), "nuvem": false, "senha": SENHA_ANA }));
    assert!(diferencas(&foto, &foto_do_negocio(&b.conexao())).is_empty());
    // E B continua conectado e funcionando.
    b.peca("Depois", "1", "2", "1");
}

fn config_da_loja(l: &Loja) -> Vec<(String, String)> {
    let c = l.conexao();
    let mut s = c
        .prepare("SELECT chave, valor FROM config ORDER BY chave")
        .unwrap()
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    s.retain(|(k, _)| rustinho_nucleo::sync::CONFIG_DA_LOJA.contains(&k.as_str()));
    s
}
