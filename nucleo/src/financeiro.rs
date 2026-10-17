//! Dinheiro da loja: contas (Caixa geral e bancos), categorias, movimentos
//! avulsos, transferências e o fluxo de caixa.
//!
//! Saldo de uma conta = saldo inicial + movimentos − pagamentos feitos dela
//! (só os de hoje para trás; cheque pré-datado só conta no "bom para").
//!
//! Fluxo de caixa: o que entrou e saiu da loja por dia. Entram as vendas
//! (menos crediário e cheque, que contam quando o dinheiro chega), os
//! recebimentos do crediário, os cheques depositados ou repassados e as
//! entradas avulsas. Saem os pagamentos e as saídas avulsas. Sangria,
//! transferência e conferência de saldo são só dinheiro mudando de lugar.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::erro::{Erro, Resultado};
use crate::receber::{iso, ler_iso};
use crate::{auditoria, fiscal, numeros, pagar, tempo, texto};

// ---------------------------------------------------------------------------
// Contas da loja.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContaLoja {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    pub saldo_inicial: String,
    pub saldo: String,
    pub ativo: bool,
    #[serde(skip)]
    pub v_saldo: i64,
}

/// Saldo de uma conta até a data (inclusive).
pub fn saldo(conn: &Connection, conta_id: &str, ate: &str) -> Resultado<i64> {
    Ok(conn.query_row(
        "SELECT l.saldo_inicial
              + COALESCE((SELECT SUM(m.valor) FROM movimento_conta m
                           WHERE m.conta_id = l.id AND m.estornado_em IS NULL AND m.data <= ?2), 0)
              - COALESCE((SELECT SUM(g.valor + g.juros - g.desconto) FROM pagamento g
                           WHERE g.conta_loja_id = l.id AND g.estornado_em IS NULL AND g.data <= ?2), 0)
           FROM conta_loja l WHERE l.id = ?1",
        params![conta_id, ate],
        |l| l.get(0),
    )?)
}

pub fn contas(conn: &Connection, inativas: bool) -> Resultado<Vec<ContaLoja>> {
    let hoje = fiscal::hoje(conn)?;
    let mut c = conn.prepare(&format!(
        "SELECT id, nome, tipo, saldo_inicial, ativo FROM conta_loja {} ORDER BY ativo DESC, ordem, nome COLLATE NOCASE",
        if inativas { "" } else { "WHERE ativo = 1" }
    ))?;
    let linhas: Vec<(String, String, String, i64, bool)> =
        c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)))?.collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for (id, nome, tipo, inicial, ativo) in linhas {
        let s = saldo(conn, &id, &hoje)?;
        lista.push(ContaLoja {
            id,
            nome,
            tipo,
            saldo_inicial: numeros::escrever_dinheiro(inicial),
            saldo: numeros::escrever_dinheiro(s),
            ativo,
            v_saldo: s,
        });
    }
    Ok(lista)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContaEntrada {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    pub saldo_inicial: String,
    pub ativo: bool,
}

/// Cria ou altera uma conta da loja.
pub fn salvar_conta(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &ContaEntrada) -> Resultado<String> {
    let nome = texto::limpar(&e.nome);
    if nome.is_empty() {
        return Err(Erro::campo("nome", "Dê um nome (ex.: Banco do Brasil, Sicoob)."));
    }
    if texto::tamanho(&nome) > 40 {
        return Err(Erro::campo("nome", "Use no máximo 40 letras."));
    }
    if e.tipo != "caixa" && e.tipo != "banco" {
        return Err(Erro::campo("tipo", "Escolha o tipo."));
    }
    let inicial = numeros::ler_dinheiro(&e.saldo_inicial).map_err(|m| Erro::campo("saldoInicial", m))?.unwrap_or(0);
    let repetida: Option<String> = conn
        .query_row(
            "SELECT id FROM conta_loja WHERE normalizar(nome) = normalizar(?1) AND id <> ?2",
            params![nome, e.id],
            |l| l.get(0),
        )
        .optional()?;
    if repetida.is_some() {
        return Err(Erro::campo("nome", "Já existe uma conta com este nome."));
    }
    let agora = tempo::agora();
    let id = if e.id.is_empty() {
        let id = tempo::novo_id();
        let ordem: i64 = conn.query_row("SELECT COALESCE(MAX(ordem), 0) + 1 FROM conta_loja", [], |l| l.get(0))?;
        conn.execute(
            "INSERT INTO conta_loja (id, nome, tipo, saldo_inicial, ativo, ordem, criado_em, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6)",
            params![id, nome, e.tipo, inicial, ordem, agora],
        )?;
        id
    } else {
        if e.id == "caixa-geral" && !e.ativo {
            return Err(Erro::campo("ativo", "O Caixa geral não pode ser desativado."));
        }
        let n = conn.execute(
            "UPDATE conta_loja SET nome = ?2, tipo = ?3, saldo_inicial = ?4, ativo = ?5, atualizado_em = ?6 WHERE id = ?1",
            params![e.id, nome, e.tipo, inicial, e.ativo, agora],
        )?;
        if n == 0 {
            return Err(Erro::NaoEncontrado("Conta".into()));
        }
        e.id.clone()
    };
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "financeiro.conta",
        "conta_loja",
        &id,
        &[format!("{nome} · saldo inicial R$ {}", numeros::escrever_dinheiro(inicial))],
    )?;
    Ok(id)
}

