//! FALHA-04 (auditoria da 0.9.15): chave da operação.
//!
//! Regra aprovada: a chave identifica UMA tentativa lógica de operação.
//! Repetir a mesma tentativa (duplo clique, Enter duas vezes, de novo depois
//! de um erro de rede) usa a mesma chave e não grava duas vezes; uma operação
//! nova, mesmo com os mesmos dados, tem chave nova e é gravada. Nada de
//! janela de tempo nem de resumo do conteúdo.

use std::path::Path;

use serde_json::{json, Value};

use crate::api::NAO_REPETIR;

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

fn loja() -> crate::Servico {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    s
}

fn contar(s: &mut crate::Servico, sql: &str) -> i64 {
    s.banco_para_teste().conn.query_row(sql, [], |l| l.get(0)).unwrap()
}

/// Duas vendas legítimas iguais, uma logo depois da outra: as duas ficam.
/// A mesma venda mandada duas vezes (mesma chave): uma só.
#[test]
fn duas_vendas_iguais_ficam_e_a_repetida_nao() {
    let mut s = loja();
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("10");
    f["chaveOperacao"] = json!("peca-1");
    let p = ok(&mut s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(&mut s, "caixa_abrir", json!({ "valor": "0", "chaveOperacao": "abrir-1" }));
    let venda = |k: &str| {
        json!({ "itens": [{ "produtoId": p, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "dinheiro", "valor": "100" }], "chaveOperacao": k })
    };
    ok(&mut s, "venda_concluir", venda("venda-1"));
    ok(&mut s, "venda_concluir", venda("venda-2"));
    let r = chamar(&mut s, "venda_concluir", venda("venda-2"));
    assert_eq!(r["campo"], "repetida", "a mesma tentativa de novo: {r}");
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"), 2);
}

/// Os outros comandos críticos: recebimento, pagamento, compra (rascunho
/// lançado), devolução e OS — a mesma chave grava uma vez; chave nova grava.
#[test]
fn comandos_criticos_respeitam_a_chave() {
    let mut s = loja();
    // Crediário: lançar e receber.
    let mut c = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    c["nome"] = json!("Zé");
    c["chaveOperacao"] = json!("cli-1");
    let cliente = ok(&mut s, "pessoa_salvar", c)["id"].as_str().unwrap().to_string();
    let lancar = |k: &str| {
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1,
                "primeiroVencimento": "", "chaveOperacao": k })
    };
    ok(&mut s, "receber_lancar", lancar("cr-1"));
    assert_eq!(chamar(&mut s, "receber_lancar", lancar("cr-1"))["campo"], "repetida");
    ok(&mut s, "receber_lancar", lancar("cr-2"));
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM conta_receber"), 2);
    // Contas a pagar (gerente).
    let pagar = |k: &str| json!({ "descricao": "Aluguel", "valor": "100", "primeiroVencimento": "", "parcelas": 1, "chaveOperacao": k });
    ok(&mut s, "pagar_lancar", pagar("cp-1"));
    assert_eq!(chamar(&mut s, "pagar_lancar", pagar("cp-1"))["campo"], "repetida");
    ok(&mut s, "pagar_lancar", pagar("cp-2"));
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM conta_pagar"), 2);
    // OS.
    let os = |k: &str| {
        json!({ "clienteNome": "Carlos", "placa": "ABC1D23",
                "itens": [{ "tipo": "servico", "descricao": "Revisão", "preco": "50" }], "chaveOperacao": k })
    };
    ok(&mut s, "os_salvar", os("os-1"));
    assert_eq!(chamar(&mut s, "os_salvar", os("os-1"))["campo"], "repetida");
    ok(&mut s, "os_salvar", os("os-2"));
    assert_eq!(contar(&mut s, "SELECT COUNT(*) FROM ordem_servico"), 2);
}

/// Depois que todas as telas mandam a chave, o núcleo exige a chave nos
/// comandos de `NAO_REPETIR`: sem ela, nada é gravado.
#[test]
fn sem_chave_nada_e_gravado() {
    let mut s = loja();
    for c in NAO_REPETIR {
        let antes = s.banco_para_teste().conn.total_changes();
        let r = chamar(&mut s, c, json!({}));
        assert_eq!(r["ok"], false, "{c} sem chave: {r}");
        assert_eq!(r["campo"], "chave", "{c} sem chave: {r}");
        assert_eq!(antes, s.banco_para_teste().conn.total_changes(), "{c} sem chave gravou");
    }
}

// ---------------------------------------------------------------------------
// As telas
// ---------------------------------------------------------------------------

/// Comandos que CRIAM algo (uma venda, um recebimento, um lançamento...):
/// a tela guarda a chave da operação que está montando (renovada quando
/// começa outra), e não uma chave por clique.
const CRIAM: &[&str] = &[
    "venda_concluir",
    "orcamento_salvar",
    "os_salvar",
    "os_rascunho_salvar",
    "devolucao_gravar",
    "caixa_abrir",
    "caixa_fechar",
    "caixa_movimentar",
    "receber_lancar",
    "receber_baixar",
    "encomenda_criar",
    "compra_importar",
    "compra_lancar",
    "pessoa_salvar",
    "produto_salvar",
    "produto_variacao_criar",
    "unidade_salvar",
    "estoque_ajustar",
    "pagar_lancar",
    "pagar_pagar",
    "movimento_lancar",
    "transferir",
    "conta_loja_salvar",
    "categoria_salvar",
    "comissao_fechar",
    "usuario_salvar",
    "importar_cadastros",
];

