//! Cheques recebidos (na venda ou no crediário).
//!
//! O cheque fica "em carteira" até ser depositado numa conta da loja,
//! repassado (para um fornecedor, por exemplo) ou devolvido pelo banco.
//! Cheque devolvido pode virar dívida do cliente no Receber.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::receber::{self, dias_entre, iso, ler_iso, mais_meses, LancamentoEntrada};
use crate::{auditoria, fiscal, numeros, tempo, texto};

/// Máximo de cheques de uma vez.
pub const MAX_CHEQUES: i64 = 12;

/// Os cheques que vêm junto de uma venda ou recebimento.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChequesEntrada {
    /// Quantos cheques (vazio = 1).
    pub quantidade: i64,
    /// Bom para do 1º (vazio = hoje). Os outros, um mês depois do outro.
    pub primeiro_bom_para: String,
    pub banco: String,
    /// Número do 1º cheque; os seguintes vão em sequência.
    pub numero: String,
    /// Quem assinou (vazio = o cliente).
    pub emitente: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChequePrevia {
    pub numero: String,
    pub bom_para: String,
    pub valor: String,
    #[serde(skip)]
    pub v_bom_para: String,
    #[serde(skip)]
    pub v_valor: i64,
}

/// Próximo número de cheque: "000123" → "000124" (mantém os zeros).
fn seguinte(numero: &str, passos: i64) -> String {
    if passos == 0 || numero.is_empty() || !numero.chars().all(|c| c.is_ascii_digit()) {
        return numero.to_string();
    }
    match numero.parse::<u64>() {
        Ok(n) => format!("{:0largura$}", n + passos as u64, largura = numero.len()),
        Err(_) => numero.to_string(),
    }
}

/// Divide o valor nos cheques. Erro com o campo, para a tela destacar.
pub fn previa(conn: &Connection, valor: i64, e: &ChequesEntrada) -> Resultado<Vec<ChequePrevia>> {
    let n = if e.quantidade <= 0 { 1 } else { e.quantidade };
    if n > MAX_CHEQUES {
        return Err(Erro::campo("chequeQuantidade", format!("Use no máximo {MAX_CHEQUES} cheques.")));
    }
    if valor <= 0 {
        return Ok(vec![]);
    }
    let hoje = fiscal::hoje(conn)?;
    let primeiro = fiscal::ler_data(&e.primeiro_bom_para).map_err(|m| Erro::campo("chequeBomPara", m))?.unwrap_or(hoje);
    let inicio = ler_iso(&primeiro)?;
    let numero: String = texto::limpar(&e.numero).chars().filter(|c| !c.is_whitespace()).take(20).collect();
    Ok(receber::parcelar(inicio, inicio.day(), valor, n)
        .into_iter()
        .enumerate()
        .map(|(i, (data, v))| ChequePrevia {
            numero: seguinte(&numero, i as i64),
            bom_para: fiscal::data_br(&data),
            valor: numeros::escrever_dinheiro(v),
            v_bom_para: data,
            v_valor: v,
        })
        .collect())
}

/// Confere os dados antes de gravar (número é obrigatório).
pub fn conferir(e: &ChequesEntrada) -> Resultado<()> {
    if texto::limpar(&e.numero).is_empty() {
        return Err(Erro::campo("chequeNumero", "Digite o número do cheque."));
    }
    Ok(())
}

/// Grava os cheques de uma venda ou recebimento.
#[allow(clippy::too_many_arguments)]
pub fn criar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    cliente_id: Option<&str>,
    venda_id: Option<&str>,
    recebimento_id: Option<&str>,
    valor: i64,
    e: &ChequesEntrada,
) -> Resultado<usize> {
    conferir(e)?;
    let lista = previa(conn, valor, e)?;
    let mut emitente: String = texto::limpar(&e.emitente).chars().take(80).collect();
    if emitente.is_empty() {
        if let Some(c) = cliente_id {
            emitente = conn
                .query_row("SELECT nome FROM pessoa WHERE id = ?1", [c], |l| l.get(0))
                .optional()?
                .unwrap_or_default();
        }
    }
    let banco: String = texto::limpar(&e.banco).chars().take(40).collect();
    let agora = tempo::agora();
    for c in &lista {
        conn.execute(
            "INSERT INTO cheque (id, cliente_id, emitente, banco, numero, valor, bom_para, venda_id, recebimento_id,
                                 criado_em, criado_por, dispositivo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                tempo::novo_id(),
                cliente_id,
                emitente,
                banco,
                c.numero,
                c.v_valor,
                c.v_bom_para,
                venda_id,
                recebimento_id,
                agora,
                usuario_id,
                dispositivo
            ],
        )?;
    }
    Ok(lista.len())
}

