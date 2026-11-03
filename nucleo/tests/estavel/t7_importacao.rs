//! Prioridade 2 — Importação de uma cópia dos dados do sistema antigo
//! (SINTEC exporta os XMLs das notas): conferência dos totais com a conta à
//! mão e a origem intacta (nada é gravado, apagado ou mudado nela).
//!
//! O acervo é FICTÍCIO (`tests/pki/gerar_acervo.py`), numa cópia em pasta
//! temporária, como se fosse o pen drive com a exportação:
//!
//! | Arquivo | O quê | Conta |
//! |---|---|---|
//! | compras/nf501, nf502 | fornecedor F1: F1-100 (código de barras G1), F1-200, F1-300 | 2 notas de fornecedor |
//! | compras/nf503 | F1 usa o código F1-200 em duas correias diferentes (D-13) | 1 nota de fornecedor |
//! | compras/nf900 e a cópia | fornecedor F2: B-1, B-2 (a cópia é a mesma nota) | 1 nota (a cópia não conta) |
//! | compras/nf901-devolucao | F2, finalidade devolução | 1 nota, não cadastra nada |
//! | vendas/2026-09.zip | nf10 (Oficina do Zé, CNPJ), nf11 (João, CPF), nfce12 (sem cliente) | 3 notas da loja |
//! | vendas/nf13 | Oficina do Zé de novo | 1 nota da loja |
//! | vendas/nf14-devolucao | devolução da loja para F1 | 1 nota da loja, não cadastra nada |
//! | vendas/nf15 | código 1005 em duas correias diferentes (D-13) e a 1002 escrita de outro jeito | 1 nota da loja |
//! | outros/ | adulterada, evento de cancelamento, arquivo que não é XML | 3 ignorados |
//!
//! Esperado: 15 arquivos, 11 notas (6 da loja, 5 de fornecedor), 3 ignorados;
//! 2 clientes novos, 2 fornecedores novos; 6 peças da loja (1001 a 1004 e as
//! duas correias do código 1005, separadas e sem esse código, que fica como
//! código antigo das duas; a 1002 escrita de outro jeito é a mesma peça) e 5
//! de fornecedor (as duas correias do F1-200, sem ligar o código a nenhuma,
//! F1-300, B-1, B-2); F1-100 é a mesma peça que a 1001
//! (mesmo código de barras): não vira peça nova, só liga o código do fornecedor.
//! Preço da 1001 = o da nota mais nova (nf11: 35,00). Custo da F1-200 =
//! 100,00 ÷ 5 = 20,00, preço com 100% de lucro = 40,00.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// O acervo real (fora do repositório) não entra aqui: nos testes o Rustinho só
// confia na PKI fictícia. As assinaturas reais são conferidas em
// `nfe_assinatura::testes::acervo_*`; a importação real fica no roteiro físico.

use serde_json::json;

use crate::comum::*;

const LOJA: &str = "11444777000161";
const F1: &str = "33444555000181";
const F2: &str = "55666777000181";
const C1: &str = "77888999000181";
const CPF_JOAO: &str = "52998224725";
const G1: &str = "7891000000014";

/// Copia a pasta toda (como copiar a exportação para o pen drive).
fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let alvo = para.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar(&e.path(), &alvo);
        } else {
            std::fs::copy(e.path(), &alvo).unwrap();
        }
    }
}

/// Retrato da pasta: cada arquivo, com tamanho, soma e data de modificação.
fn retrato(p: &Path, base: &Path, r: &mut BTreeMap<String, (u64, String, std::time::SystemTime)>) {
    for e in std::fs::read_dir(p).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            retrato(&e.path(), base, r);
        } else {
            let (t, h) = marca_do_arquivo(&e.path());
            let m = e.metadata().unwrap().modified().unwrap();
            r.insert(e.path().strip_prefix(base).unwrap().display().to_string(), (t, h, m));
        }
    }
}

fn acervo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pki/xml/acervo/exportacao")
}

