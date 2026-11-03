//! Encomendas: a peça que o cliente pediu e a loja não tem.
//!
//! O cliente pode pagar tudo na hora, dar um sinal (o resto fica no Receber)
//! ou pagar quando a peça chegar. Nos dois primeiros casos a encomenda nasce
//! na venda; no último, ela é anotada sozinha e vira venda na entrega.
//!
//! Quando a compra da peça é lançada (XML), a encomenda passa sozinha para
//! "chegou" e o Rustinho lembra de avisar o cliente (com o WhatsApp pronto).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, config, fiscal, numeros, tempo, texto};

pub const SITUACOES: &[(&str, &str)] = &[
    ("aberta", "Falta pedir"),
    ("pedida", "Pedida"),
    ("chegou", "Chegou"),
    ("entregue", "Entregue"),
    ("cancelada", "Cancelada"),
];

pub fn nome_situacao(s: &str) -> &'static str {
    SITUACOES.iter().find(|x| x.0 == s).map(|x| x.1).unwrap_or("")
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EncomendaEntrada {
    pub cliente_id: String,
    /// Sem cadastro: nome e telefone digitados.
    pub cliente_nome: String,
    pub telefone: String,
    /// Peça cadastrada (ou vazio, e então vale a descrição).
    pub produto_id: String,
    pub descricao: String,
    pub quantidade: String,
    pub observacoes: String,
}

/// Só os números do telefone.
fn so_numeros(t: &str) -> String {
    t.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn telefone_do_cliente(conn: &Connection, cliente_id: &str) -> Resultado<String> {
    let (cel, tel): (String, String) = conn
        .query_row("SELECT celular, telefone FROM pessoa WHERE id = ?1", [cliente_id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?
        .unwrap_or_default();
    Ok(if !cel.trim().is_empty() { cel } else { tel })
}

fn proximo_numero(conn: &Connection) -> Resultado<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(numero), 0) + 1 FROM encomenda", [], |l| l.get(0))?)
}

#[allow(clippy::too_many_arguments)]
fn inserir(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    cliente_id: Option<&str>,
    cliente_nome: &str,
    telefone: &str,
    produto_id: Option<&str>,
    descricao: &str,
    quantidade: i64,
    venda_id: Option<&str>,
    observacoes: &str,
) -> Resultado<String> {
    let id = tempo::novo_id();
    let numero = proximo_numero(conn)?;
    let agora = tempo::agora();
    conn.execute(
        "INSERT INTO encomenda (id, numero, cliente_id, cliente_nome, telefone, produto_id, descricao, quantidade,
                                venda_id, situacao, observacoes, criado_em, criado_por, atualizado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'aberta', ?10, ?11, ?12, ?11, ?13)",
        params![
            id,
            numero,
            cliente_id,
            cliente_nome,
            telefone,
            produto_id,
            descricao,
            quantidade,
            venda_id,
            observacoes,
            agora,
            usuario_id,
            dispositivo
        ],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "encomenda.criar",
        "encomenda",
        &id,
        &[format!(
            "Encomenda {numero}: {} × {descricao} para {cliente_nome}",
            numeros::escrever_quantidade(quantidade)
        )],
    )?;
    Ok(id)
}

/// Anota uma encomenda que o cliente paga quando a peça chegar.
pub fn criar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &EncomendaEntrada) -> Resultado<String> {
    let (cliente_id, cliente_nome) = if e.cliente_id.is_empty() {
        let nome = texto::limpar(&e.cliente_nome);
        if nome.is_empty() {
            return Err(Erro::campo("cliente", "Para quem é a encomenda?"));
        }
        (None, nome)
    } else {
        let nome: String = conn
            .query_row("SELECT nome FROM pessoa WHERE id = ?1", [&e.cliente_id], |l| l.get(0))
            .optional()?
            .ok_or_else(|| Erro::NaoEncontrado("Cliente".into()))?;
        (Some(e.cliente_id.as_str()), nome)
    };
    if texto::tamanho(&cliente_nome) > 120 {
        return Err(Erro::campo("cliente", "Use no máximo 120 letras."));
    }
    let telefone = match (texto::limpar(&e.telefone), cliente_id) {
        (t, _) if !t.is_empty() => t,
        (_, Some(c)) => telefone_do_cliente(conn, c)?,
        _ => String::new(),
    };
    if !telefone.is_empty() && !(10..=13).contains(&so_numeros(&telefone).len()) {
        return Err(Erro::campo("telefone", "Telefone com DDD, ex.: (15) 91234-0000."));
    }
    let (produto_id, descricao) = if e.produto_id.is_empty() {
        let d = texto::limpar(&e.descricao);
        if d.is_empty() {
            return Err(Erro::campo("peca", "Qual é a peça? Escolha uma cadastrada ou descreva."));
        }
        (None, d)
    } else {
        let nome: String = conn
            .query_row(
                "SELECT nome || CASE WHEN marca <> '' THEN ' · ' || marca ELSE '' END FROM produto WHERE id = ?1",
                [&e.produto_id],
                |l| l.get(0),
            )
            .optional()?
            .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
        (Some(e.produto_id.as_str()), nome)
    };
    if texto::tamanho(&descricao) > 160 {
        return Err(Erro::campo("peca", "Use no máximo 160 letras."));
    }
    let quantidade = numeros::ler_quantidade(&e.quantidade).map_err(|m| Erro::campo("quantidade", m))?.unwrap_or(1000);
    if quantidade <= 0 {
        return Err(Erro::campo("quantidade", "Quantidade precisa ser maior que zero."));
    }
    let obs = texto::limpar(&e.observacoes);
    if texto::tamanho(&obs) > 300 {
        return Err(Erro::campo("observacoes", "Use no máximo 300 letras."));
    }
    inserir(
        conn,
        dispositivo,
        usuario_id,
        cliente_id,
        &cliente_nome,
        &telefone,
        produto_id,
        &descricao,
        quantidade,
        None,
        &obs,
    )
}

