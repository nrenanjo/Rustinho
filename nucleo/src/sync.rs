//! Sincronização entre os caixas da loja (pela rede local / Wi-Fi).
//!
//! Como funciona:
//! - Toda alteração nas tabelas da loja vira uma linha em `alteracao`
//!   (gatilhos do SQLite criados ao abrir o banco).
//! - Cada caixa escuta na porta TCP 47810 e anuncia que existe pela porta
//!   UDP 47811 (assim um acha o outro sozinho). Também dá para digitar o
//!   endereço dos outros caixas.
//! - A cada poucos segundos, cada caixa pede aos outros as alterações que
//!   ainda não tem ("desde a número N") e aplica.
//! - Cada registro fica com a alteração mais recente (o id da alteração é um
//!   UUID v7, que já vem em ordem de tempo). O estoque é só soma de
//!   movimentos, então nunca há conflito nele.
//! - Tudo que passa pela rede vai criptografado (ChaCha20-Poly1305) com uma
//!   chave tirada da "senha da rede" da loja por Argon2id, com um "sal" de
//!   cada caixa que vai junto na mensagem (protocolo versão 2, desde a
//!   0.9.11). A senha precisa ser a mesma nos caixas: sem ela, nada é lido
//!   nem aceito.
//! - Registros repetidos no log de alterações (versões velhas de um mesmo
//!   registro) são apagados de tempos em tempos: só a mais nova importa.
//! - Um caixa com o relógio errado ganharia sempre; a diferença de relógio
//!   aparece como aviso na tela dos caixas da loja.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockWriteGuard};
use std::thread;
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use rusqlite::types::Value as SqlValor;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::erro::{Erro, Resultado};
use crate::{banco, config, tempo};

pub const PORTA_TCP: u16 = 47810;
pub const PORTA_UDP: u16 = 47811;
const LOTE: i64 = 2000;
/// Resposta com um lote de alterações.
const TAMANHO_MAXIMO: usize = 64 * 1024 * 1024;
/// Pedido de outro caixa (é sempre pequeno).
const TAMANHO_PEDIDO: usize = 64 * 1024;
/// Conexões atendidas ao mesmo tempo (as outras esperam a próxima rodada).
const CONEXOES_MAXIMAS: usize = 8;
/// Diferença de relógio entre caixas que já vira aviso.
const RELOGIO_TOLERANCIA_MS: i64 = 2 * 60 * 1000;
/// Começo de toda mensagem do protocolo 2.
const MAGICA: &[u8; 4] = b"RST2";

/// Enquanto um backup é restaurado, a troca de dados espera (e vice-versa).
static TRAVA: RwLock<()> = RwLock::new(());
static ATENDENDO: AtomicUsize = AtomicUsize::new(0);

/// Uma vaga de atendimento. Ao sair de cena (fim normal, erro ou pânico da
/// tarefa) a vaga volta (C-01): o contador nunca fica preso.
struct Vaga(&'static AtomicUsize);

impl Vaga {
    fn pegar() -> Option<Vaga> {
        Vaga::pegar_de(&ATENDENDO)
    }

    fn pegar_de(contador: &'static AtomicUsize) -> Option<Vaga> {
        let mut atual = contador.load(Ordering::SeqCst);
        loop {
            if atual >= CONEXOES_MAXIMAS {
                return None;
            }
            match contador.compare_exchange(atual, atual + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Some(Vaga(contador)),
                Err(outro) => atual = outro,
            }
        }
    }
}

impl Drop for Vaga {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Para a troca de dados enquanto a guarda existir (restaurar backup).
pub fn pausar() -> RwLockWriteGuard<'static, ()> {
    TRAVA.write().unwrap_or_else(|e| e.into_inner())
}

/// Tabelas que os caixas compartilham.
pub const TABELAS: &[&str] = &[
    "config",
    "usuario",
    "tabela_preco",
    "regra_st",
    "unidade",
    "produto",
    "produto_preco",
    "produto_aplicacao",
    "produto_equivalente",
    "produto_fornecedor",
    "estoque_movimento",
    "pessoa",
    "pessoa_veiculo",
    "compra",
    "compra_item",
    "caixa_sessao",
    "caixa_movimento",
    "venda",
    "venda_item",
    "venda_pagamento",
    "conta_receber",
    "recebimento",
    "recebimento_parcela",
    "conta_loja",
    "categoria",
    "conta_pagar",
    "pagamento",
    "movimento_conta",
    "cheque",
    "comissao_fechamento",
    "encomenda",
    "devolucao",
    "devolucao_item",
    "vale",
    "ordem_servico",
    "ordem_servico_item",
    "auditoria",
];

/// Configurações que valem para a loja toda. As outras (nome deste caixa,
/// senha da rede...) são de cada computador.
pub const CONFIG_DA_LOJA: &[&str] = &[
    "loja_nome",
    "loja_uf",
    "loja_cnpj",
    "loja_endereco",
    "loja_telefone",
    "crediario_multa",
    "crediario_juros",
    "crediario_carencia",
    "orcamento_validade",
    "rodape_venda",
    "rodape_orcamento",
    "rodape_os",
    "imprimir_local",
    "estoque_dias_compra",
    "estoque_dias_parado",
    // "backup_senha" (até a 0.9.1) e "backup_chave" (rascunho da 0.9.11)
    // ficam na lista para o apagar chegar aos outros caixas.
    "backup_senha",
    "backup_chave",
    // Parte pública da chave do backup (a secreta só vai trancada, no usuário).
    "backup_publica",
    "gerente_livres",
];

// ---------------------------------------------------------------------------
// Gatilhos e registro das alterações.
// ---------------------------------------------------------------------------

fn colunas(conn: &Connection, tabela: &str) -> Resultado<(Vec<String>, Vec<String>)> {
    let mut c = conn.prepare(&format!("PRAGMA table_info(\"{tabela}\")"))?;
    let mut linhas: Vec<(String, i64)> = c.query_map([], |l| Ok((l.get(1)?, l.get(5)?)))?.collect::<Result<_, _>>()?;
    let todas: Vec<String> = linhas.iter().map(|l| l.0.clone()).collect();
    linhas.retain(|l| l.1 > 0);
    linhas.sort_by_key(|l| l.1);
    Ok((todas, linhas.into_iter().map(|l| l.0).collect()))
}

fn expressoes(tabela: &str, todas: &[String], chave: &[String], prefixo: &str) -> (String, String) {
    let chave =
        format!("json_array({})", chave.iter().map(|c| format!("{prefixo}.\"{c}\"")).collect::<Vec<_>>().join(", "));
    let dados = format!(
        "json_object({})",
        todas.iter().map(|c| format!("'{c}', {prefixo}.\"{c}\"")).collect::<Vec<_>>().join(", ")
    );
    let _ = tabela;
    (chave, dados)
}

fn filtro_config(prefixo: &str) -> String {
    format!(
        " AND {prefixo}.chave IN ({})",
        CONFIG_DA_LOJA.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ")
    )
}

/// (Re)cria os gatilhos que registram as alterações. Roda ao abrir o banco.
pub fn criar_gatilhos(conn: &Connection) -> Resultado<()> {
    for tabela in TABELAS {
        let (todas, chave) = colunas(conn, tabela)?;
        if chave.is_empty() {
            return Err(Erro::Interno(format!("a tabela {tabela} não tem chave primária")));
        }
        let quando = "WHEN (SELECT aplicando FROM sync_estado WHERE id = 1) = 0";
        let disp = "(SELECT valor FROM config WHERE chave = 'dispositivo_id')";
        let mut sql = String::new();
        for (sufixo, evento, prefixo, operacao) in
            [("i", "INSERT", "NEW", "gravar"), ("u", "UPDATE", "NEW", "gravar"), ("d", "DELETE", "OLD", "apagar")]
        {
            let (k, d) = expressoes(tabela, &todas, &chave, prefixo);
            let dados = if operacao == "apagar" { "NULL".to_string() } else { d };
            let extra = if *tabela == "config" { filtro_config(prefixo) } else { String::new() };
            sql.push_str(&format!(
                "DROP TRIGGER IF EXISTS sync_{tabela}_{sufixo};
                 CREATE TRIGGER sync_{tabela}_{sufixo} AFTER {evento} ON \"{tabela}\" {quando}{extra}
                 BEGIN
                   INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base)
                   VALUES (rustinho_id(), '{tabela}', {k}, '{operacao}', {dados}, {disp}, rustinho_agora(),
                           (SELECT b.id FROM alteracao b WHERE b.tabela = '{tabela}' AND b.chave = {k}
                             ORDER BY b.id DESC LIMIT 1));
                 END;"
            ));
        }
        conn.execute_batch(&sql)?;
    }
    Ok(())
}