#[test]
fn t7_1_importar_a_copia_confere_os_totais_e_nao_mexe_na_origem() {
    let mut l = Loja::nova("t7-1");
    l.executar(&format!(
        "INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '{LOJA}')
         ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor"
    ));
    let copia = l.pasta.join("pendrive/exportacao-sintec");
    copiar(&acervo(), &copia);
    let mut antes = BTreeMap::new();
    retrato(&copia, &copia, &mut antes);
    assert_eq!(antes.len(), 13, "arquivos na cópia (o .zip tem 3 dentro): {:?}", antes.keys());
    let foto_antes = foto_do_negocio(&l.conexao());
    let pasta = copia.display().to_string();

    // Prévia: a conta à mão.
    let p = l.ok("importar_previa", json!({ "arquivos": [pasta] }));
    let n = |k: &str| p[k].as_i64().unwrap_or_else(|| panic!("{k}: {p}"));
    assert_eq!(
        (n("arquivos"), n("notas"), n("notasLoja"), n("notasFornecedor"), n("ignorados")),
        (15, 11, 6, 5, 3),
        "{p}"
    );
    assert_eq!((n("clientesNovos"), n("fornecedoresNovos")), (2, 2), "{p}");
    assert_eq!((n("pecasLojaNovas"), n("pecasFornecedorNovas"), n("pecasJa")), (6, 5, 1), "{p}");
    assert_eq!(n("codigosRepetidos"), 2, "{p}");
    assert!(p["avisos"].to_string().contains("Código 1005"), "avisa o código repetido: {p}");
    assert!(p["avisos"].as_array().unwrap().len() >= 2, "avisa a adulterada e o arquivo estragado: {p}");
    // A prévia não grava nada.
    assert!(diferencas(&foto_antes, &foto_do_negocio(&l.conexao())).is_empty(), "a prévia gravou algo");

    let r = l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta], "clientes": true, "fornecedores": true, "pecasLoja": true, "pecasFornecedor": true }),
    );
    assert_eq!(r, json!({ "clientes": 2, "fornecedores": 2, "pecas": 11, "codigosLigados": 1 }), "{r}");

    // Os cadastros, um por um.
    let c = l.conexao();
    let um = |sql: &str| -> String { c.query_row(sql, [], |x| x.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}")) };
    assert_eq!(um(&format!("SELECT nome FROM pessoa WHERE documento = '{C1}'")), "OFICINA DO ZE DE TESTE LTDA");
    assert_eq!(um(&format!("SELECT nome FROM pessoa WHERE documento = '{CPF_JOAO}'")), "JOAO DA SILVA TESTE");
    assert!(um(&format!("SELECT nome FROM pessoa WHERE documento = '{F1}'")).contains("DISTRIBUIDORA UM"));
    assert!(um(&format!("SELECT nome FROM pessoa WHERE documento = '{F2}'")).contains("DOIS"));
    assert_eq!(um("SELECT COUNT(*) || '' FROM pessoa"), "4", "nem a loja nem o consumidor sem nome viram cadastro");
    let preco = |codigo: &str| -> String {
        um(&format!(
            "SELECT pp.preco || '' FROM produto p JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1
              WHERE p.codigo = '{codigo}'"
        ))
    };
    assert_eq!(preco("1001"), "3500", "preço da nota mais nova");
    assert_eq!(preco("1004"), "1250", "50,00 ÷ 4");
    assert_eq!(um("SELECT codigo_barras FROM produto WHERE codigo = '1001'"), G1);
    // F1-100 ficou ligada à 1001 pelo código de barras.
    assert_eq!(
        um(&format!(
            "SELECT p.codigo FROM produto_fornecedor f JOIN produto p ON p.id = f.produto_id
              JOIN pessoa x ON x.id = f.fornecedor_id WHERE x.documento = '{F1}' AND f.codigo = 'F1-100'"
        )),
        "1001"
    );
    let (custo, preco_f): (i64, i64) = c
        .query_row(
            "SELECT p.custo, pp.preco FROM produto_fornecedor f JOIN produto p ON p.id = f.produto_id
               JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1 WHERE f.codigo = 'F1-300'",
            [],
            |x| Ok((x.get(0)?, x.get(1)?)),
        )
        .unwrap();
    assert_eq!((custo, preco_f), (4500, 9000), "F1-300: 90,00 ÷ 2 = 45,00 e preço com 100% de lucro");
    let (custo, preco_f): (i64, i64) = c
        .query_row(
            "SELECT p.custo, pp.preco FROM produto p JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1
              WHERE p.nome = 'CORREIA DENTADA TESTE'",
            [],
            |x| Ok((x.get(0)?, x.get(1)?)),
        )
        .unwrap();
    assert_eq!((custo, preco_f), (2000, 4000), "F1-200: 100,00 ÷ 5 = 20,00 e preço com 100% de lucro");
    assert_eq!(um("SELECT COUNT(*) || '' FROM produto"), "11");
    assert_eq!(
        um("SELECT COUNT(*) || '' FROM produto_fornecedor WHERE codigo = 'F1-200'"),
        "0",
        "F1-200 não liga a nenhuma"
    );
    // D-13: as duas correias do código 1005 são peças separadas; nenhuma tem o
    // código 1005 (o leitor daria a errada); as duas o têm como código antigo.
    assert_eq!(um("SELECT COUNT(*) || '' FROM produto WHERE codigo = '1005'"), "0");
    assert_eq!(
        um("SELECT group_concat(nome, ' | ') FROM (SELECT p.nome FROM produto_codigo_antigo a JOIN produto p ON p.id = a.produto_id
             WHERE a.codigo = '1005' ORDER BY p.nome)"),
        "CORREIA EM V TESTE 13X950 - | CORREIA EM V TESTE 13X980 -"
    );
    assert_eq!(
        um("SELECT COUNT(*) || '' FROM produto WHERE nome LIKE 'PASTILHA DE FREIO%'"),
        "1",
        "a 1002 não duplicou"
    );
    drop(c);
    // Procurar ou ler o código 1005 mostra as duas para escolher.
    let b = l.ok("produtos_buscar", json!({ "texto": "1005", "limite": 20 }));
    let nomes: Vec<&str> = b["itens"].as_array().unwrap().iter().filter_map(|i| i["nome"].as_str()).collect();
    assert!(nomes.iter().filter(|n| n.starts_with("CORREIA")).count() == 2, "{b}");
    let c = l.conexao();
    let um = |sql: &str| -> String { c.query_row(sql, [], |x| x.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}")) };
    // Cadastro não mexe em estoque nem em dinheiro.
    for t in ["estoque_movimento", "venda", "conta_receber", "conta_pagar", "compra", "pagamento", "recebimento"] {
        assert_eq!(um(&format!("SELECT COUNT(*) || '' FROM {t}")), "0", "a importação gravou em {t}");
    }
    drop(c);

    // De novo: nada novo (nada é cadastrado duas vezes).
    let p2 = l.ok("importar_previa", json!({ "arquivos": [pasta] }));
    assert_eq!(
        (
            p2["clientesNovos"].as_i64(),
            p2["fornecedoresNovos"].as_i64(),
            p2["pecasLojaNovas"].as_i64(),
            p2["pecasFornecedorNovas"].as_i64()
        ),
        (Some(0), Some(0), Some(0), Some(0)),
        "{p2}"
    );
    assert_eq!(
        (p2["clientesJa"].as_i64(), p2["fornecedoresJa"].as_i64(), p2["pecasJa"].as_i64()),
        (Some(2), Some(2), Some(12))
    );
    let r2 = l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta], "clientes": true, "fornecedores": true, "pecasLoja": true, "pecasFornecedor": true }),
    );
    assert_eq!(r2, json!({ "clientes": 0, "fornecedores": 0, "pecas": 0, "codigosLigados": 0 }), "{r2}");

    // A origem está intacta: os mesmos arquivos, com o mesmo conteúdo e a mesma data.
    let mut depois = BTreeMap::new();
    retrato(&copia, &copia, &mut depois);
    assert_eq!(antes, depois, "a cópia do sistema antigo foi alterada");
    // E nada do Rustinho foi parar lá (nenhum arquivo temporário).
    assert_eq!(depois.len(), 13);
}