/// Encomendas que nascem de uma venda (pagas na hora ou com sinal).
/// `itens`: (peça, descrição, quantidade que falta).
pub fn criar_da_venda(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    venda_id: &str,
    cliente_id: &str,
    itens: &[(String, String, i64)],
) -> Resultado<()> {
    if itens.is_empty() {
        return Ok(());
    }
    let nome: String = conn.query_row("SELECT nome FROM pessoa WHERE id = ?1", [cliente_id], |l| l.get(0))?;
    let telefone = telefone_do_cliente(conn, cliente_id)?;
    for (pid, descricao, q) in itens {
        inserir(
            conn,
            dispositivo,
            usuario_id,
            Some(cliente_id),
            &nome,
            &telefone,
            Some(pid),
            descricao,
            *q,
            Some(venda_id),
            "",
        )?;
    }
    Ok(())
}

struct Atual {
    numero: i64,
    situacao: String,
    produto_id: Option<String>,
    venda_id: Option<String>,
    descricao: String,
}

fn atual(conn: &Connection, id: &str) -> Resultado<Atual> {
    conn.query_row("SELECT numero, situacao, produto_id, venda_id, descricao FROM encomenda WHERE id = ?1", [id], |l| {
        Ok(Atual {
            numero: l.get(0)?,
            situacao: l.get(1)?,
            produto_id: l.get(2)?,
            venda_id: l.get(3)?,
            descricao: l.get(4)?,
        })
    })
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Encomenda".into()))
}

fn mudar(conn: &Connection, id: &str, situacao: &str, coluna: Option<&str>) -> Resultado<()> {
    let agora = tempo::agora();
    match coluna {
        Some(c) => conn.execute(
            &format!("UPDATE encomenda SET situacao = ?2, {c} = ?3, atualizado_em = ?3 WHERE id = ?1"),
            params![id, situacao, agora],
        )?,
        None => conn.execute(
            "UPDATE encomenda SET situacao = ?2, atualizado_em = ?3 WHERE id = ?1",
            params![id, situacao, agora],
        )?,
    };
    Ok(())
}

/// Marca que a peça foi pedida ao fornecedor (ou desfaz: volta a "falta pedir").
pub fn pedir(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, desfazer: bool) -> Resultado<()> {
    let a = atual(conn, id)?;
    if desfazer {
        if a.situacao != "pedida" {
            return Err(Erro::validacao("Esta encomenda não está como pedida."));
        }
        conn.execute(
            "UPDATE encomenda SET situacao = 'aberta', pedida_em = NULL, atualizado_em = ?2 WHERE id = ?1",
            params![id, tempo::agora()],
        )?;
        return Ok(());
    }
    if a.situacao != "aberta" {
        return Err(Erro::validacao("Esta encomenda já foi pedida."));
    }
    mudar(conn, id, "pedida", Some("pedida_em"))?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "encomenda.pedir",
        "encomenda",
        id,
        &[format!("Encomenda {}", a.numero)],
    )?;
    Ok(())
}

/// Marca à mão que a peça chegou (sem nota, ou peça sem cadastro).
pub fn chegou(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let a = atual(conn, id)?;
    if a.situacao != "aberta" && a.situacao != "pedida" {
        return Err(Erro::validacao("Esta encomenda não está esperando a peça."));
    }
    conn.execute(
        "UPDATE encomenda SET situacao = 'chegou', situacao_antes = ?2, chegou_em = ?3, atualizado_em = ?3 WHERE id = ?1",
        params![id, a.situacao, tempo::agora()],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "encomenda.chegou",
        "encomenda",
        id,
        &[format!("Encomenda {}", a.numero)],
    )?;
    Ok(())
}

