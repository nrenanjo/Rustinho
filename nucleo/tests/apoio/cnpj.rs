//! Desde a 0.9.17, Compras só importa com o CNPJ da loja salvo e válido. Para
//! os testes que importam a nota de exemplo: salva, pela tela de
//! Configurações (`config_salvar`, com a gerente conectada), o CNPJ do
//! destinatário dela.

use serde_json::{json, Value};

pub const DA_NOTA_DE_EXEMPLO: &str = "11.444.777/0001-61";

/// `ok` chama um comando e devolve os dados (falha o teste se não deu certo).
pub fn salvar(ok: &mut dyn FnMut(&str, Value) -> Value) {
    let c = ok("config_ler", json!({}));
    let tabelas: Vec<Value> = c["tabelas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({ "numero": t["numero"], "nome": t["nome"], "lucroPadrao": t["lucroPadrao"] }))
        .collect();
    ok(
        "config_salvar",
        json!({ "loja": c["loja"], "caixa": c["caixa"], "cnpj": DA_NOTA_DE_EXEMPLO, "uf": c["uf"],
                "endereco": c["endereco"], "telefone": c["telefone"], "imprimirDireto": c["imprimirDireto"],
                "tabelas": tabelas, "crediarioMulta": c["crediarioMulta"], "crediarioJuros": c["crediarioJuros"],
                "crediarioCarencia": c["crediarioCarencia"], "orcamentoValidade": c["orcamentoValidade"],
                "rodapeVenda": c["rodapeVenda"], "rodapeOrcamento": c["rodapeOrcamento"], "rodapeOs": c["rodapeOs"],
                "imprimirLocal": c["imprimirLocal"] }),
    );
}