/// Cancela os cheques de uma venda ou recebimento desfeito. Se algum já
/// saiu da carteira (depositado, repassado), não deixa.
pub fn cancelar_de(conn: &Connection, usuario_id: &str, coluna: &str, id: &str) -> Resultado<()> {
    debug_assert!(coluna == "venda_id" || coluna == "recebimento_id");
    let fora: Option<(String, String)> = conn
        .query_row(
            &format!("SELECT numero, situacao FROM cheque WHERE {coluna} = ?1 AND situacao NOT IN ('carteira', 'cancelado') LIMIT 1"),
            [id],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?;
    if let Some((numero, situacao)) = fora {
        return Err(Erro::validacao(format!(
            "O cheque nº {numero} já foi {}. Desfaça isso no Financeiro (Cheques) antes.",
            nome_situacao(&situacao).to_lowercase()
        )));
    }
    conn.execute(
        &format!("UPDATE cheque SET situacao = 'cancelado', situacao_em = ?2, situacao_por = ?3 WHERE {coluna} = ?1 AND situacao = 'carteira'"),
        params![id, tempo::agora(), usuario_id],
    )?;
    Ok(())
}

pub fn nome_situacao(s: &str) -> &'static str {
    match s {
        "carteira" => "Em carteira",
        "depositado" => "Depositado",
        "devolvido" => "Devolvido",
        "repassado" => "Repassado",
        "cancelado" => "Cancelado",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Lista.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cheque {
    pub id: String,
    pub cliente: String,
    pub cliente_id: String,
    pub emitente: String,
    pub banco: String,
    pub numero: String,
    pub valor: String,
    pub bom_para: String,
    pub situacao: String,
    pub nome_situacao: String,
    /// Em carteira e já pode depositar.
    pub pode_depositar: bool,
    /// Em carteira e passou do "bom para" há mais de 5 dias.
    pub atrasado: bool,
    pub dias_para: i64,
    pub conta: String,
    pub situacao_em: String,
    pub observacoes: String,
    pub venda_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Painel {
    pub em_carteira: String,
    pub quantos_em_carteira: i64,
    pub bons_ate_hoje: String,
    pub proximos_30_dias: String,
    pub devolvidos: String,
    pub cheques: Vec<Cheque>,
}

/// Cheques: "carteira" (padrão, por bom para) ou "todos" (mais novos antes).
pub fn painel(conn: &Connection, busca: &str, quais: &str) -> Resultado<Painel> {
    let hoje = fiscal::hoje(conn)?;
    let mes = iso(mais_meses(ler_iso(&hoje)?, 1, ler_iso(&hoje)?.day()));
    let (carteira, quantos, ate_hoje, prox, devolvidos): (i64, i64, i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN situacao = 'carteira' THEN valor END), 0),
                COUNT(CASE WHEN situacao = 'carteira' THEN 1 END),
                COALESCE(SUM(CASE WHEN situacao = 'carteira' AND bom_para <= ?1 THEN valor END), 0),
                COALESCE(SUM(CASE WHEN situacao = 'carteira' AND bom_para > ?1 AND bom_para <= ?2 THEN valor END), 0),
                COALESCE(SUM(CASE WHEN situacao = 'devolvido' THEN valor END), 0)
           FROM cheque",
        params![hoje, mes],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
    )?;
    let filtro = format!("%{}%", texto::normalizar(busca));
    let (onde, ordem) = match quais {
        "todos" => ("ch.situacao <> 'cancelado'", "ch.criado_em DESC"),
        _ => ("ch.situacao = 'carteira'", "ch.bom_para, ch.numero"),
    };
    let sql = format!(
        "SELECT ch.id, COALESCE(p.nome, ''), COALESCE(ch.cliente_id, ''), ch.emitente, ch.banco, ch.numero, ch.valor,
                ch.bom_para, ch.situacao, COALESCE(l.nome, ''), COALESCE(ch.situacao_em, ''), ch.observacoes,
                COALESCE(ch.venda_id, '')
           FROM cheque ch
           LEFT JOIN pessoa p ON p.id = ch.cliente_id
           LEFT JOIN conta_loja l ON l.id = ch.conta_loja_id
          WHERE {onde}
            AND (?1 = '%%' OR normalizar(ch.emitente || ' ' || COALESCE(p.nome, '') || ' ' || ch.numero || ' ' || ch.banco) LIKE ?1)
          ORDER BY {ordem} LIMIT 500"
    );
    let mut c = conn.prepare(&sql)?;
    let linhas = c.query_map([&filtro], |l| {
        Ok((
            l.get::<_, String>(0)?,
            l.get::<_, String>(1)?,
            l.get::<_, String>(2)?,
            l.get::<_, String>(3)?,
            l.get::<_, String>(4)?,
            l.get::<_, String>(5)?,
            l.get::<_, i64>(6)?,
            l.get::<_, String>(7)?,
            l.get::<_, String>(8)?,
            l.get::<_, String>(9)?,
            l.get::<_, String>(10)?,
            l.get::<_, String>(11)?,
            l.get::<_, String>(12)?,
        ))
    })?;
    let mut cheques = Vec::new();
    for linha in linhas {
        let (
            id,
            cliente,
            cliente_id,
            emitente,
            banco,
            numero,
            valor,
            bom_para,
            situacao,
            conta,
            situacao_em,
            observacoes,
            venda_id,
        ) = linha?;
        let dias = dias_entre(&hoje, &bom_para);
        let quando = if situacao_em.is_empty() { String::new() } else { fiscal::data_local_br(conn, &situacao_em)? };
        cheques.push(Cheque {
            id,
            cliente,
            cliente_id,
            emitente,
            banco,
            numero,
            valor: numeros::escrever_dinheiro(valor),
            bom_para: fiscal::data_br(&bom_para),
            nome_situacao: nome_situacao(&situacao).into(),
            pode_depositar: situacao == "carteira" && dias <= 0,
            atrasado: situacao == "carteira" && dias < -5,
            situacao,
            dias_para: dias,
            conta,
            situacao_em: quando,
            observacoes,
            venda_id,
        });
    }
    Ok(Painel {
        em_carteira: numeros::escrever_dinheiro(carteira),
        quantos_em_carteira: quantos,
        bons_ate_hoje: numeros::escrever_dinheiro(ate_hoje),
        proximos_30_dias: numeros::escrever_dinheiro(prox),
        devolvidos: numeros::escrever_dinheiro(devolvidos),
        cheques,
    })
}

// ---------------------------------------------------------------------------
// Depositar, repassar, devolver, desfazer.
// ---------------------------------------------------------------------------

struct Dados {
    numero: String,
    valor: i64,
    situacao: String,
    cliente_id: Option<String>,
    conta_loja_id: Option<String>,
}

fn dados(conn: &Connection, id: &str) -> Resultado<Dados> {
    conn.query_row("SELECT numero, valor, situacao, cliente_id, conta_loja_id FROM cheque WHERE id = ?1", [id], |l| {
        Ok(Dados {
            numero: l.get(0)?,
            valor: l.get(1)?,
            situacao: l.get(2)?,
            cliente_id: l.get(3)?,
            conta_loja_id: l.get(4)?,
        })
    })
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Cheque".into()))
}

#[allow(clippy::too_many_arguments)]
fn movimento(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    conta: &str,
    valor: i64,
    tipo: &str,
    descricao: &str,
    cheque_id: &str,
) -> Resultado<()> {
    conn.execute(
        "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, descricao, origem_id, usuario_id, criado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            tempo::novo_id(),
            conta,
            fiscal::hoje(conn)?,
            valor,
            tipo,
            descricao,
            cheque_id,
            usuario_id,
            tempo::agora(),
            dispositivo
        ],
    )?;
    Ok(())
}

