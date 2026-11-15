//! R0 da recuperação do trabalho (QT-1, preparado em 14/11/2026; parecer da 0.9.19 §3 e §5): a operação já
//! feita fica no banco, na transação da própria operação.
//!
//! No Android, o Qt 6.8 reinicia o processo numa recriação da atividade (visto no POCO C85): a lista das
//! chaves que ficava só na memória se perdia. Estes testes provam que a decisão agora é do banco.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::api::{EXCECOES_R0, NAO_REPETIR};

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

fn temp(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-r0-{nome}-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join("rustinho.db")
}

fn contar(s: &mut crate::Servico, sql: &str) -> i64 {
    s.banco_para_teste().conn.query_row(sql, [], |l| l.get(0)).unwrap()
}

fn configurar(s: &mut crate::Servico) {
    ok(
        s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
}

/// Uma peça com estoque e o caixa aberto; devolve o id da peça.
fn preparar_venda(s: &mut crate::Servico) -> String {
    let mut f = ok(s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("10");
    f["chaveOperacao"] = json!("peca-1");
    let p = ok(s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(s, "caixa_abrir", json!({ "valor": "0", "chaveOperacao": "abrir-1" }));
    p
}

fn venda(p: &str, chave: &str, quantidade: &str) -> Value {
    json!({ "itens": [{ "produtoId": p, "quantidade": quantidade }], "tabela": 1,
            "pagamentos": [{ "forma": "dinheiro", "valor": "100" }], "chaveOperacao": chave })
}

/// Cenário 12 do parecer (§5): depois de REINICIAR (o serviço fechado e aberto de novo, como no reinício
/// do Qt), a mesma chave não grava de novo e devolve o resultado da primeira vez.
#[test]
fn depois_de_reiniciar_a_mesma_chave_nao_grava_de_novo() {
    let caminho = temp("reiniciar");
    let (p, primeira) = {
        let mut s = crate::Servico::abrir(&caminho).unwrap();
        configurar(&mut s);
        let p = preparar_venda(&mut s);
        let primeira = ok(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
        (p, primeira)
    };
    let mut s = crate::Servico::abrir(&caminho).unwrap();
    let ana = usuario_ana(&mut s);
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let r = chamar(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
    assert_eq!((r["ok"].clone(), r["repetida"].clone()), (json!(true), json!(true)), "{r}");
    assert_eq!(r["dados"], primeira, "o resultado da primeira vez, inteiro");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"), 1, "uma venda só");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

fn usuario_ana(s: &mut crate::Servico) -> String {
    let lista = ok(s, "usuarios_entrada", json!({}));
    lista.as_array().unwrap().iter().find(|u| u["nome"] == "Ana").unwrap()["id"].as_str().unwrap().to_string()
}

/// Cenário 13 (§5): a mesma chave com OUTRO pedido é recusada, e nada é gravado.
#[test]
fn mesma_chave_com_outro_pedido_e_recusada() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
    // QT-1b: a entrega da venda (marca de controle do serviço) acontece na chamada seguinte; a medição é do comando.
    ok(&mut s, "estado", json!({}));
    let antes = s.banco_para_teste().conn.total_changes();
    let r = chamar(&mut s, "venda_concluir", venda(&p, "venda-1", "2"));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "chave", "{r}");
    assert_eq!(s.banco_para_teste().conn.total_changes(), antes, "nada gravado");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"), 1);
}

/// Cenário 14 (§5): uma operação que falha dentro da transação não deixa marca; a mesma chave, depois,
/// faz a operação normalmente (sem um "já feita" falso).
#[test]
fn falha_na_transacao_nao_deixa_marca() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    // Pagamento negativo: recusado dentro de `vendas::concluir`, com a transação já aberta.
    let mut ruim = venda(&p, "venda-x", "1");
    ruim["pagamentos"] = json!([{ "forma": "dinheiro", "valor": "-1" }]);
    let r = chamar(&mut s, "venda_concluir", ruim);
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM operacao_feita WHERE comando = 'venda_concluir'"), 0, "sem marca");
    // A tela corrige e manda de novo com a MESMA chave: como nada ficou marcado, a operação é feita.
    let r = chamar(&mut s, "venda_concluir", venda(&p, "venda-x", "1"));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r.get("repetida").is_none(), "não é repetição: {r}");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"), 1);
}

/// A marca entra NA MESMA TRANSAÇÃO, com a resposta completa que foi para a tela.
#[test]
fn a_marca_guarda_a_resposta_da_tela() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let resposta = ok(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
    let guardada: String = s
        .banco_para_teste()
        .conn
        .query_row("SELECT resultado FROM operacao_feita WHERE comando = 'venda_concluir'", [], |l| l.get(0))
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&guardada).unwrap(), resposta);
}

