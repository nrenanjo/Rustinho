//! Contratos entre o núcleo e as telas (C-14): o envelope de toda resposta
//! e os campos que as telas leem dos comandos mais importantes. Se um campo
//! mudar de nome ou de tipo, este teste falha antes de a tela quebrar.
//!
//! Envelope: `{"ok": true, "dados": ...}` ou
//!           `{"ok": false, "erro": "texto", "campo": "nome" | null}`.

use rustinho_nucleo::Servico;
use serde_json::{json, Value};

fn chamar(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let texto = s.chamar(comando, &dados.to_string());
    let v: Value = serde_json::from_str(&texto).expect("a resposta é JSON");
    // Envelope sempre igual.
    let ok = v["ok"].as_bool().expect("ok é verdadeiro/falso");
    if ok {
        assert!(v.get("dados").is_some(), "{comando}: sucesso sem 'dados': {v}");
        assert!(v.get("erro").is_none(), "{comando}: sucesso com 'erro': {v}");
    } else {
        assert!(v["erro"].as_str().is_some_and(|e| !e.is_empty()), "{comando}: erro sem mensagem: {v}");
        assert!(v["campo"].is_null() || v["campo"].is_string(), "{comando}: 'campo' inválido: {v}");
    }
    v
}

fn ok(s: &mut Servico, comando: &str, dados: Value) -> Value {
    let v = chamar(s, comando, dados.clone());
    assert_eq!(v["ok"], true, "{comando} {dados}: {v}");
    v["dados"].clone()
}

fn erro(s: &mut Servico, comando: &str, dados: Value) -> (String, Option<String>) {
    let v = chamar(s, comando, dados.clone());
    assert_eq!(v["ok"], false, "{comando} {dados} deveria dar erro: {v}");
    (v["erro"].as_str().unwrap().to_string(), v["campo"].as_str().map(String::from))
}

