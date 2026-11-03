//! Identidade da loja e entrada de um caixa numa loja (FALHA-01 da auditoria
//! da 0.9.15).
//!
//! Até a 0.9.15 a loja não tinha identidade: dois caixas eram "da mesma loja"
//! só por conhecerem a mesma senha da rede. Duas lojas criadas separadamente
//! com a mesma senha se fundiam pela troca de dados (gerentes somados, faixas
//! de códigos cruzadas). Aqui:
//!
//! - A loja ganha um identificador (`loja_id`), criado na primeira tela e
//!   replicado como as outras configurações da loja (`sync::CONFIG_DA_LOJA`).
//!   Banco de antes, sem identificador: o caixa que distribui as faixas de
//!   códigos (um só por loja) cria ao abrir; os outros recebem pela troca.
//! - Cada pedido e cada resposta da troca levam dois campos a mais, `loja` e
//!   `novo` (sem usuários). O protocolo continua o 3 (campos a mais são
//!   ignorados por quem não conhece; o recurso "loja-v1" diz que o caixa
//!   confere a loja).
//! - Antes de entregar ou de aplicar qualquer alteração, cada lado decide
//!   (`decidir`): mesma loja, caixa novo entrando, ou caixa com quem já trocou
//!   dados de verdade → troca; senão → recusa, com aviso no Início. Um banco
//!   copiado (registrado como caixa novo) ou restaurado leva a mesma
//!   identidade; divergência por falta de rede não muda a identidade.
//! - Um caixa novo (sem usuários) só **recebe**: o que ele tiver não é
//!   aplicado por ninguém. A senha da rede deixa um caixa novo receber a loja;
//!   não deixa ninguém introduzir um gerente nela.
//!
//! O que fica para a etapa D1 (`docs/SIMULTANEIDADE-requisitos.md`): o gerente
//! da loja existente aprovar a entrada de cada caixa novo, com chaves por
//! computador. Até lá, quem tem a senha da rede e um banco sem usuários
//! recebe os dados da loja (como antes).

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::erro::{Erro, Resultado};
use crate::{config, sync, tempo};

/// Configuração da loja com a identidade (vai para todos os caixas).
pub const CHAVE: &str = "loja_id";
/// Recurso da troca de dados: este caixa confere a loja.
pub const RECURSO: &str = "loja-v1";
/// Marca deste computador: começou a entrar numa loja (`sync_juntar`).
const MARCA_JUNCAO: &str = "juntar_tentativa";
/// Marca deste computador: a loja escolhida na entrada (a do primeiro caixa
/// de quem este recebeu), em JSON `{loja, origem}`. Enquanto este caixa não
/// tem usuários (e enquanto a identidade da loja não chegou), só essa loja
/// entrega dados para ele.
const MARCA_ALVO: &str = "juntar_alvo";
/// Marca deste computador: com este caixa a troca já foi aceita.
const PREFIXO_PAR: &str = "loja_par:";
/// Última recusa por ser de outra loja (JSON), para o aviso do Início.
/// Chave da última recusa de um computador de outra loja (aviso crítico do Início).
pub const OUTRA_LOJA: &str = "sync_outra_loja";
/// Por quantos dias o aviso continua depois da última tentativa.
const DIAS_DO_AVISO: f64 = 7.0;

pub const TEXTO_RECUSA_AQUI: &str =
    "Esse computador é de outra loja (foi configurado separadamente). Para não misturar \
     as duas lojas, a troca de dados com ele foi recusada. Se ele devia ser um caixa desta loja, ele precisa entrar \
     pela opção \"Entrar numa loja que já existe\", com o banco vazio.";
pub const TEXTO_RECUSA_LA: &str = "Este computador é de outra loja para o caixa procurado. Para não misturar as duas \
     lojas, a troca de dados foi recusada.";

/// Identidade da loja ("" = este banco ainda não tem).
pub fn id(conn: &Connection) -> Resultado<String> {
    Ok(config::ler(conn, CHAVE)?.unwrap_or_default().trim().to_string())
}

/// Loja nova (primeira tela): cria a identidade, se ainda não tem.
pub fn criar(conn: &Connection) -> Resultado<String> {
    let atual = id(conn)?;
    if !atual.is_empty() {
        return Ok(atual);
    }
    let novo = tempo::novo_id();
    config::gravar(conn, CHAVE, &novo)?;
    Ok(novo)
}

