//! Teste do caminho completo, do jeito que a interface usa o núcleo:
//! comandos com JSON, respostas em JSON.

use rustinho_nucleo::Servico;
use serde_json::{json, Value};

/// Chama um comando e devolve a resposta inteira.
fn chamar(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let resposta = s.chamar(comando, &dados.to_string());
    serde_json::from_str(&resposta).expect("resposta em JSON")
}

/// Chama e exige sucesso; devolve só os dados.
fn ok(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let r = chamar(s, comando, dados);
    assert_eq!(r["ok"], true, "{comando} falhou: {r}");
    r["dados"].clone()
}

/// Chama e exige erro; devolve (mensagem, campo).
fn erro(s: &mut Servico, comando: &str, dados: Value) -> (String, String) {
    let r = chamar(s, comando, dados);
    assert_eq!(r["ok"], false, "{comando} deveria falhar: {r}");
    (
        r["erro"].as_str().unwrap_or_default().to_string(),
        r["campo"].as_str().unwrap_or_default().to_string(),
    )
}

fn rolamento() -> Value {
    json!({
        "nome": "Rolamento roda traseira",
        "marca": "FAG",
        "codigoOriginal": "32206A",
        "codigoBarras": "7891234567895",
        "unidade": "un",
        "custo": "58,00",
        "precos": [{ "tabela": 1, "lucro": "100", "preco": "116,00" }],
        "estoqueInicial": "4",
        "estoqueMinimo": "2",
        "local": "B3",
        "ativo": true,
        "aplicacoes": [
            { "montadora": "VW", "modelo": "Gol G5", "motor": "1.6", "anoInicio": "2009", "anoFim": "2013" },
            { "montadora": "", "modelo": "", "motor": "", "anoInicio": "", "anoFim": "" }
        ],
        "equivalentes": [{ "marca": "SKF", "codigo": "BR-32206" }]
    })
}

fn pastilha() -> Value {
    json!({
        "nome": "Pastilha de freio dianteira",
        "marca": "Cobreq",
        "custo": "40",
        "precos": [{ "tabela": 1, "lucro": "100", "preco": "" }],
        "ativo": true,
        "aplicacoes": [
            { "montadora": "Volkswagen", "modelo": "Gol", "motor": "1.0 / 1.6", "anoInicio": "15", "anoFim": "2020" },
            { "montadora": "Fiat", "modelo": "Uno", "motor": "1.0", "anoInicio": "", "anoFim": "" }
        ]
    })
}

