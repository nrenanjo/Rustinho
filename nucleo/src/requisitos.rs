//! Requisitos de uma operação que dependem de um cadastro ou de uma
//! configuração (navegação contextual guiada). A operação que não pode seguir
//! recusa com `Erro::campo("requisito:<id>", mensagem)`; a tela leva ao lugar
//! de resolver e só volta quando `conferir` diz, pelo banco, que o requisito
//! foi satisfeito. Nada aqui grava.

use rusqlite::Connection;
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::{config, documento};

/// Prefixo do `campo` do erro que diz qual requisito falta.
pub const PREFIXO: &str = "requisito:";
/// CNPJ da loja (importação de XML em Peças e em Compras).
pub const CNPJ_DA_LOJA: &str = "loja.cnpj";
pub const CAMPO_CNPJ_DA_LOJA: &str = "requisito:loja.cnpj";
/// Estado (UF) e CNPJ da loja (conferência fiscal).
pub const UF_E_CNPJ_DA_LOJA: &str = "loja.uf_cnpj";

/// Por que a importação de cadastros (Peças) precisa do CNPJ da loja.
pub const PARA_CADASTROS: &str = "É por ele que o Rustinho separa as notas da loja das notas de fornecedores.";
/// Por que a importação de compras precisa do CNPJ da loja.
pub const PARA_COMPRAS: &str = "É por ele que o Rustinho confere se a nota de compra foi emitida para a loja.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Situacao {
    pub id: String,
    pub satisfeito: bool,
    /// O que ainda falta (vazio quando satisfeito).
    pub falta: String,
}

/// O requisito está satisfeito pelo que está gravado no banco?
pub fn conferir(conn: &Connection, id: &str) -> Resultado<Situacao> {
    let (satisfeito, falta) = match id {
        CNPJ_DA_LOJA => match documento_da_loja(conn)? {
            Ok(_) => (true, String::new()),
            Err(falta) => (false, falta.to_string()),
        },
        UF_E_CNPJ_DA_LOJA => {
            let uf = config::ler(conn, "loja_uf")?.unwrap_or_default();
            let uf_ok = documento::UFS.contains(&uf.trim().to_uppercase().as_str());
            match (uf_ok, documento_da_loja(conn)?) {
                (true, Ok(_)) => (true, String::new()),
                (false, Ok(_)) => (false, "O estado (UF) da loja ainda não está salvo.".to_string()),
                (true, Err(falta)) => (false, falta.to_string()),
                (false, Err(falta)) => (false, format!("O estado (UF) da loja ainda não está salvo. {falta}")),
            }
        }
        _ => return Err(Erro::validacao(format!("Requisito desconhecido: {id}."))),
    };
    Ok(Situacao { id: id.to_string(), satisfeito, falta })
}

/// Para a operação que precisa do CNPJ da loja: o documento gravado (limpo),
/// ou a recusa com o requisito (a tela leva a Configurações › Loja). `para`
/// diz por que esta operação precisa dele.
pub fn exigir_cnpj_da_loja(conn: &Connection, para: &str) -> Resultado<String> {
    documento_da_loja(conn)?.map_err(|falta| {
        Erro::campo(CAMPO_CNPJ_DA_LOJA, format!("{falta} Informe o CNPJ da loja em Configurações → Loja. {para}"))
    })
}

/// O documento da loja gravado (limpo), ou o que falta nele. A mesma regra
/// para conferir o requisito e para a operação que o exige: um valor gravado
/// mas inválido (de outra versão, de um backup restaurado) conta como
/// faltando, senão as notas da loja passariam por notas de fornecedores.
fn documento_da_loja(conn: &Connection) -> Resultado<Result<String, &'static str>> {
    let cnpj = documento::limpar(&config::ler(conn, "loja_cnpj")?.unwrap_or_default());
    Ok(if cnpj.is_empty() {
        Err("O CNPJ da loja ainda não está salvo.")
    } else if !config::documento_da_loja_valido(&cnpj) {
        Err("O CNPJ salvo não é válido.")
    } else {
        Ok(cnpj)
    })
}

#[derive(Debug, Serialize)]
pub struct Arquivos {
    pub existem: Vec<String>,
    pub faltam: Vec<String>,
}

/// Quais dos arquivos (ou pastas) escolhidos continuam no lugar.
pub fn arquivos(lista: &[String]) -> Arquivos {
    let (existem, faltam) = lista.iter().cloned().partition(|c| std::path::Path::new(c).exists());
    Arquivos { existem, faltam }
}
