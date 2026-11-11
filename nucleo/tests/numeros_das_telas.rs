//! Números das telas lidos num lugar só (dev.3.3, passo 2.2: G01, H13, ARQ-09).
//!
//! As telas não leem nem somam número do jeito delas: pedem ao núcleo
//! (`numero_somar`, `crediario_exemplo`), que usa a mesma leitura de todo o
//! resto ("1.5" = 1,5; "1.234" = mil duzentos e trinta e quatro; "0.250" =
//! 0,250). Antes, o carrinho lia "1.5" como 15 (H13), o total da baixa de
//! contas a pagar e o exemplo de atraso do crediário também (ARQ-09).
//!
//! Dados de mentira.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use comum::*;
use serde_json::json;

fn somar(l: &mut Loja, tipo: &str, somar: &[&str], subtrair: &[&str]) -> String {
    l.ok("numero_somar", json!({ "tipo": tipo, "somar": somar, "subtrair": subtrair }))["texto"]
        .as_str()
        .unwrap()
        .to_string()
}

/// H13: "1.5" + 1 no carrinho = 2,5 (a tela fazia 15 + 1 = 16).
#[test]
fn h13_soma_do_carrinho_pela_regra_do_nucleo() {
    let mut l = Loja::nova("numeros-somar");
    assert_eq!(somar(&mut l, "quantidade", &["1.5", "1"], &[]), "2,5");
    assert_eq!(somar(&mut l, "quantidade", &["1,5", "1.5"], &[]), "3");
    assert_eq!(somar(&mut l, "quantidade", &["0.250", "0,75"], &[]), "1", "G01: 0.250 é 0,250");
    assert_eq!(somar(&mut l, "quantidade", &["1.234", "1"], &[]), "1.235", "1.234 continua milhar");
    assert_eq!(somar(&mut l, "quantidade", &[], &[]), "0");
}

/// ARQ-09: o total da baixa ("Sai R$ …") = valor + juros − desconto, pela leitura do núcleo.
#[test]
fn arq09_total_da_baixa_pela_regra_do_nucleo() {
    let mut l = Loja::nova("numeros-baixa");
    assert_eq!(somar(&mut l, "dinheiro", &["1.234,56", "1.5"], &["0,06"]), "1.236,00");
    assert_eq!(somar(&mut l, "dinheiro", &["100", ""], &[""]), "100,00", "vazio vale 0");
    assert_eq!(somar(&mut l, "dinheiro", &["10"], &["15"]), "-5,00");
}

/// Texto que não é número é recusado com a mensagem da leitura (nunca vira 0
/// em silêncio, como o parseFloat das telas fazia).
#[test]
fn numero_invalido_recusado_com_a_mensagem() {
    let mut l = Loja::nova("numeros-invalido");
    let erro = l.erro("numero_somar", json!({ "tipo": "quantidade", "somar": ["1", "1,2,3"], "subtrair": [] }));
    assert!(erro.contains("vírgula") || erro.contains("inválido"), "{erro}");
    let erro = l.erro("numero_somar", json!({ "tipo": "dinheiro", "somar": ["abc"], "subtrair": [] }));
    assert!(erro.contains("Digite só números"), "{erro}");
}

fn exemplo(l: &mut Loja, multa: &str, juros: &str, carencia: &str) -> serde_json::Value {
    l.ok(
        "crediario_exemplo",
        json!({ "multa": multa, "juros": juros, "carencia": carencia, "valor": "100", "dias": 10 }),
    )
}

/// ARQ-09: o exemplo de atraso das Configurações é a conta da cobrança
/// (`juros_de`): R$ 100,00 com 10 dias de atraso, 2% de multa e 1% ao mês =
/// 2,00 + 0,33 de juros (arredondado) = R$ 102,33.
#[test]
fn arq09_exemplo_de_atraso_pela_conta_da_cobranca() {
    let mut l = Loja::nova("numeros-exemplo");
    let e = exemplo(&mut l, "2", "1", "");
    assert_eq!((e["extra"].as_str(), e["total"].as_str()), (Some("2,33"), Some("102,33")), "{e}");
    assert_eq!(e["dentroDaTolerancia"], false);
    assert_eq!(e["valor"], "100,00");
    // "2.5" é 2,5% (a tela lia 25%).
    let e = exemplo(&mut l, "2.5", "1", "");
    assert_eq!((e["extra"].as_str(), e["total"].as_str()), (Some("2,83"), Some("102,83")), "{e}");
    // Dentro da tolerância não se cobra nada.
    let e = exemplo(&mut l, "2", "1", "10");
    assert_eq!((e["extra"].as_str(), e["total"].as_str()), (Some("0,00"), Some("100,00")), "{e}");
    assert_eq!(e["dentroDaTolerancia"], true);
    // Os mesmos limites de gravar: acima de 20% é recusado, com o campo.
    let r = l.chamar(
        "crediario_exemplo",
        json!({ "multa": "30", "juros": "1", "carencia": "", "valor": "100", "dias": 10 }),
    );
    assert_eq!((r["ok"].as_bool(), r["campo"].as_str()), (Some(false), Some("crediarioMulta")), "{r}");
}