fn nomes(lista: &Value) -> Vec<String> {
    lista["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["nome"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn fluxo_completo() {
    let mut s = Servico::em_memoria().unwrap();

    // Primeira vez: ninguém cadastrado.
    let estado = ok(&mut s, "estado", json!({}));
    assert_eq!(estado["primeiraVez"], true);
    assert!(estado["usuario"].is_null());

    // Sem estar conectado, não dá para buscar peças.
    let (msg, _) = erro(&mut s, "produtos_buscar", json!({}));
    assert!(msg.contains("Entre com seu usuário"), "{msg}");

    // Configuração inicial.
    let (_, campo) = erro(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja Modelo Auto Peças", "caixa": "Caixa 1", "nome": "Maria", "senha": "1234", "senha2": "4321" }),
    );
    assert_eq!(campo, "senha2");
    let estado = ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja Modelo Auto Peças", "caixa": "Caixa 1", "nome": "Maria", "senha": "1234", "senha2": "1234" }),
    );
    assert_eq!(estado["primeiraVez"], false);
    assert_eq!(estado["usuario"]["nome"], "Maria");
    assert_eq!(estado["loja"], "Loja Modelo Auto Peças");
    erro(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "X", "caixa": "Caixa 1", "nome": "Outro", "senha": "1234", "senha2": "1234" }),
    );

    // Ficha nova vem com código 1 e 100% de lucro na Revenda.
    let nova = ok(&mut s, "produto_novo", json!({}));
    assert_eq!(nova["codigo"], "1");
    assert_eq!(nova["precos"][0]["nome"], "Revenda");
    assert_eq!(nova["precos"][0]["lucro"], "100");
    assert_eq!(nova["precos"].as_array().unwrap().len(), 5);

    // Conferências.
    let mut sem_nome = rolamento();
    sem_nome["nome"] = json!("  ");
    assert_eq!(erro(&mut s, "produto_salvar", sem_nome).1, "nome");
    let mut sem_preco = rolamento();
    sem_preco["precos"] = json!([]);
    assert_eq!(erro(&mut s, "produto_salvar", sem_preco).1, "preco1");
    let mut barras_errado = rolamento();
    barras_errado["codigoBarras"] = json!("7891234567890");
    assert_eq!(erro(&mut s, "produto_salvar", barras_errado).1, "codigoBarras");
    let mut ano_errado = rolamento();
    ano_errado["aplicacoes"][0]["anoFim"] = json!("2005");
    assert_eq!(erro(&mut s, "produto_salvar", ano_errado).1, "aplicacoes.0.anoFim");

    // Cadastro.
    let rol = ok(&mut s, "produto_salvar", rolamento());
    let rol_id = rol["id"].as_str().unwrap().to_string();
    assert_eq!(rol["codigo"], "1");
    assert_eq!(rol["unidade"], "UN");
    assert_eq!(rol["estoque"], "4");
    assert_eq!(rol["precos"][0]["preco"], "116,00");
    assert_eq!(rol["aplicacoes"].as_array().unwrap().len(), 1, "linha vazia descartada");
    assert_eq!(rol["historico"][0]["descricao"], "cadastrou a peça");
    assert_eq!(rol["historico"][0]["usuario"], "Maria");

    let pas = ok(&mut s, "produto_salvar", pastilha());
    let pas_id = pas["id"].as_str().unwrap().to_string();
    assert_eq!(pas["codigo"], "2");
    assert_eq!(pas["precos"][0]["preco"], "80,00", "preço calculado pelo percentual");
    assert_eq!(pas["aplicacoes"][0]["anoInicio"], "2015");

    // Busca.
    let b = |s: &mut Servico, texto: &str| ok(s, "produtos_buscar", json!({ "texto": texto }));
    assert_eq!(nomes(&b(&mut s, "32206")), ["Rolamento roda traseira"]);
    let exato = b(&mut s, "32206a");
    assert_eq!(exato["itens"][0]["codigoExato"], true);
    assert_eq!(nomes(&b(&mut s, "7891234567895")), ["Rolamento roda traseira"]);
    assert_eq!(nomes(&b(&mut s, "br-32206")), ["Rolamento roda traseira"]);
    assert_eq!(nomes(&b(&mut s, "br32206")), ["Rolamento roda traseira"]);
    assert_eq!(nomes(&b(&mut s, "volkswagen gol 2010")), ["Rolamento roda traseira"]);
    assert_eq!(nomes(&b(&mut s, "vw gol 2016")), ["Pastilha de freio dianteira"]);
    assert_eq!(nomes(&b(&mut s, "PASTILHA uno")), ["Pastilha de freio dianteira"]);
    assert_eq!(nomes(&b(&mut s, "rolamento uno")).len(), 0);

    // Balão: por qual código achou (só quando o texto é um código).
    let achou = |s: &mut Servico, texto: &str| b(s, texto)["itens"][0]["achou"].as_str().unwrap().to_string();
    assert_eq!(achou(&mut s, "7891234567895"), "Cód. barras");
    assert_eq!(achou(&mut s, "32206a"), "Cód. original");
    assert_eq!(achou(&mut s, "32206"), "Cód. original", "começa com");
    assert_eq!(achou(&mut s, "br-32206"), "Similar · SKF");
    assert_eq!(achou(&mut s, "rolamento"), "", "pelo nome não tem balão");
    let todos = b(&mut s, "");
    assert_eq!(todos["total"], 2);
    assert_eq!(nomes(&todos).len(), 2);
    let r0 = &b(&mut s, "rolamento")["itens"][0];
    assert_eq!(r0["preco"], "116,00");
    assert_eq!(r0["estoque"], "4");
    assert_eq!(r0["estoqueBaixo"], false);
    assert_eq!(r0["aplicacoes"], "VW Gol G5 1.6 2009–2013");
    let p0 = &b(&mut s, "pastilha")["itens"][0];
    assert_eq!(p0["estoqueBaixo"], true, "sem estoque");

    // Modelo escrito junto acha o nome escrito separado, e o contrário.
    let mut correia = pastilha();
    correia["nome"] = json!("Correia poli V 8PK 1515");
    correia["aplicacoes"] = json!([]);
    ok(&mut s, "produto_salvar", correia);
    assert_eq!(nomes(&b(&mut s, "8pk1515")), ["Correia poli V 8PK 1515"]);
    assert_eq!(nomes(&b(&mut s, "correia 8pk 1515")), ["Correia poli V 8PK 1515"]);
    assert_eq!(achou(&mut s, "8pk1515"), "");
    let todos = b(&mut s, "");
    assert_eq!(todos["total"], 3);

    // Alterar o preço pelo percentual.
    let mut ficha = ok(&mut s, "produto_abrir", json!({ "id": rol_id }));
    ficha["precos"][0]["lucro"] = json!("50");
    ficha["precos"][0]["preco"] = json!("");
    let ficha = ok(&mut s, "produto_salvar", ficha);
    assert_eq!(ficha["precos"][0]["preco"], "87,00");
    assert_eq!(ficha["precos"][0]["lucro"], "50");
    let detalhes = ficha["historico"][0]["detalhes"].to_string();
    assert!(detalhes.contains("Preço Revenda: 116,00 → 87,00"), "{detalhes}");
    let quantos = ficha["historico"].as_array().unwrap().len();

    // Salvar sem mudar nada não cria histórico.
    let ficha = ok(&mut s, "produto_salvar", ficha);
    assert_eq!(ficha["historico"].as_array().unwrap().len(), quantos);

    // Ajuste de estoque.
    assert_eq!(
        erro(&mut s, "estoque_ajustar", json!({ "produtoId": rol_id, "quantidade": "3", "motivo": "" })).1,
        "motivo"
    );
    let ficha = ok(
        &mut s,
        "estoque_ajustar",
        json!({ "produtoId": rol_id, "quantidade": "3", "motivo": "Contagem" }),
    );
    assert_eq!(ficha["estoque"], "3");
    assert_eq!(ficha["historico"][0]["descricao"], "ajustou o estoque");

    // Código e código de barras não se repetem.
    let mut repetida = pastilha();
    repetida["codigoBarras"] = json!("7891234567895");
    assert_eq!(erro(&mut s, "produto_salvar", repetida).1, "codigoBarras");
    let mut ficha_p = ok(&mut s, "produto_abrir", json!({ "id": pas_id }));
    ficha_p["codigo"] = json!("1");
    assert_eq!(erro(&mut s, "produto_salvar", ficha_p).1, "codigo");

    // Desativar: some da busca normal.
    let mut ficha_p = ok(&mut s, "produto_abrir", json!({ "id": pas_id }));
    ficha_p["ativo"] = json!(false);
    ok(&mut s, "produto_salvar", ficha_p);
    assert_eq!(nomes(&b(&mut s, "pastilha")).len(), 0);
    let com_inativas = ok(&mut s, "produtos_buscar", json!({ "texto": "pastilha", "inativas": true }));
    assert_eq!(nomes(&com_inativas), ["Pastilha de freio dianteira"]);

    // Cálculo de preço para a tela.
    let c = ok(&mut s, "preco_calcular", json!({ "custo": "58", "lucro": "", "preco": "85", "de": "preco" }));
    assert_eq!(c["lucro"], "46,55");

    // Menu: mais usados.
    ok(&mut s, "menu_usar", json!({ "id": "pecas" }));
    ok(&mut s, "menu_usar", json!({ "id": "usuarios" }));
    ok(&mut s, "menu_usar", json!({ "id": "usuarios" }));
    let m = ok(&mut s, "menu", json!({}));
    assert_eq!(m["maisUsados"][0]["id"], "usuarios");
    assert_eq!(m["maisUsados"][1]["id"], "pecas");
    assert_eq!(m["todos"][0]["id"], "vender");

    // Tabelas de preço.
    let mut cfg = ok(&mut s, "config_ler", json!({}));
    cfg["tabelas"][1]["nome"] = json!("Oficina");
    cfg["tabelas"][1]["lucroPadrao"] = json!("80");
    let cfg = ok(&mut s, "config_salvar", cfg);
    assert_eq!(cfg["tabelas"][1]["nome"], "Oficina");
    let nova = ok(&mut s, "produto_novo", json!({}));
    assert_eq!(nova["codigo"], "4");
    assert_eq!(nova["precos"][1]["nome"], "Oficina");
    assert_eq!(nova["precos"][1]["lucro"], "80");

    // Usuários e entrada.
    let luigi = ok(
        &mut s,
        "usuario_salvar",
        json!({ "nome": "Luigi", "senha": "4321", "senha2": "4321", "ativo": true }),
    );
    let luigi_id = luigi["id"].as_str().unwrap().to_string();
    let lista = ok(&mut s, "usuarios_listar", json!({}));
    assert_eq!(lista.as_array().unwrap().len(), 2);
    let entrada = ok(&mut s, "usuarios_entrada", json!({}));
    assert_eq!(entrada[0]["nome"], "Luigi");
    assert_eq!(entrada[1]["nome"], "Maria");

    ok(&mut s, "sair", json!({}));
    assert!(ok(&mut s, "estado", json!({}))["usuario"].is_null());
    for _ in 0..5 {
        let (msg, campo) = erro(&mut s, "entrar", json!({ "id": luigi_id, "senha": "errada" }));
        assert_eq!(campo, "senha");
        assert_eq!(msg, "Senha errada.");
    }
    let (msg, _) = erro(&mut s, "entrar", json!({ "id": luigi_id, "senha": "4321" }));
    assert!(msg.contains("Espere"), "{msg}");
    let maria_id = entrada[1]["id"].as_str().unwrap().to_string();
    let estado = ok(&mut s, "entrar", json!({ "id": maria_id, "senha": "1234" }));
    assert_eq!(estado["usuario"]["nome"], "Maria");

    // Trocar a própria senha.
    assert_eq!(
        erro(&mut s, "senha_trocar", json!({ "atual": "0000", "nova": "abcd", "nova2": "abcd" })).1,
        "atual"
    );
    ok(&mut s, "senha_trocar", json!({ "atual": "1234", "nova": "abcd", "nova2": "abcd" }));
    ok(&mut s, "sair", json!({}));
    erro(&mut s, "entrar", json!({ "id": maria_id, "senha": "1234" }));
    ok(&mut s, "entrar", json!({ "id": maria_id, "senha": "abcd" }));

    // Comando desconhecido e JSON quebrado viram erro, sem derrubar nada.
    erro(&mut s, "nao_existe", json!({}));
    let r: Value = serde_json::from_str(&s.chamar("estado", "{quebrado")).unwrap();
    assert_eq!(r["ok"], false);
}