/// Ao abrir: banco de antes da identidade, já com usuários, no caixa que
/// distribui as faixas (um só por loja, replicado): cria a identidade, que
/// chega aos outros caixas pela troca.
pub fn garantir_ao_abrir(conn: &Connection) -> Resultado<()> {
    if !id(conn)?.is_empty() || sem_usuarios(conn)? || !crate::faixas::sou_distribuidor(conn)? {
        return Ok(());
    }
    criar(conn)?;
    crate::registro::anotar("loja: identidade criada (banco de antes da 0.9.15, caixa que distribui as faixas)");
    Ok(())
}

fn sem_usuarios(conn: &Connection) -> Resultado<bool> {
    Ok(crate::usuarios::quantidade(conn)? == 0)
}

/// Os campos que vão em cada pedido e resposta da troca.
pub fn campos(conn: &Connection) -> Resultado<(String, bool)> {
    Ok((id(conn)?, sem_usuarios(conn)?))
}

/// O que fazer com o outro caixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decisao {
    /// A mesma loja (ou caixa novo entrando): troca.
    Aceita,
    /// O outro é um caixa novo (sem usuários): não tem nada que valha
    /// aplicar aqui; ele é que recebe.
    NadaAReceber,
    /// Outra loja: não entrega nem aplica nada.
    Recusa,
}

/// Decide a troca com o caixa `outro`, pelo que ele mandou (`mensagem`, o
/// pedido ou a resposta). `sou_quem_pede`: este caixa está puxando (vai
/// aplicar o que vier).
pub fn decidir(conn: &Connection, outro: &str, mensagem: &Value, sou_quem_pede: bool) -> Resultado<Decisao> {
    let dele = mensagem["loja"].as_str().unwrap_or_default().trim().to_string();
    let ele_novo = mensagem["novo"].as_bool().unwrap_or(false);
    // Este caixa não tem usuários: está entrando numa loja. Ele só recebe, e
    // só da loja escolhida: a do primeiro caixa de quem aceitou receber
    // (fixada aqui, antes de aplicar qualquer coisa, e guardada só neste
    // computador). Os lotes chegam aos poucos; até o usuário e a identidade
    // da loja chegarem, a escolha é que impede de misturar outra loja com a
    // mesma senha da rede (complemento da FALHA-01).
    if sem_usuarios(conn)? {
        // Outro caixa sem usuários não é a fonte de loja nenhuma.
        if ele_novo {
            return Ok(Decisao::NadaAReceber);
        }
        let Some(alvo) = alvo(conn)? else {
            if !sou_quem_pede {
                // Quem pede daqui não recebe nada (este caixa não tem dado próprio).
                return Ok(Decisao::NadaAReceber);
            }
            fixar_alvo(conn, &dele, outro)?;
            return Ok(Decisao::Aceita);
        };
        let mesma = if !alvo.loja.is_empty() && !dele.is_empty() {
            alvo.loja == dele
        } else {
            // Um dos lados sem identidade (loja de antes da 0.9.15): só a
            // mesma origem de antes.
            !alvo.origem.is_empty() && alvo.origem == outro
        };
        if !mesma {
            return Ok(Decisao::Recusa);
        }
        if !sou_quem_pede {
            return Ok(Decisao::NadaAReceber);
        }
        if alvo.loja.is_empty() && !dele.is_empty() {
            // A loja escolhida passou a ter identidade: completa a escolha.
            fixar_alvo(conn, &dele, &alvo.origem)?;
        }
        return Ok(Decisao::Aceita);
    }
    // O outro é um caixa novo: ele recebe; o que ele tiver não é aplicado aqui.
    if ele_novo {
        return Ok(if sou_quem_pede { Decisao::NadaAReceber } else { Decisao::Aceita });
    }
    // A identidade da loja ainda não chegou (o caixa acabou de entrar): vale
    // a loja escolhida na entrada.
    let minha = match id(conn)? {
        m if !m.is_empty() => m,
        _ => alvo(conn)?.map(|a| a.loja).unwrap_or_default(),
    };
    let (puxou_dele, confirmado) = ja_trocou(conn, outro)?;
    if !minha.is_empty() && !dele.is_empty() {
        if minha == dele {
            return Ok(Decisao::Aceita);
        }
        // Identidades diferentes: só vale se os dois já trocavam dados de
        // verdade antes (lojas que já estavam juntas antes desta versão; a
        // configuração replicada faz as duas identidades virarem uma).
        return Ok(if puxou_dele { Decisao::Aceita } else { Decisao::Recusa });
    }
    // Um dos dois (ou os dois) sem identidade: banco de antes desta versão.
    // Só com quem já trocou dados (inclusive alterações dele já aplicadas
    // aqui, de antes desta versão).
    Ok(if puxou_dele || confirmado || tem_alteracao_dele(conn, outro)? { Decisao::Aceita } else { Decisao::Recusa })
}

