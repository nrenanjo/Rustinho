//! Contratos entre o núcleo e as telas, parte 2 (R-11 / C-14): os CAMPOS que
//! as telas de compra por XML, receber, pagar, devolução, ordem de serviço e
//! Gerente › Códigos realmente leem. A lista de campos de cada tela foi tirada
//! dos arquivos `app/qml` (`tela.compra.*`, `modelData.*`, `painel.*`...). Se
//! um campo mudar de nome ou de tipo no núcleo, este teste falha antes de a
//! tela quebrar. (O envelope e os fluxos de entrada, venda, backup e troca de
//! dados continuam em `contratos.rs`.)
//!
//! Limite: confere o que as telas leem hoje, não toda a API. Uma tela nova ou
//! um campo novo na tela pede uma linha nova aqui.

use rustinho_nucleo::Servico;
use serde_json::{json, Value};

fn chamar(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let texto = s.chamar(comando, &dados.to_string());
    serde_json::from_str(&texto).expect("a resposta é JSON")
}

fn ok(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let v = chamar(s, comando, dados.clone());
    assert_eq!(v["ok"], true, "{comando} {dados}: {v}");
    v["dados"].clone()
}

/// Os campos existem e têm o tipo que a tela espera.
fn tem(onde: &str, v: &Value, campos: &[(&str, &str)]) {
    for (nome, tipo) in campos {
        let c = v.get(*nome).unwrap_or_else(|| panic!("{onde}: falta '{nome}' em {v}"));
        let certo = match *tipo {
            "texto" => c.is_string(),
            "numero" => c.is_number(),
            "sim/nao" => c.is_boolean(),
            "lista" => c.is_array(),
            "objeto" => c.is_object(),
            "objeto|nada" => c.is_object() || c.is_null(),
            "texto|nada" => c.is_string() || c.is_null(),
            outro => panic!("tipo desconhecido {outro}"),
        };
        assert!(certo, "{onde}: '{nome}' devia ser {tipo}: {c}");
    }
}

fn loja() -> Servico {
    let mut s = Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja Teste", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    s
}

fn pessoa(s: &mut Servico, papel: &str, nome: &str) -> String {
    let mut f = ok(s, "pessoa_nova", json!({ "papel": papel }));
    f["nome"] = json!(nome);
    ok(s, "pessoa_salvar", f)["id"].as_str().unwrap().to_string()
}

const XML: &str = include_str!("nfe_exemplo.xml");

fn arquivo_xml(nome: &str) -> String {
    let p = std::env::temp_dir().join(format!("rustinho-contrato-{nome}-{}.xml", std::process::id()));
    std::fs::write(&p, XML).unwrap();
    p.display().to_string()
}