/// Compras: um .zip com a nota e um evento (cancelamento) dentro, e o mesmo
/// XML de novo (repetido não entra duas vezes).
#[test]
fn compra_por_zip() {
    use std::io::Write;
    let mut s = Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Maria", "senha": "1234", "senha2": "1234" }),
    );
    let nota = include_str!("nfe_exemplo.xml");
    let pasta = std::env::temp_dir().join(format!("rustinho-compra-zip-{}", std::process::id()));
    std::fs::create_dir_all(&pasta).unwrap();
    let zip_caminho = pasta.join("notas do fornecedor.zip");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_caminho).unwrap());
        let op = zip::write::SimpleFileOptions::default();
        z.start_file("nota.xml", op).unwrap();
        z.write_all(nota.as_bytes()).unwrap();
        z.start_file("evento.xml", op).unwrap();
        z.write_all(b"<procEventoNFe><evento/></procEventoNFe>").unwrap();
        z.finish().unwrap();
    }
    let url = format!("file://{}", zip_caminho.display()).replace(' ', "%20");
    let r = ok(&mut s, "compra_importar", json!({ "caminho": url }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["erros"].as_array().unwrap().len(), 0, "{r}");

    let solto = pasta.join("nota.xml");
    std::fs::write(&solto, nota).unwrap();
    let r = ok(&mut s, "compra_importar", json!({ "caminho": solto.display().to_string() }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 0, "{r}");
    assert_eq!(r["erros"].as_array().unwrap().len(), 1, "{r}");

    let (msg, _) = erro(&mut s, "compra_importar", json!({ "caminho": pasta.join("x.pdf").display().to_string() }));
    assert!(msg.contains("Não achei"), "{msg}");
    std::fs::remove_dir_all(&pasta).ok();
}

#[test]
fn gerente_e_caixa() {
    let mut s = Servico::em_memoria().unwrap();
    let estado = ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja Modelo", "caixa": "Caixa 1", "nome": "Maria", "senha": "1234", "senha2": "1234" }),
    );
    // Quem configura é o gerente.
    assert_eq!(estado["usuario"]["gerente"], true);
    let joao = ok(&mut s, "usuario_salvar", json!({ "nome": "João", "senha": "5678", "senha2": "5678" }));
    assert_eq!(joao["gerente"], false);
    let peca = ok(&mut s, "produto_salvar", rolamento());
    let peca_id = peca["id"].as_str().unwrap().to_string();
    ok(&mut s, "sair", json!({}));

    // O caixa entra e não pode ajustar o estoque nem abrir a pílula Gerente.
    let estado = ok(&mut s, "entrar", json!({ "id": joao["id"], "senha": "5678" }));
    assert_eq!(estado["usuario"]["gerente"], false);
    let ajuste = json!({ "produtoId": peca_id, "quantidade": "3", "motivo": "Contagem" });
    assert_eq!(erro(&mut s, "estoque_ajustar", ajuste.clone()).1, "gerente");
    assert_eq!(erro(&mut s, "gerente_ler", json!({})).1, "gerente");
    assert_eq!(ok(&mut s, "permissao", json!({ "acao": "ajustar_estoque" }))["pode"], false);
    // Ele não pode se fazer gerente.
    assert_eq!(
        erro(&mut s, "usuario_salvar", json!({ "id": joao["id"], "nome": "João", "gerente": true })).1,
        "gerente"
    );

    // O "aperto": o gerente digita a senha na hora.
    assert_eq!(erro(&mut s, "gerente_liberar", json!({ "senha": "5678" })).1, "senha");
    assert_eq!(ok(&mut s, "gerente_liberar", json!({ "senha": "1234" }))["nome"], "Maria");
    ok(&mut s, "estoque_ajustar", ajuste.clone());
    ok(&mut s, "gerente_encerrar", json!({}));
    assert_eq!(erro(&mut s, "estoque_ajustar", ajuste.clone()).1, "gerente");
    ok(&mut s, "sair", json!({}));

    // A gerente libera o ajuste de estoque para todos.
    let maria = ok(&mut s, "usuarios_entrada", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["nome"] == "Maria")
        .unwrap()["id"]
        .clone();
    ok(&mut s, "entrar", json!({ "id": maria, "senha": "1234" }));
    let g = ok(&mut s, "gerente_salvar", json!({ "acoes": { "ajustar_estoque": false } }));
    let ajustar = g["acoes"].as_array().unwrap().iter().find(|a| a["id"] == "ajustar_estoque").unwrap().clone();
    assert_eq!(ajustar["soGerente"], false);
    // Não pode ficar sem gerente.
    assert_eq!(erro(&mut s, "gerente_salvar", json!({ "gerentes": { maria.as_str().unwrap(): false } })).1, "gerente");
    ok(&mut s, "sair", json!({}));
    ok(&mut s, "entrar", json!({ "id": joao["id"], "senha": "5678" }));
    ok(&mut s, "estoque_ajustar", json!({ "produtoId": peca_id, "quantidade": "5", "motivo": "Contagem" }));
    // Cancelar venda continua só do gerente.
    assert_eq!(ok(&mut s, "permissao", json!({ "acao": "cancelar_venda" }))["pode"], false);
}