/// A loja escolhida na entrada: (identidade, caixa de origem).
struct Alvo {
    loja: String,
    origem: String,
}

fn alvo(conn: &Connection) -> Resultado<Option<Alvo>> {
    if let Some(v) = config::ler(conn, MARCA_ALVO)?.and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        return Ok(Some(Alvo {
            loja: v["loja"].as_str().unwrap_or_default().to_string(),
            origem: v["origem"].as_str().unwrap_or_default().to_string(),
        }));
    }
    // Entrada começada antes desta correção, com a identidade já recebida.
    let minha = id(conn)?;
    Ok((!minha.is_empty()).then_some(Alvo { loja: minha, origem: String::new() }))
}

fn fixar_alvo(conn: &Connection, loja: &str, origem: &str) -> Resultado<()> {
    if loja.is_empty() {
        crate::registro::anotar("entrada na loja: loja escolhida (caixa de antes da identidade da loja)");
    }
    config::gravar(conn, MARCA_ALVO, &json!({ "loja": loja, "origem": origem }).to_string())
}

/// Este banco já tem alterações feitas pelo caixa `outro`.
fn tem_alteracao_dele(conn: &Connection, outro: &str) -> Resultado<bool> {
    if outro.is_empty() {
        return Ok(false);
    }
    Ok(conn.query_row("SELECT EXISTS (SELECT 1 FROM alteracao WHERE dispositivo = ?1)", [outro], |l| l.get(0))?)
}

/// (este caixa já recebeu dados dele, a troca com ele já foi aceita antes)
fn ja_trocou(conn: &Connection, outro: &str) -> Resultado<(bool, bool)> {
    if outro.is_empty() {
        return Ok((false, false));
    }
    let puxou: bool = conn
        .query_row("SELECT cursor > 0 FROM sync_par WHERE dispositivo = ?1", [outro], |l| l.get(0))
        .optional()?
        .unwrap_or(false);
    let confirmado = config::ler(conn, &format!("{PREFIXO_PAR}{outro}"))?.is_some();
    Ok((puxou, confirmado))
}

/// A troca com este caixa foi aceita (fica só neste computador).
pub fn confirmar(conn: &Connection, outro: &str) -> Resultado<()> {
    if outro.is_empty() {
        return Ok(());
    }
    let chave = format!("{PREFIXO_PAR}{outro}");
    if config::ler(conn, &chave)?.is_none() {
        config::gravar(conn, &chave, &tempo::agora())?;
    }
    Ok(())
}

/// Anota a recusa (para o aviso do Início e o registro).
pub fn anotar_outra_loja(conn: &Connection, nome: &str, endereco: &str) -> Resultado<()> {
    let antes: Option<Value> = config::ler(conn, OUTRA_LOJA)?.and_then(|t| serde_json::from_str(&t).ok());
    let agora = tempo::agora();
    // Não regrava a cada poucos segundos (a troca tenta de novo sozinha).
    let recente = antes.as_ref().is_some_and(|a| {
        a["nome"].as_str() == Some(nome) && a["endereco"].as_str() == Some(endereco) && {
            let q: f64 = conn
                .query_row(
                    "SELECT (julianday(?1) - julianday(?2)) * 86400",
                    params![agora, a["quando"].as_str()],
                    |l| l.get::<_, Option<f64>>(0),
                )
                .ok()
                .flatten()
                .unwrap_or(f64::MAX);
            q < 60.0
        }
    });
    if recente {
        return Ok(());
    }
    if antes.is_none() {
        crate::registro::anotar(&format!("troca de dados recusada: caixa de outra loja ({endereco})"));
    }
    let nome: String = nome.chars().take(60).collect();
    config::gravar(conn, OUTRA_LOJA, &json!({ "nome": nome, "endereco": endereco, "quando": agora }).to_string())?;
    Ok(())
}

