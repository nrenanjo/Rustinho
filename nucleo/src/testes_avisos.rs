//! Notificações do Início pela API (auditoria da 0.9.16): o que a tela pede
//! (`avisos_do_dia`, `aviso_minimizar`, `aviso_esconder`) respeita a
//! categoria, guarda só a apresentação e sobrevive a fechar e abrir.

use serde_json::{json, Value};

fn chamar(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    serde_json::from_str(&crate::apoio_chave::chamar(crate::api::NAO_REPETIR, cmd, &d.to_string(), |x| {
        s.chamar(cmd, x)
    }))
    .unwrap()
}

fn ok(s: &mut crate::Servico, cmd: &str, d: Value) -> Value {
    let r = chamar(s, cmd, d);
    assert_eq!(r["ok"], true, "{cmd}: {r}");
    r["dados"].clone()
}

fn aviso(s: &mut crate::Servico, id: &str) -> Option<Value> {
    ok(s, "avisos_do_dia", json!({})).as_array().unwrap().iter().find(|a| a["id"] == id).cloned()
}

/// Loja em arquivo com uma notificação de cada categoria: informativa
/// (parcela vencida), atenção (backup atrasado) e crítica (outra loja).
fn loja(caminho: &std::path::Path) -> (crate::Servico, String) {
    let mut s = crate::Servico::abrir(caminho).unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    let ana: String = s.banco_para_teste().conn.query_row("SELECT id FROM usuario", [], |l| l.get(0)).unwrap();
    let mut c = ok(&mut s, "pessoa_nova", json!({ "papel": "cliente" }));
    c["nome"] = json!("Zé da Silva");
    let cliente = ok(&mut s, "pessoa_salvar", c)["id"].as_str().unwrap().to_string();
    ok(
        &mut s,
        "receber_lancar",
        json!({ "clienteId": cliente, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let conn = &s.banco_para_teste().conn;
    conn.execute("UPDATE conta_receber SET vencimento = date('now', '-3 days')", []).unwrap();
    crate::config::gravar(conn, "backup_desde", &crate::tempo::dias_atras(3)).unwrap();
    crate::config::gravar(
        conn,
        crate::loja::OUTRA_LOJA,
        &json!({ "nome": "Caixa da outra loja", "endereco": "192.168.0.9", "quando": crate::tempo::agora() })
            .to_string(),
    )
    .unwrap();
    (s, ana)
}

fn pasta() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rustinho-avisos-{}", crate::tempo::novo_id())).join("rustinho.db")
}

/// Arrastar para a esquerda (a tela chama `aviso_esconder`): a informativa
/// some até a meia-noite; a atenção e a crítica são recusadas e continuam.
#[test]
fn n06_esconder_pela_api_respeita_a_categoria() {
    let caminho = pasta();
    let (mut s, _) = loja(&caminho);
    assert_eq!(aviso(&mut s, "receber").unwrap()["categoria"], "informativa");
    assert_eq!(aviso(&mut s, "backup").unwrap()["categoria"], "atencao");
    assert_eq!(aviso(&mut s, "outra_loja").unwrap()["categoria"], "critica");
    for id in ["backup", "outra_loja"] {
        let r = chamar(&mut s, "aviso_esconder", json!({ "id": id }));
        assert_eq!(r["ok"], false, "{id}: {r}");
        assert!(aviso(&mut s, id).is_some(), "{id} continua");
    }
    assert_eq!(chamar(&mut s, "aviso_esconder", json!({ "id": "receber" }))["ok"], true);
    assert!(aviso(&mut s, "receber").is_none(), "informativa escondida");
    // Uma que nem está aparecendo: recusa clara, nada gravado.
    let r = chamar(&mut s, "aviso_esconder", json!({ "id": "nao_existe" }));
    assert_eq!(r["ok"], false, "{r}");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// As não críticas abertas (em faixa), na ordem da tela.
fn abertas(s: &mut crate::Servico) -> Vec<String> {
    ok(s, "avisos_do_dia", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["categoria"] != "critica" && a["minimizado"] == false)
        .map(|a| a["id"].as_str().unwrap().to_string())
        .collect()
}

/// RNI-01: uma base de outra versão (ou um backup restaurado) pode ter várias
/// não críticas guardadas como abertas. Aparece aberta só a primeira; abrir
/// ou minimizar uma grava as outras como minimizadas, de uma vez, e nenhuma
/// marca antiga reabre outra depois. A crítica não entra na conta.
#[test]
fn n08_so_uma_nao_critica_aberta_mesmo_com_marcas_antigas() {
    let caminho = pasta();
    let (mut s, ana) = loja(&caminho);
    for id in ["receber", "backup", "outra_loja"] {
        crate::config::gravar(&s.banco_para_teste().conn, &format!("aviso_minimizado.{id}"), "0").unwrap();
    }
    let ordem: Vec<String> = ok(&mut s, "avisos_do_dia", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["id"] == "receber" || a["id"] == "backup")
        .map(|a| a["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ordem.len(), 2, "{ordem:?}");
    let (primeira, segunda) = (ordem[0].clone(), ordem[1].clone());
    assert_eq!(abertas(&mut s), std::slice::from_ref(&primeira), "só a primeira aberta");
    assert_eq!(aviso(&mut s, "outra_loja").unwrap()["minimizado"], false, "crítica continua aberta");
    ok(&mut s, "aviso_minimizar", json!({ "id": primeira, "minimizado": true }));
    assert!(abertas(&mut s).is_empty(), "minimizar a aberta não reabre a outra marca antiga");
    ok(&mut s, "aviso_minimizar", json!({ "id": segunda, "minimizado": false }));
    assert_eq!(abertas(&mut s), std::slice::from_ref(&segunda));
    ok(&mut s, "aviso_minimizar", json!({ "id": primeira, "minimizado": false }));
    assert_eq!(abertas(&mut s), std::slice::from_ref(&primeira), "abrir uma fecha a outra, no núcleo");
    drop(s);
    let mut s = crate::Servico::abrir(&caminho).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    assert_eq!(abertas(&mut s), [primeira], "continua assim depois de fechar e abrir");
    assert_eq!(aviso(&mut s, "outra_loja").unwrap()["minimizado"], false);
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}

/// Minimizar e expandir (clique, arrastar para a direita) fica guardado
/// neste computador e vale depois de fechar e abrir; a atenção minimizada
/// continua lá até a condição acabar. A crítica não minimiza.
#[test]
fn n07_minimizada_continua_depois_de_fechar_e_abrir() {
    let caminho = pasta();
    let (mut s, ana) = loja(&caminho);
    assert_eq!(aviso(&mut s, "backup").unwrap()["minimizado"], true, "atenção começa minimizada (quadrado)");
    assert_eq!(aviso(&mut s, "outra_loja").unwrap()["minimizado"], false, "crítica sempre expandida");
    let r = chamar(&mut s, "aviso_minimizar", json!({ "id": "outra_loja", "minimizado": true }));
    assert_eq!(r["ok"], false, "crítica não minimiza: {r}");
    ok(&mut s, "aviso_minimizar", json!({ "id": "backup", "minimizado": false }));
    drop(s);
    let mut s = crate::Servico::abrir(&caminho).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    let b = aviso(&mut s, "backup").expect("atenção continua depois de abrir de novo");
    assert_eq!(b["minimizado"], false, "expandida continua expandida: {b}");
    ok(&mut s, "aviso_minimizar", json!({ "id": "backup", "minimizado": true }));
    drop(s);
    let mut s = crate::Servico::abrir(&caminho).unwrap();
    ok(&mut s, "entrar", json!({ "id": ana, "senha": "1234" }));
    assert_eq!(aviso(&mut s, "backup").unwrap()["minimizado"], true, "minimizada continua minimizada");
    assert_eq!(aviso(&mut s, "outra_loja").unwrap()["minimizado"], false);
    // Só a condição resolve: com o backup feito, a atenção sai.
    crate::config::gravar(&s.banco_para_teste().conn, "backup_ultimo", &crate::tempo::agora()).unwrap();
    assert!(aviso(&mut s, "backup").is_none(), "resolvido, saiu");
    assert!(aviso(&mut s, "outra_loja").is_some(), "a outra ocorrência continua");
    drop(s);
    let _ = std::fs::remove_dir_all(caminho.parent().unwrap());
}
