//! A nota fiscal emitida pelo Rustinho (0.9.19, etapa 3, pacote E; FIS-P1 e FIS-P2): a máquina de estados, o painel
//! e o detalhe.
//!
//! Estados: `rascunho` → `enviada` → `autorizada` | `rejeitada` | `denegada` | `desconhecida`; a consulta leva a
//! `autorizada`, `denegada`, `cancelada` ou `nao_recebida`; o rascunho, a rejeitada e a não recebida podem ser
//! `descartada`s. A assinatura é do emissor (o ACBrMonitorPLUS assina com o A3): o Rustinho não guarda nem vê a chave
//! privada.
//!
//! Regras (FIS-P2 e o texto A §13 e §14; R1 C-A1, C-M1 a C-M4):
//! - **cada passo é gravado antes de chamar o emissor** (`marcar_enviada` numa transação confirmada antes da chamada),
//!   e o resultado é gravado depois, em outra transação;
//! - **o número sai na hora de transmitir**, e não antes: não sobra número reservado e não usado (em AC, BA e SP não há
//!   mais inutilização desde 01/11/2026, Ajustes SINIEF 38/26 e 40/26, e a quebra de sequência presume nota emitida em
//!   contingência e não transmitida, Ajuste SINIEF 19/16, cl. 11ª, § 5º). O contador fica neste computador
//!   (`config` local, que a restauração preserva): restaurar um backup antigo não faz o número voltar. O número da
//!   rejeitada ou da não recebida que for descartada volta a ser usado (a SEFAZ não o registrou);
//! - só uma resposta **conclusiva** muda a nota de vez: autorizada (100, 120 e 150), denegada e, na consulta, cancelada e
//!   não recebida pelo código exato; rejeitada é todo código de 100 a 9999 fora dos resultados do processamento (MOC 7.0,
//!   Anexo I, 4.4.1) e dos não conclusivos, e nunca uma resposta com protocolo (`estado_da_resposta`). O resto (lote em
//!   processamento, serviço parado, duplicidade, número já usado, erro não catalogado, texto que não é código, resposta
//!   sem chave ou de outro emitente) fica `desconhecida`, e **não se reenvia**: só a consulta tira a nota desse estado;
//! - a resposta só vale para a nota que está esperando por ela (`enviada` ou `desconhecida`), e a chave de acesso tem
//!   de ser desta nota (44 dígitos, o dígito verificador, o modelo, a série e o número); a autorizada pede o protocolo;
//! - a nota só sai com o fiscal ligado neste computador e no ambiente em que foi criada;
//! - toda conversa com o emissor passa pela porta do FIS-LIGA (`fiscal_porta::com_adaptador`).
//!
//! Nesta etapa não há emissor de verdade (o adaptador ACBr é a T008, com os dados do proprietário): a tela só lê.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::erro::{Erro, Resultado};
use crate::fiscal_porta::{NotaParaEnviar, RespostaSefaz};
use crate::tempo;

/// De onde veio a resposta da SEFAZ: a transmissão (autorização) ou a consulta da situação da nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origem {
    Envio,
    Consulta,
}

/// Os códigos de rejeição que **não** dizem que a nota ficou fora da SEFAZ (MOC 7.0, Anexo I, 4.4.2): a duplicidade (204
/// e 539: a nota pode já estar autorizada), a nota já denegada (205), já inutilizada (206), já cancelada (218) ou com o
/// mesmo número já transmitida e aguardando (635) na base (K2 da R2: o número já existe lá e não pode voltar), o EPEC
/// registrado para a série e o número (692, F3-4 da R3), o "NF-e não consta na base" (217, que só tem sentido na
/// consulta), a chave diferente da existente (613), o consumo indevido (656: a SEFAZ nem olhou) e o erro não
/// catalogado (999). Comparados como número (Cód-2 da R3: "0206" é o 206).
/// O 691 (a chave diverge da do EPEC: o número está reservado por um EPEC) vem da R4 (N-3).
const REJEICAO_NAO_CONCLUSIVA: &[u32] = &[204, 205, 206, 217, 218, 539, 613, 635, 656, 691, 692, 999];

/// Os códigos de resultado do processamento (MOC 7.0, Anexo I, tabela 4.4.1), fora o 142, que também é rejeição
/// (4.4.2): não são rejeição da nota.
const RESULTADOS_DO_PROCESSAMENTO: &[u32] = &[
    100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 120, 124, 128, 135, 136, 137, 138, 139, 140, 150,
    151, 155,
];

/// O que uma resposta da SEFAZ quer dizer para a nota:
/// - 100, 120 e 150 (`nfe_xml::cstat_autorizado`: MOC 7.0, Anexo I, 4.4.1, e NT 2026.002): autorizada; 110 (4.4.1) e 301, 302 e 303 (4.4.3): denegada;
/// - na consulta: 101 e 151 (4.4.1) e 155 (Visão Geral, 5.9.4), cancelada; 217 ("NF-e não consta na base de dados da
///   SEFAZ"), não recebida;
/// - no envio, rejeitada: **todo código de 100 a 9999 que não é resultado do processamento (4.4.1) nem está em
///   [`REJEICAO_NAO_CONCLUSIVA`]** (F3-1 da R3). Assim entram as rejeições das NT em vigor sem precisar de lista: 126
///   (NT 2023.002), 129 (NT 2023.002), 156 a 188 (NT 2026.007, validação cadastral), os de 4 dígitos (NT 2025.002,
///   2026.005, 2026.007);
/// - todo o resto (os resultados 103 a 109 do lote e do serviço, código fora de 100 a 9999, texto que não é código)
///   não é conclusivo: desconhecida.
pub fn estado_da_resposta(origem: Origem, cstat: &str) -> &'static str {
    let c = cstat.trim();
    // C-2 da R4: autorização, denegação e cancelamento só com o código exato (sem zero à esquerda).
    match c {
        // 120: "Autorizado o uso da NF-e, com alerta" (NT 2026.002 v1.11, item 2.1: a nota é autorizada normalmente;
        // N-1 da R4).
        _ if crate::nfe_xml::cstat_autorizado(c) => return "autorizada",
        _ if crate::nfe_xml::cstat_denegado(c) => return "denegada",
        "101" | "151" | "155" if origem == Origem::Consulta => return "cancelada",
        "217" if origem == Origem::Consulta => return "nao_recebida",
        _ => {}
    }
    let Some(n) = (if (1..=4).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_digit()) {
        c.parse::<u32>().ok()
    } else {
        None
    }) else {
        return "desconhecida";
    };
    let rejeicao = (100..=9999).contains(&n)
        && !RESULTADOS_DO_PROCESSAMENTO.contains(&n)
        && !REJEICAO_NAO_CONCLUSIVA.contains(&n)
        // os de autorização e denegação com zero à esquerda ("0100") não são rejeição
        && !matches!(n, 100 | 110 | 120 | 150 | 301 | 302 | 303);
    if origem == Origem::Envio && rejeicao {
        "rejeitada"
    } else {
        "desconhecida"
    }
}

/// O código da UF no IBGE (o `cUF` das duas primeiras posições da chave; MOC 7.0, Visão Geral, 8.1).
pub fn codigo_da_uf(uf: &str) -> Option<&'static str> {
    Some(match uf.trim().to_uppercase().as_str() {
        "RO" => "11",
        "AC" => "12",
        "AM" => "13",
        "RR" => "14",
        "PA" => "15",
        "AP" => "16",
        "TO" => "17",
        "MA" => "21",
        "PI" => "22",
        "CE" => "23",
        "RN" => "24",
        "PB" => "25",
        "PE" => "26",
        "AL" => "27",
        "SE" => "28",
        "BA" => "29",
        "MG" => "31",
        "ES" => "32",
        "RJ" => "33",
        "SP" => "35",
        "PR" => "41",
        "SC" => "42",
        "RS" => "43",
        "MS" => "50",
        "MT" => "51",
        "GO" => "52",
        "DF" => "53",
        _ => return None,
    })
}

/// K3 da R2: a chave é da loja? A UF (posições 1–2) e o CNPJ do emitente (7–20) têm de ser os da loja. `Err`: a loja
/// ainda não informou o CNPJ ou a UF (sem eles, não há como saber de quem é a chave).
fn chave_e_da_loja(conn: &Connection, chave: &str) -> Result<bool, &'static str> {
    // F3-3 da R3: `limpar` (números e letras), como o CNPJ é guardado: o alfanumérico também confere.
    let cnpj = crate::documento::limpar(&crate::config::ler(conn, "loja_cnpj").ok().flatten().unwrap_or_default());
    let uf = crate::config::ler(conn, "loja_uf").ok().flatten().unwrap_or_default();
    let (Some(cuf), 14) = (codigo_da_uf(&uf), cnpj.len()) else {
        return Err("sem o CNPJ e a UF da loja, o Rustinho não confere de quem é a chave");
    };
    let c = chave.trim();
    Ok(c.len() == 44 && &c[..2] == cuf && c[6..20] == cnpj)
}

/// O dígito verificador da chave de acesso (C-7 da R4: a mesma conta de `nfe_xml::digito_da_chave`, módulo 11 com
/// pesos de 2 a 9; as letras do CNPJ alfanumérico valem o código ASCII menos 48). Os testes montam chaves com ela.
#[cfg(test)]
fn digito_da_chave(corpo: &[u8]) -> u8 {
    crate::nfe_xml::digito_da_chave(std::str::from_utf8(corpo).unwrap_or("")).unwrap_or(10) as u8
}

/// A chave de acesso confere com esta nota? 44 dígitos, o dígito verificador, e o modelo (posições 21–22), a série
/// (23–25) e o número (26–34) da nota (MOC 7.0, Anexo I, campo B03 e item 5.4).
///
/// F3-3 da R3: o CNPJ alfanumérico (gerado pela Receita desde 31/07/2026; NT Conjunta 2025.001 e NT 2026.004): as 12
/// primeiras posições do CNPJ na chave (7 a 18) podem ter letras maiúsculas; o resto é número, e o dígito verificador
/// usa o código ASCII menos 48 (o mesmo de `digito_da_chave`).
pub fn chave_e_desta_nota(chave: &str, modelo: i64, serie: i64, numero: i64) -> bool {
    // C-7 da R4: o formato e o dígito verificador são os de `nfe_xml` (uma regra só para a chave no programa).
    let c = chave.trim();
    if !crate::nfe_xml::chave_formato(c) {
        return false;
    }
    let campo = |de: usize, ate: usize| c[de..ate].parse::<i64>().ok();
    crate::nfe_xml::digito_da_chave(&c[..43]) == c[43..].parse::<u32>().ok()
        && campo(20, 22) == Some(modelo)
        && campo(22, 25) == Some(serie)
        && campo(25, 34) == Some(numero)
}

fn evento(
    tx: &Connection,
    nota_id: &str,
    de: &str,
    para: &str,
    detalhe: &str,
    usuario_id: Option<&str>,
) -> Resultado<()> {
    tx.execute(
        "INSERT INTO nota_fiscal_evento (id, nota_id, quando, de, para, detalhe, usuario_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![tempo::novo_id(), nota_id, tempo::agora(), de, para, detalhe, usuario_id],
    )?;
    Ok(())
}

fn estado(tx: &Connection, id: &str) -> Resultado<String> {
    tx.query_row("SELECT estado FROM nota_fiscal WHERE id = ?1", [id], |l| l.get(0))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))
}