/// Recusa recente por ser de outra loja: (nome do outro caixa, endereço, quando).
pub fn ultima_recusa(conn: &Connection) -> Resultado<Option<(String, String, String)>> {
    let Some(v) = config::ler(conn, OUTRA_LOJA)?.and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
        return Ok(None);
    };
    let quando = v["quando"].as_str().unwrap_or_default().to_string();
    let dias: f64 = conn
        .query_row("SELECT julianday('now') - julianday(?1)", [&quando], |l| l.get::<_, Option<f64>>(0))?
        .unwrap_or(f64::MAX);
    if dias > DIAS_DO_AVISO {
        return Ok(None);
    }
    Ok(Some((
        v["nome"].as_str().unwrap_or_default().to_string(),
        v["endereco"].as_str().unwrap_or_default().to_string(),
        quando,
    )))
}

/// A senha da rede mudou: a recusa de antes não vale mais para o aviso.
pub fn esquecer_recusa(conn: &Connection) -> Resultado<()> {
    conn.execute("DELETE FROM config WHERE chave = ?1", [OUTRA_LOJA])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entrar numa loja que já existe (e desistir).
// ---------------------------------------------------------------------------

/// `sync_juntar`: este computador começou a entrar numa loja.
pub fn comecar_juncao(conn: &Connection) -> Resultado<()> {
    config::gravar(conn, MARCA_JUNCAO, &tempo::agora())
}

/// Recebeu alguma alteração de outro caixa.
pub fn recebeu_de_outro(conn: &Connection) -> Resultado<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM alteracao
                         WHERE dispositivo <> COALESCE((SELECT valor FROM config WHERE chave = 'dispositivo_id'), ''))",
        [],
        |l| l.get(0),
    )?)
}

/// Está no meio de uma entrada numa loja (sem usuários ainda): marca da
/// junção, ou dados que vieram de outro caixa.
pub fn juntando(conn: &Connection) -> Resultado<bool> {
    Ok(sem_usuarios(conn)?
        && (config::ler(conn, MARCA_JUNCAO)?.is_some()
            || config::ler(conn, MARCA_ALVO)?.is_some()
            || recebeu_de_outro(conn)?))
}

