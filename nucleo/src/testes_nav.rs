//! Navegação contextual guiada (piloto: Peças › Importar XML › Configurações ›
//! Loja › volta a Peças). O que o núcleo garante:
//!
//! - a falta do CNPJ da loja na importação vem com o requisito que falta
//!   (`campo` = "requisito:loja.cnpj"), para a tela levar ao lugar certo sem
//!   depender do texto da mensagem;
//! - `requisito_conferir` diz, pelo banco, se o requisito foi satisfeito
//!   (CNPJ salvo e válido pela mesma regra de `config_salvar`); a tela só
//!   volta à origem quando ele diz que sim;
//! - `arquivos_conferir` diz quais arquivos escolhidos continuam no lugar,
//!   para a importação seguir sem perder as outras escolhas.
//!
//! Nada disso grava: os três comandos estão em `SO_LEITURA`.

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

/// Loja nova sem CNPJ (gerente Ana conectada), e uma pasta com um XML.
fn loja() -> (crate::Servico, std::path::PathBuf) {
    let mut s = crate::Servico::em_memoria().unwrap();
    ok(
        &mut s,
        "configurar_inicio",
        json!({ "loja": "Loja", "caixa": "Caixa 1", "nome": "Ana", "senha": "1234", "senha2": "1234" }),
    );
    let pasta = std::env::temp_dir().join(format!("rustinho-nav-{}", crate::tempo::novo_id()));
    std::fs::create_dir_all(&pasta).unwrap();
    std::fs::write(pasta.join("compra.xml"), crate::nfe_xml::EXEMPLO).unwrap();
    (s, pasta)
}

/// O que a tela Configurações manda ao salvar: o que leu, com o CNPJ trocado.
fn salvar_cnpj(s: &mut crate::Servico, cnpj: &str) -> Value {
    let c = ok(s, "config_ler", json!({}));
    let tabelas: Vec<Value> = c["tabelas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({ "numero": t["numero"], "nome": t["nome"], "lucroPadrao": t["lucroPadrao"] }))
        .collect();
    chamar(
        s,
        "config_salvar",
        json!({ "loja": c["loja"], "caixa": c["caixa"], "cnpj": cnpj, "uf": c["uf"], "endereco": c["endereco"],
                "telefone": c["telefone"], "imprimirDireto": c["imprimirDireto"], "tabelas": tabelas,
                "crediarioMulta": c["crediarioMulta"], "crediarioJuros": c["crediarioJuros"],
                "crediarioCarencia": c["crediarioCarencia"], "orcamentoValidade": c["orcamentoValidade"],
                "rodapeVenda": c["rodapeVenda"], "rodapeOrcamento": c["rodapeOrcamento"], "rodapeOs": c["rodapeOs"],
                "imprimirLocal": c["imprimirLocal"] }),
    )
}

fn satisfeito(s: &mut crate::Servico) -> bool {
    ok(s, "requisito_conferir", json!({ "id": "loja.cnpj" }))["satisfeito"].as_bool().unwrap()
}

/// Sem CNPJ, a prévia da importação recusa (como antes) e diz qual requisito
/// falta. Com o CNPJ salvo, a mesma prévia passa.
#[test]
fn nav01_previa_sem_cnpj_diz_o_requisito() {
    let (mut s, pasta) = loja();
    let arquivos = json!([pasta.display().to_string()]);
    let r = chamar(&mut s, "importar_previa", json!({ "arquivos": arquivos }));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "requisito:loja.cnpj", "{r}");
    assert!(r["erro"].as_str().unwrap().contains("CNPJ da loja"), "{r}");
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    let p = ok(&mut s, "importar_previa", json!({ "arquivos": arquivos }));
    assert_eq!(p["notas"], 1, "{p}");
}

/// O requisito só fica satisfeito com o CNPJ salvo e válido pela regra de
/// `config_salvar`. Campo inválido é recusado ao salvar e o requisito
/// continua pendente; vazio também.
#[test]
fn nav01_requisito_cnpj_so_com_o_salvamento_valido() {
    let (mut s, _) = loja();
    assert!(!satisfeito(&mut s), "loja nova sem CNPJ");
    let r = salvar_cnpj(&mut s, "11.444.777/0001-00");
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!("cnpj")), "{r}");
    assert!(!satisfeito(&mut s), "CNPJ inválido não foi salvo: continua pendente");
    assert_eq!(salvar_cnpj(&mut s, "")["ok"], true, "vazio é aceito ao salvar (regra de hoje)");
    assert!(!satisfeito(&mut s), "mas não satisfaz o requisito");
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    assert!(satisfeito(&mut s), "CNPJ válido salvo");
    // Requisito que o Rustinho não conhece: recusa clara (nada de "satisfeito").
    let r = chamar(&mut s, "requisito_conferir", json!({ "id": "nao.existe" }));
    assert_eq!(r["ok"], false, "{r}");
}

