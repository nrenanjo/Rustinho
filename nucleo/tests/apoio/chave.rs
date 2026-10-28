//! Só para os testes (FALHA-04): põe a chave da operação nos comandos que a
//! exigem, como as telas fazem, quando o teste não pôs uma.
//!
//! - Cada chamada sem chave ganha uma chave nova (uma operação nova).
//! - Uma senha do gerente pedida para um comando com estes dados
//!   (`gerente_liberar` com `comando` e `dados`) ganha uma chave, e as
//!   chamadas desse comando com os mesmos dados usam a mesma (como a tela, que
//!   repete a mesma tentativa depois da senha ou de um erro).
//!
//! Usado pelos testes de dentro do núcleo e pelos de `tests/`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use serde_json::{json, Value};

thread_local! {
    static PENDENTES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static CONTADOR: Cell<u64> = const { Cell::new(0) };
}

fn nova() -> String {
    let n = CONTADOR.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    format!("teste-{:?}-{n}", std::thread::current().id())
}

fn escrever(v: &Value, saida: &mut String) {
    match v {
        Value::Object(m) => {
            let mut chaves: Vec<&String> = m.keys().filter(|k| *k != "chaveOperacao").collect();
            chaves.sort();
            saida.push('{');
            for k in chaves {
                saida.push_str(&Value::String(k.clone()).to_string());
                saida.push(':');
                escrever(&m[k], saida);
                saida.push(',');
            }
            saida.push('}');
        }
        Value::Array(a) => {
            saida.push('[');
            for x in a {
                escrever(x, saida);
                saida.push(',');
            }
            saida.push(']');
        }
        outro => saida.push_str(&outro.to_string()),
    }
}

fn canonico(comando: &str, v: &Value) -> String {
    let mut s = format!("{comando}|");
    escrever(v, &mut s);
    s
}

/// Os dados do comando, com a chave da operação quando ele exige.
pub fn preparar(nao_repetir: &[&str], comando: &str, dados: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<Value>(if dados.trim().is_empty() { "{}" } else { dados }) else {
        return dados.to_string();
    };
    if !v.is_object() {
        return dados.to_string();
    }
    if comando == "gerente_liberar" {
        if let Some(c) = v["comando"].as_str().map(str::to_string) {
            if nao_repetir.contains(&c.as_str()) {
                let interno = v.get("dados").cloned().filter(|d| !d.is_null()).unwrap_or_else(|| json!({}));
                if interno.is_object() && interno.get("chaveOperacao").is_none() {
                    let k = nova();
                    PENDENTES.with(|p| p.borrow_mut().insert(canonico(&c, &interno), k.clone()));
                    let mut d = interno;
                    d["chaveOperacao"] = json!(k);
                    v["dados"] = d;
                }
            }
        }
        return v.to_string();
    }
    if nao_repetir.contains(&comando) && v.get("chaveOperacao").is_none() {
        // A mesma tentativa (depois da senha do gerente, ou de novo depois de
        // um erro) continua com a mesma chave, como na tela.
        let k = PENDENTES.with(|p| p.borrow().get(&canonico(comando, &v)).cloned()).unwrap_or_else(nova);
        v["chaveOperacao"] = json!(k);
    }
    v.to_string()
}

/// Depois da resposta: deu certo, a tentativa acabou (a próxima chamada igual
/// é outra operação, com chave nova — como a tela, que troca a chave).
pub fn depois(comando: &str, dados_preparados: &str, resposta: &str) {
    if comando == "gerente_liberar" {
        return;
    }
    let ok = serde_json::from_str::<Value>(resposta).ok().is_some_and(|r| r["ok"] == true);
    if !ok {
        return;
    }
    if let Ok(v) = serde_json::from_str::<Value>(dados_preparados) {
        PENDENTES.with(|p| p.borrow_mut().remove(&canonico(comando, &v)));
    }
}

/// Chama com a chave (`f` faz a chamada de verdade) e cuida do depois.
pub fn chamar(nao_repetir: &[&str], comando: &str, dados: &str, f: impl FnOnce(&str) -> String) -> String {
    let d = preparar(nao_repetir, comando, dados);
    let r = f(&d);
    depois(comando, &d, &r);
    r
}
