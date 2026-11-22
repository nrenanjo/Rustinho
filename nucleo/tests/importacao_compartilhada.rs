//! T108 / T111 (Decisão D-T108 — Opção C do proprietário, 22/11/2026):
//! a importação de XML compartilhada entre Peças e Compras com registro garantido de produtos.
//!
//! Princípio inegociável da Opção C:
//! "Nenhum produto deve ficar fora do sistema por falta de cadastro manual. A automação deve fazer
//! o trabalho pesado; o usuário intervém somente nas exceções."
//!
//! Comportamento verificado ponta a ponta:
//! 1. Produto existente: identificado automaticamente (fornecedor, GTIN ou código original) e vinculado.
//! 2. Produto novo com identidade forte (GTIN válido livre ou código do fornecedor ≥ 3 alfanuméricos,
//!    sem ambiguidade no lote nem descrição idêntica já cadastrada): cadastrado automaticamente em Compras
//!    com todos os dados do XML e estoque 0.
//! 3. Identificação duvidosa (código fraco sem GTIN, só símbolos, repetido na nota/lote, ambíguo, conflito
//!    GTIN × fornecedor, peça desativada ou possível duplicata por descrição): vira registro provisório em
//!    `xml_pendencia_identificacao` preservando todos os dados do XML; o usuário confirma, associa ou corrige
//!    a identificação sem redigitar, religando todas as notas em rascunho (R9).
//! 4. Antiduplicidade: nunca cria 2ª peça ao lançar múltiplas notas em qualquer ordem (A1), nunca funde
//!    produtos distintos por semelhança de descrição, limpa peças provisórias órfãs ao trocar/ignorar/descartar
//!    preservando as usadas ou editadas (A5).
//! 5. Importação compartilhada nos dois sentidos (Peças ↔ Compras) sem duplicar.
//! 6. Integridade comercial: estoque, custo efetivo, contas a pagar e regra de preços do gerente (D-09 / A6)
//!    permanecem sujeitos ao lançamento definitivo da compra.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use std::path::Path;

use comum::*;
use serde_json::{json, Value};

/// CNPJ da loja de teste: o destinatário das notas de exemplo e do acervo.
const LOJA_CNPJ: &str = "11444777000161";
/// Um CNPJ válido que não é o destinatário da nota de exemplo.
const OUTRO_CNPJ: &str = "12345678000195";
/// Emitente da nota de exemplo.
const FORNECEDOR: &str = "11222333000181";
const EXEMPLO: &str = include_str!("nfe_exemplo.xml");

fn acervo_compras(arquivo: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pki/xml/acervo/exportacao/compras").join(arquivo);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn fixture_codigos(arquivo: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pki/xml/codigos").join(arquivo);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

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

