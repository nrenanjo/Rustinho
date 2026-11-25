//! R-03 (auditoria da 0.9.14): dois caixas sem rede recebem (ou pagam) a mesma
//! parcela, ou usam o mesmo vale-troca. Cada caixa conferiu o saldo com o que
//! ele mesmo sabia; depois da troca de dados a parcela fica paga a mais (ou o
//! vale usado além do valor), e antes isso sumia das listas sem aviso.
//!
//! Exigido: os pagamentos legítimos de cada caixa ficam (nada é apagado para
//! fechar a conta); aparece um conflito explícito para o gerente, com o que é
//! preciso para achar as operações; a correção é a do próprio gerente (estorno
//! pelo Receber, Pagar ou pela venda, que já fica no histórico); e, corrigido,
//! o conflito sai sozinho.
//!
//! Os dados "do outro caixa" entram aqui como entrariam pela troca de dados
//! (as linhas, com o dispositivo do outro caixa); a detecção é a mesma que
//! roda depois de cada puxada (`sync::verificar_depois`).

use serde_json::{json, Value};

use crate::sync;

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| {
        s.chamar(cmd, x)
    }))
    .unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

/// Loja de um caixa só (gerente Ana), com o caixa aberto e um cliente.
fn loja() -> (crate::Servico, String) {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "12345678", "senha2": "12345678" }),
    );
    ok(&mut s, "caixa_abrir", json!({ "valor": "100" }));
    let mut c = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    c["nome"] = json!("Zé da Silva");
    let cliente = ok(&mut s, "pessoa_salvar", c)["id"].as_str().unwrap().to_string();
    (s, cliente)
}

fn conn(s: &mut crate::Servico) -> &rusqlite::Connection {
    &s.banco_para_teste().conn
}

/// O outro caixa conhecido pela troca de dados.
fn outro_caixa(s: &mut crate::Servico) {
    conn(s)
        .execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco, visto_em) VALUES ('disp-caixa-2', 'Caixa 2', '', '2026-10-28')",
            [],
        )
        .unwrap();
}