fn mudar(tx: &Connection, id: &str, de: &str, para: &str, detalhe: &str, usuario_id: Option<&str>) -> Resultado<()> {
    tx.execute(
        "UPDATE nota_fiscal SET estado = ?2, atualizado_em = ?3 WHERE id = ?1",
        params![id, para, tempo::agora()],
    )?;
    evento(tx, id, de, para, detalhe, usuario_id)
}

/// Um rascunho de nota. `ambiente`: 1 produção, 2 homologação; `modelo`: 55 ou 65.
#[allow(clippy::too_many_arguments)]
pub fn criar_rascunho(
    tx: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    ambiente: i64,
    modelo: i64,
    serie: i64,
    venda_id: Option<&str>,
    valor: i64,
    destinatario_doc: &str,
    destinatario_nome: &str,
) -> Resultado<String> {
    if !matches!(ambiente, 1 | 2) {
        return Err(Erro::validacao("Ambiente fiscal inválido."));
    }
    if !matches!(modelo, 55 | 65) {
        return Err(Erro::validacao("Modelo de nota inválido (55 ou 65)."));
    }
    if !(0..=999).contains(&serie) {
        return Err(Erro::validacao("Série inválida (0 a 999)."));
    }
    let id = tempo::novo_id();
    let agora = tempo::agora();
    tx.execute(
        "INSERT INTO nota_fiscal (id, ambiente, modelo, serie, numero, estado, venda_id, destinatario_doc,
                                  destinatario_nome, valor, criado_em, atualizado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, 0, 'rascunho', ?5, ?6, ?7, ?8, ?9, ?9, ?10)",
        params![id, ambiente, modelo, serie, venda_id, destinatario_doc, destinatario_nome, valor, agora, dispositivo],
    )?;
    evento(tx, &id, "", "rascunho", "Nota criada", Some(usuario_id))?;
    Ok(id)
}

/// A chave do contador deste computador (o último número usado) e a do piso (a última nota que o sistema de antes
/// emitiu nessa série, achada pela descoberta ou informada). As duas ficam fora de `CONFIG_DA_LOJA`: só deste
/// computador, preservadas na restauração.
pub fn chave_do_contador(ambiente: i64, modelo: i64, serie: i64) -> String {
    format!("fiscal_numero:{ambiente}:{modelo}:{serie}")
}
pub fn chave_do_piso(ambiente: i64, modelo: i64, serie: i64) -> String {
    format!("fiscal_ultimo_de_antes:{ambiente}:{modelo}:{serie}")
}
fn chave_dos_livres(ambiente: i64, modelo: i64, serie: i64) -> String {
    format!("fiscal_numeros_livres:{ambiente}:{modelo}:{serie}")
}

fn ler_numero(tx: &Connection, chave: &str) -> Resultado<i64> {
    Ok(crate::config::ler(tx, chave)?.and_then(|v| v.trim().parse().ok()).unwrap_or(0))
}

fn ler_livres(tx: &Connection, chave: &str) -> Resultado<Vec<i64>> {
    let mut v: Vec<i64> = crate::config::ler(tx, chave)?
        .unwrap_or_default()
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .filter(|n: &i64| *n > 0)
        .collect();
    v.sort_unstable();
    v.dedup();
    Ok(v)
}

fn gravar_livres(tx: &Connection, chave: &str, v: &[i64]) -> Resultado<()> {
    crate::config::gravar(tx, chave, &v.iter().map(i64::to_string).collect::<Vec<_>>().join(","))
}

/// C-M4 da R1: a chave (que viaja para todos os caixas) do computador que emite na série.
pub fn chave_da_estacao(modelo: i64, serie: i64) -> String {
    format!("{}{modelo}:{serie}", crate::sync::PREFIXO_ESTACAO_FISCAL)
}

/// O computador que emite na série: (dispositivo, nome do caixa), se já há um.
pub fn estacao_da_serie(conn: &Connection, modelo: i64, serie: i64) -> Resultado<Option<(String, String)>> {
    Ok(crate::config::ler(conn, &chave_da_estacao(modelo, serie))?
        .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
        .and_then(|v| Some((v["dispositivo"].as_str()?.to_string(), v["nome"].as_str().unwrap_or("").to_string()))))
}

/// C-M4 da R1: a série passa a ser deste computador. Recusa a série que já é de outro (dois computadores na mesma
/// série repetiriam números). O nome do caixa vai junto, para a mensagem.
pub fn reservar_serie(tx: &Connection, dispositivo: &str, modelo: i64, serie: i64) -> Resultado<()> {
    if let Some((outro, nome)) = estacao_da_serie(tx, modelo, serie)? {
        if outro != dispositivo {
            let quem =
                if nome.trim().is_empty() { "outro computador da loja".to_string() } else { format!("o caixa {nome}") };
            return Err(Erro::validacao(format!(
                "A série {serie} da {} já é usada por {quem}. Dois computadores na mesma série repetiriam o número da nota: use outra série (combine com o contador).",
                if modelo == 65 { "NFC-e" } else { "NF-e" }
            )));
        }
        return Ok(());
    }
    let nome = crate::config::nome_caixa(tx)?;
    crate::config::gravar(
        tx,
        &chave_da_estacao(modelo, serie),
        &serde_json::json!({ "dispositivo": dispositivo, "nome": nome }).to_string(),
    )
}

/// O número da próxima nota da série: primeiro um número devolvido por uma nota descartada (a SEFAZ não o registrou);
/// senão, um depois do maior entre o contador deste computador, o piso e as notas da tabela.
fn proximo_numero(tx: &Connection, ambiente: i64, modelo: i64, serie: i64) -> Resultado<i64> {
    let chave_livres = chave_dos_livres(ambiente, modelo, serie);
    let mut livres = ler_livres(tx, &chave_livres)?;
    while !livres.is_empty() {
        let n = livres.remove(0);
        let usado: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM nota_fiscal WHERE ambiente = ?1 AND modelo = ?2 AND serie = ?3 AND numero = ?4)",
            params![ambiente, modelo, serie, n],
            |l| l.get(0),
        )?;
        if !usado {
            gravar_livres(tx, &chave_livres, &livres)?;
            return Ok(n);
        }
    }
    gravar_livres(tx, &chave_livres, &livres)?;
    let da_tabela: i64 = tx.query_row(
        "SELECT COALESCE(MAX(numero), 0) FROM nota_fiscal WHERE ambiente = ?1 AND modelo = ?2 AND serie = ?3",
        params![ambiente, modelo, serie],
        |l| l.get(0),
    )?;
    let contador = chave_do_contador(ambiente, modelo, serie);
    let n = ler_numero(tx, &contador)?.max(ler_numero(tx, &chave_do_piso(ambiente, modelo, serie))?).max(da_tabela) + 1;
    if n > 999_999_999 {
        return Err(Erro::validacao("A numeração desta série chegou ao fim (999.999.999). Use outra série."));
    }
    crate::config::gravar(tx, &contador, &n.to_string())?;
    Ok(n)
}

/// Os dados que o emissor recebe.
pub fn para_enviar(tx: &Connection, id: &str) -> Resultado<NotaParaEnviar> {
    tx.query_row(
        "SELECT id, ambiente, modelo, serie, numero, chave, valor, destinatario_doc, destinatario_nome
           FROM nota_fiscal WHERE id = ?1",
        [id],
        |l| {
            Ok(NotaParaEnviar {
                id: l.get(0)?,
                ambiente: l.get(1)?,
                modelo: l.get(2)?,
                serie: l.get(3)?,
                numero: l.get(4)?,
                chave: l.get(5)?,
                valor: l.get(6)?,
                destinatario_doc: l.get(7)?,
                destinatario_nome: l.get(8)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))
}

/// O texto de quando o Rustinho não sabe se a SEFAZ recebeu a nota.
pub const TEXTO_CONSULTE_ANTES: &str =
    "O Rustinho não sabe se a SEFAZ recebeu esta nota. Não transmita de novo: consulte na SEFAZ primeiro.";

/// Grava "enviada" **antes** de chamar o emissor, e dá o número à nota que ainda não tem. Só o rascunho, a rejeitada
/// (corrigida, à mão, com o mesmo número) e a não recebida se transmitem; a enviada e a desconhecida não: consulte
/// antes.
pub fn marcar_enviada(tx: &Connection, usuario_id: &str, id: &str) -> Resultado<()> {
    let e = estado(tx, id)?;
    match e.as_str() {
        "rascunho" | "rejeitada" | "nao_recebida" => {}
        "enviada" | "desconhecida" => return Err(Erro::validacao(TEXTO_CONSULTE_ANTES)),
        _ => return Err(Erro::validacao(format!("Uma nota {} não se transmite.", nome_do_estado(&e)))),
    }
    let (ambiente, modelo, serie, numero): (i64, i64, i64, i64) =
        tx.query_row("SELECT ambiente, modelo, serie, numero FROM nota_fiscal WHERE id = ?1", [id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?))
        })?;
    // C-M4 da R1: a série é deste computador (a primeira nota a reserva; a de outro computador, não sai daqui).
    let eu = crate::config::ler(tx, "dispositivo_id")?.unwrap_or_default();
    reservar_serie(tx, &eu, modelo, serie)?;
    let detalhe = if numero == 0 {
        let n = proximo_numero(tx, ambiente, modelo, serie)?;
        tx.execute("UPDATE nota_fiscal SET numero = ?2 WHERE id = ?1", params![id, n])?;
        format!("Transmissão começou, com o número {n}")
    } else {
        "Transmissão começou".to_string()
    };
    mudar(tx, id, &e, "enviada", &detalhe, Some(usuario_id))
}

/// Descarta a nota que não chegou à SEFAZ (o rascunho, a rejeitada e a não recebida). O número dela, se tinha, volta a
/// ser usado pela próxima nota da série: a SEFAZ não o registrou.
pub fn descartar(tx: &Connection, usuario_id: &str, id: &str) -> Resultado<()> {
    let e = estado(tx, id)?;
    if !matches!(e.as_str(), "rascunho" | "rejeitada" | "nao_recebida") {
        return Err(Erro::validacao(if matches!(e.as_str(), "enviada" | "desconhecida") {
            TEXTO_CONSULTE_ANTES.to_string()
        } else {
            format!("Uma nota {} não se descarta.", nome_do_estado(&e))
        }));
    }
    let (ambiente, modelo, serie, numero): (i64, i64, i64, i64) =
        tx.query_row("SELECT ambiente, modelo, serie, numero FROM nota_fiscal WHERE id = ?1", [id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?))
        })?;
    let detalhe = if numero > 0 {
        let chave = chave_dos_livres(ambiente, modelo, serie);
        let mut livres = ler_livres(tx, &chave)?;
        livres.push(numero);
        gravar_livres(tx, &chave, &livres)?;
        tx.execute("UPDATE nota_fiscal SET numero = 0, chave = '' WHERE id = ?1", [id])?;
        format!("Nota descartada; o número {numero} volta para a próxima nota")
    } else {
        "Nota descartada".to_string()
    };
    mudar(tx, id, &e, "descartada", &detalhe, Some(usuario_id))
}