/// Registra como alteração tudo que já existia antes da sincronização (só
/// uma vez). Num caixa novo, sem usuários, não precisa: ele vai receber
/// tudo dos outros.
pub fn registrar_existentes(conn: &Connection) -> Resultado<()> {
    if config::ler(conn, "sync_inicio")?.is_some() {
        return Ok(());
    }
    let usuarios: i64 = conn.query_row("SELECT COUNT(*) FROM usuario", [], |l| l.get(0))?;
    if usuarios > 0 {
        for tabela in TABELAS {
            let (todas, chave) = colunas(conn, tabela)?;
            let (k, d) = expressoes(tabela, &todas, &chave, "t");
            let filtro =
                if *tabela == "config" { format!(" WHERE 1 = 1{}", filtro_config("t")) } else { String::new() };
            conn.execute(
                &format!(
                    "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando)
                     SELECT rustinho_id(), '{tabela}', {k}, 'gravar', {d},
                            (SELECT valor FROM config WHERE chave = 'dispositivo_id'), rustinho_agora()
                       FROM \"{tabela}\" t{filtro}"
                ),
                [],
            )?;
        }
    }
    config::gravar(conn, "sync_inicio", &tempo::agora())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ler e aplicar alterações.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alteracao {
    pub seq: i64,
    pub id: String,
    pub tabela: String,
    pub chave: String,
    pub operacao: String,
    pub dados: Option<String>,
    pub dispositivo: String,
    pub quando: String,
    /// A versão do registro que o caixa conhecia quando mexeu (id da
    /// alteração). Caixas de antes da 0.9.11 não mandam.
    #[serde(default)]
    pub base: Option<String>,
}

/// Alterações deste banco depois da número `desde`.
pub fn alteracoes_desde(conn: &Connection, desde: i64, limite: i64) -> Resultado<(Vec<Alteracao>, bool)> {
    let mut c = conn.prepare_cached(
        "SELECT seq, id, tabela, chave, operacao, dados, dispositivo, quando, base FROM alteracao
          WHERE seq > ?1 ORDER BY seq LIMIT ?2",
    )?;
    let lista: Vec<Alteracao> = c
        .query_map(params![desde, limite + 1], |l| {
            Ok(Alteracao {
                seq: l.get(0)?,
                id: l.get(1)?,
                tabela: l.get(2)?,
                chave: l.get(3)?,
                operacao: l.get(4)?,
                dados: l.get(5)?,
                dispositivo: l.get(6)?,
                quando: l.get(7)?,
                base: l.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let tem_mais = lista.len() as i64 > limite;
    Ok((lista.into_iter().take(limite as usize).collect(), tem_mais))
}

fn para_sql(v: &Value) -> SqlValor {
    match v {
        Value::Null => SqlValor::Null,
        Value::Bool(b) => SqlValor::Integer(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => SqlValor::Integer(i),
            None => SqlValor::Real(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => SqlValor::Text(s.clone()),
        outro => SqlValor::Text(outro.to_string()),
    }
}

/// Aplica alterações vindas de outro caixa. Devolve quantas eram novas.
pub fn aplicar(conn: &mut Connection, lista: &[Alteracao]) -> Resultado<usize> {
    // Na ordem em que chegam, um item pode vir antes da peça (ou a peça ter
    // sido apagada depois). As chaves estrangeiras ficam desligadas aqui e
    // são conferidas no fim da troca (veja `pendencias`).
    conn.pragma_update(None, "foreign_keys", false)?;
    let resultado = aplicar_dentro(conn, lista);
    conn.pragma_update(None, "foreign_keys", true)?;
    resultado
}

/// Hora (ms desde 1970) guardada no começo de um UUID v7.
fn ms_do_id(id: &str) -> Option<i64> {
    let hex: String = id.chars().filter(|c| *c != '-').take(12).collect();
    if hex.len() < 12 {
        return None;
    }
    i64::from_str_radix(&hex, 16).ok()
}

/// Quantos registros apontam para outro que ainda não chegou (ou foi
/// apagado). Normalmente é zero depois de trocar tudo com os outros caixas.
pub fn pendencias(conn: &Connection) -> Resultado<usize> {
    let mut c = conn.prepare("PRAGMA foreign_key_check")?;
    let n = c.query_map([], |_| Ok(()))?.count();
    Ok(n)
}

/// Apaga do log as versões velhas de cada registro (fica a mais nova, que é
/// a que vale). Devolve quantas apagou. Pode rodar a qualquer hora: quem
/// pedir depois recebe só a versão que vale.
pub fn compactar(conn: &Connection) -> Resultado<usize> {
    let mut total = 0;
    loop {
        let n = conn.execute(
            "DELETE FROM alteracao WHERE seq IN (
                SELECT a.seq FROM alteracao a
                 WHERE EXISTS (SELECT 1 FROM alteracao b
                                WHERE b.tabela = a.tabela AND b.chave = a.chave AND b.id > a.id)
                 LIMIT 5000)",
            [],
        )?;
        total += n;
        if n < 5000 {
            return Ok(total);
        }
    }
}

/// Uma versão de um registro no log de alterações.
struct Versao {
    id: String,
    dispositivo: String,
    base: Option<String>,
    dados: Option<String>,
}

/// Colunas que mudam sozinhas (hora da última entrada, carimbo de
/// alteração, cadeado do backup) e não contam como conflito.
/// (`dispositivo` diz só quem mexeu por último; `busca` sai do nome.)
const SEM_CONFLITO: &[&str] = &["atualizado_em", "ultimo_acesso", "backup_acesso", "dispositivo", "busca"];

/// Colunas que diferem entre duas versões (sem as que mudam sozinhas).
pub fn diferencas(a: Option<&str>, b: Option<&str>) -> Vec<String> {
    let ler = |t: Option<&str>| -> Option<serde_json::Map<String, Value>> {
        t.and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|v| v.as_object().cloned())
    };
    match (ler(a), ler(b)) {
        (Some(x), Some(y)) => {
            let mut cols: Vec<String> = x
                .keys()
                .chain(y.keys())
                .filter(|k| !SEM_CONFLITO.contains(&k.as_str()) && x.get(*k) != y.get(*k))
                .cloned()
                .collect();
            cols.sort();
            cols.dedup();
            cols
        }
        // Um apagou e o outro mexeu (ou os dois apagaram).
        (None, None) => Vec::new(),
        _ => vec!["(apagado)".into()],
    }
}

/// As duas versões foram feitas sem uma ver a outra? `ficou` é a mais nova
/// pelo id (a que vale); `chegou_e_perdeu`: a que perdeu acabou de chegar.
/// - Chegou e ganhou: quem fez `ficou` conhecia uma versão mais velha que
///   a nossa (`base` < `perdeu`), então não viu a nossa.
/// - Chegou e perdeu: nós nunca tivemos a que chegou, então a nossa não a
///   viu. Só conta se as duas foram feitas no mesmo dia (versões bem velhas
///   que voltam depois de uma restauração não são conflito).
/// - Ou `perdeu` foi feita em cima de `ficou` e perdeu só porque o relógio
///   do caixa dela estava atrasado.
///
/// Sem a "base" (caixa de antes da 0.9.11): a menos de 30 minutos uma da
/// outra. E só conta se mudou algo de verdade.
fn em_conflito(ficou: &Versao, perdeu: &Versao, chegou_e_perdeu: bool) -> bool {
    let perto = |limite: i64| match (ms_do_id(&ficou.id), ms_do_id(&perdeu.id)) {
        (Some(x), Some(y)) => (x - y).abs() < limite,
        _ => false,
    };
    let sem_ver = if chegou_e_perdeu {
        ficou.base.as_deref() != Some(perdeu.id.as_str()) && perto(24 * 60 * 60 * 1000)
    } else {
        match &ficou.base {
            Some(b) => b.as_str() < perdeu.id.as_str(),
            None => perto(30 * 60 * 1000),
        }
    };
    let relogio = perdeu.base.as_deref() == Some(ficou.id.as_str());
    (sem_ver || relogio) && !diferencas(ficou.dados.as_deref(), perdeu.dados.as_deref()).is_empty()
}

fn anotar_conflito(conn: &Connection, tabela: &str, chave: &str, ficou: &Versao, perdeu: &Versao) -> Resultado<()> {
    let ja: bool = conn
        .query_row("SELECT 1 FROM sync_conflito WHERE perdeu = ?1", [&perdeu.id], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if ja {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo, dados_perdeu, dispositivo_ficou)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'registro', ?7, ?8)",
        params![tabela, chave, ficou.id, perdeu.id, perdeu.dispositivo, tempo::agora(), perdeu.dados, ficou.dispositivo],
    )?;
    // Guarda os últimos 500 já resolvidos (os abertos ficam sempre).
    conn.execute(
        "DELETE FROM sync_conflito WHERE resolvido_em IS NOT NULL
            AND id <= (SELECT MAX(id) - 500 FROM sync_conflito)",
        [],
    )?;
    Ok(())
}

fn aplicar_dentro(conn: &mut Connection, lista: &[Alteracao]) -> Resultado<usize> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", [])?;
    let meu = config::ler(&tx, "dispositivo_id")?.unwrap_or_default();
    let mut novas = 0;
    let mut colunas_cache: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for a in lista {
        if !TABELAS.contains(&a.tabela.as_str()) || a.dispositivo == meu {
            continue;
        }
        let ja: bool =
            tx.query_row("SELECT 1 FROM alteracao WHERE id = ?1", [&a.id], |_| Ok(true)).optional()?.unwrap_or(false);
        if ja {
            continue;
        }
        // A versão mais nova que este caixa tem deste registro.
        let ultima: Option<Versao> = tx
            .query_row(
                "SELECT id, dispositivo, base, dados FROM alteracao WHERE tabela = ?1 AND chave = ?2 ORDER BY id DESC LIMIT 1",
                params![a.tabela, a.chave],
                |l| Ok(Versao { id: l.get(0)?, dispositivo: l.get(1)?, base: l.get(2)?, dados: l.get(3)? }),
            )
            .optional()?;
        tx.execute(
            "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![a.id, a.tabela, a.chave, a.operacao, a.dados, a.dispositivo, a.quando, a.base],
        )?;
        novas += 1;
        let chegou = Versao {
            id: a.id.clone(),
            dispositivo: a.dispositivo.clone(),
            base: a.base.clone(),
            dados: a.dados.clone(),
        };
        if let Some(u) = &ultima {
            // Vale a mais nova (pelo id). Se as duas foram feitas sem uma ver
            // a outra (ou o relógio inverteu a ordem), fica anotado para o
            // gerente: o que perdeu continua guardado.
            let chegou_e_perdeu = u.id.as_str() >= a.id.as_str();
            let (ficou, perdeu) = if chegou_e_perdeu { (u, &chegou) } else { (&chegou, u) };
            if u.dispositivo == meu && a.tabela != "auditoria" && em_conflito(ficou, perdeu, chegou_e_perdeu) {
                anotar_conflito(&tx, &a.tabela, &a.chave, ficou, perdeu)?;
            }
            if u.id.as_str() >= a.id.as_str() {
                continue;
            }
        }
        // Quem mandou esta já tinha visto a versão que ganhou de um conflito
        // aberto aqui: o conflito foi resolvido lá.
        if let Some(b) = &a.base {
            tx.execute(
                "UPDATE sync_conflito SET resolvido_em = ?4, resolucao = 'resolvido em outro caixa'
                  WHERE tabela = ?1 AND chave = ?2 AND ficou = ?3 AND resolvido_em IS NULL",
                params![a.tabela, a.chave, b, tempo::agora()],
            )?;
        }
        if !colunas_cache.iter().any(|c| c.0 == a.tabela) {
            let (todas, chave) = colunas(&tx, &a.tabela)?;
            colunas_cache.push((a.tabela.clone(), todas, chave));
        }
        let Some((_, todas, chave)) = colunas_cache.iter().find(|c| c.0 == a.tabela) else { continue };
        let valores_chave: Vec<Value> = serde_json::from_str(&a.chave)?;
        if valores_chave.len() != chave.len() {
            continue;
        }
        if a.operacao == "apagar" {
            let onde = chave
                .iter()
                .enumerate()
                .map(|(i, c)| format!("\"{c}\" = ?{}", i + 1))
                .collect::<Vec<_>>()
                .join(" AND ");
            tx.execute(
                &format!("DELETE FROM \"{}\" WHERE {onde}", a.tabela),
                params_from_iter(valores_chave.iter().map(para_sql)),
            )?;
            continue;
        }
        let dados: serde_json::Map<String, Value> = match a.dados.as_deref().map(serde_json::from_str::<Value>) {
            Some(Ok(Value::Object(m))) => m,
            _ => continue,
        };
        // Só as colunas que existem aqui (um caixa com versão mais nova pode ter mais).
        let cols: Vec<&String> = todas.iter().filter(|c| dados.contains_key(c.as_str())).collect();
        if chave.iter().any(|k| !dados.contains_key(k.as_str())) {
            continue;
        }
        let nomes = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let marcas = (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
        let atualizar: Vec<String> =
            cols.iter().filter(|c| !chave.contains(c)).map(|c| format!("\"{c}\" = excluded.\"{c}\"")).collect();
        let conflito = if atualizar.is_empty() {
            "DO NOTHING".to_string()
        } else {
            format!("DO UPDATE SET {}", atualizar.join(", "))
        };
        let sql = format!(
            "INSERT INTO \"{}\" ({nomes}) VALUES ({marcas}) ON CONFLICT ({}) {conflito}",
            a.tabela,
            chave.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")
        );
        tx.execute(&sql, params_from_iter(cols.iter().map(|c| para_sql(&dados[c.as_str()]))))?;
    }
    // Conflito em que, no fim, a versão que vale ficou igual à que perdeu
    // (apagar e gravar de novo o mesmo preço, por exemplo) não é conflito.
    let abertos: Vec<(i64, String, String, Option<String>)> = {
        let mut c = tx.prepare(
            "SELECT id, tabela, chave, dados_perdeu FROM sync_conflito
              WHERE tipo = 'registro' AND (resolvido_em IS NULL OR resolucao = 'resolvido em outro caixa')",
        )?;
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?;
        l.collect::<Result<_, _>>()?
    };
    for (id, tabela, chave, perdeu) in abertos {
        let atual: Option<String> = tx
            .query_row(
                "SELECT dados FROM alteracao WHERE tabela = ?1 AND chave = ?2 ORDER BY id DESC LIMIT 1",
                params![tabela, chave],
                |l| l.get(0),
            )
            .optional()?
            .flatten();
        if diferencas(atual.as_deref(), perdeu.as_deref()).is_empty() {
            tx.execute("DELETE FROM sync_conflito WHERE id = ?1", [id])?;
        }
    }
    // A que perdeu foi substituída depois pelo próprio caixa que a fez
    // (apagou e gravou de novo, por exemplo): vale a última dele.
    tx.execute(
        "DELETE FROM sync_conflito WHERE tipo = 'registro' AND resolvido_em IS NULL
            AND EXISTS (SELECT 1 FROM alteracao x WHERE x.tabela = sync_conflito.tabela
                           AND x.chave = sync_conflito.chave AND x.dispositivo = sync_conflito.dispositivo
                           AND x.id > sync_conflito.perdeu)",
        [],
    )?;
    tx.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", [])?;
    tx.commit()?;
    Ok(novas)
}

/// Depois de juntar os dados dos caixas, confere o que nenhuma alteração
/// sozinha mostra:
/// - o mesmo orçamento virou venda em dois caixas sem rede (F-12): fica
///   anotado para o gerente cancelar a que sobrou;
/// - a loja ficou sem gerente (cada caixa tirou um, sem ver o outro, C-06):
///   o usuário mais antigo (quem configurou a loja) volta a ser gerente.
///   Todos os caixas escolhem o mesmo, então o resultado é igual em todos.
pub fn verificar_depois(conn: &Connection) -> Resultado<()> {
    let repetidos: Vec<(String, String)> = {
        let mut c = conn.prepare(
            "SELECT orcamento_id, group_concat(id, ',') FROM
               (SELECT orcamento_id, id FROM venda
                 WHERE tipo = 'venda' AND situacao = 'concluida' AND orcamento_id IS NOT NULL ORDER BY id)
              GROUP BY orcamento_id HAVING COUNT(*) > 1",
        )?;
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?;
        l.collect::<Result<_, _>>()?
    };
    for (orcamento, vendas) in repetidos {
        let ja: bool = conn
            .query_row(
                "SELECT 1 FROM sync_conflito WHERE tipo = 'orcamento' AND chave = ?1 AND perdeu = ?2",
                params![orcamento, vendas],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !ja {
            let primeira = vendas.split(',').next().unwrap_or_default().to_string();
            conn.execute(
                "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
                 VALUES ('venda', ?1, ?2, ?3, '', ?4, 'orcamento')",
                params![orcamento, primeira, vendas, tempo::agora()],
            )?;
        }
    }
    let (usuarios, gerentes): (i64, i64) =
        conn.query_row("SELECT COUNT(*), COALESCE(SUM(gerente = 1), 0) FROM usuario WHERE ativo = 1", [], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })?;
    if usuarios > 0 && gerentes == 0 {
        let id: String =
            conn.query_row("SELECT id FROM usuario WHERE ativo = 1 ORDER BY criado_em, id LIMIT 1", [], |l| l.get(0))?;
        conn.execute("UPDATE usuario SET gerente = 1, atualizado_em = ?2 WHERE id = ?1", params![id, tempo::agora()])?;
        conn.execute(
            "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
             VALUES ('usuario', ?1, '', '', '', ?2, 'gerente')",
            params![id, tempo::agora()],
        )?;
    }
    Ok(())
}

/// Tabelas em que o gerente pode escolher "usar a versão que perdeu"
/// (cadastros). Nas de dinheiro e movimento (vendas, caixa, contas), mexer
/// assim poderia desequilibrar as contas: lá o gerente só fica ciente e
/// corrige pela tela de cada coisa.
const PODE_USAR: &[&str] = &[
    "produto",
    "produto_preco",
    "produto_aplicacao",
    "produto_equivalente",
    "produto_fornecedor",
    "pessoa",
    "pessoa_veiculo",
    "tabela_preco",
    "regra_st",
    "unidade",
    "categoria",
    "conta_loja",
    "config",
];

fn nome_da_tabela(t: &str) -> &'static str {
    match t {
        "produto" | "produto_preco" | "produto_aplicacao" | "produto_equivalente" | "produto_fornecedor" => "Peça",
        "pessoa" | "pessoa_veiculo" => "Cliente ou fornecedor",
        "usuario" => "Usuário",
        "config" => "Configuração da loja",
        "tabela_preco" => "Tabela de preço",
        "venda" | "venda_item" | "venda_pagamento" => "Venda",
        "caixa_sessao" | "caixa_movimento" => "Caixa",
        "conta_receber" | "recebimento" | "recebimento_parcela" => "Contas a receber",
        "conta_pagar" | "pagamento" => "Contas a pagar",
        "conta_loja" | "movimento_conta" | "categoria" => "Financeiro",
        "cheque" => "Cheque",
        "encomenda" => "Encomenda",
        "devolucao" | "devolucao_item" | "vale" => "Troca",
        "ordem_servico" | "ordem_servico_item" => "Ordem de serviço",
        "compra" | "compra_item" => "Compra",
        _ => "Registro",
    }
}

/// Um conflito para a lista do gerente.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflito {
    pub id: i64,
    pub tipo: String,
    pub o_que: String,
    pub texto: String,
    pub quando: String,
    pub caixa: String,
    /// O que mudou: campo, valor que ficou, valor que perdeu.
    pub campos: Vec<Value>,
    pub pode_usar: bool,
    pub resolvido: String,
}

/// Nome do campo para a tela.
fn nome_do_campo(campo: &str) -> String {
    let n = match campo {
        "nome" => "Nome",
        "preco" => "Preço",
        "custo" => "Custo",
        "lucro" => "Lucro",
        "codigo" => "Código",
        "codigo_barras" => "Código de barras",
        "codigo_original" => "Código original",
        "marca" => "Marca",
        "unidade" => "Unidade",
        "local" => "Local",
        "ativo" => "Ativo",
        "estoque_minimo" => "Estoque mínimo",
        "desconto_maximo" => "Desconto máximo",
        "observacoes" => "Observações",
        "telefone" => "Telefone",
        "email" => "E-mail",
        "documento" => "CPF/CNPJ",
        "limite_credito" => "Limite de crédito",
        "gerente" => "Gerente",
        "senha_hash" => "Senha",
        "valor" => "Valor",
        "situacao" => "Situação",
        "(apagado)" => "Apagado",
        outro => return outro.replace('_', " "),
    };
    n.into()
}

/// Colunas em centavos.
const DINHEIRO: &[&str] =
    &["preco", "custo", "valor", "total", "subtotal", "desconto", "limite_credito", "saldo_inicial", "oferta_preco"];

fn valor_curto(v: Option<&Value>, campo: &str) -> String {
    if campo == "senha_hash" {
        return "(senha)".into();
    }
    if let (true, Some(n)) = (DINHEIRO.contains(&campo), v.and_then(|x| x.as_i64())) {
        return format!("R$ {}", crate::numeros::escrever_dinheiro(n));
    }
    if campo == "lucro" || campo == "desconto_maximo" {
        if let Some(n) = v.and_then(|x| x.as_i64()) {
            return format!("{}%", crate::numeros::escrever_percentual(n));
        }
    }
    match v {
        None | Some(Value::Null) => "—".into(),
        Some(Value::String(t)) => t.chars().take(80).collect(),
        Some(outro) => outro.to_string().chars().take(80).collect(),
    }
}

/// Conflitos abertos (e os últimos resolvidos), mais novos primeiro.
pub fn conflitos(conn: &Connection) -> Resultado<Vec<Conflito>> {
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = {
        let mut c = conn.prepare(
            "SELECT id, tipo, tabela, chave, ficou, perdeu, quando, dados_perdeu, resolucao,
                    (SELECT nome FROM sync_par p WHERE p.dispositivo = k.dispositivo)
               FROM sync_conflito k
              WHERE resolvido_em IS NULL OR id > (SELECT COALESCE(MAX(id), 0) - 30 FROM sync_conflito)
              ORDER BY resolvido_em IS NOT NULL, id DESC LIMIT 200",
        )?;
        let l = c.query_map([], |l| {
            Ok((
                l.get(0)?,
                l.get(1)?,
                l.get(2)?,
                l.get(3)?,
                l.get(4)?,
                l.get(5)?,
                l.get(6)?,
                l.get(7)?,
                l.get(8)?,
                l.get(9)?,
            ))
        })?;
        l.collect::<Result<_, _>>()?
    };
    let mut lista = Vec::new();
    for (id, tipo, tabela, chave, _ficou, perdeu, quando, dados_perdeu, resolucao, caixa) in linhas {
        let mut k = Conflito {
            id,
            tipo: tipo.clone(),
            o_que: nome_da_tabela(&tabela).into(),
            texto: String::new(),
            quando: crate::fiscal::data_local_br(conn, &quando).unwrap_or(quando.clone()),
            caixa: caixa.unwrap_or_default(),
            campos: Vec::new(),
            pode_usar: false,
            resolvido: resolucao.unwrap_or_default(),
        };
        match tipo.as_str() {
            "orcamento" => {
                let mut c = conn.prepare(
                    "SELECT numero, caixa_nome FROM venda WHERE id IN (SELECT value FROM json_each('[\"' || replace(?1, ',', '\",\"') || '\"]')) ORDER BY id",
                )?;
                let vendas: Vec<String> = c
                    .query_map([&perdeu], |l| Ok(format!("Nº {} ({})", l.get::<_, i64>(0)?, l.get::<_, String>(1)?)))?
                    .collect::<Result<_, _>>()?;
                let numero: Option<i64> =
                    conn.query_row("SELECT numero FROM venda WHERE id = ?1", [&chave], |l| l.get(0)).optional()?;
                k.o_que = "Orçamento".into();
                k.texto = format!(
                    "O orçamento Nº {} virou mais de uma venda em caixas sem rede: {}. Cancele a venda que sobrou (se o cliente pagou uma vez só).",
                    numero.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                    vendas.join(" e ")
                );
            }
            "gerente" => {
                let nome: String = conn
                    .query_row("SELECT nome FROM usuario WHERE id = ?1", [&chave], |l| l.get(0))
                    .optional()?
                    .unwrap_or_default();
                k.o_que = "Gerente".into();
                k.texto = format!(
                    "Depois da troca de dados a loja ficou sem gerente (cada caixa tirou um). {nome} voltou a ser gerente. Confira na lista acima quem deve ser gerente."
                );
            }
            _ => {
                let atual: Option<Option<String>> = conn
                    .query_row(
                        "SELECT dados FROM alteracao WHERE tabela = ?1 AND chave = ?2 ORDER BY id DESC LIMIT 1",
                        params![tabela, chave],
                        |l| l.get(0),
                    )
                    .optional()?;
                let atual = atual.flatten();
                let ler = |t: &Option<String>| -> Value {
                    t.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null)
                };
                let (a, p) = (ler(&atual), ler(&dados_perdeu));
                let nome = a.get("nome").or_else(|| p.get("nome")).and_then(|v| v.as_str()).unwrap_or_default();
                k.texto = if nome.is_empty() {
                    "Dois caixas mexeram no mesmo registro sem ver o do outro. Ficou a alteração mais nova.".into()
                } else {
                    format!("Dois caixas mexeram em \"{nome}\" sem ver o do outro. Ficou a alteração mais nova.")
                };
                for campo in diferencas(atual.as_deref(), dados_perdeu.as_deref()) {
                    k.campos.push(json!({
                        "campo": campo,
                        "nome": nome_do_campo(&campo),
                        "ficou": if atual.is_none() { "(apagado)".into() } else { valor_curto(a.get(&campo), &campo) },
                        "perdeu": if dados_perdeu.is_none() { "(apagado)".into() } else { valor_curto(p.get(&campo), &campo) },
                    }));
                }
                k.pode_usar = PODE_USAR.contains(&tabela.as_str());
            }
        }
        lista.push(k);
    }
    Ok(lista)
}

/// Quantos conflitos estão esperando o gerente.
pub fn conflitos_abertos(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM sync_conflito WHERE resolvido_em IS NULL", [], |l| l.get(0))?)
}

/// O gerente decidiu: `usar` = volta a versão que tinha perdido (vira uma
/// alteração nova, que vai para os outros caixas); senão fica como está.
pub fn resolver_conflito(conn: &Connection, id: i64, usar: bool, usuario_id: &str) -> Resultado<()> {
    let (tipo, tabela, chave, dados, resolvido): (String, String, String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT tipo, tabela, chave, dados_perdeu, resolvido_em FROM sync_conflito WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Conflito".into()))?;
    if resolvido.is_some() {
        return Err(Erro::validacao("Este conflito já foi resolvido."));
    }
    if usar {
        if tipo != "registro" || !PODE_USAR.contains(&tabela.as_str()) {
            return Err(Erro::validacao("Aqui não dá para voltar a outra versão: corrija pela tela de cada coisa."));
        }
        let (todas, pk) = colunas(conn, &tabela)?;
        let valores_chave: Vec<Value> = serde_json::from_str(&chave)?;
        if valores_chave.len() != pk.len() {
            return Err(Erro::Interno("chave do conflito inválida".into()));
        }
        match dados.as_deref().map(serde_json::from_str::<Value>) {
            None => {
                let onde = pk
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("\"{c}\" = ?{}", i + 1))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                conn.execute(
                    &format!("DELETE FROM \"{tabela}\" WHERE {onde}"),
                    params_from_iter(valores_chave.iter().map(para_sql)),
                )?;
            }
            Some(Ok(Value::Object(m))) => {
                let cols: Vec<&String> =
                    todas.iter().filter(|c| m.contains_key(c.as_str()) && c.as_str() != "atualizado_em").collect();
                let mut nomes: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
                let mut valores: Vec<SqlValor> = cols.iter().map(|c| para_sql(&m[c.as_str()])).collect();
                if todas.iter().any(|c| c == "atualizado_em") {
                    nomes.push("\"atualizado_em\"".into());
                    valores.push(SqlValor::Text(tempo::agora()));
                }
                let marcas = (1..=nomes.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
                let atualizar: Vec<String> = nomes
                    .iter()
                    .filter(|n| !pk.iter().any(|k| format!("\"{k}\"") == **n))
                    .map(|n| format!("{n} = excluded.{n}"))
                    .collect();
                let conflito = if atualizar.is_empty() {
                    "DO NOTHING".to_string()
                } else {
                    format!("DO UPDATE SET {}", atualizar.join(", "))
                };
                conn.execute(
                    &format!(
                        "INSERT INTO \"{tabela}\" ({}) VALUES ({marcas}) ON CONFLICT ({}) {conflito}",
                        nomes.join(", "),
                        pk.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")
                    ),
                    params_from_iter(valores),
                )
                .map_err(|e| match e {
                    rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                        Erro::validacao("A versão que perdeu aponta para algo que não existe mais (ou repete outro registro). Corrija pela tela.")
                    }
                    outro => outro.into(),
                })?;
            }
            _ => return Err(Erro::Interno("versão do conflito inválida".into())),
        }
    }
    conn.execute(
        "UPDATE sync_conflito SET resolvido_em = ?2, resolvido_por = ?3, resolucao = ?4 WHERE id = ?1",
        params![
            id,
            tempo::agora(),
            usuario_id,
            if usar { "voltou a versão que tinha perdido" } else { "ficou como está" }
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Criptografia e mensagens.
// ---------------------------------------------------------------------------

/// Chaves da rede: a deste caixa (com o sal dele) e as dos outros, tiradas
/// da senha da rede por Argon2id e guardadas na memória (calcular demora).
#[derive(Clone)]
struct Chaves {
    senha: String,
    sal: [u8; 16],
    propria: [u8; 32],
}

/// (resumo da senha, sal) → chave.
type CacheChaves = Mutex<HashMap<([u8; 32], [u8; 16]), [u8; 32]>>;

fn cache() -> &'static CacheChaves {
    static CACHE: std::sync::OnceLock<CacheChaves> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn derivar(senha: &str, sal: &[u8; 16]) -> Resultado<[u8; 32]> {
    let id: [u8; 32] = Sha256::digest(senha.as_bytes()).into();
    if let Some(k) = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&(id, *sal)) {
        return Ok(*k);
    }
    let mut k = [0u8; 32];
    crate::usuarios::argon2()
        .hash_password_into(senha.as_bytes(), sal, &mut k)
        .map_err(|e| Erro::Interno(format!("não foi possível proteger a rede: {e}")))?;
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    if c.len() > 64 {
        c.clear();
    }
    c.insert((id, *sal), k);
    Ok(k)
}

impl Chaves {
    fn de(conn: &Connection, senha: &str) -> Resultado<Chaves> {
        let sal =
            match config::ler(conn, "sync_sal")?.and_then(|t| de_hex(&t)).and_then(|v| <[u8; 16]>::try_from(v).ok()) {
                Some(s) => s,
                None => {
                    let mut s = [0u8; 16];
                    getrandom::fill(&mut s).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
                    config::gravar(conn, "sync_sal", &s.iter().map(|b| format!("{b:02x}")).collect::<String>())?;
                    s
                }
            };
        Ok(Chaves { senha: senha.to_string(), sal, propria: derivar(senha, &sal)? })
    }
}

fn de_hex(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len()).step_by(2).map(|i| u8::from_str_radix(t.get(i..i + 2)?, 16).ok()).collect()
}

/// Mensagem: "RST2", sal de quem mandou (16), nonce (12), texto cifrado.
fn cifrar(chaves: &Chaves, dados: &[u8]) -> Resultado<Vec<u8>> {
    let cifra =
        ChaCha20Poly1305::new_from_slice(&chaves.propria).map_err(|_| Erro::Interno("chave inválida".into()))?;
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
    let mut saida = MAGICA.to_vec();
    saida.extend(chaves.sal);
    saida.extend(nonce);
    let corpo = cifra.encrypt(&nonce.into(), dados).map_err(|_| Erro::Interno("falha ao criptografar".into()))?;
    saida.extend(corpo);
    Ok(saida)
}

/// Erro quando o outro caixa fala outra versão (ou a senha não confere).
fn decifrar(chaves: &Chaves, dados: &[u8]) -> Result<Vec<u8>, &'static str> {
    if dados.len() < 4 + 16 + 12 + 16 || &dados[..4] != MAGICA {
        return Err("O outro caixa está com outra versão do Rustinho. Atualize todos os caixas para a mesma versão.");
    }
    let mut sal = [0u8; 16];
    sal.copy_from_slice(&dados[4..20]);
    let chave =
        if sal == chaves.sal { chaves.propria } else { derivar(&chaves.senha, &sal).map_err(|_| "chave inválida")? };
    let cifra = ChaCha20Poly1305::new_from_slice(&chave).map_err(|_| "chave inválida")?;
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&dados[20..32]);
    cifra.decrypt(&nonce.into(), &dados[32..]).map_err(|_| "A senha da rede não confere com a deste caixa.")
}