/// A compra lançada trouxe peças encomendadas: as mais antigas primeiro,
/// enquanto a quantidade da nota der. Devolve quantas chegaram.
pub fn chegou_na_compra(conn: &Connection, compra_id: &str, pecas: &[(String, i64)]) -> Resultado<i64> {
    let fornecedor: Option<String> = conn
        .query_row("SELECT fornecedor_id FROM compra WHERE id = ?1", [compra_id], |l| l.get(0))
        .optional()?
        .flatten();
    let mut chegaram = 0;
    let agora = tempo::agora();
    for (pid, recebido) in pecas {
        let mut sobra = *recebido;
        let lista: Vec<(String, i64, String)> = conn
            .prepare(
                "SELECT id, quantidade, situacao FROM encomenda
                  WHERE produto_id = ?1 AND situacao IN ('aberta', 'pedida') ORDER BY criado_em, numero",
            )?
            .query_map([pid], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (id, q, situacao) in lista {
            if q > sobra {
                break;
            }
            sobra -= q;
            conn.execute(
                "UPDATE encomenda SET situacao = 'chegou', situacao_antes = ?2, chegou_em = ?3, compra_id = ?4,
                                      fornecedor_id = COALESCE(fornecedor_id, ?5), atualizado_em = ?3
                  WHERE id = ?1",
                params![id, situacao, agora, compra_id, fornecedor],
            )?;
            chegaram += 1;
        }
    }
    Ok(chegaram)
}

/// A compra foi estornada: as encomendas que ela trouxe voltam a esperar.
pub fn desfazer_da_compra(conn: &Connection, compra_id: &str) -> Resultado<()> {
    conn.execute(
        "UPDATE encomenda SET situacao = CASE WHEN situacao_antes = 'pedida' THEN 'pedida' ELSE 'aberta' END,
                              chegou_em = NULL, avisado_em = NULL, compra_id = NULL, atualizado_em = ?2
          WHERE compra_id = ?1 AND situacao = 'chegou'",
        params![compra_id, tempo::agora()],
    )?;
    Ok(())
}

/// Entrega ao cliente uma encomenda que já foi paga (ou uma peça sem cadastro).
pub fn entregar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let a = atual(conn, id)?;
    if a.situacao == "entregue" || a.situacao == "cancelada" {
        return Err(Erro::validacao("Esta encomenda já foi encerrada."));
    }
    if a.venda_id.is_none() && a.produto_id.is_some() {
        return Err(Erro::validacao("Esta encomenda ainda não foi paga. Use “Vender” para entregar."));
    }
    mudar(conn, id, "entregue", Some("entregue_em"))?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "encomenda.entregar",
        "encomenda",
        id,
        &[format!("Encomenda {}: {}", a.numero, a.descricao)],
    )?;
    Ok(())
}

/// A venda feita na entrega encerra a encomenda.
pub fn entregue_na_venda(conn: &Connection, id: &str, venda_id: &str) -> Resultado<()> {
    let a = atual(conn, id)?;
    if a.situacao == "entregue" || a.situacao == "cancelada" {
        return Err(Erro::validacao(format!("A encomenda {} já foi encerrada.", a.numero)));
    }
    conn.execute(
        "UPDATE encomenda SET situacao = 'entregue', entregue_em = ?2, venda_entrega_id = ?3, atualizado_em = ?2 WHERE id = ?1",
        params![id, tempo::agora(), venda_id],
    )?;
    Ok(())
}

/// Cancela a encomenda. A que foi paga na venda só cancela junto com a venda
/// (assim o dinheiro e o estoque voltam certos).
pub fn cancelar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    let a = atual(conn, id)?;
    if a.situacao == "entregue" || a.situacao == "cancelada" {
        return Err(Erro::validacao("Esta encomenda já foi encerrada."));
    }
    if let Some(v) = &a.venda_id {
        let (numero, situacao): (i64, String) =
            conn.query_row("SELECT numero, situacao FROM venda WHERE id = ?1", [v], |l| Ok((l.get(0)?, l.get(1)?)))?;
        if situacao == "concluida" {
            return Err(Erro::validacao(format!(
                "Esta encomenda foi paga na venda {numero}. Cancele a venda (o dinheiro e o estoque voltam) e a encomenda é cancelada junto."
            )));
        }
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo (ex.: cliente desistiu, fornecedor não tem)."));
    }
    conn.execute(
        "UPDATE encomenda SET situacao = 'cancelada', cancelada_em = ?2, motivo = ?3, atualizado_em = ?2 WHERE id = ?1",
        params![id, tempo::agora(), motivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "encomenda.cancelar",
        "encomenda",
        id,
        &[format!("Encomenda {}: {}", a.numero, a.descricao), format!("Motivo: {motivo}")],
    )?;
    Ok(())
}