/// Cenário 15 (§5): duas tentativas ao mesmo tempo com a mesma chave, em conexões diferentes: a chave
/// primária decide, e a transação da segunda é desfeita inteira.
#[test]
fn duas_tentativas_ao_mesmo_tempo_uma_so_grava() {
    let caminho = temp("concorrencia");
    drop(crate::banco::Banco::abrir(&caminho).unwrap());
    let op = crate::operacoes::identificar("venda_concluir", "k-1", "u-1", &json!({ "a": 1 }));
    let mut c1 = rusqlite::Connection::open(&caminho).unwrap();
    let mut c2 = rusqlite::Connection::open(&caminho).unwrap();
    for c in [&mut c1, &mut c2] {
        c.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
        c.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
    }
    let t1 = c1.transaction().unwrap();
    crate::operacoes::marcar(&t1, &op, &json!({ "numero": 1 })).unwrap();
    t1.commit().unwrap();
    let t2 = c2.transaction().unwrap();
    assert!(crate::operacoes::marcar(&t2, &op, &json!({ "numero": 2 })).is_err(), "a segunda bate na chave primária");
    drop(t2); // desfeita
    let n: i64 = c1.query_row("SELECT COUNT(*) FROM operacao_feita", [], |l| l.get(0)).unwrap();
    assert_eq!(n, 1);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// O `synchronous` FULL fica fixado: o que foi respondido como feito está no disco.
#[test]
fn synchronous_full_fixado() {
    let s = crate::Servico::em_memoria().unwrap();
    let mut s = s;
    let v: i64 = s.banco_para_teste().conn.query_row("PRAGMA synchronous", [], |l| l.get(0)).unwrap();
    assert_eq!(v, 2, "FULL");
}

/// Retenção (DR-5): só as chaves com mais de 30 dias saem, e só na abertura.
#[test]
fn retencao_tira_so_as_antigas() {
    let s = crate::Servico::em_memoria().unwrap();
    let mut s = s;
    let c = &s.banco_para_teste().conn;
    let velha = crate::operacoes::identificar("venda_concluir", "velha", "u", &json!({}));
    let nova = crate::operacoes::identificar("venda_concluir", "nova", "u", &json!({}));
    crate::operacoes::marcar(c, &velha, &json!(1)).unwrap();
    crate::operacoes::marcar(c, &nova, &json!(2)).unwrap();
    c.execute(
        "UPDATE operacao_feita SET feita_em = ?1 WHERE chave = ?2",
        rusqlite::params![crate::tempo::dias_atras(31), &velha.chave[..]],
    )
    .unwrap();
    assert_eq!(crate::operacoes::limpar_antigas(c, crate::operacoes::RETENCAO_DIAS).unwrap(), 1);
    assert!(!crate::operacoes::marcada(c, &velha).unwrap());
    assert!(crate::operacoes::marcada(c, &nova).unwrap());
}

/// A senha do gerente digitada para a operação não entra no resumo do pedido (nem como hash), e as chaves
/// de identificação também não.
#[test]
fn o_pedido_nao_leva_senha_nem_chaves() {
    let a = json!({ "valor": "10", "senhaGerente": "1234", "chaveOperacao": "k1", "chaveTela": "t1" });
    let b = json!({ "valor": "10", "senhaGerente": "9999", "chaveOperacao": "k2", "chaveTela": "t2" });
    assert_eq!(crate::operacoes::pedido(&a), crate::operacoes::pedido(&b));
    assert_ne!(crate::operacoes::pedido(&a), crate::operacoes::pedido(&json!({ "valor": "11" })));
    // A ordem das chaves não importa (forma canônica).
    assert_eq!(
        crate::operacoes::pedido(&json!({ "a": 1, "b": [ { "y": 2, "x": 1 } ] })),
        crate::operacoes::pedido(&serde_json::from_str::<Value>(r#"{"b":[{"x":1,"y":2}],"a":1}"#).unwrap())
    );
}

/// TESTE GERADO DA LISTA (como o R-05): todo atendente de `NAO_REPETIR` marca a operação NA PRÓPRIA
/// transação (`marcar_feita` antes do `commit`), menos as exceções de `EXCECOES_R0`. Tirar a chamada de
/// um atendente faz este teste falhar (prova por mutação no relatório da QT-1).
#[test]
fn todo_atendente_de_nao_repetir_marca_na_propria_transacao() {
    const FONTES: &[(&str, &str)] = &[
        ("operacao/vendas.rs", include_str!("operacao/vendas.rs")),
        ("operacao/oficina.rs", include_str!("operacao/oficina.rs")),
        ("operacao/devolucoes.rs", include_str!("operacao/devolucoes.rs")),
        ("operacao/caixa.rs", include_str!("operacao/caixa.rs")),
        ("operacao/receber.rs", include_str!("operacao/receber.rs")),
        ("operacao/encomendas.rs", include_str!("operacao/encomendas.rs")),
        ("operacao/pagar.rs", include_str!("operacao/pagar.rs")),
        ("operacao/financeiro.rs", include_str!("operacao/financeiro.rs")),
        ("operacao/cheques.rs", include_str!("operacao/cheques.rs")),
        ("operacao/comissoes.rs", include_str!("operacao/comissoes.rs")),
        ("compras/mod.rs", include_str!("compras/mod.rs")),
        ("compras/importar.rs", include_str!("compras/importar.rs")),
        ("cadastros/pessoas.rs", include_str!("cadastros/pessoas.rs")),
        ("catalogo/produtos.rs", include_str!("catalogo/produtos.rs")),
        ("catalogo/unidades.rs", include_str!("catalogo/unidades.rs")),
        ("servico/api.rs", include_str!("servico/api.rs")),
    ];
    // O corpo de `fn nome(` até a próxima `fn ` no mesmo nível (basta para achar a chamada e a ordem).
    fn corpo<'a>(fonte: &'a str, inicio: &str) -> Option<&'a str> {
        let i = fonte.find(inicio)?;
        let resto = &fonte[i + inicio.len()..];
        let fim = resto.find("\n    fn ").or_else(|| resto.find("\nfn ")).unwrap_or(resto.len());
        Some(&resto[..fim])
    }
    // O pedaço de um braço do `match` do serviço: de `"comando" =>` até o próximo braço com `" =>`.
    fn braco<'a>(fonte: &'a str, comando: &str) -> Option<&'a str> {
        let i = fonte.find(&format!("            \"{comando}\" => {{"))?;
        let resto = &fonte[i + comando.len() + 20..];
        let fim = resto.find("\n            \"").unwrap_or(resto.len());
        Some(&resto[..fim])
    }
    let mut falhas = Vec::new();
    for comando in NAO_REPETIR {
        if EXCECOES_R0.contains(comando) {
            continue;
        }
        let atendente = FONTES.iter().find_map(|(arquivo, fonte)| {
            let marca = format!("Comando {{ nome: \"{comando}\",");
            let linha = fonte.lines().find(|l| l.contains(&marca))?;
            let funcao = linha.split("funcao: ").nth(1)?.trim_end_matches([' ', '}', ',']);
            Some((arquivo.to_string(), corpo(fonte, &format!("fn {funcao}(p: &mut Pedido"))?))
        });
        let (onde, texto) = match atendente {
            Some(x) => x,
            None if *comando == "usuario_salvar" => {
                let api = FONTES.iter().find(|(a, _)| *a == "servico/api.rs").unwrap().1;
                assert!(api.contains("\"usuario_salvar\" => self.usuario_salvar("), "o braço do usuario_salvar mudou");
                ("servico/api.rs (usuario_salvar)".into(), corpo(api, "fn usuario_salvar(&mut self").unwrap())
            }
            None => {
                let api = FONTES.iter().find(|(a, _)| *a == "servico/api.rs").unwrap().1;
                match braco(api, comando) {
                    Some(b) => ("servico/api.rs".into(), b),
                    None => {
                        falhas.push(format!("{comando}: atendente não achado"));
                        continue;
                    }
                }
            }
        };
        let marca = texto.rfind("marcar_feita(");
        let commit = texto.rfind(".commit()");
        match (marca, commit) {
            (Some(m), Some(c)) if m < c => {}
            _ => falhas.push(format!("{comando} ({onde}): não chama marcar_feita antes do commit")),
        }
        // O-01 da revisão: atômico de verdade = uma transação só (um `commit`), salvo as de várias transações
        // declaradas (o aviso as mostra como "confira").
        let commits = texto.matches(".commit()").count();
        if commits != 1 && !crate::operacoes::VARIAS_TRANSACOES.contains(comando) {
            falhas.push(format!("{comando} ({onde}): {commits} commits; a marca só é atômica com um"));
        }
    }
    assert!(falhas.is_empty(), "R0: atendentes sem a marca na própria transação:\n{}", falhas.join("\n"));
}

