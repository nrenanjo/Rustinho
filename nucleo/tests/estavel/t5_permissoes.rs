//! Prioridade 1 — Permissões reais de cada perfil: o que a tela esconde
//! também tem que ser recusado quando a função é chamada direto (atalho de
//! teclado, tela aberta por outro caminho, chamada sem passar pela tela).

use std::collections::BTreeSet;

use rustinho_nucleo::api;
use serde_json::{json, Value};

use crate::comum::*;

/// Todos os comandos que a API atende (lidos do próprio `api.rs`, como o
/// teste `todo_comando_tem_permissao`).
fn comandos() -> Vec<String> {
    let fonte = include_str!("../../src/api.rs");
    let fim = fonte.find("#[cfg(test)]\nmod testes").unwrap();
    let mut v = BTreeSet::new();
    for linha in fonte[..fim].lines() {
        let t = linha.trim_start();
        if !(linha.starts_with("            \"") || linha.starts_with("        | \"")) || !t.contains('"') {
            continue;
        }
        if !(t.ends_with("=> {") || t.contains("\" =>") || t.starts_with("| \"")) {
            continue;
        }
        for parte in t.split('|') {
            if let Some(nome) = parte.trim().strip_prefix('"').and_then(|r| r.split('"').next()) {
                if nome.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !nome.starts_with("__") {
                    v.insert(nome.to_string());
                }
            }
        }
    }
    assert!(v.len() > 130);
    v.into_iter().collect()
}

/// Comandos que só o gerente (ou quem ele liberou) pode usar.
fn do_gerente() -> Vec<String> {
    comandos().into_iter().filter(|c| !matches!(api::classe_do_comando(c), Some("leitura") | Some("caixa"))).collect()
}

fn loja_com_bia() -> (Loja, Povo) {
    let mut l = Loja::nova("t5");
    let p = povoar(&mut l);
    l.entrar("Bia", SENHA_BIA);
    l.abrir_caixa();
    (l, p)
}

/// Dados que fazem cada comando passar da conferência de permissão e chegar
/// a gravar, se a permissão falhasse (ids reais da loja de teste).
fn dados_para(c: &str, p: &Povo) -> Value {
    match c {
        "venda_cancelar" => json!({ "id": p.v2, "motivo": "teste" }),
        "compra_estornar" => json!({ "id": p.compra, "motivo": "teste" }),
        "estoque_ajustar" => json!({ "produtoId": p.filtro, "quantidade": "99", "motivo": "teste" }),
        "usuario_salvar" => {
            json!({ "nome": "Intrusa", "senha": "abcd", "senha2": "abcd", "ativo": true, "gerente": true })
        }
        "pagar_lancar" => json!({ "categoriaId": "cat-aluguel", "descricao": "x", "valor": "10", "parcelas": 1 }),
        "movimento_lancar" => {
            json!({ "contaId": "caixa-geral", "tipo": "saida", "valor": "5", "categoriaId": "cat-energia", "descricao": "x" })
        }
        "dre_meta_salvar" => json!({ "periodo": "mes", "indicador": "vendas", "vendedorId": "", "valor": "5" }),
        "gerente_salvar" => {
            json!({ "livres": ["cancelar_venda", "estornar", "ajustar_estoque", "backup", "configuracoes", "usuarios", "financeiro", "resultados"] })
        }
        _ => json!({}),
    }
}

/// T5.1 — Cada comando do gerente, chamado direto por um caixa, é recusado
/// com a mensagem do gerente, e nada muda no banco.
#[test]
fn t5_1_caixa_chamando_direto_cada_funcao_do_gerente() {
    let (mut l, p) = loja_com_bia();
    let antes = foto_do_negocio(&l.conexao());
    let lista = do_gerente();
    assert!(lista.len() > 50, "{}", lista.len());
    let mut passaram = Vec::new();
    for c in &lista {
        let r = l.chamar(c, dados_para(c, &p));
        let msg = r["erro"].as_str().unwrap_or_default();
        if r["ok"] != false || !msg.contains("Só o gerente") {
            passaram.push(format!("{c}: {r}"));
        }
    }
    assert!(passaram.is_empty(), "o caixa conseguiu usar funções do gerente:\n{}", passaram.join("\n"));
    let d = diferencas(&antes, &foto_do_negocio(&l.conexao()));
    assert!(d.is_empty(), "algo mudou: {d:?}");
}

/// T5.2 — Sem ninguém conectado, nada além da entrada funciona.
#[test]
fn t5_2_sem_entrar_nada_funciona() {
    let (mut l, p) = loja_com_bia();
    l.ok("sair", json!({}));
    let antes = foto_do_negocio(&l.conexao());
    let mut passaram = Vec::new();
    for c in comandos() {
        if api::SEM_SESSAO.contains(&c.as_str()) {
            continue;
        }
        let r = l.chamar(&c, dados_para(&c, &p));
        if r["ok"] != false || !r["erro"].as_str().unwrap_or_default().contains("Entre") {
            passaram.push(format!("{c}: {r}"));
        }
    }
    assert!(passaram.is_empty(), "funcionou sem entrar:\n{}", passaram.join("\n"));
    assert!(diferencas(&antes, &foto_do_negocio(&l.conexao())).is_empty());
}