/// A venda foi cancelada: as encomendas pagas nela são canceladas, e a que
/// foi entregue por ela volta a "chegou".
pub fn cancelar_da_venda(conn: &Connection, venda_id: &str) -> Resultado<()> {
    let agora = tempo::agora();
    conn.execute(
        "UPDATE encomenda SET situacao = 'cancelada', cancelada_em = ?2, motivo = 'Venda cancelada', atualizado_em = ?2
          WHERE venda_id = ?1 AND situacao NOT IN ('entregue', 'cancelada')",
        params![venda_id, agora],
    )?;
    conn.execute(
        "UPDATE encomenda SET situacao = 'chegou', entregue_em = NULL, venda_entrega_id = NULL, atualizado_em = ?2
          WHERE venda_entrega_id = ?1 AND situacao = 'entregue'",
        params![venda_id, agora],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// WhatsApp
// ---------------------------------------------------------------------------

/// Codifica o texto para ir num endereço (URL).
fn codificar(t: &str) -> String {
    let mut s = String::new();
    for b in t.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => s.push(b as char),
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

/// Número para o WhatsApp: com 55 na frente quando vier só com o DDD.
fn numero_whatsapp(telefone: &str) -> Option<String> {
    let n = so_numeros(telefone);
    let n = n.trim_start_matches('0').to_string();
    match n.len() {
        10 | 11 => Some(format!("55{n}")),
        12 | 13 if n.starts_with("55") => Some(n),
        _ => None,
    }
}

fn primeiro_nome(nome: &str) -> String {
    let p = nome.split_whitespace().next().unwrap_or("");
    let mut c = p.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Endereço do WhatsApp com a mensagem pronta. Marca o cliente como avisado.
pub fn whatsapp(conn: &Connection, id: &str) -> Resultado<String> {
    let (numero, nome, telefone, descricao, situacao): (i64, String, String, String, String) = conn
        .query_row(
            "SELECT numero, cliente_nome, telefone, descricao, situacao FROM encomenda WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Encomenda".into()))?;
    let destino = numero_whatsapp(&telefone)
        .ok_or_else(|| Erro::campo("telefone", "Esta encomenda está sem um celular válido (com DDD)."))?;
    let loja = config::nome_loja(conn)?;
    let loja = if loja.is_empty() { "loja".to_string() } else { loja };
    let ola = match primeiro_nome(&nome) {
        n if n.is_empty() => "Olá!".to_string(),
        n => format!("Olá, {n}!"),
    };
    let mensagem = if situacao == "chegou" {
        format!(
            "{ola} Aqui é da {loja}. A peça que você encomendou chegou: {descricao}. Pode vir buscar quando quiser."
        )
    } else {
        format!("{ola} Aqui é da {loja}, sobre a sua encomenda nº {numero}: {descricao}.")
    };
    if situacao == "chegou" {
        conn.execute(
            "UPDATE encomenda SET avisado_em = ?2, atualizado_em = ?2 WHERE id = ?1",
            params![id, tempo::agora()],
        )?;
    }
    Ok(format!("https://wa.me/{destino}?text={}", codificar(&mensagem)))
}

// ---------------------------------------------------------------------------
// Ler
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Linha {
    pub id: String,
    pub numero: i64,
    /// 0915-M: o caixa que fez a encomenda (o número é da tabela; dois caixas
    /// sem rede podem dar o mesmo). Como na OS (FALHA-02).
    pub caixa: String,
    pub cliente_id: String,
    pub cliente: String,
    pub telefone: String,
    pub tem_whatsapp: bool,
    pub produto_id: String,
    pub codigo: String,
    pub descricao: String,
    pub quantidade: String,
    pub unidade: String,
    pub situacao: String,
    pub nome_situacao: String,
    /// "hoje", "há 3 dias".
    pub desde: String,
    pub dias: i64,
    pub criado_em: String,
    pub chegou_em: String,
    pub avisado: bool,
    pub avisado_em: String,
    /// "pago", "falta" ou "entrega".
    pub pagamento: String,
    /// "Pago", "Falta R$ 150,00", "Paga na entrega".
    pub nome_pagamento: String,
    pub venda_id: String,
    pub venda_numero: i64,
    pub fornecedor: String,
    pub observacoes: String,
    pub motivo: String,
    /// Estoque atual da peça (se cadastrada).
    pub estoque: String,
    pub pode_vender: bool,
    pub pode_entregar: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Painel {
    pub a_pedir: i64,
    pub pedidas: i64,
    pub chegaram: i64,
    /// Chegaram e o cliente ainda não foi avisado.
    pub sem_aviso: i64,
    pub encomendas: Vec<Linha>,
}

fn desde(dias: i64) -> String {
    match dias {
        d if d <= 0 => "hoje".into(),
        1 => "ontem".into(),
        d => format!("há {d} dias"),
    }
}

/// `quais`: "abertas" (falta pedir, pedidas e que chegaram) ou "todas".
pub fn painel(conn: &Connection, busca: &str, quais: &str) -> Resultado<Painel> {
    let (a_pedir, pedidas, chegaram, sem_aviso): (i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(CASE WHEN situacao = 'aberta' THEN 1 END), COUNT(CASE WHEN situacao = 'pedida' THEN 1 END),
                COUNT(CASE WHEN situacao = 'chegou' THEN 1 END),
                COUNT(CASE WHEN situacao = 'chegou' AND avisado_em IS NULL THEN 1 END)
           FROM encomenda",
        [],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?;
    let filtro = texto::normalizar(busca);
    let hoje = fiscal::hoje(conn)?;
    let mut c = conn.prepare(
        "SELECT e.id, e.numero, COALESCE(e.cliente_id, ''), e.cliente_nome, e.telefone, COALESCE(e.produto_id, ''),
                COALESCE(p.codigo, ''), e.descricao, e.quantidade, COALESCE(p.unidade, 'UN'), e.situacao,
                date(e.criado_em, 'localtime'), COALESCE(date(e.chegou_em, 'localtime'), ''), e.avisado_em IS NOT NULL,
                COALESCE(strftime('%d/%m às %H:%M', e.avisado_em, 'localtime'), ''), COALESCE(e.venda_id, ''),
                COALESCE(v.numero, 0), COALESCE(v.situacao, ''), COALESCE(f.nome, ''), e.observacoes, e.motivo,
                COALESCE((SELECT SUM(valor - pago) FROM conta_receber_saldo r
                           WHERE r.venda_id = e.venda_id AND r.cancelado_em IS NULL), 0),
                (SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento m WHERE m.produto_id = e.produto_id),
                e.dispositivo
           FROM encomenda e
           LEFT JOIN produto p ON p.id = e.produto_id
           LEFT JOIN venda v ON v.id = e.venda_id
           LEFT JOIN pessoa f ON f.id = e.fornecedor_id
          WHERE (?1 = 'todas' OR e.situacao IN ('aberta', 'pedida', 'chegou'))
            AND (?2 = '' OR normalizar(e.cliente_nome || ' ' || e.descricao || ' ' || COALESCE(p.codigo, '') || ' ' || e.numero) LIKE '%' || ?2 || '%')
          ORDER BY CASE e.situacao WHEN 'chegou' THEN 0 WHEN 'aberta' THEN 1 WHEN 'pedida' THEN 2 ELSE 3 END,
                   e.criado_em DESC
          LIMIT 300",
    )?;
    let mut nomes = NomesDosCaixas::default();
    let linhas: Vec<(Linha, String)> = c
        .query_map(params![quais, filtro], |l| {
            let situacao: String = l.get(10)?;
            let criado: String = l.get(11)?;
            let venda_id: String = l.get(15)?;
            let venda_situacao: String = l.get(17)?;
            let falta: i64 = l.get(21)?;
            let telefone: String = l.get(4)?;
            let produto_id: String = l.get(5)?;
            let estoque: i64 = l.get(22)?;
            let dias = crate::receber::dias_entre(&criado, &hoje);
            let (pagamento, nome_pagamento) = if venda_id.is_empty() || venda_situacao != "concluida" {
                ("entrega", "Paga na entrega".to_string())
            } else if falta > 0 {
                ("falta", format!("Falta R$ {}", numeros::escrever_dinheiro(falta)))
            } else {
                ("pago", "Paga".to_string())
            };
            let aberta = situacao != "entregue" && situacao != "cancelada";
            Ok((
                Linha {
                    id: l.get(0)?,
                    numero: l.get(1)?,
                    caixa: String::new(),
                    cliente_id: l.get(2)?,
                    cliente: l.get(3)?,
                    tem_whatsapp: numero_whatsapp(&telefone).is_some(),
                    telefone,
                    pode_vender: aberta && venda_id.is_empty(),
                    pode_entregar: aberta && (!venda_id.is_empty() || produto_id.is_empty()),
                    estoque: if produto_id.is_empty() { String::new() } else { numeros::escrever_quantidade(estoque) },
                    produto_id,
                    codigo: l.get(6)?,
                    descricao: l.get(7)?,
                    quantidade: numeros::escrever_quantidade(l.get(8)?),
                    unidade: l.get(9)?,
                    nome_situacao: nome_situacao(&situacao).to_string(),
                    situacao,
                    desde: desde(dias),
                    dias,
                    criado_em: fiscal::data_br(&criado),
                    chegou_em: {
                        let c: String = l.get(12)?;
                        if c.is_empty() {
                            c
                        } else {
                            fiscal::data_br(&c)
                        }
                    },
                    avisado: l.get(13)?,
                    avisado_em: l.get(14)?,
                    pagamento: pagamento.into(),
                    nome_pagamento,
                    venda_id,
                    venda_numero: l.get(16)?,
                    fornecedor: l.get(18)?,
                    observacoes: l.get(19)?,
                    motivo: l.get(20)?,
                },
                l.get(23)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut encomendas = Vec::with_capacity(linhas.len());
    for (mut linha, dispositivo) in linhas {
        linha.caixa = nomes.nome(conn, &dispositivo)?;
        encomendas.push(linha);
    }
    Ok(Painel { a_pedir, pedidas, chegaram, sem_aviso, encomendas })
}

/// 0915-M: o nome do caixa de cada encomenda, lido uma vez por caixa.
#[derive(Default)]
pub(crate) struct NomesDosCaixas(std::collections::HashMap<String, String>);

impl NomesDosCaixas {
    pub(crate) fn nome(&mut self, conn: &Connection, dispositivo: &str) -> Resultado<String> {
        if let Some(n) = self.0.get(dispositivo) {
            return Ok(n.clone());
        }
        let n = nome_do_caixa(conn, dispositivo)?;
        self.0.insert(dispositivo.to_string(), n.clone());
        Ok(n)
    }
}

/// O nome do caixa, como na OS ("Caixa 2"; "outro caixa" se este computador
/// ainda não o conhece pelo nome).
fn nome_do_caixa(conn: &Connection, dispositivo: &str) -> Resultado<String> {
    if config::ler(conn, "dispositivo_id")?.as_deref() == Some(dispositivo) {
        return config::nome_caixa(conn);
    }
    Ok(conn
        .query_row("SELECT nome FROM sync_par WHERE dispositivo = ?1", [dispositivo], |l| l.get::<_, String>(0))
        .optional()?
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "outro caixa".into()))
}

/// O que a tela de vender precisa para vender a encomenda na entrega.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaVenda {
    pub encomenda_id: String,
    pub numero: i64,
    /// 0915-M: o caixa que fez a encomenda.
    pub caixa: String,
    pub cliente_id: String,
    pub cliente_nome: String,
    pub itens: Vec<serde_json::Value>,
}

pub fn para_venda(conn: &Connection, id: &str) -> Resultado<ParaVenda> {
    #[allow(clippy::type_complexity)]
    let (numero, cliente_id, cliente_nome, produto_id, quantidade, situacao, venda_id, dispositivo): (
        i64,
        Option<String>,
        String,
        Option<String>,
        i64,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT numero, cliente_id, cliente_nome, produto_id, quantidade, situacao, venda_id, dispositivo
               FROM encomenda WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?, l.get(7)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Encomenda".into()))?;
    if situacao == "entregue" || situacao == "cancelada" {
        return Err(Erro::validacao("Esta encomenda já foi encerrada."));
    }
    if venda_id.is_some() {
        return Err(Erro::validacao("Esta encomenda já foi vendida. Use “Entregar”."));
    }
    let itens = match produto_id {
        Some(p) => vec![serde_json::json!({
            "produtoId": p,
            "quantidade": numeros::escrever_quantidade(quantidade),
            "preco": "",
            "desconto": "",
            "encomendar": false
        })],
        None => Vec::new(),
    };
    Ok(ParaVenda {
        encomenda_id: id.to_string(),
        numero,
        caixa: nome_do_caixa(conn, &dispositivo)?,
        cliente_id: cliente_id.unwrap_or_default(),
        cliente_nome,
        itens,
    })
}

/// Para o aviso do Início: (chegaram sem avisar o cliente, faltam pedir).
pub fn do_dia(conn: &Connection) -> Resultado<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COUNT(CASE WHEN situacao = 'chegou' AND avisado_em IS NULL THEN 1 END),
                COUNT(CASE WHEN situacao = 'aberta' THEN 1 END)
           FROM encomenda",
        [],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?)
}

/// Quanto de cada peça está encomendado e ainda não chegou (pagas na venda),
/// e quanto já chegou e está guardado para o cliente (não pago).
pub fn reservas(conn: &Connection) -> Resultado<Vec<(String, i64, i64)>> {
    Ok(conn
        .prepare(
            "SELECT produto_id,
                    COALESCE(SUM(CASE WHEN venda_id IS NOT NULL AND situacao IN ('aberta', 'pedida') THEN quantidade END), 0),
                    COALESCE(SUM(CASE WHEN venda_id IS NULL AND situacao = 'chegou' THEN quantidade END), 0)
               FROM encomenda WHERE produto_id IS NOT NULL GROUP BY produto_id",
        )?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::vendas::{self, ItemEntrada, PagamentoEntrada, VendaEntrada};
    use crate::{caixa, compras, estoque, nfe_xml, pessoas, produtos, usuarios};

    fn peca(b: &Banco, u: &str, nome: &str, barras: &str) -> String {
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = nome.into();
        f.custo = "50".into();
        f.codigo_barras = barras.into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
    }

    /// 0915-M: dois caixas sem rede dão o mesmo número a encomendas
    /// diferentes (o número é da tabela, não por caixa). Depois da troca de
    /// dados, as listas (encomendas, o que comprar) e a venda da encomenda
    /// mostram o caixa junto do número, como na OS (FALHA-02). Nenhum número
    /// muda e nada é recusado.
    #[test]
    fn m0915_mesmo_numero_de_dois_caixas_mostra_o_caixa() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        crate::config::gravar(c, "caixa_nome", "Caixa 1").unwrap();
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let pastilha = peca(&b, &u, "Pastilha", "");
        let e = EncomendaEntrada {
            cliente_nome: "Ana Souza".into(),
            telefone: "(15) 91234-0000".into(),
            produto_id: pastilha.clone(),
            descricao: "Pastilha".into(),
            quantidade: "1".into(),
            ..Default::default()
        };
        let minha = criar(c, d, &u, &e).unwrap();
        // A do outro caixa, feita sem rede com o mesmo número, chegou pela troca.
        c.execute(
            "INSERT INTO sync_par (dispositivo, nome, endereco, visto_em) VALUES ('disp-caixa-2', 'Caixa 2', '', '2026-10-28')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO encomenda (id, numero, cliente_nome, telefone, produto_id, descricao, quantidade, situacao,
                                    criado_em, atualizado_em, dispositivo)
             SELECT 'enc-caixa-2', numero, 'Beto Lima', '(15) 98888-0000', produto_id, 'Pastilha', 1000, 'aberta',
                    criado_em, atualizado_em, 'disp-caixa-2' FROM encomenda WHERE id = ?1",
            [&minha],
        )
        .unwrap();
        let v = serde_json::to_value(painel(c, "", "abertas").unwrap()).unwrap();
        let linhas = v["encomendas"].as_array().unwrap();
        assert_eq!(linhas.len(), 2);
        assert_eq!(linhas[0]["numero"], linhas[1]["numero"], "o mesmo número (nada renumerado)");
        let caixas: Vec<&str> = linhas.iter().map(|l| l["caixa"].as_str().unwrap_or("")).collect();
        assert!(caixas.iter().all(|c| !c.is_empty()), "cada linha diz o caixa: {caixas:?}");
        assert_ne!(caixas[0], caixas[1], "os dois caixas aparecem diferentes: {caixas:?}");
        assert!(caixas.iter().any(|c| c.starts_with("Caixa 2")), "{caixas:?}");
        // A busca pelo número traz as duas, cada uma com o caixa.
        let v = serde_json::to_value(painel(c, "1", "abertas").unwrap()).unwrap();
        assert!(v["encomendas"].as_array().unwrap().iter().all(|l| !l["caixa"].as_str().unwrap_or("").is_empty()));
        // O que comprar (reposição) e a venda da encomenda também dizem o caixa.
        let r = serde_json::to_value(crate::reposicao::sugestao(c, "").unwrap()).unwrap();
        let enc = r["encomendas"].as_array().unwrap();
        assert_eq!(enc.len(), 2, "{r}");
        assert!(enc.iter().all(|l| !l["caixa"].as_str().unwrap_or("").is_empty()), "{r}");
        let pv = serde_json::to_value(para_venda(c, "enc-caixa-2").unwrap()).unwrap();
        assert!(pv["caixa"].as_str().unwrap_or("").starts_with("Caixa 2"), "{pv}");
        // A lista de compra impressa também.
        let html = crate::reposicao::lista_compra_html(c).unwrap();
        assert!(html.contains("Caixa 2") && html.contains("Caixa 1"), "{html}");
    }

    #[test]
    fn encomenda_paga_com_sinal_chega_pela_compra_e_e_entregue() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
        cli.nome = "ANA SOUZA".into();
        cli.celular = "(15) 91234-0000".into();
        let ana = pessoas::salvar(c, d, &u, &cli).unwrap();
        let rol = peca(&b, &u, "Rolamento roda traseira", "7891234567895"); // preço 100,00
        let pas = peca(&b, &u, "Pastilha", "");
        caixa::abrir(c, d, &u, "Caixa 1", "0").unwrap();

        // Venda com sinal: 50 em dinheiro e o resto no crediário.
        let mut e = VendaEntrada {
            itens: vec![ItemEntrada {
                produto_id: rol.clone(),
                quantidade: "2".into(),
                encomendar: true,
                ..Default::default()
            }],
            tabela: 1,
            pagamentos: vec![
                PagamentoEntrada { forma: "dinheiro".into(), valor: "50".into(), ..Default::default() },
                PagamentoEntrada { forma: "crediario".into(), valor: "150".into(), ..Default::default() },
            ],
            parcelas: 1,
            ..Default::default()
        };
        let calc = vendas::calcular(c, &e).unwrap();
        assert_eq!((calc.encomendas, calc.itens[0].falta.as_str()), (1, "2"));
        assert_eq!(vendas::concluir(c, d, &u, &e).unwrap_err().nome_do_campo(), Some("cliente"));
        e.cliente_id = ana.clone();
        let venda = vendas::concluir(c, d, &u, &e).unwrap();
        assert_eq!(estoque::saldo(c, &rol).unwrap(), -2000);
        let p = painel(c, "", "abertas").unwrap();
        assert_eq!(p.a_pedir, 1);
        let enc = &p.encomendas[0];
        assert_eq!((enc.situacao.as_str(), enc.quantidade.as_str(), enc.pagamento.as_str()), ("aberta", "2", "falta"));
        assert_eq!(enc.nome_pagamento, "Falta R$ 150,00");
        assert!(enc.tem_whatsapp && enc.pode_entregar && !enc.pode_vender);
        let enc_id = enc.id.clone();

        // Na sugestão de compra ela aparece como encomenda, não como reposição.
        let s = crate::reposicao::sugestao(c, "").unwrap();
        assert_eq!(s.encomendas.len(), 1);
        assert!(s.itens.is_empty(), "venda de encomenda não vira ritmo de venda");

        pedir(c, d, &u, &enc_id, false).unwrap();
        assert!(pedir(c, d, &u, &enc_id, false).is_err());

        // A nota chega: o rolamento liga pelo código de barras; o resto não entra.
        let compra = compras::importar_xml(c, d, &u, nfe_xml::EXEMPLO).unwrap();
        let f = compras::ficha(c, &compra).unwrap();
        let itens = f
            .itens
            .iter()
            .enumerate()
            .map(|(i, it)| compras::ItemEntrada {
                id: it.id.clone(),
                acao: if i == 0 { "ligar".into() } else { "ignorar".into() },
                produto_id: if i == 0 { rol.clone() } else { String::new() },
                fator: "1".into(),
                baixar_preco: false,
            })
            .collect();
        compras::atualizar(c, &compras::CompraEntrada { id: compra.clone(), itens, ..Default::default() }).unwrap();
        compras::liberar_para_teste(c, d, &compra);
        let r = compras::lancar(c, d, &u, &compra).unwrap();
        assert_eq!(r.encomendas, 1);
        assert_eq!(estoque::saldo(c, &rol).unwrap(), 8000);
        let p = painel(c, "", "abertas").unwrap();
        assert_eq!((p.chegaram, p.sem_aviso), (1, 1));
        assert_eq!(do_dia(c).unwrap(), (1, 0));

        let url = whatsapp(c, &enc_id).unwrap();
        assert!(url.starts_with("https://wa.me/5515912340000?text=Ol%C3%A1%2C%20Ana%21"), "{url}");
        assert_eq!(do_dia(c).unwrap(), (0, 0), "avisado");

        // Estorno da compra volta a encomenda para "pedida".
        compras::estornar(c, d, &u, &compra, "nota errada").unwrap();
        assert_eq!(atual(c, &enc_id).unwrap().situacao, "pedida");
        chegou(c, d, &u, &enc_id).unwrap();
        entregar(c, d, &u, &enc_id).unwrap();
        assert!(entregar(c, d, &u, &enc_id).is_err());

        // Encomenda para pagar quando chegar: anotada à parte e vendida na entrega.
        let id2 = criar(
            c,
            d,
            &u,
            &EncomendaEntrada {
                cliente_id: ana.clone(),
                produto_id: pas.clone(),
                quantidade: "1".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(entregar(c, d, &u, &id2).is_err(), "sem pagar não entrega");
        chegou(c, d, &u, &id2).unwrap();
        let pv = para_venda(c, &id2).unwrap();
        assert_eq!((pv.cliente_id.as_str(), pv.itens.len()), (ana.as_str(), 1));
        let e2 = VendaEntrada {
            itens: vec![ItemEntrada { produto_id: pas.clone(), quantidade: "1".into(), ..Default::default() }],
            tabela: 1,
            cliente_id: ana.clone(),
            pagamentos: vec![PagamentoEntrada { forma: "pix".into(), valor: "100".into(), ..Default::default() }],
            encomenda_id: id2.clone(),
            ..Default::default()
        };
        let v2 = vendas::concluir(c, d, &u, &e2).unwrap();
        assert_eq!(atual(c, &id2).unwrap().situacao, "entregue");
        // Cancelou a venda da entrega: a encomenda volta a "chegou".
        vendas::cancelar(c, d, &u, &v2, "engano").unwrap();
        assert_eq!(atual(c, &id2).unwrap().situacao, "chegou");
        cancelar(c, d, &u, &id2, "cliente desistiu").unwrap();

        // Encomenda paga na venda só cancela junto com a venda.
        let e3 = VendaEntrada {
            itens: vec![ItemEntrada {
                produto_id: pas.clone(),
                quantidade: "1".into(),
                encomendar: true,
                ..Default::default()
            }],
            tabela: 1,
            cliente_id: ana.clone(),
            pagamentos: vec![PagamentoEntrada { forma: "pix".into(), valor: "100".into(), ..Default::default() }],
            ..Default::default()
        };
        let v3 = vendas::concluir(c, d, &u, &e3).unwrap();
        let id3: String = c.query_row("SELECT id FROM encomenda WHERE venda_id = ?1", [&v3], |l| l.get(0)).unwrap();
        assert!(cancelar(c, d, &u, &id3, "x").unwrap_err().to_string().contains("Cancele a venda"));
        vendas::cancelar(c, d, &u, &v3, "desistiu").unwrap();
        assert_eq!(atual(c, &id3).unwrap().situacao, "cancelada");
        let _ = venda;
    }

    #[test]
    fn monta_o_numero_do_whatsapp() {
        assert_eq!(numero_whatsapp("(15) 91234-0000").as_deref(), Some("5515912340000"));
        assert_eq!(numero_whatsapp("015 3552-1234").as_deref(), Some("551535521234"));
        assert_eq!(numero_whatsapp("+55 15 91234-0000").as_deref(), Some("5515912340000"));
        assert_eq!(numero_whatsapp("91234-0000"), None);
        assert_eq!(codificar("Olá, Ana!"), "Ol%C3%A1%2C%20Ana%21");
        assert_eq!(primeiro_nome("JOÃO DA SILVA"), "João");
    }
}