/// Uma pasta com os XMLs fornecidos `(nome_arquivo, conteudo)`.
fn pasta_com(l: &Loja, nome: &str, arquivos: &[(&str, &str)]) -> String {
    let p = l.pasta.join(nome);
    std::fs::create_dir_all(&p).unwrap();
    for (arq, conteudo) in arquivos {
        std::fs::write(p.join(arq), conteudo).unwrap();
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

fn conferir_autorizacao(l: &mut Loja, compra_id: &str, protocolo: &str) {
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": compra_id, "situacao": "autorizada", "protocolo": protocolo,
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
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

fn escolhas_atuais(compra: &Value) -> Vec<Value> {
    compra["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            json!({
                "id": it["id"],
                "acao": it["acao"],
                "produtoId": it["peca"]["id"].as_str().unwrap_or(""),
                "fator": it["fator"],
                "baixarPreco": false
            })
        })
        .collect()
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
// Por Compras — Decisão D-T108 Opção C
// ---------------------------------------------------------------------------

/// Opção C (Regras 2, 5 e 6): ao importar por Compras uma nota cujos itens novos têm identidade forte,
/// o fornecedor e as peças são cadastrados automaticamente (com dados completos do XML e vínculo do fornecedor),
/// sem mexer no estoque nem no A pagar até o lançamento definitivo da compra.
#[test]
fn importou_por_compras_cadastra_pecas_novas_automaticamente_sem_mexer_em_estoque() {
    let mut l = loja("t111-compras-auto", LOJA_CNPJ);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["pecasCadastradas"], 3, "as 3 peças com identidade forte nascem automaticamente: {r}");
    assert!(r.get("pendenciasIdentificacao").is_none(), "nenhuma pendência para itens inequívocos: {r}");
    assert_eq!(n(&l, &format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{FORNECEDOR}'")), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "as 3 peças foram cadastradas automaticamente");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto_fornecedor"), 3, "os 3 vínculos de fornecedor foram gravados");
    assert_eq!(
        n(&l, "SELECT COUNT(*) FROM auditoria WHERE acao = 'produto.importar_compra'"),
        3,
        "cada peça automática tem trilha de auditoria própria"
    );
    assert_eq!(n(&l, "SELECT COUNT(*) FROM xml_pendencia_identificacao"), 0);
    assert_eq!(
        itens(&l),
        vec![
            ("nova".to_string(), "importacao".to_string(), true),
            ("nova".to_string(), "importacao".to_string(), true),
            ("nova".to_string(), "importacao".to_string(), true),
        ],
        "os itens ficam marcados como peça nova criada pela importação"
    );
    // Dados do XML preservados (ex.: NCM, CEST e GTIN do rolamento).
    let (ncm, cest, gtin): (String, String, String) = l
        .conexao()
        .query_row("SELECT ncm, cest, codigo_barras FROM produto WHERE nome LIKE 'ROLAMENTO%'", [], |x| {
            Ok((x.get(0)?, x.get(1)?, x.get(2)?))
        })
        .unwrap();
    assert_eq!((ncm.as_str(), cest.as_str(), gtin.as_str()), ("84822010", "0100100", "7891234567895"));
    nada_no_estoque_nem_no_financeiro(&l);

    // Lançar a compra entra estoque nas mesmas 3 peças sem duplicar nenhuma.
    let id = r["compras"][0]["id"].as_str().unwrap().to_string();
    let c = l.ok("compra_abrir", json!({ "id": id }));
    conferir_autorizacao(&mut l, &id, "135260000000001");
    let lanc = l.ok("compra_lancar", json!({ "id": id, "itens": escolhas_atuais(&c), "gerarPagar": true }));
    assert_eq!(lanc["resumo"]["pecasNovas"], 3, "{lanc}");
    assert_eq!(lanc["resumo"]["itensEstoque"], 3, "{lanc}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "nenhuma peça duplicada no lançar");
    assert_eq!(n(&l, "SELECT COUNT(DISTINCT produto_id) FROM estoque_movimento"), 3);
    assert!(n(&l, "SELECT COUNT(*) FROM conta_pagar") >= 1, "contas a pagar nascem somente no lançar");
}

/// Opção C + A6 (D-09): o caixa importa a compra e as peças novas nascem automaticamente sem expor custo/lucro;
/// porém, se o gerente já editou o preço de uma peça provisória antes do lançamento, o caixa NÃO pode
/// sobrescrever esse preço ao lançar (D-09 exige senha do gerente).
#[test]
fn o_caixa_importa_por_compras_e_regra_d09_continua_protegida() {
    let mut l = loja("t111-caixa-d09", LOJA_CNPJ);
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let pasta = pasta(&l, "xml", 1);
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["pecasCadastradas"], 3, "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
    let compra_id = r["compras"][0]["id"].as_str().unwrap().to_string();
    let c = l.ok("compra_abrir", json!({ "id": compra_id }));
    let pid0 = c["itens"][0]["peca"]["id"].as_str().unwrap().to_string();
    // Caixa não vê custo na ficha da peça.
    let ficha_caixa = l.ok("produto_abrir", json!({ "id": pid0 }));
    assert_eq!(ficha_caixa["custo"], "", "D-09: custo escondido para o caixa");

    // Gerente Ana edita o preço da 1ª peça antes do lançamento.
    l.entrar("Ana", SENHA_ANA);
    let mut ficha_ana = l.ok("produto_abrir", json!({ "id": pid0 }));
    ficha_ana["precos"][0]["preco"] = json!("150,00");
    ficha_ana["precos"][0]["lucro"] = json!("100");
    l.ok("produto_salvar", ficha_ana);
    conferir_autorizacao(&mut l, &compra_id, "135260000000001");

    // Volta para a caixa Bia: tentar lançar com frete extra e atualizarPrecos = true tem de pedir D-09!
    l.entrar("Bia", SENHA_BIA);
    let tentar = l.chamar(
        "compra_lancar",
        json!({
            "id": compra_id,
            "itens": escolhas_atuais(&c),
            "freteExtra": "90,00",
            "atualizarPrecos": true
        }),
    );
    assert_eq!(tentar["ok"], false, "A6/D-09: caixa alterou preço editado pelo gerente sem senha: {tentar}");
    assert_eq!(tentar["campo"], "precos", "{tentar}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM estoque_movimento"), 0, "nada foi lançado sem D-09");

    // Com atualizarPrecos = false, a caixa lança normalmente preservando o preço editado pelo gerente.
    l.ok(
        "compra_lancar",
        json!({
            "id": compra_id,
            "itens": escolhas_atuais(&c),
            "freteExtra": "90,00",
            "atualizarPrecos": false
        }),
    );
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "sem duplicatas no lançar");
    let preco_final: i64 = l
        .conexao()
        .query_row("SELECT preco FROM produto_preco WHERE produto_id = ?1 AND tabela = 1", [&pid0], |x| x.get(0))
        .unwrap();
    assert_eq!(preco_final, 15_000, "preço definido pelo gerente foi preservado");
}

/// Opção C + A1 (antiduplicidade multi-nota e inversão de ordem de lançamento):
/// pasta com nf501 (F1-100, F1-200) e nf502 (F1-100, F1-300).
/// F1-100 nasce em nf501 e já liga automaticamente em nf502.
/// Lançar nf502 ANTES de nf501 e depois lançar nf501 gera 1 única peça F1-100 com estoque somado.
#[test]
fn opcao_c_a1_duas_notas_mesmo_item_lancar_segunda_antes_da_primeira() {
    let mut l = loja("t111-a1-multi", LOJA_CNPJ);
    let nf501 = acervo_compras("nf501.xml");
    let nf502 = acervo_compras("nf502.xml");
    let p = pasta_com(&l, "lote", &[("01_nf501.xml", &nf501), ("02_nf502.xml", &nf502)]);
    let r = l.ok("compra_importar", json!({ "caminho": p }));
    let compras = r["compras"].as_array().unwrap();
    assert_eq!(compras.len(), 2, "{r}");
    assert_eq!(r["pecasCadastradas"], 3, "F1-100 (1x), F1-200, F1-300 = 3 peças: {r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "F1-100 não duplicou entre nf501 e nf502");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto_fornecedor"), 3);

    let id501 = compras.iter().find(|c| c["numero"] == "501").unwrap()["id"].as_str().unwrap().to_string();
    let id502 = compras.iter().find(|c| c["numero"] == "502").unwrap()["id"].as_str().unwrap().to_string();
    let c501 = l.ok("compra_abrir", json!({ "id": id501 }));
    let c502 = l.ok("compra_abrir", json!({ "id": id502 }));

    let it501_f100 = c501["itens"].as_array().unwrap().iter().find(|i| i["codigoFornecedor"] == "F1-100").unwrap();
    let it502_f100 = c502["itens"].as_array().unwrap().iter().find(|i| i["codigoFornecedor"] == "F1-100").unwrap();
    assert_eq!(it501_f100["acao"], "nova", "{it501_f100}");
    assert_eq!(it501_f100["achadoPor"], "importacao", "{it501_f100}");
    assert_eq!(it502_f100["acao"], "ligar", "{it502_f100}");
    assert_eq!(it502_f100["achadoPor"], "fornecedor", "{it502_f100}");
    let pid_f100 = it501_f100["peca"]["id"].as_str().unwrap().to_string();
    assert_eq!(it502_f100["peca"]["id"].as_str(), Some(pid_f100.as_str()), "mesma peça nas duas notas");

    // Lança NF 502 PRIMEIRO, depois NF 501!
    conferir_autorizacao(&mut l, &id502, "135260000000502");
    l.ok("compra_lancar", json!({ "id": id502, "itens": escolhas_atuais(&c502) }));
    let saldo_apos_502 = l.estoque_pelos_movimentos(&pid_f100);
    assert!(saldo_apos_502 > 0, "estoque da nf502 entrou em F1-100");

    conferir_autorizacao(&mut l, &id501, "135260000000501");
    l.ok("compra_lancar", json!({ "id": id501, "itens": escolhas_atuais(&c501) }));
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "A1: nenhuma 2ª peça criada ao lançar nf501 depois");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto_fornecedor WHERE codigo = 'F1-100'"), 1);
    let saldo_final = l.estoque_pelos_movimentos(&pid_f100);
    assert!(saldo_final > saldo_apos_502, "estoque da nf501 somou na mesma peça F1-100");
}

