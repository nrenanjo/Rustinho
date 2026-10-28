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
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
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