/// Grava o resultado de uma transmissão (`Origem::Envio`) ou de uma consulta (`Origem::Consulta`). `resposta`: `Ok`
/// com a resposta da SEFAZ, ou `Err` com o que aconteceu (sem resposta conclusiva). Só a nota que espera uma resposta
/// (`enviada`, e na consulta também a `desconhecida`) a recebe.
pub fn registrar_resposta(
    tx: &Connection,
    usuario_id: &str,
    id: &str,
    origem: Origem,
    resposta: Result<&RespostaSefaz, &str>,
) -> Resultado<String> {
    let e = estado(tx, id)?;
    let espera = match origem {
        Origem::Envio => e == "enviada",
        Origem::Consulta => matches!(e.as_str(), "enviada" | "desconhecida"),
    };
    if !espera {
        return Err(Erro::validacao(format!(
            "Esta nota está {} e não espera resposta da SEFAZ: a resposta não foi gravada.",
            nome_do_estado(&e)
        )));
    }
    let (modelo, serie, numero, chave_antes): (i64, i64, i64, String) =
        tx.query_row("SELECT modelo, serie, numero, chave FROM nota_fiscal WHERE id = ?1", [id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?))
        })?;
    let desconhecida = |o_que: &str| -> Resultado<String> {
        mudar(tx, id, &e, "desconhecida", &format!("Sem resposta conclusiva: {o_que}"), Some(usuario_id))?;
        Ok("desconhecida".into())
    };
    let r = match resposta {
        Err(o_que) => return desconhecida(o_que),
        Ok(r) => r,
    };
    let cstat = r.cstat.trim();
    if cstat.is_empty() {
        return desconhecida("resposta sem cStat");
    }
    let mut novo = estado_da_resposta(origem, cstat);
    // N-1 da R4 (defesa): a rejeição não gera protocolo (MOC 7.0, Visão Geral, 4.3.5: o nProt identifica a autorização,
    // a denegação, o cancelamento e a inutilização; C-2 da R5).
    // Uma resposta com protocolo nunca vira "rejeitada": um código novo de sucesso de uma NT futura fica sem resposta
    // (e a consulta decide), e o número não volta pelo descarte.
    if novo == "rejeitada" && !r.protocolo.trim().is_empty() {
        novo = "desconhecida";
    }
    let chave = r.chave.trim();
    // A rejeição e a "não consta" não criam a nota na SEFAZ: a chave pode faltar; se vier, tem de ser desta nota.
    let pede_chave = !matches!(novo, "rejeitada" | "nao_recebida");
    if (pede_chave || !chave.is_empty()) && novo != "desconhecida" {
        if !chave_e_desta_nota(chave, modelo, serie, numero) {
            return desconhecida(&format!("{cstat} - {}; a chave de acesso não é desta nota", r.motivo.trim()));
        }
        // K3 da R2: o mesmo modelo, série e número de outro emitente (outra loja, outro CNPJ) não é esta nota.
        match chave_e_da_loja(tx, chave) {
            Ok(true) => {}
            Ok(false) => {
                return desconhecida(&format!(
                    "{cstat} - {}; a chave de acesso é de outro emitente (CNPJ ou UF diferente da loja)",
                    r.motivo.trim()
                ))
            }
            Err(o_que) => return desconhecida(&format!("{cstat} - {}; {o_que}", r.motivo.trim())),
        }
        if !chave_antes.is_empty() && chave != chave_antes {
            return desconhecida(&format!("{cstat} - {}; resposta de outra nota (outra chave)", r.motivo.trim()));
        }
    }
    if novo == "autorizada" && r.protocolo.trim().is_empty() {
        return desconhecida(&format!("{cstat} - {}; a autorização veio sem o protocolo", r.motivo.trim()));
    }
    // A nota que não entrou na SEFAZ não guarda chave: a correção pode trazer outra (outro código numérico).
    let guarda_chave = matches!(novo, "autorizada" | "denegada" | "cancelada");
    tx.execute(
        "UPDATE nota_fiscal SET cstat = ?2, motivo = ?3,
                                protocolo = CASE WHEN ?7 AND ?4 <> '' THEN ?4 ELSE protocolo END,
                                chave = CASE WHEN ?7 THEN CASE WHEN chave = '' THEN ?5 ELSE chave END ELSE '' END,
                                xml_nome = CASE WHEN ?6 <> '' THEN ?6 ELSE xml_nome END
          WHERE id = ?1",
        params![id, cstat, r.motivo, r.protocolo.trim(), chave, r.xml_nome, guarda_chave],
    )?;
    if novo == "desconhecida" {
        return desconhecida(&format!("{cstat} - {}", r.motivo.trim()));
    }
    mudar(tx, id, &e, novo, &format!("{cstat} - {}", r.motivo), Some(usuario_id))?;
    Ok(novo.into())
}

/// O nome do estado para as mensagens.
pub fn nome_do_estado(e: &str) -> &str {
    match e {
        "rascunho" => "em rascunho",
        "enviada" => "enviada, esperando a resposta",
        "autorizada" => "autorizada",
        "rejeitada" => "rejeitada",
        "denegada" => "denegada",
        "desconhecida" => "sem resposta conclusiva",
        "nao_recebida" => "não recebida pela SEFAZ",
        "cancelada" => "cancelada",
        "descartada" => "descartada",
        outro => outro,
    }
}

// ------------------------------------------------------------------ painel (FIS-P1) e detalhe (FIS-P2)

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotaResumo {
    pub id: String,
    pub modelo: i64,
    pub serie: i64,
    pub numero: i64,
    pub estado: String,
    pub destinatario_doc: String,
    pub destinatario_nome: String,
    pub valor: i64,
    pub criado_em: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Painel {
    /// 1 = produção, 2 = homologação: sempre à vista.
    pub ambiente: i64,
    /// Os quatro indicadores do texto A §12 (R1 C-M9).
    pub autorizadas: i64,
    /// Rascunho, enviada, sem resposta conclusiva e não recebida.
    pub pendentes: i64,
    /// Rejeitada e denegada.
    pub rejeitadas: i64,
    pub canceladas: i64,
    /// Das pendentes, as que pedem consulta na SEFAZ antes de qualquer coisa (aviso à parte, sem ser indicador).
    pub sem_resposta: i64,
    pub notas: Vec<NotaResumo>,
    pub total: i64,
}

/// O ambiente fiscal deste computador (homologação de fábrica; a troca é o FIS-P3).
pub fn ambiente(conn: &Connection) -> Resultado<i64> {
    Ok(crate::config::ler(conn, "fiscal_ambiente")?
        .and_then(|v| v.parse().ok())
        .filter(|v| matches!(v, 1 | 2))
        .unwrap_or(2))
}

/// Os filtros do painel. `indicador`: "autorizadas", "pendentes", "rejeitadas", "canceladas", "sem_resposta" ou vazio
/// (todas).
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filtros {
    pub indicador: String,
    pub texto: String,
    /// "AAAA-MM-DD", no dia deste computador.
    pub de: String,
    pub ate: String,
    /// B2 da R1: o período em dias até hoje, contados no dia deste computador (1 = hoje; 7 = os últimos 7 dias). Vence
    /// o `de`. A tela manda os dias, e não a data: a data em UTC errava depois das 21h.
    /// 0 = os dois; 55 ou 65.
    pub modelo: i64,
    pub dias: i64,
}

fn estados_do_indicador(indicador: &str) -> &'static [&'static str] {
    match indicador {
        "autorizadas" => &["autorizada"],
        "pendentes" => &["rascunho", "enviada", "desconhecida", "nao_recebida"],
        "rejeitadas" => &["rejeitada", "denegada"],
        "canceladas" => &["cancelada"],
        "sem_resposta" => &["enviada", "desconhecida"],
        _ => &[],
    }
}