/// RNI-02: um CNPJ gravado mas inválido (de outra versão, de um backup
/// restaurado; `config_salvar` não deixa gravar assim) não passa pela
/// importação: a prévia recusa com o requisito, como sem CNPJ, em vez de
/// tratar as notas da loja como notas de fornecedores.
#[test]
fn nav02_cnpj_gravado_invalido_tambem_e_requisito() {
    let (mut s, pasta) = loja();
    crate::config::gravar(&s.banco_para_teste().conn, "loja_cnpj", "11.444.777/0001-00").unwrap();
    let arquivos = json!([pasta.display().to_string()]);
    let r = chamar(&mut s, "importar_previa", json!({ "arquivos": arquivos }));
    assert_eq!(r["ok"], false, "{r}");
    assert_eq!(r["campo"], "requisito:loja.cnpj", "{r}");
    assert!(r["erro"].as_str().unwrap().contains("não é válido"), "{r}");
    let c = ok(&mut s, "requisito_conferir", json!({ "id": "loja.cnpj" }));
    assert_eq!((c["satisfeito"].clone(), c["falta"].clone()), (json!(false), json!("O CNPJ salvo não é válido.")));
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    assert_eq!(ok(&mut s, "importar_previa", json!({ "arquivos": arquivos }))["notas"], 1);
}

fn contar(s: &mut crate::Servico, tabela: &str) -> i64 {
    s.banco_para_teste().conn.query_row(&format!("SELECT COUNT(*) FROM {tabela}"), [], |l| l.get(0)).unwrap()
}