/// Os campos existem e têm o tipo que a tela espera.
fn tem(v: &Value, campos: &[(&str, &str)]) {
    for (nome, tipo) in campos {
        let c = v.get(*nome).unwrap_or_else(|| panic!("falta '{nome}' em {v}"));
        let certo = match *tipo {
            "texto" => c.is_string(),
            "numero" => c.is_number(),
            "sim/nao" => c.is_boolean(),
            "lista" => c.is_array(),
            "objeto" => c.is_object(),
            "objeto|nada" => c.is_object() || c.is_null(),
            outro => panic!("tipo desconhecido {outro}"),
        };
        assert!(certo, "'{nome}' devia ser {tipo}: {c}");
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

#[test]
fn envelope_e_entradas_ruins() {
    let mut s = Servico::em_memoria().unwrap();
    // Comando que não existe, sem ninguém conectado.
    let (e, _) = erro(&mut s, "nao_existe", json!({}));
    assert!(!e.is_empty());
    // JSON quebrado na entrada.
    let v: Value = serde_json::from_str(&s.chamar("configurar_inicio", "{ isto não é json")).unwrap();
    assert_eq!(v["ok"], false);
    // Entrada vazia vale como {}.
    let v: Value = serde_json::from_str(&s.chamar("estado", "")).unwrap();
    assert_eq!(v["ok"], true);
    // Sem sessão: comando do caixa recusado.
    erro(&mut s, "produtos_buscar", json!({ "texto": "" }));

    let mut s = loja();
    // Tipo errado num campo.
    erro(&mut s, "venda_calcular", json!({ "itens": "não é lista" }));
    // Campo a mais é aceito (e ignorado).
    ok(&mut s, "produtos_buscar", json!({ "texto": "", "campoQueNaoExiste": 1 }));
    // Validação aponta o campo.
    let (_, campo) = erro(&mut s, "produto_salvar", json!({ "nome": "" }));
    assert_eq!(campo.as_deref(), Some("nome"));
    // Senha errada aponta "senha".
    let id = ok(&mut s, "estado", json!({}))["usuario"]["id"].as_str().unwrap().to_string();
    ok(&mut s, "sair", json!({}));
    let (_, campo) = erro(&mut s, "entrar", json!({ "id": id, "senha": "errada" }));
    assert_eq!(campo.as_deref(), Some("senha"));
    // Faltou um campo obrigatório.
    erro(&mut s, "entrar", json!({ "id": id }));
}

#[test]
fn estado_e_entrar() {
    let mut s = loja();
    let e = ok(&mut s, "estado", json!({}));
    tem(
        &e,
        &[
            ("primeiraVez", "sim/nao"),
            ("loja", "texto"),
            ("caixa", "texto"),
            ("usuario", "objeto|nada"),
            ("versao", "texto"),
            ("etapa", "numero"),
            ("validadeOrcamento", "numero"),
            ("escuro", "sim/nao"),
        ],
    );
    tem(&e["usuario"], &[("id", "texto"), ("nome", "texto"), ("gerente", "sim/nao")]);
    let lista = ok(&mut s, "usuarios_entrada", json!({}));
    tem(&lista[0], &[("id", "texto"), ("nome", "texto")]);
}

#[test]
fn venda_e_caixa() {
    let mut s = loja();
    // A tela pede a ficha nova, preenche e salva.
    let mut f = ok(&mut s, "produto_novo", json!({}));
    f["nome"] = json!("Pastilha");
    f["custo"] = json!("40");
    f["estoqueInicial"] = json!("5");
    let p = ok(&mut s, "produto_salvar", f);
    tem(&p, &[("id", "texto"), ("nome", "texto"), ("precos", "lista")]);
    let pid = p["id"].as_str().unwrap();
    let c = ok(&mut s, "venda_calcular", json!({ "itens": [{ "produtoId": pid, "quantidade": "2" }], "tabela": 1 }));
    tem(
        &c,
        &[
            ("itens", "lista"),
            ("subtotal", "texto"),
            ("descontoItens", "texto"),
            ("total", "texto"),
            ("falta", "texto"),
            ("troco", "texto"),
            ("quantidadeItens", "texto"),
            ("erroDesconto", "texto"),
            ("erroEncomenda", "texto"),
            ("encomendas", "numero"),
            ("cliente", "texto"),
            ("crediario", "objeto"),
            ("cheques", "objeto"),
        ],
    );
    tem(
        &c["itens"][0],
        &[("descricao", "texto"), ("quantidade", "texto"), ("preco", "texto"), ("total", "texto"), ("erro", "texto")],
    );
    // Valores internos (centavos) não vão para a tela.
    assert!(c["itens"][0].get("vTotal").is_none() && c["itens"][0].get("v_total").is_none());
    // Sem caixa aberto, concluir dá erro.
    erro(&mut s, "venda_concluir", json!({ "itens": [{ "produtoId": pid, "quantidade": "1" }], "tabela": 1 }));
    let estado = ok(&mut s, "caixa_estado", json!({}));
    tem(&estado, &[("aberto", "sim/nao")]);
    ok(&mut s, "caixa_abrir", json!({ "valor": "50" }));
    // Pago a menos: aponta "pagamentos".
    let (_, campo) = erro(
        &mut s,
        "venda_concluir",
        json!({ "itens": [{ "produtoId": pid, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "pix", "valor": "1" }] }),
    );
    assert_eq!(campo.as_deref(), Some("pagamentos"));
    let v = ok(
        &mut s,
        "venda_concluir",
        json!({ "itens": [{ "produtoId": pid, "quantidade": "1" }], "tabela": 1,
                "pagamentos": [{ "forma": "dinheiro", "valor": "100" }] }),
    );
    assert!(v.is_object() || v.is_string(), "{v}");
    let estado = ok(&mut s, "caixa_estado", json!({}));
    assert_eq!(estado["aberto"], true);
}

#[test]
fn gerente_backup_e_caixas() {
    let mut s = loja();
    let g = ok(&mut s, "gerente_ler", json!({}));
    tem(&g, &[("acoes", "lista"), ("usuarios", "lista")]);
    tem(&g["acoes"][0], &[("id", "texto"), ("nome", "texto"), ("soGerente", "sim/nao")]);
    let p = ok(&mut s, "permissao", json!({ "acao": "backup" }));
    tem(&p, &[("pode", "sim/nao"), ("gerente", "sim/nao")]);
    let k = ok(&mut s, "conflitos_listar", json!({}));
    assert!(k.is_array());
    let b = ok(&mut s, "backup_ler", json!({}));
    tem(
        &b,
        &[
            ("pasta", "texto"),
            ("dias", "texto"),
            ("hora", "texto"),
            ("temSenha", "sim/nao"),
            ("nuvem", "sim/nao"),
            ("situacao", "objeto"),
            ("ocupado", "sim/nao"),
            ("lista", "lista"),
        ],
    );
    // O gerente entrou: a chave do backup existe (backup sai trancado).
    assert_eq!(b["temSenha"], true);
    let y = ok(&mut s, "sync_ler", json!({}));
    tem(
        &y,
        &[
            ("ligado", "sim/nao"),
            ("temSenha", "sim/nao"),
            ("pares", "lista"),
            ("pendencias", "numero"),
            ("porta", "numero"),
        ],
    );
    let a = ok(&mut s, "avisos_do_dia", json!({}));
    assert!(a.is_array());
    // Resolver conflito que não existe: erro, não sucesso.
    erro(&mut s, "conflito_resolver", json!({ "id": 999, "usar": true }));
}
