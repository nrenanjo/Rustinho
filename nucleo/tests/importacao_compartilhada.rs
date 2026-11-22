//! T108 (texto R do proprietário, 21/11/2026): a importação de XML compartilhada entre Peças e Compras.
//!
//! "Importou por Peças: processar também as informações pertinentes a Compras. Importou por Compras: processar
//! também as informações pertinentes a Peças. Evitar registros duplicados caso o mesmo XML seja selecionado
//! novamente. Preservar as regras e finalidades próprias de cada módulo."
//!
//! - **Por Peças** ("Trazer cadastros pelo XML"): a nota de compra que serve para a entrada (as regras de Compras:
//!   autorizada, para o CNPJ da loja, de venda do fornecedor, normal) e ainda não está em Compras vira o rascunho da
//!   compra, com a opção "Notas de compra em Compras" da prévia. O estoque, o custo e o A pagar não mudam: só no
//!   lançar, como sempre.
//! - **Por Compras**: o que é de Peças segue o caminho de antes, sem peça nascendo sem a decisão da pessoa (a revisão
//!   da T108 reprovou o cadastro automático: peça repetida). O fornecedor é cadastrado e o item sem peça vai para as
//!   pendências que a tela de Peças mostra ("associar" ou "cadastrar nova"); resolvida a pendência, o item da compra
//!   se liga sozinho.
//! - **Sem duplicar**: a compra pela chave de acesso; os cadastros só cadastram o que falta. Num pacote (pasta,
//!   .zip, .rar), a nota que já estava em Compras é contada, não é erro; o XML sozinho repetido continua recusado.
//!
//! Tudo com a nota de exemplo (assinada pela PKI de teste), em pastas temporárias.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use comum::*;
use serde_json::{json, Value};

/// CNPJ da loja de teste: o destinatário da nota de exemplo.
const LOJA_CNPJ: &str = "11444777000161";
/// Um CNPJ válido que não é o destinatário da nota de exemplo.
const OUTRO_CNPJ: &str = "12345678000195";
/// Emitente da nota de exemplo.
const FORNECEDOR: &str = "11222333000181";
const EXEMPLO: &str = include_str!("nfe_exemplo.xml");

fn loja(nome: &str, cnpj: &str) -> Loja {
    let l = Loja::nova(nome);
    l.executar(&format!("INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '{cnpj}')"));
    l
}

/// Uma pasta com a nota de exemplo numa subpasta (`copias` arquivos com a mesma nota).
fn pasta(l: &Loja, nome: &str, copias: usize) -> String {
    let p = l.pasta.join(nome);
    let sub = p.join("notas").join("2026");
    std::fs::create_dir_all(&sub).unwrap();
    for i in 0..copias {
        std::fs::write(sub.join(format!("compra-{i}.xml")), EXEMPLO).unwrap();
    }
    p.display().to_string()
}

/// O arquivo sozinho da nota de exemplo.
fn arquivo(l: &Loja, nome: &str) -> String {
    let p = l.pasta.join(nome);
    std::fs::create_dir_all(&p).unwrap();
    let a = p.join("compra.xml");
    std::fs::write(&a, EXEMPLO).unwrap();
    a.display().to_string()
}

/// O "Cadastrar" do "Trazer cadastros" com todas as opções; `compras` é a opção nova.
fn cadastrar(l: &mut Loja, caminho: &str, compras: bool) -> Value {
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [caminho], "clientes": true, "fornecedores": true, "pecasLoja": true,
                "pecasFornecedor": true, "compras": compras }),
    )
}

fn n(l: &Loja, sql: &str) -> i64 {
    l.numero(sql)
}

/// O que a compra não pode ter feito antes do lançar: estoque, custo lançado e A pagar.
fn nada_no_estoque_nem_no_financeiro(l: &Loja) {
    assert_eq!(n(l, "SELECT COUNT(*) FROM estoque_movimento"), 0, "a importação mexeu no estoque");
    assert_eq!(n(l, "SELECT COUNT(*) FROM conta_pagar"), 0, "a importação criou contas a pagar");
    assert_eq!(n(l, "SELECT COUNT(*) FROM compra WHERE situacao <> 'rascunho'"), 0, "a compra não ficou em rascunho");
}

/// Os itens da compra e como cada um foi ligado (acao, achado_por, com peça?).
fn itens(l: &Loja) -> Vec<(String, String, bool)> {
    let c = l.conexao();
    let mut s = c.prepare("SELECT acao, achado_por, produto_id IS NOT NULL FROM compra_item ORDER BY ordem").unwrap();
    s.query_map([], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?))).unwrap().collect::<Result<_, _>>().unwrap()
}

// ---------------------------------------------------------------------------
// Por Peças
// ---------------------------------------------------------------------------

