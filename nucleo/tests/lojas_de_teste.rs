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

/// Chamadas com a senha do gerente (dev.3.3, passo 2.1: H11 e H12) — a loja do
/// roteiro `app/testes/telas_chamadas_gerente.py`:
/// - Ana (gerente, senha 1234) e o caixa aberto;
/// - a cliente "Oficina do Zé" com R$ 300,00 no crediário (3 parcelas) e um
///   recebimento de R$ 100,00 (o que o roteiro estorna);
/// - o backup apontado para `<pasta>/backups` (o roteiro troca a pasta por um
///   arquivo para provocar o erro).
///
/// Grava `<pasta>/ids.json` com o id da cliente, para o roteiro abrir a conta dela.
#[test]
#[ignore = "só à mão: monta a loja do roteiro das chamadas com a senha do gerente (H11, H12)"]
fn montar_chamadas_gerente() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&pasta).unwrap();
    let mut l = Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                "senha": SENHA_ANA, "senha2": SENHA_ANA }),
    );
    let cliente = l.pessoa("cliente", "Oficina do Zé");
    l.ok(
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno de setembro", "valor": "300", "parcelas": 3,
                "primeiroVencimento": "" }),
    );
    l.ok("caixa_abrir", json!({ "valor": "50" }));
    let conta = l.ok("receber_cliente", json!({ "id": cliente, "todas": false }));
    let parcela = conta["parcelas"][0]["id"].as_str().unwrap().to_string();
    l.ok("receber_baixar", json!({ "clienteId": cliente, "contas": [parcela], "valor": "100", "forma": "dinheiro" }));
    let backups = pasta.join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    let b = l.ok("backup_ler", json!({}));
    l.ok("backup_salvar", json!({ "pasta": backups.display().to_string(), "hora": b["hora"], "dias": b["dias"] }));
    std::fs::write(pasta.join("ids.json"), json!({ "cliente": cliente }).to_string()).unwrap();
    l.fechar();
}

/// Números das telas (dev.3.3, passo 2.2: H13, ARQ-09) — a loja do roteiro
/// `app/testes/telas_numeros.py`: Ana (gerente, senha 1234), o caixa aberto e
/// uma peça vendida a quilo com o código de barras 7891000000014 (entra direto no carrinho).
#[test]
#[ignore = "só à mão: monta a loja do roteiro dos números das telas (H13, ARQ-09)"]
fn montar_numeros_das_telas() {
    let Some(pasta) = std::env::var_os("RUSTINHO_LOJA_TESTE").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&pasta).unwrap();
    let mut l = Loja::abrir_em(&pasta, false);
    l.ok(
        "configurar_inicio",
        json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                "senha": SENHA_ANA, "senha2": SENHA_ANA }),
    );
    let mut f = l.ok("produto_novo", json!({}));
    f["nome"] = json!("PARAFUSO SINTETICO A GRANEL");
    f["codigoBarras"] = json!("7891000000014");
    f["unidade"] = json!("KG");
    f["estoqueInicial"] = json!("100");
    f["precos"][0]["preco"] = json!("2,00");
    f["precos"][0]["lucro"] = json!("");
    l.ok("produto_salvar", f);
    l.ok("caixa_abrir", json!({ "valor": "50" }));
    l.fechar();
}