/// Retrato do que uma venda mexe: vendas, itens, pagamentos, saldo da peça e movimentos de caixa.
fn retrato(s: &mut crate::Servico, peca: &str) -> (i64, i64, i64, i64, i64) {
    let c = &s.banco_para_teste().conn;
    let n = |sql: &str| -> i64 { c.query_row(sql, [], |l| l.get(0)).unwrap() };
    (
        n("SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"),
        n("SELECT COUNT(*) FROM venda_item"),
        n("SELECT COUNT(*) FROM venda_pagamento"),
        crate::estoque::saldo(c, peca).unwrap(),
        n("SELECT COUNT(*) FROM caixa_movimento"),
    )
}

/// Reinício ANTES de gravar: a operação não chegou ao `COMMIT`, nada ficou marcado, e a mesma chave grava
/// normalmente depois (sem um "já feita" falso).
#[test]
fn reinicio_antes_de_gravar_a_mesma_chave_grava_normalmente() {
    let caminho = temp("antes");
    let p = {
        let mut s = crate::Servico::abrir(&caminho).unwrap();
        configurar(&mut s);
        preparar_venda(&mut s)
    };
    let mut s = crate::Servico::abrir(&caminho).unwrap();
    let ana = usuario_ana(&mut s);
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let r = chamar(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r.get("repetida").is_none(), "não é repetição: {r}");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"), 1);
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// A mesma chave repetida não mexe de novo em nada: itens, pagamentos, estoque e caixa ficam como depois
/// da primeira vez.
#[test]
fn repeticao_com_a_mesma_chave_nao_mexe_em_estoque_pagamento_nem_caixa() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1", "2"));
    let depois_da_primeira = retrato(&mut s, &p);
    let r = chamar(&mut s, "venda_concluir", venda(&p, "venda-1", "2"));
    assert_eq!(r["repetida"], true, "{r}");
    assert_eq!(retrato(&mut s, &p), depois_da_primeira, "nada muda na repetição");
}