/// Uma cópia de uma linha, com outro id e o dispositivo do outro caixa
/// (como se tivesse chegado pela troca de dados).
fn copiar_do_outro_caixa(s: &mut crate::Servico, tabela: &str, id: &str, novo: &str, troca: &[(&str, &str)]) {
    let c = conn(s);
    let colunas: Vec<String> = c
        .prepare(&format!("SELECT name FROM pragma_table_info('{tabela}')"))
        .unwrap()
        .query_map([], |l| l.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let selecao: Vec<String> = colunas
        .iter()
        .map(|col| match col.as_str() {
            "id" => format!("'{novo}'"),
            "dispositivo" => "'disp-caixa-2'".into(),
            "caixa_nome" => "'Caixa 2'".into(),
            "sessao_id" => "NULL".into(),
            outra => troca.iter().find(|(k, _)| *k == outra).map(|(_, v)| v.to_string()).unwrap_or(outra.to_string()),
        })
        .collect();
    c.execute(
        &format!(
            "INSERT INTO {tabela} ({}) SELECT {} FROM {tabela} WHERE id = ?1",
            colunas.join(", "),
            selecao.join(", ")
        ),
        [id],
    )
    .unwrap();
}

fn abertos_do_tipo(s: &mut crate::Servico, tipo: &str) -> Vec<Value> {
    ok(s, "conflitos_listar", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .filter(|k| k["tipo"] == tipo && k["resolvido"].as_str().unwrap_or("").is_empty())
        .cloned()
        .collect()
}

/// A mesma parcela recebida nos dois caixas sem rede: depois da troca,
/// conflito para o gerente (com os dois recebimentos e os caixas), os dois
/// recebimentos continuam lá, e estornar o que sobrou fecha o conflito.
#[test]
fn r03_parcela_recebida_em_dois_caixas_vira_conflito() {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let parcela: String = conn(&mut s).query_row("SELECT id FROM conta_receber", [], |l| l.get(0)).unwrap();
    let r = ok(&mut s, "receber_baixar", json!({ "clienteId": cliente, "contas": [parcela], "forma": "dinheiro" }));
    let meu = r["id"].as_str().unwrap().to_string();
    // O Caixa 2, sem rede, recebeu a mesma parcela.
    copiar_do_outro_caixa(&mut s, "recebimento", &meu, "rec-caixa-2", &[]);
    let rp_meu: String = conn(&mut s)
        .query_row("SELECT id FROM recebimento_parcela WHERE recebimento_id = ?1", [&meu], |l| l.get(0))
        .unwrap();
    copiar_do_outro_caixa(&mut s, "recebimento_parcela", &rp_meu, "rp-caixa-2", &[("recebimento_id", "'rec-caixa-2'")]);
    let pago: i64 =
        conn(&mut s).query_row("SELECT pago FROM conta_receber_saldo WHERE id = ?1", [&parcela], |l| l.get(0)).unwrap();
    assert_eq!(pago, 10_000, "a parcela de R$ 50 ficou paga duas vezes");
    sync::verificar_depois(conn(&mut s)).unwrap();
    let k = abertos_do_tipo(&mut s, "pago_a_mais");
    assert_eq!(k.len(), 1, "um conflito para o gerente: {k:?}");
    let texto = k[0]["texto"].as_str().unwrap();
    eprintln!("R-03 texto: {texto}");
    assert!(texto.contains("Zé da Silva") && texto.contains("Caixa 2") && texto.contains("Caixa 1"), "{texto}");
    assert!(texto.contains("50,00") && texto.contains("100,00"), "valor e total pago: {texto}");
    // Nada foi apagado para fechar a conta.
    let recebimentos: i64 = conn(&mut s)
        .query_row("SELECT COUNT(*) FROM recebimento WHERE estornado_em IS NULL", [], |l| l.get(0))
        .unwrap();
    assert_eq!(recebimentos, 2);
    // Repetir a conferência não duplica o conflito.
    sync::verificar_depois(conn(&mut s)).unwrap();
    assert_eq!(abertos_do_tipo(&mut s, "pago_a_mais").len(), 1);
    // O Início avisa (diferenças entre caixas para o gerente ver).
    let avisos = ok(&mut s, "avisos_do_dia", json!({}));
    assert!(avisos.as_array().unwrap().iter().any(|a| a["id"] == "conflitos"), "{avisos}");
    // O gerente estorna o que sobrou (o estorno fica no histórico): o
    // conflito sai da lista na hora e fica anotado como corrigido.
    ok(&mut s, "recebimento_estornar", json!({ "id": meu, "motivo": "recebido também no Caixa 2" }));
    assert!(abertos_do_tipo(&mut s, "pago_a_mais").is_empty(), "corrigido, o conflito sai");
    sync::verificar_depois(conn(&mut s)).unwrap();
    let abertos: i64 = conn(&mut s)
        .query_row("SELECT COUNT(*) FROM sync_conflito WHERE tipo = 'pago_a_mais' AND resolvido_em IS NULL", [], |l| {
            l.get(0)
        })
        .unwrap();
    assert_eq!(abertos, 0, "anotado como corrigido");
}

/// R2 (D-AUD-4, decisão do proprietário de 25/11/2026): a ligação de abatimento zero (o recebimento que acertou só
/// os juros da parcela) não é recebimento da parcela: fica fora das operações do conflito "recebida mais de uma vez".
#[test]
fn r2_ligacao_de_abatimento_zero_fica_fora_do_conflito_de_pago_a_mais() {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let parcela: String = conn(&mut s).query_row("SELECT id FROM conta_receber", [], |l| l.get(0)).unwrap();
    let r = ok(&mut s, "receber_baixar", json!({ "clienteId": cliente, "contas": [parcela], "forma": "dinheiro" }));
    let meu = r["id"].as_str().unwrap().to_string();
    let rp_meu: String = conn(&mut s)
        .query_row("SELECT id FROM recebimento_parcela WHERE recebimento_id = ?1", [&meu], |l| l.get(0))
        .unwrap();
    copiar_do_outro_caixa(&mut s, "recebimento", &meu, "rec-caixa-2", &[]);
    copiar_do_outro_caixa(&mut s, "recebimento_parcela", &rp_meu, "rp-caixa-2", &[("recebimento_id", "'rec-caixa-2'")]);
    // Um terceiro recebimento, que só acertou os juros desta parcela (abatimento zero).
    copiar_do_outro_caixa(&mut s, "recebimento", &meu, "rec-so-juros", &[]);
    copiar_do_outro_caixa(
        &mut s,
        "recebimento_parcela",
        &rp_meu,
        "rp-so-juros",
        &[("recebimento_id", "'rec-so-juros'"), ("valor", "0")],
    );
    sync::verificar_depois(conn(&mut s)).unwrap();
    let ops: String = conn(&mut s)
        .query_row("SELECT perdeu FROM sync_conflito WHERE tipo = 'pago_a_mais'", [], |l| l.get(0))
        .unwrap();
    assert!(ops.contains(&meu) && ops.contains("rec-caixa-2"), "{ops}");
    assert!(!ops.contains("rec-so-juros"), "a ligação de abatimento zero entrou no conflito: {ops}");
}

/// A mesma conta a pagar paga nos dois caixas: o mesmo tratamento.
#[test]
fn r03_conta_paga_em_dois_caixas_vira_conflito() {
    let (mut s, _) = loja();
    outro_caixa(&mut s);
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "fornecedor" }));
    f["nome"] = json!("Distribuidora Boa");
    let fornecedor = ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string();
    ok(
        &mut s,
        "pagar_lancar",
        json!({ "fornecedorId": fornecedor, "descricao": "boleto", "valor": "80", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let conta: String = conn(&mut s).query_row("SELECT id FROM conta_pagar", [], |l| l.get(0)).unwrap();
    ok(&mut s, "pagar_pagar", json!({ "contaId": conta, "valor": "", "forma": "pix", "origem": "caixa-geral" }));
    let meu: String = conn(&mut s).query_row("SELECT id FROM pagamento", [], |l| l.get(0)).unwrap();
    copiar_do_outro_caixa(&mut s, "pagamento", &meu, "pag-caixa-2", &[]);
    sync::verificar_depois(conn(&mut s)).unwrap();
    let k = abertos_do_tipo(&mut s, "pago_a_mais");
    assert_eq!(k.len(), 1, "{k:?}");
    let texto = k[0]["texto"].as_str().unwrap();
    eprintln!("R-03 texto: {texto}");
    assert!(texto.contains("Distribuidora Boa") && texto.contains("Caixa 2"), "{texto}");
    let pagamentos: i64 =
        conn(&mut s).query_row("SELECT COUNT(*) FROM pagamento WHERE estornado_em IS NULL", [], |l| l.get(0)).unwrap();
    assert_eq!(pagamentos, 2, "nada apagado");
}

/// O mesmo vale-troca usado nos dois caixas além do valor: conflito.
#[test]
fn r03_vale_usado_em_dois_caixas_vira_conflito() {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    let c = conn(&mut s);
    c.execute(
        "INSERT INTO vale (id, numero, dispositivo, caixa_nome, cliente_id, cliente_nome, valor, criado_em)
         VALUES ('vale-1', 1, 'disp-caixa-2', 'Caixa 2', ?1, 'Zé da Silva', 3000, '2026-10-27')",
        [&cliente],
    )
    .unwrap();
    // Duas vendas (uma em cada caixa) pagas com o vale inteiro.
    for (id, disp, caixa) in [("venda-a", "disp-a", "Caixa 1"), ("venda-b", "disp-caixa-2", "Caixa 2")] {
        c.execute(
            "INSERT INTO venda (id, numero, dispositivo, caixa_nome, tipo, situacao, cliente_id, cliente_nome, subtotal, total, criado_em)
             VALUES (?1, 1, ?2, ?3, 'venda', 'concluida', ?4, 'Zé da Silva', 3000, 3000, '2026-10-28')",
            rusqlite::params![id, disp, caixa, cliente],
        )
        .unwrap();
        c.execute(
            "INSERT INTO venda_pagamento (id, venda_id, forma, valor, vale_id) VALUES (?1, ?2, 'vale', 3000, 'vale-1')",
            rusqlite::params![format!("pg-{id}"), id],
        )
        .unwrap();
    }
    let saldo: i64 = c.query_row("SELECT saldo FROM vale_saldo WHERE id = 'vale-1'", [], |l| l.get(0)).unwrap();
    assert_eq!(saldo, -3000, "o vale de R$ 30 foi usado duas vezes");
    sync::verificar_depois(c).unwrap();
    let k = abertos_do_tipo(&mut s, "vale_a_mais");
    assert_eq!(k.len(), 1, "{k:?}");
    let texto = k[0]["texto"].as_str().unwrap();
    eprintln!("R-03 texto: {texto}");
    assert!(texto.contains("Zé da Silva") && texto.contains("30,00"), "{texto}");
}

/// O id do conflito aberto do tipo pedido (o primeiro).
fn id_do_conflito(s: &mut crate::Servico, tipo: &str) -> i64 {
    abertos_do_tipo(s, tipo)[0]["id"].as_i64().unwrap()
}

/// R15 (R-03, decisão do proprietário de 25/11/2026, regra 15): a parcela recebida nos dois caixas vem com a
/// compensação pronta (estornar o recebimento que sobrou, o mais novo, de valor igual ao que passou). Só o gerente
/// faz; com um clique, o recebimento sobrando é estornado, o conflito sai, e tudo fica no histórico. Feita, não
/// repete.
#[test]
fn r15_compensacao_pronta_da_parcela_recebida_duas_vezes() {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let parcela: String = conn(&mut s).query_row("SELECT id FROM conta_receber", [], |l| l.get(0)).unwrap();
    let r = ok(&mut s, "receber_baixar", json!({ "clienteId": cliente, "contas": [parcela], "forma": "dinheiro" }));
    let meu = r["id"].as_str().unwrap().to_string();
    copiar_do_outro_caixa(
        &mut s,
        "recebimento",
        &meu,
        "rec-caixa-2",
        &[("criado_em", "strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+1 second')")],
    );
    let rp_meu: String = conn(&mut s)
        .query_row("SELECT id FROM recebimento_parcela WHERE recebimento_id = ?1", [&meu], |l| l.get(0))
        .unwrap();
    copiar_do_outro_caixa(&mut s, "recebimento_parcela", &rp_meu, "rp-caixa-2", &[("recebimento_id", "'rec-caixa-2'")]);
    sync::verificar_depois(conn(&mut s)).unwrap();
    let k = abertos_do_tipo(&mut s, "pago_a_mais");
    let proposta = k[0]["compensacao"].as_str().unwrap().to_string();
    assert!(
        proposta.contains("Estornar o recebimento Nº") && proposta.contains("Caixa 2") && proposta.contains("50,00"),
        "{proposta}"
    );
    let id = id_do_conflito(&mut s, "pago_a_mais");
    // Quem não é gerente não faz.
    let bia = crate::usuarios::criar(conn(&mut s), "disp", "Bia", "12345678", true).unwrap().id;
    let ana: String = conn(&mut s).query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    ok(&mut s, "entrar", json!({ "id": bia, "senha": "12345678" }));
    let r = chamar(&mut s, "conflito_compensar", json!({ "id": id }));
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("gerente")), "{r}");
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "12345678" }));
    // B2 da revisão R3: a operação mostrada mudou → recusa sem mudar nada; a certa passa.
    let r = chamar(&mut s, "conflito_compensar", json!({ "id": id, "operacao": "outra-operacao" }));
    assert!(r["erro"].as_str().unwrap_or("").contains("A compensação mudou"), "{r}");
    let operacao = k[0]["compensacaoOperacao"].as_str().unwrap().to_string();
    assert_eq!(operacao, "rec-caixa-2");
    ok(&mut s, "conflito_compensar", json!({ "id": id, "operacao": operacao }));
    assert!(abertos_do_tipo(&mut s, "pago_a_mais").is_empty(), "o conflito saiu");
    let estornado: Option<String> = conn(&mut s)
        .query_row("SELECT estornado_em FROM recebimento WHERE id = 'rec-caixa-2'", [], |l| l.get(0))
        .unwrap();
    assert!(estornado.is_some(), "o recebimento que sobrou (o mais novo) foi estornado");
    let ativos: i64 = conn(&mut s)
        .query_row("SELECT COUNT(*) FROM recebimento WHERE estornado_em IS NULL", [], |l| l.get(0))
        .unwrap();
    assert_eq!(ativos, 1, "o primeiro fica");
    let registro: i64 = conn(&mut s)
        .query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'conflito.compensar'", [], |l| l.get(0))
        .unwrap();
    assert_eq!(registro, 1);
    let r = chamar(&mut s, "conflito_compensar", json!({ "id": id, "operacao": "rec-caixa-2" }));
    assert!(!r["ok"].as_bool().unwrap() && r["erro"].as_str().unwrap().contains("não tem compensação pronta"), "{r}");
}

