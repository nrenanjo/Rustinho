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
//! - **Por Compras**: o item que a ligação automática de Compras não acha de jeito nenhum (nem candidata, nem dúvida)
//!   ganha a peça cadastrada já na importação, como o "Cadastrar nova" faria, com o código do fornecedor ligado; o
//!   item fica "nova", com a peça, e o lançar a usa (não cadastra outra). Os itens que acham peça ou ficam em dúvida
//!   seguem as regras de antes (os testes `pend40` e `t7_3` travam isso).
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
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r, json!({ "clientes": 0, "fornecedores": 1, "pecas": 3, "codigosLigados": 0 }), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 0);
}

// ---------------------------------------------------------------------------
// Por Compras
// ---------------------------------------------------------------------------

/// Importou por Compras: as peças que a Compras não acha vão para Peças já na importação, com o código do
/// fornecedor ligado; os itens ficam "nova", com a peça. O estoque e o A pagar não mudam.
#[test]
fn importou_por_compras_as_pecas_vao_para_pecas() {
    let mut l = loja("t108-compras", LOJA_CNPJ);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["pecasCadastradas"], 3, "{r}");
    assert_eq!(n(&l, &format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{FORNECEDOR}'")), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
    assert_eq!(
        n(&l, &format!(
            "SELECT COUNT(*) FROM produto_fornecedor pf JOIN pessoa p ON p.id = pf.fornecedor_id WHERE p.documento = '{FORNECEDOR}'"
        )),
        3,
        "os códigos do fornecedor ficam ligados às peças"
    );
    assert!(
        itens(&l).iter().all(|(acao, por, peca)| acao == "nova" && por == "importacao" && *peca),
        "{:?}",
        itens(&l)
    );
    assert_eq!(n(&l, "SELECT COUNT(*) FROM xml_pendencia_identificacao"), 0, "nada fica para identificar");
    nada_no_estoque_nem_no_financeiro(&l);
}

/// O caixa (não gerente) importa a compra como antes, e os cadastros vêm junto: cadastrar peça já é do caixa (o
/// "nova" do lançar e a tela de Peças).
#[test]
fn o_caixa_importa_e_os_cadastros_vem_junto() {
    let mut l = loja("t108-caixa", LOJA_CNPJ);
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    assert_eq!(r["pecasCadastradas"], 3, "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
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
    assert!(r2.get("pecasCadastradas").is_none(), "nada novo para cadastrar: {r2}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
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
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
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
    assert!(r.get("pecasCadastradas").is_none(), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM pessoa WHERE documento = '11222333000181'"), 0);
}

// ---------------------------------------------------------------------------
// Uma depois da outra
// ---------------------------------------------------------------------------

/// Por Compras e depois por Peças (a mesma pasta): nada se repete; a prévia diz que a nota já estava em Compras.
#[test]
fn compras_e_depois_pecas_nao_duplica() {
    let mut l = loja("t108-compras-pecas", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    l.ok("compra_importar", json!({ "caminho": pasta.clone() }));
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
    assert_eq!((p["comprasNovas"].as_i64(), p["comprasJa"].as_i64()), (Some(0), Some(1)), "{p}");
    assert_eq!(p["pecasFornecedorNovas"].as_i64(), Some(0), "as peças já vieram por Compras: {p}");
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r, json!({ "clientes": 0, "fornecedores": 0, "pecas": 0, "codigosLigados": 0 }), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
}

/// Por Peças e depois por Compras (a mesma pasta): nada se repete; a nota é contada como já em Compras.
#[test]
fn pecas_e_depois_compras_nao_duplica() {
    let mut l = loja("t108-pecas-compras", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    cadastrar(&mut l, &pasta, true);
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!((r["jaEmCompras"].clone(), r["erros"].clone()), (json!(1), json!([])), "{r}");
    assert!(r.get("pecasCadastradas").is_none(), "{r}");
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

/// O lançar usa a peça que a importação cadastrou, também quando a tela manda "Cadastrar nova" sem a peça (o
/// "nova" de sempre): nenhuma peça repetida; o custo leva o frete pago à parte.
fn conferir_e_lancar(l: &mut Loja, id: &str, frete_extra: &str, atualizar_precos: bool) -> Value {
    let c = l.ok("compra_abrir", json!({ "id": id }));
    let escolhas: Vec<Value> = c["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| json!({ "id": it["id"], "acao": "nova", "produtoId": "", "fator": "1", "baixarPreco": false }))
        .collect();
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": id, "situacao": "autorizada", "protocolo": "135260000000001",
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    l.ok(
        "compra_lancar",
        json!({ "id": id, "itens": escolhas, "freteExtra": frete_extra, "atualizarPrecos": atualizar_precos }),
    )
}

#[test]
fn o_lancar_usa_a_peca_que_a_importacao_cadastrou() {
    let mut l = loja("t108-lancar-nova", LOJA_CNPJ);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    let id = r["compras"][0]["id"].as_str().unwrap().to_string();
    let custo_antes: i64 = l.numero("SELECT SUM(custo) FROM produto");
    let lancou = conferir_e_lancar(&mut l, &id, "90,00", true);
    assert_eq!(lancou["resumo"]["pecasNovas"], 3, "{lancou}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "nenhuma peça repetida no lançar");
    assert_eq!(n(&l, "SELECT COUNT(DISTINCT produto_id) FROM estoque_movimento"), 3);
    assert!(l.numero("SELECT SUM(custo) FROM produto") > custo_antes, "o frete pago à parte entrou no custo");
    let c = l.ok("compra_abrir", json!({ "id": id }));
    assert!(c["itens"].as_array().unwrap().iter().all(|it| it["acao"] == "nova" && !it["peca"].is_null()), "{c}");
}

/// D-09: a peça que a importação desta compra cadastrou é peça nova da compra. O caixa lança com o frete pago à
/// parte e os preços seguindo o custo sem a senha do gerente, como no "Cadastrar nova" de antes.
#[test]
fn o_caixa_lanca_a_peca_que_a_importacao_cadastrou_sem_o_gerente() {
    let mut l = loja("t108-caixa-lanca", LOJA_CNPJ);
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    let id = r["compras"][0]["id"].as_str().unwrap().to_string();
    let lancou = conferir_e_lancar(&mut l, &id, "90,00", true);
    assert_eq!(lancou["resumo"]["precosAtualizados"], 0, "{lancou}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
}