fn enviar(fluxo: &mut TcpStream, chaves: &Chaves, v: &Value) -> Resultado<()> {
    let corpo = cifrar(chaves, v.to_string().as_bytes())?;
    let tam = (corpo.len() as u32).to_be_bytes();
    fluxo.write_all(&tam).and_then(|_| fluxo.write_all(&corpo)).map_err(rede)?;
    Ok(())
}

fn receber(fluxo: &mut TcpStream, chaves: &Chaves, limite: usize) -> Resultado<Value> {
    let mut tam = [0u8; 4];
    fluxo.read_exact(&mut tam).map_err(rede)?;
    let tam = u32::from_be_bytes(tam) as usize;
    if tam > limite {
        return Err(Erro::validacao("Mensagem grande demais."));
    }
    let mut corpo = vec![0u8; tam];
    fluxo.read_exact(&mut corpo).map_err(rede)?;
    let claro = decifrar(chaves, &corpo).map_err(Erro::validacao)?;
    Ok(serde_json::from_slice(&claro)?)
}

fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn rede(e: std::io::Error) -> Erro {
    Erro::validacao(format!("Falha na rede: {e}"))
}

// ---------------------------------------------------------------------------
// Configuração e estado.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Ajustes {
    pub ligado: bool,
    pub senha: String,
    /// Endereços digitados (ex.: "192.168.0.12").
    pub enderecos: Vec<String>,
    pub dispositivo: String,
    pub nome: String,
}