/// LIMITE CONHECIDO DO R0 (QT-1 §3), caracterizado de propósito: depois de um reinício, a tela e a chave
/// morreram; a pessoa volta pela tela de entrada e relança a MESMA venda com chave NOVA. Para o banco é
/// outra operação: grava outra venda e baixa o estoque de novo. O R0 sozinho não impede isso; o aviso de
/// fechamento inesperado (QT-1b) mostra a venda já gravada antes de a pessoa relançar, e a R1 (0.9.19)
/// guarda a chave com o rascunho. Quando a identidade da intenção estiver protegida, este teste muda.
#[test]
fn limite_r0_relancar_com_chave_nova_depois_do_reinicio_grava_outra_venda() {
    let caminho = temp("limite");
    let (p, antes) = {
        let mut s = crate::Servico::abrir(&caminho).unwrap();
        configurar(&mut s);
        let p = preparar_venda(&mut s);
        ok(&mut s, "venda_concluir", venda(&p, "tela-1-venda", "1"));
        let antes = retrato(&mut s, &p);
        (p, antes)
    };
    let mut s = crate::Servico::abrir(&caminho).unwrap();
    let ana = usuario_ana(&mut s);
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let r = chamar(&mut s, "venda_concluir", venda(&p, "tela-2-venda", "1"));
    assert_eq!(r["ok"], true, "{r}");
    assert!(r.get("repetida").is_none(), "chave nova: não é repetição para o banco: {r}");
    let depois = retrato(&mut s, &p);
    assert_eq!(depois.0, antes.0 + 1, "OUTRA venda gravada (o limite)");
    assert_eq!(depois.3, antes.3 - 1000, "o estoque baixou DE NOVO (o limite)");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

// ------------------------------------------------------------------------------------------------------------
// QT-1b: o aviso de fechamento inesperado (gravada / não concluída / confira), preparado em 14/11/2026.
// ------------------------------------------------------------------------------------------------------------

fn abrir_sem_rede(caminho: &std::path::Path) -> crate::Servico {
    crate::Servico::abrir_para_teste(caminho, false).unwrap()
}

fn entrar(s: &mut crate::Servico, nome: &str, senha: &str) {
    let lista = ok(s, "usuarios_entrada", json!({}));
    let id = lista.as_array().unwrap().iter().find(|u| u["nome"] == nome).unwrap()["id"].as_str().unwrap().to_string();
    ok(s, "entrar", json!({ "id": id, "senha": senha }));
}

fn aviso_de_fechamento(s: &mut crate::Servico) -> Option<Value> {
    let avisos = ok(s, "avisos_do_dia", json!({}));
    avisos
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap_or("").starts_with("fechamento_inesperado"))
        .cloned()
}

