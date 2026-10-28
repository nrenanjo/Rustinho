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
    // Mudar o preço depois, mesmo sem custo, é do gerente (veja d09_7).
    let mut f = l.ok("produto_abrir", json!({ "id": id }));
    f["precos"][0]["preco"] = json!("27");
    assert_eq!(l.chamar("produto_salvar", f)["campo"], "precos");
    assert_eq!(gravado(&l, &id), (0, 2500, None));
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

/// A senha do gerente para esta gravação, com estes dados exatos.
fn liberar(l: &mut Loja, comando: &str, dados: &Value) {
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "comando": comando, "dados": dados }));
}

/// A senha do gerente para consultar a tela desta ação.
fn consultar(l: &mut Loja, acao: &str) {
    l.ok("gerente_liberar", json!({ "senha": SENHA_ANA, "acao": acao }));
}

/// Uma mudança na ficha da peça (para os testes de preço).
type Mudar = fn(&mut Value);

fn so_o_gerente(l: &mut Loja, comando: &str, dados: Value) {
    let r = l.chamar(comando, dados);
    assert_eq!(r["ok"], false, "{comando} passou sem a senha do gerente: {r}");
    let campo = r["campo"].as_str().unwrap_or("");
    assert!(campo == "gerente" || campo == "precos", "{comando}: {r}");
}

/// D-09.4 — Regras de preço só com a senha do gerente: o preço e o % de
/// lucro da peça e o lucro sugerido das tabelas. A senha vale para aquele
/// salvar, com aqueles dados, uma vez.
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
    // Senha do gerente para este salvar: grava uma vez.
    liberar(&mut l, "produto_salvar", &f);
    l.ok("produto_salvar", f.clone());
    let depois = gravado(&l, &p.filtro);
    assert_eq!((depois.0, depois.1), (6200, 12000));
    assert_ne!(depois.2, antes.2, "o % segue o preço");
    // A senha já foi usada: outra mudança (e até a mesma de novo) pede outra vez.
    let mut f2 = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f2["precos"][0]["preco"] = json!("130");
    assert_eq!(l.chamar("produto_salvar", f2)["campo"], "precos");
    assert_eq!(gravado(&l, &p.filtro), depois);
    let mut f3 = l.ok("produto_abrir", json!({ "id": p.filtro }));
    f3["precos"][1]["preco"] = json!("95");
    assert_eq!(l.chamar("produto_salvar", f3)["campo"], "precos");

    // Lucro sugerido das tabelas: o caixa (com a senha para salvar as
    // Configurações) não vê, e o que ele mandar fica como estava.
    let mut cfg = l.ok("config_ler", json!({}));
    let tabelas: Vec<Value> = cfg["tabelas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({ "numero": t["numero"], "nome": t["nome"], "lucroPadrao": "300" }))
        .collect();
    cfg["tabelas"] = json!(tabelas);
    liberar(&mut l, "config_salvar", &cfg);
    l.ok("config_salvar", cfg.clone());
    assert_eq!(l.numero("SELECT lucro_padrao FROM tabela_preco WHERE numero = 1"), 10_000);

    // O gerente muda: grava e fica na auditoria.
    l.entrar("Ana", SENHA_ANA);
    l.ok("config_salvar", cfg);
    assert_eq!(l.numero("SELECT lucro_padrao FROM tabela_preco WHERE numero = 1"), 30_000);
    assert_eq!(l.numero("SELECT COUNT(*) FROM auditoria WHERE acao = 'precos.regras'"), 1);
    assert_eq!(l.ok("produto_novo", json!({}))["precos"][0]["lucro"], "300");

    // Loja que libera os resultados para todos: o caixa vê custo e %, mas
    // mudar preço continua pedindo o gerente (a pílula não tem marca para isso,
    // nem mandando a marca por chamada direta).
    l.ok("gerente_salvar", json!({ "acoes": { "resultados": false, "precos": false } }));
    let g = l.ok("gerente_ler", json!({}));
    assert!(g["acoes"].as_array().unwrap().iter().all(|a| a["id"] != "precos"), "{g}");
    l.entrar("Bia", SENHA_BIA);
    let mut f = l.ok("produto_abrir", json!({ "id": p.pastilha }));
    assert_eq!(f["verCustos"], true);
    assert_eq!(f["custo"], "40,00");
    f["precos"][0]["lucro"] = json!("150");
    f["precos"][0]["preco"] = json!("100");
    assert_eq!(l.chamar("produto_salvar", f.clone())["campo"], "precos");
    // A senha dada para salvar a peça não serve para as Configurações.
    liberar(&mut l, "produto_salvar", &f);
    let mut cfg = l.ok("config_ler", json!({}));
    cfg["tabelas"][0]["lucroPadrao"] = json!("80");
    assert_eq!(l.chamar("config_salvar", cfg)["campo"], "gerente");
    l.ok("produto_salvar", f);
    assert_eq!(gravado(&l, &p.pastilha).1, 10_000);
}