/// Opção C (Regras 3 e 4 + A4): identificações duvidosas NÃO cadastram peça automática nem fundem por descrição;
/// todas criam registro provisório em `xml_pendencia_identificacao` preservando os dados do XML.
#[test]
fn opcao_c_identificacao_duvidosa_vira_registro_provisorio_sem_duplicar_nem_fundir() {
    // 1) Código fraco (< 3 caracteres alfanuméricos) sem GTIN: nf900 tem B-1 e B-2.
    let mut l = loja("t111-fraco", LOJA_CNPJ);
    let nf900 = acervo_compras("nf900.xml");
    let p = pasta_com(&l, "nf900", &[("nf900.xml", &nf900)]);
    let r = l.ok("compra_importar", json!({ "caminho": p }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert!(r.get("pecasCadastradas").is_none(), "código fraco sem GTIN não cadastra sozinho: {r}");
    assert_eq!(r["pendenciasIdentificacao"], 2, "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0);
    let pends = l.ok("xml_pendencias_listar", json!({}));
    assert_eq!(pends.as_array().unwrap().len(), 2, "{pends}");
    assert!(pends.as_array().unwrap().iter().all(|x| x["motivo"] == "codigo_fraco_sem_gtin"), "{pends}");

    // 2) Código repetido entre notas diferentes do mesmo pacote (A4): nf501 + nf503 com F1-200 em 2 descrições.
    let mut l2 = loja("t111-lote-amb", LOJA_CNPJ);
    let nf501 = acervo_compras("nf501.xml");
    let nf503 = acervo_compras("nf503.xml");
    let p2 = pasta_com(&l2, "lote", &[("01_nf501.xml", &nf501), ("02_nf503.xml", &nf503)]);
    let r2 = l2.ok("compra_importar", json!({ "caminho": p2 }));
    assert_eq!(r2["compras"].as_array().unwrap().len(), 2, "{r2}");
    // Só F1-100 tem identidade inequívoca; F1-200 está em 2 descrições diferentes no lote!
    assert_eq!(n(&l2, "SELECT COUNT(*) FROM produto"), 1, "só F1-100 nasce sozinho; F1-200 é ambíguo no lote");
    assert_eq!(
        n(&l2, "SELECT COUNT(*) FROM produto_fornecedor WHERE codigo = 'F1-200'"),
        0,
        "F1-200 não pode ser vinculado automaticamente"
    );
    let pends2 = l2.ok("xml_pendencias_listar", json!({}));
    assert_eq!(pends2.as_array().unwrap().len(), 2, "as 2 descrições de F1-200 em pendência: {pends2}");
    assert!(pends2.as_array().unwrap().iter().all(|x| x["motivo"] == "codigo_repetido"), "{pends2}");

    // 3) Possível duplicata por descrição idêntica (Regra 4): nf601 (00008373) já cadastrada, depois nf602 (8373,
    //    sem GTIN, mesma descrição) → NÃO duplica nem funde; vira registro provisório com a peça de nf601 como candidata.
    let mut l3 = loja("t111-dup-desc", LOJA_CNPJ);
    let nf601 = fixture_codigos("nf601_patral_00008373.xml");
    let nf602 = fixture_codigos("nf602_patral_8373.xml");
    let p601 = pasta_com(&l3, "n1", &[("nf601.xml", &nf601)]);
    let r601 = l3.ok("compra_importar", json!({ "caminho": p601 }));
    assert_eq!(r601["pecasCadastradas"], 1, "{r601}");
    let peca601 = r601["compras"][0]["itens"][0]["peca"]["id"].as_str().unwrap().to_string();

    let p602 = pasta_com(&l3, "n2", &[("nf602.xml", &nf602)]);
    let r602 = l3.ok("compra_importar", json!({ "caminho": p602 }));
    assert!(r602.get("pecasCadastradas").is_none(), "não pode duplicar peça com descrição idêntica: {r602}");
    assert_eq!(r602["pendenciasIdentificacao"], 1, "{r602}");
    assert_eq!(n(&l3, "SELECT COUNT(*) FROM produto"), 1, "zero duplicatas por descrição");
    let pends3 = l3.ok("xml_pendencias_listar", json!({}));
    assert_eq!(pends3.as_array().unwrap().len(), 1, "{pends3}");
    assert_eq!(pends3[0]["motivo"], "possivel_duplicata_descricao", "{pends3}");
    assert_eq!(pends3[0]["candidatos"][0]["id"], peca601, "traz a peça existente como candidata: {pends3}");
}

/// Opção C (Regra 3) + R9: o usuário corrige apenas a identificação do registro provisório sem redigitar
/// NCM/CEST/origem/custo, e TODAS as compras em rascunho que aguardavam aquele item religam automaticamente.
#[test]
fn opcao_c_corrigir_identificacao_sem_redigitar_e_religar_multiplas_notas() {
    let mut l = loja("t111-corrigir-r9", LOJA_CNPJ);
    let nf501 = acervo_compras("nf501.xml");
    let nf503 = acervo_compras("nf503.xml");
    let p = pasta_com(&l, "lote", &[("01_nf501.xml", &nf501), ("02_nf503.xml", &nf503)]);
    let r = l.ok("compra_importar", json!({ "caminho": p }));
    let compras = r["compras"].as_array().unwrap();
    let id501 = compras.iter().find(|c| c["numero"] == "501").unwrap()["id"].as_str().unwrap().to_string();
    let id503 = compras.iter().find(|c| c["numero"] == "503").unwrap()["id"].as_str().unwrap().to_string();

    let pends = l.ok("xml_pendencias_listar", json!({}));
    let pend_correia = pends
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["descricao"] == "CORREIA DENTADA TESTE")
        .expect("pendência da correia");
    let ncm_xml = pend_correia["ncm"].as_str().unwrap().to_string();
    let id_pend = pend_correia["id"].as_str().unwrap().to_string();

    // Corrige só o nome e a unidade (sem redigitar NCM/custo/origem).
    let res = l.ok(
        "xml_pendencia_resolver",
        json!({
            "id": id_pend,
            "acao": "nova",
            "produtoId": "",
            "nome": "CORREIA DENTADA GATES CORRIGIDA",
            "codigoFornecedor": "F1-200-A",
            "codigoBarras": "",
            "unidade": "PC"
        }),
    );
    let nova_id = res["produtoId"].as_str().unwrap().to_string();
    let ficha = l.ok("produto_abrir", json!({ "id": nova_id }));
    assert_eq!(ficha["nome"], "CORREIA DENTADA GATES CORRIGIDA");
    assert_eq!(ficha["unidade"], "PC");
    assert_eq!(ficha["ncm"], ncm_xml, "dados fiscais do XML preservados sem redigitar");

    // R9: AMBAS as notas em rascunho (nf501 e nf503) religaram o item "CORREIA DENTADA TESTE",
    // enquanto "CORREIA DENTADA TESTE REFORCADA" em nf503 continuou pendente.
    let c501 = l.ok("compra_abrir", json!({ "id": id501 }));
    let it501 = c501["itens"].as_array().unwrap().iter().find(|i| i["descricao"] == "CORREIA DENTADA TESTE").unwrap();
    assert_eq!(it501["acao"], "ligar", "nf501 religou após resolver pendência: {it501}");
    assert_eq!(it501["peca"]["id"].as_str(), Some(nova_id.as_str()));

    let c503 = l.ok("compra_abrir", json!({ "id": id503 }));
    let it503_normal =
        c503["itens"].as_array().unwrap().iter().find(|i| i["descricao"] == "CORREIA DENTADA TESTE").unwrap();
    let it503_reforcada =
        c503["itens"].as_array().unwrap().iter().find(|i| i["descricao"] == "CORREIA DENTADA TESTE REFORCADA").unwrap();
    assert_eq!(it503_normal["acao"], "ligar", "nf503 também religou: {it503_normal}");
    assert_eq!(it503_normal["peca"]["id"].as_str(), Some(nova_id.as_str()));
    assert_eq!(it503_reforcada["acao"], "", "a outra descrição NÃO pode ser fundida: {it503_reforcada}");
}

/// Opção C + A5 (limpeza anti-órfão):
/// - descartar rascunho remove peças provisórias nunca usadas nem editadas (e seus vínculos);
/// - trocar peça ou marcar "ignorar" remove a peça provisória órfã daquele item;
/// - peças editadas manualmente ou referenciadas por outro rascunho são preservadas.
#[test]
fn opcao_c_a5_limpa_peca_provisoria_orfa_e_preserva_usadas_ou_editadas() {
    // 1) Descartar rascunho inteiro remove as 3 peças provisórias e vínculos.
    let mut l = loja("t111-a5-desc", LOJA_CNPJ);
    let a = arquivo(&l, "compra");
    let r = l.ok("compra_importar", json!({ "caminho": a }));
    let id = r["compras"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto_fornecedor"), 3);
    l.ok("compra_descartar", json!({ "id": id }));
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 0, "A5: descarte removeu peças provisórias órfãs");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto_fornecedor"), 0, "A5: vínculos órfãos removidos");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM auditoria WHERE acao = 'produto.desfazer_provisorio'"), 3);

    // 2) Trocar peça e ignorar item via compra_atualizar remove apenas as órfãs trocadas/ignoradas.
    let mut l2 = loja("t111-a5-troca", LOJA_CNPJ);
    let manual = l2.peca("PECA MANUAL EXISTENTE", "10,00", "20,00", "5");
    let a2 = arquivo(&l2, "compra");
    let r2 = l2.ok("compra_importar", json!({ "caminho": a2 }));
    let id2 = r2["compras"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(n(&l2, "SELECT COUNT(*) FROM produto"), 4, "1 manual + 3 provisórias");
    let c2 = l2.ok("compra_abrir", json!({ "id": id2 }));
    let its = c2["itens"].as_array().unwrap();
    let pid_mantida = its[2]["peca"]["id"].as_str().unwrap().to_string();
    l2.ok(
        "compra_atualizar",
        json!({
            "id": id2,
            "freteExtra": "",
            "atualizarPrecos": false,
            "observacoes": "",
            "gerarPagar": false,
            "itens": [
                { "id": its[0]["id"], "acao": "ligar", "produtoId": manual, "fator": "1", "baixarPreco": false },
                { "id": its[1]["id"], "acao": "ignorar", "produtoId": "", "fator": "1", "baixarPreco": false },
                { "id": its[2]["id"], "acao": "nova", "produtoId": "", "fator": "1", "baixarPreco": false }
            ]
        }),
    );
    assert_eq!(n(&l2, "SELECT COUNT(*) FROM produto"), 2, "sobram só a manual e a 3ª mantida");
    assert_eq!(n(&l2, &format!("SELECT COUNT(*) FROM produto WHERE id = '{pid_mantida}'")), 1);
    assert_eq!(n(&l2, &format!("SELECT COUNT(*) FROM produto WHERE id = '{manual}'")), 1);

    // 3) Preserva peça usada por outro rascunho ou editada manualmente; limpa ao descartar o último rascunho.
    let mut l3 = loja("t111-a5-preserva", LOJA_CNPJ);
    let nf501 = acervo_compras("nf501.xml");
    let nf502 = acervo_compras("nf502.xml");
    let p3 = pasta_com(&l3, "lote", &[("01_nf501.xml", &nf501), ("02_nf502.xml", &nf502)]);
    let r3 = l3.ok("compra_importar", json!({ "caminho": p3 }));
    let compras = r3["compras"].as_array().unwrap();
    let id501 = compras.iter().find(|c| c["numero"] == "501").unwrap()["id"].as_str().unwrap().to_string();
    let id502 = compras.iter().find(|c| c["numero"] == "502").unwrap()["id"].as_str().unwrap().to_string();
    let c501 = l3.ok("compra_abrir", json!({ "id": id501 }));
    let pid_f200 = c501["itens"].as_array().unwrap().iter().find(|i| i["codigoFornecedor"] == "F1-200").unwrap()
        ["peca"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Edita F1-200 manualmente antes de descartar nf501.
    let mut f200 = l3.ok("produto_abrir", json!({ "id": pid_f200 }));
    f200["observacoes"] = json!("Editada pelo gerente");
    l3.ok("produto_salvar", f200);
    l3.ok("compra_descartar", json!({ "id": id501 }));
    // F1-100 ainda está em nf502, F1-200 foi editada, F1-300 está em nf502 → todas 3 preservadas!
    assert_eq!(n(&l3, "SELECT COUNT(*) FROM produto"), 3, "F1-100 (em nf502) e F1-200 (editada) preservadas");
    // Agora descarta nf502 → F1-100 e F1-300 viram órfãs e saem; só F1-200 (editada) fica!
    l3.ok("compra_descartar", json!({ "id": id502 }));
    assert_eq!(n(&l3, "SELECT COUNT(*) FROM produto"), 1, "só a peça editada permanece");
    assert_eq!(n(&l3, &format!("SELECT COUNT(*) FROM produto WHERE id = '{pid_f200}'")), 1);
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
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3, "as peças não se repetem");
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
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
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

/// Por Compras e depois por Peças (a mesma pasta): a nota e as 3 peças já entraram em Compras;
/// o "Trazer cadastros" em Peças reconhece tudo como já existente sem duplicar nada.
#[test]
fn compras_e_depois_pecas_nao_duplica() {
    let mut l = loja("t108-compras-pecas", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    l.ok("compra_importar", json!({ "caminho": pasta.clone() }));
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
    assert_eq!((p["comprasNovas"].as_i64(), p["comprasJa"].as_i64()), (Some(0), Some(1)), "{p}");
    assert_eq!(p["pecasFornecedorNovas"].as_i64(), Some(0), "as 3 peças já existem: {p}");
    let r = cadastrar(&mut l, &pasta, true);
    assert_eq!(r, json!({ "clientes": 0, "fornecedores": 0, "pecas": 0, "codigosLigados": 0 }), "{r}");
    assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM produto"), 3);
    assert_eq!(n(&l, "SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'"), 0);
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
    conferir_autorizacao(&mut l, &id, "135260000000001");
    // Como a tela manda (o "Gerar contas a pagar" vem marcado).
    l.ok("compra_lancar", json!({ "id": id, "itens": escolhas_atuais(&c), "gerarPagar": true }));
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

/// N3 da 2ª conferência da T108: a mesma nota em dois arquivos, um sem o protocolo de autorização (a NF-e sozinha,
/// que serve para os cadastros e não para Compras) e outro com ele (a procNFe). Para Compras vale a cópia que serve,
/// venha ela antes ou depois.
#[test]
fn a_copia_com_protocolo_vale_para_compras() {
    let ini = EXEMPLO.find("<NFe xmlns").unwrap();
    let fim = EXEMPLO.find("</NFe>").unwrap() + "</NFe>".len();
    let sem_protocolo = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n", &EXEMPLO[ini..fim]);
    for (nome, primeiro, segundo) in
        [("sem-antes", &sem_protocolo, EXEMPLO), ("com-antes", &EXEMPLO.to_string(), sem_protocolo.as_str())]
    {
        let mut l = loja(&format!("t108-copias-{nome}"), LOJA_CNPJ);
        let p = l.pasta.join("xml");
        std::fs::create_dir_all(&p).unwrap();
        // A pasta é lida na ordem dos nomes.
        std::fs::write(p.join("a.xml"), primeiro).unwrap();
        std::fs::write(p.join("b.xml"), segundo).unwrap();
        let pasta = p.display().to_string();
        let prev = l.ok("importar_previa", json!({ "arquivos": [pasta.clone()] }));
        assert_eq!(prev["notas"].as_i64(), Some(1), "a mesma nota: {nome} {prev}");
        assert_eq!(
            (prev["comprasNovas"].as_i64(), prev["comprasNaoServe"].as_i64()),
            (Some(1), Some(0)),
            "{nome}: {prev}"
        );
        let r = cadastrar(&mut l, &pasta, true);
        assert_eq!(r["compras"], 1, "{nome}: {r}");
        assert_eq!(n(&l, "SELECT COUNT(*) FROM compra"), 1, "{nome}");
    }
}

/// N4 da 2ª conferência da T108: a pasta que chega como endereço (`file://...`) também é um pacote: a nota que já
/// está em Compras é contada, não é erro.
#[test]
fn compras_pasta_por_endereco_tambem_e_pacote() {
    let mut l = loja("t108-pasta-endereco", LOJA_CNPJ);
    let pasta = pasta(&l, "xml", 1);
    let endereco = format!("file://{pasta}");
    let r = l.ok("compra_importar", json!({ "caminho": endereco.clone() }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    let r2 = l.ok("compra_importar", json!({ "caminho": endereco }));
    assert_eq!((r2["jaEmCompras"].clone(), r2["erros"].clone()), (json!(1), json!([])), "{r2}");
}