#[test]
fn senhas_erradas_seguidas_esperam() {
    let mut s = Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Maria", "senha": "1234", "senha2": "1234" }),
    );
    let id = ok(&mut s, "estado", json!({}))["usuario"]["id"].clone();
    ok(&mut s, "sair", json!({}));
    for _ in 0..5 {
        assert_eq!(erro(&mut s, "entrar", json!({ "id": id, "senha": "0000" })).1, "senha");
    }
    let (msg, _) = erro(&mut s, "entrar", json!({ "id": id, "senha": "1234" }));
    assert!(msg.contains("Espere"), "{msg}");
}

#[test]
fn venda_confere_cliente_orcamento_e_limites() {
    let mut s = Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Maria", "senha": "1234", "senha2": "1234" }),
    );
    let peca = ok(&mut s, "produto_salvar", rolamento());
    ok(&mut s, "caixa_abrir", json!({ "fundo": "0" }));
    let item = json!([{ "produtoId": peca["id"], "quantidade": "1", "preco": "", "desconto": "" }]);
    // Cliente que não existe.
    let venda = json!({ "itens": item, "clienteId": "nao-existe", "pagamentos": [{ "forma": "dinheiro", "valor": "116,00" }] });
    assert_eq!(erro(&mut s, "venda_concluir", venda).1, "cliente");
    // Quantidade e preço absurdos viram erro, não conta errada.
    let calc = ok(&mut s, "venda_calcular", json!({ "itens": [{ "produtoId": peca["id"], "quantidade": "9999999999999", "preco": "", "desconto": "" }] }));
    assert!(calc["itens"][0]["erro"].as_str().unwrap().contains("grande demais"), "{calc}");
    let calc = ok(&mut s, "venda_calcular", json!({ "itens": [{ "produtoId": peca["id"], "quantidade": "1", "preco": "9999999999999", "desconto": "" }] }));
    assert!(calc["itens"][0]["erro"].as_str().unwrap().contains("grande demais"), "{calc}");
    // Orçamento vendido duas vezes: a segunda não entra.
    let orc = ok(&mut s, "orcamento_salvar", json!({ "itens": item }));
    let orc_id = orc["id"].as_str().unwrap_or_else(|| orc.as_str().unwrap()).to_string();
    let venda = json!({ "itens": item, "orcamentoId": orc_id, "pagamentos": [{ "forma": "dinheiro", "valor": "116,00" }] });
    ok(&mut s, "venda_concluir", venda.clone());
    let (msg, _) = erro(&mut s, "venda_concluir", venda);
    assert!(msg.contains("orçamento"), "{msg}");
}