fn mudar(
    conn: &Connection,
    usuario_id: &str,
    id: &str,
    situacao: &str,
    conta: Option<&str>,
    observacoes: Option<&str>,
) -> Resultado<()> {
    conn.execute(
        "UPDATE cheque SET situacao = ?2, situacao_em = ?3, situacao_por = ?4, conta_loja_id = ?5,
                observacoes = COALESCE(?6, observacoes) WHERE id = ?1",
        params![id, situacao, tempo::agora(), usuario_id, conta, observacoes],
    )?;
    Ok(())
}

/// Deposita o cheque numa conta da loja (o valor entra nela).
pub fn depositar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, conta: &str) -> Resultado<()> {
    let d = dados(conn, id)?;
    if d.situacao != "carteira" {
        return Err(Erro::validacao("Só dá para depositar cheque em carteira."));
    }
    let ativa: Option<bool> =
        conn.query_row("SELECT ativo FROM conta_loja WHERE id = ?1", [conta], |l| l.get(0)).optional()?;
    if ativa != Some(true) {
        return Err(Erro::campo("conta", "Escolha em qual conta depositou."));
    }
    movimento(
        conn,
        dispositivo,
        usuario_id,
        conta,
        d.valor,
        "deposito_cheque",
        &format!("Depósito do cheque nº {}", d.numero),
        id,
    )?;
    mudar(conn, usuario_id, id, "depositado", Some(conta), None)?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "cheque.depositar",
        "cheque",
        id,
        &[format!("Cheque nº {} · R$ {}", d.numero, numeros::escrever_dinheiro(d.valor))],
    )?;
    Ok(())
}

