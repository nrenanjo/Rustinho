//! D-09 — O caixa e os custos (decisão do dono, 21/10/2026):
//!
//! 1. no cadastro e na consulta de peças, o caixa não vê custo nem % de lucro;
//! 2. na compra, vê e informa o custo (para conferir e cadastrar peças novas),
//!    sem o % de lucro;
//! 3. o Rustinho calcula o preço pelo custo e pelas regras, sem mostrar a margem;
//! 4. mudar as regras de preço pede a senha do gerente;
//! 5. relatórios, rankings e indicadores de custo, lucro e margem continuam só
//!    para quem pode ver os resultados;
//! 6. o caixa continua cadastrando peças e lançando compras.
//!
//! E a senha do gerente digitada no aperto vale só para o que foi pedido, e
//! a de uma ação só acaba quando é usada.

use serde_json::{json, Value};

use crate::comum::*;

/// Caminhos (no JSON) de campos de custo, lucro ou margem com valor.
fn custos_a_mostra(v: &Value, caminho: &str, achados: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let chave = k.to_lowercase();
                let sensivel = chave.contains("lucro") || chave.contains("margem") || chave == "custo";
                let tem_valor = match x {
                    Value::String(s) => !s.is_empty(),
                    Value::Number(_) => true,
                    _ => false,
                };
                if sensivel && tem_valor {
                    achados.push(format!("{caminho}.{k} = {x}"));
                }
                custos_a_mostra(x, &format!("{caminho}.{k}"), achados);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                custos_a_mostra(x, &format!("{caminho}[{i}]"), achados);
            }
        }
        Value::String(s) if s.starts_with("Custo") => achados.push(format!("{caminho} = {s}")),
        _ => {}
    }
}

fn sem_custos(v: &Value, onde: &str) {
    let mut achados = Vec::new();
    custos_a_mostra(v, onde, &mut achados);
    assert!(achados.is_empty(), "o caixa viu custo ou lucro em {onde}: {achados:?}");
}