/// As linhas do quadro de detalhes, como a tela mostra ("Gravada: Venda · nº 1 · R$ 10,00 · 21:04").
fn linhas(a: &Value) -> Vec<String> {
    a["detalhes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| format!("{}: {}", d["rotulo"].as_str().unwrap(), d["valor"].as_str().unwrap()))
        .collect()
}

fn iniciada(s: &mut crate::Servico, comando: &str, chave: &str) {
    let usuario: String =
        s.banco_para_teste().conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
    let mut op = crate::operacoes::identificar(comando, chave, &usuario, &json!({}));
    op.abertura = crate::operacoes::abertura_atual(&s.banco_para_teste().conn).unwrap();
    crate::operacoes::iniciar(&s.banco_para_teste().conn, &op).unwrap();
}

/// Depois de uma queda, quem entra vê o que FOI GRAVADO (a venda, com número e total), antes de refazer.
#[test]
fn qt1b_depois_da_queda_o_aviso_mostra_o_que_foi_gravado() {
    let caminho = temp("qt1b-gravada");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    assert!(aviso_de_fechamento(&mut s).is_none(), "sem queda, sem aviso");
    // A venda é gravada e o processo morre antes de a tela seguir (sem outra chamada: a tela não a mostrou).
    let venda1 = ok(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
    s.cair_para_teste();

    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    let a = aviso_de_fechamento(&mut s).expect("o aviso aparece depois da queda");
    assert_eq!(a["categoria"], "atencao", "{a}");
    assert_eq!((a["acao"].clone(), a["resolver"].clone()), (json!("Ver vendas do caixa"), json!("Conferi")));
    assert_eq!(a["funcao"], "caixa");
    let l = linhas(&a);
    let numero = venda1["numero"].as_i64().unwrap();
    let total = venda1["total"].as_str().unwrap();
    assert!(
        l.last().unwrap().starts_with(&format!("Gravada: Venda · nº {numero} · R$ {total} · ")),
        "a última gravada é a venda, com número e total: {l:?}"
    );
    assert!(l.iter().all(|x| x.starts_with("Gravada: ")), "nada em curso: {l:?}");
    // Nunca promete o que estava só na tela.
    let descricao = a["descricao"].as_str().unwrap();
    assert!(!descricao.contains("  "), "sem espaços sobrando (a quebra de linha do código): {descricao}");
    assert!(descricao.contains("não foi guardado"), "{descricao}");
    assert!(!descricao.to_lowercase().contains("rascunho salvo") && !descricao.contains("foi salv"), "{descricao}");
    // O aviso vem primeiro (é o que conferir antes de refazer qualquer coisa).
    assert!(ok(&mut s, "avisos_do_dia", json!({}))[0]["id"].as_str().unwrap().starts_with("fechamento_inesperado"));
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// A operação que começou e não chegou ao COMMIT aparece como "não concluída" (nada gravado); a de várias
/// transações (importação de compras), como "confira" (pode ter gravado parte).
#[test]
fn qt1b_em_curso_aparece_como_nao_concluida_ou_confira() {
    let caminho = temp("qt1b-em-curso");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    preparar_venda(&mut s);
    entrar(&mut s, "Ana", "1234");
    iniciada(&mut s, "receber_baixar", "receber-1");
    iniciada(&mut s, "compra_importar", "compra-1");
    s.cair_para_teste();

    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso_de_fechamento(&mut s).unwrap());
    let nao = l.iter().find(|x| x.starts_with("Não concluída: Recebimento · ")).expect("o recebimento");
    assert!(nao.ends_with("nada foi gravado"), "{nao}");
    let confira = l.iter().find(|x| x.starts_with("Confira: Importação de compras · ")).expect("a importação");
    assert!(confira.ends_with("pode ter gravado só uma parte"), "{confira}");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// Encerramento limpo não avisa: pelo `Drop` (o computador) e pelo `backup_ao_fechar` (o fechar da janela; no
/// Android o Qt sai por System.exit logo depois, sem `Drop`).
#[test]
fn qt1b_encerramento_limpo_nao_avisa() {
    // O backup ao fechar usa a vez dos backups: um teste de backup por vez (senão um restaurar em outro teste
    // acha "backup em andamento").
    let _vez = crate::backup::testes::um_por_vez();
    let caminho = temp("qt1b-limpo");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1", "1"));
    drop(s);
    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    assert!(aviso_de_fechamento(&mut s).is_none(), "fechou pelo Drop");
    ok(&mut s, "venda_concluir", venda(&p, "venda-2", "1"));
    chamar(&mut s, "backup_ao_fechar", json!({}));
    s.cair_para_teste();
    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    assert!(aviso_de_fechamento(&mut s).is_none(), "fechou pela janela (backup_ao_fechar) e depois o System.exit");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// O caixa vê as operações dele; o gerente, as de todos. "Conferi" tira o aviso só de quem conferiu, e não
/// grava, refaz nem desfaz operação nenhuma.
#[test]
fn qt1b_conferi_por_pessoa_e_o_caixa_ve_so_as_dele() {
    let caminho = temp("qt1b-pessoas");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(
        &mut s,
        "usuario_salvar",
        json!({ "nome": "Bia", "senha": "mesa-verde", "senha2": "mesa-verde", "ativo": true, "gerente": false,
                "chaveOperacao": "bia-1" }),
    );
    ok(&mut s, "venda_concluir", venda(&p, "venda-ana", "1"));
    ok(&mut s, "sair", json!({}));
    entrar(&mut s, "Bia", "mesa-verde");
    let da_bia = ok(&mut s, "venda_concluir", venda(&p, "venda-bia", "2"));
    s.cair_para_teste();

    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Bia", "mesa-verde");
    let l = linhas(&aviso_de_fechamento(&mut s).expect("a Bia vê o aviso"));
    assert_eq!(l.len(), 1, "só a venda dela: {l:?}");
    assert!(l[0].starts_with(&format!("Gravada: Venda · nº {} · ", da_bia["numero"])), "{l:?}");
    let vendas_antes = contar(&mut s, "SELECT COUNT(*) FROM venda");
    let mudancas = s.banco_para_teste().conn.total_changes();
    ok(&mut s, "aviso_resolver", json!({ "id": "fechamento_inesperado" }));
    assert!(s.banco_para_teste().conn.total_changes() - mudancas <= 2, "só a marca de visto (e o em curso dela)");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM venda"), vendas_antes, "Conferi não mexe em venda");
    assert!(aviso_de_fechamento(&mut s).is_none(), "some para quem conferiu");
    let r = chamar(&mut s, "aviso_resolver", json!({ "id": "receber" }));
    assert_eq!(r["ok"], false, "aviso sem o que conferir: {r}");
    ok(&mut s, "sair", json!({}));

    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso_de_fechamento(&mut s).expect("a Ana (gerente) ainda vê"));
    // A venda da Ana foi seguida de outra chamada (a tela dela a mostrou: F-02); a da Bia, não. O gerente vê a da
    // Bia, mesmo depois do "Conferi" dela.
    assert!(
        l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", da_bia["numero"]))),
        "o gerente vê a venda da Bia: {l:?}"
    );
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// O comando que falhou de forma limpa (recusado, nada gravado) não aparece como "não concluída".
#[test]
fn qt1b_falha_limpa_nao_aparece() {
    let caminho = temp("qt1b-falha");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let mut ruim = venda(&p, "venda-x", "1");
    ruim["pagamentos"] = json!([{ "forma": "dinheiro", "valor": "-1" }]);
    assert_eq!(chamar(&mut s, "venda_concluir", ruim)["ok"], false);
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM operacao_em_curso"), 0, "saiu do em curso");
    s.cair_para_teste();
    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    // Depois da revisão (F-02), o que a tela já mostrou não entra: aqui pode nem haver aviso. Se houver, a venda
    // recusada não está nele.
    let l = aviso_de_fechamento(&mut s).map(|a| linhas(&a)).unwrap_or_default();
    assert!(l.iter().all(|x| !x.contains("Venda")), "a venda recusada não aparece: {l:?}");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// O aviso mostra TODAS as gravadas que a tela não mostrou, na ordem em que aconteceram (N-02 da re-revisão: antes,
/// só as 5 últimas; as outras ficavam sem aviso para sempre).
#[test]
fn qt1b_todas_as_gravadas_sem_confirmacao() {
    let caminho = temp("qt1b-ultimas");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    s.cair_para_teste();
    // Sete aberturas seguidas que caem, cada uma com uma venda que a tela não chegou a mostrar (numa abertura só,
    // cada venda "entrega" a anterior: F-02).
    let mut numeros = Vec::new();
    for i in 0..7 {
        let mut s = abrir_sem_rede(&caminho);
        entrar(&mut s, "Ana", "1234");
        numeros.push(ok(&mut s, "venda_concluir", venda(&p, &format!("venda-{i}"), "1"))["numero"].as_i64().unwrap());
        s.cair_para_teste();
    }
    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso_de_fechamento(&mut s).unwrap());
    // Também a abertura do caixa: foi gravada e o programa caiu antes de a tela seguir (ela não tinha confirmação).
    assert!(l.iter().any(|x| x.starts_with("Gravada: Abertura do caixa")), "{l:?}");
    let vendas: Vec<&String> = l.iter().filter(|x| x.starts_with("Gravada: Venda")).collect();
    assert_eq!(vendas.len(), numeros.len(), "{l:?}");
    for (linha, n) in vendas.iter().zip(&numeros) {
        assert!(linha.starts_with(&format!("Gravada: Venda · nº {n} · ")), "{linha} ≠ nº {n}");
    }
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// Depois de restaurar um backup, as aberturas que vieram na cópia (uma delas "caída", com uma operação em curso)
/// não viram um aviso falso; e a abertura nova, depois da restauração, ainda avisa se cair.
#[test]
fn qt1b_restaurar_nao_traz_aviso_falso() {
    let b = crate::banco::Banco::em_memoria().unwrap();
    let c = &b.conn;
    let n1 = crate::operacoes::abrir_sessao(c).unwrap(); // a abertura da hora do backup (caiu)
    let mut op = crate::operacoes::identificar("receber_baixar", "k", "u1", &json!({}));
    op.abertura = n1;
    crate::operacoes::iniciar(c, &op).unwrap();
    let n2 = crate::operacoes::abrir_sessao(c).unwrap();
    assert!(crate::operacoes::fechamento(c, "u1", false, n2).unwrap().is_some(), "antes: o aviso da cópia");
    crate::operacoes::depois_de_restaurar(c).unwrap();
    let n3 = crate::operacoes::abrir_sessao(c).unwrap(); // quem restaurou abre uma abertura nova
    assert!(crate::operacoes::fechamento(c, "u1", false, n3).unwrap().is_none(), "depois: nada");
    // Uma queda depois da restauração ainda avisa.
    let mut op2 = crate::operacoes::identificar("receber_baixar", "k2", "u1", &json!({}));
    op2.abertura = n3;
    crate::operacoes::iniciar(c, &op2).unwrap();
    let n4 = crate::operacoes::abrir_sessao(c).unwrap();
    assert_eq!(crate::operacoes::fechamento(c, "u1", false, n4).unwrap().map(|x| x.2.len()), Some(1));
}

/// Todo comando que não se repete (menos as exceções do R0) tem nome no aviso; nenhum nome sobra.
#[test]
fn todo_comando_que_nao_se_repete_tem_nome_no_aviso() {
    let nomes: Vec<&str> = crate::avisos::NOMES_DAS_OPERACOES.iter().map(|(c, _)| *c).collect();
    for c in NAO_REPETIR.iter().filter(|c| !EXCECOES_R0.contains(c)) {
        assert!(nomes.contains(c), "{c} sem nome no aviso de fechamento inesperado");
    }
    for c in &nomes {
        assert!(NAO_REPETIR.contains(c), "{c} não é comando que não se repete");
    }
}

/// O que a abertura ANTERIOR gravou não entra no aviso da abertura seguinte, nem no mesmo segundo (o Qt
/// reinicia o processo dentro do mesmo segundo no Android): a janela é em milissegundos.
#[test]
fn qt1b_operacao_da_abertura_anterior_nao_entra_no_aviso() {
    let caminho = temp("qt1b-segundo");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-a", "1"));
    drop(s); // a abertura A fecha direito
    let s = abrir_sem_rede(&caminho); // a abertura B começa (talvez no mesmo segundo da venda)
    s.cair_para_teste(); // e cai sem gravar nada
    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    assert!(aviso_de_fechamento(&mut s).is_none(), "a venda da abertura A não é da abertura que caiu");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// F-04 (revisão): um erro interno (pânico) marca a abertura; mesmo fechando pela janela (`backup_ao_fechar`) e pelo
/// `Drop`, ela não vira "limpa", e a abertura seguinte avisa o que ficou em curso nela.
#[test]
fn rev_f04_panico_marca_a_abertura_e_a_seguinte_avisa() {
    let _vez = crate::backup::testes::um_por_vez();
    let caminho = temp("f04-panico");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    let n = crate::operacoes::abertura_atual(&s.banco_para_teste().conn).unwrap();
    // Uma operação que começou nesta abertura e o erro interno no meio (a transação dela foi desfeita).
    iniciada(&mut s, "receber_baixar", "receber-panico");
    assert_eq!(chamar(&mut s, "__teste_panico", json!({ "onde": "antes_do_commit" }))["ok"], false);
    chamar(&mut s, "backup_ao_fechar", json!({}));
    drop(s);
    let mut s = abrir_sem_rede(&caminho);
    let (limpa, com_erro): (i64, i64) = s
        .banco_para_teste()
        .conn
        .query_row("SELECT limpa, com_erro FROM programa_abertura WHERE numero = ?1", [n], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .unwrap();
    assert_eq!((limpa, com_erro), (0, 1), "a abertura do pânico não vira limpa");
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso_de_fechamento(&mut s).expect("F-04: depois do erro interno, a abertura seguinte avisa"));
    assert!(l.iter().any(|x| x.starts_with("Não concluída: Recebimento")), "{l:?}");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// F-12 (revisão): a hora vem com o dia quando a operação não é de hoje.
#[test]
fn rev_f12_hora_com_dia_quando_nao_e_hoje() {
    let caminho = temp("f12-dia");
    let mut s = abrir_sem_rede(&caminho);
    configurar(&mut s);
    iniciada(&mut s, "receber_baixar", "receber-ontem");
    s.banco_para_teste()
        .conn
        .execute("UPDATE operacao_em_curso SET iniciada_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-2 days')", [])
        .unwrap();
    s.cair_para_teste();
    let mut s = abrir_sem_rede(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso_de_fechamento(&mut s).unwrap());
    let dia: String = s
        .banco_para_teste()
        .conn
        .query_row("SELECT strftime('%d/%m', 'now', '-2 days', 'localtime')", [], |l| l.get(0))
        .unwrap();
    assert!(l.iter().any(|x| x.contains(&format!("· {dia} "))), "o dia {dia} na linha: {l:?}");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}
