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
//! - Tudo que passa pela rede vai criptografado (ChaCha20-Poly1305) com a
//!   "senha da rede" da loja, que precisa ser a mesma nos caixas. Sem a
//!   senha certa, nada é lido nem aceito.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
const TAMANHO_MAXIMO: usize = 64 * 1024 * 1024;

/// Tabelas que os caixas compartilham.
pub const TABELAS: &[&str] = &[
    "config",
    "usuario",
    "tabela_preco",
    "regra_st",
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
    "backup_senha",
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
    let chave = format!(
        "json_array({})",
        chave.iter().map(|c| format!("{prefixo}.\"{c}\"")).collect::<Vec<_>>().join(", ")
    );
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
        for (sufixo, evento, prefixo, operacao) in [("i", "INSERT", "NEW", "gravar"), ("u", "UPDATE", "NEW", "gravar"), ("d", "DELETE", "OLD", "apagar")] {
            let (k, d) = expressoes(tabela, &todas, &chave, prefixo);
            let dados = if operacao == "apagar" { "NULL".to_string() } else { d };
            let extra = if *tabela == "config" { filtro_config(prefixo) } else { String::new() };
            sql.push_str(&format!(
                "DROP TRIGGER IF EXISTS sync_{tabela}_{sufixo};
                 CREATE TRIGGER sync_{tabela}_{sufixo} AFTER {evento} ON \"{tabela}\" {quando}{extra}
                 BEGIN
                   INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando)
                   VALUES (rustinho_id(), '{tabela}', {k}, '{operacao}', {dados}, {disp}, rustinho_agora());
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
            let filtro = if *tabela == "config" { format!(" WHERE 1 = 1{}", filtro_config("t")) } else { String::new() };
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
}

/// Alterações deste banco depois da número `desde`.
pub fn alteracoes_desde(conn: &Connection, desde: i64, limite: i64) -> Resultado<(Vec<Alteracao>, bool)> {
    let mut c = conn.prepare_cached(
        "SELECT seq, id, tabela, chave, operacao, dados, dispositivo, quando FROM alteracao
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
    // sido apagada depois). As chaves estrangeiras ficam desligadas aqui.
    conn.pragma_update(None, "foreign_keys", false)?;
    let resultado = aplicar_dentro(conn, lista);
    conn.pragma_update(None, "foreign_keys", true)?;
    resultado
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
        let ja: bool = tx.query_row("SELECT 1 FROM alteracao WHERE id = ?1", [&a.id], |_| Ok(true)).optional()?.unwrap_or(false);
        if ja {
            continue;
        }
        let ultima: Option<String> = tx.query_row(
            "SELECT MAX(id) FROM alteracao WHERE tabela = ?1 AND chave = ?2",
            params![a.tabela, a.chave],
            |l| l.get(0),
        )?;
        tx.execute(
            "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![a.id, a.tabela, a.chave, a.operacao, a.dados, a.dispositivo, a.quando],
        )?;
        novas += 1;
        if ultima.as_deref().is_some_and(|u| u >= a.id.as_str()) {
            continue; // já temos uma alteração mais nova deste registro
        }
        if !colunas_cache.iter().any(|c| c.0 == a.tabela) {
            let (todas, chave) = colunas(&tx, &a.tabela)?;
            colunas_cache.push((a.tabela.clone(), todas, chave));
        }
        let (_, todas, chave) = colunas_cache.iter().find(|c| c.0 == a.tabela).expect("acabou de entrar");
        let valores_chave: Vec<Value> = serde_json::from_str(&a.chave)?;
        if valores_chave.len() != chave.len() {
            continue;
        }
        if a.operacao == "apagar" {
            let onde = chave.iter().enumerate().map(|(i, c)| format!("\"{c}\" = ?{}", i + 1)).collect::<Vec<_>>().join(" AND ");
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
    tx.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", [])?;
    tx.commit()?;
    Ok(novas)
}

// ---------------------------------------------------------------------------
// Criptografia e mensagens.
// ---------------------------------------------------------------------------

/// Chave de 32 bytes tirada da senha da rede.
fn chave(senha: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"rustinho-sync-v1:");
    h.update(senha.as_bytes());
    let r = h.finalize();
    let mut k = [0u8; 32];
    k.copy_from_slice(&r);
    k
}

fn cifrar(chave: &[u8; 32], dados: &[u8]) -> Resultado<Vec<u8>> {
    let cifra = ChaCha20Poly1305::new_from_slice(chave).map_err(|_| Erro::Interno("chave inválida".into()))?;
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| Erro::Interno(format!("sem números aleatórios: {e}")))?;
    let mut saida = nonce.to_vec();
    let corpo = cifra
        .encrypt(&nonce.into(), dados)
        .map_err(|_| Erro::Interno("falha ao criptografar".into()))?;
    saida.extend(corpo);
    Ok(saida)
}

fn decifrar(chave: &[u8; 32], dados: &[u8]) -> Option<Vec<u8>> {
    if dados.len() < 12 + 16 {
        return None;
    }
    let cifra = ChaCha20Poly1305::new_from_slice(chave).ok()?;
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&dados[..12]);
    cifra.decrypt(&nonce.into(), &dados[12..]).ok()
}

fn enviar(fluxo: &mut TcpStream, chave: &[u8; 32], v: &Value) -> Resultado<()> {
    let corpo = cifrar(chave, v.to_string().as_bytes())?;
    let tam = (corpo.len() as u32).to_be_bytes();
    fluxo.write_all(&tam).and_then(|_| fluxo.write_all(&corpo)).map_err(rede)?;
    Ok(())
}

fn receber(fluxo: &mut TcpStream, chave: &[u8; 32]) -> Resultado<Value> {
    let mut tam = [0u8; 4];
    fluxo.read_exact(&mut tam).map_err(rede)?;
    let tam = u32::from_be_bytes(tam) as usize;
    if tam > TAMANHO_MAXIMO {
        return Err(Erro::validacao("Mensagem grande demais."));
    }
    let mut corpo = vec![0u8; tam];
    fluxo.read_exact(&mut corpo).map_err(rede)?;
    let claro = decifrar(chave, &corpo).ok_or_else(|| Erro::validacao("A senha da rede não confere com a deste caixa."))?;
    Ok(serde_json::from_slice(&claro)?)
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
        return Err(Erro::campo("senha", "A senha da rede precisa ter pelo menos 8 caracteres. Use a mesma em todos os caixas."));
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
    fluxo.set_read_timeout(Some(Duration::from_secs(10))).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let conn = abrir_conexao(caminho)?;
    let a = ajustes(&conn)?;
    if !a.ligado || a.senha.is_empty() {
        return Ok(());
    }
    let k = chave(&a.senha);
    let pedido = receber(&mut fluxo, &k)?;
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
        &json!({ "dispositivo": a.dispositivo, "nome": a.nome, "alteracoes": lista, "temMais": tem_mais }),
    )
}