/// Custo e % de lucro gravados da peça (centavos; % × 100).
fn gravado(l: &Loja, peca: &str) -> (i64, i64, Option<i64>) {
    let c = l.conexao();
    c.query_row(
        "SELECT p.custo, pp.preco, pp.lucro FROM produto p JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1
          WHERE p.id = ?1",
        [peca],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn loja() -> (Loja, Povo) {
    let mut l = Loja::nova("d09");
    let p = povoar(&mut l);
    // Uma mudança de custo no histórico da peça (como a que a compra deixa).
    let mut f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f["custo"] = json!("62");
    f["precos"][0]["preco"] = json!("100");
    l.ok("produto_salvar", f);
    (l, p)
}

/// D-09.1 — Cadastro e consulta: o caixa não vê custo, % de lucro nem o
/// custo no histórico; o gerente vê tudo. O mesmo na peça nova (o lucro
/// sugerido não vem), nas configurações e no estoque parado.
#[test]
fn d09_1_caixa_nao_ve_custo_no_cadastro() {
    let (mut l, p) = loja();
    l.entrar("Bia", SENHA_BIA);
    let f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    assert_eq!(f["verCustos"], false);
    assert_eq!(f["precos"][0]["preco"], "100,00", "o preço continua à mostra");
    sem_custos(&f, "produto_abrir");
    sem_custos(&l.ok("produto_novo", json!({})), "produto_novo");
    for texto in ["", "filtro"] {
        sem_custos(&l.ok("produtos_buscar", json!({ "texto": texto })), "produtos_buscar");
    }
    let cfg = l.ok("config_ler", json!({}));
    assert_eq!(cfg["verLucro"], false);
    sem_custos(&cfg["tabelas"], "config_ler.tabelas");
    let parados = l.ok("estoque_parados", json!({ "texto": "" }));
    assert_eq!(parados["valor"], "", "dinheiro parado é pelo custo: {parados}");
    assert!(parados["itens"].as_array().unwrap().iter().all(|i| i["valor"] == ""), "{parados}");
    // Peça que veio da compra (custo da nota): também sem custo.
    let da_compra = l.ok("compra_abrir", json!({ "id": p.compra }))["itens"][0]["peca"]["id"].clone();
    sem_custos(&l.ok("produto_abrir", json!({ "id": da_compra })), "peça da compra");

    // O gerente vê.
    l.entrar("Ana", SENHA_ANA);
    let f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    assert_eq!(f["verCustos"], true);
    assert_eq!(f["custo"], "62,00");
    assert_ne!(f["precos"][0]["lucro"], "");
    assert!(f["historico"].to_string().contains("Custo: 60,00 → 62,00"), "{}", f["historico"]);
    assert_eq!(l.ok("produto_novo", json!({}))["precos"][0]["lucro"], "100");
    assert_eq!(l.ok("config_ler", json!({}))["tabelas"][0]["lucroPadrao"], "100");
    assert_ne!(l.ok("estoque_parados", json!({ "texto": "" }))["valor"], "");
}

/// D-09.6 e D-09.1 — O caixa continua cadastrando e alterando peças; o que
/// ele manda não mexe no custo nem no % de lucro, nem por chamada direta.
#[test]
fn d09_2_caixa_cadastra_sem_mexer_no_custo() {
    let (mut l, p) = loja();
    let antes = gravado(&l, &p.filtro);
    assert_eq!(antes.0, 6200);
    l.entrar("Bia", SENHA_BIA);
    // Muda o local e o nome: custo e lucro ficam iguais.
    let mut f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f["local"] = json!("Prateleira 3");
    f["nome"] = json!("Filtro de óleo (caixa azul)");
    let salvo = l.ok("produto_salvar", f);
    sem_custos(&salvo, "produto_salvar");
    assert_eq!(gravado(&l, &p.filtro), antes);
    // Chamada direta tentando mudar custo e % de lucro: ignorados.
    let mut f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f["custo"] = json!("1");
    f["precos"][0]["lucro"] = json!("900");
    f["observacoes"] = json!("conferido");
    l.ok("produto_salvar", f);
    assert_eq!(gravado(&l, &p.filtro), antes, "o caixa mudou custo ou lucro por chamada direta");
    // Peça nova: cadastra com o preço; custo e % que ele mandar não entram.
    let mut f = l.ok("produto_novo", json!({}));
    f["nome"] = json!("Lâmpada H4");
    f["custo"] = json!("10");
    f["precos"][0]["lucro"] = json!("50");
    f["precos"][0]["preco"] = json!("25");
    f["estoqueInicial"] = json!("4");
    let nova = l.ok("produto_salvar", f);
    sem_custos(&nova, "peça nova do caixa");
    let id = nova["id"].as_str().unwrap().to_string();
    assert_eq!(gravado(&l, &id), (0, 2500, None));
    assert_eq!(l.estoque(&id), "4");
    // Peça sem custo: o caixa muda o preço à vontade (não há regra).
    let mut f = l.ok("produto_abrir", json!({ "id": id }));
    f["precos"][0]["preco"] = json!("27");
    l.ok("produto_salvar", f);
    assert_eq!(gravado(&l, &id), (0, 2700, None));
    // Mais um tamanho a partir de uma peça com custo: copia a regra, sem mostrar.
    let v = l.ok("produto_variacao_criar", json!({ "id": p.pastilha, "atual": "Dianteira", "nova": "Traseira" }));
    sem_custos(&v, "produto_variacao_criar");
    let id_v = v["id"].as_str().unwrap().to_string();
    assert_eq!(gravado(&l, &id_v).0, gravado(&l, &p.pastilha).0);
}

/// D-09.2, 3 e 6 — Na compra o caixa vê o custo da nota e o custo de antes,
/// lança, e as peças novas saem com o preço pela regra (lucro sugerido),
/// sem que nenhum % de lucro apareça para ele.
#[test]
fn d09_3_compra_mostra_custo_sem_margem() {
    let mut l = Loja::nova("d09-3");
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let caminho = l.pasta.join("nota.xml");
    std::fs::write(&caminho, XML_COMPRA).unwrap();
    let r = l.ok("compra_importar", json!({ "caminho": caminho.display().to_string() }));
    let compra = r["compras"][0]["id"].as_str().unwrap().to_string();
    let c = l.ok("compra_abrir", json!({ "id": compra }));
    let itens = c["itens"].as_array().unwrap();
    assert!(itens.iter().all(|i| i["custoUnitario"] != ""), "o caixa precisa ver o custo da nota: {c}");
    let mut achados = Vec::new();
    custos_a_mostra(&c, "compra", &mut achados);
    assert!(achados.iter().all(|a| !a.contains("lucro") && !a.contains("margem")), "{achados:?}");
    let escolhas: Vec<Value> = itens
        .iter()
        .map(|it| json!({ "id": it["id"], "acao": "nova", "produtoId": "", "fator": "1", "baixarPreco": false }))
        .collect();
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": compra, "situacao": "autorizada", "protocolo": "135260000000001",
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    let lancada = l.ok("compra_lancar", json!({ "id": compra, "itens": escolhas }));
    let itens = lancada["compra"]["itens"].as_array().unwrap().clone();
    for it in &itens {
        let peca = &it["peca"];
        assert_ne!(peca["custoAtual"], "", "o custo da peça nova aparece na compra: {it}");
        assert!(peca.get("lucro").is_none() && peca.get("margem").is_none());
        // Regra: lucro sugerido de 100% → preço = 2 × custo, gravado com o %.
        let (custo, preco, lucro) = gravado(&l, peca["id"].as_str().unwrap());
        assert!(custo > 0);
        assert_eq!(lucro, Some(10_000));
        assert_eq!(preco, crate::comum::centavos(peca["precoAtual"].as_str().unwrap()));
        assert!((preco - 2 * custo).abs() <= 1, "preço {preco} para custo {custo}");
        // No cadastro, a mesma peça não mostra o custo ao caixa.
        sem_custos(&l.ok("produto_abrir", json!({ "id": peca["id"] })), "peça nova da compra");
    }
}

/// D-09.4 — Regras de preço só com a senha do gerente: o % de lucro da peça
/// (o caixa mudar o preço de uma peça com custo muda o %) e o lucro
/// sugerido das tabelas. A senha vale para um salvar só.
#[test]
fn d09_4_regras_de_preco_pedem_o_gerente() {
    let (mut l, p) = loja();
    let antes = gravado(&l, &p.filtro);
    l.entrar("Bia", SENHA_BIA);
    let mut f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f["precos"][0]["preco"] = json!("120");
    let r = l.chamar("produto_salvar", f.clone());
    assert_eq!(r["ok"], false);
    assert_eq!(r["campo"], "precos", "{r}");
    assert_eq!(gravado(&l, &p.filtro), antes, "a recusa não pode gravar nada");
    // Senha do gerente para isso: salva uma vez.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": true, "acao": "precos" }));
    l.ok("produto_salvar", f);
    let depois = gravado(&l, &p.filtro);
    assert_eq!((depois.0, depois.1), (6200, 12000));
    assert_ne!(depois.2, antes.2, "o % segue o preço");
    // A senha já foi usada: outra mudança pede de novo.
    let mut f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f["precos"][0]["preco"] = json!("130");
    assert_eq!(l.chamar("produto_salvar", f)["campo"], "precos");
    assert_eq!(gravado(&l, &p.filtro), depois);
    // Tabela 2 com preço novo numa peça com custo também é regra.
    let mut f = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f["precos"][1]["preco"] = json!("95");
    assert_eq!(l.chamar("produto_salvar", f)["campo"], "precos");

    // Lucro sugerido das tabelas: o caixa (com a senha do gerente para as
    // Configurações) não vê e o que ele mandar fica como estava.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": false, "acao": "configuracoes" }));
    let mut cfg = l.ok("config_ler", json!({}));
    let tabelas: Vec<Value> = cfg["tabelas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({ "numero": t["numero"], "nome": t["nome"], "lucroPadrao": "300" }))
        .collect();
    cfg["tabelas"] = json!(tabelas);
    l.ok("config_salvar", cfg.clone());
    assert_eq!(l.numero("SELECT lucro_padrao FROM tabela_preco WHERE numero = 1"), 10_000);
    l.ok("gerente_encerrar", json!({}));

    // O gerente muda: grava e fica na auditoria.
    l.entrar("Ana", SENHA_ANA);
    l.ok("config_salvar", cfg);
    assert_eq!(l.numero("SELECT lucro_padrao FROM tabela_preco WHERE numero = 1"), 30_000);
    assert_eq!(l.numero("SELECT COUNT(*) FROM auditoria WHERE acao = 'precos.regras'"), 1);
    // A regra nova vale para a próxima peça.
    assert_eq!(l.ok("produto_novo", json!({}))["precos"][0]["lucro"], "300");

    // Loja que libera os resultados para todos: o caixa vê custo e %, mas
    // mudar a regra continua pedindo o gerente (a pílula não tem marca para isso).
    l.ok("gerente_salvar", json!({ "acoes": { "resultados": false } }));
    let g = l.ok("gerente_ler", json!({}));
    assert!(g["acoes"].as_array().unwrap().iter().all(|a| a["id"] != "precos"), "{g}");
    l.entrar("Bia", SENHA_BIA);
    let mut f = l.ok("produto_abrir", json!({ "id": p.pastilha }));
    assert_eq!(f["verCustos"], true);
    assert_eq!(f["custo"], "40,00");
    f["precos"][0]["lucro"] = json!("150");
    f["precos"][0]["preco"] = json!("100");
    assert_eq!(l.chamar("produto_salvar", f.clone())["campo"], "precos");
    let mut cfg = l.ok("config_ler", json!({}));
    cfg["tabelas"][0]["lucroPadrao"] = json!("80");
    // config_salvar é das Configurações: com a senha só para a regra de preço,
    // não passa (cada senha vale para o que foi pedido).
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": true, "acao": "precos" }));
    assert_eq!(l.chamar("config_salvar", cfg)["campo"], "gerente");
    l.ok("produto_salvar", f);
    assert_eq!(gravado(&l, &p.pastilha).1, 10_000);
}

/// D-09 — A senha do gerente não fica aberta para outras operações: vale
/// só para a ação pedida; a de uma ação só acaba quando dá certo (não quando
/// dá erro); a de uma tela não vale para outra coisa (estornar no
/// Financeiro pede de novo); "encerrar" acaba com todas; não dá para pedir
/// a senha sem dizer para quê.
#[test]
fn d09_5_senha_do_gerente_nao_fica_aberta() {
    let (mut l, p) = loja();
    l.entrar("Bia", SENHA_BIA);
    l.abrir_caixa();
    let v = id(&l.vender(&[(&p.filtro, "1")], &[("dinheiro", "100")]));
    let ajuste = json!({ "produtoId": p.filtro, "quantidade": "3", "motivo": "contagem" });

    assert!(l.erro("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": true })).contains("para que"));
    assert!(l.erro("gerente_liberar", json!({ "senha": SENHA_ANA, "acao": "tudo" })).contains("para que"));

    // Para cancelar uma venda: não serve para ajustar o estoque.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": true, "acao": "cancelar_venda" }));
    assert_eq!(l.ok("permissao", json!({ "acao": "cancelar_venda" }))["pode"], true);
    assert_eq!(l.ok("permissao", json!({ "acao": "ajustar_estoque" }))["pode"], false);
    assert!(l.erro("estoque_ajustar", ajuste.clone()).contains("Só o gerente"));
    // Erro (sem motivo) não gasta a senha; dar certo gasta.
    l.erro("venda_cancelar", json!({ "id": v, "motivo": "" }));
    l.ok("venda_cancelar", json!({ "id": v, "motivo": "cliente desistiu" }));
    assert_eq!(l.ok("permissao", json!({ "acao": "cancelar_venda" }))["pode"], false);
    assert!(l.erro("venda_cancelar", json!({ "id": p.v2, "motivo": "de novo" })).contains("Só o gerente"));

    // Tela do Financeiro: vale para o Financeiro (várias vezes), não para estornar.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": false, "acao": "financeiro" }));
    l.ok("financeiro_resumo", json!({}));
    l.ok("pagar_painel", json!({}));
    l.ok("contas_loja", json!({}));
    let pago: String =
        l.conexao().query_row("SELECT id FROM pagamento WHERE estornado_em IS NULL LIMIT 1", [], |r| r.get(0)).unwrap();
    let m = l.erro("pagamento_estornar", json!({ "id": pago, "motivo": "errado" }));
    assert!(m.contains("Só o gerente"), "{m}");
    assert!(l.erro("estoque_ajustar", ajuste.clone()).contains("Só o gerente"));
    assert!(l.erro("dre_apurar", json!({ "periodo": "mes" })).contains("Só o gerente"));
    // Senha só para estornar, dentro do Financeiro: estorna uma vez; o
    // Financeiro continua aberto.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": true, "acao": "estornar" }));
    l.ok("pagamento_estornar", json!({ "id": pago, "motivo": "errado" }));
    assert_eq!(l.ok("permissao", json!({ "acao": "estornar" }))["pode"], false);
    l.ok("financeiro_resumo", json!({}));
    // Sair da tela (encerrar) acaba com a do Financeiro.
    l.ok("gerente_encerrar", json!({}));
    assert!(l.erro("financeiro_resumo", json!({})).contains("Só o gerente"));

    // Configurações: a tela vale para o backup e a pílula Gerente (estão
    // nela), não para cancelar venda nem para a regra de preço.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": false, "acao": "configuracoes" }));
    l.ok("gerente_ler", json!({}));
    l.ok("backup_ler", json!({}));
    assert_eq!(l.ok("permissao", json!({ "acao": "precos" }))["pode"], false);
    assert_eq!(l.ok("permissao", json!({ "acao": "cancelar_venda" }))["pode"], false);
    l.ok("gerente_encerrar", json!({}));
    assert_eq!(l.ok("permissao", json!({ "acao": "backup" }))["pode"], false);

    // Trocar de usuário acaba com qualquer senha.
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "curta": true, "acao": "ajustar_estoque" }));
    l.entrar("Bia", SENHA_BIA);
    assert_eq!(l.ok("permissao", json!({ "acao": "ajustar_estoque" }))["pode"], false);

    // Cada liberação fica na auditoria com o que foi liberado.
    let c = l.conexao();
    let detalhes: Vec<String> = c
        .prepare("SELECT detalhes FROM auditoria WHERE acao = 'gerente.liberar' ORDER BY rowid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(detalhes.len(), 5, "{detalhes:?}");
    assert!(detalhes[0].contains("cancelar venda") && detalhes[0].contains("uma vez"), "{detalhes:?}");
    assert!(detalhes[1].contains("nesta tela"), "{detalhes:?}");
}

/// D-09.5 — Relatórios e indicadores de custo, lucro e margem continuam só
/// para quem pode ver os resultados, por qualquer porta.
#[test]
fn d09_6_relatorios_de_custo_continuam_fechados() {
    let (mut l, _p) = loja();
    l.entrar("Bia", SENHA_BIA);
    let r = l.ok("painel_resumo", json!({}));
    assert_eq!(r["lucroMes"], "");
    assert_eq!(r["margemMes"], "");
    let mv = l.ok("estoque_mais_vendidos", json!({ "dias": 30, "ordem": "lucro", "texto": "" }));
    assert_eq!(mv["lucro"], "");
    assert!(mv["itens"].as_array().unwrap().iter().all(|i| i["lucro"] == "" && i["margem"] == ""));
    for c in ["dre_apurar", "dre_indicadores", "dre_serie", "dre_resumo", "dre_detalhe", "dre_metas"] {
        let m = l.erro(c, json!({ "filtro": { "periodo": "mes" }, "periodo": "mes", "linha": "receita_bruta" }));
        assert!(m.contains("Só o gerente"), "{c}: {m}");
    }
}