/// R15: a conta paga nos dois caixas vem com a compensação pronta (estornar o pagamento que sobrou).
#[test]
fn r15_compensacao_pronta_da_conta_paga_duas_vezes() {
    let (mut s, _) = loja();
    outro_caixa(&mut s);
    let mut f = ok(&mut s, "pessoa_nova", json!({ "papel": "fornecedor" }));
    f["nome"] = json!("Distribuidora Boa");
    let fornecedor = ok(&mut s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string();
    ok(
        &mut s,
        "pagar_lancar",
        json!({ "fornecedorId": fornecedor, "descricao": "boleto", "valor": "80", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let conta: String = conn(&mut s).query_row("SELECT id FROM conta_pagar", [], |l| l.get(0)).unwrap();
    ok(&mut s, "pagar_pagar", json!({ "contaId": conta, "valor": "", "forma": "pix", "origem": "caixa-geral" }));
    let meu: String = conn(&mut s).query_row("SELECT id FROM pagamento", [], |l| l.get(0)).unwrap();
    copiar_do_outro_caixa(
        &mut s,
        "pagamento",
        &meu,
        "pag-caixa-2",
        &[("criado_em", "strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+1 second')")],
    );
    sync::verificar_depois(conn(&mut s)).unwrap();
    let k = abertos_do_tipo(&mut s, "pago_a_mais");
    let proposta = k[0]["compensacao"].as_str().unwrap().to_string();
    assert!(proposta.contains("Estornar o pagamento de R$ 80,00 feito no Caixa 2"), "{proposta}");
    let id = id_do_conflito(&mut s, "pago_a_mais");
    // B1 da revisão R4: sem a operação mostrada, recusa (B2 da R5: com a mensagem em português simples).
    let r = chamar(&mut s, "conflito_compensar", json!({ "id": id }));
    assert!(r["erro"].as_str().unwrap_or("").contains("A tela não mandou qual operação"), "{r}");
    ok(&mut s, "conflito_compensar", json!({ "id": id, "operacao": k[0]["compensacaoOperacao"] }));
    assert!(abertos_do_tipo(&mut s, "pago_a_mais").is_empty());
    let estornado: Option<String> = conn(&mut s)
        .query_row("SELECT estornado_em FROM pagamento WHERE id = 'pag-caixa-2'", [], |l| l.get(0))
        .unwrap();
    assert!(estornado.is_some());
}

/// R15: sem uma operação de valor igual ao que passou (três recebimentos: 50 aqui, 30 e 30 no outro caixa, numa
/// parcela de 50), não há compensação exata: a proposta vem vazia e o comando recusa sem mudar nada (o gerente
/// estorna pela tela do Receber, como antes).
#[test]
fn r15_sem_compensacao_exata_recusa() {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let parcela: String = conn(&mut s).query_row("SELECT id FROM conta_receber", [], |l| l.get(0)).unwrap();
    let r = ok(&mut s, "receber_baixar", json!({ "clienteId": cliente, "contas": [parcela], "forma": "dinheiro" }));
    let meu = r["id"].as_str().unwrap().to_string();
    let rp_meu: String = conn(&mut s)
        .query_row("SELECT id FROM recebimento_parcela WHERE recebimento_id = ?1", [&meu], |l| l.get(0))
        .unwrap();
    for n in ["2", "3"] {
        copiar_do_outro_caixa(&mut s, "recebimento", &meu, &format!("rec-caixa-{n}"), &[("valor", "3000")]);
        copiar_do_outro_caixa(
            &mut s,
            "recebimento_parcela",
            &rp_meu,
            &format!("rp-caixa-{n}"),
            &[("recebimento_id", &format!("'rec-caixa-{n}'")), ("valor", "3000")],
        );
    }
    sync::verificar_depois(conn(&mut s)).unwrap();
    let k = abertos_do_tipo(&mut s, "pago_a_mais");
    assert_eq!(k[0]["compensacao"], "", "{k:?}");
    let id = id_do_conflito(&mut s, "pago_a_mais");
    let r = chamar(&mut s, "conflito_compensar", json!({ "id": id, "operacao": "rec-caixa-2" }));
    assert!(!r["ok"].as_bool().unwrap(), "{r}");
    let ativos: i64 = conn(&mut s)
        .query_row("SELECT COUNT(*) FROM recebimento WHERE estornado_em IS NULL", [], |l| l.get(0))
        .unwrap();
    assert_eq!(ativos, 3, "nada mudou");
}

/// R15: um recebimento que pagou duas parcelas, repetido no outro caixa, deixa as duas pagas a mais. Estornar um
/// recebimento inteiro reabriria a outra parcela, então nenhuma das duas tem compensação pronta (o gerente estorna
/// pela tela do Receber, como antes).
#[test]
fn r15_recebimento_de_duas_parcelas_nao_tem_compensacao_pronta() {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "100", "parcelas": 2, "primeiroVencimento": "" }),
    );
    let parcelas: Vec<String> = conn(&mut s)
        .prepare("SELECT id FROM conta_receber ORDER BY parcela")
        .unwrap()
        .query_map([], |l| l.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let r = ok(&mut s, "receber_baixar", json!({ "clienteId": cliente, "contas": parcelas, "forma": "dinheiro" }));
    let meu = r["id"].as_str().unwrap().to_string();
    copiar_do_outro_caixa(&mut s, "recebimento", &meu, "rec-caixa-2", &[]);
    let rps: Vec<String> = conn(&mut s)
        .prepare("SELECT id FROM recebimento_parcela WHERE recebimento_id = ?1")
        .unwrap()
        .query_map([&meu], |l| l.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for (i, rp) in rps.iter().enumerate() {
        copiar_do_outro_caixa(
            &mut s,
            "recebimento_parcela",
            rp,
            &format!("rp-caixa-2-{i}"),
            &[("recebimento_id", "'rec-caixa-2'")],
        );
    }
    sync::verificar_depois(conn(&mut s)).unwrap();
    let k = abertos_do_tipo(&mut s, "pago_a_mais");
    assert_eq!(k.len(), 2, "{k:?}");
    assert!(k.iter().all(|x| x["compensacao"] == ""), "{k:?}");
}

/// O vale de R$ 30 usado duas vezes (R$ 30 em cada caixa), como no `r03_vale_usado_em_dois_caixas_vira_conflito`,
/// com ou sem cliente cadastrado. Devolve o serviço e o id do cliente (vazio sem cadastro).
fn vale_usado_duas_vezes(com_cliente: bool) -> (crate::Servico, String) {
    let (mut s, cliente) = loja();
    outro_caixa(&mut s);
    let cliente = if com_cliente { cliente } else { String::new() };
    let c = conn(&mut s);
    c.execute(
        "INSERT INTO vale (id, numero, dispositivo, caixa_nome, cliente_id, cliente_nome, valor, criado_em)
         VALUES ('vale-1', 7, 'disp-caixa-2', 'Caixa 2', NULLIF(?1, ''), ?2, 3000, '2026-10-27')",
        rusqlite::params![cliente, if com_cliente { "Zé da Silva" } else { "" }],
    )
    .unwrap();
    for (id, disp, caixa) in [("venda-a", "disp-a", "Caixa 1"), ("venda-b", "disp-caixa-2", "Caixa 2")] {
        c.execute(
            "INSERT INTO venda (id, numero, dispositivo, caixa_nome, tipo, situacao, subtotal, total, criado_em)
             VALUES (?1, 1, ?2, ?3, 'venda', 'concluida', 3000, 3000, '2026-10-28')",
            rusqlite::params![id, disp, caixa],
        )
        .unwrap();
        c.execute(
            "INSERT INTO venda_pagamento (id, venda_id, forma, valor, vale_id) VALUES (?1, ?2, 'vale', 3000, 'vale-1')",
            rusqlite::params![format!("pg-{id}"), id],
        )
        .unwrap();
    }
    sync::verificar_depois(c).unwrap();
    (s, cliente)
}

/// Decisão 6 do proprietário (25/11/2026): vale usado além do valor, com cliente cadastrado. A compensação pronta
/// lança os R$ 30 como dívida dele no crediário (com o gerente), o conflito sai e não volta na próxima conferência.
#[test]
fn d6_vale_a_mais_com_cliente_vira_divida() {
    let (mut s, cliente) = vale_usado_duas_vezes(true);
    let k = abertos_do_tipo(&mut s, "vale_a_mais");
    let proposta = k[0]["compensacao"].as_str().unwrap().to_string();
    assert!(proposta.contains("Lançar os R$ 30,00") && proposta.contains("dívida de Zé da Silva"), "{proposta}");
    let operacao = k[0]["compensacaoOperacao"].as_str().unwrap().to_string();
    ok(&mut s, "conflito_compensar", json!({ "id": k[0]["id"], "operacao": operacao }));
    assert!(abertos_do_tipo(&mut s, "vale_a_mais").is_empty(), "o conflito saiu");
    let aberto = crate::receber::situacao(conn(&mut s), &cliente).unwrap().em_aberto;
    assert_eq!(aberto, "30,00", "a diferença virou dívida do cliente");
    sync::verificar_depois(conn(&mut s)).unwrap();
    assert!(abertos_do_tipo(&mut s, "vale_a_mais").is_empty(), "não volta na próxima conferência");
    let registro: i64 =
        conn(&mut s).query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'vale.acertar'", [], |l| l.get(0)).unwrap();
    assert_eq!(registro, 1);
}

/// Decisão 6: sem cliente cadastrado, a loja assume a diferença; fica o registro, e nenhuma dívida é lançada.
#[test]
fn d6_vale_a_mais_sem_cliente_a_loja_assume() {
    let (mut s, _) = vale_usado_duas_vezes(false);
    let k = abertos_do_tipo(&mut s, "vale_a_mais");
    let proposta = k[0]["compensacao"].as_str().unwrap().to_string();
    assert!(proposta.contains("A loja assume os R$ 30,00") && proposta.contains("fica o registro"), "{proposta}");
    let operacao = k[0]["compensacaoOperacao"].as_str().unwrap().to_string();
    ok(&mut s, "conflito_compensar", json!({ "id": k[0]["id"], "operacao": operacao }));
    assert!(abertos_do_tipo(&mut s, "vale_a_mais").is_empty());
    let dividas: i64 = conn(&mut s).query_row("SELECT COUNT(*) FROM conta_receber", [], |l| l.get(0)).unwrap();
    assert_eq!(dividas, 0, "nenhuma dívida");
    let detalhes: String =
        conn(&mut s).query_row("SELECT detalhes FROM auditoria WHERE acao = 'vale.acertar'", [], |l| l.get(0)).unwrap();
    assert!(detalhes.contains("A loja assumiu R$ 30,00"), "{detalhes}");
}