/// T7.2 — Só o gerente importa; sem o CNPJ da loja, explica e não grava.
#[test]
fn t7_2_regras() {
    let mut l = Loja::nova("t7-2");
    let pasta = acervo().display().to_string();
    let m = l.erro("importar_previa", json!({ "arquivos": [pasta] }));
    assert!(m.contains("CNPJ da loja"), "{m}");
    l.executar(&format!("INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '{LOJA}')"));
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let m = l.erro("importar_cadastros", json!({ "arquivos": [pasta], "clientes": true }));
    assert!(m.contains("Só o gerente"), "{m}");
    assert_eq!(l.numero("SELECT COUNT(*) FROM pessoa"), 0);
}

/// T7.3 — Compra por XML com o mesmo código do fornecedor em duas peças
/// diferentes (D-13): nenhuma das duas liga sozinha pelo código (seria a
/// mesma peça para as duas), e lançar não troca a ligação que já existia.
#[test]
fn t7_3_compra_com_codigo_repetido_nao_liga_sozinha() {
    let mut l = Loja::nova("t7-3");
    l.executar(&format!("INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '{LOJA}')"));
    let importar = |l: &mut Loja, arquivo: &str| -> String {
        let caminho = acervo().join("compras").join(arquivo).display().to_string();
        let r = l.ok("compra_importar", json!({ "caminho": caminho }));
        r["compras"][0]["id"].as_str().unwrap_or_else(|| panic!("{r}")).to_string()
    };
    // O protocolo de cada nota de teste é 1352600000 + o número da nota.
    let conferir = |l: &mut Loja, id: &str, protocolo: &str| {
        l.ok(
            "compra_conferir_autorizacao",
            json!({ "id": id, "situacao": "autorizada", "protocolo": protocolo,
                    "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
        );
    };
    // 1ª compra: F1-200 (uma correia) entra como peça nova e fica ligada.
    let c1 = importar(&mut l, "nf501.xml");
    let f = l.ok("compra_abrir", json!({ "id": c1 }));
    let escolhas: Vec<serde_json::Value> = f["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| json!({ "id": it["id"], "acao": "nova", "produtoId": "", "fator": "1" }))
        .collect();
    conferir(&mut l, &c1, "135260000000501");
    l.ok("compra_lancar", json!({ "id": c1, "itens": escolhas }));
    let correia: String = l
        .conexao()
        .query_row("SELECT produto_id FROM produto_fornecedor WHERE codigo = 'F1-200'", [], |x| x.get(0))
        .unwrap();

    // 2ª compra: F1-200 em duas correias diferentes.
    let c2 = importar(&mut l, "nf503.xml");
    let f = l.ok("compra_abrir", json!({ "id": c2 }));
    let itens = f["itens"].as_array().unwrap();
    assert_eq!(itens.len(), 2);
    for it in itens {
        assert_eq!(it["acao"], "", "não liga sozinha pelo código repetido: {it}");
    }
    // A pessoa decide: a primeira é a correia de antes; a reforçada é nova.
    let escolhas = vec![
        json!({ "id": itens[0]["id"], "acao": "ligar", "produtoId": correia, "fator": "1" }),
        json!({ "id": itens[1]["id"], "acao": "nova", "produtoId": "", "fator": "1" }),
    ];
    conferir(&mut l, &c2, "135260000000503");
    l.ok("compra_lancar", json!({ "id": c2, "itens": escolhas }));
    let ligada: String = l
        .conexao()
        .query_row("SELECT produto_id FROM produto_fornecedor WHERE codigo = 'F1-200'", [], |x| x.get(0))
        .unwrap();
    assert_eq!(ligada, correia, "a ligação de antes continua (a nova não tomou o lugar)");
    assert_eq!(l.estoque_pelos_movimentos(&correia), 10, "5 da 1ª compra + 5 da 2ª");
    let reforcada: String = l
        .conexao()
        .query_row("SELECT id FROM produto WHERE nome = 'CORREIA DENTADA TESTE REFORCADA'", [], |x| x.get(0))
        .unwrap();
    assert_eq!(l.estoque_pelos_movimentos(&reforcada), 2, "a reforçada é outra peça, com o estoque dela");
}