/// A prévia conta a nota de compra que vai para Compras (e a que já está lá), com a categoria do resumo.
#[test]
fn a_previa_de_pecas_conta_as_notas_de_compra() {
    let mut l = loja("t108-previa", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
    assert_eq!((p["comprasNovas"].as_i64(), p["comprasJa"].as_i64()), (Some(1), Some(0)), "{p}");
    assert_eq!(p["opcoes"]["compras"], true, "a prévia sem opções supõe todas (P-5): {p}");
    let cat = |p: &Value, c: &str| {
        p["categorias"].as_array().unwrap().iter().find(|x| x["codigo"] == c).map(|x| x["quantidade"].clone())
    };
    assert_eq!(cat(&p, "compra.nova"), Some(json!(1)), "{p}");
    // Desmarcada: a nota não vai, e o resumo diz.
    let p2 = l.ok(
        "importar_previa",
        json!({ "arquivos": [pasta], "clientes": true, "fornecedores": true, "pecasLoja": true,
                "pecasFornecedor": true, "compras": false }),
    );
    assert_eq!(cat(&p2, "compra.nao_importada"), Some(json!(1)), "{p2}");
    assert_eq!(cat(&p2, "compra.nova"), None, "{p2}");
    assert_ne!(p["carimbo"], p2["carimbo"], "a opção muda o carimbo (P-4)");
    // A prévia não grava nada.
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
}

/// Importou por Peças: a nota de compra vira o rascunho em Compras, com os itens já ligados às peças que a mesma
/// importação cadastrou. O estoque e o A pagar não mudam.
#[test]
fn importou_por_pecas_a_nota_vai_para_compras_em_rascunho() {
    let mut l = loja("t108-pecas", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r["compras"], 1, "{r}");
    assert_eq!(r["pecas"], 3, "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra WHERE situacao = 'rascunho' AND autorizacao = 'aguardando'"), 1);
    let chave: String = l.conexao().query_row("SELECT chave FROM compra", [], |x| x.get(0)).unwrap();
    assert!(EXEMPLO.contains(&chave), "a compra é a da nota");
    assert_eq!(
        itens(&l),
        vec![
            ("ligar".to_string(), "fornecedor".to_string(), true),
            ("ligar".to_string(), "fornecedor".to_string(), true),
            ("ligar".to_string(), "fornecedor".to_string(), true),
        ],
        "os itens nascem ligados às peças cadastradas"
    );
    nada_no_estoque_nem_no_financeiro(&l);
    // A compra abre e se lança como qualquer outra (a regra de Compras: a conferência do gerente antes).
    let id: String = l.conexao().query_row("SELECT id FROM compra", [], |x| x.get(0)).unwrap();
    let c = l.ok("compra_abrir", json!({ "id": id }));
    assert_eq!(c["situacao"], "rascunho", "{c}");
    assert!(
        l.erro("compra_lancar", json!({ "id": id, "itens": [] })).contains("SEFAZ"),
        "sem a conferência, não lança"
    );
}