/// Campos que `TelaCompras`/`LinhaCompra` e `TelaCompra`/`LinhaItemCompra` leem.
#[test]
fn compra_por_xml() {
    let mut s = loja();
    let caminho = arquivo_xml("compra");
    let r = ok(&mut s, "compra_importar", json!({ "caminho": caminho }));
    let _ = std::fs::remove_file(&caminho);
    tem("compra_importar", &r, &[("compras", "lista"), ("erros", "lista")]);
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    let id = r["compras"][0]["id"].as_str().unwrap().to_string();

    // Lista (LinhaCompra).
    let lista = ok(&mut s, "compras_listar", json!({ "texto": "", "limite": 300 }));
    assert_eq!(lista.as_array().unwrap().len(), 1);
    tem(
        "compras_listar",
        &lista[0],
        &[
            ("id", "texto"),
            ("fornecedor", "texto"),
            ("numero", "texto"),
            ("emissao", "texto"),
            ("total", "texto"),
            ("itens", "numero"),
            ("pendentes", "numero"),
            ("situacao", "texto"),
        ],
    );

    // Ficha (TelaCompra).
    let c = ok(&mut s, "compra_abrir", json!({ "id": id }));
    tem(
        "compra_abrir",
        &c,
        &[
            ("id", "texto"),
            ("situacao", "texto"),
            ("fornecedor", "texto"),
            ("fornecedorDocumento", "texto"),
            ("fornecedorCidade", "texto"),
            ("chave", "texto"),
            ("numero", "texto"),
            ("natureza", "texto"),
            ("totalProdutos", "texto"),
            ("frete", "texto"),
            ("ipi", "texto"),
            ("icmsSt", "texto"),
            ("desconto", "texto"),
            ("total", "texto"),
            ("freteExtra", "texto"),
            ("avisoDestinatario", "texto"),
            // R-01: o aviso do limite da conferência do XML chega à tela.
            ("avisoConferencia", "texto"),
            ("itens", "lista"),
            ("pendentes", "numero"),
            ("novas", "numero"),
            ("ignorados", "numero"),
            ("custoTotal", "texto"),
            ("duplicatas", "lista"),
            ("parcelasPagar", "numero"),
        ],
    );
    assert!(c["avisoConferencia"].as_str().unwrap().contains("assinatura digital"));
    // Itens (LinhaItemCompra).
    assert_eq!(c["itens"].as_array().unwrap().len(), 3);
    for it in c["itens"].as_array().unwrap() {
        tem(
            "compra item",
            it,
            &[
                ("ordem", "numero"),
                ("codigoFornecedor", "texto"),
                ("gtin", "texto"),
                ("descricao", "texto"),
                ("ncm", "texto"),
                ("cest", "texto"),
                ("unidade", "texto"),
                ("quantidade", "texto"),
                ("valorUnitario", "texto"),
                ("valorProdutos", "texto"),
                ("acao", "texto"),
                ("achadoPor", "texto"),
                ("fator", "texto"),
                ("quantidadePecas", "texto"),
                ("custoUnitario", "texto"),
                ("monofasico", "sim/nao"),
                ("peca", "objeto|nada"),
            ],
        );
    }
    // Lançar, e a peça ligada (quando há) tem os campos que a linha lê.
    // As escolhas da tela: cadastrar cada item como peça nova.
    let escolhas: Vec<Value> = c["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| json!({ "id": it["id"], "acao": "nova", "produtoId": "", "fator": "1", "baixarPreco": false }))
        .collect();
    let l = ok(&mut s, "compra_lancar", json!({ "id": id, "itens": escolhas }));
    tem("compra_lancar", &l, &[("resumo", "objeto|nada"), ("compra", "objeto")]);
    let depois = &l["compra"];
    assert_eq!(depois["situacao"], "lancada");
    for it in depois["itens"].as_array().unwrap() {
        if let Some(p) = it.get("peca").filter(|p| p.is_object()) {
            tem(
                "compra item.peca",
                p,
                &[
                    ("id", "texto"),
                    ("codigo", "texto"),
                    ("nome", "texto"),
                    ("marca", "texto"),
                    ("unidade", "texto"),
                    ("custoAtual", "texto"),
                    ("precoAtual", "texto"),
                ],
            );
        }
    }
}

/// Campos que `TelaReceber` lê do painel, da conta do cliente e da prévia.
#[test]
fn receber() {
    let mut s = loja();
    let cliente = pessoa(&mut s, "cliente", "Oficina do Zé");
    let previa = ok(
        &mut s,
        "receber_previa",
        json!({ "clienteId": cliente, "valor": "300", "parcelas": 3, "primeiroVencimento": "" }),
    );
    assert!(previa.is_array() || previa.is_object(), "{previa}");
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno de setembro", "valor": "300", "parcelas": 3, "primeiroVencimento": "" }),
    );
    let p = ok(&mut s, "receber_painel", json!({ "texto": "" }));
    tem(
        "receber_painel",
        &p,
        &[
            ("clientes", "lista"),
            ("total", "texto"),
            ("vencido", "texto"),
            ("venceHoje", "texto"),
            ("vence7Dias", "texto"),
        ],
    );
    assert_eq!(p["clientes"].as_array().unwrap().len(), 1, "{p}");
    tem(
        "receber_painel.clientes",
        &p["clientes"][0],
        &[
            ("clienteId", "texto"),
            ("nome", "texto"),
            ("telefone", "texto"),
            ("emAberto", "texto"),
            ("vencido", "texto"),
            ("parcelas", "numero"),
            ("proximoVencimento", "texto"),
            ("diasAtraso", "numero"),
        ],
    );
    // Conta do cliente (tela.conta.*).
    let c = ok(&mut s, "receber_cliente", json!({ "id": cliente, "todas": false }));
    tem(
        "receber_cliente",
        &c,
        &[
            ("nome", "texto"),
            ("documento", "texto"),
            ("telefone", "texto"),
            ("parcelas", "lista"),
            ("recebimentos", "lista"),
            ("situacao", "objeto"),
        ],
    );
    tem(
        "receber_cliente.situacao",
        &c["situacao"],
        &[
            ("emAberto", "texto"),
            ("vencido", "texto"),
            ("limite", "texto|nada"),
            ("disponivel", "texto|nada"),
            ("diaVencimento", "texto|nada"),
        ],
    );
    assert_eq!(c["parcelas"].as_array().unwrap().len(), 3);
    // Receber uma parcela (precisa de caixa aberto): o recibo tem o que a tela mostra.
    ok(&mut s, "caixa_abrir", json!({ "valor": "50" }));
    let conta = c["parcelas"][0]["id"].as_str().unwrap().to_string();
    let recibo = ok(
        &mut s,
        "receber_baixar",
        json!({ "clienteId": cliente, "contas": [conta], "valor": "100", "forma": "dinheiro" }),
    );
    tem(
        "receber_baixar (recibo)",
        &recibo,
        &[("id", "texto"), ("cliente", "texto"), ("valor", "texto"), ("troco", "texto"), ("emAberto", "texto")],
    );
    // O cálculo de um recebimento.
    let calc = ok(&mut s, "receber_calcular", json!({ "clienteId": cliente, "itens": [], "pagamentos": [] }));
    tem("receber_calcular", &calc, &[("total", "texto"), ("juros", "texto"), ("erro", "texto")]);
}

