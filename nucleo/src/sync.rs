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
//! - Cada alteração guarda sobre qual versão foi feita ("base"), um contador
//!   do registro ("ordem": base + 1) e as versões anteriores que o caixa
//!   conhecia ("historia", até 8, só como atalho). Vale a de maior ordem (empate: maior id,
//!   o UUID v7). Uma alteração feita depois de outra sempre vale, mesmo com
//!   o relógio do caixa atrasado, e todos os caixas escolhem a mesma, em
//!   qualquer ordem de chegada (R-08).
//! - Duas alterações em que nenhuma está na história da outra foram feitas
//!   sem uma ver a outra: conflito. Se a história não basta para saber,
//!   também conta como conflito ("ordem incerta"). Nos dois casos as duas
//!   versões ficam guardadas e o gerente decide; a decisão vai para os
//!   outros caixas. Nada disso olha o relógio.
//! - Compactar o log só tira os dados das versões velhas (ficam id, base e
//!   ordem): a cadeia de versões continua inteira em todos os caixas. Uma
//!   alteração repetida (reenvio, backup restaurado) é reconhecida pelo id
//!   e não vira aviso de novo.
//! - O estoque é só soma de movimentos, então nunca há conflito nele.
//! - Tudo que passa pela rede vai criptografado (ChaCha20-Poly1305) com uma
//!   chave tirada da "senha da rede" da loja por Argon2id, com um "sal" de
//!   cada caixa que vai junto na mensagem (protocolo versão 2, desde a
//!   0.9.11). A senha precisa ser a mesma nos caixas: sem ela, nada é lido
//!   nem aceito.
//! - Registros repetidos no log de alterações (versões velhas de um mesmo
//!   registro) são apagados de tempos em tempos: só a mais nova importa.
//! - A diferença de relógio entre caixas aparece como aviso na tela dos
//!   caixas da loja (só para conferir; não decide nada).
//! - Protocolo 3 (desde o fim da 0.9.11): mensagens começam com "RST3".
//!   Caixa com outro protocolo é recusado antes de ler qualquer dado.

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
/// Começo de toda mensagem do protocolo 3 (ordem e história das alterações).
const MAGICA: &[u8; 4] = b"RST3";
/// Versão do protocolo, também dentro de cada pedido e resposta.
pub const PROTOCOLO: i64 = 3;
const PROTOCOLO_DIFERENTE: &str =
    "O outro caixa está com outra versão do Rustinho. Atualize todos os caixas para a mesma versão.";
/// Quantas versões anteriores cada alteração leva na história. É só um
/// atalho: a relação certa entre versões vem da cadeia de bases no log (as
/// versões compactadas continuam lá). Pouca coisa para não pesar no banco
/// (medido no teste `volume_do_historico`).
pub const HISTORIA_MAXIMA: usize = 8;
/// Marca no fim da história: ela não chega até o começo do registro.
const INCOMPLETA: &str = "+";

/// Enquanto um backup é restaurado, a troca de dados espera (e vice-versa).
static TRAVA: RwLock<()> = RwLock::new(());

/// Uma vaga de atendimento. Ao sair de cena (fim normal, erro ou pânico da
/// tarefa) a vaga volta (C-01): o contador nunca fica preso.
struct Vaga(Arc<AtomicUsize>);

impl Vaga {
    fn pegar_de(contador: &Arc<AtomicUsize>) -> Option<Vaga> {
        let mut atual = contador.load(Ordering::SeqCst);
        loop {
            if atual >= CONEXOES_MAXIMAS {
                return None;
            }
            match contador.compare_exchange(atual, atual + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Some(Vaga(contador.clone())),
                Err(outro) => atual = outro,
            }
        }
    }
}

/// Conexões atendidas ao mesmo tempo vindas de um mesmo endereço.
const VAGAS_POR_ORIGEM: usize = 2;
/// Prazo para quem conectou mandar o pedido (um caixa de verdade manda na
/// hora; o pedido é pequeno).
const PRAZO_DO_PEDIDO: Duration = Duration::from_secs(2);
/// Conexões que chegaram e ainda não mandaram nada (esperam numa fila, sem
/// tarefa e sem vaga; a mais antiga sai quando a fila enche).
const NA_FILA_MAXIMO: usize = 64;

/// Uma conexão sendo atendida pelo servidor.
struct Atendimento {
    ip: std::net::IpAddr,
    tarefa: thread::JoinHandle<()>,
}

/// Uma conexão na fila, esperando o pedido chegar.
struct NaFila {
    fluxo: TcpStream,
    ip: std::net::IpAddr,
    desde: Instant,
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
    // Códigos que uma peça já teve.
    "produto_codigo_antigo",
    // Faixas de códigos das peças de cada caixa (veja `faixas`).
    "faixa_codigo",
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
    // Decisões do gerente sobre conflitos (vão para todos os caixas).
    "sync_resolucao",
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
    // O caixa que dá as faixas de códigos das peças.
    "codigos_distribuidor",
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
        // A versão que vale agora deste registro (a base da nova).
        let ultima = |k: &str, coluna: &str| {
            format!(
                "(SELECT b.{coluna} FROM alteracao b WHERE b.tabela = '{tabela}' AND b.chave = {k} AND b.compactada = 0
                   ORDER BY b.ordem DESC, b.id DESC LIMIT 1)"
            )
        };
        let mut sql = String::new();
        for (sufixo, evento, prefixo, operacao) in
            [("i", "INSERT", "NEW", "gravar"), ("u", "UPDATE", "NEW", "gravar"), ("d", "DELETE", "OLD", "apagar")]
        {
            let (k, d) = expressoes(tabela, &todas, &chave, prefixo);
            let dados = if operacao == "apagar" { "NULL".to_string() } else { d };
            let extra = if *tabela == "config" { filtro_config(prefixo) } else { String::new() };
            let (base, ordem, historia, base_da_base) =
                (ultima(&k, "id"), ultima(&k, "ordem"), ultima(&k, "historia"), ultima(&k, "base"));
            sql.push_str(&format!(
                "DROP TRIGGER IF EXISTS sync_{tabela}_{sufixo};
                 CREATE TRIGGER sync_{tabela}_{sufixo} AFTER {evento} ON \"{tabela}\" {quando}{extra}
                 BEGIN
                   INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia)
                   VALUES (rustinho_id(), '{tabela}', {k}, '{operacao}', {dados}, {disp}, rustinho_agora(),
                           {base}, COALESCE({ordem}, 0) + 1, rustinho_historia({base}, {historia}, {base_da_base}));
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
    /// Contador do registro (base + 1). Vale a de maior ordem.
    #[serde(default)]
    pub ordem: i64,
    /// Versões anteriores conhecidas (JSON, a base primeiro; "+" no fim =
    /// não chega ao começo).
    #[serde(default)]
    pub historia: Option<String>,
    /// Versão velha sem os dados (o log foi compactado): serve só para a
    /// cadeia de versões; nunca é aplicada.
    #[serde(default)]
    pub compactada: bool,
}

const COLUNAS_ALTERACAO: &str =
    "seq, id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia, compactada";

fn ler_alteracao(l: &rusqlite::Row) -> rusqlite::Result<Alteracao> {
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
        ordem: l.get(9)?,
        historia: l.get(10)?,
        compactada: l.get::<_, i64>(11)? != 0,
    })
}

/// Alterações deste banco depois da número `desde`.
pub fn alteracoes_desde(conn: &Connection, desde: i64, limite: i64) -> Resultado<(Vec<Alteracao>, bool)> {
    let mut c = conn
        .prepare_cached(&format!("SELECT {COLUNAS_ALTERACAO} FROM alteracao WHERE seq > ?1 ORDER BY seq LIMIT ?2"))?;
    let lista: Vec<Alteracao> = c.query_map(params![desde, limite + 1], ler_alteracao)?.collect::<Result<_, _>>()?;
    let tem_mais = lista.len() as i64 > limite;
    Ok((lista.into_iter().take(limite as usize).collect(), tem_mais))
}

/// Resposta do distribuidor a um pedido de faixa de códigos das peças: as
/// alterações do log com as faixas (as mesmas que ele gravou), ou o motivo
/// da recusa.
pub fn responder_faixa(conn: &mut Connection, pedido: &Value) -> Resultado<Value> {
    let eu = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let dono = pedido["dispositivo"].as_str().unwrap_or_default();
    let geracao_dono = pedido["geracao"].as_str().unwrap_or_default();
    let nome_dono = pedido["nome"].as_str().unwrap_or_default();
    let tenho: Vec<i64> = pedido["tenho"]
        .as_array()
        .map(|l| l.iter().filter_map(|v| v.as_i64()).take(10_000).collect())
        .unwrap_or_default();
    Ok(match crate::faixas::atender_pedido(conn, dono, geracao_dono, nome_dono, &tenho) {
        Ok(chaves) => json!({ "protocolo": PROTOCOLO, "dispositivo": eu,
                              "alteracoes": alteracoes_das_faixas(conn, &chaves)? }),
        Err(Erro::Validacao { mensagem, .. }) => {
            json!({ "protocolo": PROTOCOLO, "dispositivo": eu, "recusado": mensagem })
        }
        Err(e) => return Err(e),
    })
}

/// O caixa que pediu grava as faixas que vieram (como alterações do log,
/// com o mesmo id). Devolve quantas eram novas.
pub fn gravar_faixa_recebida(conn: &mut Connection, resposta: &Value) -> Resultado<usize> {
    if resposta["protocolo"].as_i64() != Some(PROTOCOLO) {
        return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
    }
    if let Some(m) = resposta["recusado"].as_str() {
        return Err(Erro::validacao(m));
    }
    let lista: Vec<Alteracao> = serde_json::from_value(resposta["alteracoes"].clone())?;
    let lista: Vec<Alteracao> = lista.into_iter().filter(|x| x.tabela == "faixa_codigo").collect();
    aplicar(conn, &lista)
}

/// As alterações (com dados) das faixas de códigos pedidas, para a
/// resposta do pedido de faixa.
fn alteracoes_das_faixas(conn: &Connection, chaves: &[(i64, String, String)]) -> Resultado<Vec<Alteracao>> {
    let mut c = conn.prepare_cached(&format!(
        "SELECT {COLUNAS_ALTERACAO} FROM alteracao
          WHERE tabela = 'faixa_codigo' AND chave = json_array(?1, ?2, ?3) AND compactada = 0 ORDER BY seq"
    ))?;
    let mut lista = Vec::new();
    for (inicio, dono, geracao) in chaves {
        for a in c.query_map(params![inicio, dono, geracao], ler_alteracao)? {
            lista.push(a?);
        }
    }
    Ok(lista)
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

/// Quantos registros apontam para outro que ainda não chegou (ou foi
/// apagado). Normalmente é zero depois de trocar tudo com os outros caixas.
pub fn pendencias(conn: &Connection) -> Resultado<usize> {
    let mut c = conn.prepare("PRAGMA foreign_key_check")?;
    let n = c.query_map([], |_| Ok(()))?.count();
    Ok(n)
}

/// R-05: referências que não batem, separadas em dois tipos.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Referencias {
    /// Apontam para algo que ainda não chegou de outro caixa (normal no
    /// meio de uma troca; resolve sozinho quando o resto chegar).
    pub esperando: usize,
    /// Apontam para algo que foi apagado (aqui ou em outro caixa). Não
    /// resolve sozinho; nada é apagado para esconder: o gerente decide.
    pub quebradas: usize,
    /// Das que esperam, quantas esperam há 24 horas ou mais. É só um aviso:
    /// atraso não é inconsistência e não bloqueia nada.
    pub atrasadas: usize,
}

/// Uma referência que não bate, para a lista em "Caixas da loja".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Referencia {
    /// Tabela e chave (JSON) do registro que aponta.
    pub tabela: String,
    pub chave: String,
    /// "Venda", "Peça"... (para a tela).
    pub o_que: String,
    /// Tabela e chave do registro que falta.
    pub pai: String,
    pub falta: String,
    pub o_que_falta: String,
    /// "esperando", "apagado", "inconsistente" (com prova) ou "aceito" (o
    /// gerente aceitou como está).
    pub estado: String,
    pub desde: String,
    /// Prova da inconsistência: "log" ou "pares" (vazio nos outros estados).
    pub motivo: String,
    /// Esperando há 24 horas ou mais (aviso, não erro).
    pub atrasada: bool,
    /// Dá para o gerente "aceitar como está".
    pub pode_aceitar: bool,
    pub aceita_por: String,
    pub aceita_em: String,
    pub aceite_motivo: String,
    /// O que fazer.
    pub acao: String,
}

/// Depois de quanto tempo esperando uma referência passa a ser mostrada como
/// "atrasada". É só um aviso para o gerente olhar: **o tempo sozinho nunca a
/// torna inconsistente** (o dado pode chegar quando o outro caixa ligar).
const ATRASO_AVISO_HORAS: i64 = 24;

/// Confere cada referência quebrada: o registro que falta ainda não chegou
/// (espera), foi apagado em algum caixa (o log diz), ou é inconsistente (o
/// log diz que existe e a tabela não tem).
/// (tabela, chave, tabela que falta, chave que falta, estado).
type ReferenciaAgora = (String, String, String, String, &'static str);

fn referencias_agora(conn: &Connection) -> Resultado<Vec<ReferenciaAgora>> {
    let mut c = conn.prepare("PRAGMA foreign_key_check")?;
    let violacoes: Vec<(String, Option<i64>, String, i64)> =
        c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?.collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for (tabela, linha, pai, fk) in violacoes {
        let limpa = |t: &str| t.replace('"', "");
        let mut ligacao = Vec::new();
        {
            let mut q = conn.prepare(&format!("PRAGMA foreign_key_list(\"{}\")", limpa(&tabela)))?;
            let l = q.query_map([], |l| Ok((l.get::<_, i64>(0)?, l.get::<_, i64>(1)?, l.get::<_, String>(3)?)))?;
            for item in l {
                let (id, seq, de) = item?;
                if id == fk {
                    ligacao.push((seq, de));
                }
            }
        }
        ligacao.sort();
        let Some(linha) = linha else { continue };
        let (_, pk) = colunas(conn, &tabela)?;
        let juntar = |cols: &[String]| cols.iter().map(|c| format!("\"{}\"", limpa(c))).collect::<Vec<_>>().join(", ");
        let de: Vec<String> = ligacao.iter().map(|(_, c)| c.clone()).collect();
        if de.is_empty() || pk.is_empty() {
            continue;
        }
        let (chave, falta): (String, String) = conn.query_row(
            &format!(
                "SELECT json_array({}), json_array({}) FROM \"{}\" WHERE rowid = ?1",
                juntar(&pk),
                juntar(&de),
                limpa(&tabela)
            ),
            [linha],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )?;
        let ultima: Option<String> = conn
            .query_row(
                "SELECT operacao FROM alteracao WHERE tabela = ?1 AND chave = ?2 AND compactada = 0
                  ORDER BY ordem DESC, id DESC LIMIT 1",
                params![pai, falta],
                |l| l.get(0),
            )
            .optional()?;
        let estado = match ultima.as_deref() {
            Some("apagar") => "apagado",
            Some(_) => "inconsistente",
            None => "esperando",
        };
        lista.push((tabela, chave, pai, falta, estado));
    }
    Ok(lista)
}

/// Contagem das referências (veja `registrar_referencias`).
pub fn conferir_referencias(conn: &Connection) -> Resultado<Referencias> {
    let mut r = Referencias::default();
    for (.., estado) in referencias_agora(conn)? {
        if estado == "esperando" {
            r.esperando += 1;
        } else {
            r.quebradas += 1;
        }
    }
    Ok(r)
}

/// Confere de novo e guarda a lista (cada referência com a data em que
/// apareceu). Refeita a cada conferência, sem estado "grudado":
/// - **apagado**: o log diz que o registro foi apagado;
/// - **inconsistente (log)**: o log diz que o registro existe e a tabela não
///   tem (contradição entre dois registros locais, prova por si);
/// - **inconsistente (pares)**: o log nunca o mencionou, existe ao menos um
///   outro caixa conhecido e **todos** eles fizeram uma troca completa depois
///   de a referência aparecer, e o registro continua faltando;
/// - **esperando**: todo o resto. Sem prova não se bloqueia, e o tempo
///   sozinho não é prova (passadas 24 horas só ganha o marcador "atrasada").
///
/// O que o gerente aceitou (`aceito`) continua aceito enquanto o que falta for
/// o mesmo. A que deixou de faltar (o registro chegou) sai da lista sozinha.
/// Nenhum dado da loja é apagado.
pub fn registrar_referencias(conn: &Connection) -> Resultado<Referencias> {
    let agora = referencias_agora(conn)?;
    let agora_txt = tempo::agora();
    conn.execute("CREATE TEMP TABLE IF NOT EXISTS ref_atual (tabela TEXT, chave TEXT, pai TEXT)", [])?;
    conn.execute("DELETE FROM temp.ref_atual", [])?;
    // O aceite do gerente vale para a mesma falta, desde que a prova continue
    // sendo de inconsistência (apagado volta a ser apagado).
    const MANTEM_ACEITE: &str =
        "(sync_referencia.estado = 'aceito' AND sync_referencia.falta = excluded.falta AND excluded.estado <> 'apagado')";
    let upsert = format!(
        "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (tabela, chave, pai) DO UPDATE SET
             -- A espera recomeça só se o registro que falta mudou; a mesma
             -- referência, conferida de novo, guarda a data do começo.
             desde = CASE WHEN sync_referencia.falta <> excluded.falta THEN excluded.desde ELSE sync_referencia.desde END,
             estado = CASE WHEN {MANTEM_ACEITE} THEN 'aceito' ELSE excluded.estado END,
             motivo = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.motivo ELSE excluded.motivo END,
             aceita_por = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.aceita_por END,
             aceita_em = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.aceita_em END,
             aceite_motivo = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.aceite_motivo END,
             falta = excluded.falta"
    );
    for (tabela, chave, pai, falta, estado) in &agora {
        conn.execute("INSERT INTO temp.ref_atual VALUES (?1, ?2, ?3)", params![tabela, chave, pai])?;
        let motivo = if *estado == "inconsistente" { "log" } else { "" };
        conn.execute(&upsert, params![tabela, chave, pai, falta, estado, agora_txt, motivo])?;
    }
    conn.execute(
        "DELETE FROM sync_referencia WHERE NOT EXISTS
            (SELECT 1 FROM temp.ref_atual a WHERE a.tabela = sync_referencia.tabela
                AND a.chave = sync_referencia.chave AND a.pai = sync_referencia.pai)",
        [],
    )?;
    // Prova dos pares: todos os caixas conhecidos já entregaram tudo (troca
    // completa) DEPOIS de a referência aparecer, e o registro não veio.
    conn.execute(
        "UPDATE sync_referencia SET estado = 'inconsistente', motivo = 'pares'
          WHERE estado = 'esperando'
            AND EXISTS (SELECT 1 FROM sync_par)
            AND NOT EXISTS (SELECT 1 FROM sync_par p
                             WHERE p.completo_em IS NULL OR p.completo_em <= sync_referencia.desde)",
        [],
    )?;
    let (esperando, atrasadas, quebradas): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(estado = 'esperando'), 0),
                COALESCE(SUM(estado = 'esperando' AND (julianday('now') - julianday(desde)) * 24 >= ?1), 0),
                COALESCE(SUM(estado IN ('apagado', 'inconsistente')), 0)
           FROM sync_referencia",
        [ATRASO_AVISO_HORAS],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    config::gravar(conn, "sync_pendencias", &esperando.to_string())?;
    config::gravar(conn, "sync_atrasadas", &atrasadas.to_string())?;
    config::gravar(conn, "sync_quebradas", &quebradas.to_string())?;
    Ok(Referencias { esperando: esperando as usize, quebradas: quebradas as usize, atrasadas: atrasadas as usize })
}