/// Desistir de entrar numa loja (o caixa ainda sem usuários): tudo o que a
/// tentativa deixou sai, numa transação só — a troca de dados desligada, a
/// senha e os endereços da rede, os caixas conhecidos, as pendências, e cada
/// registro que veio de outro caixa (o banco não tinha dado próprio). Um
/// registro de fábrica (tabela de preço, unidade, categoria...) que a outra
/// loja tenha mudado volta a ser o de fábrica. Devolve quantos registros saíram.
///
/// A4 (0.9.15): só com uma tentativa de verdade (a marca da entrada, a loja
/// escolhida ou dados de outro caixa); sem ela, não mexe em nada (repetir a
/// desistência também não). Um registro que veio de outro caixa e também foi
/// mexido aqui tem origem incerta: nada é apagado e a desistência é recusada
/// (a loja nova continua bloqueada, para a recuperação com o suporte).
/// Preferências deste computador, a identidade dele e os arquivos (backups)
/// não são tocados.
pub fn desistir_da_juncao(conn: &mut Connection) -> Resultado<usize> {
    if !sem_usuarios(conn)? {
        return Err(Erro::validacao(
            "Este caixa já está configurado (tem usuários): não há junção para desistir. Para mudar a troca de \
             dados, use Configurações.",
        ));
    }
    let marcado = config::ler(conn, MARCA_JUNCAO)?.is_some() || config::ler(conn, MARCA_ALVO)?.is_some();
    if !marcado && !recebeu_de_outro(conn)? {
        return Ok(0);
    }
    let eu = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let incertos: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (SELECT DISTINCT a.tabela, a.chave FROM alteracao a
                                WHERE a.dispositivo <> ?1
                                  AND EXISTS (SELECT 1 FROM alteracao b
                                               WHERE b.dispositivo = ?1 AND b.tabela = a.tabela AND b.chave = a.chave))",
        [&eu],
        |l| l.get(0),
    )?;
    if incertos > 0 {
        crate::registro::anotar(&format!(
            "entrada na loja: desistência recusada ({incertos} registros de origem incerta)"
        ));
        return Err(Erro::validacao(format!(
            "Não deu para desfazer a entrada na loja: {incertos} registro(s) vieram de outro caixa e também foram \
             mexidos neste computador, e não dá para saber de quem são. Nada foi apagado. Chame o suporte para \
             recuperar este computador antes de criar uma loja nova."
        )));
    }
    let tocados: Vec<(String, String)> = conn
        .prepare("SELECT DISTINCT tabela, chave FROM alteracao WHERE dispositivo <> ?1")?
        .query_map([&eu], |l| Ok((l.get(0)?, l.get(1)?)))?
        .collect::<Result<_, _>>()?;
    // Um banco novo, de fábrica, para pôr de volta o que era de fábrica.
    let fabrica = crate::banco::Banco::em_memoria()?;
    let tx = conn.transaction()?;
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    tx.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", [])?;
    let mut saiu = 0;
    for (tabela, chave) in &tocados {
        if !sync::TABELAS.contains(&tabela.as_str()) {
            continue;
        }
        let (todas, pk) = sync::colunas_da_tabela(&tx, tabela)?;
        let valores: Vec<Value> = serde_json::from_str(chave).unwrap_or_default();
        if valores.len() != pk.len() || pk.is_empty() {
            continue;
        }
        let onde =
            pk.iter().enumerate().map(|(i, c)| format!("\"{c}\" = ?{}", i + 1)).collect::<Vec<_>>().join(" AND ");
        let args: Vec<rusqlite::types::Value> = valores.iter().map(json_para_sql).collect();
        saiu +=
            tx.execute(&format!("DELETE FROM \"{tabela}\" WHERE {onde}"), rusqlite::params_from_iter(args.iter()))?;
        if tabela == "config" {
            continue;
        }
        // De fábrica? Volta como era.
        let lista = todas.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let linha: Option<Vec<rusqlite::types::Value>> = fabrica
            .conn
            .query_row(
                &format!("SELECT {lista} FROM \"{tabela}\" WHERE {onde}"),
                rusqlite::params_from_iter(args.iter()),
                |l| (0..todas.len()).map(|i| l.get::<_, rusqlite::types::Value>(i)).collect(),
            )
            .optional()?;
        if let Some(linha) = linha {
            let marcas = (1..=todas.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
            tx.execute(
                &format!("INSERT INTO \"{tabela}\" ({lista}) VALUES ({marcas})"),
                rusqlite::params_from_iter(linha.iter()),
            )?;
        }
    }
    tx.execute("DELETE FROM alteracao WHERE dispositivo <> ?1", [&eu])?;
    for t in ["sync_par", "sync_endereco", "sync_pendencia", "sync_referencia", "sync_conflito"] {
        tx.execute(&format!("DELETE FROM {t}"), [])?;
    }
    tx.execute(
        "DELETE FROM config WHERE chave IN ('sync_ligado', 'sync_senha', 'sync_enderecos', 'sync_pendencias',
                                            'sync_quebradas', 'sync_atrasadas', ?1, ?2, ?4)
            OR chave LIKE ?3",
        params![MARCA_JUNCAO, OUTRA_LOJA, format!("{PREFIXO_PAR}%"), MARCA_ALVO],
    )?;
    tx.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", [])?;
    // Faixas de códigos: o que este caixa possa ter recebido não vale mais.
    crate::faixas::nova_geracao(&tx, "desistiu de entrar na loja")?;
    config::gravar(&tx, "sync_geracao", &tempo::novo_id())?;
    // Só nos testes: uma falha no fim da limpeza (a transação inteira volta).
    #[cfg(test)]
    if config::ler(&tx, "juntar_falhar_teste")?.is_some() {
        return Err(Erro::Interno("falha simulada no meio da desistência".into()));
    }
    tx.commit()?;
    crate::registro::anotar(&format!("entrada na loja desfeita: {saiu} registros recebidos saíram"));
    Ok(saiu)
}

fn json_para_sql(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as V;
    match v {
        Value::Null => V::Null,
        Value::Bool(b) => V::Integer(*b as i64),
        Value::Number(n) => n.as_i64().map(V::Integer).or_else(|| n.as_f64().map(V::Real)).unwrap_or(V::Null),
        Value::String(s) => V::Text(s.clone()),
        outro => V::Text(outro.to_string()),
    }
}
