//! Lojas de teste montadas PELO NÚCLEO (pelos mesmos comandos das telas), para
//! os roteiros de tela. Nada de SQL direto nem funções falsas: a loja fica como
//! o Rustinho a deixaria. Só à mão (os roteiros chamam):
//!
//! `RUSTINHO_LOJA_TESTE=/pasta cargo test -p rustinho-nucleo --test lojas_de_teste -- --ignored --exact montar_lista_de_pecas`
//!
//! Dados de mentira.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use comum::*;
use serde_json::json;

const LOJA_CNPJ: &str = "11444777000161";
const EXEMPLO: &str = include_str!("nfe_exemplo.xml");

/// K06 (dev.3.3, passo 1.3) — a loja do roteiro `app/testes/telas_lista_pecas.py`:
/// - Ana (gerente, senha 1234);
/// - 120 peças: 1 sem marca; 2 a correia que fica com o código de fornecedor
///   `FAG-32206`; 3 com marca e código original; 4 a 60 "FILTRO…"; 61 a 120
///   "PASTILHA…" (as de 4 a 120 com marca e código original);
/// - a nota de exemplo trazida só com o fornecedor (3 itens viram pendências) e a
///   pendência `FAG-32206` resolvida ligando à peça 2 (ficam 2 pendências).
#[test]
#[ignore = "só à mão: monta a loja do roteiro da lista de peças (K06)"]
fn montar_lista_de_pecas() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&pasta).unwrap();
    let mut l = Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                "senha": SENHA_ANA, "senha2": SENHA_ANA }),
    );
    l.executar(&format!("INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '{LOJA_CNPJ}')"));

    let xml = pasta.join("xml-da-loja-de-teste");
    std::fs::create_dir_all(&xml).unwrap();
    std::fs::write(xml.join("compra.xml"), EXEMPLO).unwrap();
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [xml.display().to_string()], "clientes": false, "fornecedores": true,
                "pecasLoja": false, "pecasFornecedor": false }),
    );

    let mut correia = String::new();
    for i in 1..=120 {
        let (nome, marca, original) = match i {
            1 => ("PECA SEM MARCA NEM APLICACOES SINTETICA".to_string(), String::new(), String::new()),
            2 => ("CORREIA DENTADA MOTOR DAYCO SINTETICA".to_string(), "DAYCO".to_string(), "04E109119C".to_string()),
            3 => {
                ("AMORTECEDOR DIANTEIRO PRESSURIZADO TURBOGAS".to_string(), "COFAP".to_string(), "GP32488".to_string())
            }
            _ => (
                if i <= 60 {
                    format!("FILTRO DE AR MOTOR SINTETICO MOD {i:02}")
                } else {
                    format!("PASTILHA DE FREIO CERAMICA SINTETICA MOD {i:02}")
                },
                if i % 2 == 0 { "GENERICA".to_string() } else { "BOSCH".to_string() },
                format!("ORIG-{i:04}"),
            ),
        };
        let mut f = l.ok("produto_novo", json!({}));
        f["nome"] = json!(nome);
        f["marca"] = json!(marca);
        f["codigoOriginal"] = json!(original);
        f["local"] = json!("A-01");
        f["custo"] = json!("");
        f["estoqueInicial"] = json!("10");
        f["precos"][0]["preco"] = json!("150,00");
        f["precos"][0]["lucro"] = json!("");
        let id = l.ok("produto_salvar", f)["id"].as_str().unwrap().to_string();
        if i == 2 {
            correia = id;
        }
    }

    let lista = l.ok("xml_pendencias_listar", json!({}));
    let pend = lista.as_array().unwrap().iter().find(|p| p["codigoFornecedor"] == "FAG-32206").unwrap()["id"].clone();
    l.ok("xml_pendencia_resolver", json!({ "id": pend, "acao": "ligar", "produtoId": correia }));
    l.fechar();
}

/// Geometria (dev.3.3, passo 1.3) — a loja do roteiro `app/testes/geometria_telas.py`:
/// Ana (gerente) e mais quatro usuários (5 na tela de entrada, como na revisão
/// visual da auditoria, VIS33-06) e algumas peças.
#[test]
#[ignore = "só à mão: monta a loja do roteiro de geometria"]
fn montar_geometria() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&pasta).unwrap();
    let mut l = Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                "senha": SENHA_ANA, "senha2": SENHA_ANA }),
    );
    for nome in ["Bia", "Carlos", "Dani", "Eduarda"] {
        l.usuario_caixa(nome, "1111");
    }
    for i in 1..=12 {
        l.peca(&format!("PECA DE GEOMETRIA {i:02}"), "", "10,00", "5");
    }
    l.fechar();
}