fn conta_ativa(conn: &Connection, id: &str, campo: &str) -> Resultado<String> {
    let nome: Option<(String, bool)> = conn
        .query_row("SELECT nome, ativo FROM conta_loja WHERE id = ?1", [id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?;
    match nome {
        Some((n, true)) => Ok(n),
        Some((_, false)) => Err(Erro::campo(campo, "Esta conta está desativada.")),
        None => Err(Erro::campo(campo, "Escolha a conta.")),
    }
}

// ---------------------------------------------------------------------------
// Categorias.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Categoria {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    pub ativo: bool,
}

pub fn categorias(conn: &Connection, inativas: bool) -> Resultado<Vec<Categoria>> {
    let mut c = conn.prepare(&format!(
        "SELECT id, nome, tipo, ativo FROM categoria {} ORDER BY tipo, ativo DESC, ordem, nome COLLATE NOCASE",
        if inativas { "" } else { "WHERE ativo = 1" }
    ))?;
    let linhas =
        c.query_map([], |l| Ok(Categoria { id: l.get(0)?, nome: l.get(1)?, tipo: l.get(2)?, ativo: l.get(3)? }))?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CategoriaEntrada {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    pub ativo: bool,
}

pub fn salvar_categoria(conn: &Connection, e: &CategoriaEntrada) -> Resultado<String> {
    let nome = texto::limpar(&e.nome);
    if nome.is_empty() {
        return Err(Erro::campo("nome", "Dê um nome à categoria."));
    }
    if texto::tamanho(&nome) > 40 {
        return Err(Erro::campo("nome", "Use no máximo 40 letras."));
    }
    let tipo = if e.tipo == "receita" { "receita" } else { "despesa" };
    let repetida: Option<String> = conn
        .query_row(
            "SELECT id FROM categoria WHERE normalizar(nome) = normalizar(?1) AND id <> ?2",
            params![nome, e.id],
            |l| l.get(0),
        )
        .optional()?;
    if repetida.is_some() {
        return Err(Erro::campo("nome", "Já existe uma categoria com este nome."));
    }
    let agora = tempo::agora();
    if e.id.is_empty() {
        let id = tempo::novo_id();
        let ordem: i64 = conn.query_row("SELECT COALESCE(MAX(ordem), 0) + 1 FROM categoria", [], |l| l.get(0))?;
        conn.execute(
            "INSERT INTO categoria (id, nome, tipo, ordem, ativo, atualizado_em) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            params![id, nome, tipo, ordem, agora],
        )?;
        Ok(id)
    } else {
        conn.execute(
            "UPDATE categoria SET nome = ?2, tipo = ?3, ativo = ?4, atualizado_em = ?5 WHERE id = ?1",
            params![e.id, nome, tipo, e.ativo, agora],
        )?;
        Ok(e.id.clone())
    }
}

// ---------------------------------------------------------------------------
// Movimentos avulsos, transferência e conferência.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MovimentoEntrada {
    pub conta_id: String,
    /// entrada ou saida
    pub tipo: String,
    pub valor: String,
    pub data: String,
    pub categoria_id: String,
    pub descricao: String,
}

fn ler_positivo(texto: &str, campo: &str) -> Resultado<i64> {
    numeros::ler_dinheiro(texto)
        .map_err(|m| Erro::campo(campo, m))?
        .filter(|v| *v > 0)
        .ok_or_else(|| Erro::campo(campo, "Digite o valor."))
}

fn ler_data_ate_hoje(conn: &Connection, texto: &str) -> Resultado<String> {
    let hoje = fiscal::hoje(conn)?;
    let data = fiscal::ler_data(texto).map_err(|m| Erro::campo("data", m))?.unwrap_or(hoje.clone());
    if data > hoje {
        return Err(Erro::campo("data", "A data não pode ser depois de hoje."));
    }
    Ok(data)
}

/// Entrada ou saída avulsa (tarifa do banco, rendimento, dinheiro do dono...).
pub fn lancar_movimento(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    e: &MovimentoEntrada,
) -> Resultado<String> {
    let nome_conta = conta_ativa(conn, &e.conta_id, "contaId")?;
    let sinal = match e.tipo.as_str() {
        "entrada" => 1,
        "saida" => -1,
        _ => return Err(Erro::campo("tipo", "Escolha entrada ou saída.")),
    };
    let valor = ler_positivo(&e.valor, "valor")?;
    let data = ler_data_ate_hoje(conn, &e.data)?;
    let descricao = texto::limpar(&e.descricao);
    if descricao.is_empty() {
        return Err(Erro::campo("descricao", "Diga do que é (ex.: tarifa do banco)."));
    }
    if texto::tamanho(&descricao) > 80 {
        return Err(Erro::campo("descricao", "Use no máximo 80 letras."));
    }
    let categoria = if e.categoria_id.is_empty() {
        None
    } else {
        conn.query_row("SELECT id FROM categoria WHERE id = ?1", [&e.categoria_id], |l| l.get::<_, String>(0))
            .optional()?
    };
    let id = tempo::novo_id();
    conn.execute(
        "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, categoria_id, descricao, usuario_id, criado_em, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, e.conta_id, data, sinal * valor, e.tipo, categoria, descricao, usuario_id, tempo::agora(), dispositivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "financeiro.movimento",
        "conta_loja",
        &e.conta_id,
        &[format!(
            "{} de R$ {} em {nome_conta}: {descricao}",
            if sinal > 0 { "Entrada" } else { "Saída" },
            numeros::escrever_dinheiro(valor)
        )],
    )?;
    Ok(id)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TransferenciaEntrada {
    pub de: String,
    pub para: String,
    pub valor: String,
    pub data: String,
    pub descricao: String,
}

/// Leva dinheiro de uma conta para outra (ex.: depósito do cofre no banco).
pub fn transferir(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &TransferenciaEntrada) -> Resultado<()> {
    let nome_de = conta_ativa(conn, &e.de, "de")?;
    let nome_para = conta_ativa(conn, &e.para, "para")?;
    if e.de == e.para {
        return Err(Erro::campo("para", "Escolha outra conta."));
    }
    let valor = ler_positivo(&e.valor, "valor")?;
    let data = ler_data_ate_hoje(conn, &e.data)?;
    let extra = texto::limpar(&e.descricao);
    let grupo = tempo::novo_id();
    let agora = tempo::agora();
    for (conta, sinal, texto_mov) in [(&e.de, -1, format!("Para {nome_para}")), (&e.para, 1, format!("De {nome_de}"))] {
        let descricao = if extra.is_empty() { texto_mov } else { format!("{texto_mov} · {extra}") };
        conn.execute(
            "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, descricao, grupo, usuario_id, criado_em, dispositivo)
             VALUES (?1, ?2, ?3, ?4, 'transferencia', ?5, ?6, ?7, ?8, ?9)",
            params![tempo::novo_id(), conta, data, sinal * valor, descricao, grupo, usuario_id, agora, dispositivo],
        )?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "financeiro.transferir",
        "conta_loja",
        &e.de,
        &[format!("R$ {} de {nome_de} para {nome_para}", numeros::escrever_dinheiro(valor))],
    )?;
    Ok(())
}

/// Confere o saldo com o extrato do banco (ou o dinheiro do cofre): a
/// diferença vira um ajuste.
pub fn conferir(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    conta_id: &str,
    saldo_real: &str,
) -> Resultado<i64> {
    let nome = conta_ativa(conn, conta_id, "contaId")?;
    let real = numeros::ler_dinheiro(saldo_real)
        .map_err(|m| Erro::campo("saldo", m))?
        .ok_or_else(|| Erro::campo("saldo", "Digite o saldo de verdade (do extrato ou contado)."))?;
    let hoje = fiscal::hoje(conn)?;
    let atual = saldo(conn, conta_id, &hoje)?;
    let diferenca = real - atual;
    if diferenca != 0 {
        conn.execute(
            "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, descricao, usuario_id, criado_em, dispositivo)
             VALUES (?1, ?2, ?3, ?4, 'ajuste', ?5, ?6, ?7, ?8)",
            params![
                tempo::novo_id(),
                conta_id,
                hoje,
                diferenca,
                format!("Conferência: saldo era R$ {}, de verdade R$ {}", numeros::escrever_dinheiro(atual), numeros::escrever_dinheiro(real)),
                usuario_id,
                tempo::agora(),
                dispositivo
            ],
        )?;
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "financeiro.conferir",
            "conta_loja",
            conta_id,
            &[format!("{nome}: ajuste de R$ {}", numeros::escrever_dinheiro(diferenca))],
        )?;
    }
    Ok(diferenca)
}

/// Desfaz um movimento avulso, transferência (as duas pontas) ou ajuste.
pub fn estornar_movimento(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    id: &str,
    motivo: &str,
) -> Resultado<()> {
    let (tipo, grupo, estornado, descricao, conta): (String, Option<String>, Option<String>, String, String) = conn
        .query_row(
            "SELECT tipo, grupo, estornado_em, descricao, conta_id FROM movimento_conta WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Movimento".into()))?;
    if estornado.is_some() {
        return Err(Erro::validacao("Este movimento já foi estornado."));
    }
    match tipo.as_str() {
        "entrada" | "saida" | "transferencia" | "ajuste" => {}
        "sangria" | "suprimento" => {
            return Err(Erro::validacao("Este movimento veio de uma sangria ou suprimento do caixa."))
        }
        _ => return Err(Erro::validacao("Este movimento é de um cheque; desfaça na lista de cheques.")),
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do estorno."));
    }
    let agora = tempo::agora();
    match grupo {
        Some(g) => conn.execute(
            "UPDATE movimento_conta SET estornado_em = ?2, estornado_por = ?3, motivo_estorno = ?4 WHERE grupo = ?1",
            params![g, agora, usuario_id, motivo],
        )?,
        None => conn.execute(
            "UPDATE movimento_conta SET estornado_em = ?2, estornado_por = ?3, motivo_estorno = ?4 WHERE id = ?1",
            params![id, agora, usuario_id, motivo],
        )?,
    };
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "financeiro.estornar",
        "conta_loja",
        &conta,
        &[format!("{descricao} · {motivo}")],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Extrato.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinhaExtrato {
    pub id: String,
    /// "movimento" ou "pagamento"
    pub origem: String,
    pub tipo: String,
    pub data: String,
    pub descricao: String,
    pub categoria: String,
    pub valor: String,
    pub entrada: bool,
    pub saldo: String,
    pub estornado: bool,
    pub motivo_estorno: String,
    pub futuro: bool,
    /// Dá para estornar aqui mesmo.
    pub estornavel: bool,
    pub conta_pagar_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extrato {
    pub conta: ContaLoja,
    pub linhas: Vec<LinhaExtrato>,
}

/// Extrato da conta, mais novo primeiro (até `limite` linhas).
pub fn extrato(conn: &Connection, conta_id: &str, limite: i64) -> Resultado<Extrato> {
    let conta = contas(conn, true)?
        .into_iter()
        .find(|c| c.id == conta_id)
        .ok_or_else(|| Erro::NaoEncontrado("Conta".into()))?;
    let hoje = fiscal::hoje(conn)?;
    let inicial: i64 =
        conn.query_row("SELECT saldo_inicial FROM conta_loja WHERE id = ?1", [conta_id], |l| l.get(0))?;
    let mut c = conn.prepare(
        "SELECT * FROM (
            SELECT m.id, 'movimento', m.tipo, m.data, m.descricao, COALESCE(g.nome, ''), m.valor,
                   m.estornado_em IS NOT NULL, m.motivo_estorno, m.criado_em, ''
              FROM movimento_conta m LEFT JOIN categoria g ON g.id = m.categoria_id
             WHERE m.conta_id = ?1
            UNION ALL
            SELECT p.id, 'pagamento', p.forma, p.data, c.descricao || CASE WHEN c.parcelas > 1 THEN ' · ' || c.parcela || '/' || c.parcelas ELSE '' END,
                   COALESCE(g.nome, ''), -(p.valor + p.juros - p.desconto),
                   p.estornado_em IS NOT NULL, p.motivo_estorno, p.criado_em, c.id
              FROM pagamento p JOIN conta_pagar c ON c.id = p.conta_pagar_id
              LEFT JOIN categoria g ON g.id = c.categoria_id
             WHERE p.conta_loja_id = ?1
         ) ORDER BY 4, 1",
    )?;
    #[allow(clippy::type_complexity)]
    let todas: Vec<(String, String, String, String, String, String, i64, bool, String, String, String)> = c
        .query_map([conta_id], |l| {
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
    let mut corrente = inicial;
    let mut linhas = Vec::new();
    for (id, origem, tipo, data, descricao, categoria, valor, estornado, motivo, _criado, conta_pagar_id) in todas {
        if !estornado {
            corrente += valor;
        }
        let nome_tipo = match (origem.as_str(), tipo.as_str()) {
            ("pagamento", f) => format!("Pagamento · {}", pagar::nome_forma(f)),
            (_, "entrada") => "Entrada".into(),
            (_, "saida") => "Saída".into(),
            (_, "transferencia") => "Transferência".into(),
            (_, "sangria") => "Sangria".into(),
            (_, "suprimento") => "Suprimento".into(),
            (_, "deposito_cheque") => "Depósito de cheque".into(),
            (_, "cheque_devolvido") => "Cheque devolvido".into(),
            (_, "ajuste") => "Ajuste".into(),
            (_, t) => t.to_string(),
        };
        linhas.push(LinhaExtrato {
            estornavel: !estornado
                && origem == "movimento"
                && matches!(tipo.as_str(), "entrada" | "saida" | "transferencia" | "ajuste"),
            id,
            futuro: data > hoje,
            data: fiscal::data_br(&data),
            descricao,
            categoria,
            valor: numeros::escrever_dinheiro(valor.abs()),
            entrada: valor >= 0,
            saldo: numeros::escrever_dinheiro(corrente),
            estornado,
            motivo_estorno: motivo,
            tipo: nome_tipo,
            origem,
            conta_pagar_id,
        });
    }
    linhas.reverse();
    linhas.truncate(limite.clamp(1, 2000) as usize);
    Ok(Extrato { conta, linhas })
}

// ---------------------------------------------------------------------------
// Resumo e fluxo de caixa.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resumo {
    pub saldo_contas: String,
    pub saldo_negativo: bool,
    pub pagar_vencido: String,
    pub pagar_hoje: String,
    pub pagar_7_dias: String,
}

pub fn resumo(conn: &Connection) -> Resultado<Resumo> {
    let total: i64 = contas(conn, false)?.iter().map(|c| c.v_saldo).sum();
    let p = pagar::painel(conn, "", "nenhuma")?;
    Ok(Resumo {
        saldo_contas: numeros::escrever_dinheiro(total),
        saldo_negativo: total < 0,
        pagar_vencido: p.vencido,
        pagar_hoje: p.vence_hoje,
        pagar_7_dias: p.vence_7_dias,
    })
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaFluxo {
    pub data: String,
    pub dia_semana: String,
    pub entradas: String,
    pub saidas: String,
    pub resultado: String,
    pub saldo: String,
    /// Proporções (0 a 1) para as barrinhas.
    pub barra_entrada: f64,
    pub barra_saida: f64,
    pub negativo: bool,
    pub hoje: bool,
    pub detalhes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fluxo {
    pub de: String,
    pub ate: String,
    pub entradas: String,
    pub saidas: String,
    pub resultado: String,
    pub resultado_negativo: bool,
    pub realizado: Vec<DiaFluxo>,
    pub saldo_hoje: String,
    /// Atrasados (vencidos e não pagos/recebidos) antes de hoje.
    pub atrasado_receber: String,
    pub atrasado_pagar: String,
    pub previsto: Vec<DiaFluxo>,
    pub saldo_previsto: String,
    pub saldo_previsto_negativo: bool,
}

const SEMANA: [&str; 7] = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"];

fn dia_semana(data: &str) -> String {
    ler_iso(data).map(|d| SEMANA[d.weekday().number_days_from_monday() as usize].to_string()).unwrap_or_default()
}

type Somas = BTreeMap<String, (i64, i64, Vec<(String, usize)>)>;

fn somar(mapa: &mut Somas, data: String, valor: i64, detalhe: String) {
    let e = mapa.entry(data).or_default();
    if valor >= 0 {
        e.0 += valor;
    } else {
        e.1 += -valor;
    }
    if let Some(c) = e.2.iter_mut().find(|c| c.0 == detalhe) {
        c.1 += 1;
    } else if e.2.len() < 6 {
        e.2.push((detalhe, 1));
    }
}

fn consultar(conn: &Connection, sql: &str, de: &str, ate: &str) -> Resultado<Vec<(String, i64, String)>> {
    let mut c = conn.prepare(sql)?;
    let linhas = c.query_map(params![de, ate], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Fluxo de caixa: o que aconteceu de `de` a `ate` e o previsto para os
/// próximos `dias` dias.
pub fn fluxo(conn: &Connection, de: &str, ate: &str, dias: i64) -> Resultado<Fluxo> {
    let hoje = fiscal::hoje(conn)?;
    let de = fiscal::ler_data(de).map_err(|m| Erro::campo("de", m))?.unwrap_or_else(|| format!("{}-01", &hoje[..7]));
    let ate = fiscal::ler_data(ate).map_err(|m| Erro::campo("ate", m))?.unwrap_or(hoje.clone());
    if ate < de {
        return Err(Erro::campo("ate", "A data final é antes da inicial."));
    }
    if crate::receber::dias_entre(&de, &ate) > 400 {
        return Err(Erro::campo("de", "Escolha no máximo um ano."));
    }

    // ---- Realizado ----
    let mut real: Somas = BTreeMap::new();
    for (data, valor, d) in consultar(
        conn,
        "SELECT date(v.criado_em, 'localtime'), SUM(p.valor) - MAX(CASE WHEN p.forma = 'dinheiro' THEN v.troco ELSE 0 END), 'Vendas'
           FROM venda_pagamento p JOIN venda v ON v.id = p.venda_id
          WHERE v.tipo = 'venda' AND v.situacao = 'concluida' AND p.forma NOT IN ('crediario', 'cheque', 'vale')
            AND date(v.criado_em, 'localtime') BETWEEN ?1 AND ?2
          GROUP BY v.id",
        &de,
        &ate,
    )? {
        somar(&mut real, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT date(criado_em, 'localtime'), valor, 'Crediário recebido' FROM recebimento
          WHERE estornado_em IS NULL AND forma <> 'cheque' AND date(criado_em, 'localtime') BETWEEN ?1 AND ?2",
        &de,
        &ate,
    )? {
        somar(&mut real, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT date(criado_em, 'localtime'), -(dinheiro + pix), 'Devolução a cliente' FROM devolucao
          WHERE cancelado_em IS NULL AND dinheiro + pix > 0 AND date(criado_em, 'localtime') BETWEEN ?1 AND ?2",
        &de,
        &ate,
    )? {
        somar(&mut real, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT date(situacao_em, 'localtime'), valor, 'Cheque ' || CASE situacao WHEN 'depositado' THEN 'depositado' ELSE 'repassado' END
           FROM cheque WHERE situacao IN ('depositado', 'repassado') AND date(situacao_em, 'localtime') BETWEEN ?1 AND ?2",
        &de,
        &ate,
    )? {
        somar(&mut real, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT data, valor, descricao FROM movimento_conta
          WHERE estornado_em IS NULL AND tipo IN ('entrada', 'saida', 'cheque_devolvido') AND data BETWEEN ?1 AND ?2",
        &de,
        &ate,
    )? {
        somar(&mut real, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT g.data, -(g.valor + g.juros - g.desconto), c.descricao FROM pagamento g JOIN conta_pagar c ON c.id = g.conta_pagar_id
          WHERE g.estornado_em IS NULL AND g.data BETWEEN ?1 AND ?2 AND g.data <= date('now', 'localtime')",
        &de,
        &ate,
    )? {
        somar(&mut real, data, valor, d);
    }
    let maior = real.values().map(|v| v.0.max(v.1)).max().unwrap_or(0).max(1) as f64;
    let (mut tot_e, mut tot_s, mut acumulado) = (0, 0, 0);
    let mut realizado = Vec::new();
    for (data, (e, s, det)) in &real {
        tot_e += e;
        tot_s += s;
        acumulado += e - s;
        realizado.push(DiaFluxo {
            dia_semana: dia_semana(data),
            hoje: *data == hoje,
            data: fiscal::data_br(data),
            entradas: numeros::escrever_dinheiro(*e),
            saidas: numeros::escrever_dinheiro(*s),
            resultado: numeros::escrever_dinheiro(e - s),
            saldo: numeros::escrever_dinheiro(acumulado),
            barra_entrada: *e as f64 / maior,
            barra_saida: *s as f64 / maior,
            negativo: e - s < 0,
            detalhes: resumir(det),
        });
    }
    realizado.reverse();

    // ---- Previsto ----
    let saldo_hoje: i64 = contas(conn, false)?.iter().map(|c| c.v_saldo).sum();
    let fim = iso(ler_iso(&hoje)? + Duration::days(dias.clamp(1, 180)));
    let (atr_receber, atr_pagar): (i64, i64) = (
        conn.query_row(
            "SELECT COALESCE(SUM(valor - pago), 0) FROM conta_receber_saldo WHERE cancelado_em IS NULL AND valor > pago AND vencimento < ?1",
            [&hoje],
            |l| l.get(0),
        )?,
        conn.query_row(
            "SELECT COALESCE(SUM(valor - pago), 0) FROM conta_pagar_saldo WHERE cancelado_em IS NULL AND valor > pago AND vencimento < ?1",
            [&hoje],
            |l| l.get(0),
        )?,
    );
    let mut prev: Somas = BTreeMap::new();
    for (data, valor, d) in consultar(
        conn,
        "SELECT vencimento, valor - pago, 'Crediário' FROM conta_receber_saldo
          WHERE cancelado_em IS NULL AND valor > pago AND vencimento BETWEEN ?1 AND ?2",
        &hoje,
        &fim,
    )? {
        somar(&mut prev, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT bom_para, valor, 'Cheque nº ' || numero FROM cheque WHERE situacao = 'carteira' AND bom_para BETWEEN ?1 AND ?2",
        &hoje,
        &fim,
    )? {
        somar(&mut prev, data, valor, d);
    }
    for (data, valor, d) in consultar(
        conn,
        "SELECT vencimento, -(valor - pago), descricao FROM conta_pagar_saldo
          WHERE cancelado_em IS NULL AND valor > pago AND vencimento BETWEEN ?1 AND ?2",
        &hoje,
        &fim,
    )? {
        somar(&mut prev, data, valor, d);
    }
    // Cheques que a loja passou com "bom para" no futuro: saem no dia.
    for (data, valor, d) in consultar(
        conn,
        "SELECT g.data, -(g.valor + g.juros - g.desconto), 'Cheque nº ' || g.cheque_numero || ' · ' || c.descricao
           FROM pagamento g JOIN conta_pagar c ON c.id = g.conta_pagar_id
          WHERE g.estornado_em IS NULL AND g.data > ?1 AND g.data <= ?2",
        &hoje,
        &fim,
    )? {
        somar(&mut prev, data, valor, d);
    }
    let maior = prev.values().map(|v| v.0.max(v.1)).max().unwrap_or(0).max(1) as f64;
    let mut corrente = saldo_hoje;
    let mut previsto = Vec::new();
    for (data, (e, s, det)) in &prev {
        corrente += e - s;
        previsto.push(DiaFluxo {
            dia_semana: dia_semana(data),
            hoje: *data == hoje,
            data: fiscal::data_br(data),
            entradas: numeros::escrever_dinheiro(*e),
            saidas: numeros::escrever_dinheiro(*s),
            resultado: numeros::escrever_dinheiro(e - s),
            saldo: numeros::escrever_dinheiro(corrente),
            barra_entrada: *e as f64 / maior,
            barra_saida: *s as f64 / maior,
            negativo: corrente < 0,
            detalhes: resumir(det),
        });
    }
    Ok(Fluxo {
        de: fiscal::data_br(&de),
        ate: fiscal::data_br(&ate),
        entradas: numeros::escrever_dinheiro(tot_e),
        saidas: numeros::escrever_dinheiro(tot_s),
        resultado: numeros::escrever_dinheiro(tot_e - tot_s),
        resultado_negativo: tot_e < tot_s,
        realizado,
        saldo_hoje: numeros::escrever_dinheiro(saldo_hoje),
        atrasado_receber: numeros::escrever_dinheiro(atr_receber),
        atrasado_pagar: numeros::escrever_dinheiro(atr_pagar),
        previsto,
        saldo_previsto: numeros::escrever_dinheiro(corrente),
        saldo_previsto_negativo: corrente < 0,
    })
}

/// [("Vendas", 2), ("Aluguel", 1)] → "Vendas (2)", "Aluguel".
fn resumir(itens: &[(String, usize)]) -> Vec<String> {
    itens.iter().map(|(t, n)| if *n > 1 { format!("{t} ({n})") } else { t.clone() }).collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::cheques::{self, ChequesEntrada};
    use crate::pagar::{LancamentoEntrada, PagamentoEntrada};
    use crate::receber::{self, ReceberEntrada};
    use crate::vendas::{self, ItemEntrada, PagamentoEntrada as Pg, VendaEntrada};
    use crate::{caixa, comissoes, pessoas, produtos, usuarios};

    struct Loja {
        b: Banco,
        u: String,
        banco: String,
        peca: String,
        cliente: String,
    }

    fn loja() -> Loja {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, &b.dispositivo);
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let banco = salvar_conta(
            c,
            d,
            &u,
            &ContaEntrada {
                nome: "Sicoob".into(),
                tipo: "banco".into(),
                saldo_inicial: "1.000".into(),
                ativo: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Bateria 60Ah".into();
        f.custo = "150".into();
        f.estoque_inicial = "10".into();
        let peca = produtos::salvar(c, d, &u, &f).unwrap(); // 300,00
        let mut cli = pessoas::ficha_nova(c, "cliente").unwrap();
        cli.nome = "Oficina do Zé".into();
        let cliente = pessoas::salvar(c, d, &u, &cli).unwrap();
        Loja { b, u, banco, peca, cliente }
    }

    fn saldo_de(l: &Loja, conta: &str) -> String {
        numeros::escrever_dinheiro(saldo(&l.b.conn, conta, &fiscal::hoje(&l.b.conn).unwrap()).unwrap())
    }

    #[test]
    fn contas_a_pagar_e_contas_da_loja() {
        let l = loja();
        let (c, d, u) = (&l.b.conn, &l.b.dispositivo, &l.u);
        assert_eq!(saldo_de(&l, &l.banco), "1.000,00");
        // nome repetido não
        assert!(salvar_conta(
            c,
            d,
            u,
            &ContaEntrada { nome: "sicoob".into(), tipo: "banco".into(), ..Default::default() }
        )
        .is_err());

        // aluguel em 1x vencendo hoje
        assert_eq!(
            pagar::lancar(c, d, u, &LancamentoEntrada { valor: "1500".into(), ..Default::default() })
                .unwrap_err()
                .nome_do_campo(),
            Some("descricao")
        );
        let ids = pagar::lancar(
            c,
            d,
            u,
            &LancamentoEntrada {
                descricao: "Aluguel de outubro".into(),
                categoria_id: "cat-aluguel".into(),
                valor: "1.500".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let conta = &ids[0];
        let p = pagar::painel(c, "aluguel", "abertas").unwrap();
        assert_eq!((p.vence_hoje.as_str(), p.parcelas.len(), p.parcelas[0].situacao.as_str()), ("1.500,00", 1, "hoje"));
        assert_eq!(p.parcelas[0].categoria, "Aluguel");
        assert_eq!(crate::avisos::do_dia(c).unwrap()[0].texto, "1 conta a pagar · R$ 1.500,00");

        // paga 500 no Pix pelo banco; o resto (+10 de juros) pelo Caixa geral
        let pg = |valor: &str, juros: &str, forma: &str, origem: &str| PagamentoEntrada {
            conta_id: conta.clone(),
            valor: valor.into(),
            juros: juros.into(),
            forma: forma.into(),
            origem: origem.into(),
            ..Default::default()
        };
        assert_eq!(pagar::pagar(c, d, u, &pg("2000", "", "pix", &l.banco)).unwrap_err().nome_do_campo(), Some("valor"));
        assert_eq!(pagar::pagar(c, d, u, &pg("500", "", "pix", "")).unwrap_err().nome_do_campo(), Some("origem"));
        pagar::pagar(c, d, u, &pg("500", "", "pix", &l.banco)).unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "500,00");
        assert_eq!(pagar::ficha(c, conta).unwrap().parcela.saldo, "1.000,00");
        let segundo = pagar::pagar(c, d, u, &pg("", "10", "dinheiro", "caixa-geral")).unwrap();
        assert_eq!(saldo_de(&l, "caixa-geral"), "-1.010,00");
        assert!(pagar::painel(c, "", "abertas").unwrap().parcelas.is_empty());
        assert_eq!(pagar::painel(c, "", "pagas").unwrap().parcelas[0].situacao, "paga");
        assert!(pagar::pagar(c, d, u, &pg("1", "", "pix", &l.banco)).is_err(), "já paga");
        // conta paga não se tira
        assert!(pagar::cancelar(c, d, u, conta, "erro").is_err());
        pagar::estornar(c, d, u, &segundo, "pago no lugar errado").unwrap();
        assert_eq!(saldo_de(&l, "caixa-geral"), "0,00");

        // da gaveta: vira sangria; não tira mais do que tem
        caixa::abrir(c, d, u, "Caixa 1", "100").unwrap();
        let s = caixa::sessao_aberta(c, d).unwrap().unwrap();
        assert_eq!(
            pagar::pagar(c, d, u, &pg("500", "", "dinheiro", "gaveta")).unwrap_err().nome_do_campo(),
            Some("valor")
        );
        assert_eq!(pagar::pagar(c, d, u, &pg("50", "", "pix", "gaveta")).unwrap_err().nome_do_campo(), Some("origem"));
        let gav = pagar::pagar(c, d, u, &pg("60", "", "dinheiro", "gaveta")).unwrap();
        let r = caixa::resumo(c, &s.id).unwrap();
        assert_eq!((r.sangrias.as_str(), r.esperado.as_str()), ("60,00", "40,00"));
        pagar::estornar(c, d, u, &gav, "engano").unwrap();
        assert_eq!(caixa::resumo(c, &s.id).unwrap().esperado, "100,00");

        // cheque da loja com bom para daqui a 10 dias: só sai do banco no dia
        let mut ch = pg("", "", "cheque", &l.banco);
        assert_eq!(pagar::pagar(c, d, u, &ch).unwrap_err().nome_do_campo(), Some("chequeNumero"));
        ch.cheque_numero = "850".into();
        ch.data = fiscal::data_br(&iso(ler_iso(&fiscal::hoje(c).unwrap()).unwrap() + Duration::days(10)));
        pagar::pagar(c, d, u, &ch).unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "500,00");
        let f = fluxo(c, "", "", 30).unwrap();
        assert_eq!(f.previsto.len(), 1);
        assert_eq!(f.previsto[0].saidas, "1.000,00");
        assert_eq!(f.saldo_previsto, "-500,00");
        // data no futuro só no cheque
        let mut futuro = pg("1", "", "pix", &l.banco);
        futuro.data = ch.data.clone();
        assert!(pagar::pagar(c, d, u, &futuro).is_err());

        // sangria que vai para o Caixa geral
        caixa::movimentar_com_conta(c, d, u, "sangria", "40", "Levar ao cofre", Some("caixa-geral")).unwrap();
        assert_eq!(saldo_de(&l, "caixa-geral"), "40,00");
        // transferência e conferência
        transferir(
            c,
            d,
            u,
            &TransferenciaEntrada {
                de: "caixa-geral".into(),
                para: l.banco.clone(),
                valor: "40".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((saldo_de(&l, "caixa-geral").as_str(), saldo_de(&l, &l.banco).as_str()), ("0,00", "540,00"));
        assert_eq!(conferir(c, d, u, &l.banco, "530").unwrap(), -1000);
        assert_eq!(saldo_de(&l, &l.banco), "530,00");
        let e = extrato(c, &l.banco, 50).unwrap();
        assert_eq!(e.linhas[0].tipo, "Pagamento · Cheque", "o cheque futuro fica em cima");
        assert!(e.linhas[0].futuro);
        assert_eq!(e.linhas[1].tipo, "Ajuste");
        assert!(e.linhas[1].estornavel);
        let ajuste = e.linhas[1].id.clone();
        estornar_movimento(c, d, u, &ajuste, "contei errado").unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "540,00");
        // movimento avulso
        lancar_movimento(
            c,
            d,
            u,
            &MovimentoEntrada {
                conta_id: l.banco.clone(),
                tipo: "saida".into(),
                valor: "12,90".into(),
                categoria_id: "cat-tarifas".into(),
                descricao: "Tarifa do mês".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "527,10");
        let f = fluxo(c, "", "", 30).unwrap();
        // saídas de hoje: 500 (Pix do aluguel) + 12,90 (tarifa)
        assert_eq!(f.realizado[0].saidas, "512,90");
        assert!(f.realizado[0].detalhes.iter().any(|x| x == "Tarifa do mês"));
        assert!(resumo(c).unwrap().saldo_contas.starts_with("527"));
    }

    #[test]
    fn cheques_da_venda_ao_deposito() {
        let l = loja();
        let (c, d, u) = (&l.b.conn, &l.b.dispositivo, &l.u);
        caixa::abrir(c, d, u, "Caixa 1", "0").unwrap();
        let mut e = VendaEntrada {
            itens: vec![ItemEntrada { produto_id: l.peca.clone(), quantidade: "1".into(), ..Default::default() }],
            tabela: 1,
            cliente_id: l.cliente.clone(),
            pagamentos: vec![Pg { forma: "cheque".into(), valor: "300".into(), ..Default::default() }],
            cheques: ChequesEntrada { quantidade: 3, banco: "Itaú".into(), ..Default::default() },
            ..Default::default()
        };
        let calc = vendas::calcular(c, &e).unwrap();
        assert_eq!((calc.cheques.cheques.len(), calc.cheques.campo.as_str()), (3, "chequeNumero"));
        assert_eq!(vendas::concluir(c, d, u, &e).unwrap_err().nome_do_campo(), Some("chequeNumero"));
        e.cheques.numero = "000101".into();
        let v = vendas::concluir(c, d, u, &e).unwrap();
        let ficha = vendas::ficha(c, &v).unwrap();
        assert_eq!(ficha.cheques.len(), 3);
        assert!(ficha.cheques[2].starts_with("nº 000103 · Itaú"), "{:?}", ficha.cheques);
        let p = cheques::painel(c, "", "carteira").unwrap();
        assert_eq!((p.em_carteira.as_str(), p.bons_ate_hoje.as_str(), p.quantos_em_carteira), ("300,00", "100,00", 3));
        assert!(p.cheques[0].pode_depositar && !p.cheques[1].pode_depositar);
        assert_eq!(p.cheques[0].emitente, "Oficina do Zé");
        assert_eq!(
            crate::avisos::do_dia(c).unwrap().iter().find(|a| a.id == "cheques").unwrap().texto,
            "1 cheque bom para depositar · R$ 100,00"
        );
        // cheque não é dinheiro na gaveta
        let s = caixa::sessao_aberta(c, d).unwrap().unwrap();
        assert_eq!(caixa::resumo(c, &s.id).unwrap().esperado, "0,00");
        // fluxo: o cheque conta quando o dinheiro chega
        assert!(fluxo(c, "", "", 90).unwrap().realizado.is_empty());
        assert_eq!(fluxo(c, "", "", 90).unwrap().previsto.len(), 3);

        // deposita o 1º; desfaz; deposita de novo
        let primeiro = p.cheques[0].id.clone();
        cheques::depositar(c, d, u, &primeiro, &l.banco).unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "1.100,00");
        cheques::desfazer(c, d, u, &primeiro).unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "1.000,00");
        cheques::depositar(c, d, u, &primeiro, &l.banco).unwrap();
        assert_eq!(fluxo(c, "", "", 90).unwrap().realizado[0].entradas, "100,00");
        // venda com cheque depositado não se cancela
        assert!(vendas::cancelar(c, d, u, &v, "desistiu").unwrap_err().to_string().contains("depositado"));
        // devolvido: sai do banco e vira dívida
        cheques::devolver(c, d, u, &primeiro, "Sem fundos", true).unwrap();
        assert_eq!(saldo_de(&l, &l.banco), "1.000,00");
        assert_eq!(receber::situacao(c, &l.cliente).unwrap().em_aberto, "100,00");
        // repassar o 2º
        let segundo = p.cheques[1].id.clone();
        assert!(cheques::repassar(c, d, u, &segundo, "").is_err());
        cheques::repassar(c, d, u, &segundo, "Distribuidora X").unwrap();
        assert_eq!(cheques::painel(c, "", "carteira").unwrap().cheques.len(), 1);
        assert_eq!(cheques::painel(c, "", "todos").unwrap().cheques.len(), 3);

        // crediário pago com cheque: estorno cancela o cheque
        let r = ReceberEntrada {
            cliente_id: l.cliente.clone(),
            forma: "cheque".into(),
            cheques: ChequesEntrada { numero: "555".into(), ..Default::default() },
            ..Default::default()
        };
        assert_eq!(receber::calcular(c, &r).unwrap().cheques.cheques.len(), 1);
        let rec = receber::receber(c, d, u, &r).unwrap();
        assert_eq!(cheques::painel(c, "555", "carteira").unwrap().cheques.len(), 1);
        receber::estornar(c, d, u, &rec, "errado").unwrap();
        assert!(cheques::painel(c, "555", "carteira").unwrap().cheques.is_empty());
    }

    #[test]
    fn comissao_do_periodo() {
        let l = loja();
        let (c, d, u) = (&l.b.conn, &l.b.dispositivo, &l.u);
        usuarios::definir_comissao(c, u, "5").unwrap();
        // uma peça com comissão própria de 10%
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Palheta".into();
        f.custo = "50".into();
        f.comissao = "10".into();
        f.estoque_inicial = "5".into();
        let palheta = produtos::salvar(c, d, u, &f).unwrap(); // 100,00
        caixa::abrir(c, d, u, "Caixa 1", "0").unwrap();
        // 300 + 100 = 400, com 10% de desconto na venda = 360
        let e = VendaEntrada {
            itens: vec![
                ItemEntrada { produto_id: l.peca.clone(), quantidade: "1".into(), ..Default::default() },
                ItemEntrada { produto_id: palheta, quantidade: "1".into(), ..Default::default() },
            ],
            tabela: 1,
            desconto: "10%".into(),
            pagamentos: vec![Pg { forma: "pix".into(), valor: "360".into(), ..Default::default() }],
            ..Default::default()
        };
        let v = vendas::concluir(c, d, u, &e).unwrap();
        let r = comissoes::relatorio(c, "", "").unwrap();
        let maria = &r.vendedores[0];
        // 270 × 5% + 90 × 10% = 13,50 + 9,00
        assert_eq!((maria.vendas, maria.vendido.as_str(), maria.comissao.as_str()), (1, "360,00", "22,50"));
        comissoes::fechar(c, d, u, u, "", "", "").unwrap();
        assert!(comissoes::fechar(c, d, u, u, "", "", "").unwrap_err().to_string().contains("já foi fechada"));
        let p = pagar::painel(c, "comissao", "abertas").unwrap();
        assert_eq!((p.parcelas[0].valor.as_str(), p.parcelas[0].categoria.as_str()), ("22,50", "Comissões"));
        let r = comissoes::relatorio(c, "", "").unwrap();
        assert!(!r.vendedores[0].ja_fechado.is_empty());
        comissoes::reabrir(c, d, u, &r.fechamentos[0].id).unwrap();
        assert!(pagar::painel(c, "comissao", "abertas").unwrap().parcelas.is_empty());
        // venda cancelada sai da comissão
        vendas::cancelar(c, d, u, &v, "desistiu").unwrap();
        assert_eq!(comissoes::relatorio(c, "", "").unwrap().vendedores[0].comissao, "0,00");
    }
}