/// Campos que `SecaoPagar` e `LinhaContaPagar` leem.
#[test]
fn pagar() {
    let mut s = loja();
    let fornecedor = pessoa(&mut s, "fornecedor", "Distribuidora Vale");
    let ids = ok(
        &mut s,
        "pagar_lancar",
        json!({ "fornecedorId": fornecedor, "categoriaId": "", "descricao": "Duplicata 123", "documento": "123",
                "valor": "500", "parcelas": 2, "primeiroVencimento": "", "observacoes": "" }),
    );
    let id = ids[0].as_str().unwrap().to_string();
    let p = ok(&mut s, "pagar_painel", json!({ "texto": "", "quais": "abertas" }));
    tem(
        "pagar_painel",
        &p,
        &[
            ("parcelas", "lista"),
            ("total", "texto"),
            ("vencido", "texto"),
            ("venceHoje", "texto"),
            ("vence7Dias", "texto"),
        ],
    );
    assert!(!p["parcelas"].as_array().unwrap().is_empty());
    tem(
        "pagar_painel.parcelas",
        &p["parcelas"][0],
        &[
            ("descricao", "texto"),
            ("documento", "texto"),
            ("fornecedor", "texto"),
            ("categoria", "texto"),
            ("parcela", "texto"),
            ("valor", "texto"),
            ("pago", "texto"),
            ("saldo", "texto"),
            ("vencimento", "texto"),
            ("diasAtraso", "numero"),
            ("situacao", "texto"),
        ],
    );
    let f = ok(&mut s, "pagar_ficha", json!({ "id": id }));
    assert!(f.is_object(), "{f}");
}

/// Campos que `TelaOrdens` e `TelaOrdem` leem.
#[test]
fn ordem_de_servico() {
    let mut s = loja();
    let r = ok(
        &mut s,
        "os_salvar",
        json!({ "clienteNome": "Maria", "telefone": "(15) 99999-0000", "placa": "ABC1D23", "veiculo": "Gol 1.0",
                "km": "100000", "defeito": "Barulho na frente", "itens": [] }),
    );
    tem(
        "os_salvar (ficha)",
        &r,
        &[
            ("id", "texto"),
            ("numero", "numero"),
            ("situacao", "texto"),
            ("placa", "texto"),
            ("cliente", "texto"),
            ("veiculo", "texto"),
            ("atrasada", "sim/nao"),
            ("historico", "lista"),
        ],
    );
    let l = ok(&mut s, "os_listar", json!({ "quais": "abertas", "texto": "" }));
    tem("os_listar", &l, &[("itens", "lista"), ("contagem", "objeto")]);
    tem("os_listar.contagem", &l["contagem"], &[("abertas", "numero"), ("prontas", "numero"), ("atrasadas", "numero")]);
    assert_eq!(l["itens"].as_array().unwrap().len(), 1, "{l}");
    tem(
        "os_listar.itens",
        &l["itens"][0],
        &[
            ("id", "texto"),
            ("numero", "numero"),
            ("placa", "texto"),
            ("cliente", "texto"),
            ("veiculo", "texto"),
            ("situacao", "texto"),
            ("nomeSituacao", "texto"),
            ("atrasada", "sim/nao"),
            ("total", "texto"),
        ],
    );
    let c = ok(&mut s, "os_calcular", json!({ "itens": [] }));
    tem("os_calcular", &c, &[("erro", "texto"), ("itens", "lista"), ("total", "texto")]);
}