/// T5.3 — O que o caixa lê não traz lucro, custos de resultado nem dados do
/// financeiro (D-01 a D-04).
#[test]
fn t5_3_leituras_do_caixa_sem_resultados_nem_financeiro() {
    let (mut l, _p) = loja_com_bia();
    let mv = l.ok("estoque_mais_vendidos", json!({ "dias": 30, "ordem": "lucro", "texto": "" }));
    assert_eq!(mv["lucro"], "", "D-01 total: {mv}");
    for it in mv["itens"].as_array().unwrap() {
        assert_eq!((it["lucro"].as_str(), it["margem"].as_str()), (Some(""), Some("")), "D-01: {it}");
    }
    let contas = l.ok("contas_loja", json!({}));
    for c in contas.as_array().unwrap() {
        assert!(c.get("saldo").is_none_or(|s| s == ""), "D-02: {c}");
        assert!(c.get("saldoInicial").is_none_or(|s| s == ""), "D-02: {c}");
        assert!(c["nome"].is_string(), "o nome continua (a tela do caixa usa): {c}");
    }
    let avisos = l.ok("avisos_do_dia", json!({}));
    for a in avisos.as_array().unwrap() {
        assert_ne!(a["funcao"], "financeiro", "D-03: {a}");
    }
    let r = l.ok("painel_resumo", json!({}));
    assert_eq!((r["pagar7Dias"].as_str(), r["pagarVencido"].as_str()), (Some(""), Some("")), "D-04: {r}");
    assert_eq!(r["financeiro"], false, "D-04: a tela sabe que não pode mostrar: {r}");
}

/// T5.4 — O gerente vê tudo isso (as correções não esconderam do gerente).
#[test]
fn t5_4_gerente_continua_vendo() {
    let (mut l, _p) = loja_com_bia();
    l.entrar("Ana", SENHA_ANA);
    let mv = l.ok("estoque_mais_vendidos", json!({ "dias": 30, "ordem": "valor", "texto": "" }));
    assert_ne!(mv["lucro"], "", "{mv}");
    let contas = l.ok("contas_loja", json!({}));
    assert!(contas[0]["saldo"].is_string() && contas[0]["saldo"] != "", "{contas}");
    let r = l.ok("painel_resumo", json!({}));
    assert_ne!(r["pagar7Dias"], "", "{r}");
    assert_eq!(r["financeiro"], true);
}

/// T5.5 — A senha do gerente no aperto: uma ação só (curta) vale para aquilo
/// e acaba ao encerrar; senha errada não libera e muitas erradas fazem esperar.
#[test]
fn t5_5_liberacao_do_gerente() {
    let (mut l, p) = loja_com_bia();
    for _ in 0..5 {
        l.erro(
            "gerente_liberar",
            json!({ "senha": "errada", "comando": "venda_cancelar",
                      "dados": { "id": p.v2, "motivo": "o gerente liberou" } }),
        );
    }
    let m = l.erro(
        "gerente_liberar",
        json!({ "senha": SENHA_ANA, "comando": "venda_cancelar",
                      "dados": { "id": p.v2, "motivo": "o gerente liberou" } }),
    );
    assert!(m.contains("Espere"), "depois de 5 erradas, espera: {m}");
    l.executar("DELETE FROM entrada_falha");
    l.ok(
        "gerente_liberar",
        json!({ "senha": SENHA_ANA, "comando": "venda_cancelar",
                      "dados": { "id": p.v2, "motivo": "o gerente liberou" } }),
    );
    l.ok("venda_cancelar", json!({ "id": p.v2, "motivo": "o gerente liberou" }));
    l.ok("gerente_encerrar", json!({}));
    let m = l.erro("estoque_ajustar", json!({ "produtoId": p.filtro, "quantidade": "1", "motivo": "x" }));
    assert!(m.contains("Só o gerente"), "{m}");
    // A liberação fica na auditoria, com quem liberou para quem.
    let n = l.numero("SELECT COUNT(*) FROM auditoria WHERE acao = 'gerente.liberar'");
    assert_eq!(n, 1);
}

/// T5.6 — Gerente que deixa de ser gerente (mudado em outro caixa) perde o
/// acesso na hora; usuário desativado em outro caixa não continua usando.
#[test]
fn t5_6_mudanca_feita_em_outro_caixa_vale_na_hora() {
    let mut l = Loja::nova("t5-6");
    let p = povoar(&mut l);
    l.ok(
        "usuario_salvar",
        json!({ "nome": "Cida", "senha": "4321", "senha2": "4321", "ativo": true, "gerente": true }),
    );
    l.entrar("Cida", "4321");
    l.ok("estoque_ajustar", json!({ "produtoId": p.filtro, "quantidade": "5", "motivo": "contagem" }));
    // Outro caixa tirou o "gerente" da Cida (chega pela troca de dados).
    l.executar("UPDATE usuario SET gerente = 0 WHERE nome = 'Cida'");
    let m = l.erro("estoque_ajustar", json!({ "produtoId": p.filtro, "quantidade": "6", "motivo": "contagem" }));
    assert!(m.contains("Só o gerente"), "{m}");
    // Outro caixa desativou a Bia enquanto ela estava usando.
    l.entrar("Bia", SENHA_BIA);
    l.executar("UPDATE usuario SET ativo = 0 WHERE nome = 'Bia'");
    let r = l.chamar("produtos_buscar", json!({ "texto": "" }));
    assert_eq!(r["ok"], false, "usuário desativado continuou usando: {r}");
}