/// Os caminhos que levam a chave ao núcleo.
const CAMINHOS: &[&str] =
    &["Api.gravar(", "Api.gravarDemorado(", "chamarGerente(", "chamarGerenteDemorado(", "secao.acao("];

fn telas() -> Vec<(String, String)> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../app");
    let mut lista = Vec::new();
    for sub in ["qml", "qml_android", "base"] {
        let Ok(d) = std::fs::read_dir(raiz.join(sub)) else { continue };
        for a in d.flatten() {
            if a.path().extension().is_some_and(|e| e == "qml") {
                lista.push((
                    format!("{sub}/{}", a.file_name().to_string_lossy()),
                    std::fs::read_to_string(a.path()).unwrap(),
                ));
            }
        }
    }
    lista.sort();
    lista
}

/// Os argumentos de uma chamada que começa em `texto` (logo depois do "(").
fn argumentos(texto: &str) -> &str {
    let mut n = 1;
    for (i, ch) in texto.char_indices() {
        match ch {
            '(' | '{' | '[' => n += 1,
            ')' | '}' | ']' => {
                n -= 1;
                if n == 0 {
                    return &texto[..i];
                }
            }
            _ => {}
        }
    }
    texto
}

/// FALHA-04: toda tela que chama um comando de `NAO_REPETIR` manda a chave
/// (pelo `Api.gravar`, pela senha do gerente ou à mão), e os que criam algo
/// mandam a chave da operação da tela.
#[test]
fn telas_mandam_a_chave() {
    let mut problemas = Vec::new();
    let mut achados = 0;
    for (nome, texto) in telas() {
        for c in NAO_REPETIR {
            let alvo = format!("\"{c}\"");
            for (i, _) in texto.match_indices(&alvo) {
                // O que vem antes do nome do comando, na mesma linha.
                let ini = texto[..i].rfind('\n').map_or(0, |p| p + 1);
                let antes = &texto[ini..i];
                let linha = texto[..i].matches('\n').count() + 1;
                if antes.trim_end().ends_with("Api.chamar(") || antes.trim_end().ends_with("Api.chamarDemorado(") {
                    problemas.push(format!("{nome}:{linha}: {c} vai por Api.chamar, sem chave"));
                    continue;
                }
                let Some(caminho) = CAMINHOS.iter().find(|p| antes.trim_end().ends_with(*p)) else { continue };
                achados += 1;
                if CRIAM.contains(c) {
                    let resto = &texto[i + alvo.len()..];
                    let args = argumentos(resto);
                    if !args.contains("chave") && !args.contains("Chave") {
                        problemas.push(format!("{nome}:{linha}: {c} ({caminho}) sem a chave da operação da tela"));
                    }
                }
            }
        }
    }
    assert!(problemas.is_empty(), "telas sem chave:\n{}", problemas.join("\n"));
    assert!(achados >= NAO_REPETIR.len(), "só {achados} chamadas achadas");
    // Todos os comandos têm ao menos um caminho com chave nas telas.
    let tudo: String = telas().into_iter().map(|(_, t)| t).collect();
    for c in NAO_REPETIR {
        assert!(tudo.contains(&format!("\"{c}\"")), "{c} não aparece em tela nenhuma");
    }
    // Os caminhos de fato põem a chave, e não há janela de tempo nem resumo.
    let api = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/base/Api.qml")).unwrap();
    let corpo = |texto: &str, f: &str| -> String {
        texto
            .split(f)
            .nth(1)
            .unwrap_or_else(|| panic!("falta {f}"))
            .split("\n    function ")
            .next()
            .unwrap()
            .to_string()
    };
    let com_chave = corpo(&api, "function comChave(");
    assert!(com_chave.contains("chaveOperacao") && com_chave.contains("novaChave()"), "{com_chave}");
    for f in ["function gravar(", "function gravarDemorado(", "function comChave("] {
        let c = corpo(&api, f);
        assert!(f.contains("comChave") || c.contains("comChave("), "{f} usa a chave: {c}");
        assert!(
            !c.contains("Date.now()") && !c.contains("corpo") && !c.contains("_ultimas"),
            "{f} sem janela de tempo nem resumo: {c}"
        );
    }
    let main = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/qml/Main.qml")).unwrap();
    for f in ["function chamarGerente(", "function chamarGerenteDemorado("] {
        let c = corpo(&main, f);
        assert!(c.contains("dados = Api.comChave(dados, chave)"), "{f} manda a chave: {c}");
    }
    let enc =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/qml/SecaoEncomendas.qml")).unwrap();
    let acao = enc.split("function acao(").nth(1).expect("secao.acao").split("\n    function ").next().unwrap();
    assert!(acao.contains("Api.gravar("), "secao.acao grava com chave: {acao}");
}