pub fn painel(conn: &Connection, f: &Filtros) -> Resultado<Painel> {
    let amb = ambiente(conn)?;
    let conta = |estados: &[&str]| -> Resultado<i64> {
        let lista = estados.iter().map(|e| format!("'{e}'")).collect::<Vec<_>>().join(", ");
        Ok(conn.query_row(
            &format!("SELECT COUNT(*) FROM nota_fiscal WHERE ambiente = ?1 AND estado IN ({lista})"),
            [amb],
            |l| l.get(0),
        )?)
    };
    let estados = estados_do_indicador(f.indicador.trim());
    let filtro_estado = if estados.is_empty() {
        String::new()
    } else {
        format!(" AND estado IN ({})", estados.iter().map(|e| format!("'{e}'")).collect::<Vec<_>>().join(", "))
    };
    let texto = format!("%{}%", f.texto.trim());
    let de = if f.dias > 0 {
        conn.query_row("SELECT date('now', 'localtime', ?1)", [format!("-{} days", f.dias - 1)], |l| l.get(0))?
    } else if f.de.trim().is_empty() {
        "0000-00-00".to_string()
    } else {
        f.de.trim().to_string()
    };
    let ate = if f.ate.trim().is_empty() { "9999-12-31".to_string() } else { f.ate.trim().to_string() };
    let mut c = conn.prepare(&format!(
        "SELECT id, modelo, serie, numero, estado, destinatario_doc, destinatario_nome, valor, criado_em
           FROM nota_fiscal
          WHERE ambiente = ?1{filtro_estado}
            AND (?2 = 0 OR modelo = ?2)
            AND date(criado_em, 'localtime') BETWEEN ?3 AND ?4
            AND estado <> 'descartada'
            AND (CAST(numero AS TEXT) LIKE ?5 OR destinatario_nome LIKE ?5 OR (?6 AND chave LIKE ?5))
          ORDER BY criado_em DESC, numero DESC"
    ))?;
    let notas: Vec<NotaResumo> = c
        // K8 da R2: a chave (44 dígitos) só entra na pesquisa com 10 dígitos ou mais; pesquisar "3" achava quase todas.
        .query_map(params![amb, f.modelo, de, ate, texto, crate::documento::limpar(&f.texto).len() >= 10], |l| {
            Ok(NotaResumo {
                id: l.get(0)?,
                modelo: l.get(1)?,
                serie: l.get(2)?,
                numero: l.get(3)?,
                estado: l.get(4)?,
                destinatario_doc: l.get(5)?,
                destinatario_nome: l.get(6)?,
                valor: l.get(7)?,
                criado_em: l.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Painel {
        ambiente: amb,
        autorizadas: conta(estados_do_indicador("autorizadas"))?,
        pendentes: conta(estados_do_indicador("pendentes"))?,
        rejeitadas: conta(estados_do_indicador("rejeitadas"))?,
        canceladas: conta(estados_do_indicador("canceladas"))?,
        sem_resposta: conta(estados_do_indicador("sem_resposta"))?,
        total: notas.len() as i64,
        notas,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evento {
    pub quando: String,
    pub de: String,
    pub para: String,
    pub detalhe: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Detalhe {
    pub nota: NotaResumo,
    pub ambiente: i64,
    pub chave: String,
    pub protocolo: String,
    /// O código e a mensagem da SEFAZ, na íntegra (para copiar).
    pub cstat: String,
    pub motivo: String,
    pub xml_nome: String,
    /// Primeiro a explicação humana (FIS-P2).
    pub explicacao: String,
    /// Só a desconhecida (e a enviada que ficou sem volta) pedem consulta; nenhuma se reenvia sozinha.
    pub pede_consulta: bool,
    /// O rascunho, a rejeitada e a não recebida se transmitem (a rejeitada depois de corrigida).
    pub pode_transmitir: bool,
    /// O rascunho, a rejeitada e a não recebida se descartam (o número volta).
    pub pode_descartar: bool,
    pub eventos: Vec<Evento>,
}

/// A explicação humana do estado (FIS-P2: primeiro ela; o código vem depois).
pub fn explicacao(estado: &str) -> &'static str {
    match estado {
        "rascunho" => "Esta nota ainda não foi transmitida. O número sai na hora de transmitir.",
        "enviada" => "A transmissão começou e a resposta ainda não chegou. Não transmita de novo: consulte na SEFAZ.",
        "autorizada" => "A SEFAZ autorizou esta nota.",
        "rejeitada" => "A SEFAZ recusou esta nota. Veja o motivo abaixo. Se ele fala de um dado cadastrado (da peça, do cliente ou da loja), acerte o cadastro; se não, chame o contador ou o suporte. Quando o emissor estiver ligado, ela pode ser transmitida de novo, com o mesmo número; ou pode ser descartada.",
        "denegada" => "A SEFAZ denegou esta nota (problema no cadastro do emitente ou do destinatário). O número não pode ser usado de novo.",
        "desconhecida" => "O Rustinho não sabe se a SEFAZ recebeu esta nota: a resposta não veio ou não foi conclusiva. Não transmita de novo: consulte na SEFAZ primeiro.",
        "nao_recebida" => "A SEFAZ informou que esta nota não chegou lá. Ela pode ser transmitida de novo (quando o emissor estiver ligado) ou descartada.",
        "cancelada" => "Esta nota foi cancelada.",
        "descartada" => "Esta nota foi descartada antes de chegar à SEFAZ.",
        _ => "",
    }
}

pub fn detalhe(conn: &Connection, id: &str) -> Resultado<Detalhe> {
    let (nota, ambiente, chave, protocolo, cstat, motivo, xml_nome) = conn
        .query_row(
            "SELECT id, modelo, serie, numero, estado, destinatario_doc, destinatario_nome, valor, criado_em,
                    ambiente, chave, protocolo, cstat, motivo, xml_nome
               FROM nota_fiscal WHERE id = ?1",
            [id],
            |l| {
                Ok((
                    NotaResumo {
                        id: l.get(0)?,
                        modelo: l.get(1)?,
                        serie: l.get(2)?,
                        numero: l.get(3)?,
                        estado: l.get(4)?,
                        destinatario_doc: l.get(5)?,
                        destinatario_nome: l.get(6)?,
                        valor: l.get(7)?,
                        criado_em: l.get(8)?,
                    },
                    l.get::<_, i64>(9)?,
                    l.get::<_, String>(10)?,
                    l.get::<_, String>(11)?,
                    l.get::<_, String>(12)?,
                    l.get::<_, String>(13)?,
                    l.get::<_, String>(14)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))?;
    let mut c = conn.prepare(
        "SELECT quando, de, para, detalhe FROM nota_fiscal_evento WHERE nota_id = ?1 ORDER BY quando, rowid",
    )?;
    let eventos = c
        .query_map([id], |l| Ok(Evento { quando: l.get(0)?, de: l.get(1)?, para: l.get(2)?, detalhe: l.get(3)? }))?
        .collect::<Result<_, _>>()?;
    let pede_consulta = matches!(nota.estado.as_str(), "desconhecida" | "enviada");
    let livre = matches!(nota.estado.as_str(), "rascunho" | "rejeitada" | "nao_recebida");
    Ok(Detalhe {
        pode_transmitir: livre,
        pode_descartar: livre,
        explicacao: explicacao(&nota.estado).to_string(),
        nota,
        ambiente,
        chave,
        protocolo,
        cstat,
        motivo,
        xml_nome,
        pede_consulta,
        eventos,
    })
}

// ------------------------------------------------------------------ troca de ambiente (FIS-P3)

/// O que impede trocar de ambiente agora (texto A §14). Vazio: pode trocar.
/// - o certificado precisa estar liberado (FIS-LIGA) e testado (o teste de conexão chega com o emissor, T008);
/// - a série da NFC-e e a da NF-e precisam estar definidas (dados da loja, com o contador);
/// - o credenciamento da loja na SEFAZ precisa estar confirmado;
/// - nenhuma nota do ambiente atual pode estar sem resposta conclusiva ou no meio do caminho (consultar antes).
pub fn impedimentos_da_troca(conn: &Connection) -> Resultado<Vec<String>> {
    let mut v = Vec::new();
    // B7 da R1: voltar para a homologação não pede certificado, série nem credenciamento; só as notas no meio do caminho.
    if ambiente(conn)? == 1 {
        notas_no_meio(conn, &mut v)?;
        return Ok(v);
    }
    if !crate::fiscal_porta::ligado(conn)? {
        v.push("O uso do certificado A3 está desligado neste computador.".to_string());
    } else if !crate::config::ligado(conn, "fiscal_certificado_testado")? {
        v.push("O certificado ainda não foi testado com a SEFAZ (o teste chega junto com o emissor).".to_string());
    }
    for (chave, nome) in [("fiscal_serie_nfce", "NFC-e"), ("fiscal_serie_nfe", "NF-e")] {
        if crate::config::ler(conn, chave)?.filter(|s| !s.trim().is_empty()).is_none() {
            v.push(format!("A série da {nome} ainda não foi definida (com o contador)."));
        }
    }
    if !crate::config::ligado(conn, "fiscal_credenciado")? {
        v.push("O credenciamento da loja na SEFAZ ainda não foi confirmado.".to_string());
    }
    notas_no_meio(conn, &mut v)?;
    Ok(v)
}

/// As notas do ambiente atual que podem estar na SEFAZ sem o Rustinho saber (consultar antes de trocar).
fn notas_no_meio(conn: &Connection, v: &mut Vec<String>) -> Resultado<()> {
    let atual = ambiente(conn)?;
    let soltas: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nota_fiscal WHERE ambiente = ?1 AND estado IN ('enviada', 'desconhecida')",
        [atual],
        |l| l.get(0),
    )?;
    if soltas > 0 {
        v.push(format!(
            "{soltas} {} deste ambiente sem resposta conclusiva ou no meio do caminho: consulte antes de trocar.",
            if soltas == 1 { "nota" } else { "notas" }
        ));
    }
    Ok(())
}

fn nome_do_ambiente(a: i64) -> &'static str {
    if a == 1 {
        "produção"
    } else {
        "homologação"
    }
}

/// O que se confere antes de pedir a senha (o bloqueio não gasta tentativa de senha, e a senha nunca o contorna).
pub fn conferir_antes_da_senha(conn: &Connection, gerente: bool, para: i64, confirmacao: &str) -> Resultado<()> {
    if !gerente {
        return Err(Erro::validacao("Só um gerente troca o ambiente fiscal."));
    }
    if !matches!(para, 1 | 2) {
        return Err(Erro::validacao("Ambiente fiscal inválido."));
    }
    if ambiente(conn)? == para {
        return Ok(());
    }
    let impedimentos = impedimentos_da_troca(conn)?;
    if !impedimentos.is_empty() {
        return Err(Erro::validacao(format!("Ainda não dá para trocar: {}", impedimentos.join(" "))));
    }
    if para == 1 && confirmacao.trim() != "PRODUÇÃO" {
        return Err(Erro::campo("confirmacao", "Para ir para a produção, digite PRODUÇÃO."));
    }
    Ok(())
}

/// Troca o ambiente fiscal deste computador. Só o gerente, só sem impedimento; ir para a produção pede digitar
/// PRODUÇÃO e a senha de um gerente (texto A §14: digitar PRODUÇÃO e a senha nunca contornam um bloqueio). A senha é
/// conferida antes, por `fiscal_porta::conferir_senha_do_gerente` (com a espera e o registro do erro, B1 da R1):
/// `liberou` é o nome do gerente. O registro diz quem, quando e de qual para qual.
pub fn trocar_ambiente(
    tx: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    gerente: bool,
    para: i64,
    confirmacao: &str,
    liberou: Option<&str>,
) -> Resultado<i64> {
    conferir_antes_da_senha(tx, gerente, para, confirmacao)?;
    let de = ambiente(tx)?;
    if de == para {
        return Ok(para);
    }
    if para == 1 && liberou.is_none_or(|n| n.trim().is_empty()) {
        return Err(Erro::campo("senha", "Digite a senha do gerente para ir para a produção."));
    }
    crate::config::gravar(tx, "fiscal_ambiente", &para.to_string())?;
    let nome = crate::usuarios::buscar(tx, usuario_id).map(|u| u.nome).unwrap_or_default();
    crate::auditoria::registrar(
        tx,
        dispositivo,
        Some(usuario_id),
        "fiscal.ambiente",
        "config",
        "fiscal_ambiente",
        &[format!(
            "{nome} trocou o ambiente fiscal deste computador de {} para {}",
            nome_do_ambiente(de),
            nome_do_ambiente(para)
        )],
    )?;
    Ok(para)
}

/// C-M2 e C-M3 da R1: antes de gravar qualquer coisa, a nota só fala com a SEFAZ com o fiscal ligado neste computador
/// e no ambiente em que foi criada. Recusa sem mudar a nota (nada saiu do computador).
/// `consulta`: é uma consulta (a nota já saiu); a mensagem diz que a consulta não foi feita (C-4 da R4).
pub fn pode_falar_com_a_sefaz(conn: &Connection, id: &str, consulta: bool) -> Resultado<()> {
    let nada = if consulta { "a consulta à SEFAZ não foi feita" } else { "nada foi enviado à SEFAZ" };
    if !crate::fiscal_porta::ligado(conn)? {
        return Err(Erro::validacao(format!(
            "O fiscal está desligado neste computador: {nada}. Um gerente liga em Configurações › Fiscal."
        )));
    }
    // Cód-4 da R3: sem o CNPJ e a UF da loja, a resposta da SEFAZ não teria como ser conferida (K3): não sai.
    let cnpj = crate::documento::limpar(&crate::config::ler(conn, "loja_cnpj")?.unwrap_or_default());
    let uf = crate::config::ler(conn, "loja_uf")?.unwrap_or_default();
    // C-7 da R5: a loja pode ter informado um CPF (a tela da loja aceita); a nota pede o CNPJ.
    if cnpj.len() == 11 {
        return Err(Erro::validacao(format!(
            "A loja está com um CPF em Configurações › Loja, e a nota fiscal pede o CNPJ. Troque pelo CNPJ da loja em Configurações › Loja. {}.",
            primeira_maiuscula(nada)
        )));
    }
    if cnpj.len() != 14 || codigo_da_uf(&uf).is_none() {
        return Err(Erro::validacao(format!(
            "Falta informar o CNPJ e o estado (UF) da loja em Configurações › Loja: {nada}."
        )));
    }
    let da_nota: i64 = tx_ambiente(conn, id)?;
    let agora = ambiente(conn)?;
    if da_nota != agora {
        return Err(Erro::validacao(format!(
            "Esta nota é da {} e o computador está na {}: {nada}.",
            nome_do_ambiente(da_nota),
            nome_do_ambiente(agora)
        )));
    }
    Ok(())
}

fn primeira_maiuscula(t: &str) -> String {
    let mut c = t.chars();
    c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn tx_ambiente(conn: &Connection, id: &str) -> Resultado<i64> {
    conn.query_row("SELECT ambiente FROM nota_fiscal WHERE id = ?1", [id], |l| l.get(0))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Nota fiscal".into()))
}

// ------------------------------------------------------------- comandos das telas e a transmissão

pub use comandos::COMANDOS;

/// Os atendentes: abrem as transações (regra C5). A transmissão e a consulta ficam aqui porque gravam e confirmam
/// antes de chamar o emissor e gravam de novo depois (duas transações em volta da chamada).
pub(crate) mod comandos {
    use rusqlite::Connection;
    use serde_json::{json, Value};

    use crate::despacho::{ler, Classe, Comando, Id, Pedido};
    use crate::erro::Resultado;
    use crate::fiscal_porta::{self, Adaptador};

    pub const COMANDOS: &[Comando] = &[
        Comando { nome: "nota_painel", classe: Classe::Leitura, funcao: nota_painel },
        Comando { nome: "nota_detalhe", classe: Classe::Leitura, funcao: nota_detalhe },
        // FIS-P3: os impedimentos (só lê) e a troca (a função confere o gerente, a confirmação e a senha).
        Comando { nome: "fiscal_ambiente_impedimentos", classe: Classe::Leitura, funcao: fiscal_ambiente_impedimentos },
        Comando { nome: "fiscal_ambiente_trocar", classe: Classe::Caixa, funcao: fiscal_ambiente_trocar },
        // F2-2/C-M4: a nota que não chegou à SEFAZ (rascunho, rejeitada, não recebida) se descarta; o número volta.
        Comando { nome: "nota_descartar", classe: Classe::Gerente("configuracoes"), funcao: nota_descartar },
    ];

    fn nota_descartar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Id = ler(dados)?;
        let tx = p.conn.transaction()?;
        super::descartar(&tx, p.usuario_id, &d.id)?;
        tx.commit()?;
        Ok(json!(super::detalhe(p.conn, &d.id)?))
    }

    fn fiscal_ambiente_impedimentos(p: &mut Pedido, _dados: Value) -> Resultado<Value> {
        Ok(json!({ "ambiente": super::ambiente(p.conn)?, "impedimentos": super::impedimentos_da_troca(p.conn)? }))
    }

    fn fiscal_ambiente_trocar(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Troca = ler(dados)?;
        super::conferir_antes_da_senha(p.conn, p.gerente, d.para, &d.confirmacao)?;
        let liberou = if d.para == 1 && super::ambiente(p.conn)? != 1 {
            Some(fiscal_porta::conferir_senha_do_gerente(p.conn, p.disp, p.usuario_id, &d.senha, "ir para a produção")?)
        } else {
            None
        };
        let tx = p.conn.transaction()?;
        let novo =
            super::trocar_ambiente(&tx, p.disp, p.usuario_id, p.gerente, d.para, &d.confirmacao, liberou.as_deref())?;
        tx.commit()?;
        Ok(json!({ "ambiente": novo }))
    }

    #[derive(serde::Deserialize, Default)]
    #[serde(default)]
    struct Troca {
        para: i64,
        confirmacao: String,
        senha: String,
    }

    fn nota_painel(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let f: super::Filtros = ler(dados)?;
        Ok(json!({
            "painel": super::painel(p.conn, &f)?,
            "liga": fiscal_porta::estado(p.conn, p.gerente)?,
        }))
    }

    fn nota_detalhe(p: &mut Pedido, dados: Value) -> Resultado<Value> {
        let d: Id = ler(dados)?;
        Ok(json!(super::detalhe(p.conn, &d.id)?))
    }

    /// Transmite: grava "enviada" e confirma; chama o emissor pela porta; grava o resultado. Sem resposta
    /// conclusiva, fica "desconhecida". Devolve o estado novo.
    // Sem emissor de verdade nesta etapa (o adaptador ACBr é a T008): por enquanto só os testes chamam.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn transmitir(
        conn: &mut Connection,
        usuario_id: &str,
        adaptador: &mut dyn Adaptador,
        id: &str,
    ) -> Resultado<String> {
        let nota = {
            let tx = conn.transaction()?;
            super::pode_falar_com_a_sefaz(&tx, id, false)?;
            super::marcar_enviada(&tx, usuario_id, id)?;
            let n = super::para_enviar(&tx, id)?;
            tx.commit()?;
            n
        };
        let resposta = fiscal_porta::com_adaptador(conn, adaptador, |a, l| a.enviar(l, &nota));
        let tx = conn.transaction()?;
        let novo = match &resposta {
            Ok(r) => super::registrar_resposta(&tx, usuario_id, id, super::Origem::Envio, Ok(r))?,
            Err(e) => super::registrar_resposta(&tx, usuario_id, id, super::Origem::Envio, Err(&e.to_string()))?,
        };
        tx.commit()?;
        Ok(novo)
    }

    /// Consulta a nota enviada ou desconhecida na SEFAZ e grava o que vier.
    // Sem emissor de verdade nesta etapa (o adaptador ACBr é a T008): por enquanto só os testes chamam.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn consultar(
        conn: &mut Connection,
        usuario_id: &str,
        adaptador: &mut dyn Adaptador,
        id: &str,
    ) -> Resultado<String> {
        let nota = super::para_enviar(conn, id)?;
        let e = super::estado(conn, id)?;
        if !matches!(e.as_str(), "enviada" | "desconhecida") {
            return Err(crate::erro::Erro::validacao("Só a nota sem resposta conclusiva precisa de consulta."));
        }
        super::pode_falar_com_a_sefaz(conn, id, true)?;
        let resposta = fiscal_porta::com_adaptador(conn, adaptador, |a, l| a.consultar(l, &nota));
        let tx = conn.transaction()?;
        let novo = match &resposta {
            Ok(r) => super::registrar_resposta(&tx, usuario_id, id, super::Origem::Consulta, Ok(r))?,
            Err(e) => super::registrar_resposta(&tx, usuario_id, id, super::Origem::Consulta, Err(&e.to_string()))?,
        };
        tx.commit()?;
        Ok(novo)
    }
}

#[cfg(test)]
mod testes {
    use super::comandos::{consultar, transmitir};
    use super::*;
    use crate::banco::Banco;
    use crate::fiscal_porta::{Adaptador, Licenca};

    /// A chave de acesso de uma nota de teste (SP, 11/2026, CNPJ fixo, emissão normal), com o dígito verificador.
    fn chave(modelo: i64, serie: i64, numero: i64, codigo: u32) -> String {
        let corpo = format!("35261111222333000181{modelo:02}{serie:03}{numero:09}1{codigo:08}");
        format!("{corpo}{}", digito_da_chave(corpo.as_bytes()))
    }

    /// Qual chave a resposta falsa traz.
    #[derive(Clone, Copy)]
    enum Chave {
        Desta,
        OutroCnpj,
        OutraUf,
        DestaOutroCodigo,
        Vazia,
        DeOutroNumero,
        DigitoErrado,
    }

    /// Uma fala do emissor falso: `None` é "sem resposta" (a conexão caiu).
    type Fala = Option<(&'static str, Chave, bool)>;

    fn ok(cstat: &'static str) -> Fala {
        Some((cstat, Chave::Desta, true))
    }

    /// O emissor falso: responde na ordem da fila, montando a chave pela nota que recebeu.
    struct Falso {
        falas: Vec<Fala>,
        enviadas: Vec<NotaParaEnviar>,
        consultas: u32,
    }

    impl Falso {
        fn com(falas: Vec<Fala>) -> Falso {
            Falso { falas, enviadas: Vec::new(), consultas: 0 }
        }
        fn proxima(&mut self, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            let Some((cstat, qual, com_protocolo)) = self.falas.remove(0) else {
                return Err(Erro::Interno("a conexão com o emissor caiu".into()));
            };
            let c = match qual {
                Chave::Desta => chave(nota.modelo, nota.serie, nota.numero, 1),
                Chave::OutroCnpj | Chave::OutraUf => {
                    let mut corpo = format!(
                        "{}2611{}{:02}{:03}{:09}1{:08}",
                        if matches!(qual, Chave::OutraUf) { "41" } else { "35" },
                        if matches!(qual, Chave::OutroCnpj) { "99888777000166" } else { "11222333000181" },
                        nota.modelo,
                        nota.serie,
                        nota.numero,
                        1
                    );
                    corpo.push((b'0' + digito_da_chave(corpo.as_bytes())) as char);
                    corpo
                }
                Chave::DestaOutroCodigo => chave(nota.modelo, nota.serie, nota.numero, 2),
                Chave::Vazia => String::new(),
                Chave::DeOutroNumero => chave(nota.modelo, nota.serie, nota.numero + 1, 1),
                Chave::DigitoErrado => {
                    let mut c = chave(nota.modelo, nota.serie, nota.numero, 1);
                    let d = (c.pop().unwrap() as u8 - b'0' + 1) % 10;
                    c.push((b'0' + d) as char);
                    c
                }
            };
            Ok(RespostaSefaz {
                chave: c,
                cstat: cstat.into(),
                motivo: format!("Motivo {cstat}"),
                protocolo: if com_protocolo && matches!(cstat, "100" | "120" | "150" | "110" | "101") {
                    "135260000000001".into()
                } else {
                    String::new()
                },
                xml_nome: String::new(),
            })
        }
    }

    impl Adaptador for Falso {
        fn status(&mut self, _l: &Licenca) -> Resultado<String> {
            Ok("OK".into())
        }
        fn enviar(&mut self, _l: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            self.enviadas.push(nota.clone());
            self.proxima(nota)
        }
        fn consultar(&mut self, _l: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
            self.consultas += 1;
            self.proxima(nota)
        }
    }

    /// Um banco com o fiscal ligado e um gerente.
    fn banco() -> (Banco, String) {
        let b = Banco::em_memoria().unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        crate::usuarios::definir_gerente(&b.conn, &u, true).unwrap();
        crate::fiscal_porta::ligar(&b.conn, &b.dispositivo, &u, true, "senha-da-ana-1").unwrap();
        loja_de_sp(&b);
        (b, u)
    }

    /// O CNPJ e a UF da loja de teste (os da chave das respostas falsas).
    fn loja_de_sp(b: &Banco) {
        crate::config::gravar(&b.conn, "loja_cnpj", "11.222.333/0001-81").unwrap();
        crate::config::gravar(&b.conn, "loja_uf", "SP").unwrap();
    }

    fn nova(b: &Banco, u: &str) -> String {
        criar_rascunho(&b.conn, &b.dispositivo, u, 2, 65, 1, None, 15000, "", "").unwrap()
    }

    fn numero(b: &Banco, id: &str) -> i64 {
        para_enviar(&b.conn, id).unwrap().numero
    }

    fn situacao(b: &Banco, id: &str) -> String {
        estado(&b.conn, id).unwrap()
    }

    /// C-A1 da R1: a lista fechada do MOC 7.0, Anexo I (tabelas 4.4.1 e 4.4.2). Só conclui o que é conclusivo; o
    /// resto é "sem resposta conclusiva" e não libera o reenvio.
    #[test]
    fn r1_ca1_lista_fechada_dos_codigos() {
        use Origem::{Consulta, Envio};
        for (o, c, e) in [
            (Envio, "100", "autorizada"),
            (Envio, "150", "autorizada"),
            // N-1 da R4: 120, "Autorizado o uso da NF-e, com alerta" (NT 2026.002).
            (Envio, "120", "autorizada"),
            (Consulta, "120", "autorizada"),
            (Envio, "110", "denegada"),
            (Envio, "301", "denegada"),
            (Envio, "302", "denegada"),
            (Envio, "303", "denegada"),
            (Envio, "225", "rejeitada"),
            (Envio, "142", "rejeitada"),
            (Envio, "383", "rejeitada"),
            // K1 da R2: as rejeições de 4 dígitos das NT em vigor e a 129 da NT 2023.002.
            (Envio, "1153", "rejeitada"),
            (Envio, "1275", "rejeitada"),
            (Envio, "1002", "rejeitada"),
            (Envio, "129", "rejeitada"),
            // F3-1 da R3: 126 (NT 2023.002) e 156 a 188 (NT 2026.007, validação cadastral).
            (Envio, "126", "rejeitada"),
            (Envio, "156", "rejeitada"),
            (Envio, "164", "rejeitada"),
            (Envio, "188", "rejeitada"),
            (Consulta, "100", "autorizada"),
            (Consulta, "110", "denegada"),
            (Consulta, "101", "cancelada"),
            (Consulta, "151", "cancelada"),
            (Consulta, "155", "cancelada"),
            (Consulta, "217", "nao_recebida"),
        ] {
            assert_eq!(estado_da_resposta(o, c), e, "{o:?} {c}");
        }
        for c in [
            "103", "104", "105", "106", "107", "108", "109", "124", "135", "204", "205", "206", "218", "539", "613",
            "635", "656", "691", "692", "999", "0206", "0635", "0100", "0150", "0110", "000", "099", "10000", "12a",
            "abc", "", " ",
        ] {
            assert_eq!(estado_da_resposta(Envio, c), "desconhecida", "envio {c:?}");
        }
        // Na consulta, o código de rejeição é do pedido de consulta, e não da nota; e o cancelamento só vem da consulta.
        for c in ["225", "236", "105", "108", "999", "1153", "129"] {
            assert_eq!(estado_da_resposta(Consulta, c), "desconhecida", "consulta {c}");
        }
        for c in ["101", "151", "155", "217"] {
            assert_eq!(estado_da_resposta(Envio, c), "desconhecida", "envio {c}");
        }
    }

    /// C-A1: a resposta não conclusiva (lote em processamento, serviço parado, duplicidade) fica "desconhecida", não
    /// se transmite de novo e não se descarta; a consulta resolve.
    #[test]
    fn r1_ca1_nao_conclusiva_nao_libera_reenvio() {
        for cstat in ["103", "105", "108", "109", "204", "539", "999"] {
            let (mut b, u) = banco();
            let id = nova(&b, &u);
            let mut falso = Falso::com(vec![ok(cstat)]);
            assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida", "{cstat}");
            let mut outro = Falso::com(vec![ok("100")]);
            assert!(transmitir(&mut b.conn, &u, &mut outro, &id).is_err(), "{cstat}: não reenvia");
            assert!(outro.enviadas.is_empty());
            assert!(descartar(&b.conn, &u, &id).is_err(), "{cstat}: não se descarta");
            assert_eq!(consultar(&mut b.conn, &u, &mut outro, &id).unwrap(), "autorizada", "{cstat}");
        }
    }

    #[test]
    fn pacote_e_autorizada_rejeitada_e_reenvio_da_rejeitada() {
        let (mut b, u) = banco();
        let mut falso = Falso::com(vec![ok("100"), Some(("225", Chave::Desta, false))]);
        let a = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &a).unwrap(), "autorizada");
        let r = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &r).unwrap(), "rejeitada");
        let d = detalhe(&b.conn, &r).unwrap();
        assert_eq!((d.cstat.as_str(), d.motivo.as_str()), ("225", "Motivo 225"), "na íntegra");
        assert!(d.explicacao.contains("recusou"));
        assert!(!d.pede_consulta && d.pode_transmitir && d.pode_descartar);
        assert_eq!(d.chave, "", "a rejeitada não guarda chave");
        // C-M1: a correção vem com outro código numérico (outra chave da mesma nota) e é autorizada, com o mesmo número.
        let mut falso2 = Falso::com(vec![Some(("100", Chave::DestaOutroCodigo, true))]);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso2, &r).unwrap(), "autorizada");
        assert_eq!(falso2.enviadas[0].numero, 2);
        assert_eq!(detalhe(&b.conn, &r).unwrap().chave, chave(65, 1, 2, 2));
    }

    /// C-M1 da R1: a resposta só vale para a nota que espera por ela, com a chave desta nota, e a autorização pede o
    /// protocolo.
    #[test]
    fn r1_cm1_resposta_tem_de_ser_desta_nota() {
        for (qual, protocolo, o_que) in [
            (Chave::Vazia, true, "sem chave"),
            (Chave::DeOutroNumero, true, "chave de outro número"),
            (Chave::DigitoErrado, true, "dígito verificador errado"),
            (Chave::Desta, false, "sem protocolo"),
        ] {
            let (mut b, u) = banco();
            let id = nova(&b, &u);
            let mut falso = Falso::com(vec![Some(("100", qual, protocolo))]);
            assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida", "{o_que}");
            assert_eq!(detalhe(&b.conn, &id).unwrap().chave, "", "{o_que}: nada gravado como chave");
        }
        // A autorizada não volta atrás com uma resposta tardia (nem de envio nem de consulta).
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok("100")]), &id).unwrap();
        let tardia = RespostaSefaz { cstat: "225".into(), ..Default::default() };
        assert!(registrar_resposta(&b.conn, &u, &id, Origem::Envio, Ok(&tardia)).is_err());
        assert!(registrar_resposta(&b.conn, &u, &id, Origem::Consulta, Ok(&tardia)).is_err());
        assert!(registrar_resposta(&b.conn, &u, &id, Origem::Envio, Err("caiu")).is_err());
        assert_eq!(situacao(&b, &id), "autorizada");
        // O rascunho não recebe resposta.
        let r = nova(&b, &u);
        assert!(registrar_resposta(&b.conn, &u, &r, Origem::Envio, Ok(&tardia)).is_err());
        assert_eq!(situacao(&b, &r), "rascunho");
        let _ = &mut b;
    }

    /// K2 da R2: 206 (já inutilizada), 218 (já cancelada) e 635 (o mesmo número já transmitido, aguardando) dizem que o
    /// número já existe na SEFAZ: não é rejeição que se descarta (o número não pode voltar); fica sem resposta, e só a
    /// consulta decide.
    #[test]
    fn r2_k2_numero_que_ja_existe_na_sefaz_nao_volta() {
        for cstat in ["206", "218", "635"] {
            let (mut b, u) = banco();
            let id = nova(&b, &u);
            assert_eq!(transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok(cstat)]), &id).unwrap(), "desconhecida");
            assert!(descartar(&b.conn, &u, &id).is_err(), "{cstat}: não se descarta");
            let livres = crate::config::ler(&b.conn, &chave_dos_livres(2, 65, 1)).unwrap().unwrap_or_default();
            assert!(livres.is_empty(), "{cstat}: o número não voltou");
            let outra = nova(&b, &u);
            transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok("100")]), &outra).unwrap();
            assert_eq!(numero(&b, &outra), 2, "{cstat}: a próxima não repete o número");
        }
    }

    /// K3 da R2: a resposta com o mesmo modelo, série e número, mas de outro emitente (outro CNPJ ou outra UF), não é
    /// desta nota; sem o CNPJ e a UF da loja, não há como conferir: fica sem resposta.
    #[test]
    fn r2_k3_resposta_de_outro_emitente_nao_vale() {
        for (qual, o_que) in [(Chave::OutroCnpj, "outro CNPJ"), (Chave::OutraUf, "outra UF")] {
            let (mut b, u) = banco();
            let id = nova(&b, &u);
            let mut falso = Falso::com(vec![None, Some(("100", qual, true))]);
            transmitir(&mut b.conn, &u, &mut falso, &id).unwrap();
            assert_eq!(consultar(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida", "{o_que}");
            assert_eq!(detalhe(&b.conn, &id).unwrap().chave, "", "{o_que}");
        }
        // Cód-4 da R3: sem o CNPJ (ou a UF) da loja, a nota nem sai, e a mensagem diz onde informar.
        for (chave, o_que) in [("loja_cnpj", "sem CNPJ"), ("loja_uf", "sem UF")] {
            let (mut b, u) = banco();
            crate::config::gravar(&b.conn, chave, "").unwrap();
            let id = nova(&b, &u);
            let mut falso = Falso::com(vec![ok("100")]);
            let e = transmitir(&mut b.conn, &u, &mut falso, &id).unwrap_err().to_string();
            assert!(e.contains("Configurações › Loja") && e.contains("nada foi enviado"), "{o_que}: {e}");
            assert_eq!((situacao(&b, &id), falso.enviadas.len()), ("rascunho".to_string(), 0), "{o_que}");
        }
        // C-7 da R5: a loja com CPF recebe a mensagem certa.
        let (mut b, u) = banco();
        crate::config::gravar(&b.conn, "loja_cnpj", "123.456.789-09").unwrap();
        let id = nova(&b, &u);
        let e = transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok("100")]), &id).unwrap_err().to_string();
        assert!(
            e.contains("está com um CPF") && e.contains("Troque pelo CNPJ da loja") && e.contains("Nada foi enviado"),
            "{e}"
        );
        // Se o CNPJ some depois do envio, a resposta ainda não é aceita sem conferir (K3).
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![None]), &id).unwrap();
        crate::config::gravar(&b.conn, "loja_cnpj", "").unwrap();
        let r = RespostaSefaz {
            cstat: "100".into(),
            chave: chave(65, 1, 1, 1),
            protocolo: "1".into(),
            ..Default::default()
        };
        assert_eq!(registrar_resposta(&b.conn, &u, &id, Origem::Consulta, Ok(&r)).unwrap(), "desconhecida");
    }

    /// N-1 da R4: o 120 (autorizada com alerta) é autorizada, com a chave guardada, e não se descarta; e uma resposta com
    /// protocolo nunca vira "rejeitada" (a rejeição não gera protocolo): um código desconhecido de sucesso fica sem
    /// resposta, e o número não volta.
    #[test]
    fn r4_n1_autorizada_com_alerta_e_protocolo_nunca_e_rejeicao() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok("120")]), &id).unwrap(), "autorizada");
        assert_eq!(detalhe(&b.conn, &id).unwrap().chave, chave(65, 1, 1, 1));
        assert!(descartar(&b.conn, &u, &id).is_err());
        let outra = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![None]), &outra).unwrap();
        b.conn.execute("UPDATE nota_fiscal SET estado = 'enviada' WHERE id = ?1", [&outra]).unwrap();
        let r = RespostaSefaz {
            cstat: "225".into(),
            chave: chave(65, 1, 2, 1),
            protocolo: "135260000000077".into(),
            motivo: "x".into(),
            xml_nome: String::new(),
        };
        assert_eq!(registrar_resposta(&b.conn, &u, &outra, Origem::Envio, Ok(&r)).unwrap(), "desconhecida");
        assert!(descartar(&b.conn, &u, &outra).is_err(), "o número não volta");
        // C-6 da R5: a resposta que não é conclusiva não deixa o protocolo dela nesta nota.
        assert_eq!(detalhe(&b.conn, &outra).unwrap().protocolo, "");
    }

    /// F3-3 da R3: a loja com CNPJ alfanumérico. A chave com letras nas posições do CNPJ confere (o dígito verificador
    /// pelo código ASCII menos 48), e a autorização vale; letra fora das posições do CNPJ não.
    #[test]
    fn r3_f33_cnpj_alfanumerico() {
        let (mut b, u) = banco();
        crate::config::gravar(&b.conn, "loja_cnpj", "12.ABC.345/01DE-35").unwrap();
        let id = nova(&b, &u);
        struct Alfa;
        impl Adaptador for Alfa {
            fn status(&mut self, _l: &Licenca) -> Resultado<String> {
                Ok("OK".into())
            }
            fn enviar(&mut self, _l: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
                let mut corpo =
                    format!("352611{}{:02}{:03}{:09}100000001", "12ABC34501DE35", nota.modelo, nota.serie, nota.numero);
                corpo.push((b'0' + digito_da_chave(corpo.as_bytes())) as char);
                Ok(RespostaSefaz {
                    chave: corpo,
                    cstat: "100".into(),
                    motivo: "Autorizado".into(),
                    protocolo: "135260000000009".into(),
                    xml_nome: String::new(),
                })
            }
            fn consultar(&mut self, _l: &Licenca, _nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
                unreachable!()
            }
        }
        assert_eq!(transmitir(&mut b.conn, &u, &mut Alfa, &id).unwrap(), "autorizada");
        let c = detalhe(&b.conn, &id).unwrap().chave;
        assert!(chave_e_desta_nota(&c, 65, 1, 1));
        let mut errada = c.clone();
        errada.replace_range(21..22, "A");
        assert!(!chave_e_desta_nota(&errada, 65, 1, 1), "letra fora do CNPJ");
        // A pesquisa pela chave alfanumérica acha a nota.
        assert_eq!(painel(&b.conn, &Filtros { texto: c.clone(), ..Default::default() }).unwrap().notas.len(), 1);
    }

    /// FIS-P2: sem resposta não é rejeição, e não se reenvia; só a consulta resolve. A consulta que diz "não consta"
    /// (217) libera o reenvio.
    #[test]
    fn pacote_e_sem_resposta_fica_desconhecida_e_a_consulta_resolve() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        let mut falso = Falso::com(vec![None]);
        assert_eq!(transmitir(&mut b.conn, &u, &mut falso, &id).unwrap(), "desconhecida");
        let d = detalhe(&b.conn, &id).unwrap();
        assert!(d.pede_consulta && !d.pode_transmitir && !d.pode_descartar);
        assert!(d.explicacao.contains("Não transmita de novo"));
        let mut outro = Falso::com(vec![Some(("217", Chave::Vazia, false)), ok("100")]);
        assert_eq!(consultar(&mut b.conn, &u, &mut outro, &id).unwrap(), "nao_recebida");
        assert!(detalhe(&b.conn, &id).unwrap().pode_transmitir);
        assert_eq!(transmitir(&mut b.conn, &u, &mut outro, &id).unwrap(), "autorizada");
        assert_eq!(outro.enviadas[0].numero, 1, "o mesmo número");
    }

    /// "Enviada" é gravada antes de chamar o emissor: se o programa cair durante a chamada, a nota não volta atrás (e
    /// não se transmite de novo às cegas). O número já está nela.
    #[test]
    fn pacote_e_enviada_e_gravada_antes_da_chamada() {
        struct Espiao {
            visto: Option<(String, i64)>,
            caminho: std::path::PathBuf,
        }
        impl Adaptador for Espiao {
            fn status(&mut self, _l: &Licenca) -> Resultado<String> {
                Ok("OK".into())
            }
            fn enviar(&mut self, _l: &Licenca, nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
                // Outra conexão, como outro processo: o estado já tem de estar confirmado no disco.
                let c = Connection::open(&self.caminho).unwrap();
                self.visto = c
                    .query_row("SELECT estado, numero FROM nota_fiscal WHERE id = ?1", [&nota.id], |l| {
                        Ok((l.get(0)?, l.get(1)?))
                    })
                    .ok();
                Err(Erro::Interno("caiu".into()))
            }
            fn consultar(&mut self, _l: &Licenca, _nota: &NotaParaEnviar) -> Resultado<RespostaSefaz> {
                unreachable!()
            }
        }
        let pasta = std::env::temp_dir().join(format!("rustinho-nota-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("rustinho.db");
        let mut b = Banco::abrir(&caminho).unwrap();
        let u = crate::usuarios::criar(&b.conn, &b.dispositivo, "Ana", "senha-da-ana-1", true).unwrap().id;
        crate::usuarios::definir_gerente(&b.conn, &u, true).unwrap();
        crate::fiscal_porta::ligar(&b.conn, &b.dispositivo, &u, true, "senha-da-ana-1").unwrap();
        loja_de_sp(&b);
        let id = nova(&b, &u);
        let mut espiao = Espiao { visto: None, caminho: caminho.clone() };
        assert_eq!(transmitir(&mut b.conn, &u, &mut espiao, &id).unwrap(), "desconhecida");
        assert_eq!(espiao.visto, Some(("enviada".to_string(), 1)));
        drop(b);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// F2-2 + C-M4 da R1: o número sai na hora de transmitir (o rascunho não tem), é por ambiente, modelo e série, nunca
    /// se repete, respeita o piso (a última nota do sistema de antes) e não volta com um backup antigo (o contador é
    /// deste computador). O número da rejeitada descartada volta; o da denegada, não.
    #[test]
    fn r1_cm4_numeracao_na_hora_de_transmitir() {
        let (mut b, u) = banco();
        let a = nova(&b, &u);
        assert_eq!(numero(&b, &a), 0, "o rascunho não tem número");
        let mut falso = Falso::com(vec![ok("100"); 8]);
        transmitir(&mut b.conn, &u, &mut falso, &a).unwrap();
        assert_eq!(numero(&b, &a), 1);
        // Outro modelo, outro ambiente: outra numeração.
        let nfe = criar_rascunho(&b.conn, &b.dispositivo, &u, 2, 55, 1, None, 0, "", "").unwrap();
        transmitir(&mut b.conn, &u, &mut falso, &nfe).unwrap();
        assert_eq!(numero(&b, &nfe), 1, "outro modelo");
        // O piso: o sistema de antes já emitiu até a 500 nesta série.
        crate::config::gravar(&b.conn, &chave_do_piso(2, 65, 1), "500").unwrap();
        let p = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut falso, &p).unwrap();
        assert_eq!(numero(&b, &p), 501);
        // Um backup antigo, sem as notas: o contador (local, preservado) não deixa o número voltar.
        b.conn.execute("DELETE FROM nota_fiscal_evento", []).unwrap();
        b.conn.execute("DELETE FROM nota_fiscal", []).unwrap();
        crate::config::gravar(&b.conn, &chave_do_piso(2, 65, 1), "").unwrap();
        let depois = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut falso, &depois).unwrap();
        assert_eq!(numero(&b, &depois), 502);
        // A rejeitada descartada devolve o número; a próxima nota o usa.
        let mut rej = Falso::com(vec![Some(("225", Chave::Vazia, false)), Some(("110", Chave::Desta, true))]);
        let r = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut rej, &r).unwrap(), "rejeitada");
        assert_eq!(numero(&b, &r), 503);
        descartar(&b.conn, &u, &r).unwrap();
        assert_eq!((situacao(&b, &r), numero(&b, &r)), ("descartada".to_string(), 0));
        let d = nova(&b, &u);
        assert_eq!(transmitir(&mut b.conn, &u, &mut rej, &d).unwrap(), "denegada");
        assert_eq!(numero(&b, &d), 503, "o número devolvido");
        assert!(descartar(&b.conn, &u, &d).is_err(), "a denegada não se descarta (o número foi usado)");
        let e = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut falso, &e).unwrap();
        assert_eq!(numero(&b, &e), 504);
        // O banco recusa o número repetido.
        assert!(b.conn.execute("UPDATE nota_fiscal SET numero = 504 WHERE id = ?1", [&d]).is_err());
        // O rascunho descartado não mexe na numeração.
        let rasc = nova(&b, &u);
        descartar(&b.conn, &u, &rasc).unwrap();
        let f = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut falso, &f).unwrap();
        assert_eq!(numero(&b, &f), 505);
    }

    /// C-M4 da R1: dois computadores não emitem na mesma série. A primeira nota reserva a série para este computador
    /// (a reserva viaja para os outros caixas); a série que já é de outro computador não sai daqui, e a descoberta não a
    /// confirma.
    #[test]
    fn r1_cm4_uma_serie_e_de_um_computador_so() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        let log = |b: &Banco| -> i64 { b.conn.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap() };
        let antes = log(&b);
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok("100")]), &id).unwrap();
        assert_eq!(estacao_da_serie(&b.conn, 65, 1).unwrap().map(|e| e.0), Some(b.dispositivo.clone()));
        assert!(crate::sync::chave_da_loja(&chave_da_estacao(65, 1)), "a reserva vai para os outros caixas");
        assert!(log(&b) > antes, "e entra no registro da troca");
        // A série 2 já é de outro computador (chegou pela troca).
        crate::config::gravar(
            &b.conn,
            &chave_da_estacao(65, 2),
            &serde_json::json!({ "dispositivo": "outro-caixa", "nome": "Caixa 2" }).to_string(),
        )
        .unwrap();
        let outra = criar_rascunho(&b.conn, &b.dispositivo, &u, 2, 65, 2, None, 100, "", "").unwrap();
        let mut falso = Falso::com(vec![ok("100")]);
        let e = transmitir(&mut b.conn, &u, &mut falso, &outra).unwrap_err().to_string();
        assert!(e.contains("Caixa 2") && e.contains("série 2"), "{e}");
        assert_eq!((situacao(&b, &outra), numero(&b, &outra)), ("rascunho".to_string(), 0));
        assert!(falso.enviadas.is_empty());
        assert!(crate::fiscal_descoberta::usar(&b.conn, &b.dispositivo, &u, "serieNfce", "2", None).is_err());
        assert!(crate::config::ler(&b.conn, "fiscal_serie_nfce").unwrap().is_none());
        crate::fiscal_descoberta::usar(&b.conn, &b.dispositivo, &u, "serieNfce", "1", None).unwrap();
    }

    /// C-M2 da R1: com o fiscal desligado, nada sai e nada muda (a nota não vira "sem resposta" sem ter saído).
    #[test]
    fn r1_cm2_fiscal_desligado_nao_muda_a_nota() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        let outra = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![None]), &outra).unwrap();
        crate::fiscal_porta::desligar(&b.conn, &b.dispositivo, &u, true).unwrap();
        let mut falso = Falso::com(vec![ok("100"), ok("100")]);
        let e = transmitir(&mut b.conn, &u, &mut falso, &id).unwrap_err().to_string();
        assert!(e.contains("desligado") && e.contains("nada foi enviado"), "{e}");
        assert_eq!((situacao(&b, &id), numero(&b, &id)), ("rascunho".to_string(), 0));
        // C-4 da R5: na consulta, a mensagem diz que a consulta não foi feita (a nota já tinha saído).
        let e = consultar(&mut b.conn, &u, &mut falso, &outra).unwrap_err().to_string();
        assert!(e.contains("a consulta à SEFAZ não foi feita") && !e.contains("nada foi enviado"), "{e}");
        assert_eq!(situacao(&b, &outra), "desconhecida");
        assert!(falso.enviadas.is_empty() && falso.consultas == 0);
    }

    /// C-M3 da R1: a nota só fala com a SEFAZ no ambiente em que foi criada.
    #[test]
    fn r1_cm3_ambiente_da_nota_e_o_do_computador() {
        let (mut b, u) = banco();
        let id = nova(&b, &u);
        let solta = nova(&b, &u);
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![None]), &solta).unwrap();
        crate::config::gravar(&b.conn, "fiscal_ambiente", "1").unwrap();
        let mut falso = Falso::com(vec![ok("100"), ok("100")]);
        let e = transmitir(&mut b.conn, &u, &mut falso, &id).unwrap_err().to_string();
        assert!(e.contains("homologação") && e.contains("produção"), "{e}");
        assert!(consultar(&mut b.conn, &u, &mut falso, &solta).is_err());
        assert_eq!((situacao(&b, &id), situacao(&b, &solta)), ("rascunho".to_string(), "desconhecida".to_string()));
        assert!(falso.enviadas.is_empty() && falso.consultas == 0);
    }

    /// FIS-P3: cada impedimento sozinho bloqueia; sem impedimento, ir para a produção pede PRODUÇÃO e a senha conferida;
    /// o registro diz quem e de qual para qual. B7 da R1: voltar para a homologação só depende das notas no meio.
    #[test]
    fn pacote_f_troca_de_ambiente() {
        let (mut b, u) = banco();
        let liberar = |b: &Banco| {
            crate::config::gravar(&b.conn, "fiscal_certificado_testado", "1").unwrap();
            crate::config::gravar(&b.conn, "fiscal_serie_nfce", "1").unwrap();
            crate::config::gravar(&b.conn, "fiscal_serie_nfe", "1").unwrap();
            crate::config::gravar(&b.conn, "fiscal_credenciado", "1").unwrap();
        };
        assert_eq!(
            impedimentos_da_troca(&b.conn).unwrap().len(),
            4,
            "de fábrica: certificado, 2 séries, credenciamento"
        );
        liberar(&b);
        assert!(impedimentos_da_troca(&b.conn).unwrap().is_empty());
        for (chave, valor) in [
            ("fiscal_certificado_testado", "0"),
            ("fiscal_serie_nfce", ""),
            ("fiscal_serie_nfe", ""),
            ("fiscal_credenciado", "0"),
        ] {
            crate::config::gravar(&b.conn, chave, valor).unwrap();
            assert_eq!(impedimentos_da_troca(&b.conn).unwrap().len(), 1, "{chave}");
            assert!(
                trocar_ambiente(&b.conn, &b.dispositivo, &u, true, 1, "PRODUÇÃO", Some("Ana")).is_err(),
                "{chave}: a senha conferida não contorna"
            );
            liberar(&b);
        }
        crate::fiscal_porta::desligar(&b.conn, &b.dispositivo, &u, true).unwrap();
        assert_eq!(impedimentos_da_troca(&b.conn).unwrap().len(), 1, "o fiscal desligado");
        crate::fiscal_porta::ligar(&b.conn, &b.dispositivo, &u, true, "senha-da-ana-1").unwrap();
        // Uma nota sem resposta conclusiva no ambiente atual bloqueia; o rascunho, não.
        let id = nova(&b, &u);
        assert!(impedimentos_da_troca(&b.conn).unwrap().is_empty(), "o rascunho não bloqueia");
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![None]), &id).unwrap();
        assert_eq!(impedimentos_da_troca(&b.conn).unwrap().len(), 1, "a sem resposta");
        consultar(&mut b.conn, &u, &mut Falso::com(vec![ok("100")]), &id).unwrap();
        assert!(trocar_ambiente(&b.conn, &b.dispositivo, &u, false, 1, "PRODUÇÃO", Some("Ana")).is_err());
        assert!(trocar_ambiente(&b.conn, &b.dispositivo, &u, true, 1, "producao", Some("Ana")).is_err());
        assert!(trocar_ambiente(&b.conn, &b.dispositivo, &u, true, 1, "PRODUÇÃO", None).is_err(), "sem a senha");
        assert_eq!(ambiente(&b.conn).unwrap(), 2);
        assert_eq!(trocar_ambiente(&b.conn, &b.dispositivo, &u, true, 1, "PRODUÇÃO", Some("Ana")).unwrap(), 1);
        let registro: String = b
            .conn
            .query_row("SELECT detalhes FROM auditoria WHERE acao = 'fiscal.ambiente'", [], |l| l.get(0))
            .unwrap();
        assert!(registro.contains("Ana") && registro.contains("de homologação para produção"), "{registro}");
        // A numeração é separada por ambiente: a primeira de produção é a 1.
        let p = criar_rascunho(&b.conn, &b.dispositivo, &u, 1, 65, 1, None, 0, "", "").unwrap();
        transmitir(&mut b.conn, &u, &mut Falso::com(vec![ok("100")]), &p).unwrap();
        assert_eq!(numero(&b, &p), 1);
        // B7: na produção, sem certificado testado nem credenciamento, a volta para a homologação é livre.
        crate::config::gravar(&b.conn, "fiscal_certificado_testado", "0").unwrap();
        crate::config::gravar(&b.conn, "fiscal_credenciado", "0").unwrap();
        assert!(impedimentos_da_troca(&b.conn).unwrap().is_empty());
        assert_eq!(trocar_ambiente(&b.conn, &b.dispositivo, &u, true, 2, "", None).unwrap(), 2);
    }

    /// B1 da R1: a senha da troca para a produção tem a espera depois de 5 erros e o registro de cada erro (a mesma
    /// conferência do FIS-LIGA).
    #[test]
    fn r1_b1_senha_da_producao_com_espera_e_registro() {
        let (b, u) = banco();
        for _ in 0..5 {
            let e = crate::fiscal_porta::conferir_senha_do_gerente(
                &b.conn,
                &b.dispositivo,
                &u,
                "errada",
                "ir para a produção",
            )
            .unwrap_err()
            .to_string();
            assert!(e.contains("errada"), "{e}");
        }
        let e = crate::fiscal_porta::conferir_senha_do_gerente(
            &b.conn,
            &b.dispositivo,
            &u,
            "senha-da-ana-1",
            "ir para a produção",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("Espere"), "a certa também espera: {e}");
        let n: i64 = b
            .conn
            .query_row(
                "SELECT COUNT(*) FROM auditoria WHERE acao = 'gerente.senha_errada' AND detalhes LIKE '%ir para a produção%'",
                [],
                |l| l.get(0),
            )
            .unwrap();
        assert_eq!(n, 5);
    }

    /// FIS-P1 (texto A §12, C-M9 da R1): os quatro indicadores (Autorizadas, Pendentes, Rejeitadas e Canceladas) e o
    /// aviso das sem resposta; a pesquisa e o período filtram; a descartada some; B8: a pesquisa não acha pelo
    /// documento do destinatário (fica escondido).
    #[test]
    fn pacote_e_painel_filtra() {
        let (mut b, u) = banco();
        let mut falso = Falso::com(vec![ok("100"), Some(("225", Chave::Vazia, false)), None]);
        for _ in 0..3 {
            let id = nova(&b, &u);
            transmitir(&mut b.conn, &u, &mut falso, &id).unwrap();
        }
        let c = criar_rascunho(&b.conn, &b.dispositivo, &u, 2, 65, 1, None, 0, "12345678909", "Bia").unwrap();
        b.conn.execute("UPDATE nota_fiscal SET estado = 'cancelada' WHERE id = ?1", [&c]).unwrap();
        nova(&b, &u);
        let descartada = nova(&b, &u);
        descartar(&b.conn, &u, &descartada).unwrap();
        let p = painel(&b.conn, &Filtros::default()).unwrap();
        assert_eq!(
            (p.autorizadas, p.pendentes, p.rejeitadas, p.canceladas, p.sem_resposta, p.total),
            (1, 2, 1, 1, 1, 5)
        );
        assert_eq!(p.ambiente, 2, "homologação de fábrica");
        let so = painel(&b.conn, &Filtros { indicador: "sem_resposta".into(), ..Default::default() }).unwrap();
        assert_eq!((so.notas.len(), so.notas[0].estado.as_str()), (1, "desconhecida"));
        let canc = painel(&b.conn, &Filtros { indicador: "canceladas".into(), ..Default::default() }).unwrap();
        assert_eq!(canc.notas.len(), 1);
        // K8 da R2: o número acha só a nota dele (a chave entra na pesquisa só com 10 dígitos ou mais).
        let numero3 = painel(&b.conn, &Filtros { texto: "3".into(), ..Default::default() }).unwrap();
        assert_eq!(numero3.notas.len(), 1, "{:?}", numero3.notas);
        assert_eq!((numero3.notas[0].numero, numero3.notas[0].estado.as_str()), (3, "desconhecida"));
        let pela_chave = painel(&b.conn, &Filtros { texto: chave(65, 1, 1, 1), ..Default::default() }).unwrap();
        assert_eq!((pela_chave.notas.len(), pela_chave.notas[0].estado.as_str()), (1, "autorizada"));
        assert_eq!(painel(&b.conn, &Filtros { texto: "Bia".into(), ..Default::default() }).unwrap().notas.len(), 1);
        assert!(painel(&b.conn, &Filtros { texto: "456789".into(), ..Default::default() }).unwrap().notas.is_empty());
        let nada =
            painel(&b.conn, &Filtros { de: "2000-01-01".into(), ate: "2000-01-02".into(), ..Default::default() })
                .unwrap();
        assert!(nada.notas.is_empty());
        assert!(painel(&b.conn, &Filtros { modelo: 55, ..Default::default() }).unwrap().notas.is_empty());
        // B2: "hoje" e "últimos 7 dias" contam no dia deste computador.
        let velha = nova(&b, &u);
        b.conn
            .execute(
                "UPDATE nota_fiscal SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-3 days') WHERE id = ?1",
                [&velha],
            )
            .unwrap();
        let hoje = painel(&b.conn, &Filtros { dias: 1, ..Default::default() }).unwrap();
        assert!(!hoje.notas.is_empty() && hoje.notas.iter().all(|n| n.id != velha));
        let sete = painel(&b.conn, &Filtros { dias: 7, ..Default::default() }).unwrap();
        assert!(sete.notas.iter().any(|n| n.id == velha));
    }

    /// Cód-5 da R3 (B2 da R1): a borda do dia local. A nota criada na meia-noite local de hoje (gravada em UTC) é de
    /// "hoje"; a criada um segundo antes, não. Sem mexer no fuso: o SQLite dá a meia-noite local em UTC. (C-5 da R4: num
    /// ambiente em UTC, como a CI, o dia local é o dia UTC e o teste não distingue o código antigo; a prova de que
    /// reprova antes foi feita com `TZ=America/Sao_Paulo`, em `evidencias/R3_reprova_antes.txt`.)
    #[test]
    fn r3_cod5_hoje_comeca_na_meia_noite_local() {
        let (b, u) = banco();
        let meia_noite = nova(&b, &u);
        let antes = nova(&b, &u);
        b.conn
            .execute(
                "UPDATE nota_fiscal SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', date('now', 'localtime'), 'utc') WHERE id = ?1",
                [&meia_noite],
            )
            .unwrap();
        b.conn
            .execute(
                "UPDATE nota_fiscal SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', date('now', 'localtime'), 'utc', '-1 second')
                  WHERE id = ?1",
                [&antes],
            )
            .unwrap();
        let hoje = painel(&b.conn, &Filtros { dias: 1, ..Default::default() }).unwrap();
        assert!(hoje.notas.iter().any(|n| n.id == meia_noite), "a da meia-noite local é de hoje");
        assert!(!hoje.notas.iter().any(|n| n.id == antes), "a de um segundo antes é de ontem");
        let dois = painel(&b.conn, &Filtros { dias: 2, ..Default::default() }).unwrap();
        assert!(dois.notas.iter().any(|n| n.id == antes));
    }

    #[test]
    fn chave_de_acesso_confere_digito_modelo_serie_e_numero() {
        let c = chave(65, 1, 7, 1);
        assert!(chave_e_desta_nota(&c, 65, 1, 7));
        // C-5 da R4: um vetor alfanumérico com o dígito calculado por fora (NT Conjunta 2025.001: ASCII − 48, módulo 11).
        assert!(chave_e_desta_nota("35261112ABC34501DE35650010000000011000000018", 65, 1, 1));
        assert!(!chave_e_desta_nota("35261112ABC34501DE35650010000000011000000017", 65, 1, 1));
        assert!(!chave_e_desta_nota(&c, 55, 1, 7));
        assert!(!chave_e_desta_nota(&c, 65, 2, 7));
        assert!(!chave_e_desta_nota(&c, 65, 1, 8));
        assert!(!chave_e_desta_nota(&c[..43], 65, 1, 7));
        assert!(!chave_e_desta_nota(&format!("{}x", &c[..43]), 65, 1, 7));
        // O exemplo do MOC 7.0, Visão Geral, item 2.2.6.2: soma das ponderações 644, resto 6, dígito 5.
        assert_eq!(digito_da_chave(b"5206043300991100250655012000000780026730161"), 5);
        assert!(chave_e_desta_nota("52060433009911002506550120000007800267301615", 55, 12, 780));
    }
}