/// A lista para a tela (as mais antigas primeiro, até 100).
pub fn referencias(conn: &Connection) -> Resultado<Vec<Referencia>> {
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        bool,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .prepare(
            "SELECT r.tabela, r.chave, r.pai, r.falta, r.estado, r.desde, r.motivo,
                    r.estado = 'esperando' AND (julianday('now') - julianday(r.desde)) * 24 >= ?1,
                    (SELECT nome FROM usuario WHERE id = r.aceita_por), r.aceita_em, r.aceite_motivo
               FROM sync_referencia r
              ORDER BY r.estado = 'esperando', r.desde LIMIT 100",
        )?
        .query_map([ATRASO_AVISO_HORAS], |l| {
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
                l.get(10)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for (tabela, chave, pai, falta, estado, desde, motivo, atrasada, aceita_por, aceita_em, aceite_motivo) in linhas {
        let o_que_falta = nome_da_tabela(&pai).to_string();
        let acao = match (estado.as_str(), motivo.as_str()) {
            ("esperando", _) if atrasada => "Está esperando há mais de 24 horas. Isso não é erro e não bloqueia nada: o dado pode \
                  chegar quando o outro caixa ligar. Confira se os outros caixas estão ligados, na mesma rede e na mesma \
                  versão, e se algum aparece acima com erro."
                .to_string(),
            ("esperando", _) => {
                "Ainda não chegou de outro caixa. Deixe os caixas ligados na mesma rede: resolve sozinho.".to_string()
            }
            ("apagado", _) => format!(
                "{o_que_falta} foi apagado em um caixa, mas este registro ainda aponta para ele. Abra o registro e \
                 corrija (escolha outro), ou cadastre de novo o que foi apagado. Nada foi apagado por isso."
            ),
            ("aceito", _) => "O gerente aceitou este registro como está. As operações estão liberadas.".to_string(),
            (_, "pares") => "Todos os caixas conhecidos já entregaram tudo o que têm e nenhum tem o que falta (o caixa que \
                  criou isto pode ter se perdido). Restaure o backup desse caixa, ou o gerente pode aceitar como está."
                .to_string(),
            _ => "O registro de alterações diz que o que falta existe, mas o banco não tem. Faça um backup e deixe os \
                  caixas trocarem dados de novo; se continuar, o gerente pode aceitar como está."
                .to_string(),
        };
        lista.push(Referencia {
            o_que: nome_da_tabela(&tabela).into(),
            tabela,
            chave,
            pai,
            falta,
            o_que_falta,
            pode_aceitar: estado == "inconsistente",
            estado,
            desde: crate::fiscal::data_local_br(conn, &desde).unwrap_or(desde),
            motivo,
            atrasada,
            aceita_por: aceita_por.unwrap_or_default(),
            aceita_em: aceita_em.map(|q| crate::fiscal::data_local_br(conn, &q).unwrap_or(q)).unwrap_or_default(),
            aceite_motivo: aceite_motivo.unwrap_or_default(),
            acao,
        });
    }
    Ok(lista)
}

/// O gerente aceita um registro **comprovadamente inconsistente** como está
/// (R-05). Exige o motivo; só vale para o que está `inconsistente`; não
/// inventa o registro que falta, só libera as operações. Quem chama grava a
/// auditoria. Devolve quantas referências foram aceitas.
pub fn aceitar_inconsistencia(
    conn: &Connection,
    usuario_id: &str,
    tabela: &str,
    chave: &str,
    motivo: &str,
) -> Resultado<usize> {
    let motivo = crate::texto::limpar(motivo);
    if motivo.chars().count() < 5 {
        return Err(Erro::campo("motivo", "Escreva o motivo (pelo menos 5 letras): fica no histórico."));
    }
    let n = conn.execute(
        "UPDATE sync_referencia SET estado = 'aceito', aceita_por = ?3, aceita_em = ?4, aceite_motivo = ?5
          WHERE tabela = ?1 AND chave = ?2 AND estado = 'inconsistente'",
        params![tabela, chave, usuario_id, tempo::agora(), motivo],
    )?;
    if n == 0 {
        return Err(Erro::validacao(
            "Este registro não está inconsistente (pode já ter sido resolvido). Atualize a lista.",
        ));
    }
    Ok(n)
}

/// R-05: recusa mexer no dinheiro de um registro **comprovadamente
/// inconsistente** (`sync_referencia.estado = 'inconsistente'`). Registro
/// esperando, apagado ou aceito pelo gerente passa; ver e imprimir nunca
/// passam por aqui. `id` é a chave do registro em `tabela`.
pub fn exigir_editavel(conn: &Connection, tabela: &str, id: &str) -> Resultado<()> {
    exigir_sem_inconsistencia(
        conn,
        "r.tabela = ?1 AND EXISTS (SELECT 1 FROM json_each(r.chave) WHERE value = ?2)",
        params![tabela, id],
    )
}

/// O mesmo para os registros "filhos" de um registro (os itens de uma compra,
/// os itens de uma devolução...): `filha.coluna = id`.
pub fn exigir_filhos_editaveis(conn: &Connection, filha: &str, coluna: &str, id: &str) -> Resultado<()> {
    let nome_ok = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    if !nome_ok(filha) || !nome_ok(coluna) {
        return Err(Erro::Interno("exigir_filhos_editaveis: nome de tabela ou coluna inválido".into()));
    }
    exigir_sem_inconsistencia(
        conn,
        &format!("r.tabela = ?1 AND r.chave IN (SELECT json_array(f.id) FROM \"{filha}\" f WHERE f.\"{coluna}\" = ?2)"),
        params![filha, id],
    )
}

fn exigir_sem_inconsistencia(conn: &Connection, onde: &str, valores: impl rusqlite::Params) -> Resultado<()> {
    let achada: Option<(String, String, String)> = conn
        .query_row(
            &format!(
                "SELECT r.tabela, r.pai, r.motivo FROM sync_referencia r
                  WHERE r.estado = 'inconsistente' AND {onde} LIMIT 1"
            ),
            valores,
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
        )
        .optional()?;
    let Some((tabela, pai, motivo)) = achada else { return Ok(()) };
    let prova = if motivo == "pares" {
        "todos os outros caixas já entregaram tudo e nenhum tem esse dado"
    } else {
        "o registro de alterações diz que ele existe e o banco não tem"
    };
    Err(Erro::validacao(format!(
        "Este registro ({}) ficou inconsistente: falta {} ({prova}). Por segurança o Rustinho não deixa mexer no \
         dinheiro dele até o gerente resolver. Configurações › Caixas da loja mostra o que fazer: restaurar o backup do \
         caixa de origem, ou o gerente aceitar como está.",
        nome_da_tabela(&tabela).to_lowercase(),
        nome_da_tabela(&pai).to_lowercase(),
    )))
}

/// Tira os dados das versões velhas de cada registro (fica a que vale).
/// A versão velha continua no log, sem dados: id, base e ordem bastam para
/// os caixas seguirem a cadeia de versões (R-08). Devolve quantas apagou. Pode rodar a qualquer hora: quem
/// pedir depois recebe só a versão que vale.
pub fn compactar(conn: &Connection) -> Resultado<usize> {
    let mut total = 0;
    loop {
        // Fica a versão que vale (maior ordem, depois maior id) e as que
        // perderam um conflito ainda aberto (para irem aos outros caixas).
        let n = conn.execute(
            "UPDATE alteracao SET dados = NULL, historia = NULL, compactada = 1 WHERE seq IN (
                SELECT a.seq FROM alteracao a
                 WHERE a.compactada = 0
                   AND EXISTS (SELECT 1 FROM alteracao b
                                WHERE b.tabela = a.tabela AND b.chave = a.chave AND b.compactada = 0
                                  AND (b.ordem > a.ordem OR (b.ordem = a.ordem AND b.id > a.id)))
                   AND a.id NOT IN (SELECT perdeu FROM sync_conflito WHERE resolvido_em IS NULL)
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
#[derive(Debug, Clone)]
struct Versao {
    id: String,
    dispositivo: String,
    base: Option<String>,
    dados: Option<String>,
    ordem: i64,
    historia: Option<String>,
}

impl Versao {
    /// Vale mais que `outra`? Maior ordem; empate, maior id. Não depende da
    /// ordem de chegada: todos os caixas escolhem a mesma.
    fn vale_mais_que(&self, outra: &Versao) -> bool {
        (self.ordem, self.id.as_str()) > (outra.ordem, outra.id.as_str())
    }
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

/// História de uma alteração nova feita sobre `base`: a base e o que a base
/// conhecia, até [`HISTORIA_MAXIMA`]; "+" no fim se não chega ao começo. Sem
/// base (registro novo): `[]`. Usada pelos gatilhos (`rustinho_historia`).
pub fn historia_nova(base: Option<&str>, historia_base: Option<&str>, base_da_base: Option<&str>) -> String {
    let Some(base) = base else { return "[]".into() };
    let anterior = linhagem(base_da_base, historia_base);
    let mut ids = vec![base.to_string()];
    ids.extend(anterior.ids);
    let mut completa = anterior.completa;
    if ids.len() > HISTORIA_MAXIMA {
        ids.truncate(HISTORIA_MAXIMA);
        completa = false;
    }
    if !completa {
        ids.push(INCOMPLETA.into());
    }
    serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())
}

/// As versões anteriores de uma versão, e se a lista chega ao começo.
struct Linhagem {
    ids: Vec<String>,
    completa: bool,
}

fn linhagem(base: Option<&str>, historia: Option<&str>) -> Linhagem {
    match historia.and_then(|h| serde_json::from_str::<Vec<String>>(h).ok()) {
        Some(lista) => {
            let completa = !lista.iter().any(|x| x == INCOMPLETA);
            let mut ids: Vec<String> = lista.into_iter().filter(|x| x != INCOMPLETA).collect();
            if let Some(b) = base {
                if !ids.iter().any(|x| x == b) {
                    ids.insert(0, b.to_string());
                }
            }
            Linhagem { ids, completa }
        }
        // Sem história: registro novo (sem base) ou alteração de antes do
        // protocolo 3 (só a base; o resto não se sabe).
        None => match base {
            None => Linhagem { ids: Vec::new(), completa: true },
            Some(b) => Linhagem { ids: vec![b.to_string()], completa: false },
        },
    }
}

/// Como uma versão está para outra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relacao {
    /// Foi feita depois (a outra está no caminho de versões dela).
    Depois,
    /// Foi feita antes (está no caminho da outra).
    Antes,
    /// Com certeza nenhuma está no caminho da outra: feitas sem uma ver a
    /// outra.
    Concorrente,
    /// O caminho de versões está incompleto neste caixa (alteração de antes
    /// do protocolo 3, por exemplo): não dá para saber. Conta como conflito
    /// (R-08: nada decidido em silêncio).
    Incerta,
}

/// `x` foi feita em cima de `alvo` (direto ou depois de outras)?
/// `Some(true)`: sim; `Some(false)`: com certeza não; `None`: não dá para
/// saber (falta uma versão no caminho).
///
/// Primeiro olha a história que vem na própria alteração; senão segue as
/// bases no log (as versões compactadas continuam lá, sem dados). A ordem
/// para a busca cedo: uma versão anterior sempre tem ordem menor.
fn descende(conn: &Connection, x: &Versao, alvo: &Versao) -> Resultado<Option<bool>> {
    if linhagem(x.base.as_deref(), x.historia.as_deref()).ids.contains(&alvo.id) {
        return Ok(Some(true));
    }
    // Ordem 0 = versão de antes do protocolo 3 (sem contador): não serve.
    let ordem_vale = |o: i64| o > 0 && alvo.ordem > 0;
    if ordem_vale(x.ordem) && x.ordem <= alvo.ordem {
        return Ok(Some(false));
    }
    let mut c = conn.prepare_cached("SELECT base, ordem FROM alteracao WHERE id = ?1")?;
    let mut atual = x.base.clone();
    let mut passos = 0u32;
    while let Some(id) = atual {
        if id == alvo.id {
            return Ok(Some(true));
        }
        let Some((base, ordem)) =
            c.query_row([&id], |l| Ok((l.get::<_, Option<String>>(0)?, l.get::<_, i64>(1)?))).optional()?
        else {
            return Ok(None);
        };
        if ordem_vale(ordem) && ordem <= alvo.ordem {
            return Ok(Some(false));
        }
        passos += 1;
        if passos > 1_000_000 {
            return Ok(None);
        }
        atual = base;
    }
    // Chegou ao começo do registro sem passar por `alvo`.
    Ok(Some(false))
}

fn relacao(conn: &Connection, x: &Versao, y: &Versao) -> Resultado<Relacao> {
    let depois = descende(conn, x, y)?;
    if depois == Some(true) {
        return Ok(Relacao::Depois);
    }
    let antes = descende(conn, y, x)?;
    if antes == Some(true) {
        return Ok(Relacao::Antes);
    }
    Ok(if depois == Some(false) && antes == Some(false) { Relacao::Concorrente } else { Relacao::Incerta })
}

/// A versão que vale agora de um registro, neste caixa.
fn versao_atual(conn: &Connection, tabela: &str, chave: &str) -> Resultado<Option<Versao>> {
    Ok(conn
        .query_row(
            "SELECT id, dispositivo, base, dados, ordem, historia FROM alteracao
              WHERE tabela = ?1 AND chave = ?2 AND compactada = 0 ORDER BY ordem DESC, id DESC LIMIT 1",
            params![tabela, chave],
            |l| {
                Ok(Versao {
                    id: l.get(0)?,
                    dispositivo: l.get(1)?,
                    base: l.get(2)?,
                    dados: l.get(3)?,
                    ordem: l.get(4)?,
                    historia: l.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Alguma versão deste registro que está no log foi feita em cima de
/// `id` (ele está na base ou na história dela)? Então `id` não se perdeu: o
/// caixa que a fez (ou outro) mexeu de novo depois.
fn alguem_partiu_de(conn: &Connection, tabela: &str, chave: &str, id: &str) -> Resultado<bool> {
    let mut c = conn.prepare_cached(
        "SELECT base, historia FROM alteracao WHERE tabela = ?1 AND chave = ?2 AND id <> ?3
            AND (base = ?3 OR instr(historia, ?4) > 0)",
    )?;
    let l = c.query_map(params![tabela, chave, id, format!("\"{id}\"")], |l| {
        Ok((l.get::<_, Option<String>>(0)?, l.get::<_, Option<String>>(1)?))
    })?;
    for v in l {
        let (base, historia) = v?;
        if linhagem(base.as_deref(), historia.as_deref()).ids.iter().any(|i| i == id) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Anota o conflito (uma vez por versão que perdeu: reenvio não repete).
/// Só a última versão de cada caminho conta: se outra versão já foi feita
/// em cima da que perdeu, não é conflito (veja `alguem_partiu_de`).
/// Se o gerente já decidiu em outro caixa, já entra resolvido.
fn anotar_conflito(
    conn: &Connection,
    tabela: &str,
    chave: &str,
    ficou: &Versao,
    perdeu: &Versao,
    certeza: Relacao,
) -> Resultado<()> {
    let ja: bool = conn
        .query_row("SELECT 1 FROM sync_conflito WHERE perdeu = ?1", [&perdeu.id], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if ja || alguem_partiu_de(conn, tabela, chave, &perdeu.id)? {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo, dados_perdeu, dispositivo_ficou,
                                    base_perdeu, historia_perdeu, certeza)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'registro', ?7, ?8, ?9, ?10, ?11)",
        params![
            tabela,
            chave,
            ficou.id,
            perdeu.id,
            perdeu.dispositivo,
            tempo::agora(),
            perdeu.dados,
            ficou.dispositivo,
            perdeu.base,
            perdeu.historia,
            if certeza == Relacao::Incerta { "incerta" } else { "concorrente" }
        ],
    )?;
    fechar_decididos(conn)?;
    // Guarda os últimos 5000 já resolvidos, com a versão que perdeu (os
    // abertos ficam sempre).
    conn.execute(
        "DELETE FROM sync_conflito WHERE resolvido_em IS NOT NULL
            AND id <= (SELECT MAX(id) - 5000 FROM sync_conflito)",
        [],
    )?;
    Ok(())
}

/// Fecha os conflitos que um gerente já decidiu (aqui ou em outro caixa) e
/// escreve no conflito o que cada gerente decidiu.
///
/// Dois gerentes podem decidir diferente sobre o mesmo conflito, com os
/// caixas sem rede. A regra (igual em todos os caixas, sem relógio): **vale
/// "usar a que perdeu"**. Essa escolha gravou uma alteração nova feita em
/// cima da versão que valia, e uma alteração feita depois sempre vale (pela
/// ordem). "Manter" não grava nada no registro, então não tem como valer
/// contra ela. A divergência fica registrada num evento "aplicada", com o
/// mesmo id em todos os caixas (feito a partir das decisões), e a tela diz
/// quem decidiu o quê e qual versão vale agora.
fn fechar_decididos(conn: &Connection) -> Resultado<()> {
    #[allow(clippy::type_complexity)]
    let decisoes: Vec<(String, String, String, String, String, Option<String>, String)> = {
        let mut c = conn.prepare(
            "SELECT r.id, r.tabela, r.chave, r.perdeu, r.escolha, u.nome, r.quando FROM sync_resolucao r
               LEFT JOIN usuario u ON u.id = r.usuario_id
              WHERE r.escolha IN ('usar', 'manter') ORDER BY r.tabela, r.chave, r.perdeu, r.id",
        )?;
        let l =
            c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)))?;
        l.collect::<Result<_, _>>()?
    };
    // (tabela, chave, versão que perdeu) → decisões (id, escolha, quem, quando).
    type Decisao = (String, String, String, String);
    let mut grupos: Vec<((String, String, String), Vec<Decisao>)> = Vec::new();
    for (id, tabela, chave, perdeu, escolha, nome, quando) in decisoes {
        let k = (tabela, chave, perdeu);
        let d = (id, escolha, nome.unwrap_or_else(|| "um gerente".into()), quando);
        match grupos.last_mut() {
            Some((g, lista)) if *g == k => lista.push(d),
            _ => grupos.push((k, vec![d])),
        }
    }
    for ((tabela, chave, perdeu), lista) in grupos {
        let usou = lista.iter().any(|d| d.1 == "usar");
        let manteve = lista.iter().any(|d| d.1 == "manter");
        let quem = |e: &str| lista.iter().filter(|d| d.1 == e).map(|d| d.2.clone()).collect::<Vec<_>>().join(", ");
        let texto = if usou && manteve {
            // Evento da divergência: mesmo id e mesmo conteúdo em todos os caixas.
            let ids: Vec<&str> = lista.iter().map(|d| d.0.as_str()).collect();
            let resumo: [u8; 32] =
                Sha256::digest(format!("{tabela}|{chave}|{perdeu}|{}", ids.join(",")).as_bytes()).into();
            let id_evento =
                format!("aplicada-{}", resumo.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>());
            let quando = lista.iter().map(|d| d.3.as_str()).max().unwrap_or_default().to_string();
            conn.execute(
                "INSERT OR IGNORE INTO sync_resolucao (id, tabela, chave, ficou, perdeu, escolha, usuario_id, quando, decisoes)
                 VALUES (?1, ?2, ?3, '', ?4, 'aplicada', NULL, ?5, ?6)",
                params![id_evento, tabela, chave, perdeu, quando, serde_json::to_string(&ids)?],
            )?;
            format!(
                "decisões diferentes: {} manteve, {} usou a que tinha perdido. Valeu a de usar a que tinha perdido \
                 (ela gravou uma alteração nova); a decisão de manter ficou registrada",
                quem("manter"),
                quem("usar")
            )
        } else if usou {
            format!("voltou a versão que tinha perdido ({})", quem("usar"))
        } else {
            format!("ficou como está ({})", quem("manter"))
        };
        let primeiro = conn
            .query_row(
                "SELECT usuario_id FROM sync_resolucao WHERE tabela = ?1 AND chave = ?2 AND perdeu = ?3
                    AND escolha IN ('usar', 'manter') ORDER BY id LIMIT 1",
                params![tabela, chave, perdeu],
                |l| l.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        conn.execute(
            "UPDATE sync_conflito SET resolvido_em = COALESCE(resolvido_em, ?4), resolvido_por = COALESCE(resolvido_por, ?5),
                    resolucao = ?6
              WHERE tabela = ?1 AND chave = ?2 AND perdeu = ?3 AND (resolucao IS NULL OR resolucao <> ?6)",
            params![tabela, chave, perdeu, tempo::agora(), primeiro, texto],
        )?;
    }
    Ok(())
}

/// Conflitos abertos que deixaram de ser conflito (o resultado é o mesmo
/// em qualquer ordem de chegada):
/// - a versão que vale (ou outra que perdeu) foi feita em cima da que
///   perdeu: ela não se perdeu, alguém a viu e mexeu depois;
/// - no fim, a versão que vale ficou igual à que perdeu.
///
/// Esses saem da lista; os decididos pelo gerente ficam como resolvidos.
fn limpar_conflitos(conn: &Connection) -> Resultado<()> {
    fechar_decididos(conn)?;
    #[allow(clippy::type_complexity)]
    let abertos: Vec<(i64, String, String, String, Option<String>, Option<String>, Option<String>)> = {
        let mut c = conn.prepare(
            "SELECT id, tabela, chave, perdeu, dados_perdeu, base_perdeu, historia_perdeu FROM sync_conflito
              WHERE tipo = 'registro' AND resolvido_em IS NULL",
        )?;
        let l =
            c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)))?;
        l.collect::<Result<_, _>>()?
    };
    for (id, tabela, chave, perdeu, dados, _, _) in &abertos {
        let Some(atual) = versao_atual(conn, tabela, chave)? else { continue };
        let partiu =
            |base: Option<&str>, historia: Option<&str>| linhagem(base, historia).ids.iter().any(|i| i == perdeu);
        // Outra que perdeu foi feita em cima desta (vale mesmo se o log já
        // não tem mais a versão: a história fica no conflito).
        let outra_partiu = abertos
            .iter()
            .any(|o| o.0 != *id && o.1 == *tabela && o.2 == *chave && partiu(o.5.as_deref(), o.6.as_deref()));
        let sem_diferenca = atual.id != *perdeu && diferencas(atual.dados.as_deref(), dados.as_deref()).is_empty();
        if outra_partiu || sem_diferenca || alguem_partiu_de(conn, tabela, chave, perdeu)? {
            conn.execute("DELETE FROM sync_conflito WHERE id = ?1", [id])?;
        }
    }
    Ok(())
}

fn aplicar_dentro(conn: &mut Connection, lista: &[Alteracao]) -> Resultado<usize> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", [])?;
    let mut novas = 0;
    let mut colunas_cache: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for a in lista {
        if !TABELAS.contains(&a.tabela.as_str()) {
            continue;
        }
        // Já vista (reenvio, a própria alteração voltando, backup restaurado
        // em outro caixa): nada a fazer, nem aviso. Pelo id, não pela data
        // (o log guarda o id de toda versão, mesmo compactada). Uma alteração
        // deste caixa que ele não tem mais (perdeu ao restaurar um backup)
        // volta dos outros e é aplicada.
        let ja: bool =
            tx.query_row("SELECT 1 FROM alteracao WHERE id = ?1", [&a.id], |_| Ok(true)).optional()?.unwrap_or(false);
        if ja {
            continue;
        }
        // Versão velha sem dados (o outro caixa compactou): entra só para a
        // cadeia de versões. Já perdeu lá para outra, que também vem.
        if a.compactada {
            tx.execute(
                "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia, compactada)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, NULL, 1)",
                params![a.id, a.tabela, a.chave, a.operacao, a.dispositivo, a.quando, a.base, a.ordem],
            )?;
            novas += 1;
            continue;
        }
        // A versão que vale agora deste registro, neste caixa.
        let ultima = versao_atual(&tx, &a.tabela, &a.chave)?;
        tx.execute(
            "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![a.id, a.tabela, a.chave, a.operacao, a.dados, a.dispositivo, a.quando, a.base, a.ordem, a.historia],
        )?;
        novas += 1;
        let chegou = Versao {
            id: a.id.clone(),
            dispositivo: a.dispositivo.clone(),
            base: a.base.clone(),
            dados: a.dados.clone(),
            ordem: a.ordem,
            historia: a.historia.clone(),
        };
        if let Some(u) = &ultima {
            let ganhou = chegou.vale_mais_que(u);
            // Feitas sem uma ver a outra, ou sem como saber: fica anotado para
            // o gerente, com as duas guardadas. Uma feita depois da outra
            // (mesmo com o relógio atrasado) não é conflito.
            let r = relacao(&tx, &chegou, u)?;
            if matches!(r, Relacao::Concorrente | Relacao::Incerta) && a.tabela != "auditoria" {
                let (ficou, perdeu) = if ganhou { (&chegou, u) } else { (u, &chegou) };
                if !diferencas(ficou.dados.as_deref(), perdeu.dados.as_deref()).is_empty() {
                    anotar_conflito(&tx, &a.tabela, &a.chave, ficou, perdeu, r)?;
                }
            }
            if !ganhou {
                continue;
            }
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
    limpar_conflitos(&tx)?;
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
        "produto"
        | "produto_preco"
        | "produto_aplicacao"
        | "produto_equivalente"
        | "produto_fornecedor"
        | "produto_codigo_antigo" => "Peça",
        "faixa_codigo" => "Faixa de códigos das peças",
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
    /// Caixa da versão que perdeu.
    pub caixa: String,
    /// Caixa da versão que vale agora.
    pub caixa_ficou: String,
    /// "concorrente" (feitas sem uma ver a outra) ou "incerta" (não deu
    /// para saber a ordem, por exemplo depois de restaurar um backup).
    pub certeza: String,
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

/// Nome de um caixa para a tela ("este caixa" para o próprio).
pub(crate) fn nome_do_caixa(conn: &Connection, dispositivo: &str) -> Resultado<String> {
    if dispositivo.is_empty() {
        return Ok(String::new());
    }
    if config::ler(conn, "dispositivo_id")?.as_deref() == Some(dispositivo) {
        return Ok(format!("{} (este caixa)", config::nome_caixa(conn)?));
    }
    Ok(conn
        .query_row("SELECT nome FROM sync_par WHERE dispositivo = ?1", [dispositivo], |l| l.get::<_, String>(0))
        .optional()?
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "outro caixa".into()))
}

/// Conflitos abertos (e os últimos resolvidos), mais novos primeiro.
pub fn conflitos(conn: &Connection) -> Resultado<Vec<Conflito>> {
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(i64, String, String, String, String, String, Option<String>, Option<String>, String, String)> = {
        let mut c = conn.prepare(
            "SELECT id, tipo, tabela, chave, perdeu, quando, dados_perdeu, resolucao, dispositivo, certeza
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
    for (id, tipo, tabela, chave, perdeu, quando, dados_perdeu, resolucao, dispositivo, certeza) in linhas {
        let mut k = Conflito {
            id,
            tipo: tipo.clone(),
            o_que: nome_da_tabela(&tabela).into(),
            texto: String::new(),
            quando: crate::fiscal::data_local_br(conn, &quando).unwrap_or(quando.clone()),
            caixa: nome_do_caixa(conn, &dispositivo)?,
            caixa_ficou: String::new(),
            certeza: certeza.clone(),
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
                let atual = versao_atual(conn, &tabela, &chave)?;
                k.caixa_ficou = match &atual {
                    Some(v) => nome_do_caixa(conn, &v.dispositivo)?,
                    None => String::new(),
                };
                let atual = atual.and_then(|v| v.dados);
                let ler = |t: &Option<String>| -> Value {
                    t.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null)
                };
                let (a, p) = (ler(&atual), ler(&dados_perdeu));
                let nome = a.get("nome").or_else(|| p.get("nome")).and_then(|v| v.as_str()).unwrap_or_default();
                let oque = if nome.is_empty() { "no mesmo registro".to_string() } else { format!("em \"{nome}\"") };
                let quem = |c: &str| if c.is_empty() { "outro caixa".to_string() } else { c.to_string() };
                k.texto = if certeza == "incerta" {
                    format!(
                        "Não deu para saber qual alteração {oque} veio depois (um caixa ficou muito tempo sem rede ou voltou de um backup). \
                         Ficou a de {}; a de {} está guardada.",
                        quem(&k.caixa_ficou),
                        quem(&k.caixa)
                    )
                } else {
                    format!(
                        "Dois caixas mexeram {oque} sem ver o do outro. Ficou a de {}; a de {} está guardada.",
                        quem(&k.caixa_ficou),
                        quem(&k.caixa)
                    )
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
                // Depois de resolvido, o que vale agora de fato (pelos dados,
                // não pela decisão anotada).
                if !k.resolvido.is_empty() {
                    let igual_a_que_perdeu = diferencas(atual.as_deref(), dados_perdeu.as_deref()).is_empty();
                    k.resolvido.push_str(if igual_a_que_perdeu {
                        ". Vale agora: a versão que tinha perdido"
                    } else {
                        ". Vale agora: a versão que estava valendo; a outra continua guardada aqui"
                    });
                }
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
///
/// A decisão vira um registro em `sync_resolucao`, que vai para os outros
/// caixas: lá o mesmo conflito fecha sozinho, e a versão que perdeu não volta
/// a valer por engano. Quando volta a que tinha perdido, a alteração nova
/// leva na história as duas versões (a que valia e a que perdeu).
pub fn resolver_conflito(conn: &Connection, id: i64, usar: bool, usuario_id: &str) -> Resultado<()> {
    #[allow(clippy::type_complexity)]
    let (tipo, tabela, chave, perdeu, dados, resolvido): (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT tipo, tabela, chave, perdeu, dados_perdeu, resolvido_em FROM sync_conflito WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Conflito".into()))?;
    if resolvido.is_some() {
        return Err(Erro::validacao("Este conflito já foi resolvido."));
    }
    let valia = versao_atual(conn, &tabela, &chave)?.map(|v| v.id).unwrap_or_default();
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
        // A alteração que o gerente acabou de fazer conhece as duas versões.
        if let Some(nova) = versao_atual(conn, &tabela, &chave)?.filter(|v| v.id != valia) {
            let l = linhagem(nova.base.as_deref(), nova.historia.as_deref());
            let mut ids = l.ids;
            if !ids.contains(&perdeu) {
                ids.insert(ids.len().min(1), perdeu.clone());
            }
            let mut completa = l.completa;
            if ids.len() > HISTORIA_MAXIMA {
                ids.truncate(HISTORIA_MAXIMA);
                completa = false;
            }
            if !completa {
                ids.push(INCOMPLETA.into());
            }
            conn.execute(
                "UPDATE alteracao SET historia = ?2 WHERE id = ?1",
                params![nova.id, serde_json::to_string(&ids)?],
            )?;
        }
    }
    // A decisão vai para os outros caixas (o gatilho anota a alteração).
    conn.execute(
        "INSERT INTO sync_resolucao (id, tabela, chave, ficou, perdeu, escolha, usuario_id, quando)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            tempo::novo_id(),
            tabela,
            chave,
            valia,
            perdeu,
            if usar { "usar" } else { "manter" },
            usuario_id,
            tempo::agora()
        ],
    )?;
    fechar_decididos(conn)?;
    Ok(())
}

/// "Deixar todas como estão": fecha de uma vez os conflitos abertos (por
/// exemplo, muitos de "ordem incerta" depois de restaurar um backup). As
/// versões que perderam continuam guardadas na lista dos resolvidos.
pub fn manter_todos(conn: &Connection, usuario_id: &str) -> Resultado<usize> {
    let ids: Vec<i64> = {
        let mut c = conn.prepare("SELECT id FROM sync_conflito WHERE resolvido_em IS NULL ORDER BY id")?;
        let l = c.query_map([], |l| l.get(0))?;
        l.collect::<Result<_, _>>()?
    };
    for id in &ids {
        resolver_conflito(conn, *id, false, usuario_id)?;
    }
    Ok(ids.len())
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

/// R-09: um pacote com um sal nunca visto obriga a uma conta cara (Argon2)
/// antes de saber se a senha confere. Para ninguém na rede ocupar o caixa
/// mandando sais aleatórios, só `SAIS_NOVOS_POR_MINUTO` contas dessas por
/// minuto (somando UDP e TCP); os sais que já deram certo ficam guardados e
/// não gastam nada. A conta do próprio sal não entra aqui.
const SAIS_NOVOS_POR_MINUTO: u32 = 10;
const SAIS_GUARDADOS: usize = 64;

/// Contas caras que um mesmo endereço pode pedir por minuto. Um aparelho
/// sozinho não gasta o limite de todos; um caixa novo na rede continua
/// entrando. (Antes de conferir a senha não há como saber quem mandou de
/// verdade: o endereço é o que existe. Quem trocar de endereço esbarra no
/// limite de todos, `SAIS_NOVOS_POR_MINUTO`.)
const SAIS_NOVOS_POR_ORIGEM: u32 = 3;
/// Endereços diferentes lembrados por minuto (o controle não cresce sem fim).
const ORIGENS_POR_MINUTO: usize = 256;

struct Porteiro {
    inicio: Instant,
    usados: u32,
    por_origem: HashMap<Option<std::net::IpAddr>, u32>,
}

impl Porteiro {
    fn novo() -> Porteiro {
        Porteiro { inicio: Instant::now(), usados: 0, por_origem: HashMap::new() }
    }

    /// Pode fazer mais uma conta agora, para este endereço?
    fn permitir(&mut self, agora: Instant, origem: Option<std::net::IpAddr>) -> bool {
        if agora.duration_since(self.inicio) >= Duration::from_secs(60) {
            self.inicio = agora;
            self.usados = 0;
            self.por_origem.clear();
        }
        if self.usados >= SAIS_NOVOS_POR_MINUTO {
            return false;
        }
        let ja = self.por_origem.get(&origem).copied().unwrap_or(0);
        if ja >= SAIS_NOVOS_POR_ORIGEM || (ja == 0 && self.por_origem.len() >= ORIGENS_POR_MINUTO) {
            return false;
        }
        self.por_origem.insert(origem, ja + 1);
        self.usados += 1;
        true
    }
}

/// Pode fazer mais uma conta cara agora? No programa, um controle só para
/// todas as tarefas; nos testes, um por tarefa (os testes rodam juntos e um
/// não pode gastar o limite do outro).
fn pode_contar(origem: Option<std::net::IpAddr>) -> bool {
    #[cfg(not(test))]
    {
        static P: std::sync::OnceLock<Mutex<Porteiro>> = std::sync::OnceLock::new();
        P.get_or_init(|| Mutex::new(Porteiro::novo()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .permitir(Instant::now(), origem)
    }
    #[cfg(test)]
    {
        thread_local! {
            static P: std::cell::RefCell<Porteiro> = std::cell::RefCell::new(Porteiro::novo());
        }
        P.with(|p| p.borrow_mut().permitir(Instant::now(), origem))
    }
}

/// Chaves dos outros caixas que já conferiram (sal e senha certos).
fn conhecidos() -> &'static CacheChaves {
    static C: std::sync::OnceLock<CacheChaves> = std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Erro quando o outro caixa fala outra versão (ou a senha não confere).
fn decifrar(chaves: &Chaves, dados: &[u8], origem: Option<std::net::IpAddr>) -> Result<Vec<u8>, &'static str> {
    if dados.len() < 4 + 16 + 12 + 16 || &dados[..4] != MAGICA {
        return Err("O outro caixa está com outra versão do Rustinho. Atualize todos os caixas para a mesma versão.");
    }
    let mut sal = [0u8; 16];
    sal.copy_from_slice(&dados[4..20]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&dados[20..32]);
    let abrir = |chave: &[u8; 32]| -> Option<Vec<u8>> {
        let cifra = ChaCha20Poly1305::new_from_slice(chave).ok()?;
        cifra.decrypt(&nonce.into(), &dados[32..]).ok()
    };
    let senha_errada = "A senha da rede não confere com a deste caixa.";
    if sal == chaves.sal {
        return abrir(&chaves.propria).ok_or(senha_errada);
    }
    let id: [u8; 32] = Sha256::digest(chaves.senha.as_bytes()).into();
    let guardada = conhecidos().lock().unwrap_or_else(|e| e.into_inner()).get(&(id, sal)).copied();
    if let Some(k) = guardada {
        return abrir(&k).ok_or(senha_errada);
    }
    if !pode_contar(origem) {
        return Err("Muitos pedidos de outros aparelhos agora; a troca de dados tenta de novo em instantes.");
    }
    let mut k = [0u8; 32];
    crate::usuarios::argon2()
        .hash_password_into(chaves.senha.as_bytes(), &sal, &mut k)
        .map_err(|_| "chave inválida")?;
    let claro = abrir(&k).ok_or(senha_errada)?;
    // Só guarda depois de conferir: sal inventado nunca ocupa lugar.
    let mut c = conhecidos().lock().unwrap_or_else(|e| e.into_inner());
    if c.len() >= SAIS_GUARDADOS {
        if let Some(qualquer) = c.keys().next().copied() {
            c.remove(&qualquer);
        }
    }
    c.insert((id, sal), k);
    Ok(claro)
}

fn enviar(fluxo: &mut TcpStream, chaves: &Chaves, v: &Value) -> Resultado<()> {
    let corpo = cifrar(chaves, v.to_string().as_bytes())?;
    let tam = (corpo.len() as u32).to_be_bytes();
    fluxo.write_all(&tam).and_then(|_| fluxo.write_all(&corpo)).map_err(rede)?;
    Ok(())
}

/// Lê exatamente `buf.len()` bytes em até `prazo`, aos pedaços: se pedirem
/// para parar (fechar o programa), desiste logo em vez de esperar o prazo.
fn ler_tudo(fluxo: &mut TcpStream, buf: &mut [u8], prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<()> {
    let fim = Instant::now() + prazo;
    fluxo.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let mut lido = 0;
    while lido < buf.len() {
        if parar.is_some_and(|p| p.load(Ordering::SeqCst)) {
            return Err(Erro::validacao("A troca de dados parou (o Rustinho está fechando)."));
        }
        match fluxo.read(&mut buf[lido..]) {
            Ok(0) => return Err(rede(std::io::ErrorKind::UnexpectedEof.into())),
            Ok(n) => lido += n,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                if Instant::now() >= fim {
                    return Err(rede(e));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(rede(e)),
        }
    }
    Ok(())
}

fn receber(
    fluxo: &mut TcpStream,
    chaves: &Chaves,
    limite: usize,
    prazo: Duration,
    parar: Option<&AtomicBool>,
) -> Resultado<Value> {
    let origem = fluxo.peer_addr().ok().map(|a| a.ip());
    let mut tam = [0u8; 4];
    ler_tudo(fluxo, &mut tam, prazo, parar)?;
    let tam = u32::from_be_bytes(tam) as usize;
    if tam > limite {
        return Err(Erro::validacao("Mensagem grande demais."));
    }
    let mut corpo = vec![0u8; tam];
    ler_tudo(fluxo, &mut corpo, prazo, parar)?;
    let claro = decifrar(chaves, &corpo, origem).map_err(Erro::validacao)?;
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
fn atender(caminho: &Path, mut fluxo: TcpStream, parar: &AtomicBool) -> Resultado<()> {
    // Quem não manda o pedido em `PRAZO_DO_PEDIDO` é desligado.
    fluxo.set_read_timeout(Some(PRAZO_DO_PEDIDO)).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    let mut conn = abrir_conexao(caminho)?;
    let a = ajustes(&conn)?;
    if !a.ligado || a.senha.is_empty() {
        return Ok(());
    }
    // Banco copiado de outro computador: tem o identificador de troca do
    // caixa de onde veio e não pode trocar dados com ele até ser registrado
    // como caixa novo (veja `faixas::registrar_caixa_novo`).
    if crate::faixas::copia_detectada(&conn)? {
        return Ok(());
    }
    let k = Chaves::de(&conn, &a.senha)?;
    let pedido = receber(&mut fluxo, &k, TAMANHO_PEDIDO, PRAZO_DO_PEDIDO, Some(parar))?;
    if pedido["tipo"] != "puxar" && pedido["tipo"] != "faixa" {
        return Ok(());
    }
    if pedido["protocolo"].as_i64() != Some(PROTOCOLO) {
        return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
    }
    // Pedido de faixa de códigos das peças: só o distribuidor atende, numa
    // gravação só (veja `faixas::atender_pedido`).
    if pedido["tipo"] == "faixa" {
        let resposta = responder_faixa(&mut conn, &pedido)?;
        return enviar(&mut fluxo, &k, &resposta);
    }
    // Quem pediu também vira um par conhecido (com o endereço de onde veio).
    if let (Some(d), Ok(origem)) = (pedido["dispositivo"].as_str(), fluxo.peer_addr()) {
        let porta = pedido["porta"].as_u64().unwrap_or(PORTA_TCP as u64);
        if d != a.dispositivo {
            registrar_par(&conn, d, pedido["nome"].as_str().unwrap_or(""), &format!("{}:{porta}", origem.ip()))?;
        }
    }
    let desde = pedido["desde"].as_i64().unwrap_or(0).max(0);
    let geracao = geracao(&conn)?;
    let seq_maxima = seq_maxima(&conn)?;
    // O contador de quem pediu ainda vale neste banco? A alteração número
    // `desde` aqui tem de ser a mesma que ele recebeu (a "marca"). Se não é
    // (este banco voltou de um backup, é outro computador, foi copiado à
    // mão), ele recomeça do zero.
    if desde > 0 {
        let aqui: Option<String> =
            conn.query_row("SELECT id FROM alteracao WHERE seq = ?1", [desde], |l| l.get(0)).optional()?;
        if aqui.is_none() || aqui.as_deref() != pedido["marca"].as_str() {
            return enviar(
                &mut fluxo,
                &k,
                &json!({ "protocolo": PROTOCOLO, "dispositivo": a.dispositivo, "nome": a.nome, "alteracoes": [],
                         "temMais": false, "agoraMs": agora_ms(), "geracao": geracao, "seqMaxima": seq_maxima,
                         "recomecar": true }),
            );
        }
    }
    let (lista, tem_mais) = alteracoes_desde(&conn, desde, LOTE)?;
    enviar(
        &mut fluxo,
        &k,
        &json!({ "protocolo": PROTOCOLO, "dispositivo": a.dispositivo, "nome": a.nome, "alteracoes": lista,
                 "temMais": tem_mais, "agoraMs": agora_ms(), "geracao": geracao, "seqMaxima": seq_maxima }),
    )
}

/// Geração deste banco: muda quando ele volta de um backup (veja
/// `depois_de_restaurar`). Fica só neste computador.
pub fn geracao(conn: &Connection) -> Resultado<String> {
    if let Some(g) = config::ler(conn, "sync_geracao")?.filter(|g| !g.is_empty()) {
        return Ok(g);
    }
    let g = tempo::novo_id();
    config::gravar(conn, "sync_geracao", &g)?;
    Ok(g)
}

/// O maior número de alteração deste banco.
fn seq_maxima(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM alteracao", [], |l| l.get(0))?)
}

/// Depois de restaurar um backup neste caixa: a numeração das alterações
/// continua de onde estava (`seq_anterior`), este caixa recomeça do zero com
/// cada outro caixa (recebe de volta o que o backup não tinha) e o banco
/// ganha uma geração nova (os outros caixas também recomeçam com ele).
pub fn depois_de_restaurar(conn: &Connection, seq_anterior: i64) -> Resultado<()> {
    conn.execute("UPDATE sync_par SET cursor = 0, marca = NULL, geracao = NULL, completo_em = NULL", [])?;
    conn.execute("UPDATE sqlite_sequence SET seq = MAX(seq, ?1) WHERE name = 'alteracao'", [seq_anterior])?;
    conn.execute(
        "INSERT INTO sqlite_sequence (name, seq) SELECT 'alteracao', ?1
          WHERE NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'alteracao')",
        [seq_anterior],
    )?;
    config::gravar(conn, "sync_geracao", &tempo::novo_id())?;
    // Códigos das peças: geração nova (a faixa de antes não é mais usada).
    crate::faixas::depois_de_restaurar(conn)?;
    Ok(())
}

/// Puxa de um endereço tudo o que há de novo. Devolve (dispositivo, quantas novas).
pub fn puxar(caminho: &Path, endereco: &str, porta_local: u16) -> Resultado<(String, usize)> {
    puxar_ate(caminho, endereco, porta_local, None)
}

/// Como `puxar`, mas para entre um lote e outro se pedirem (fechar o programa).
fn puxar_ate(
    caminho: &Path,
    endereco: &str,
    porta_local: u16,
    parar: Option<&AtomicBool>,
) -> Resultado<(String, usize)> {
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    let mut conn = abrir_conexao(caminho)?;
    if crate::faixas::copia_detectada(&conn)? {
        return Err(Erro::validacao(crate::faixas::texto_do_bloqueio(crate::faixas::BLOQUEIO_COPIA)));
    }
    let a = ajustes(&conn)?;
    let k = Chaves::de(&conn, &a.senha)?;
    let alvo: SocketAddr = match endereco.parse::<SocketAddr>() {
        Ok(s) => s,
        Err(_) => format!("{endereco}:{PORTA_TCP}").parse().map_err(|_| Erro::validacao("Endereço inválido."))?,
    };
    let mut total = 0;
    let mut dispositivo = String::new();
    let mut recomecos = 0;
    loop {
        // Contador, marca e geração guardados deste outro caixa (na primeira
        // volta, pelo endereço: pode ser de um computador que já não está lá).
        #[allow(clippy::type_complexity)]
        let guardado: Option<(String, i64, Option<String>, Option<String>)> = if dispositivo.is_empty() {
            conn.query_row(
                "SELECT dispositivo, cursor, marca, geracao FROM sync_par WHERE endereco = ?1 ORDER BY visto_em DESC LIMIT 1",
                [alvo.to_string()],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
            )
            .optional()?
        } else {
            conn.query_row(
                "SELECT dispositivo, cursor, marca, geracao FROM sync_par WHERE dispositivo = ?1",
                [&dispositivo],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
            )
            .optional()?
        };
        let (dono, cursor, marca, geracao_guardada) = guardado.unwrap_or_default();
        let mut fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).map_err(rede)?;
        fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok();
        fluxo.set_write_timeout(Some(Duration::from_secs(10))).ok();
        let saiu = agora_ms();
        enviar(
            &mut fluxo,
            &k,
            &json!({ "tipo": "puxar", "protocolo": PROTOCOLO, "desde": cursor, "marca": marca,
                     "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_local }),
        )?;
        let resposta = receber(&mut fluxo, &k, TAMANHO_MAXIMO, Duration::from_secs(30), parar)?;
        // Outro protocolo: recusa antes de ler qualquer alteração.
        if resposta["protocolo"].as_i64() != Some(PROTOCOLO) {
            return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
        }
        let voltou = agora_ms();
        dispositivo = resposta["dispositivo"].as_str().unwrap_or_default().to_string();
        if dispositivo.is_empty() || dispositivo == a.dispositivo {
            return Err(Erro::validacao("Este endereço é deste mesmo caixa."));
        }
        registrar_par(&conn, &dispositivo, resposta["nome"].as_str().unwrap_or(""), &alvo.to_string())?;
        // Neste endereço agora está outro caixa (computador trocado): pede de
        // novo com o contador dele, sem aplicar o que veio.
        if !dono.is_empty() && dono != dispositivo {
            recomecos += 1;
            if recomecos > 3 {
                return Err(Erro::validacao("A troca de dados não conseguiu recomeçar com este caixa."));
            }
            continue;
        }
        let geracao_dele = resposta["geracao"].as_str().unwrap_or_default().to_string();
        let seq_dele = resposta["seqMaxima"].as_i64().unwrap_or(i64::MAX);
        let motivo = if resposta["recomecar"].as_bool().unwrap_or(false) {
            Some("o contador não bate com o banco dele")
        } else if geracao_guardada.as_deref().is_some_and(|g| !g.is_empty() && g != geracao_dele) {
            Some("o banco dele voltou de um backup")
        } else if seq_dele < cursor {
            Some("a numeração dele voltou para trás")
        } else {
            None
        };
        if let Some(motivo) = motivo {
            // Recomeça do zero com ele: recebe tudo de novo; o que já estava
            // aqui é reconhecido pelo id e não é aplicado outra vez.
            crate::registro::anotar(&format!("troca de dados: recomeçando do zero com um caixa ({motivo})"));
            conn.execute(
                "UPDATE sync_par SET cursor = 0, marca = NULL, geracao = ?2, completo_em = NULL WHERE dispositivo = ?1",
                params![dispositivo, geracao_dele],
            )?;
            recomecos += 1;
            if recomecos > 3 {
                return Err(Erro::validacao("A troca de dados não conseguiu recomeçar com este caixa."));
            }
            continue;
        }
        let lista: Vec<Alteracao> = serde_json::from_value(resposta["alteracoes"].clone())?;
        let ultima = lista.iter().max_by_key(|x| x.seq).map(|x| (x.seq, x.id.clone()));
        let novo_cursor = ultima.as_ref().map_or(cursor, |(m, _)| *m);
        total += aplicar(&mut conn, &lista)?;
        let avancou =
            match ultima {
                // Só avança se ninguém zerou o cursor no meio (restauração de backup).
                Some((m, id)) => conn.execute(
                    "UPDATE sync_par SET cursor = ?2, marca = ?4, geracao = ?5 WHERE dispositivo = ?1 AND cursor = ?3",
                    params![dispositivo, m, cursor, id, geracao_dele],
                )? > 0,
                None => {
                    conn.execute(
                        "UPDATE sync_par SET geracao = ?2 WHERE dispositivo = ?1 AND (geracao IS NULL OR geracao = '')",
                        params![dispositivo, geracao_dele],
                    )?;
                    true
                }
            };
        // Troca COMPLETA com este caixa (R-05): não há mais nada a receber e o
        // contador chegou ao máximo dele. É a prova de que ele já entregou tudo.
        if avancou && !resposta["temMais"].as_bool().unwrap_or(false) && novo_cursor >= seq_dele {
            conn.execute(
                "UPDATE sync_par SET completo_em = ?2 WHERE dispositivo = ?1",
                params![dispositivo, tempo::agora()],
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
        if !resposta["temMais"].as_bool().unwrap_or(false) || parar.is_some_and(|p| p.load(Ordering::SeqCst)) {
            break;
        }
    }
    // Códigos das peças: depois de receber tudo do caixa que distribui as
    // faixas (assim este caixa já conhece todos os códigos que ele conhece),
    // pede a faixa se não tem ou se está acabando (veja `faixas`).
    if !parar.is_some_and(|p| p.load(Ordering::SeqCst))
        && crate::faixas::distribuidor(&conn)?.as_deref() == Some(dispositivo.as_str())
        && crate::faixas::precisa(&conn)?
    {
        match pedir_faixa(&mut conn, &k, &alvo) {
            Ok(n) => total += n,
            Err(e) => crate::registro::anotar(&format!("troca de dados: pedido de faixa de códigos: {e}")),
        }
    }
    // Depois de tudo: algum registro ficou apontando para outro que não veio?
    registrar_referencias(&conn)?;
    if total > 0 {
        crate::usuarios::resolver_nomes_repetidos(&conn)?;
        verificar_depois(&conn)?;
    }
    // Peças deste caixa com código a definir recebem código da faixa.
    if let Err(e) = crate::faixas::manter(&mut conn) {
        crate::registro::anotar(&format!("códigos das peças: {e}"));
    }
    Ok((dispositivo, total))
}

/// Pede ao distribuidor uma faixa de códigos das peças. A faixa chega como
/// uma alteração do log (com o mesmo id que o distribuidor gravou) e fica
/// gravada aqui antes de qualquer código dela ser dado. Devolve quantas
/// alterações eram novas.
fn pedir_faixa(conn: &mut Connection, k: &Chaves, alvo: &SocketAddr) -> Resultado<usize> {
    let mut pedido = crate::faixas::pedido(conn)?;
    pedido["tipo"] = json!("faixa");
    pedido["protocolo"] = json!(PROTOCOLO);
    let mut fluxo = TcpStream::connect_timeout(alvo, Duration::from_secs(3)).map_err(rede)?;
    fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(10))).ok();
    enviar(&mut fluxo, k, &pedido)?;
    let resposta = receber(&mut fluxo, k, TAMANHO_PEDIDO, Duration::from_secs(30), None)?;
    gravar_faixa_recebida(conn, &resposta)
}

/// O motor da sincronização: três tarefas em segundo plano (servidor,
/// anúncio/escuta na rede e puxada periódica). Fica parado enquanto a
/// sincronização estiver desligada nas configurações.
///
/// R-06: as tarefas são guardadas e, ao encerrar, o motor espera todas
/// terminarem (inclusive os atendimentos em andamento) antes de devolver.
pub struct Motor {
    parar: Arc<AtomicBool>,
    tarefas: Vec<thread::JoinHandle<()>>,
    /// Já pediram para encerrar com um prazo (quem pediu decide o que fazer
    /// se não deu tempo; o descarte não espera de novo).
    encerrado: bool,
}

impl Drop for Motor {
    fn drop(&mut self) {
        if !self.encerrado {
            self.encerrar(Duration::from_secs(45));
        }
    }
}

/// Dorme aos pouquinhos, acordando logo se pedirem para parar.
fn cochilar(parar: &AtomicBool, tempo: Duration) {
    let fim = Instant::now() + tempo;
    while !parar.load(Ordering::SeqCst) && Instant::now() < fim {
        thread::sleep(Duration::from_millis(100));
    }
}

impl Motor {
    /// Pede para parar e espera as tarefas até o prazo. true = todas
    /// terminaram (ninguém mais usa o banco). Pode ser chamado de novo.
    pub fn encerrar(&mut self, prazo: Duration) -> bool {
        self.encerrado = true;
        self.parar.store(true, Ordering::SeqCst);
        let fim = Instant::now() + prazo;
        while self.tarefas.iter().any(|t| !t.is_finished()) && Instant::now() < fim {
            thread::sleep(Duration::from_millis(50));
        }
        let (prontas, presas): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.tarefas).into_iter().partition(|t| t.is_finished());
        for t in prontas {
            let _ = t.join();
        }
        let ok = presas.is_empty();
        self.tarefas = presas;
        ok
    }

    /// Só nos testes: uma tarefa a mais para o motor esperar ao encerrar
    /// (simula uma tarefa travada).
    #[cfg(test)]
    pub(crate) fn esperar_tambem(&mut self, tarefa: thread::JoinHandle<()>) {
        self.tarefas.push(tarefa);
    }

    pub fn iniciar(caminho: PathBuf, porta_tcp: u16, porta_udp: u16) -> Motor {
        let parar = Arc::new(AtomicBool::new(false));
        let mut tarefas = Vec::new();

        // 1. Servidor: atende os outros caixas.
        {
            let caminho = caminho.clone();
            let parar = parar.clone();
            tarefas.push(thread::spawn(move || {
                let Ok(ouvinte) = TcpListener::bind(("0.0.0.0", porta_tcp)) else { return };
                ouvinte.set_nonblocking(true).ok();
                let vagas = Arc::new(AtomicUsize::new(0));
                let mut atendendo: Vec<Atendimento> = Vec::new();
                // Quem conecta espera na fila até mandar alguma coisa: conexão
                // muda nunca ocupa vaga nem tarefa. Um caixa de verdade manda o
                // pedido na hora e passa logo para o atendimento.
                let mut fila: Vec<NaFila> = Vec::new();
                while !parar.load(Ordering::SeqCst) {
                    atendendo.retain(|a| !a.tarefa.is_finished());
                    let mut mexeu = false;
                    // 1. Aceita quem chegou.
                    while let Ok((fluxo, origem)) = ouvinte.accept() {
                        mexeu = true;
                        let ip = origem.ip();
                        // Um endereço não tem mais que `VAGAS_POR_ORIGEM` na fila.
                        if fila.iter().filter(|f| f.ip == ip).count() >= VAGAS_POR_ORIGEM {
                            continue;
                        }
                        if fluxo.set_nonblocking(true).is_err() {
                            continue;
                        }
                        // Fila cheia: sai a mais antiga (está muda há mais tempo).
                        if fila.len() >= NA_FILA_MAXIMO {
                            fila.remove(0);
                        }
                        fila.push(NaFila { fluxo, ip, desde: Instant::now() });
                    }
                    // 2. Quem já mandou alguma coisa passa para o atendimento (se
                    // houver vaga, e no máximo `VAGAS_POR_ORIGEM` por endereço);
                    // quem passou do prazo calado, ou fechou, sai da fila.
                    let mut i = 0;
                    while i < fila.len() {
                        let mut espiada = [0u8; 4];
                        let estado = fila[i].fluxo.peek(&mut espiada);
                        let sair = match estado {
                            Ok(0) => true,
                            Ok(_) => {
                                let do_ip = atendendo.iter().filter(|a| a.ip == fila[i].ip).count();
                                if do_ip >= VAGAS_POR_ORIGEM {
                                    false
                                } else if let Some(vaga) = Vaga::pegar_de(&vagas) {
                                    let NaFila { fluxo, ip, .. } = fila.remove(i);
                                    mexeu = true;
                                    fluxo.set_nonblocking(false).ok();
                                    let caminho = caminho.clone();
                                    let parar_tarefa = parar.clone();
                                    let tarefa = thread::Builder::new().name("sync-atender".into()).spawn(move || {
                                        let _vaga = vaga;
                                        if let Err(e) = atender(&caminho, fluxo, &parar_tarefa) {
                                            if matches!(e, Erro::Banco(_) | Erro::Interno(_)) {
                                                crate::registro::anotar(&format!("sync atender: {e}"));
                                            }
                                        }
                                    });
                                    // Sem tarefa nova, a vaga já voltou (ela foi junto e caiu).
                                    match tarefa {
                                        Ok(t) => atendendo.push(Atendimento { ip, tarefa: t }),
                                        Err(e) => crate::registro::anotar(&format!("sync: não abriu tarefa: {e}")),
                                    }
                                    continue;
                                } else {
                                    false
                                }
                            }
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                                ) =>
                            {
                                fila[i].desde.elapsed() > PRAZO_DO_PEDIDO
                            }
                            Err(_) => true,
                        };
                        if sair {
                            fila.remove(i);
                            mexeu = true;
                        } else {
                            i += 1;
                        }
                    }
                    if !mexeu {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                // Parou de aceitar; espera quem já estava sendo atendido (cada
                // conexão tem prazo de leitura e escrita, então termina).
                drop(ouvinte);
                for a in atendendo {
                    let _ = a.tarefa.join();
                }
            }));
        }

        // 2. Descoberta: anuncia este caixa e escuta os outros (UDP).
        {
            let caminho = caminho.clone();
            let parar = parar.clone();
            tarefas.push(thread::spawn(move || {
                let Ok(socket) = UdpSocket::bind(("0.0.0.0", porta_udp)) else { return };
                socket.set_broadcast(true).ok();
                socket.set_read_timeout(Some(Duration::from_secs(1))).ok();
                let mut ultimo_anuncio = Instant::now() - Duration::from_secs(60);
                let mut buf = vec![0u8; 2048];
                while !parar.load(Ordering::SeqCst) {
                    let Ok(conn) = abrir_conexao(&caminho) else {
                        cochilar(&parar, Duration::from_secs(5));
                        continue;
                    };
                    let Ok(a) = ajustes(&conn) else {
                        cochilar(&parar, Duration::from_secs(1));
                        continue;
                    };
                    if !a.ligado || a.senha.is_empty() {
                        cochilar(&parar, Duration::from_secs(3));
                        continue;
                    }
                    // Gravações deste laço (sal novo, par visto) respeitam a
                    // pausa da restauração de backup, como as outras tarefas.
                    let chaves = {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        Chaves::de(&conn, &a.senha)
                    };
                    let Ok(k) = chaves else {
                        cochilar(&parar, Duration::from_secs(1));
                        continue;
                    };
                    if ultimo_anuncio.elapsed() > Duration::from_secs(10) {
                        let ola = json!({ "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_tcp });
                        if let Ok(pacote) = cifrar(&k, ola.to_string().as_bytes()) {
                            let _ = socket.send_to(&pacote, ("255.255.255.255", porta_udp));
                        }
                        ultimo_anuncio = Instant::now();
                    }
                    if let Ok((n, origem)) = socket.recv_from(&mut buf) {
                        // R-09: sal desconhecido só passa pela conta cara
                        // (Argon2) poucas vezes por minuto (veja `decifrar`).
                        if let Ok(claro) = decifrar(&k, &buf[..n], Some(origem.ip())) {
                            if let Ok(v) = serde_json::from_slice::<Value>(&claro) {
                                let d = v["dispositivo"].as_str().unwrap_or_default();
                                let porta = v["porta"].as_u64().unwrap_or(PORTA_TCP as u64);
                                if !d.is_empty() && d != a.dispositivo {
                                    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
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
            }));
        }

        // 3. Puxada: a cada 4 segundos, pede o que há de novo a cada caixa.
        {
            let parar = parar.clone();
            tarefas.push(thread::spawn(move || {
                let mut ultima_limpeza = Instant::now() - Duration::from_secs(50 * 60);
                while !parar.load(Ordering::SeqCst) {
                    cochilar(&parar, Duration::from_secs(4));
                    if parar.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut conn) = abrir_conexao(&caminho) else { continue };
                    if ultima_limpeza.elapsed() > Duration::from_secs(60 * 60) {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        let _ = compactar(&conn);
                        ultima_limpeza = Instant::now();
                    }
                    // Códigos das peças (mesmo com a troca de dados desligada):
                    // o distribuidor cria a faixa dele quando falta e as peças
                    // com código a definir recebem código.
                    {
                        let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = crate::faixas::manter(&mut conn) {
                            if matches!(e, Erro::Banco(_) | Erro::Interno(_)) {
                                crate::registro::anotar(&format!("códigos das peças: {e}"));
                            }
                        }
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
                        if parar.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Err(e) = puxar_ate(&caminho, &alvo, porta_tcp, Some(&parar)) {
                            let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
                            if let Ok(conn) = abrir_conexao(&caminho) {
                                let _ = conn.execute(
                                    "UPDATE sync_par SET erro = ?2 WHERE endereco = ?1",
                                    params![alvo, e.to_string()],
                                );
                            }
                        }
                    }
                }
            }));
        }

        Motor { parar, tarefas, encerrado: false }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{caixa, estoque, produtos, usuarios, vendas};

    fn temp(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-sync-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p.join("rustinho.db")
    }

    /// Pedido de faixa de códigos das peças pela rede: os caixas novos puxam
    /// do distribuidor, pedem a faixa (dois ao mesmo tempo) e já dão códigos
    /// dela; as faixas são diferentes.
    #[test]
    fn faixa_de_codigos_pela_rede() {
        thread::spawn(|| {
            let (pt_a, pu_a) = (48210, 48211);
            let ca = temp("faixa-a");
            let a = Banco::abrir(&ca).unwrap();
            let u = crate::faixas::para_testes::configurar(&a, "Caixa A", 0);
            salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
            let mut ma = Motor::iniciar(ca.clone(), pt_a, pu_a);
            thread::sleep(Duration::from_millis(400));
            let para_a = format!("127.0.0.1:{pt_a}");
            let mut tarefas = Vec::new();
            for (i, nome) in ["Caixa B", "Caixa C"].into_iter().enumerate() {
                let (para_a, u) = (para_a.clone(), u.clone());
                tarefas.push(thread::spawn(move || {
                    let c = temp(&format!("faixa-{i}"));
                    let b = Banco::abrir(&c).unwrap();
                    config::gravar(&b.conn, "caixa_nome", nome).unwrap();
                    salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
                    puxar(&c, &para_a, 48220 + i as u16).unwrap();
                    let p = crate::faixas::para_testes::peca(&b, &u, "Peça nova");
                    (crate::faixas::para_testes::minhas(&b), crate::faixas::para_testes::codigo(&b, &p))
                }));
            }
            let mut r: Vec<(Vec<(i64, i64)>, String)> = tarefas.into_iter().map(|t| t.join().unwrap()).collect();
            r.sort();
            assert_eq!(
                r,
                vec![
                    (vec![(100_000, 199_999)], "100000".to_string()),
                    (vec![(200_000, 299_999)], "200000".to_string())
                ]
            );
            assert!(crate::faixas::cruzadas(&a.conn).unwrap().is_empty());
            assert!(ma.encerrar(Duration::from_secs(20)));
        })
        .join()
        .unwrap();
    }

    /// Banco copiado de outro computador (identidade diferente da guardada):
    /// não responde a quem pede dados nem puxa de ninguém até ser registrado
    /// como caixa novo; depois do registro, troca de novo com os outros.
    #[test]
    fn banco_copiado_nao_troca_dados_pela_rede_ate_ser_registrado() {
        thread::spawn(|| {
            let (pt_a, pu_a) = (48300, 48301);
            let (ca, cb) = (temp("copia-rede-a"), temp("copia-rede-b"));
            let a = Banco::abrir(&ca).unwrap();
            let u = crate::faixas::para_testes::configurar(&a, "Caixa A", 0);
            salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
            crate::faixas::para_testes::peca(&a, &u, "Filtro");
            let b = Banco::abrir(&cb).unwrap();
            config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
            salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
            // A vira "cópia": o banco diz que nasceu em outro computador.
            config::gravar(&a.conn, "computador_id", &crate::computador::impressao("outro-pc")).unwrap();
            crate::faixas::conferir_computador(&a.conn, crate::computador::atual().as_deref()).unwrap();
            assert!(crate::faixas::copia_detectada(&a.conn).unwrap());
            let mut ma = Motor::iniciar(ca.clone(), pt_a, pu_a);
            thread::sleep(Duration::from_millis(400));
            let para_a = format!("127.0.0.1:{pt_a}");
            // Quem pede a ela não recebe nada.
            let r = puxar(&cb, &para_a, 48302);
            assert!(r.is_err(), "{r:?}");
            let produtos_b: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
            assert_eq!(produtos_b, 0);
            // Ela também não puxa de ninguém.
            let r = puxar(&ca, "127.0.0.1:48399", pt_a);
            assert!(matches!(r, Err(Erro::Validacao { .. })), "{r:?}");
            // Registrada como caixa nova, volta a trocar.
            crate::faixas::registrar_caixa_novo(&a.conn, crate::computador::atual().as_deref(), Some("Caixa A2"))
                .unwrap();
            assert!(!crate::faixas::copia_detectada(&a.conn).unwrap());
            puxar(&cb, &para_a, 48302).unwrap();
            let produtos_b: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
            assert_eq!(produtos_b, 1);
            assert!(ma.encerrar(Duration::from_secs(20)));
        })
        .join()
        .unwrap();
    }

    /// Item 1 da revisão: o caixa A volta para trás (backup restaurado no
    /// mesmo computador, computador novo no mesmo endereço, ou banco copiado
    /// à mão) e faz uma alteração nova. O caixa B, que guardava um contador
    /// alto de A, tem de receber a alteração nova; A recupera o que perdeu;
    /// nada é aplicado duas vezes (vendas, itens, estoque), o apagado não
    /// volta, e depois os dois continuam trocando normalmente.
    #[test]
    fn recuo_do_contador_e_ressincronizacao() {
        // Cada cenário numa tarefa própria: o limite de contas caras dos
        // testes é por tarefa, e cada cenário tem caixas novos.
        for (n, cenario) in ["mesmo computador", "computador novo", "copia manual"].into_iter().enumerate() {
            thread::spawn(move || recuo_um_cenario(n, cenario)).join().expect(cenario);
        }
    }

    fn recuo_um_cenario(n: usize, cenario: &str) {
        {
            let (pt_a, pu_a, pt_b, pu_b) =
                (48010 + 10 * n as u16, 48011 + 10 * n as u16, 48012 + 10 * n as u16, 48013 + 10 * n as u16);
            let ca = temp(&format!("recuo-a-{n}"));
            let cb = temp(&format!("recuo-b-{n}"));
            let a = Banco::abrir(&ca).unwrap();
            let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
            usuarios::definir_gerente(&a.conn, &u, true).unwrap();
            config::gravar(&a.conn, "caixa_nome", "Caixa A").unwrap();
            salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
            let b = Banco::abrir(&cb).unwrap();
            config::gravar(&b.conn, "caixa_nome", "Caixa B").unwrap();
            salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
            let mut ma = Motor::iniciar(ca.clone(), pt_a, pu_a);
            let _mb = Motor::iniciar(cb.clone(), pt_b, pu_b);
            thread::sleep(Duration::from_millis(400));
            let (para_a, para_b) = (format!("127.0.0.1:{pt_a}"), format!("127.0.0.1:{pt_b}"));
            let trocar_rede = || {
                for _ in 0..3 {
                    puxar(&cb, &para_a, pt_b).unwrap();
                    puxar(&ca, &para_b, pt_a).unwrap();
                }
            };
            // A tem uma peça com aplicação, faz uma venda; B recebe tudo.
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = "Correia".into();
            f.custo = "10".into();
            f.estoque_inicial = "10".into();
            f.aplicacoes =
                vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
            let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            caixa::abrir(&a.conn, &a.dispositivo, &u, "Caixa A", "0").unwrap();
            let venda = |x: &Banco| {
                let e = vendas::VendaEntrada {
                    itens: vec![vendas::ItemEntrada {
                        produto_id: p.clone(),
                        quantidade: "1".into(),
                        ..Default::default()
                    }],
                    tabela: 1,
                    pagamentos: vec![vendas::PagamentoEntrada {
                        forma: "dinheiro".into(),
                        valor: "100".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                vendas::concluir(&x.conn, &x.dispositivo, &u, &e).unwrap();
            };
            venda(&a);
            trocar_rede();
            // Cópia do banco de A (o "backup" ou a cópia manual).
            let copia = ca.with_file_name("copia.db");
            a.conn.backup(rusqlite::MAIN_DB, &copia, None).unwrap();
            let seq_antes_do_backup: i64 =
                a.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
            // Depois da cópia: A apaga a aplicação, vende de novo, mexe muito
            // (o contador de B para A sobe bem acima do da cópia).
            let mut f = produtos::ficha(&a.conn, &p).unwrap();
            f.aplicacoes.clear();
            produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            venda(&a);
            for i in 0..30 {
                let mut f = produtos::ficha(&a.conn, &p).unwrap();
                f.observacoes = format!("obs {i}");
                produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            }
            trocar_rede();
            let cursor_b: i64 = b
                .conn
                .query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [&a.dispositivo], |l| l.get(0))
                .unwrap();
            assert!(cursor_b > seq_antes_do_backup, "{cenario}: B guardou um contador alto de A");
            // A volta para trás.
            assert!(ma.encerrar(Duration::from_secs(20)));
            let seq_antes: i64 = a.conn.query_row("SELECT MAX(seq) FROM alteracao", [], |l| l.get(0)).unwrap();
            drop(a);
            for fim in ["", "-wal", "-shm"] {
                let mut x = ca.as_os_str().to_owned();
                x.push(fim);
                let _ = std::fs::remove_file(std::path::PathBuf::from(x));
            }
            match cenario {
                "mesmo computador" => {
                    std::fs::copy(&copia, &ca).unwrap();
                    let a = Banco::abrir(&ca).unwrap();
                    depois_de_restaurar(&a.conn, seq_antes).unwrap();
                }
                "copia manual" => {
                    std::fs::copy(&copia, &ca).unwrap();
                }
                _ => {
                    // Computador novo no mesmo endereço: banco vazio, entra na rede.
                    let a = Banco::abrir(&ca).unwrap();
                    config::gravar(&a.conn, "caixa_nome", "Caixa A").unwrap();
                    salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
                }
            }
            let mut a = Banco::abrir(&ca).unwrap();
            let _ma = Motor::iniciar(ca.clone(), pt_a, pu_a);
            thread::sleep(Duration::from_millis(400));
            // A, antes de qualquer troca, faz uma alteração nova.
            let mut g = produtos::ficha_nova(&a.conn).unwrap();
            g.nome = format!("Feita depois de voltar ({cenario})");
            g.custo = "5".into();
            // Sem faixa de códigos ainda: a peça fica com código a definir
            // (antes das faixas, o computador novo dava o código 1, que já
            // era de outra peça na loja).
            let nova = produtos::salvar(&a.conn, &a.dispositivo, &u, &g).unwrap_or_else(|_| {
                // Computador novo ainda não tem o usuário: cria um.
                let u2 = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "1234", true).unwrap().id;
                produtos::salvar(&a.conn, &a.dispositivo, &u2, &g).unwrap()
            });
            // B pede a A com o contador antigo: tem de receber a alteração nova.
            puxar(&cb, &para_a, pt_b).unwrap();
            assert!(produtos::ficha(&b.conn, &nova).is_ok(), "{cenario}: B recebeu a alteração nova de A");
            trocar_rede();
            let contar = |x: &Banco, sql: &str| -> i64 { x.conn.query_row(sql, [], |l| l.get(0)).unwrap() };
            for (quem, x) in [("A", &a), ("B", &b)] {
                assert_eq!(
                    contar(x, "SELECT COUNT(*) FROM venda WHERE tipo = 'venda'"),
                    2,
                    "{cenario} {quem}: duas vendas, sem repetir"
                );
                assert_eq!(contar(x, "SELECT COUNT(*) FROM venda_item"), 2, "{cenario} {quem}");
                assert_eq!(estoque::saldo(&x.conn, &p).unwrap(), 8000, "{cenario} {quem}: estoque certo");
                assert!(
                    produtos::ficha(&x.conn, &p).unwrap().aplicacoes.is_empty(),
                    "{cenario} {quem}: o apagado não volta"
                );
                assert_eq!(produtos::ficha(&x.conn, &p).unwrap().observacoes, "obs 29", "{cenario} {quem}");
                assert!(produtos::ficha(&x.conn, &nova).is_ok(), "{cenario} {quem}");
                assert_eq!(pendencias(&x.conn).unwrap(), 0, "{cenario} {quem}: nada apontando para o que falta");
                assert_eq!(
                    conflitos_abertos(&x.conn).unwrap(),
                    0,
                    "{cenario} {quem}: sem aviso falso: {:?}",
                    x.conn
                        .prepare("SELECT tabela, chave, ficou, perdeu, certeza, dados_perdeu FROM sync_conflito")
                        .unwrap()
                        .query_map([], |l| Ok(format!(
                            "{:?}",
                            (
                                l.get::<_, String>(0)?,
                                l.get::<_, String>(1)?,
                                l.get::<_, String>(4)?,
                                l.get::<_, Option<String>>(5)?
                            )
                        )))
                        .unwrap()
                        .collect::<Vec<_>>()
                );
            }
            // Depois, os dois continuam trocando alterações novas.
            let mut h = produtos::ficha_nova(&b.conn).unwrap();
            h.nome = "Nova de B".into();
            h.custo = "1".into();
            let de_b = produtos::salvar(&b.conn, &b.dispositivo, &u, &h).unwrap();
            let mut f = produtos::ficha(&a.conn, &nova).unwrap();
            f.nome = "Nova de A, mexida".into();
            let quem_salva = usuarios::listar(&a.conn, false).unwrap()[0].id.clone();
            produtos::salvar(&a.conn, &a.dispositivo, &quem_salva, &f).unwrap();
            trocar_rede();
            assert!(produtos::ficha(&a.conn, &de_b).is_ok(), "{cenario}: A recebe de B");
            assert_eq!(produtos::ficha(&b.conn, &nova).unwrap().nome, "Nova de A, mexida", "{cenario}: B recebe de A");
            let _ = &mut a;
        }
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

    /// R-09: rajada de pacotes com sais inventados, de um endereço e de
    /// muitos: um endereço só faz no máximo `SAIS_NOVOS_POR_ORIGEM` contas
    /// caras por minuto e todos juntos no máximo `SAIS_NOVOS_POR_MINUTO`; o
    /// controle não cresce sem fim; um caixa novo de verdade, de outro
    /// endereço, ainda entra; os caixas conhecidos continuam sem gastar nada;
    /// pacote repetido não gasta de novo depois de aceito.
    #[test]
    fn rajada_de_sais_inventados() {
        use std::net::{IpAddr, Ipv4Addr};
        let ip = |n: u32| Some(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + n)));
        let b = Banco::em_memoria().unwrap();
        let k1 = Chaves::de(&b.conn, "senha-da-loja").unwrap();
        let falso = |i: u32| {
            let mut p = MAGICA.to_vec();
            let mut sal = [0u8; 16];
            sal[..4].copy_from_slice(&i.to_be_bytes());
            sal[4] = 0xAB;
            p.extend(sal);
            p.extend([0u8; 12]);
            p.extend([1u8; 40]);
            p
        };
        // Um caixa de verdade conversa antes da rajada (o sal dele fica guardado).
        let sal_bom = [7u8; 16];
        let k2 = Chaves {
            senha: "senha-da-loja".into(),
            sal: sal_bom,
            propria: derivar("senha-da-loja", &sal_bom).unwrap(),
        };
        assert_eq!(decifrar(&k1, &cifrar(&k2, b"ola").unwrap(), ip(1)).unwrap(), b"ola");
        // Rajada de um endereço só: só 3 contas caras.
        let inicio = Instant::now();
        let recusados =
            (0..300).filter(|i| decifrar(&k1, &falso(*i), ip(66)).unwrap_err().contains("Muitos pedidos")).count();
        assert_eq!(recusados, 300 - SAIS_NOVOS_POR_ORIGEM as usize);
        // Um caixa novo de verdade, de outro endereço, ainda entra.
        let sal_novo = [8u8; 16];
        let k3 = Chaves {
            senha: "senha-da-loja".into(),
            sal: sal_novo,
            propria: derivar("senha-da-loja", &sal_novo).unwrap(),
        };
        assert_eq!(decifrar(&k1, &cifrar(&k3, b"cheguei").unwrap(), ip(2)).unwrap(), b"cheguei");
        // Rajada de muitos endereços inventados: o total de contas para no limite de todos.
        let recusados = (0..2000u32)
            .filter(|i| decifrar(&k1, &falso(10_000 + i), ip(1000 + i)).unwrap_err().contains("Muitos pedidos"))
            .count();
        let feitas = SAIS_NOVOS_POR_MINUTO as usize - 1 - SAIS_NOVOS_POR_ORIGEM as usize - 1;
        assert_eq!(recusados, 2000 - feitas);
        assert!(inicio.elapsed() < Duration::from_secs(20), "{:?}", inicio.elapsed());
        // Os conhecidos e o próprio caixa continuam (sem gastar o limite).
        assert_eq!(decifrar(&k1, &cifrar(&k2, b"de novo").unwrap(), ip(1)).unwrap(), b"de novo");
        assert_eq!(decifrar(&k1, &cifrar(&k3, b"eu de novo").unwrap(), ip(2)).unwrap(), b"eu de novo");
        assert_eq!(decifrar(&k1, &cifrar(&k1, b"eu").unwrap(), None).unwrap(), b"eu");
        // O controle não cresce sem fim e volta depois de um minuto.
        let mut p = Porteiro::novo();
        for i in 0..10_000u32 {
            p.permitir(Instant::now(), ip(i));
        }
        assert!(p.por_origem.len() <= ORIGENS_POR_MINUTO);
        assert!(!p.permitir(Instant::now(), ip(99_999)));
        assert!(p.permitir(Instant::now() + Duration::from_secs(61), ip(99_999)));
        assert_eq!(p.por_origem.len(), 1);
    }

    /// Conexão TCP saindo de um endereço escolhido (127.0.0.x), para simular
    /// aparelhos diferentes na rede.
    #[cfg(unix)]
    fn conectar_de(origem: [u8; 4], porta: u16) -> std::io::Result<TcpStream> {
        use std::os::fd::FromRawFd;
        // SAFETY: chamadas de socket do sistema com estruturas zeradas e do
        // tamanho certo; o descritor vira um TcpStream (dono) ou é fechado.
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let tam = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            let mut de: libc::sockaddr_in = std::mem::zeroed();
            de.sin_family = libc::AF_INET as libc::sa_family_t;
            de.sin_addr.s_addr = u32::from_ne_bytes(origem);
            if libc::bind(fd, &de as *const libc::sockaddr_in as *const libc::sockaddr, tam) != 0 {
                let e = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(e);
            }
            let mut para: libc::sockaddr_in = std::mem::zeroed();
            para.sin_family = libc::AF_INET as libc::sa_family_t;
            para.sin_port = porta.to_be();
            para.sin_addr.s_addr = u32::from_ne_bytes([127, 0, 0, 1]);
            if libc::connect(fd, &para as *const libc::sockaddr_in as *const libc::sockaddr, tam) != 0 {
                let e = std::io::Error::last_os_error();
                libc::close(fd);
                return Err(e);
            }
            Ok(TcpStream::from_raw_fd(fd))
        }
    }

    /// A conexão foi fechada pelo servidor (lê fim de arquivo logo)?
    #[cfg(unix)]
    fn fechada(f: &mut TcpStream) -> bool {
        f.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let mut b = [0u8; 1];
        match f.read(&mut b) {
            Ok(0) => true,
            Ok(_) => false,
            Err(e) => !matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut),
        }
    }

    /// Item 5 da revisão: conexões que não mandam nada. Um endereço não
    /// ocupa mais que `VAGAS_POR_ORIGEM` vagas; quem fica mudo cai em
    /// `PRAZO_DO_PEDIDO`; com tudo ocupado por conexões mudas de vários
    /// endereços (e outras chegando sem parar), o caixa de verdade ainda
    /// consegue trocar dados; o servidor nunca passa de `CONEXOES_MAXIMAS`.
    #[cfg(unix)]
    #[test]
    fn conexoes_mudas_nao_tomam_o_servidor() {
        let ca = temp("mudas-a");
        let a = Banco::abrir(&ca).unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Peça do servidor".into();
        f.custo = "1".into();
        let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        salvar_ajustes(&a.conn, true, "senha-da-loja", "").unwrap();
        drop(a);
        let cb = temp("mudas-b");
        let b = Banco::abrir(&cb).unwrap();
        salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
        let (pt, pu) = (48110, 48111);
        let _m = Motor::iniciar(ca.clone(), pt, pu);
        thread::sleep(Duration::from_millis(400));
        // Um endereço só abre 10 conexões mudas: só 2 ficam abertas.
        let mut de_um: Vec<TcpStream> = (0..10).filter_map(|_| conectar_de([127, 0, 0, 66], pt).ok()).collect();
        thread::sleep(Duration::from_millis(200));
        let abertas = de_um.iter_mut().map(|c| !fechada(c)).filter(|aberta| *aberta).count();
        assert_eq!(abertas, VAGAS_POR_ORIGEM, "um endereço só ocupa {VAGAS_POR_ORIGEM} vagas");
        // Quem fica mudo cai no prazo.
        thread::sleep(PRAZO_DO_PEDIDO + Duration::from_millis(600));
        assert!(de_um.iter_mut().all(fechada), "as mudas caíram no prazo");
        // Muitos endereços mudos ocupam todas as vagas e continuam chegando.
        let parar = Arc::new(AtomicBool::new(false));
        let parar_ataque = parar.clone();
        let ataque = thread::spawn(move || {
            let mut abertas: Vec<TcpStream> = Vec::new();
            let mut n = 0u32;
            while !parar_ataque.load(Ordering::SeqCst) {
                let ip = [127, 0, 1 + (n % 20) as u8, 10 + (n / 20 % 200) as u8];
                if let Ok(c) = conectar_de(ip, pt) {
                    abertas.push(c);
                }
                n += 1;
                if abertas.len() > 200 {
                    abertas.drain(..100);
                }
                thread::sleep(Duration::from_millis(15));
            }
            n
        });
        thread::sleep(Duration::from_millis(800));
        // O caixa de verdade troca dados mesmo assim (várias vezes).
        let t = Instant::now();
        for _ in 0..3 {
            puxar(&cb, &format!("127.0.0.1:{pt}"), 1).unwrap();
        }
        let tempo = t.elapsed();
        parar.store(true, Ordering::SeqCst);
        let tentativas = ataque.join().unwrap();
        assert!(produtos::ficha(&b.conn, &p).is_ok(), "o caixa de verdade recebeu os dados");
        assert!(tempo < Duration::from_secs(15), "{tempo:?}");
        assert!(tentativas > 50, "o ataque mandou {tentativas} conexões");
        println!("conexões mudas durante o teste: {tentativas}; 3 trocas de dados do caixa de verdade em {tempo:?}");
    }

    /// R-06: fechar no meio de uma puxada travada (o outro caixa aceita a
    /// conexão e não responde nada) ou com erro de rede (ninguém escutando):
    /// encerra logo, sem esperar os 30 s de prazo da rede.
    #[test]
    fn encerrar_com_rede_lenta_ou_com_erro() {
        // "Caixa" que aceita a conexão e fica mudo.
        let mudo = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let porta_muda = mudo.local_addr().unwrap().port();
        let segura = thread::spawn(move || {
            let conexoes: Vec<_> = mudo.incoming().take(1).flatten().collect();
            thread::sleep(Duration::from_secs(8));
            drop(conexoes);
        });
        // Porta sem ninguém (erro de rede).
        let porta_fechada = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        let c = temp("rede-lenta");
        let b = Banco::abrir(&c).unwrap();
        salvar_ajustes(&b.conn, true, "senha-da-loja", &format!("127.0.0.1:{porta_fechada}, 127.0.0.1:{porta_muda}"))
            .unwrap();
        drop(b);
        let mut m = Motor::iniciar(c.clone(), 47950, 47951);
        // A puxada roda a cada 4 s: espera ela ficar presa no caixa mudo.
        thread::sleep(Duration::from_millis(5500));
        let t = Instant::now();
        assert!(m.encerrar(Duration::from_secs(20)));
        assert!(t.elapsed() < Duration::from_secs(3), "parou logo: {:?}", t.elapsed());
        segura.join().unwrap();
    }

    /// R-06: ao encerrar, o motor espera a conexão que está sendo atendida
    /// (mesmo lenta) antes de devolver; com prazo curto demais, avisa que
    /// ainda tem tarefa rodando (e o serviço não solta a trava).
    #[test]
    fn encerrar_espera_as_tarefas() {
        let c = temp("encerrar");
        let b = Banco::abrir(&c).unwrap();
        salvar_ajustes(&b.conn, true, "senha-da-loja", "").unwrap();
        drop(b);
        let (pt, pu) = (47940, 47941);
        // Sem nenhuma conexão: encerra logo.
        let mut m = Motor::iniciar(c.clone(), pt, pu);
        thread::sleep(Duration::from_millis(300));
        let t = Instant::now();
        assert!(m.encerrar(Duration::from_secs(10)));
        assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
        drop(m);
        thread::sleep(Duration::from_millis(300));
        // Com um caixa lento conectado (não manda nada): o atendimento para
        // logo ao pedir para encerrar (não espera os 5 s de prazo dele), e só
        // então o motor devolve. Nenhuma tarefa fica para trás.
        let mut m = Motor::iniciar(c.clone(), pt, pu);
        thread::sleep(Duration::from_millis(400));
        let lento = TcpStream::connect(("127.0.0.1", pt)).unwrap();
        thread::sleep(Duration::from_millis(500));
        let t = Instant::now();
        assert!(m.encerrar(Duration::from_secs(20)));
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        assert!(m.tarefas.is_empty());
        // Com uma tarefa que não termina: prazo curto devolve "não deu",
        // e a tarefa continua guardada (quem chamou segura a trava do banco).
        let mut m = Motor::iniciar(c.clone(), pt, pu);
        let (solta, espera) = std::sync::mpsc::channel::<()>();
        m.esperar_tambem(thread::spawn(move || {
            let _ = espera.recv();
        }));
        assert!(!m.encerrar(Duration::from_millis(300)));
        assert_eq!(m.tarefas.len(), 1);
        solta.send(()).unwrap();
        assert!(m.encerrar(Duration::from_secs(5)));
        drop(lento);
    }

    /// R-05: as três situações de referência quebrada, cada uma com o
    /// registro, o que falta, o estado e a ação; a que espera resolve
    /// sozinha quando o registro chega (fora de ordem, repetido); a espera
    /// longa vira inconsistente. Nada é apagado.
    #[test]
    fn referencias_esperando_apagadas_e_inconsistentes() {
        // Caixa A cria uma peça com aplicação; B recebe a aplicação antes da peça.
        let a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Correia".into();
        f.custo = "10".into();
        f.aplicacoes =
            vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
        let pid = produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap();
        let (tudo, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        let sem_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela != "produto").cloned().collect();
        let so_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela == "produto").cloned().collect();
        let mut b = Banco::em_memoria().unwrap();
        assert_eq!(registrar_referencias(&b.conn).unwrap(), Referencias::default());
        aplicar(&mut b.conn, &sem_a_peca).unwrap();
        aplicar(&mut b.conn, &sem_a_peca).unwrap(); // repetido
        let r = registrar_referencias(&b.conn).unwrap();
        assert!(r.esperando >= 1 && r.quebradas == 0, "{r:?}");
        let lista = referencias(&b.conn).unwrap();
        let ap = lista.iter().find(|x| x.tabela == "produto_aplicacao").expect("a aplicação aparece");
        assert_eq!((ap.estado.as_str(), ap.pai.as_str()), ("esperando", "produto"));
        assert!(ap.falta.contains(&pid) && ap.acao.contains("resolve sozinho"));
        // Pai apagado (o log diz) e inconsistente (o log diz que existe, a tabela não tem).
        let sem_gatilho = |sql: &str| {
            b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", false).unwrap();
            b.conn.execute(sql, []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", true).unwrap();
            b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
        };
        sem_gatilho("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap2', 'peca-apagada')");
        sem_gatilho("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap3', 'peca-fantasma')");
        for (id, peca, op) in [
            ("01a00000-0000-7000-8000-000000000001", "peca-apagada", "apagar"),
            ("01a00000-0000-7000-8000-000000000002", "peca-fantasma", "gravar"),
        ] {
            b.conn
                .execute(
                    "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
                     VALUES (?1, 'produto', json_array(?2), ?3, NULL, 'outro', '2026-01-01', 2)",
                    params![id, peca, op],
                )
                .unwrap();
        }
        registrar_referencias(&b.conn).unwrap();
        let estado = |b: &Banco, chave: &str| {
            referencias(&b.conn).unwrap().into_iter().find(|x| x.chave.contains(chave)).map(|x| x.estado)
        };
        assert_eq!(estado(&b, "ap2").as_deref(), Some("apagado"));
        assert_eq!(estado(&b, "ap3").as_deref(), Some("inconsistente"));
        assert!(referencias(&b.conn)
            .unwrap()
            .iter()
            .find(|x| x.chave.contains("ap2"))
            .unwrap()
            .acao
            .contains("Nada foi apagado"));
        // Esperando há muito tempo, sem prova nenhuma: continua ESPERANDO (R-05
        // revisado). O tempo sozinho não torna nada inconsistente; só ganha o
        // marcador "atrasada".
        sem_gatilho("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap4', 'peca-que-nunca-vem')");
        registrar_referencias(&b.conn).unwrap();
        b.conn
            .execute("UPDATE sync_referencia SET desde = '2020-01-01T00:00:00Z' WHERE chave LIKE '%ap4%'", [])
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado(&b, "ap4").as_deref(), Some("esperando"));
        assert!(referencias(&b.conn).unwrap().iter().find(|x| x.chave.contains("ap4")).unwrap().atrasada);
        assert_eq!(config::ler(&b.conn, "sync_quebradas").unwrap().as_deref(), Some("2"));
        assert_eq!(config::ler(&b.conn, "sync_atrasadas").unwrap().as_deref(), Some("1"));
        // A peça chega depois (e chega de novo): a espera some sozinha.
        aplicar(&mut b.conn, &so_a_peca).unwrap();
        aplicar(&mut b.conn, &so_a_peca).unwrap();
        let r = registrar_referencias(&b.conn).unwrap();
        assert_eq!(r, Referencias { esperando: 1, quebradas: 2, atrasadas: 1 });
        assert!(referencias(&b.conn).unwrap().iter().all(|x| !x.falta.contains(&pid)), "resolvida sozinha");
        assert_eq!(produtos::ficha(&b.conn, &pid).unwrap().aplicacoes.len(), 1);
        // Nada foi apagado para esconder.
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM produto_aplicacao", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 4);
        let total: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_referencia", [], |l| l.get(0)).unwrap();
        assert_eq!(total, 3, "uma linha por referência, sem repetir");
        // A faixa do Início avisa.
        assert!(crate::avisos::do_dia(&b.conn).unwrap().iter().any(|a| a.id == "referencias"));
    }

    /// Item 3 da revisão: a espera de uma referência guarda a data do
    /// começo enquanto for o mesmo registro faltando; recomeça quando passa
    /// a faltar outro; some quando o registro que falta chega.
    #[test]
    fn espera_recomeca_quando_o_que_falta_muda() {
        // Outro caixa tem as peças X e Y; este ainda não recebeu nenhuma.
        let a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
        let mut ids = Vec::new();
        for n in ["Peça X", "Peça Y"] {
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = n.into();
            f.custo = "1".into();
            ids.push(produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap());
        }
        let (x, y) = (ids[0].clone(), ids[1].clone());
        let (tudo, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        let da_y: Vec<_> = tudo
            .iter()
            .filter(|v| v.chave.contains(&y) || v.dados.as_deref().is_some_and(|d| d.contains(&y)))
            .cloned()
            .collect();
        let mut b = Banco::em_memoria().unwrap();
        let sem_gatilho = |b: &Banco, sql: &str| {
            b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", false).unwrap();
            b.conn.execute(sql, []).unwrap();
            b.conn.pragma_update(None, "foreign_keys", true).unwrap();
            b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
        };
        let ler = |b: &Banco| -> (String, String, String) {
            b.conn
                .query_row("SELECT falta, estado, desde FROM sync_referencia WHERE chave LIKE '%ap1%'", [], |l| {
                    Ok((l.get(0)?, l.get(1)?, l.get(2)?))
                })
                .unwrap()
        };
        sem_gatilho(&b, &format!("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap1', '{x}')"));
        registrar_referencias(&b.conn).unwrap();
        // Esperando há quase 24 horas.
        b.conn
            .execute("UPDATE sync_referencia SET desde = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-23 hours')", [])
            .unwrap();
        let (_, _, quase) = ler(&b);
        // Conferir de novo (outra troca de dados): a data do começo não muda.
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(ler(&b), (format!(r#"["{x}"]"#), "esperando".into(), quase.clone()));
        // Passa a apontar para a outra peça, que também não chegou: conta do zero.
        sem_gatilho(&b, &format!("UPDATE produto_aplicacao SET produto_id = '{y}' WHERE id = 'ap1'"));
        registrar_referencias(&b.conn).unwrap();
        let (falta, estado, desde) = ler(&b);
        assert_eq!((falta, estado.as_str()), (format!(r#"["{y}"]"#), "esperando"));
        assert!(desde > quase, "a nova referência começou a contar agora: {desde} > {quase}");
        // Uma hora depois ainda é "esperando" (a anterior já passaria de 24 h).
        b.conn
            .execute("UPDATE sync_referencia SET desde = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 hours')", [])
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(ler(&b).1, "esperando");
        // A peça Y chega pela troca de dados: a pendência some.
        aplicar(&mut b.conn, &da_y).unwrap();
        registrar_referencias(&b.conn).unwrap();
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_referencia", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 0, "{:?}", referencias(&b.conn).unwrap());
    }

    // ------------------------------------------------------------------
    // R-05 (revisado): atraso não é inconsistência; só a prova bloqueia.
    // ------------------------------------------------------------------

    /// Grava direto, sem gatilho nem chave estrangeira (simula o que a troca
    /// de dados deixa para trás).
    fn sem_gatilho(b: &Banco, sql: &str) {
        b.conn.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", []).unwrap();
        b.conn.pragma_update(None, "foreign_keys", false).unwrap();
        b.conn.execute(sql, []).unwrap();
        b.conn.pragma_update(None, "foreign_keys", true).unwrap();
        b.conn.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", []).unwrap();
    }

    fn estado_de(b: &Banco, id: &str) -> (String, String) {
        b.conn
            .query_row("SELECT estado, motivo FROM sync_referencia WHERE chave = json_array(?1)", [id], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap()
    }

    fn banco_com_aplicacao_sem_peca(id: &str) -> Banco {
        let b = Banco::em_memoria().unwrap();
        sem_gatilho(
            &b,
            &format!("INSERT INTO produto_aplicacao (id, produto_id) VALUES ('{id}', 'peca-ausente-{id}')"),
        );
        registrar_referencias(&b.conn).unwrap();
        b
    }

    fn envelhecer(b: &Banco, id: &str) {
        b.conn
            .execute("UPDATE sync_referencia SET desde = '2020-01-01T00:00:00Z' WHERE chave = json_array(?1)", [id])
            .unwrap();
    }

    fn par_com_troca(b: &Banco, dispositivo: &str, completo_em: Option<&str>) {
        b.conn
            .execute(
                "INSERT OR REPLACE INTO sync_par (dispositivo, nome, endereco, completo_em) VALUES (?1, ?1, '', ?2)",
                params![dispositivo, completo_em],
            )
            .unwrap();
    }

    /// R5-T1 · R5-1, R5-2, R5-4: 30 dias esperando e nenhum caixa conhecido: continua
    /// esperando (atrasada), não bloqueia nada.
    #[test]
    fn r5_t1_tempo_sozinho_nunca_e_prova() {
        let b = banco_com_aplicacao_sem_peca("ap1");
        envelhecer(&b, "ap1");
        let r = registrar_referencias(&b.conn).unwrap();
        assert_eq!(r, Referencias { esperando: 1, quebradas: 0, atrasadas: 1 });
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        let l = referencias(&b.conn).unwrap();
        assert!(l[0].atrasada && !l[0].pode_aceitar && l[0].acao.contains("não bloqueia"), "{:?}", l[0]);
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        // Recente: esperando, mas ainda não atrasada.
        let n = banco_com_aplicacao_sem_peca("ap9");
        assert!(!referencias(&n.conn).unwrap()[0].atrasada);
        assert_eq!(config::ler(&n.conn, "sync_atrasadas").unwrap().as_deref(), Some("0"));
    }

    /// R5-T2 · R5-3, R5-7: o log diz que existe e a tabela não tem: inconsistente (log).
    #[test]
    fn r5_t2_prova_do_log_bloqueia() {
        let b = banco_com_aplicacao_sem_peca("ap3");
        b.conn
            .execute(
                "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
                 VALUES ('01a00000-0000-7000-8000-0000000000aa', 'produto', json_array('peca-ausente-ap3'), 'gravar', NULL, 'outro', '2026-01-01', 2)",
                [],
            )
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap3"), ("inconsistente".into(), "log".into()));
        let e = exigir_editavel(&b.conn, "produto_aplicacao", "ap3").unwrap_err().to_string();
        assert!(e.contains("inconsistente") && e.contains("gerente"), "{e}");
        assert!(referencias(&b.conn).unwrap()[0].pode_aceitar);
        // Outro registro da mesma tabela passa.
        exigir_editavel(&b.conn, "produto_aplicacao", "ap-outro").unwrap();
    }

    /// R5-T3 · R5-4, R5-5: a prova dos pares exige TODOS os caixas conhecidos com
    /// troca completa DEPOIS de a referência aparecer; um par novo desfaz a prova.
    #[test]
    fn r5_t3_prova_dos_pares_exige_todos() {
        let b = banco_com_aplicacao_sem_peca("ap1");
        let depois = "2999-01-01T00:00:00Z";
        // Nenhum par conhecido: sem prova.
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        // Dois pares, nenhum com troca completa: sem prova.
        par_com_troca(&b, "p1", None);
        par_com_troca(&b, "p2", None);
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        // Só um completou depois: sem prova.
        par_com_troca(&b, "p1", Some(depois));
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        // O outro completou, mas ANTES de a referência aparecer: sem prova.
        par_com_troca(&b, "p2", Some("2000-01-01T00:00:00Z"));
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "esperando");
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        // Os dois completaram depois: comprovado pelos pares.
        par_com_troca(&b, "p2", Some(depois));
        let r = registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1"), ("inconsistente".into(), "pares".into()));
        assert_eq!(r.quebradas, 1);
        let e = exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap_err().to_string();
        assert!(e.contains("todos os outros caixas"), "{e}");
        // Aparece um caixa novo, que ainda não trocou nada: a prova se desfaz
        // e o registro volta a funcionar, sem ninguém aceitar nada.
        par_com_troca(&b, "p3", None);
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1"), ("esperando".into(), "".into()));
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
    }

    /// R5-T4/T5 · R5-6, R5-7: o pai chega e o bloqueio some sozinho; apagado não bloqueia.
    #[test]
    fn r5_t4_pai_que_chega_desbloqueia_e_apagado_nao_bloqueia() {
        // Caixa A cria a peça com uma aplicação; B recebe a aplicação antes da peça.
        let a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Correia".into();
        f.custo = "10".into();
        f.aplicacoes =
            vec![produtos::AplicacaoFicha { montadora: "Fiat".into(), modelo: "Uno".into(), ..Default::default() }];
        produtos::salvar(&a.conn, &a.dispositivo, &u.id, &f).unwrap();
        let (tudo, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        let sem_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela != "produto").cloned().collect();
        let so_a_peca: Vec<_> = tudo.iter().filter(|x| x.tabela == "produto").cloned().collect();
        let mut b = Banco::em_memoria().unwrap();
        aplicar(&mut b.conn, &sem_a_peca).unwrap();
        registrar_referencias(&b.conn).unwrap();
        let ap: String = b.conn.query_row("SELECT id FROM produto_aplicacao", [], |l| l.get(0)).unwrap();
        par_com_troca(&b, "p1", Some("2999-01-01T00:00:00Z"));
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, &ap).0, "inconsistente");
        exigir_editavel(&b.conn, "produto_aplicacao", &ap).unwrap_err();
        // O pai chega: a linha e o bloqueio somem sozinhos.
        aplicar(&mut b.conn, &so_a_peca).unwrap();
        registrar_referencias(&b.conn).unwrap();
        let n: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_referencia", [], |l| l.get(0)).unwrap();
        assert_eq!(n, 0, "{:?}", referencias(&b.conn).unwrap());
        exigir_editavel(&b.conn, "produto_aplicacao", &ap).unwrap();
        // Apagado: o log diz "apagar"; a correção (editar) tem de passar.
        sem_gatilho(&b, "INSERT INTO produto_aplicacao (id, produto_id) VALUES ('ap2', 'peca-apagada')");
        b.conn
            .execute(
                "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, ordem)
                 VALUES ('01a00000-0000-7000-8000-0000000000bb', 'produto', json_array('peca-apagada'), 'apagar', NULL, 'outro', '2026-01-01', 2)",
                [],
            )
            .unwrap();
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap2").0, "apagado");
        exigir_editavel(&b.conn, "produto_aplicacao", "ap2").unwrap();
    }

    /// R5-T8 · R5-9: aceite do gerente (motivo obrigatório, só inconsistente, mesma falta).
    #[test]
    fn r5_t8_aceite_do_gerente() {
        let b = banco_com_aplicacao_sem_peca("ap1");
        let chave = r#"["ap1"]"#;
        // Esperando não pode ser aceito (não há prova de nada).
        let e = aceitar_inconsistencia(&b.conn, "u1", "produto_aplicacao", chave, "motivo bom").unwrap_err();
        assert!(e.to_string().contains("não está inconsistente"), "{e}");
        par_com_troca(&b, "p1", Some("2999-01-01T00:00:00Z"));
        registrar_referencias(&b.conn).unwrap();
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap_err();
        // Sem motivo: recusa.
        for m in ["", "  ", "abc"] {
            let e = aceitar_inconsistencia(&b.conn, "u1", "produto_aplicacao", chave, m).unwrap_err();
            assert_eq!(e.nome_do_campo(), Some("motivo"), "{m:?}");
        }
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap_err();
        // Com motivo: libera, guarda quem/quando/por quê.
        usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap();
        let ana: String = b.conn.query_row("SELECT id FROM usuario WHERE nome = 'Ana'", [], |l| l.get(0)).unwrap();
        assert_eq!(
            aceitar_inconsistencia(&b.conn, &ana, "produto_aplicacao", chave, "Caixa antigo foi perdido").unwrap(),
            1
        );
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        let l = referencias(&b.conn).unwrap();
        assert_eq!(l[0].estado, "aceito");
        assert_eq!((l[0].aceita_por.as_str(), l[0].aceite_motivo.as_str()), ("Ana", "Caixa antigo foi perdido"));
        assert!(!l[0].pode_aceitar && l[0].acao.contains("aceitou"));
        // Conferir de novo não desfaz o aceite (mesma falta).
        registrar_referencias(&b.conn).unwrap();
        assert_eq!(estado_de(&b, "ap1").0, "aceito");
        exigir_editavel(&b.conn, "produto_aplicacao", "ap1").unwrap();
        // Passa a faltar OUTRA coisa: o aceite não vale, é avaliado de novo.
        sem_gatilho(&b, "UPDATE produto_aplicacao SET produto_id = 'outra-peca' WHERE id = 'ap1'");
        registrar_referencias(&b.conn).unwrap();
        assert_ne!(estado_de(&b, "ap1").0, "aceito");
        let aceita: Option<String> = b
            .conn
            .query_row("SELECT aceita_por FROM sync_referencia WHERE chave = json_array('ap1')", [], |l| l.get(0))
            .unwrap();
        assert!(aceita.is_none());
    }

    /// R5-T10 · R5-11: a troca completa grava `completo_em`; o recomeço do zero apaga.
    #[test]
    fn r5_t10_troca_completa_marca_o_par() {
        let c1 = temp("r5-c1");
        let b1 = Banco::abrir(&c1).unwrap();
        let u = usuarios::criar(&b1.conn, &b1.dispositivo, "Maria", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&b1.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        produtos::salvar(&b1.conn, &b1.dispositivo, &u.id, &f).unwrap();
        salvar_ajustes(&b1.conn, true, "senha-da-loja", "").unwrap();
        let c2 = temp("r5-c2");
        let b2 = Banco::abrir(&c2).unwrap();
        salvar_ajustes(&b2.conn, true, "senha-da-loja", "").unwrap();
        let (pt1, pu1) = (48300, 48301);
        let _m1 = Motor::iniciar(c1.clone(), pt1, pu1);
        thread::sleep(Duration::from_millis(400));
        let completo = |b: &Banco| -> Option<String> {
            b.conn
                .query_row("SELECT completo_em FROM sync_par WHERE dispositivo = ?1", [&b1.dispositivo], |l| l.get(0))
                .unwrap()
        };
        puxar(&c2, &format!("127.0.0.1:{pt1}"), 48310).unwrap();
        assert!(completo(&b2).is_some(), "depois de receber tudo, a troca está completa");
        // Restaurar um backup recomeça do zero: a prova some até nova troca completa.
        depois_de_restaurar(&b2.conn, 0).unwrap();
        assert!(completo(&b2).is_none());
        puxar(&c2, &format!("127.0.0.1:{pt1}"), 48310).unwrap();
        assert!(completo(&b2).is_some());
    }

    // ------------------------------------------------------------------
    // R-08: conflitos sem depender do relógio.
    // ------------------------------------------------------------------

    /// Leva de `de` para `para` tudo o que `de` tem (em lotes, como pela rede).
    fn levar(de: &Banco, para: &mut Banco) -> usize {
        let (mut desde, mut total) = (0, 0);
        loop {
            let (lista, mais) = alteracoes_desde(&de.conn, desde, 40).unwrap();
            if let Some(u) = lista.last() {
                desde = u.seq;
            }
            total += aplicar(&mut para.conn, &lista).unwrap();
            if !mais {
                return total;
            }
        }
    }

    fn trocar(a: &mut Banco, b: &mut Banco) {
        for _ in 0..6 {
            if levar(a, b) + levar(b, a) == 0 {
                return;
            }
        }
        panic!("os caixas não pararam de trocar");
    }

    /// Faz `f` com o relógio deste caixa adiantado/atrasado `ms`.
    fn com_relogio<T>(ms: i64, f: impl FnOnce() -> T) -> T {
        tempo::RELOGIO_DE_TESTE.with(|c| c.set(ms));
        let r = f();
        tempo::RELOGIO_DE_TESTE.with(|c| c.set(0));
        r
    }

    const MIN: i64 = 60_000;
    const HORA: i64 = 60 * MIN;
    const DIA: i64 = 24 * HORA;

    /// Loja com dois caixas e uma peça igual nos dois.
    fn dois_caixas() -> (Banco, Banco, String, String) {
        let mut a = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
        config::gravar(&a.conn, "caixa_nome", "Caixa 1").unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        let p = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        let mut b = Banco::em_memoria().unwrap();
        config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
        trocar(&mut a, &mut b);
        (a, b, u, p)
    }

    fn renomear(b: &Banco, u: &str, p: &str, nome: &str) {
        let mut f = produtos::ficha(&b.conn, p).unwrap();
        f.nome = nome.into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap();
    }

    fn nome(b: &Banco, p: &str) -> String {
        produtos::ficha(&b.conn, p).unwrap().nome
    }

    /// (versão que perdeu, nome guardado nela) dos conflitos abertos.
    fn abertos(b: &Banco) -> Vec<(String, String, String)> {
        let mut c = b
            .conn
            .prepare(
                "SELECT perdeu, COALESCE(json_extract(dados_perdeu, '$.nome'), tabela), certeza FROM sync_conflito
                  WHERE resolvido_em IS NULL AND tipo = 'registro' ORDER BY perdeu",
            )
            .unwrap();
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?))).unwrap();
        l.collect::<Result<_, _>>().unwrap()
    }

    /// Relógios diferentes em minutos, 25 horas e vários dias (para frente e
    /// para trás): duas alterações feitas sem uma ver a outra são sempre
    /// conflito, nos dois caixas; a que perdeu fica guardada; os dois ficam
    /// iguais; o aviso do Início aparece; a lista diz quais caixas.
    #[test]
    fn conflito_com_qualquer_diferenca_de_relogio() {
        for atraso in [10 * MIN, -10 * MIN, 25 * HORA, -25 * HORA, 5 * DIA, -5 * DIA] {
            let (mut a, mut b, u, p) = dois_caixas();
            renomear(&a, &u, &p, "Pastilha dianteira");
            com_relogio(atraso, || renomear(&b, &u, &p, "Pastilha traseira"));
            trocar(&mut a, &mut b);
            assert_eq!(nome(&a, &p), nome(&b, &p), "atraso {atraso}: os dois iguais");
            for x in [&a, &b] {
                let k = abertos(x);
                assert_eq!(k.len(), 1, "atraso {atraso}: conflito anotado em cada caixa: {k:?}");
                let perdeu =
                    if nome(x, &p) == "Pastilha dianteira" { "Pastilha traseira" } else { "Pastilha dianteira" };
                assert_eq!(k[0].1, perdeu, "atraso {atraso}: a versão que perdeu está guardada");
                assert_eq!(k[0].2, "concorrente");
                let lista = conflitos(&x.conn).unwrap();
                assert!(lista[0].texto.contains("está guardada"), "{}", lista[0].texto);
                assert!(!lista[0].caixa.is_empty() && !lista[0].caixa_ficou.is_empty());
                assert!(crate::avisos::do_dia(&x.conn).unwrap().iter().any(|a| a.id == "conflitos"));
            }
            assert_eq!(abertos(&a)[0].0, abertos(&b)[0].0, "o mesmo conflito nos dois");
        }
    }

    /// Uma alteração feita DEPOIS de ver a outra vale, mesmo com o relógio
    /// do caixa dias atrasado (antes, perdia para a hora mais nova). Não é
    /// conflito.
    #[test]
    fn feita_depois_vale_mesmo_com_relogio_atrasado() {
        for atraso in [-10 * MIN, -25 * HORA, -5 * DIA] {
            let (mut a, mut b, u, p) = dois_caixas();
            renomear(&a, &u, &p, "Pastilha 1");
            trocar(&mut a, &mut b);
            com_relogio(atraso, || renomear(&b, &u, &p, "Pastilha 2"));
            trocar(&mut a, &mut b);
            assert_eq!((nome(&a, &p), nome(&b, &p)), ("Pastilha 2".into(), "Pastilha 2".into()), "atraso {atraso}");
            assert!(abertos(&a).is_empty() && abertos(&b).is_empty(), "atraso {atraso}");
        }
    }

    /// Três caixas, cada um mexe sem ver os outros, e as alterações chegam
    /// num quarto caixa em todas as ordens possíveis (com repetição): o
    /// resultado e os conflitos são sempre os mesmos.
    #[test]
    fn qualquer_ordem_de_chegada_termina_igual() {
        let (mut a, mut b, u, p) = dois_caixas();
        let mut c = Banco::em_memoria().unwrap();
        trocar(&mut a, &mut c);
        let todas = |x: &Banco, desde: i64| alteracoes_desde(&x.conn, desde, 100_000).unwrap().0;
        let ultimo = |x: &Banco| todas(x, 0).last().map(|v| v.seq).unwrap_or(0);
        let base = todas(&a, 0);
        let (sa, sb, sc) = (ultimo(&a), ultimo(&b), ultimo(&c));
        renomear(&a, &u, &p, "Versão A");
        com_relogio(-25 * HORA, || renomear(&b, &u, &p, "Versão B"));
        com_relogio(3 * DIA, || {
            renomear(&c, &u, &p, "Versão C1");
            renomear(&c, &u, &p, "Versão C2");
        });
        let (la, lb, lc) = (todas(&a, sa), todas(&b, sb), todas(&c, sc));
        let pacotes = [la, lb.clone(), lc, lb];
        let ordens: [[usize; 4]; 6] =
            [[0, 1, 2, 3], [2, 1, 0, 3], [1, 2, 3, 0], [3, 0, 2, 1], [2, 3, 1, 0], [1, 0, 3, 2]];
        let mut resultados = Vec::new();
        for o in ordens {
            let mut d = Banco::em_memoria().unwrap();
            aplicar(&mut d.conn, &base).unwrap();
            for i in o {
                aplicar(&mut d.conn, &pacotes[i]).unwrap();
            }
            resultados.push((nome(&d, &p), abertos(&d)));
        }
        for r in &resultados {
            assert_eq!(r, &resultados[0], "ordens diferentes, mesmo resultado");
        }
        // C mexeu duas vezes (ordem maior): vale C2. A e B ficam guardadas;
        // C1 não é conflito (C2 foi feita em cima dela).
        assert_eq!(resultados[0].0, "Versão C2");
        let guardadas: Vec<&str> = resultados[0].1.iter().map(|k| k.1.as_str()).collect();
        assert_eq!(guardadas.len(), 2, "{guardadas:?}");
        assert!(guardadas.contains(&"Versão A") && guardadas.contains(&"Versão B"));
        // E os três caixas de verdade, trocando entre si, ficam iguais.
        trocar(&mut a, &mut b);
        trocar(&mut b, &mut c);
        trocar(&mut a, &mut c);
        for x in [&a, &b, &c] {
            assert_eq!(nome(x, &p), "Versão C2");
            assert_eq!(abertos(x).len(), 2);
        }
    }

    /// Reenvio: o mesmo lote chega de novo (inteiro, ao contrário, depois de
    /// compactar o log): nada é aplicado de novo e o aviso não repete.
    #[test]
    fn reenvio_nao_repete() {
        let (mut a, mut b, u, p) = dois_caixas();
        renomear(&a, &u, &p, "A");
        renomear(&b, &u, &p, "B");
        let (lista, _) = alteracoes_desde(&a.conn, 0, 100_000).unwrap();
        trocar(&mut a, &mut b);
        assert_eq!(abertos(&b).len(), 1);
        let invertida: Vec<_> = lista.iter().rev().cloned().collect();
        assert_eq!(aplicar(&mut b.conn, &lista).unwrap(), 0);
        assert_eq!(aplicar(&mut b.conn, &invertida).unwrap(), 0);
        // Depois de resolver e compactar (a versão que perdeu sai do log), o
        // reenvio continua reconhecido pelo id.
        let id = conflitos(&b.conn).unwrap()[0].id;
        resolver_conflito(&b.conn, id, false, &u).unwrap();
        compactar(&b.conn).unwrap();
        assert_eq!(aplicar(&mut b.conn, &lista).unwrap(), 0);
        assert_eq!(conflitos_abertos(&b.conn).unwrap(), 0);
        let total: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(total, 1, "um só registro do conflito");
    }

    /// A decisão do gerente vai para os outros caixas: lá o mesmo conflito
    /// fecha sozinho e a versão que perdeu não volta a valer. Um caixa que
    /// recebe a versão que perdeu depois da decisão já a vê resolvida.
    #[test]
    fn decisao_do_gerente_vai_para_os_outros() {
        let (mut a, mut b, u, p) = dois_caixas();
        renomear(&a, &u, &p, "Da A");
        com_relogio(2 * DIA, || renomear(&b, &u, &p, "Da B"));
        trocar(&mut a, &mut b);
        let vale = nome(&a, &p);
        let k = conflitos(&a.conn).unwrap();
        resolver_conflito(&a.conn, k[0].id, false, &u).unwrap();
        trocar(&mut a, &mut b);
        assert_eq!(conflitos_abertos(&b.conn).unwrap(), 0, "fechou no outro caixa");
        assert!(conflitos(&b.conn).unwrap()[0].resolvido.contains("ficou como está"));
        // Um terceiro caixa chega depois: recebe tudo, inclusive a versão que
        // perdeu, e já vê o conflito resolvido.
        let mut c = Banco::em_memoria().unwrap();
        trocar(&mut b, &mut c);
        assert_eq!(nome(&c, &p), vale);
        assert_eq!(conflitos_abertos(&c.conn).unwrap(), 0);
        // Agora o gerente usa a que perdeu, num conflito novo.
        renomear(&a, &u, &p, "Nova da A");
        renomear(&c, &u, &p, "Nova da C");
        trocar(&mut a, &mut c);
        trocar(&mut a, &mut b);
        let k = conflitos(&b.conn).unwrap().into_iter().find(|k| k.resolvido.is_empty()).expect("conflito novo");
        let guardada = k.campos.iter().find(|c| c["campo"] == "nome").unwrap()["perdeu"].as_str().unwrap().to_string();
        resolver_conflito(&b.conn, k.id, true, &u).unwrap();
        trocar(&mut a, &mut b);
        trocar(&mut b, &mut c);
        trocar(&mut a, &mut c);
        for x in [&a, &b, &c] {
            assert_eq!(nome(x, &p), guardada, "a escolhida vale em todos");
            assert_eq!(conflitos_abertos(&x.conn).unwrap(), 0, "e nenhum caixa pergunta de novo");
        }
    }

    /// Item 2 da revisão: dois gerentes decidem diferente sobre o mesmo
    /// conflito, com os caixas sem rede (A: manter; B: usar a que perdeu, com
    /// o relógio de B dias atrasado). As decisões se cruzam nas duas ordens
    /// e um terceiro caixa recebe tudo depois. Todos terminam com os mesmos
    /// dados (vale "usar"), a mesma resolução escrita (que diz que houve
    /// decisões diferentes e qual valeu), as duas decisões guardadas, o
    /// mesmo evento da divergência e a versão que perdeu ainda guardada.
    #[test]
    fn dois_gerentes_decidem_diferente() {
        for ordem_a_primeiro in [true, false] {
            let (mut a, mut b, u, p) = dois_caixas();
            let bia = usuarios::criar(&a.conn, &a.dispositivo, "Bia", "5678", true).unwrap().id;
            let mut c = Banco::em_memoria().unwrap();
            trocar(&mut a, &mut b);
            trocar(&mut a, &mut c);
            renomear(&a, &u, &p, "Nome da A");
            renomear(&b, &u, &p, "Nome da B");
            trocar(&mut a, &mut b);
            let vale = nome(&a, &p);
            let perdeu = if vale == "Nome da A" { "Nome da B" } else { "Nome da A" };
            // Sem rede: cada gerente decide no seu caixa.
            let ka = conflitos(&a.conn).unwrap()[0].id;
            let kb = conflitos(&b.conn).unwrap()[0].id;
            resolver_conflito(&a.conn, ka, false, &u).unwrap();
            com_relogio(-5 * DIA, || resolver_conflito(&b.conn, kb, true, &bia).unwrap());
            assert_eq!(nome(&b, &p), perdeu);
            // As decisões se cruzam (nas duas ordens) e chegam a um terceiro.
            if ordem_a_primeiro {
                levar(&a, &mut b);
                levar(&b, &mut a);
            } else {
                levar(&b, &mut a);
                levar(&a, &mut b);
            }
            trocar(&mut a, &mut b);
            trocar(&mut b, &mut c);
            trocar(&mut a, &mut c);
            let mut textos = Vec::new();
            for (quem, x) in [("A", &a), ("B", &b), ("C", &c)] {
                assert_eq!(nome(x, &p), perdeu, "{quem}: vale usar a que perdeu");
                assert_eq!(conflitos_abertos(&x.conn).unwrap(), 0, "{quem}");
                let k = conflitos(&x.conn).unwrap();
                let r = k.iter().find(|k| k.tipo == "registro").expect("o conflito continua na lista");
                assert!(r.resolvido.contains("decisões diferentes"), "{quem}: {}", r.resolvido);
                assert!(
                    r.resolvido.contains("Ana manteve") && r.resolvido.contains("Bia usou"),
                    "{quem}: {}",
                    r.resolvido
                );
                assert!(r.resolvido.contains("Vale agora: a versão que tinha perdido"), "{quem}: {}", r.resolvido);
                textos.push(r.resolvido.clone());
                let decisoes: Vec<(String, String)> = x
                    .conn
                    .prepare("SELECT id, escolha FROM sync_resolucao ORDER BY id")
                    .unwrap()
                    .query_map([], |l| Ok((l.get(0)?, l.get(1)?)))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                assert_eq!(decisoes.len(), 3, "{quem}: as duas decisões e o evento da divergência: {decisoes:?}");
                assert_eq!(decisoes.iter().filter(|d| d.1 == "aplicada").count(), 1, "{quem}");
                // A versão que estava valendo (a de "manter") continua no log de
                // alterações, com os dados, nos três caixas.
                let guardada: i64 = x
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM alteracao WHERE tabela = 'produto' AND json_extract(dados, '$.nome') = ?1",
                        [&vale],
                        |l| l.get(0),
                    )
                    .unwrap();
                assert!(guardada >= 1, "{quem}: a versão de quem manteve não some");
            }
            assert!(textos.windows(2).all(|w| w[0] == w[1]), "a mesma resolução nos três: {textos:?}");
            let evento = |x: &Banco| -> String {
                x.conn.query_row("SELECT id FROM sync_resolucao WHERE escolha = 'aplicada'", [], |l| l.get(0)).unwrap()
            };
            assert!(evento(&a) == evento(&b) && evento(&b) == evento(&c), "o mesmo evento nos três");
        }
    }

    /// Backup restaurado num caixa, com o outro caixa tendo mexido muitas
    /// vezes no mesmo registro e compactado o log: as versões velhas do
    /// restaurado não viram aviso (a cadeia de versões mostra que vieram
    /// antes), ele fica igual ao outro e recebe de volta o que perdeu,
    /// inclusive o que ele mesmo tinha feito depois do backup.
    #[test]
    fn backup_restaurado() {
        let pasta = std::env::temp_dir().join(format!("rustinho-r08-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        for mudancas in [5, HISTORIA_MAXIMA + 8, 3 * HISTORIA_MAXIMA] {
            let (mut a, mut b, u, p) = dois_caixas();
            let arquivo = pasta.join(format!("a-{mudancas}.db"));
            a.conn.backup(rusqlite::MAIN_DB, &arquivo, None).unwrap();
            // Depois do backup: A cria uma peça (e manda para B); B mexe muitas vezes.
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = "Feita pela A depois do backup".into();
            f.custo = "1".into();
            let nova = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            trocar(&mut a, &mut b);
            for i in 0..mudancas {
                renomear(&b, &u, &p, &format!("B {i}"));
            }
            compactar(&b.conn).unwrap();
            // A volta do backup e troca de novo.
            let mut a = Banco::abrir(&arquivo).unwrap();
            assert_eq!(nome(&a, &p), "Pastilha");
            assert!(produtos::ficha(&a.conn, &nova).is_err(), "a peça nova se perdeu no backup");
            trocar(&mut a, &mut b);
            let ultima = format!("B {}", mudancas - 1);
            assert_eq!((nome(&a, &p), nome(&b, &p)), (ultima.clone(), ultima.clone()), "{mudancas}");
            assert!(
                abertos(&a).is_empty() && abertos(&b).is_empty(),
                "{mudancas}: {:?} {:?}",
                abertos(&a),
                abertos(&b)
            );
            assert_eq!(nome(&a, &nova), "Feita pela A depois do backup", "voltou dos outros caixas");
        }
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Quando o caminho de versões está incompleto (versão de antes do
    /// protocolo 3 com base que este caixa não tem), não dá para provar a
    /// ordem: conflito "incerto", com as duas versões guardadas, sem olhar
    /// o relógio. "Deixar todas como estão" fecha de uma vez e a decisão vai
    /// para o outro caixa.
    #[test]
    fn ordem_incerta_vira_conflito() {
        let (mut a, mut b, u, p) = dois_caixas();
        let (chave, dados): (String, String) = a
            .conn
            .query_row("SELECT chave, dados FROM alteracao WHERE tabela = 'produto' AND compactada = 0", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        let estranha = Alteracao {
            seq: 1,
            id: tempo::novo_id(),
            tabela: "produto".into(),
            chave,
            operacao: "gravar".into(),
            dados: Some(dados.replace("Pastilha", "Pastilha de um caixa antigo")),
            dispositivo: "caixa-antigo".into(),
            quando: tempo::agora(),
            base: Some("01000000-0000-7000-8000-000000000000".into()),
            ordem: 0,
            historia: None,
            compactada: false,
        };
        aplicar(&mut a.conn, std::slice::from_ref(&estranha)).unwrap();
        let k = abertos(&a);
        assert_eq!(k.len(), 1, "{k:?}");
        assert_eq!(k[0].2, "incerta");
        let lista = conflitos(&a.conn).unwrap();
        assert!(lista[0].texto.contains("Não deu para saber"), "{}", lista[0].texto);
        // As duas versões guardadas: a que vale está na peça, a outra no conflito.
        assert_eq!(nome(&a, &p), "Pastilha", "ordem 0 perde para a versão de ordem 1");
        assert_eq!(k[0].1, "Pastilha de um caixa antigo");
        trocar(&mut a, &mut b);
        assert_eq!(abertos(&b).len(), 1, "o outro caixa também avisa");
        assert_eq!(manter_todos(&a.conn, &u).unwrap(), 1);
        trocar(&mut a, &mut b);
        assert_eq!(conflitos_abertos(&a.conn).unwrap(), 0);
        assert_eq!(conflitos_abertos(&b.conn).unwrap(), 0, "a decisão chegou ao outro caixa");
    }

    /// Item 4 da revisão: volume. Um caixa com muitas alterações (peças
    /// criadas e mexidas, uma peça mexida milhares de vezes, vendas), o log
    /// compactado, um caixa novo recebendo tudo e um caixa que ficou muito
    /// tempo sem rede (e mexeu na peça mais mexida, sem ver as outras)
    /// voltando. Mede tamanho e tempos; confere que tudo fica igual e que o
    /// conflito é achado. Roda à parte (demora):
    /// `cargo test -p rustinho-nucleo --release -- --ignored volume --nocapture`
    #[test]
    #[ignore]
    fn volume_do_historico() {
        let (pecas, edicoes, quente, vendas_n) = (2000usize, 10usize, 3000usize, 500usize);
        let pasta = std::env::temp_dir().join(format!("rustinho-volume-{}", tempo::novo_id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let abrir = |n: &str| Banco::abrir(&pasta.join(n)).unwrap();
        let tamanho = |b: &Banco, n: &str| -> f64 {
            b.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
            std::fs::metadata(pasta.join(n)).unwrap().len() as f64 / 1_048_576.0
        };
        let linhas = |b: &Banco| -> (i64, i64) {
            b.conn
                .query_row("SELECT COUNT(*), COALESCE(SUM(compactada), 0) FROM alteracao", [], |l| {
                    Ok((l.get(0)?, l.get(1)?))
                })
                .unwrap()
        };
        let mut a = abrir("a.db");
        let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
        usuarios::definir_gerente(&a.conn, &u, true).unwrap();
        let mut f = produtos::ficha_nova(&a.conn).unwrap();
        f.nome = "Peça mais mexida".into();
        f.custo = "10".into();
        f.estoque_inicial = "100000".into();
        let quente_id = produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        // B recebe o começo e fica sem rede.
        let mut b = abrir("b.db");
        levar(&a, &mut b);
        let t = Instant::now();
        let mut ids = Vec::new();
        a.conn.execute_batch("BEGIN").unwrap();
        for i in 0..pecas {
            let mut f = produtos::ficha_nova(&a.conn).unwrap();
            f.nome = format!("Peça {i}");
            f.custo = "10".into();
            f.estoque_inicial = "50".into();
            ids.push(produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap());
        }
        for e in 0..edicoes {
            for id in &ids {
                let mut f = produtos::ficha(&a.conn, id).unwrap();
                f.observacoes = format!("edição {e}");
                produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
            }
        }
        for e in 0..quente {
            let mut f = produtos::ficha(&a.conn, &quente_id).unwrap();
            f.observacoes = format!("quente {e}");
            produtos::salvar(&a.conn, &a.dispositivo, &u, &f).unwrap();
        }
        a.conn.execute_batch("COMMIT").unwrap();
        caixa::abrir(&a.conn, &a.dispositivo, &u, "Caixa A", "0").unwrap();
        for i in 0..vendas_n {
            let e = vendas::VendaEntrada {
                itens: vec![vendas::ItemEntrada {
                    produto_id: ids[i % ids.len()].clone(),
                    quantidade: "1".into(),
                    ..Default::default()
                }],
                tabela: 1,
                pagamentos: vec![vendas::PagamentoEntrada {
                    forma: "dinheiro".into(),
                    valor: "100".into(),
                    ..Default::default()
                }],
                ..Default::default()
            };
            vendas::concluir(&a.conn, &a.dispositivo, &u, &e).unwrap();
        }
        let gerar = t.elapsed();
        let (antes_linhas, _) = linhas(&a);
        let antes_mb = tamanho(&a, "a.db");
        let t = Instant::now();
        let compactadas = compactar(&a.conn).unwrap();
        let tempo_compactar = t.elapsed();
        a.conn.execute_batch("VACUUM").unwrap();
        let (depois_linhas, sem_dados) = linhas(&a);
        let depois_mb = tamanho(&a, "a.db");
        // B, sem rede, mexe na peça mais mexida (sem ver as 3000 de A).
        let mut f = produtos::ficha(&b.conn, &quente_id).unwrap();
        f.nome = "Mexida pelo caixa sem rede".into();
        produtos::salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
        // Caixa novo recebe tudo.
        let mut c = abrir("c.db");
        let t = Instant::now();
        let recebidas = levar(&a, &mut c);
        let tempo_novo = t.elapsed();
        let c_mb = tamanho(&c, "c.db");
        // B volta (recebe só o que falta, a partir do seu contador; aqui, tudo
        // pelo id: o que ele já tem é pulado) e acha o conflito.
        let t = Instant::now();
        levar(&a, &mut b);
        let tempo_atrasado = t.elapsed();
        let t = Instant::now();
        levar(&b, &mut a);
        let tempo_volta = t.elapsed();
        let conflitos_a = conflitos_abertos(&a.conn).unwrap();
        // Tudo igual.
        for x in [&b, &c] {
            let n: i64 = x.conn.query_row("SELECT COUNT(*) FROM produto", [], |l| l.get(0)).unwrap();
            assert_eq!(n as usize, pecas + 1);
            let v: i64 = x.conn.query_row("SELECT COUNT(*) FROM venda WHERE tipo = 'venda'", [], |l| l.get(0)).unwrap();
            assert_eq!(v as usize, vendas_n);
            assert_eq!(produtos::ficha(&x.conn, &ids[0]).unwrap().observacoes, format!("edição {}", edicoes - 1));
        }
        assert_eq!(produtos::ficha(&c.conn, &quente_id).unwrap().observacoes, format!("quente {}", quente - 1));
        assert!(conflitos_abertos(&b.conn).unwrap() >= 1, "o caixa que voltou acha o conflito");
        assert!(conflitos_a >= 1, "e o principal também");
        println!("\n=== Volume do histórico ===");
        println!("peças: {pecas}, edições por peça: {edicoes}, peça mais mexida: {quente} vezes, vendas: {vendas_n}");
        println!("gerar tudo no caixa A: {:.1} s", gerar.as_secs_f64());
        println!("log antes de compactar: {antes_linhas} linhas, banco {antes_mb:.1} MB");
        println!("compactar: {compactadas} versões ficaram sem dados em {:.2} s", tempo_compactar.as_secs_f64());
        println!("log depois (VACUUM): {depois_linhas} linhas ({sem_dados} só esqueleto), banco {depois_mb:.1} MB");
        let (esqueleto, com_dados): (f64, f64) = a
            .conn
            .query_row(
                "SELECT AVG(CASE WHEN compactada = 1 THEN length(id) + length(tabela) + length(chave) + length(operacao)
                                 + length(dispositivo) + length(quando) + COALESCE(length(base), 0) + 8 END),
                        AVG(CASE WHEN compactada = 0 THEN length(id) + length(tabela) + length(chave) + length(operacao)
                                 + length(dispositivo) + length(quando) + COALESCE(length(base), 0) + 8
                                 + COALESCE(length(dados), 0) + COALESCE(length(historia), 0) END)
                   FROM alteracao",
                [],
                |l| Ok((l.get::<_, Option<f64>>(0)?.unwrap_or(0.0), l.get::<_, Option<f64>>(1)?.unwrap_or(0.0))),
            )
            .unwrap();
        println!("conteúdo médio: versão só esqueleto {esqueleto:.0} bytes; versão com dados {com_dados:.0} bytes (sem contar índices)");
        println!(
            "caixa novo recebe tudo: {recebidas} alterações em {:.1} s, banco {c_mb:.1} MB",
            tempo_novo.as_secs_f64()
        );
        println!(
            "caixa que ficou sem rede volta: {:.1} s (e manda a dele: {:.2} s)",
            tempo_atrasado.as_secs_f64(),
            tempo_volta.as_secs_f64()
        );
        println!("conflitos achados: B {}, A {conflitos_a}", conflitos_abertos(&b.conn).unwrap());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// História: cresce até o limite e marca quando não chega ao começo.
    #[test]
    fn historia_tem_tamanho_limitado() {
        assert_eq!(historia_nova(None, None, None), "[]");
        assert_eq!(historia_nova(Some("a"), Some("[]"), None), r#"["a"]"#);
        // Base de antes do protocolo 3 (sem história, mas com base): incompleta.
        assert_eq!(historia_nova(Some("b"), None, Some("a")), r#"["b","a","+"]"#);
        let mut h = "[]".to_string();
        let mut base: Option<String> = None;
        for i in 0..100 {
            let id = format!("v{i}");
            h = historia_nova(Some(&id), Some(&h), base.as_deref());
            base = Some(id);
        }
        let lista: Vec<String> = serde_json::from_str(&h).unwrap();
        assert_eq!(lista.len(), HISTORIA_MAXIMA + 1);
        assert_eq!(lista.last().unwrap(), INCOMPLETA);
        assert_eq!(lista[0], "v99");
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
        assert_eq!(decifrar(&k1, &m, None).unwrap(), b"ola");
        // Senha errada.
        let k3 = Chaves {
            senha: "outra-senha".into(),
            sal: [9u8; 16],
            propria: derivar("outra-senha", &[9u8; 16]).unwrap(),
        };
        assert!(decifrar(&k3, &m, None).unwrap_err().contains("senha"));
        // Versão antiga (sem "RST3"), inclusive a do protocolo 2: recusada
        // antes de ler qualquer dado.
        assert!(decifrar(&k1, &[0u8; 80], None).unwrap_err().contains("versão"));
        let mut antiga = cifrar(&k2, b"ola").unwrap();
        antiga[..4].copy_from_slice(b"RST2");
        assert!(decifrar(&k1, &antiga, None).unwrap_err().contains("versão"));
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
        // Fica uma versão com dados; as velhas ficam sem dados (só a cadeia).
        let (com_dados, sem_dados): (i64, i64) = b
            .conn
            .query_row(
                "SELECT SUM(compactada = 0), SUM(compactada = 1 AND dados IS NULL) FROM alteracao WHERE tabela = 'usuario'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert_eq!(com_dados, 1);
        assert_eq!(sem_dados, antes - 1);
        // Uma alteração de outro caixa, feita sem ver as nossas (sem base),
        // chega depois: não vale (ordem menor) e fica anotada, guardada.
        let (chave_u, dados): (String, String) = b
            .conn
            .query_row("SELECT chave, dados FROM alteracao WHERE tabela = 'usuario' AND compactada = 0", [], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        let velha = Alteracao {
            seq: 1,
            id: "00000000-0000-7000-8000-000000000001".into(),
            tabela: "usuario".into(),
            chave: chave_u,
            operacao: "gravar".into(),
            dados: Some(dados.replace("Maria C", "Maria velha")),
            dispositivo: "outro".into(),
            quando: tempo::agora(),
            base: None,
            ordem: 1,
            historia: Some("[]".into()),
            compactada: false,
        };
        aplicar(&mut b.conn, std::slice::from_ref(&velha)).unwrap();
        assert_eq!(usuarios::buscar(&b.conn, &u.id).unwrap().nome, "Maria C");
        let conflitos: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(conflitos, 1);
        // A mesma de novo: reconhecida pelo id, não repete.
        aplicar(&mut b.conn, &[velha]).unwrap();
        let conflitos: i64 = b.conn.query_row("SELECT COUNT(*) FROM sync_conflito", [], |l| l.get(0)).unwrap();
        assert_eq!(conflitos, 1);
        // Compactar não apaga a versão que perdeu enquanto o conflito está aberto.
        compactar(&b.conn).unwrap();
        let guardada: i64 = b
            .conn
            .query_row("SELECT COUNT(*) FROM alteracao WHERE id = '00000000-0000-7000-8000-000000000001'", [], |l| {
                l.get(0)
            })
            .unwrap();
        assert_eq!(guardada, 1);
        assert_eq!(pendencias(&b.conn).unwrap(), 0);
    }

    /// C-01: a vaga volta mesmo se a tarefa der pânico, e nunca passa do teto.
    #[test]
    fn vaga_volta_depois_de_panico() {
        let contador = Arc::new(AtomicUsize::new(0));
        let vagas: Vec<Vaga> = (0..CONEXOES_MAXIMAS).filter_map(|_| Vaga::pegar_de(&contador)).collect();
        assert_eq!(vagas.len(), CONEXOES_MAXIMAS);
        assert!(Vaga::pegar_de(&contador).is_none(), "não passa do teto");
        let mut vagas = vagas.into_iter();
        let uma = vagas.next().unwrap();
        let r = thread::spawn(move || {
            let _v = uma;
            panic!("defeito de teste");
        })
        .join();
        assert!(r.is_err());
        let outra = Vaga::pegar_de(&contador).expect("a vaga voltou depois do pânico");
        drop(outra);
        drop(vagas);
        assert_eq!(contador.load(Ordering::SeqCst), 0);
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