/// Etapa 1 (0.9.17): Compras sem o CNPJ da loja, ou com um inválido gravado,
/// não importa: recusa com o requisito (a tela leva a Configurações › Loja),
/// sem gravar nada. Salvo o CNPJ válido, o mesmo arquivo importa — quando a
/// tela mandar de novo (nada é importado sozinho).
#[test]
fn nav03_compras_sem_cnpj_e_requisito() {
    let (mut s, pasta) = loja();
    let caminho = pasta.join("compra.xml").display().to_string();
    let antes = (contar(&mut s, "compra"), contar(&mut s, "pessoa"));
    let r = chamar(&mut s, "compra_importar", json!({ "caminho": caminho }));
    assert_eq!((r["ok"].clone(), r["campo"].clone()), (json!(false), json!("requisito:loja.cnpj")), "{r}");
    assert!(r["erro"].as_str().unwrap().contains("nota de compra"), "{r}");
    crate::config::gravar(&s.banco_para_teste().conn, "loja_cnpj", "11.444.777/0001-00").unwrap();
    let r = chamar(&mut s, "compra_importar", json!({ "caminho": caminho }));
    assert_eq!(r["campo"], "requisito:loja.cnpj", "inválido gravado também: {r}");
    assert!(r["erro"].as_str().unwrap().contains("não é válido"), "{r}");
    assert_eq!((contar(&mut s, "compra"), contar(&mut s, "pessoa")), antes, "nada gravado");
    // Inválido digitado na tela: recusado ao salvar, continua pendente.
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-99")["campo"], "cnpj");
    assert!(!satisfeito(&mut s));
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    assert!(satisfeito(&mut s));
    assert_eq!(contar(&mut s, "compra"), antes.0, "salvar o CNPJ não importa nada");
    let r = ok(&mut s, "compra_importar", json!({ "caminho": caminho }));
    assert_eq!(r["compras"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(r["conflitos"], json!([]), "{r}");
}

/// Etapa 1: nota emitida para outro CNPJ. Não entra (regra de hoje) e não é
/// só um erro de texto: vem em `conflitos`, com os dois documentos e a nota
/// para conferir. Nada é gravado e o CNPJ da loja não muda.
#[test]
fn nav04_nota_de_outro_cnpj_vem_como_conflito() {
    let (mut s, pasta) = loja();
    assert_eq!(salvar_cnpj(&mut s, "11.222.333/0001-81")["ok"], true);
    let antes = (contar(&mut s, "compra"), contar(&mut s, "pessoa"));
    let r = ok(&mut s, "compra_importar", json!({ "caminho": pasta.join("compra.xml").display().to_string() }));
    assert_eq!((r["compras"].clone(), r["erros"].clone()), (json!([]), json!([])), "{r}");
    let c = &r["conflitos"][0];
    assert_eq!(r["conflitos"].as_array().unwrap().len(), 1, "{r}");
    assert_eq!(c["arquivo"], "compra.xml");
    assert_eq!(c["lojaDocumento"], "11.222.333/0001-81");
    assert_eq!(c["destinatarioDocumento"], "11.444.777/0001-61");
    assert_ne!(c["emitenteDocumento"], "", "{c}");
    assert!(!c["itens"].as_array().unwrap().is_empty(), "a nota para conferir: {c}");
    for campo in ["numero", "emissao", "total", "lojaNome", "emitenteNome", "destinatarioNome"] {
        assert!(c[campo].is_string(), "{campo}: {c}");
    }
    assert_eq!((contar(&mut s, "compra"), contar(&mut s, "pessoa")), antes, "nada gravado");
    let loja: String = crate::config::ler(&s.banco_para_teste().conn, "loja_cnpj").unwrap().unwrap();
    assert_eq!(crate::documento::limpar(&loja), "11222333000181", "o CNPJ da loja não muda");
}

/// Etapa 1: o requisito do Fiscal (estado e CNPJ da loja), conferido pelo
/// banco, com o que falta dito; só satisfeito com os dois salvos e válidos.
#[test]
fn nav05_requisito_do_fiscal_uf_e_cnpj() {
    let (mut s, _) = loja();
    let conferir = |s: &mut crate::Servico| ok(s, "requisito_conferir", json!({ "id": "loja.uf_cnpj" }));
    let c = conferir(&mut s);
    assert_eq!(c["satisfeito"], false);
    let falta = c["falta"].as_str().unwrap().to_string();
    assert!(falta.contains("UF") && falta.contains("CNPJ"), "{falta}");
    assert_eq!(salvar_cnpj(&mut s, "11.444.777/0001-61")["ok"], true);
    let c = conferir(&mut s);
    assert_eq!(
        (c["satisfeito"].clone(), c["falta"].clone()),
        (json!(false), json!("O estado (UF) da loja ainda não está salvo."))
    );
    crate::config::gravar(&s.banco_para_teste().conn, "loja_uf", "ZZ").unwrap();
    assert_eq!(conferir(&mut s)["satisfeito"], false, "UF que não existe");
    crate::config::gravar(&s.banco_para_teste().conn, "loja_uf", "SP").unwrap();
    assert_eq!(conferir(&mut s)["satisfeito"], true);
    crate::config::gravar(&s.banco_para_teste().conn, "loja_cnpj", "").unwrap();
    let c = conferir(&mut s);
    assert_eq!(
        (c["satisfeito"].clone(), c["falta"].clone()),
        (json!(false), json!("O CNPJ da loja ainda não está salvo."))
    );
}

/// Arquivos que sumiram enquanto o usuário estava nas Configurações: a
/// conferência separa os que continuam dos que faltam, sem recusar tudo.
#[test]
fn nav01_arquivos_conferir_separa_os_que_faltam() {
    let (mut s, pasta) = loja();
    let um = pasta.join("compra.xml").display().to_string();
    let dois = pasta.join("sumiu.xml").display().to_string();
    let r = ok(&mut s, "arquivos_conferir", json!({ "arquivos": [um, dois, pasta.display().to_string()] }));
    assert_eq!(r["existem"], json!([um, pasta.display().to_string()]), "{r}");
    assert_eq!(r["faltam"], json!([dois]), "{r}");
}

/// Os comandos novos são só de leitura (o teste geral `so_leitura_nao_grava`
/// também passa por eles) e não precisam de gerente.
#[test]
fn nav01_comandos_novos_sao_so_leitura() {
    for c in ["requisito_conferir", "arquivos_conferir"] {
        assert!(crate::api::SO_LEITURA.contains(&c), "{c} fora de SO_LEITURA");
    }
}