/// Repassa o cheque (para um fornecedor, por exemplo).
pub fn repassar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, para_quem: &str) -> Resultado<()> {
    let d = dados(conn, id)?;
    if d.situacao != "carteira" {
        return Err(Erro::validacao("Só dá para repassar cheque em carteira."));
    }
    let para_quem = texto::limpar(para_quem);
    if para_quem.is_empty() {
        return Err(Erro::campo("motivo", "Diga para quem foi o cheque."));
    }
    mudar(conn, usuario_id, id, "repassado", None, Some(&format!("Repassado para {para_quem}")))?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "cheque.repassar",
        "cheque",
        id,
        &[format!("Cheque nº {} para {para_quem}", d.numero)],
    )?;
    Ok(())
}

/// O banco devolveu o cheque. Se tinha sido depositado, o valor sai da
/// conta. Se `divida`, vira dívida do cliente no Receber (vence hoje).
pub fn devolver(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    id: &str,
    motivo: &str,
    divida: bool,
) -> Resultado<()> {
    let d = dados(conn, id)?;
    if d.situacao != "carteira" && d.situacao != "depositado" {
        return Err(Erro::validacao("Este cheque não está em carteira nem depositado."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo (ex.: sem fundos)."));
    }
    if d.situacao == "depositado" {
        if let Some(conta) = &d.conta_loja_id {
            movimento(
                conn,
                dispositivo,
                usuario_id,
                conta,
                -d.valor,
                "cheque_devolvido",
                &format!("Cheque nº {} devolvido · {motivo}", d.numero),
                id,
            )?;
        }
    }
    if divida {
        let cliente = d
            .cliente_id
            .clone()
            .ok_or_else(|| Erro::validacao("O cheque não tem cliente; não dá para lançar como dívida."))?;
        receber::lancar(
            conn,
            dispositivo,
            usuario_id,
            &LancamentoEntrada {
                cliente_id: cliente,
                descricao: format!("Cheque nº {} devolvido", d.numero),
                valor: numeros::escrever_dinheiro(d.valor),
                parcelas: 1,
                primeiro_vencimento: fiscal::data_br(&fiscal::hoje(conn)?),
            },
        )?;
    }
    mudar(conn, usuario_id, id, "devolvido", d.conta_loja_id.as_deref(), Some(&format!("Devolvido: {motivo}")))?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "cheque.devolver",
        "cheque",
        id,
        &[format!("Cheque nº {} · {motivo}", d.numero)],
    )?;
    Ok(())
}

/// Volta para a carteira (depositou ou repassou por engano).
pub fn desfazer(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let d = dados(conn, id)?;
    match d.situacao.as_str() {
        "depositado" => {
            conn.execute(
                "UPDATE movimento_conta SET estornado_em = ?2, estornado_por = ?3, motivo_estorno = 'Depósito desfeito'
                  WHERE origem_id = ?1 AND tipo = 'deposito_cheque' AND estornado_em IS NULL",
                params![id, tempo::agora(), usuario_id],
            )?;
        }
        "repassado" => {}
        _ => return Err(Erro::validacao("Só dá para desfazer depósito ou repasse.")),
    }
    conn.execute(
        "UPDATE cheque SET situacao = 'carteira', situacao_em = ?2, situacao_por = ?3, conta_loja_id = NULL, observacoes = ''
          WHERE id = ?1",
        params![id, tempo::agora(), usuario_id],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "cheque.desfazer",
        "cheque",
        id,
        &[format!("Cheque nº {} voltou para a carteira", d.numero)],
    )?;
    Ok(())
}

/// Cheques em carteira bons até hoje: (quantos, valor).
pub fn do_dia(conn: &Connection) -> Resultado<(i64, i64)> {
    let hoje = fiscal::hoje(conn)?;
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(valor), 0) FROM cheque WHERE situacao = 'carteira' AND bom_para <= ?1",
        [hoje],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numeros_em_sequencia() {
        assert_eq!(seguinte("000123", 2), "000125");
        assert_eq!(seguinte("99", 1), "100");
        assert_eq!(seguinte("AB12", 1), "AB12");
        assert_eq!(seguinte("", 3), "");
    }
}