/// Puxa de um endereço tudo o que há de novo. Devolve (dispositivo, quantas novas).
pub fn puxar(caminho: &Path, endereco: &str, porta_local: u16) -> Resultado<(String, usize)> {
    let mut conn = abrir_conexao(caminho)?;
    let a = ajustes(&conn)?;
    let k = chave(&a.senha);
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
        enviar(
            &mut fluxo,
            &k,
            &json!({ "tipo": "puxar", "desde": cursor, "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_local }),
        )?;
        let resposta = receber(&mut fluxo, &k)?;
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
        conn.execute(
            "UPDATE sync_par SET sincronizado_em = ?2, erro = '' WHERE dispositivo = ?1",
            params![dispositivo, tempo::agora()],
        )?;
        if !resposta["temMais"].as_bool().unwrap_or(false) {
            break;
        }
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
                            fluxo.set_nonblocking(false).ok();
                            let caminho = caminho.clone();
                            thread::spawn(move || {
                                let _ = atender(&caminho, fluxo);
                            });
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
                    let k = chave(&a.senha);
                    if ultimo_anuncio.elapsed() > Duration::from_secs(10) {
                        let ola = json!({ "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_tcp });
                        if let Ok(pacote) = cifrar(&k, ola.to_string().as_bytes()) {
                            let _ = socket.send_to(&pacote, ("255.255.255.255", porta_udp));
                        }
                        ultimo_anuncio = Instant::now();
                    }
                    if let Ok((n, origem)) = socket.recv_from(&mut buf) {
                        if let Some(claro) = decifrar(&k, &buf[..n]) {
                            if let Ok(v) = serde_json::from_slice::<Value>(&claro) {
                                let d = v["dispositivo"].as_str().unwrap_or_default();
                                let porta = v["porta"].as_u64().unwrap_or(PORTA_TCP as u64);
                                if !d.is_empty() && d != a.dispositivo {
                                    let _ = registrar_par(&conn, d, v["nome"].as_str().unwrap_or(""), &format!("{}:{porta}", origem.ip()));
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
                while !parar.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(4));
                    let Ok(conn) = abrir_conexao(&caminho) else { continue };
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
    fn senha_curta_recusada() {
        let b = Banco::em_memoria().unwrap();
        assert!(salvar_ajustes(&b.conn, true, "123", "").is_err());
        assert!(salvar_ajustes(&b.conn, true, "12345678", "192.168.0.300").is_err());
        salvar_ajustes(&b.conn, true, "12345678", "192.168.0.12, 192.168.0.13").unwrap();
        assert_eq!(ajustes(&b.conn).unwrap().enderecos.len(), 2);
    }
}