pub fn ajustes(conn: &Connection) -> Resultado<Ajustes> {
    Ok(Ajustes {
        ligado: config::ler(conn, "sync_ligado")?.as_deref() == Some("1"),
        senha: config::ler(conn, "sync_senha")?.unwrap_or_default(),
        enderecos: config::ler(conn, "sync_enderecos")?
            .unwrap_or_default()
            .split([',', ' ', ';', '\n'])
            .filter(|e| !e.trim().is_empty())
            .map(|e| e.trim().to_string())
            .collect(),
        dispositivo: config::ler(conn, "dispositivo_id")?.unwrap_or_default(),
        nome: config::nome_caixa(conn)?,
    })
}

/// Grava as configurações da sincronização (só deste computador).
pub fn salvar_ajustes(conn: &Connection, ligado: bool, senha: &str, enderecos: &str) -> Resultado<()> {
    let senha = senha.trim();
    if ligado && senha.chars().count() < 8 {
        return Err(Erro::campo(
            "senha",
            "A senha da rede precisa ter pelo menos 8 caracteres. Use a mesma em todos os caixas.",
        ));
    }
    for e in enderecos.split([',', ' ', ';', '\n']).filter(|e| !e.trim().is_empty()) {
        let e = e.trim();
        let ok = e.parse::<std::net::IpAddr>().is_ok() || e.parse::<SocketAddr>().is_ok();
        if !ok {
            return Err(Erro::campo("enderecos", format!("Endereço inválido: {e} (ex.: 192.168.0.12)")));
        }
    }
    config::gravar(conn, "sync_ligado", if ligado { "1" } else { "0" })?;
    config::gravar(conn, "sync_senha", senha)?;
    config::gravar(conn, "sync_enderecos", enderecos.trim())?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Par {
    pub dispositivo: String,
    pub nome: String,
    pub endereco: String,
    pub visto_em: String,
    pub sincronizado_em: String,
    pub erro: String,
}

pub fn pares(conn: &Connection) -> Resultado<Vec<Par>> {
    let mut c = conn.prepare(
        "SELECT dispositivo, nome, endereco, COALESCE(visto_em, ''), COALESCE(sincronizado_em, ''), erro
           FROM sync_par ORDER BY nome",
    )?;
    let linhas = c.query_map([], |l| {
        Ok(Par {
            dispositivo: l.get(0)?,
            nome: l.get(1)?,
            endereco: l.get(2)?,
            visto_em: l.get(3)?,
            sincronizado_em: l.get(4)?,
            erro: l.get(5)?,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

fn registrar_par(conn: &Connection, dispositivo: &str, nome: &str, endereco: &str) -> Resultado<()> {
    conn.execute(
        "INSERT INTO sync_par (dispositivo, nome, endereco, visto_em) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (dispositivo) DO UPDATE SET nome = excluded.nome, endereco = excluded.endereco, visto_em = excluded.visto_em",
        params![dispositivo, nome, endereco, tempo::agora()],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Servidor, descoberta e puxada.
// ---------------------------------------------------------------------------

fn abrir_conexao(caminho: &Path) -> Resultado<Connection> {
    let conn = Connection::open(caminho)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    banco::registrar_funcoes(&conn)?;
    Ok(conn)
}

/// Atende um pedido de outro caixa.
fn atender(caminho: &Path, mut fluxo: TcpStream) -> Resultado<()> {
    // Quem não manda nada em 5 s é desligado.
    fluxo.set_read_timeout(Some(Duration::from_secs(5))).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    let conn = abrir_conexao(caminho)?;
    let a = ajustes(&conn)?;
    if !a.ligado || a.senha.is_empty() {
        return Ok(());
    }
    let k = Chaves::de(&conn, &a.senha)?;
    let pedido = receber(&mut fluxo, &k, TAMANHO_PEDIDO)?;
    if pedido["tipo"] != "puxar" {
        return Ok(());
    }
    // Quem pediu também vira um par conhecido (com o endereço de onde veio).
    if let (Some(d), Ok(origem)) = (pedido["dispositivo"].as_str(), fluxo.peer_addr()) {
        let porta = pedido["porta"].as_u64().unwrap_or(PORTA_TCP as u64);
        if d != a.dispositivo {
            registrar_par(&conn, d, pedido["nome"].as_str().unwrap_or(""), &format!("{}:{porta}", origem.ip()))?;
        }
    }
    let desde = pedido["desde"].as_i64().unwrap_or(0);
    let (lista, tem_mais) = alteracoes_desde(&conn, desde, LOTE)?;
    enviar(
        &mut fluxo,
        &k,
        &json!({ "dispositivo": a.dispositivo, "nome": a.nome, "alteracoes": lista, "temMais": tem_mais, "agoraMs": agora_ms() }),
    )
}

/// Puxa de um endereço tudo o que há de novo. Devolve (dispositivo, quantas novas).
pub fn puxar(caminho: &Path, endereco: &str, porta_local: u16) -> Resultado<(String, usize)> {
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    let mut conn = abrir_conexao(caminho)?;
    let a = ajustes(&conn)?;
    let k = Chaves::de(&conn, &a.senha)?;
    let alvo: SocketAddr = match endereco.parse::<SocketAddr>() {
        Ok(s) => s,
        Err(_) => format!("{endereco}:{PORTA_TCP}").parse().map_err(|_| Erro::validacao("Endereço inválido."))?,
    };
    let mut total = 0;
    let mut dispositivo = String::new();
    loop {
        let cursor: i64 = if dispositivo.is_empty() {
            conn.query_row("SELECT cursor FROM sync_par WHERE endereco = ?1", [alvo.to_string()], |l| l.get(0))
                .optional()?
                .unwrap_or(0)
        } else {
            conn.query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [&dispositivo], |l| l.get(0))
                .optional()?
                .unwrap_or(0)
        };
        let mut fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).map_err(rede)?;
        fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok();
        fluxo.set_write_timeout(Some(Duration::from_secs(10))).ok();
        let saiu = agora_ms();
        enviar(
            &mut fluxo,
            &k,
            &json!({ "tipo": "puxar", "desde": cursor, "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_local }),
        )?;
        let resposta = receber(&mut fluxo, &k, TAMANHO_MAXIMO)?;
        let voltou = agora_ms();
        dispositivo = resposta["dispositivo"].as_str().unwrap_or_default().to_string();
        if dispositivo.is_empty() || dispositivo == a.dispositivo {
            return Err(Erro::validacao("Este endereço é deste mesmo caixa."));
        }
        registrar_par(&conn, &dispositivo, resposta["nome"].as_str().unwrap_or(""), &alvo.to_string())?;
        let lista: Vec<Alteracao> = serde_json::from_value(resposta["alteracoes"].clone())?;
        // Se o outro caixa foi reinstalado (banco novo), o contador volta do zero.
        let maior = lista.iter().map(|x| x.seq).max();
        total += aplicar(&mut conn, &lista)?;
        if let Some(m) = maior {
            // Só avança se ninguém zerou o cursor no meio (restauração de backup).
            conn.execute(
                "UPDATE sync_par SET cursor = ?2 WHERE dispositivo = ?1 AND cursor = ?3",
                params![dispositivo, m, cursor],
            )?;
        }
        // Relógio do outro caixa (descontando o tempo da viagem pela rede).
        let aviso = match resposta["agoraMs"].as_i64() {
            Some(dele) => {
                let diferenca = dele - (saiu + voltou) / 2;
                if diferenca.abs() > RELOGIO_TOLERANCIA_MS {
                    let minutos = (diferenca.abs() + 30_000) / 60_000;
                    format!(
                        "O relógio dele está {minutos} min {} em relação a este. Acerte a data e a hora dos dois.",
                        if diferenca > 0 { "adiantado" } else { "atrasado" }
                    )
                } else {
                    String::new()
                }
            }
            None => String::new(),
        };
        conn.execute(
            "UPDATE sync_par SET sincronizado_em = ?2, erro = ?3 WHERE dispositivo = ?1",
            params![dispositivo, tempo::agora(), aviso],
        )?;
        if !resposta["temMais"].as_bool().unwrap_or(false) {
            break;
        }
    }
    // Depois de tudo: algum registro ficou apontando para outro que não veio?
    let faltam = pendencias(&conn)?;
    config::gravar(&conn, "sync_pendencias", &faltam.to_string())?;
    if total > 0 {
        crate::usuarios::resolver_nomes_repetidos(&conn)?;
        verificar_depois(&conn)?;
    }
    Ok((dispositivo, total))
}

/// O motor da sincronização: três tarefas em segundo plano (servidor,
/// anúncio/escuta na rede e puxada periódica). Fica parado enquanto a
/// sincronização estiver desligada nas configurações.
pub struct Motor {
    parar: Arc<AtomicBool>,
}

impl Drop for Motor {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
    }
}

impl Motor {
    pub fn iniciar(caminho: PathBuf, porta_tcp: u16, porta_udp: u16) -> Motor {
        let parar = Arc::new(AtomicBool::new(false));

        // 1. Servidor: atende os outros caixas.
        {
            let caminho = caminho.clone();
            let parar = parar.clone();
            thread::spawn(move || {
                let Ok(ouvinte) = TcpListener::bind(("0.0.0.0", porta_tcp)) else { return };
                ouvinte.set_nonblocking(true).ok();
                while !parar.load(Ordering::Relaxed) {
                    match ouvinte.accept() {
                        Ok((fluxo, _)) => {
                            // Muitas conexões ao mesmo tempo: as de sobra são fechadas.
                            let Some(vaga) = Vaga::pegar() else {
                                drop(fluxo);
                                continue;
                            };
                            fluxo.set_nonblocking(false).ok();
                            let caminho = caminho.clone();
                            let tarefa = thread::Builder::new().name("sync-atender".into()).spawn(move || {
                                let _vaga = vaga;
                                if let Err(e) = atender(&caminho, fluxo) {
                                    if matches!(e, Erro::Banco(_) | Erro::Interno(_)) {
                                        crate::registro::anotar(&format!("sync atender: {e}"));
                                    }
                                }
                            });
                            // Sem tarefa nova, a vaga já voltou (ela foi junto e caiu).
                            if let Err(e) = tarefa {
                                crate::registro::anotar(&format!("sync: não abriu tarefa: {e}"));
                            }
                        }
                        Err(_) => thread::sleep(Duration::from_millis(200)),
                    }
                }
            });
        }

        // 2. Descoberta: anuncia este caixa e escuta os outros (UDP).
        {
            let caminho = caminho.clone();
            let parar = parar.clone();
            thread::spawn(move || {
                let Ok(socket) = UdpSocket::bind(("0.0.0.0", porta_udp)) else { return };
                socket.set_broadcast(true).ok();
                socket.set_read_timeout(Some(Duration::from_secs(1))).ok();
                let mut ultimo_anuncio = Instant::now() - Duration::from_secs(60);
                let mut buf = vec![0u8; 2048];
                while !parar.load(Ordering::Relaxed) {
                    let Ok(conn) = abrir_conexao(&caminho) else {
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    };
                    let Ok(a) = ajustes(&conn) else { continue };
                    if !a.ligado || a.senha.is_empty() {
                        thread::sleep(Duration::from_secs(3));
                        continue;
                    }
                    let Ok(k) = Chaves::de(&conn, &a.senha) else { continue };
                    if ultimo_anuncio.elapsed() > Duration::from_secs(10) {
                        let ola = json!({ "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_tcp });
                        if let Ok(pacote) = cifrar(&k, ola.to_string().as_bytes()) {
                            let _ = socket.send_to(&pacote, ("255.255.255.255", porta_udp));
                        }
                        ultimo_anuncio = Instant::now();
                    }
                    if let Ok((n, origem)) = socket.recv_from(&mut buf) {
                        if let Ok(claro) = decifrar(&k, &buf[..n]) {
                            if let Ok(v) = serde_json::from_slice::<Value>(&claro) {
                                let d = v["dispositivo"].as_str().unwrap_or_default();
                                let porta = v["porta"].as_u64().unwrap_or(PORTA_TCP as u64);
                                if !d.is_empty() && d != a.dispositivo {
                                    let _ = registrar_par(
                                        &conn,
                                        d,
                                        v["nome"].as_str().unwrap_or(""),
                                        &format!("{}:{porta}", origem.ip()),
                                    );
                                }
                            }
                        }
                    }
                }
            });
        }

        // 3. Puxada: a cada 4 segundos, pede o que há de novo a cada caixa.
        {
            let parar = parar.clone();
            thread::spawn(move || {
                let mut ultima_limpeza = Instant::now() - Duration::from_secs(50 * 60);
                while !parar.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(4));
                    let Ok(conn) = abrir_conexao(&caminho) else { continue };
                    if ultima_limpeza.elapsed() > Duration::from_secs(60 * 60) {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        let _ = compactar(&conn);
                        ultima_limpeza = Instant::now();
                    }
                    let Ok(a) = ajustes(&conn) else { continue };
                    if !a.ligado || a.senha.is_empty() {
                        continue;
                    }
                    let mut alvos: Vec<String> = pares(&conn)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|p| !p.endereco.is_empty())
                        .map(|p| p.endereco)
                        .collect();
                    for e in &a.enderecos {
                        let completo = if e.contains(':') { e.clone() } else { format!("{e}:{PORTA_TCP}") };
                        if !alvos.contains(&completo) {
                            alvos.push(completo);
                        }
                    }
                    drop(conn);
                    for alvo in alvos {
                        if let Err(e) = puxar(&caminho, &alvo, porta_tcp) {
                            if let Ok(conn) = abrir_conexao(&caminho) {
                                let _ = conn.execute(
                                    "UPDATE sync_par SET erro = ?2 WHERE endereco = ?1",
                                    params![alvo, e.to_string()],
                                );
                            }
                        }
                    }
                }
            });
        }

        Motor { parar }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{estoque, produtos, usuarios};

    fn temp(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-sync-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p.join("rustinho.db")
    }

    #[test]
    fn dois_caixas_pela_rede() {
        // Caixa 1: já tinha dados antes da sincronização.
        let c1 = temp("c1");
        let b1 = Banco::abrir(&c1).unwrap();
        let u = usuarios::criar(&b1.conn, &b1.dispositivo, "Maria", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&b1.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        f.estoque_inicial = "5".into();
        let p = produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f).unwrap();
        config::gravar(&b1.conn, "loja_nome", "Loja Modelo").unwrap();
        config::gravar(&b1.conn, "caixa_nome", "Caixa 1").unwrap();
        salvar_ajustes(&b1.conn, true, "senha-da-loja", "").unwrap();
        drop(b1);
        let b1 = Banco::abrir(&c1).unwrap(); // reabrir não duplica o registro inicial
        let n1: i64 = b1.conn.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap();
        drop(b1);
        let b1 = Banco::abrir(&c1).unwrap();
        let n1b: i64 = b1.conn.query_row("SELECT COUNT(*) FROM alteracao", [], |l| l.get(0)).unwrap();
        assert_eq!(n1, n1b);

        // Caixa 2: novo, entra na rede da loja.
        let c2 = temp("c2");
        let b2 = Banco::abrir(&c2).unwrap();
        config::gravar(&b2.conn, "caixa_nome", "Caixa 2").unwrap();
        salvar_ajustes(&b2.conn, true, "senha-da-loja", "").unwrap();

        let (pt1, pu1) = (47910, 47911);
        let (pt2, pu2) = (47920, 47921);
        let _m1 = Motor::iniciar(c1.clone(), pt1, pu1);
        let _m2 = Motor::iniciar(c2.clone(), pt2, pu2);
        thread::sleep(Duration::from_millis(400));

        // 2 puxa de 1
        let (_, novas) = puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap();
        assert!(novas > 0);
        assert_eq!(usuarios::quantidade(&b2.conn).unwrap(), 1, "o usuário chegou");
        assert_eq!(config::nome_loja(&b2.conn).unwrap(), "Loja Modelo");
        assert_eq!(config::nome_caixa(&b2.conn).unwrap(), "Caixa 2", "o nome do caixa não é copiado");
        assert_eq!(estoque::saldo(&b2.conn, &p).unwrap(), 5000);
        assert_eq!(produtos::ficha(&b2.conn, &p).unwrap().precos[0].preco, "80,00");

        // venda no caixa 2 (saída de estoque) e preço mudado no caixa 1
        estoque::lancar(&b2.conn, &b2.dispositivo, Some(&u.id), &p, -2000, "venda", "Venda 1 · Caixa 2").unwrap();
        let mut f1 = produtos::ficha(&b1.conn, &p).unwrap();
        f1.precos[0].preco = "85,00".into();
        f1.precos[0].lucro = String::new();
        produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f1).unwrap();

        puxar(&c1, &format!("127.0.0.1:{pt2}"), pt1).unwrap();
        puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap();
        assert_eq!(estoque::saldo(&b1.conn, &p).unwrap(), 3000);
        assert_eq!(estoque::saldo(&b2.conn, &p).unwrap(), 3000);
        assert_eq!(produtos::ficha(&b2.conn, &p).unwrap().precos[0].preco, "85,00");

        // puxar de novo não traz nada novo (nem fica em eco)
        assert_eq!(puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap().1, 0);
        assert_eq!(puxar(&c1, &format!("127.0.0.1:{pt2}"), pt1).unwrap().1, 0);

        // alteração mais nova vence; a mais velha não desfaz
        let mut f2 = produtos::ficha(&b2.conn, &p).unwrap();
        f2.nome = "Pastilha dianteira".into();
        produtos::salvar(&b2.conn, &b2.dispositivo, &u.id, &f2).unwrap();
        thread::sleep(Duration::from_millis(5));
        let mut f1 = produtos::ficha(&b1.conn, &p).unwrap();
        f1.nome = "Pastilha de freio dianteira".into();
        produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f1).unwrap();
        puxar(&c1, &format!("127.0.0.1:{pt2}"), pt1).unwrap();
        puxar(&c2, &format!("127.0.0.1:{pt1}"), pt2).unwrap();
        assert_eq!(produtos::ficha(&b1.conn, &p).unwrap().nome, "Pastilha de freio dianteira");
        assert_eq!(produtos::ficha(&b2.conn, &p).unwrap().nome, "Pastilha de freio dianteira");

        // senha errada: recusa
        let c3 = temp("c3");
        let b3 = Banco::abrir(&c3).unwrap();
        salvar_ajustes(&b3.conn, true, "outra-senha-qualquer", "").unwrap();
        let e = puxar(&c3, &format!("127.0.0.1:{pt1}"), 1).unwrap_err();
        assert!(e.to_string().contains("Falha na rede") || e.to_string().contains("senha"), "{e}");
        assert_eq!(usuarios::quantidade(&b3.conn).unwrap(), 0);

        // o caixa 1 conheceu o caixa 2 quando foi procurado por ele
        assert!(pares(&b1.conn).unwrap().iter().any(|x| x.nome == "Caixa 2"));
    }

    #[test]
    fn mensagens_do_protocolo_2() {
        let b = Banco::em_memoria().unwrap();
        let k1 = Chaves::de(&b.conn, "senha-da-loja").unwrap();
        // Outro caixa: mesmo senha, outro sal.
        let k2 = Chaves {
            senha: "senha-da-loja".into(),
            sal: [9u8; 16],
            propria: derivar("senha-da-loja", &[9u8; 16]).unwrap(),
        };
        assert_ne!(k1.propria, k2.propria);
        let m = cifrar(&k2, b"ola").unwrap();
        assert_eq!(decifrar(&k1, &m).unwrap(), b"ola");
        // Senha errada.
        let k3 = Chaves {
            senha: "outra-senha".into(),
            sal: [9u8; 16],
            propria: derivar("outra-senha", &[9u8; 16]).unwrap(),
        };
        assert!(decifrar(&k3, &m).unwrap_err().contains("senha"));
        // Versão antiga (sem "RST2").
        assert!(decifrar(&k1, &[0u8; 80]).unwrap_err().contains("versão"));
        // O sal deste caixa fica guardado.
        assert_eq!(Chaves::de(&b.conn, "senha-da-loja").unwrap().sal, k1.sal);
    }

    #[test]
    fn compactar_e_conflito() {
        let mut b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        config::gravar(&b.conn, "sync_inicio", "x").unwrap();
        for nome in ["Maria A", "Maria B", "Maria C"] {
            usuarios::alterar(&b.conn, &u.id, nome, true, None).unwrap();
        }
        let antes: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM alteracao WHERE tabela = 'usuario'", [], |l| l.get(0)).unwrap();
        assert!(antes >= 4);
        assert!(compactar(&b.conn).unwrap() >= 3);
        let depois: i64 =
            b.conn.query_row("SELECT COUNT(*) FROM alteracao WHERE tabela = 'usuario'", [], |l| l.get(0)).unwrap();
        assert_eq!(depois, 1);
        // Uma alteração mais velha de outro caixa chega depois: não vale e fica anotada.
        let (id_ultima, chave_u, dados): (String, String, String) = b
            .conn
            .query_row("SELECT id, chave, dados FROM alteracao WHERE tabela = 'usuario'", [], |l| {
                Ok((l.get(0)?, l.get(1)?, l.get(2)?))
            })
            .unwrap();
        // Feita 1 segundo antes da nossa, em outro caixa.
        let ms = ms_do_id(&id_ultima).unwrap() - 1000;
        let h = format!("{ms:012x}");
        let velha = Alteracao {
            seq: 1,
            id: format!("{}-{}-7000-8000-000000000000", &h[..8], &h[8..12]),
            tabela: "usuario".into(),
            chave: chave_u,
            operacao: "gravar".into(),
            dados: Some(dados.replace("Maria C", "Maria velha")),
            dispositivo: "outro".into(),
            quando: tempo::agora(),
            base: None,
        };
        assert!(velha.id < id_ultima);
        let velha_clone = velha.clone();
        aplicar(&mut b.conn, &[velha]).unwrap();
        assert_eq!(usuarios::buscar(&b.conn, &u.id).unwrap().nome, "Maria C");
        let conflitos: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(conflitos, 1);
        // Uma versão bem velha (de antes de uma restauração) não é conflito.
        let mut antiga = velha_clone;
        antiga.id = "00000000-0000-7000-8000-000000000001".into();
        aplicar(&mut b.conn, &[antiga]).unwrap();
        let conflitos: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(conflitos, 1);
        assert_eq!(pendencias(&b.conn).unwrap(), 0);
    }

    /// C-01: a vaga volta mesmo se a tarefa der pânico, e nunca passa do teto.
    #[test]
    fn vaga_volta_depois_de_panico() {
        static CONTADOR: AtomicUsize = AtomicUsize::new(0);
        let vagas: Vec<Vaga> = (0..CONEXOES_MAXIMAS).filter_map(|_| Vaga::pegar_de(&CONTADOR)).collect();
        assert_eq!(vagas.len(), CONEXOES_MAXIMAS);
        assert!(Vaga::pegar_de(&CONTADOR).is_none(), "não passa do teto");
        let mut vagas = vagas.into_iter();
        let uma = vagas.next().unwrap();
        let r = thread::spawn(move || {
            let _v = uma;
            panic!("defeito de teste");
        })
        .join();
        assert!(r.is_err());
        let outra = Vaga::pegar_de(&CONTADOR).expect("a vaga voltou depois do pânico");
        drop(outra);
        drop(vagas);
        assert_eq!(CONTADOR.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn senha_curta_recusada() {
        let b = Banco::em_memoria().unwrap();
        assert!(salvar_ajustes(&b.conn, true, "123", "").is_err());
        assert!(salvar_ajustes(&b.conn, true, "12345678", "192.168.0.300").is_err());
        salvar_ajustes(&b.conn, true, "12345678", "192.168.0.12, 192.168.0.13").unwrap();
        assert_eq!(ajustes(&b.conn).unwrap().enderecos.len(), 2);
    }
}
