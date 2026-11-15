//! QT-1b: regressão dos achados NOVOS da re-revisão independente (Fable, 15/11/2026;
//! `revisao_independente/fable/RELATORIO_REREVISAO_QT1.md`, N-01 a N-05). Cada teste usa só a interface do serviço
//! (a mesma na versão congelada da re-revisão e na corrigida): **falha na versão congelada** e passa depois.

use std::path::PathBuf;

use serde_json::{json, Value};

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&s.chamar(cmd, &d.to_string())).unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

fn temp(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rustinho-qt1b-rerev-{nome}-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join("rustinho.db")
}

fn abrir(caminho: &std::path::Path) -> crate::Servico {
    crate::Servico::abrir_para_teste(caminho, false).unwrap()
}

fn configurar(s: &mut crate::Servico) {
    ok(
        s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
}

fn preparar_venda(s: &mut crate::Servico) -> String {
    let mut f = ok(s, "produto_novo", json!({}));
    f["nome"] = json!("Filtro");
    f["custo"] = json!("10");
    f["estoqueInicial"] = json!("50");
    f["chaveOperacao"] = json!("peca-1");
    let p = ok(s, "produto_salvar", f)["id"].as_str().unwrap().to_string();
    ok(s, "caixa_abrir", json!({ "valor": "0", "chaveOperacao": "abrir-1" }));
    p
}

fn venda(p: &str, chave: &str) -> Value {
    json!({ "itens": [{ "produtoId": p, "quantidade": "1" }], "tabela": 1,
            "pagamentos": [{ "forma": "dinheiro", "valor": "100" }], "chaveOperacao": chave })
}

fn entrar(s: &mut crate::Servico, nome: &str, senha: &str) {
    let lista = ok(s, "usuarios_entrada", json!({}));
    let id = lista.as_array().unwrap().iter().find(|u| u["nome"] == nome).unwrap()["id"].as_str().unwrap().to_string();
    ok(s, "entrar", json!({ "id": id, "senha": senha }));
}

fn aviso(s: &mut crate::Servico) -> Option<Value> {
    let avisos = ok(s, "avisos_do_dia", json!({}));
    avisos
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap_or("").starts_with("fechamento_inesperado"))
        .cloned()
}