/// Sem a opção (e para quem não manda a opção, como antes): nada vai para Compras, e a resposta é a de antes.
#[test]
fn pecas_sem_a_opcao_nao_mexe_em_compras() {
    let mut l = loja("t108-sem-opcao", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let r = l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta.clone()], "clientes": true, "fornecedores": true, "pecasLoja": true,
                "pecasFornecedor": true }),
    );
    assert_eq!(r, json!({ "clientes": 0, "fornecedores": 1, "pecas": 3, "codigosLigados": 0 }), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
    let r2 = cadastrar(&mut l, &pasta, false);
    assert_eq!(r2, json!({ "clientes": 0, "fornecedores": 0, "pecas": 0, "codigosLigados": 0 }), "{r2}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
}

/// A mesma nota de novo (e duas cópias dela na pasta): uma compra só, e as peças não se repetem.
#[test]
fn pecas_de_novo_nao_duplica_nada() {
    let mut l = loja("t108-pecas-de-novo", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 2);
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r["compras"], 1, "duas cópias da mesma nota: uma compra: {r}");
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
    assert_eq!((p["comprasNovas"].as_i64(), p["comprasJa"].as_i64()), (Some(0), Some(1)), "{p}");
    let r2 = cadastrar(&mut l, &pasta, true);
    assert_eq!(r2, json!({ "clientes": 0, "fornecedores": 0, "pecas": 0, "codigosLigados": 0 }), "{r2}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
}

/// A nota para outro CNPJ não serve para Compras (a regra de Compras): só os cadastros, como antes; nenhuma compra
/// e nenhum conflito.
#[test]
fn pecas_nao_leva_para_compras_a_nota_de_outro_cnpj() {
    let mut l = loja("t108-outro-cnpj", OUTRO_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
    assert_eq!((p["comprasNovas"].as_i64(), p["comprasJa"].as_i64()), (Some(0), Some(0)), "{p}");
    // A8 da revisão da T108: a que não serve para Compras não some sem aviso.
    assert_eq!(p["comprasNaoServe"].as_i64(), Some(1), "{p}");
    let cat = p["categorias"].as_array().unwrap().iter().find(|x| x["codigo"] == "compra.nao_serve").cloned();
    assert_eq!(cat.map(|c| c["quantidade"].clone()), Some(json!(1)), "{p}");
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r, json!({ "clientes": 0, "fornecedores": 1, "pecas": 3, "codigosLigados": 0 }), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
}

// ---------------------------------------------------------------------------
// Por Compras
// ---------------------------------------------------------------------------

/// Importou por Compras: o fornecedor é cadastrado, e os itens sem peça vão para as pendências de Peças (nenhuma
/// peça nasce sem a decisão da pessoa). Resolvida a pendência em Peças, o item da compra se liga sozinho. O estoque e
/// o A pagar não mudam.
#[test]
fn importou_por_compras_o_que_e_de_pecas_vai_para_as_pendencias() {
    let mut l = loja("t108-compras", LOJA_CNPJ);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(n(&l, &format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{FORNECEDOR}'")), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0, "nenhuma peça nasce sem a decisão da pessoa");
    let pend = l.ok("xml_pendencias_listar", json!({}));
    assert_eq!(pend.as_array().unwrap().len(), 3, "os 3 itens estão nas pendências de Peças: {pend}");
    assert!(itens(&l).iter().all(|(acao, _, peca)| acao.is_empty() && !*peca), "{:?}", itens(&l));
    nada_no_estoque_nem_no_financeiro(&l);
    // Em Peças, a pessoa cadastra a peça nova de uma pendência: o item da compra se liga sozinho.
    let id_pend = pend[0]["id"].as_str().unwrap().to_string();
    l.ok("xml_pendencia_resolver", json!({ "id": id_pend, "acao": "nova", "produtoId": "" }));
    let id: String = l.conexao().query_row("SELECT id FROM compra", [], |x| x.get(0)).unwrap();
    let c = l.ok("compra_abrir", json!({ "id": id }));
    let ligados = c["itens"].as_array().unwrap().iter().filter(|it| it["acao"] == "ligar").count();
    assert_eq!(ligados, 1, "o item da pendência resolvida se ligou: {c}");
}

/// A2 da revisão da T108: o caixa importa a compra como antes, e nenhuma peça nasce na importação.
#[test]
fn o_caixa_importa_e_nenhuma_peca_nasce_sem_decisao() {
    let mut l = loja("t108-caixa", LOJA_CNPJ);
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let pasta = pasta(&l, "xml", 1);
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0);
}

/// A mesma pasta de novo: num pacote, a nota que já estava em Compras é contada, não é erro; nada se repete.
#[test]
fn compras_pasta_de_novo_conta_a_nota_que_ja_estava() {
    let mut l = loja("t108-compras-de-novo", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let r = l.ok("compra_importar", json!({ "caminho": pasta.clone() }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    let r2 = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!(r2["erros"], json!([]), "a nota repetida no pacote não é erro: {r2}");
    assert_eq!(r2["jaEmCompras"], 1, "{r2}");
    assert_eq!(r2["compras"], json!([]), "{r2}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM xml_pendencia_identificacao"), 3, "as pendências não se repetem");
}

/// Duas cópias da mesma nota na mesma pasta: uma compra só; a outra é contada como já em Compras.
#[test]
fn compras_duas_copias_na_pasta_uma_compra() {
    let mut l = loja("t108-compras-copias", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 2);
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!((r["jaEmCompras"].clone(), r["erros"].clone()), (json!(1), json!([])), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
}

/// O XML sozinho repetido continua recusado com a data em que entrou (como antes).
#[test]
fn compras_xml_sozinho_repetido_continua_recusado() {
    let mut l = loja("t108-compras-sozinho", LOJA_CNPJ);
    let a = arquivo(&l, "compra");
    l.ok("compra_importar", json!({ "caminho": a.clone() }));
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    assert!(r["erros"][0].as_str().unwrap_or_default().contains("já foi importada"), "{r}");
    assert!(r.get("jaEmCompras").is_none(), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
}

/// A nota para outro CNPJ continua na janela de conflito, e nada dela é cadastrado.
#[test]
fn compras_nota_de_outro_cnpj_nao_cadastra_nada() {
    let mut l = loja("t108-compras-outro", OUTRO_CNPJ);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    assert_eq!(r["conflitos"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM pessoa WHERE documento = '11222333000181'"), 0);
}

// ---------------------------------------------------------------------------
// Uma depois da outra
// ---------------------------------------------------------------------------

/// Por Compras e depois por Peças (a mesma pasta): a nota não entra de novo em Compras; os cadastros de Peças
/// resolvem as pendências, e os itens da compra se ligam às peças.
#[test]
fn compras_e_depois_pecas_nao_duplica() {
    let mut l = loja("t108-compras-pecas", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    l.ok("compra_importar", json!({ "caminho": pasta.clone() }));
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
    assert_eq!((p["comprasNovas"].as_i64(), p["comprasJa"].as_i64()), (Some(0), Some(1)), "{p}");
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r, json!({ "clientes": 0, "fornecedores": 0, "pecas": 3, "codigosLigados": 0 }), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'"), 0);
    let id: String = l.conexao().query_row("SELECT id FROM compra", [], |x| x.get(0)).unwrap();
    let c = l.ok("compra_abrir", json!({ "id": id }));
    assert!(c["itens"].as_array().unwrap().iter().all(|it| it["acao"] == "ligar"), "os itens se ligaram: {c}");
    let r2 = cadastrar(&mut l, &pasta, true);
    assert_eq!(r2, json!({ "clientes": 0, "fornecedores": 0, "pecas": 0, "codigosLigados": 0 }), "{r2}");
}

/// Por Peças e depois por Compras (a mesma pasta): nada se repete; a nota é contada como já em Compras.
#[test]
fn pecas_e_depois_compras_nao_duplica() {
    let mut l = loja("t108-pecas-compras", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    cadastrar(&mut l, &pasta, true);
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!((r["jaEmCompras"].clone(), r["erros"].clone()), (json!(1), json!([])), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
}

/// A compra que veio por Peças se lança como as outras: o estoque entra nas peças que a importação cadastrou (sem
/// peça repetida), e o A pagar nasce no lançar.
#[test]
fn a_compra_que_veio_por_pecas_se_lanca_nas_mesmas_pecas() {
    let mut l = loja("t108-lancar", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    cadastrar(&mut l, &pasta, true);
    let id: String = l.conexao().query_row("SELECT id FROM compra", [], |x| x.get(0)).unwrap();
    let c = l.ok("compra_abrir", json!({ "id": id }));
    let escolhas: Vec<Value> = c["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            json!({ "id": it["id"], "acao": it["acao"], "produtoId": it["peca"]["id"], "fator": "1",
                          "baixarPreco": false })
        })
        .collect();
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": id, "situacao": "autorizada", "protocolo": "135260000000001",
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    // Como a tela manda (o "Gerar contas a pagar" vem marcado).
    l.ok("compra_lancar", json!({ "id": id, "itens": escolhas, "gerarPagar": true }));
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "nenhuma peça repetida no lançar");
    assert_eq!(n(&l, "SELECT COUNT(DISTINCT produto_id) FROM estoque_movimento"), 3);
    assert!(n(&l, "SELECT COUNT(*) FROM conta_pagar") >= 1, "o A pagar nasce no lançar");
}

/// A3 da revisão da T108 (FI-1): "Notas de compra em Compras" exige "Fornecedores"; sem ela, nada é gravado.
#[test]
fn compras_sem_fornecedores_e_recusada() {
    let mut l = loja("t108-fi1", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let e = l.erro(
        "importar_cadastros",
        json!({ "arquivos": [pasta], "clientes": false, "fornecedores": false, "pecasLoja": false,
                "pecasFornecedor": false, "compras": true }),
    );
    assert!(e.contains("Fornecedores"), "{e}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
    assert_eq!(n(&l, &format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{FORNECEDOR}'")), 0);
}

/// A11 da revisão da T108: se a compra cair no meio do "Trazer cadastros", nada é gravado (nem os cadastros).
#[test]
fn queda_na_compra_desfaz_o_trazer_cadastros_inteiro() {
    let mut l = loja("t108-queda", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    l.executar(
        "CREATE TRIGGER falha_de_teste BEFORE INSERT ON compra_item
           BEGIN SELECT RAISE(ABORT, 'falha de teste no meio'); END",
    );
    l.erro(
        "importar_cadastros",
        json!({ "arquivos": [pasta.clone()], "clientes": true, "fornecedores": true, "pecasLoja": true,
                "pecasFornecedor": true, "compras": true }),
    );
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0, "os cadastros voltaram junto");
    assert_eq!(n(&l, &format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{FORNECEDOR}'")), 0);
    l.executar("DROP TRIGGER falha_de_teste");
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!((r["compras"].clone(), r["pecas"].clone()), (json!(1), json!(3)), "de novo, tudo entra: {r}");
}