/// Campos que `TelaDevolucao` lê da venda.
#[test]
fn devolucao() {
    let mut s = loja();
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Pastilha");
    f["custo"] = json!("40");
    f["estoqueInicial"] = json!("5");
    let p = ok(&mut s, "produto_salvar", f);
    let pid = p["id"].as_str().unwrap().to_string();
    ok(&mut s, "caixa_abrir", json!({ "valor": "50" }));
    let v = ok(
        &mut s,
        "venda_concluir",
        json!({ "itens": [{ "produtoId": pid, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
    );
    let venda_id =
        v["id"].as_str().or_else(|| v["vendaId"].as_str()).unwrap_or_else(|| panic!("sem id: {v}")).to_string();
    let d = ok(&mut s, "devolucao_venda", json!({ "id": venda_id }));
    tem(
        "devolucao_venda",
        &d,
        &[
            ("numero", "numero"),
            ("situacao", "texto"),
            ("total", "texto"),
            ("cliente", "texto"),
            ("itens", "lista"),
            ("crediarioAberto", "texto"),
        ],
    );
}

/// Campos que Gerente › Códigos (`TelaConfiguracoes`) lê de `codigos_situacao`,
/// incluindo a lista de clientes e fornecedores esperando confirmação (PEND-07).
#[test]
fn gerente_codigos() {
    let mut s = loja();
    let c = ok(&mut s, "codigos_situacao", json!({}));
    tem(
        "codigos_situacao",
        &c,
        &[
            ("bloqueio", "objeto|nada"),
            ("distribuidor", "objeto|nada"),
            ("restantes", "numero"),
            ("cruzadas", "lista"),
            ("pendentes", "numero"),
            ("pendentesDeste", "numero"),
            ("pessoasPendentes", "numero"),
            ("pessoasRepetidas", "lista"),
            ("pessoasEsperando", "lista"),
            ("esperando", "lista"),
            ("repetidos", "lista"),
        ],
    );
    assert!(c.get("inventario").is_some(), "falta 'inventario': {c}");
}

/// Campos que `TelaPessoa` e `LinhaPessoa` leem, incluindo a marca de código
/// aguardando confirmação (PEND-07).
#[test]
fn cliente_e_fornecedor() {
    let mut s = loja();
    let id = pessoa(&mut s, "cliente", "Mecânica Central");
    let f = ok(&mut s, "pessoa_abrir", json!({ "id": id }));
    tem(
        "pessoa_abrir",
        &f,
        &[
            ("id", "texto"),
            ("codigo", "texto"),
            ("codigoPendente", "sim/nao"),
            ("codigoAviso", "texto"),
            ("compras", "numero"),
            ("atualizadoEm", "texto"),
            ("atualizadoPor", "texto"),
        ],
    );
    assert_eq!(f["codigoPendente"], false, "loja de um caixa só confirma na hora");
    let l = ok(
        &mut s,
        "pessoas_buscar",
        json!({ "texto": "mecânica", "papel": "cliente", "inativos": false, "limite": 20 }),
    );
    tem("pessoas_buscar", &l, &[("itens", "lista"), ("total", "numero")]);
    assert_eq!(l["itens"].as_array().unwrap().len(), 1, "{l}");
    tem(
        "pessoas_buscar.itens",
        &l["itens"][0],
        &[
            ("id", "texto"),
            ("codigo", "texto"),
            ("codigoPendente", "sim/nao"),
            ("nome", "texto"),
            ("documento", "texto"),
            ("telefone", "texto"),
            ("cidade", "texto"),
            ("cliente", "sim/nao"),
            ("fornecedor", "sim/nao"),
            ("ativo", "sim/nao"),
            ("veiculos", "texto"),
        ],
    );
}