fn linhas(a: &Value) -> Vec<String> {
    a["detalhes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| format!("{}: {}", d["rotulo"].as_str().unwrap(), d["valor"].as_str().unwrap()))
        .collect()
}

fn fim(s: crate::Servico, caminho: &std::path::Path) {
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// N-01: a tela imprime logo depois de gravar, antes de mostrar. A impressão não pode valer como "a tela mostrou":
/// se o processo morre ali, a venda continua no aviso.
#[test]
fn rr_n01_impressao_nao_conta_como_entrega() {
    let caminho = temp("n01");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let v = ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    // O resultado da impressão não importa aqui (pode até recusar): o que conta é ela não marcar a entrega.
    let _ = chamar(&mut s, "impressao", json!({ "tipo": "venda", "id": v["id"] }));
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let a = aviso(&mut s).expect("N-01: a venda seguida só da impressão precisa continuar no aviso");
    let l = linhas(&a);
    assert!(l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", v["numero"]))), "N-01: {l:?}");
    fim(s, &caminho);
}

/// N-02: sete quedas seguidas, cada uma com uma venda sem confirmação: o aviso mostra as sete (antes, só as cinco
/// últimas; a 1 e a 2 ficavam sem aviso para sempre depois do "Conferi").
#[test]
fn rr_n02_todas_as_gravadas_sem_confirmacao_aparecem() {
    let caminho = temp("n02");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    s.cair_para_teste();
    let mut numeros = Vec::new();
    for i in 0..7 {
        let mut s = abrir(&caminho);
        entrar(&mut s, "Ana", "1234");
        numeros.push(ok(&mut s, "venda_concluir", venda(&p, &format!("venda-{i}")))["numero"].clone());
        s.cair_para_teste();
    }
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("o aviso"));
    for n in &numeros {
        assert!(
            l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {n} · "))),
            "N-02: a venda nº {n} sem confirmação não está no aviso: {l:?}"
        );
    }
    fim(s, &caminho);
}

/// N-03: uma queda sem nada a mostrar (no Android, a morte em segundo plano) não muda o id do aviso: o que a pessoa
/// minimizou continua minimizado, sem nada novo.
#[test]
fn rr_n03_queda_sem_linha_nao_muda_o_id_nem_reabre() {
    let caminho = temp("n03");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    s.cair_para_teste(); // queda com a venda sem confirmação
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let a = aviso(&mut s).expect("o aviso da venda");
    ok(&mut s, "aviso_minimizar", json!({ "id": a["id"], "minimizado": true }));
    ok(&mut s, "avisos_do_dia", json!({})); // a tela segue; nada novo é gravado
    s.cair_para_teste(); // queda sem nada a mostrar
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let b = aviso(&mut s).expect("o aviso continua (a venda não foi conferida)");
    assert_eq!(b["id"], a["id"], "N-03: a queda sem linha mudou o id do aviso");
    assert_eq!(b["minimizado"], true, "N-03: o aviso minimizado reabriu sem nada novo");
    fim(s, &caminho);
}

/// N-04: a abertura que começou com o relógio atrasado (ou que ficou aberta mais de um mês) e caiu HOJE, com uma
/// venda sem confirmação: o aviso aparece, e a limpeza ao abrir não apaga a abertura nem a venda.
#[test]
fn rr_n04_abertura_de_data_antiga_que_caiu_hoje_avisa() {
    let caminho = temp("n04");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let v = ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    s.banco_para_teste()
        .conn
        .execute(
            "UPDATE programa_abertura SET aberta_em = '2019-01-01T00:00:10Z'
              WHERE numero = (SELECT MAX(numero) FROM programa_abertura)",
            [],
        )
        .unwrap();
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("N-04: a queda de hoje de uma abertura com data antiga sumiu do aviso"));
    assert!(l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", v["numero"]))), "N-04: {l:?}");
    fim(s, &caminho);
}

/// N-05: o minimizar do caixa não vale para o gerente: ele recebe o aviso aberto (uma queda nova precisa ser vista).
#[test]
fn rr_n05_minimizar_e_por_pessoa() {
    let caminho = temp("n05");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(
        &mut s,
        "usuario_salvar",
        json!({ "nome": "Bia", "senha": "mesa-verde", "senha2": "mesa-verde", "ativo": true, "gerente": false,
                "chaveOperacao": "bia-1" }),
    );
    ok(&mut s, "sair", json!({}));
    entrar(&mut s, "Bia", "mesa-verde");
    ok(&mut s, "venda_concluir", venda(&p, "venda-bia"));
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Bia", "mesa-verde");
    let a = aviso(&mut s).expect("a Bia vê a venda dela");
    ok(&mut s, "aviso_minimizar", json!({ "id": a["id"], "minimizado": true }));
    ok(&mut s, "sair", json!({}));
    entrar(&mut s, "Ana", "1234");
    let b = aviso(&mut s).expect("o gerente vê a venda da Bia");
    assert_eq!(b["minimizado"], false, "N-05: o gerente recebeu o aviso já minimizado pela Bia");
    fim(s, &caminho);
}

/// N-01, parte 2 (proposta): a tela chama de novo ANTES de mostrar (recarregar o caixa): isso não vale como "a
/// pessoa viu". A venda seguida só de uma leitura, e a queda, continuam no aviso.
#[test]
fn rr_n01p2_venda_seguida_de_leitura_continua_no_aviso() {
    let caminho = temp("n01p2a");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    let v = ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    ok(&mut s, "caixa_estado", json!({})); // a tela recarrega o caixa antes de abrir a confirmação
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    let l = linhas(&aviso(&mut s).expect("N-01 (parte 2): a venda seguida só de uma leitura precisa avisar"));
    assert!(l.iter().any(|x| x.starts_with(&format!("Gravada: Venda · nº {} · ", v["numero"]))), "{l:?}");
    fim(s, &caminho);
}

/// N-01, parte 2 (proposta): a tela desenhou o resultado e confirmou a entrega: a queda depois não avisa.
#[test]
fn rr_n01p2_venda_confirmada_pela_tela_nao_avisa() {
    let caminho = temp("n01p2b");
    let mut s = abrir(&caminho);
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    ok(&mut s, "entrega_confirmar", json!({}));
    s.cair_para_teste();
    let mut s = abrir(&caminho);
    entrar(&mut s, "Ana", "1234");
    assert!(aviso(&mut s).is_none(), "a venda que a tela desenhou e confirmou não é motivo de atenção");
    fim(s, &caminho);
}

/// N-01, parte 2 (proposta): uma leitura logo depois de uma gravação não grava nada (nem a marca de entrega).
#[test]
fn rr_n01p2_leitura_depois_de_gravar_nao_grava() {
    let mut s = crate::Servico::em_memoria().unwrap();
    configurar(&mut s);
    let p = preparar_venda(&mut s);
    ok(&mut s, "venda_concluir", venda(&p, "venda-1"));
    let antes = s.banco_para_teste().conn.total_changes();
    ok(&mut s, "caixa_estado", json!({}));
    assert_eq!(s.banco_para_teste().conn.total_changes(), antes, "a leitura gravou algo");
}