/// D-09 — A senha para uma gravação fica presa ao usuário, ao comando e aos
/// dados exatos: qualquer dado diferente pede senha nova; não serve para
/// outro comando, nem para outro que precisa da mesma permissão; dá certo
/// uma vez e acaba; se der erro, só a mesma coisa pode ser repetida.
#[test]
fn d09_5_senha_presa_a_acao_e_aos_dados() {
    let (mut l, p) = loja();
    l.entrar("Bia", SENHA_BIA);
    l.abrir_caixa();
    let v = id(&l.vender(&[(&p.filtro, "1")], &[("dinheiro", "100")]));
    let cancelar = json!({ "id": v, "motivo": "cliente desistiu" });

    // Sem dizer para quê, para algo que não existe, para a regra de preço
    // "só de olhar" ou para um comando que não é do gerente: não libera.
    for pedido in [
        json!({ "senha": SENHA_ANA }),
        json!({ "senha": SENHA_ANA, "acao": "tudo" }),
        json!({ "senha": SENHA_ANA, "acao": "precos" }),
        json!({ "senha": SENHA_ANA, "comando": "venda_concluir", "dados": {} }),
        json!({ "senha": SENHA_ANA, "comando": "qualquer_coisa", "dados": {} }),
    ] {
        assert!(l.erro("gerente_liberar", pedido.clone()).contains("para que"), "{pedido}");
    }

    liberar(&mut l, "venda_cancelar", &cancelar);
    // Outros dados, outro motivo, outra venda, outro comando da mesma
    // permissão ou de outra: tudo recusado.
    so_o_gerente(&mut l, "venda_cancelar", json!({ "id": v, "motivo": "outro motivo" }));
    so_o_gerente(&mut l, "venda_cancelar", json!({ "id": p.v2, "motivo": "cliente desistiu" }));
    so_o_gerente(&mut l, "venda_cancelar", json!({ "id": v, "motivo": "cliente desistiu", "extra": 1 }));
    so_o_gerente(&mut l, "estoque_ajustar", json!({ "produtoId": p.filtro, "quantidade": "3", "motivo": "x" }));
    so_o_gerente(&mut l, "compra_estornar", json!({ "id": p.compra, "motivo": "cliente desistiu" }));
    assert_eq!(l.ok("permissao", json!({ "acao": "cancelar_venda" }))["pode"], false);
    // A mesma, com os mesmos dados (as chaves em outra ordem dão no mesmo): passa uma vez.
    l.ok("venda_cancelar", json!({ "motivo": "cliente desistiu", "id": v }));
    so_o_gerente(&mut l, "venda_cancelar", cancelar.clone());

    // Erro: a senha fica só para repetir a mesma coisa.
    let errado = json!({ "produtoId": p.filtro, "quantidade": "abc", "motivo": "contagem" });
    liberar(&mut l, "estoque_ajustar", &errado);
    for _ in 0..2 {
        let r = l.chamar("estoque_ajustar", errado.clone());
        assert_eq!(r["ok"], false);
        assert_ne!(r["campo"], "gerente", "deu erro do ajuste, não de permissão: {r}");
    }
    so_o_gerente(&mut l, "estoque_ajustar", json!({ "produtoId": p.filtro, "quantidade": "3", "motivo": "contagem" }));
    // 10 − 2 + 1 − 1 (do povoar), −1 +1 (venda cancelada acima): 8, sem ajuste.
    assert_eq!(l.estoque(&p.filtro), "8", "o ajuste corrigido não podia passar com a senha do errado");

    // Usuários: mexer na conta de um gerente com Usuários livre ainda pede
    // a senha para aquela gravação.
    l.entrar("Ana", SENHA_ANA);
    l.ok("gerente_salvar", json!({ "acoes": { "usuarios": false } }));
    l.entrar("Bia", SENHA_BIA);
    let nova_gerente =
        json!({ "nome": "Gil", "senha": "gil-12345", "senha2": "gil-12345", "ativo": true, "gerente": true });
    so_o_gerente(&mut l, "usuario_salvar", nova_gerente.clone());
    let outra =
        json!({ "nome": "Gil", "senha": "outra-senha", "senha2": "outra-senha", "ativo": true, "gerente": true });
    liberar(&mut l, "usuario_salvar", &outra);
    so_o_gerente(&mut l, "usuario_salvar", nova_gerente);
    l.ok("usuario_salvar", outra);

    // Presa ao usuário: trocar de usuário acaba com ela.
    let ajuste = json!({ "produtoId": p.filtro, "quantidade": "5", "motivo": "contagem" });
    liberar(&mut l, "estoque_ajustar", &ajuste);
    l.entrar("Bia", SENHA_BIA);
    so_o_gerente(&mut l, "estoque_ajustar", ajuste);

    // Auditoria: o que foi liberado, para quem, sem os dados.
    let c = l.conexao();
    let detalhes: Vec<String> = c
        .prepare("SELECT detalhes FROM auditoria WHERE acao = 'gerente.liberar' ORDER BY rowid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(detalhes.len(), 4, "{detalhes:?}");
    assert!(detalhes[0].contains("Bia") && detalhes[0].contains("cancelar venda"), "{detalhes:?}");
    assert!(detalhes.iter().all(|d| d.contains("uma vez, com estes dados")), "{detalhes:?}");
    assert!(detalhes.iter().all(|d| !d.contains("cliente desistiu") && !d.contains("outra-senha")), "{detalhes:?}");
}

/// D-09 — Consulta de uma tela do gerente: a senha deixa só ler a tela
/// daquela ação, até sair dela. Não grava, não abre outras telas nem a
/// pílula Gerente, nem por chamada direta.
#[test]
fn d09_8_consulta_so_le_a_propria_tela() {
    let (mut l, p) = loja();
    l.entrar("Bia", SENHA_BIA);
    let pago: String =
        l.conexao().query_row("SELECT id FROM pagamento WHERE estornado_em IS NULL LIMIT 1", [], |r| r.get(0)).unwrap();

    // Financeiro: lê (várias vezes) e vê os saldos das contas.
    consultar(&mut l, "financeiro");
    assert_eq!(l.ok("permissao", json!({ "acao": "financeiro" }))["pode"], false, "sem senha, não");
    assert_eq!(l.ok("permissao", json!({ "acao": "financeiro", "consulta": true }))["pode"], true);
    for _ in 0..2 {
        l.ok("financeiro_resumo", json!({}));
        l.ok("pagar_painel", json!({}));
        l.ok("cheques_painel", json!({}));
    }
    assert!(l.ok("contas_loja", json!({}))[0]["saldo"].is_string());
    // Não grava nada do Financeiro.
    so_o_gerente(
        &mut l,
        "pagar_lancar",
        json!({ "categoriaId": "cat-aluguel", "descricao": "x", "valor": "10", "parcelas": 1 }),
    );
    so_o_gerente(
        &mut l,
        "movimento_lancar",
        json!({ "contaId": "caixa-geral", "tipo": "saida", "valor": "5", "categoriaId": "cat-energia", "descricao": "x" }),
    );
    so_o_gerente(&mut l, "pagamento_estornar", json!({ "id": pago, "motivo": "errado" }));
    so_o_gerente(&mut l, "comissao_fechar", json!({ "usuarioId": "x", "de": "", "ate": "", "vencimento": "" }));
    // Não abre outras telas, nem a pílula Gerente nem o Backup.
    so_o_gerente(&mut l, "dre_apurar", json!({ "periodo": "mes" }));
    so_o_gerente(&mut l, "gerente_ler", json!({}));
    so_o_gerente(&mut l, "backup_ler", json!({}));
    so_o_gerente(&mut l, "gerente_salvar", json!({ "acoes": { "financeiro": false } }));
    assert_eq!(l.ok("painel_resumo", json!({}))["pagar7Dias"], "", "o Painel é outra tela");
    // Gravar com a senha para aquela gravação: passa uma vez; a consulta continua.
    let estorno = json!({ "id": pago, "motivo": "errado" });
    liberar(&mut l, "pagamento_estornar", &estorno);
    l.ok("pagamento_estornar", estorno);
    l.ok("financeiro_resumo", json!({}));
    // Sair da tela acaba com a consulta.
    l.ok("gerente_encerrar", json!({}));
    so_o_gerente(&mut l, "financeiro_resumo", json!({}));
    assert!(l.ok("contas_loja", json!({}))[0].get("saldo").is_none());

    // Resultados: lê a DRE e o lucro do Painel; não grava metas nem abre o
    // cadastro com custo nem o lucro do Estoque.
    consultar(&mut l, "resultados");
    l.ok("dre_apurar", json!({ "periodo": "mes" }));
    l.ok("dre_metas", json!({}));
    assert_ne!(l.ok("painel_resumo", json!({}))["lucroMes"], "");
    so_o_gerente(
        &mut l,
        "dre_meta_salvar",
        json!({ "periodo": "mes", "indicador": "vendas", "vendedorId": "", "valor": "5" }),
    );
    so_o_gerente(&mut l, "dre_classificacao_salvar", json!({ "categorias": [], "confirmar": true }));
    sem_custos(&l.ok("produto_abrir", json!({ "id": p.filtro })), "cadastro com a consulta dos resultados");
    assert_eq!(l.ok("estoque_mais_vendidos", json!({ "dias": 30, "ordem": "valor", "texto": "" }))["lucro"], "");
    so_o_gerente(&mut l, "financeiro_resumo", json!({}));
    l.ok("gerente_encerrar", json!({}));

    // Configurações: a senha das Configurações não abre o Backup nem a
    // pílula Gerente, nem grava as Configurações; cada um pede a sua.
    consultar(&mut l, "configuracoes");
    so_o_gerente(&mut l, "backup_ler", json!({}));
    so_o_gerente(&mut l, "gerente_ler", json!({}));
    so_o_gerente(&mut l, "codigos_situacao", json!({}));
    let cfg = l.ok("config_ler", json!({}));
    so_o_gerente(&mut l, "config_salvar", cfg);
    so_o_gerente(&mut l, "sync_salvar", json!({ "ligado": false, "senha": "", "enderecos": "" }));
    consultar(&mut l, "backup");
    l.ok("backup_ler", json!({}));
    so_o_gerente(&mut l, "backup_salvar", json!({ "pasta": "", "hora": "", "dias": "" }));
    so_o_gerente(&mut l, "gerente_ler", json!({}));
    consultar(&mut l, "gerente");
    l.ok("gerente_ler", json!({}));
    // FALHA-05: as diferenças entre caixas são da Simultaneidade, com
    // consulta própria; a da pílula Gerente não abre, e vice-versa.
    so_o_gerente(&mut l, "conflitos_listar", json!({}));
    consultar(&mut l, "simultaneidade");
    l.ok("conflitos_listar", json!({}));
    so_o_gerente(&mut l, "gerente_salvar", json!({ "acoes": { "cancelar_venda": false } }));
    so_o_gerente(&mut l, "codigos_definir_pendentes", json!({}));
    assert_eq!(l.numero("SELECT COUNT(*) FROM config WHERE chave = 'gerente_livres' AND valor LIKE '%cancelar%'"), 0);
    l.ok("gerente_encerrar", json!({}));
    so_o_gerente(&mut l, "backup_ler", json!({}));

    // Presa ao usuário: outro caixa que entra depois não herda a consulta.
    l.entrar("Ana", SENHA_ANA);
    l.usuario_caixa("Caio", "2222");
    l.entrar("Bia", SENHA_BIA);
    consultar(&mut l, "financeiro");
    l.ok("financeiro_resumo", json!({}));
    // Entra direto, sem "sair" antes (como quem troca de usuário na tela de entrada).
    let caio = l.ok("usuarios_entrada", json!({})).as_array().unwrap().iter().find(|u| u["nome"] == "Caio").unwrap()
        ["id"]
        .clone();
    l.ok("entrar", json!({ "id": caio, "senha": "2222" }));
    so_o_gerente(&mut l, "financeiro_resumo", json!({}));
    assert_eq!(l.ok("permissao", json!({ "acao": "financeiro", "consulta": true }))["pode"], false);

    // A auditoria diz que foi só consulta.
    let n = l.numero("SELECT COUNT(*) FROM auditoria WHERE acao = 'gerente.liberar' AND detalhes LIKE '%só leitura%'");
    // 7: as 6 de antes e a da Simultaneidade (FALHA-05).
    assert_eq!(n, 7);
}

/// D-09 — Preço padrão de venda: qualquer mudança numa peça cadastrada (com
/// custo ou sem, em qualquer tabela, na oferta ou no "preço na hora da
/// venda") pede o gerente. Cadastrar peça nova com preço e mudar o resto
/// da peça, não. Desconto na venda é outra coisa: segue as permissões dele.
#[test]
fn d09_7_preco_padrao_e_desconto_na_venda() {
    let (mut l, p) = loja();
    l.entrar("Bia", SENHA_BIA);
    let oleo = gravado(&l, &p.oleo);
    assert_eq!(oleo.0, 0, "o óleo não tem custo");
    let base = l.ok("produto_abrir", json!({ "id": p.oleo }));
    let mudancas: [(&str, Mudar); 4] = [
        ("preço da revenda", |f| f["precos"][0]["preco"] = json!("31")),
        ("preço de outra tabela", |f| f["precos"][1]["preco"] = json!("28")),
        ("oferta", |f| f["ofertaPreco"] = json!("25")),
        ("preço na hora da venda", |f| f["precoAberto"] = json!(true)),
    ];
    for (o_que, mudar) in &mudancas {
        let mut f = base.clone();
        mudar(&mut f);
        let r = l.chamar("produto_salvar", f);
        assert_eq!(r["campo"], "precos", "{o_que}: {r}");
    }
    assert_eq!(gravado(&l, &p.oleo), oleo);
    // O resto da peça o caixa muda sem senha.
    let mut f = base.clone();
    f["local"] = json!("Estante 2");
    f["precos"][0]["preco"] = json!("30,00");
    l.ok("produto_salvar", f);
    // Peça nova: cadastra com preço, oferta e tudo, sem senha.
    let mut f = l.ok("produto_novo", json!({}));
    f["nome"] = json!("Palheta");
    f["precos"][0]["preco"] = json!("40");
    f["precos"][1]["preco"] = json!("35");
    f["ofertaPreco"] = json!("38");
    l.ok("produto_salvar", f);
    // Com a senha para aquela mudança: grava.
    let mut f = base.clone();
    f["precos"][0]["preco"] = json!("31");
    liberar(&mut l, "produto_salvar", &f);
    l.ok("produto_salvar", f);
    assert_eq!(gravado(&l, &p.oleo).1, 3100);

    // Desconto na venda: dentro do limite da loja, o caixa dá sem senha, e o
    // preço da peça não muda.
    l.abrir_caixa();
    let v = l.vender_com(&[(&p.oleo, "1")], &[("dinheiro", "28")], json!({ "desconto": "3" }));
    assert!(v["id"].is_string(), "{v}");
    assert_eq!(gravado(&l, &p.oleo).1, 3100);
}

/// D-09 — A senha do gerente digitada no aperto não fica gravada em lugar
/// nenhum: nem no banco, nem no histórico, nem no registro de erros, nem em
/// outro arquivo da pasta da loja (também quando é digitada errada).
#[test]
fn d09_9_senha_do_gerente_nao_e_gravada() {
    let (mut l, p) = loja();
    const SENHA_GIL: &str = "Zebra-Listrada-Azul-4729";
    l.ok(
        "usuario_salvar",
        json!({ "nome": "Gil", "senha": SENHA_GIL, "senha2": SENHA_GIL, "ativo": true, "gerente": true }),
    );
    l.entrar("Bia", SENHA_BIA);
    let errada = "Girafa-Pintada-Verde-8113";
    let ajuste = json!({ "produtoId": p.filtro, "quantidade": "4", "motivo": "contagem" });
    l.erro("gerente_liberar", json!({ "senha": errada, "comando": "estoque_ajustar", "dados": ajuste }));
    l.ok("gerente_liberar", json!({ "senha": SENHA_GIL, "comando": "estoque_ajustar", "dados": ajuste }));
    l.ok("estoque_ajustar", ajuste);
    l.ok("gerente_liberar", json!({ "senha": SENHA_GIL, "acao": "financeiro" }));
    l.ok("financeiro_resumo", json!({}));
    // Erro interno vai para o registro: provoca um (comando que não existe).
    l.chamar("comando_que_nao_existe", json!({ "senha": SENHA_GIL }));
    l.fechar();
    let mut achados = Vec::new();
    fn procurar(pasta: &std::path::Path, agulhas: &[&str], achados: &mut Vec<String>) {
        let Ok(lista) = std::fs::read_dir(pasta) else { return };
        for e in lista.flatten() {
            let caminho = e.path();
            if caminho.is_dir() {
                procurar(&caminho, agulhas, achados);
            } else if let Ok(bytes) = std::fs::read(&caminho) {
                for a in agulhas {
                    if bytes.windows(a.len()).any(|w| w == a.as_bytes()) {
                        achados.push(format!("{} em {}", a, caminho.display()));
                    }
                }
            }
        }
    }
    // A pasta de todas as lojas de teste: o registro de erros é um só no
    // programa e pode estar na pasta de outro teste.
    for e in std::fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
        if e.file_name().to_string_lossy().starts_with("rustinho-estavel-") {
            procurar(&e.path(), &[SENHA_GIL, errada], &mut achados);
        }
    }
    assert!(achados.is_empty(), "a senha ficou gravada: {achados:?}");
    l.reabrir();
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

/// Nota de exemplo lançada pelo gerente e estornada; a mesma nota importada de
/// novo pela caixa Bia liga sozinha às peças da primeira. Devolve a compra
/// nova (já conferida pelo gerente), os itens para lançar e as peças.
fn segunda_compra_das_mesmas_pecas(l: &mut Loja) -> (String, Vec<Value>, Vec<String>) {
    let primeira = crate::comum::lancar_compra_de_exemplo(l);
    l.ok("compra_estornar", json!({ "id": primeira, "motivo": "teste" }));
    l.usuario_caixa("Bia", SENHA_BIA);
    l.entrar("Bia", SENHA_BIA);
    let caminho = l.pasta.join("nota-de-novo.xml");
    std::fs::write(&caminho, XML_COMPRA).unwrap();
    let r = l.ok("compra_importar", json!({ "caminho": caminho.display().to_string() }));
    let compra = r["compras"][0]["id"].as_str().unwrap().to_string();
    let c = l.ok("compra_abrir", json!({ "id": compra }));
    let itens = c["itens"].as_array().unwrap().clone();
    assert!(itens.iter().all(|i| i["acao"] == "ligar"), "os itens ligam às peças da primeira compra: {c}");
    let pecas: Vec<String> = itens.iter().map(|i| i["peca"]["id"].as_str().unwrap().to_string()).collect();
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": compra, "situacao": "autorizada", "protocolo": "135260000000001",
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    let escolhas: Vec<Value> = itens
        .iter()
        .zip(&pecas)
        .map(|(it, p)| json!({ "id": it["id"], "acao": "ligar", "produtoId": p, "fator": "1", "baixarPreco": false }))
        .collect();
    (compra, escolhas, pecas)
}

/// 0915-N (revisão dirigida): lançar uma compra com "os preços seguem o % de
/// lucro" muda o preço de venda de peças já cadastradas. Pela D-09, isso é do
/// gerente: a caixa sozinha é recusada (nada lançado, preços iguais); com a
/// senha do gerente para esta gravação, lança e os preços seguem o custo.
#[test]
fn d09_11_compra_que_muda_o_preco_pede_o_gerente() {
    let mut l = Loja::nova("d09-11");
    let (compra, escolhas, pecas) = segunda_compra_das_mesmas_pecas(&mut l);
    let antes: Vec<_> = pecas.iter().map(|p| gravado(&l, p)).collect();
    let dados = json!({ "id": compra, "itens": escolhas, "freteExtra": "90,00", "atualizarPrecos": true });
    let r = l.chamar("compra_lancar", dados.clone());
    assert_eq!(r["ok"], false, "a caixa mudou o preço de venda pela compra: {r}");
    assert_eq!(r["campo"], "precos", "{r}");
    let depois: Vec<_> = pecas.iter().map(|p| gravado(&l, p)).collect();
    assert_eq!(antes, depois, "nada mudou nas peças");
    assert_eq!(l.ok("compra_abrir", json!({ "id": compra }))["situacao"], "rascunho", "nada foi lançado");
    liberar(&mut l, "compra_lancar", &dados);
    let r = l.ok("compra_lancar", dados);
    assert!(r["resumo"]["precosAtualizados"].as_i64().unwrap() > 0, "{r}");
    let depois: Vec<_> = pecas.iter().map(|p| gravado(&l, p)).collect();
    assert!(antes.iter().zip(&depois).any(|(a, d)| d.1 > a.1), "com o gerente, os preços seguiram o custo");
}

/// Controle: sem mudar o preço, a caixa lança a compra sozinha (o custo muda,
/// o preço de venda fica), como sempre (D-09: o caixa lança compras).
#[test]
fn d09_12_compra_sem_mudar_o_preco_continua_com_o_caixa() {
    let mut l = Loja::nova("d09-12");
    let (compra, escolhas, pecas) = segunda_compra_das_mesmas_pecas(&mut l);
    let antes: Vec<_> = pecas.iter().map(|p| gravado(&l, p)).collect();
    let r = l.ok(
        "compra_lancar",
        json!({ "id": compra, "itens": escolhas, "freteExtra": "90,00", "atualizarPrecos": false }),
    );
    assert_eq!(r["resumo"]["precosAtualizados"], 0, "{r}");
    let depois: Vec<_> = pecas.iter().map(|p| gravado(&l, p)).collect();
    for (a, d) in antes.iter().zip(&depois) {
        assert_eq!(a.1, d.1, "o preço de venda ficou");
        assert!(d.0 > a.0, "o custo subiu com o frete");
    }
}
