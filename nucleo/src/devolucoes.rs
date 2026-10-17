//! Trocas e devoluções.
//!
//! O cliente devolve uma ou mais peças de uma venda. A peça volta para a
//! prateleira (ou não, se veio com defeito) e o valor volta ao cliente de um
//! destes jeitos:
//! - **vale-troca**: crédito para usar em outra compra (é assim a troca);
//! - **dinheiro**: sai da gaveta do caixa aberto;
//! - **Pix ou cartão**: devolvido fora do caixa (Pix ou estorno na maquininha);
//! - **crediário**: abate o que o cliente ainda deve desta venda.
//!
//! O valor de cada peça é o que o cliente pagou por ela, já com a parte do
//! desconto da venda. Tudo pode ser desfeito (com motivo) enquanto o vale
//! não tiver sido usado.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, caixa, estoque, numeros, tempo, texto};

pub const DESTINOS: &[(&str, &str)] =
    &[("vale", "Vale-troca"), ("dinheiro", "Dinheiro"), ("pix", "Pix ou cartão"), ("crediario", "Abater do crediário")];

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemEntrada {
    pub venda_item_id: String,
    pub quantidade: String,
    /// Com defeito: não volta para a prateleira.
    pub defeito: bool,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entrada {
    pub venda_id: String,
    pub itens: Vec<ItemEntrada>,
    /// vale, dinheiro, pix ou crediario
    pub destino: String,
    /// Crediário: o que passar do que o cliente deve vai para cá.
    pub resto: String,
    pub motivo: String,
}

/// Um item da venda, para escolher o que volta.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemVenda {
    pub venda_item_id: String,
    pub produto_id: String,
    pub codigo: String,
    pub descricao: String,
    pub unidade: String,
    pub vendido: String,
    pub devolvido: String,
    /// Quanto ainda pode voltar.
    pub pode: String,
    /// O que o cliente pagou por unidade (com o desconto da venda).
    pub valor_unitario: String,
    #[serde(skip)]
    v_quantidade: i64,
    #[serde(skip)]
    v_total: i64,
    #[serde(skip)]
    v_liquido: i64,
    #[serde(skip)]
    v_devolvido_q: i64,
    #[serde(skip)]
    v_devolvido_valor: i64,
    #[serde(skip)]
    v_devolvido_item: i64,
    #[serde(skip)]
    v_custo: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevolucaoResumo {
    pub id: String,
    pub numero: i64,
    pub caixa: String,
    pub venda_id: String,
    pub venda_numero: i64,
    pub venda_caixa: String,
    pub cliente: String,
    pub valor: String,
    /// "Vale-troca nº 3", "Dinheiro"...
    pub como: String,
    pub itens: String,
    pub motivo: String,
    pub usuario: String,
    pub criado_em: String,
    pub quando: String,
    pub cancelada: bool,
    pub vale_id: String,
    /// O vale já foi usado (não dá para desfazer).
    pub vale_usado: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaDevolver {
    pub venda_id: String,
    pub numero: i64,
    pub caixa: String,
    pub quando: String,
    pub cliente_id: String,
    pub cliente: String,
    pub total: String,
    pub situacao: String,
    pub itens: Vec<ItemVenda>,
    /// O que o cliente ainda deve desta venda no crediário.
    pub crediario_aberto: String,
    pub devolucoes: Vec<DevolucaoResumo>,
}

fn dinheiro(v: i64) -> String {
    numeros::escrever_dinheiro(v)
}

/// Itens da venda com o que já foi devolvido.
fn itens_da_venda(conn: &Connection, venda_id: &str) -> Resultado<Vec<ItemVenda>> {
    let total_venda: i64 = conn.query_row("SELECT total FROM venda WHERE id = ?1", [venda_id], |l| l.get(0))?;
    let soma_itens: i64 =
        conn.query_row("SELECT COALESCE(SUM(total), 0) FROM venda_item WHERE venda_id = ?1", [venda_id], |l| l.get(0))?;
    let mut c = conn.prepare(
        "SELECT i.id, COALESCE(i.produto_id, ''), i.codigo, i.descricao, i.unidade, i.quantidade, i.total, i.custo,
                COALESCE((SELECT SUM(di.quantidade) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL), 0),
                COALESCE((SELECT SUM(di.valor) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL), 0),
                COALESCE((SELECT SUM(di.valor_item) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
                           WHERE di.venda_item_id = i.id AND d.cancelado_em IS NULL), 0)
           FROM venda_item i WHERE i.venda_id = ?1 ORDER BY i.ordem",
    )?;
    let linhas = c.query_map([venda_id], |l| {
        let q: i64 = l.get(5)?;
        let total: i64 = l.get(6)?;
        let dq: i64 = l.get(8)?;
        // O que o cliente pagou pelo item: o total dele menos a parte do
        // desconto da venda inteira.
        let liquido = if soma_itens > 0 {
            crate::numeros::saturar((total as i128 * total_venda as i128 + soma_itens as i128 / 2) / soma_itens as i128)
        } else {
            0
        };
        let unidade: String = l.get(4)?;
        Ok(ItemVenda {
            venda_item_id: l.get(0)?,
            produto_id: l.get(1)?,
            codigo: l.get(2)?,
            descricao: l.get(3)?,
            vendido: numeros::escrever_quantidade(q),
            devolvido: if dq > 0 { numeros::escrever_quantidade(dq) } else { String::new() },
            pode: numeros::escrever_quantidade((q - dq).max(0)),
            valor_unitario: dinheiro(if q > 0 {
                crate::numeros::saturar((liquido as i128 * 1000 + q as i128 / 2) / q as i128)
            } else {
                0
            }),
            unidade,
            v_quantidade: q,
            v_total: total,
            v_liquido: liquido,
            v_devolvido_q: dq,
            v_devolvido_valor: l.get(9)?,
            v_devolvido_item: l.get(10)?,
            v_custo: l.get(7)?,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// O que o cliente ainda deve desta venda no crediário.
fn crediario_aberto(conn: &Connection, venda_id: &str) -> Resultado<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(valor - pago), 0) FROM conta_receber_saldo
          WHERE venda_id = ?1 AND cancelado_em IS NULL AND valor > pago",
        [venda_id],
        |l| l.get(0),
    )?)
}

pub fn da_venda(conn: &Connection, venda_id: &str) -> Resultado<ParaDevolver> {
    let v: (String, i64, String, Option<String>, String, i64, String, String) = conn
        .query_row(
            "SELECT tipo, numero, caixa_nome, cliente_id, cliente_nome, total, situacao, criado_em FROM venda WHERE id = ?1",
            [venda_id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?, l.get(7)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Venda".into()))?;
    if v.0 != "venda" {
        return Err(Erro::validacao("Orçamento não tem devolução."));
    }
    Ok(ParaDevolver {
        venda_id: venda_id.to_string(),
        numero: v.1,
        caixa: v.2,
        quando: quando(conn, &v.7)?,
        cliente_id: v.3.unwrap_or_default(),
        cliente: v.4,
        total: dinheiro(v.5),
        situacao: v.6,
        itens: itens_da_venda(conn, venda_id)?,
        crediario_aberto: dinheiro(crediario_aberto(conn, venda_id)?),
        devolucoes: listar_onde(conn, "d.venda_id = ?1", venda_id, 50)?,
    })
}

fn quando(conn: &Connection, iso: &str) -> Resultado<String> {
    Ok(conn
        .query_row("SELECT strftime('%d/%m/%Y %H:%M', ?1, 'localtime')", [iso], |l| l.get::<_, Option<String>>(0))?
        .unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Calcular (prévia) e devolver
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Linha {
    venda_item_id: String,
    produto_id: String,
    descricao: String,
    unidade: String,
    quantidade: i64,
    valor_item: i64,
    valor: i64,
    custo: i64,
    defeito: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calculo {
    pub valor: String,
    pub itens: i64,
    /// Como o valor volta: [{ destino, nome, valor }]
    pub partes: Vec<Parte>,
    pub crediario_aberto: String,
    /// O valor passa do que o cliente deve: escolher para onde vai o resto.
    pub precisa_resto: bool,
    pub erro: String,
    pub campo: String,
    #[serde(skip)]
    v_valor: i64,
    #[serde(skip)]
    linhas: Vec<Linha>,
    #[serde(skip)]
    v_partes: (i64, i64, i64, i64),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parte {
    pub destino: String,
    pub nome: String,
    pub valor: String,
}

fn nome_destino(d: &str) -> &'static str {
    DESTINOS.iter().find(|x| x.0 == d).map(|x| x.1).unwrap_or("")
}

pub fn calcular(conn: &Connection, e: &Entrada) -> Resultado<Calculo> {
    let mut c = Calculo::default();
    let situacao: Option<(String, String)> = conn
        .query_row("SELECT tipo, situacao FROM venda WHERE id = ?1", [&e.venda_id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?;
    match situacao {
        None => return Err(Erro::NaoEncontrado("Venda".into())),
        Some((t, _)) if t != "venda" => return Err(Erro::validacao("Orçamento não tem devolução.")),
        Some((_, s)) if s != "concluida" => {
            c.erro = "Esta venda foi cancelada: as peças já voltaram ao estoque.".into();
            return Ok(c);
        }
        _ => {}
    }
    let itens = itens_da_venda(conn, &e.venda_id)?;
    for (n, it) in e.itens.iter().enumerate() {
        let q = match numeros::ler_quantidade(&it.quantidade) {
            Ok(Some(q)) if q > 0 => q,
            Ok(_) => continue,
            Err(m) => {
                c.erro = m;
                c.campo = format!("itens.{n}");
                return Ok(c);
            }
        };
        let Some(v) = itens.iter().find(|x| x.venda_item_id == it.venda_item_id) else {
            return Err(Erro::Interno("item não é desta venda".into()));
        };
        let pode = v.v_quantidade - v.v_devolvido_q;
        if q > pode {
            c.erro = if pode <= 0 {
                format!("{}: já foi devolvido todo.", v.descricao)
            } else {
                format!(
                    "{}: dá para devolver no máximo {} {}.",
                    v.descricao,
                    numeros::escrever_quantidade(pode),
                    v.unidade
                )
            };
            c.campo = format!("itens.{n}");
            return Ok(c);
        }
        // Na última parte do item, devolve o que sobrou (sem perder centavo
        // no arredondamento).
        let (valor, valor_item) = if q == pode {
            (v.v_liquido - v.v_devolvido_valor, v.v_total - v.v_devolvido_item)
        } else {
            (
                crate::numeros::saturar(
                    (v.v_liquido as i128 * q as i128 + v.v_quantidade as i128 / 2) / v.v_quantidade as i128,
                ),
                crate::numeros::saturar(
                    (v.v_total as i128 * q as i128 + v.v_quantidade as i128 / 2) / v.v_quantidade as i128,
                ),
            )
        };
        c.linhas.push(Linha {
            venda_item_id: v.venda_item_id.clone(),
            produto_id: v.produto_id.clone(),
            descricao: v.descricao.clone(),
            unidade: v.unidade.clone(),
            quantidade: q,
            valor_item,
            valor,
            custo: v.v_custo,
            defeito: it.defeito,
        });
    }
    let total: i64 = c.linhas.iter().map(|l| l.valor).sum();
    c.v_valor = total;
    c.valor = dinheiro(total);
    c.itens = c.linhas.len() as i64;
    let aberto = crediario_aberto(conn, &e.venda_id)?;
    c.crediario_aberto = dinheiro(aberto);
    if c.linhas.is_empty() {
        return Ok(c);
    }
    let (mut din, mut pix, mut vale, mut cred) = (0, 0, 0, 0);
    let mut por = |d: &str, v: i64| -> Result<(), String> {
        match d {
            "dinheiro" => din += v,
            "pix" => pix += v,
            "vale" => vale += v,
            _ => return Err("Escolha como devolver o valor.".into()),
        }
        Ok(())
    };
    match e.destino.as_str() {
        "crediario" => {
            if aberto <= 0 {
                c.erro = "O cliente não deve nada desta venda no crediário.".into();
                c.campo = "destino".into();
                return Ok(c);
            }
            cred = total.min(aberto);
            if total > aberto {
                c.precisa_resto = true;
                if let Err(m) = por(&e.resto, total - aberto) {
                    c.erro = format!("O cliente devia só R$ {} desta venda. {m}", dinheiro(aberto));
                    c.campo = "resto".into();
                }
            }
        }
        "" => {}
        d => {
            if let Err(m) = por(d, total) {
                c.erro = m;
                c.campo = "destino".into();
            }
        }
    }
    let mut partes = Vec::new();
    for (d, v) in [("crediario", cred), ("vale", vale), ("dinheiro", din), ("pix", pix)] {
        if v > 0 {
            partes.push(Parte { destino: d.into(), nome: nome_destino(d).into(), valor: dinheiro(v) });
        }
    }
    c.partes = partes;
    c.v_partes = (din, pix, vale, cred);
    Ok(c)
}

fn proximo(conn: &Connection, tabela: &str, dispositivo: &str) -> Resultado<i64> {
    Ok(conn.query_row(
        &format!("SELECT COALESCE(MAX(numero), 0) + 1 FROM {tabela} WHERE dispositivo = ?1"),
        [dispositivo],
        |l| l.get(0),
    )?)
}

/// Grava a devolução. Devolve o id.
pub fn devolver(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada) -> Resultado<String> {
    let c = calcular(conn, e)?;
    if !c.erro.is_empty() {
        return Err(if c.campo.is_empty() { Erro::validacao(c.erro) } else { Erro::campo(&c.campo, c.erro) });
    }
    if c.linhas.is_empty() {
        return Err(Erro::campo("itens", "Diga quantas peças o cliente está devolvendo."));
    }
    if e.destino.is_empty() {
        return Err(Erro::campo("destino", "Escolha como devolver o valor."));
    }
    let motivo = texto::limpar(&e.motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo (ex.: peça errada, não serviu, defeito)."));
    }
    let motivo: String = motivo.chars().take(200).collect();
    let (din, pix, vale, cred) = c.v_partes;
    let sessao = caixa::sessao_aberta(conn, dispositivo)?;
    if din > 0 {
        let Some(s) = &sessao else {
            return Err(Erro::campo("destino", "Abra o caixa para devolver em dinheiro (sai da gaveta)."));
        };
        let r = caixa::resumo(conn, &s.id)?;
        if din > r.esperado_dinheiro {
            return Err(Erro::campo(
                "destino",
                format!("Pelo sistema, só há R$ {} em dinheiro na gaveta.", dinheiro(r.esperado_dinheiro)),
            ));
        }
    }
    let (venda_numero, venda_caixa, cliente_id, cliente_nome): (i64, String, Option<String>, String) = conn.query_row(
        "SELECT numero, caixa_nome, cliente_id, cliente_nome FROM venda WHERE id = ?1",
        [&e.venda_id],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?;
    let caixa_nome = crate::config::nome_caixa(conn)?;
    let id = tempo::novo_id();
    let numero = proximo(conn, "devolucao", dispositivo)?;
    let agora = tempo::agora();

    // Vale-troca
    let vale_id = if vale > 0 {
        let vid = tempo::novo_id();
        conn.execute(
            "INSERT INTO vale (id, numero, dispositivo, caixa_nome, cliente_id, cliente_nome, valor, devolucao_id, usuario_id, criado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![vid, proximo(conn, "vale", dispositivo)?, dispositivo, caixa_nome, cliente_id, cliente_nome, vale, id, usuario_id, agora],
        )?;
        Some(vid)
    } else {
        None
    };

    // Abatimento no crediário: um "recebimento" sem dinheiro, que paga as
    // parcelas desta venda (da mais antiga para a mais nova).
    let recebimento_id = if cred > 0 {
        let cid = cliente_id.clone().ok_or_else(|| Erro::Interno("crediário sem cliente".into()))?;
        let rid = tempo::novo_id();
        let rnum: i64 = proximo(conn, "recebimento", dispositivo)?;
        conn.execute(
            "INSERT INTO recebimento (id, cliente_id, numero, dispositivo, caixa_nome, sessao_id, forma, valor, juros, desconto,
                                      troco, observacoes, usuario_id, criado_em)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'devolucao', 0, 0, 0, 0, ?6, ?7, ?8)",
            params![rid, cid, rnum, dispositivo, caixa_nome, format!("Devolução nº {numero} da venda {venda_numero}"), usuario_id, agora],
        )?;
        let mut parcelas: Vec<(String, i64)> = conn
            .prepare(
                "SELECT id, valor - pago FROM conta_receber_saldo
                  WHERE venda_id = ?1 AND cancelado_em IS NULL AND valor > pago ORDER BY vencimento, parcela",
            )?
            .query_map([&e.venda_id], |l| Ok((l.get(0)?, l.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut resta = cred;
        for (conta, saldo) in parcelas.drain(..) {
            if resta <= 0 {
                break;
            }
            let v = resta.min(saldo);
            resta -= v;
            conn.execute(
                "INSERT INTO recebimento_parcela (id, recebimento_id, conta_id, valor) VALUES (?1, ?2, ?3, ?4)",
                params![tempo::novo_id(), rid, conta, v],
            )?;
        }
        Some(rid)
    } else {
        None
    };

    conn.execute(
        "INSERT INTO devolucao (id, numero, dispositivo, caixa_nome, sessao_id, venda_id, cliente_id, cliente_nome, motivo,
                                valor, dinheiro, pix, vale, crediario, vale_id, recebimento_id, usuario_id, criado_em)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            id,
            numero,
            dispositivo,
            caixa_nome,
            if din > 0 { sessao.as_ref().map(|s| s.id.clone()) } else { None },
            e.venda_id,
            cliente_id,
            cliente_nome,
            motivo,
            c.v_valor,
            din,
            pix,
            vale,
            cred,
            vale_id,
            recebimento_id,
            usuario_id,
            agora
        ],
    )?;
    let referencia = format!("Devolução nº {numero} · venda {venda_numero} · {venda_caixa} · {motivo}");
    for l in &c.linhas {
        conn.execute(
            "INSERT INTO devolucao_item (id, devolucao_id, venda_item_id, produto_id, descricao, unidade, quantidade,
                                         valor_item, valor, custo, volta_estoque)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tempo::novo_id(),
                id,
                l.venda_item_id,
                if l.produto_id.is_empty() { None } else { Some(&l.produto_id) },
                l.descricao,
                l.unidade,
                l.quantidade,
                l.valor_item,
                l.valor,
                l.custo,
                !l.defeito
            ],
        )?;
        if !l.produto_id.is_empty() {
            if l.defeito {
                auditoria::registrar(
                    conn,
                    dispositivo,
                    Some(usuario_id),
                    "produto.defeito",
                    "produto",
                    &l.produto_id,
                    &[format!(
                        "Voltou com defeito ({} {}), não entrou no estoque · {referencia}",
                        numeros::escrever_quantidade(l.quantidade),
                        l.unidade
                    )],
                )?;
            } else {
                estoque::lancar(
                    conn,
                    dispositivo,
                    Some(usuario_id),
                    &l.produto_id,
                    l.quantidade,
                    "devolucao",
                    &referencia,
                )?;
            }
        }
    }
    let como: Vec<String> = c.partes.iter().map(|p| format!("{} R$ {}", p.nome, p.valor)).collect();
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "venda.devolucao",
        "venda",
        &e.venda_id,
        &[format!("Devolução nº {numero}: R$ {} · {} · {motivo}", c.valor, como.join(" + "))],
    )?;
    Ok(id)
}

/// Desfaz a devolução: as peças saem de novo do estoque, o vale é
/// cancelado e o abatimento no crediário é estornado.
pub fn desfazer(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    let d: (Option<String>, Option<String>, Option<String>, String, i64, i64) = conn
        .query_row(
            "SELECT cancelado_em, vale_id, recebimento_id, venda_id, numero, dinheiro FROM devolucao WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Devolução".into()))?;
    if d.0.is_some() {
        return Err(Erro::validacao("Esta devolução já foi desfeita."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga por que está desfazendo."));
    }
    let agora = tempo::agora();
    if let Some(vale) = &d.1 {
        let (valor, saldo): (i64, i64) =
            conn.query_row("SELECT valor, saldo FROM vale_saldo WHERE id = ?1", [vale], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })?;
        if saldo < valor {
            return Err(Erro::validacao(
                "O vale-troca desta devolução já foi usado numa compra. Cancele aquela venda antes.",
            ));
        }
        conn.execute(
            "UPDATE vale SET cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4 WHERE id = ?1",
            params![vale, agora, usuario_id, motivo],
        )?;
    }
    if let Some(r) = &d.2 {
        conn.execute(
            "UPDATE recebimento SET estornado_em = ?2, estornado_por = ?3, motivo_estorno = ?4 WHERE id = ?1 AND estornado_em IS NULL",
            params![r, agora, usuario_id, format!("Devolução desfeita · {motivo}")],
        )?;
    }
    let (venda_numero, venda_caixa): (i64, String) =
        conn.query_row("SELECT numero, caixa_nome FROM venda WHERE id = ?1", [&d.3], |l| Ok((l.get(0)?, l.get(1)?)))?;
    let itens: Vec<(String, i64)> = conn
        .prepare("SELECT produto_id, quantidade FROM devolucao_item WHERE devolucao_id = ?1 AND produto_id IS NOT NULL AND volta_estoque = 1")?
        .query_map([id], |l| Ok((l.get(0)?, l.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (pid, q) in itens {
        estoque::lancar(
            conn,
            dispositivo,
            Some(usuario_id),
            &pid,
            -q,
            "devolucao_desfeita",
            &format!("Devolução nº {} desfeita · venda {venda_numero} · {venda_caixa} · {motivo}", d.4),
        )?;
    }
    conn.execute(
        "UPDATE devolucao SET cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4 WHERE id = ?1",
        params![id, agora, usuario_id, motivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "venda.devolucao_desfeita",
        "venda",
        &d.3,
        &[format!(
            "Devolução nº {} desfeita · {motivo}{}",
            d.4,
            if d.5 > 0 { " (o dinheiro volta a contar na gaveta)" } else { "" }
        )],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Listas
// ---------------------------------------------------------------------------

fn listar_onde(conn: &Connection, filtro: &str, valor: &str, limite: i64) -> Resultado<Vec<DevolucaoResumo>> {
    let sql = format!(
        "SELECT d.id, d.numero, d.caixa_nome, d.venda_id, v.numero, v.caixa_nome, d.cliente_nome, d.valor,
                d.dinheiro, d.pix, d.vale, d.crediario, COALESCE(d.vale_id, ''), d.motivo, COALESCE(u.nome, ''), d.criado_em,
                d.cancelado_em IS NOT NULL,
                (SELECT group_concat(trim(replace(printf('%.3f', di.quantidade / 1000.0), '.000', '')) || ' ' || di.descricao, ', ')
                   FROM devolucao_item di WHERE di.devolucao_id = d.id),
                COALESCE((SELECT numero FROM vale WHERE id = d.vale_id), 0),
                COALESCE((SELECT saldo < valor FROM vale_saldo WHERE id = d.vale_id), 0)
           FROM devolucao d JOIN venda v ON v.id = d.venda_id LEFT JOIN usuario u ON u.id = d.usuario_id
          WHERE {filtro}
          ORDER BY d.criado_em DESC LIMIT {limite}"
    );
    let mut c = conn.prepare(&sql)?;
    let linhas: Vec<_> = c
        .query_map([valor], |l| {
            Ok((
                (
                    l.get::<_, String>(0)?,
                    l.get::<_, i64>(1)?,
                    l.get::<_, String>(2)?,
                    l.get::<_, String>(3)?,
                    l.get::<_, i64>(4)?,
                    l.get::<_, String>(5)?,
                ),
                (
                    l.get::<_, String>(6)?,
                    l.get::<_, i64>(7)?,
                    l.get::<_, i64>(8)?,
                    l.get::<_, i64>(9)?,
                    l.get::<_, i64>(10)?,
                    l.get::<_, i64>(11)?,
                ),
                (
                    l.get::<_, String>(12)?,
                    l.get::<_, String>(13)?,
                    l.get::<_, String>(14)?,
                    l.get::<_, String>(15)?,
                    l.get::<_, bool>(16)?,
                ),
                (l.get::<_, Option<String>>(17)?, l.get::<_, i64>(18)?, l.get::<_, bool>(19)?),
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for (a, b, cc, dd) in linhas {
        let mut como = Vec::new();
        if b.5 > 0 {
            como.push("abateu do crediário".to_string());
        }
        if b.4 > 0 {
            como.push(format!("vale-troca nº {}", dd.1));
        }
        if b.2 > 0 {
            como.push("dinheiro".to_string());
        }
        if b.3 > 0 {
            como.push("Pix ou cartão".to_string());
        }
        lista.push(DevolucaoResumo {
            id: a.0,
            numero: a.1,
            caixa: a.2,
            venda_id: a.3,
            venda_numero: a.4,
            venda_caixa: a.5,
            cliente: b.0,
            valor: dinheiro(b.1),
            como: como.join(" + "),
            vale_id: cc.0,
            motivo: cc.1,
            usuario: cc.2,
            quando: quando(conn, &cc.3)?,
            criado_em: cc.3,
            cancelada: cc.4,
            itens: dd.0.unwrap_or_default(),
            vale_usado: dd.2,
        });
    }
    Ok(lista)
}

/// Devoluções mais recentes (a busca procura no cliente, no número da venda
/// ou na peça).
pub fn listar(conn: &Connection, busca: &str) -> Resultado<Vec<DevolucaoResumo>> {
    let t = texto::normalizar(busca);
    if t.is_empty() {
        return listar_onde(conn, "?1 = ''", "", 200);
    }
    listar_onde(
        conn,
        "(normalizar(d.cliente_nome) LIKE '%' || ?1 || '%' OR CAST(v.numero AS TEXT) = ?1
          OR EXISTS (SELECT 1 FROM devolucao_item di WHERE di.devolucao_id = d.id AND normalizar(di.descricao) LIKE '%' || ?1 || '%'))",
        &t,
        200,
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vale {
    pub id: String,
    pub numero: i64,
    pub caixa: String,
    pub cliente_id: String,
    pub cliente: String,
    pub valor: String,
    pub saldo: String,
    pub quando: String,
    pub devolucao_numero: i64,
    pub venda_numero: i64,
    /// "nº 3 · Maria · R$ 50,00" (para a lista de escolher no Receber).
    pub rotulo: String,
    #[serde(skip)]
    pub v_saldo: i64,
}

/// Vales com saldo. Os do cliente escolhido vêm primeiro.
pub fn vales_abertos(conn: &Connection, cliente_id: &str) -> Resultado<Vec<Vale>> {
    let mut c = conn.prepare(
        "SELECT s.id, s.numero, s.caixa_nome, COALESCE(s.cliente_id, ''), s.cliente_nome, s.valor, s.saldo, s.criado_em,
                COALESCE(d.numero, 0), COALESCE(v.numero, 0)
           FROM vale_saldo s LEFT JOIN devolucao d ON d.id = s.devolucao_id LEFT JOIN venda v ON v.id = d.venda_id
          WHERE s.cancelado_em IS NULL AND s.saldo > 0
          ORDER BY (COALESCE(s.cliente_id, '') = ?1 AND ?1 <> '') DESC, s.criado_em DESC",
    )?;
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(String, i64, String, String, String, i64, i64, String, i64, i64)> = c
        .query_map([cliente_id], |l| {
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
        })?
        .collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for l in linhas {
        lista.push(Vale {
            rotulo: format!(
                "nº {}{} · R$ {}",
                l.1,
                if l.4.is_empty() { String::new() } else { format!(" · {}", l.4) },
                dinheiro(l.6)
            ),
            id: l.0,
            numero: l.1,
            caixa: l.2,
            cliente_id: l.3,
            cliente: l.4,
            valor: dinheiro(l.5),
            saldo: dinheiro(l.6),
            quando: quando(conn, &l.7)?,
            devolucao_numero: l.8,
            venda_numero: l.9,
            v_saldo: l.6,
        });
    }
    Ok(lista)
}

/// Saldo de um vale (0 se cancelado ou não existir).
pub fn saldo_vale(conn: &Connection, vale_id: &str) -> Resultado<i64> {
    Ok(conn
        .query_row("SELECT saldo FROM vale_saldo WHERE id = ?1 AND cancelado_em IS NULL", [vale_id], |l| l.get(0))
        .optional()?
        .unwrap_or(0))
}

/// Devoluções (em valor) entre duas datas locais: (valor, custo das peças
/// que voltaram) — para tirar do faturamento e do lucro.
pub fn no_periodo(conn: &Connection, de: &str, ate: &str) -> Resultado<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(di.valor), 0), COALESCE(SUM((di.custo * di.quantidade + 500) / 1000), 0)
           FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id
          WHERE d.cancelado_em IS NULL AND date(d.criado_em, 'localtime') BETWEEN ?1 AND ?2",
        params![de, ate],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?)
}

/// Dinheiro devolvido (saiu da gaveta) numa sessão de caixa.
pub fn dinheiro_na_sessao(conn: &Connection, sessao_id: &str) -> Resultado<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(dinheiro), 0) FROM devolucao WHERE sessao_id = ?1 AND cancelado_em IS NULL",
        [sessao_id],
        |l| l.get(0),
    )?)
}

/// A venda tem devolução que vale (não desfeita)?
pub fn tem_devolucao(conn: &Connection, venda_id: &str) -> Resultado<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM devolucao WHERE venda_id = ?1 AND cancelado_em IS NULL LIMIT 1", [venda_id], |_| {
            Ok(())
        })
        .optional()?
        .is_some())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::vendas::{self, PagamentoEntrada, VendaEntrada};
    use crate::{caixa, painel, pessoas, produtos, usuarios};

    fn peca(b: &Banco, u: &str, nome: &str, custo: &str) -> String {
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = nome.into();
        f.custo = custo.into();
        f.estoque_inicial = "10".into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
    }

    fn item(p: &str, q: &str) -> vendas::ItemEntrada {
        vendas::ItemEntrada { produto_id: p.into(), quantidade: q.into(), ..Default::default() }
    }

    fn pg(forma: &str, valor: &str) -> PagamentoEntrada {
        PagamentoEntrada { forma: forma.into(), valor: valor.into(), ..Default::default() }
    }

    fn entrada(venda: &str, itens: Vec<(&str, &str, bool)>, destino: &str, resto: &str) -> Entrada {
        Entrada {
            venda_id: venda.into(),
            itens: itens
                .into_iter()
                .map(|(id, q, defeito)| ItemEntrada { venda_item_id: id.into(), quantidade: q.into(), defeito })
                .collect(),
            destino: destino.into(),
            resto: resto.into(),
            motivo: "Não serviu".into(),
        }
    }

    #[test]
    fn devolve_com_vale_dinheiro_e_crediario() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        caixa::abrir(c, d, &u, "Caixa 1", "100").unwrap();
        let filtro = peca(&b, &u, "Filtro", "10"); // preço 20
        let vela = peca(&b, &u, "Vela", "5"); // preço 10
                                              // 2 filtros + 4 velas = 80, com 10% de desconto na venda = 72, em dinheiro.
        let e = VendaEntrada {
            itens: vec![item(&filtro, "2"), item(&vela, "4")],
            tabela: 1,
            desconto: "10%".into(),
            pagamentos: vec![pg("dinheiro", "72")],
            ..Default::default()
        };
        let venda = vendas::concluir(c, d, &u, &e).unwrap();
        let p = da_venda(c, &venda).unwrap();
        assert_eq!(p.itens.len(), 2);
        assert_eq!(p.itens[0].valor_unitario, "18,00"); // 20 com 10% de desconto
        let (i_filtro, i_vela) = (p.itens[0].venda_item_id.clone(), p.itens[1].venda_item_id.clone());

        // Mais do que vendeu: não deixa.
        let x = calcular(c, &entrada(&venda, vec![(&i_filtro, "3", false)], "vale", "")).unwrap();
        assert!(x.erro.contains("no máximo 2"), "{}", x.erro);

        // 1 filtro volta em vale-troca (18,00); 1 vela com defeito em dinheiro (9,00).
        let id = devolver(c, d, &u, &entrada(&venda, vec![(&i_filtro, "1", false)], "vale", "")).unwrap();
        devolver(c, d, &u, &entrada(&venda, vec![(&i_vela, "1", true)], "dinheiro", "")).unwrap();
        assert_eq!(estoque::saldo(c, &filtro).unwrap(), 9_000); // 10 - 2 + 1
        assert_eq!(estoque::saldo(c, &vela).unwrap(), 6_000); // a com defeito não volta
        let s = caixa::sessao_aberta(c, d).unwrap().unwrap();
        let r = caixa::resumo(c, &s.id).unwrap();
        assert_eq!((r.devolvido.as_str(), r.esperado.as_str()), ("9,00", "163,00")); // 100 + 72 - 9

        // O vale aparece e paga parte de uma compra nova.
        let vales = vales_abertos(c, "").unwrap();
        assert_eq!((vales.len(), vales[0].saldo.as_str()), (1, "18,00"));
        let vale = vales[0].id.clone();
        let mais = |valor: &str| VendaEntrada {
            itens: vec![item(&vela, "1")],
            tabela: 1,
            pagamentos: vec![PagamentoEntrada { forma: "vale".into(), valor: valor.into(), vale_id: vale.clone() }],
            ..Default::default()
        };
        assert!(vendas::concluir(c, d, &u, &mais("12")).is_err()); // pagou a mais com vale
        let nova = vendas::concluir(c, d, &u, &mais("10")).unwrap();
        assert_eq!(saldo_vale(c, &vale).unwrap(), 800);
        // Vale usado: não dá para desfazer a devolução...
        assert!(desfazer(c, d, &u, &id, "engano").is_err());
        // ... até cancelar a venda que usou o vale.
        vendas::cancelar(c, d, &u, &nova, "teste").unwrap();
        assert_eq!(saldo_vale(c, &vale).unwrap(), 1_800);
        // Venda com devolução não se cancela.
        assert!(vendas::cancelar(c, d, &u, &venda, "teste").is_err());
        desfazer(c, d, &u, &id, "engano").unwrap();
        assert_eq!(estoque::saldo(c, &filtro).unwrap(), 8_000);
        assert_eq!(saldo_vale(c, &vale).unwrap(), 0);

        // No painel, a venda conta sem o que foi devolvido (72 - 9).
        let p = painel::resumo(c).unwrap();
        assert_eq!(p.hoje.total, "63,00");

        // Resto da venda: o último filtro e as 3 velas (com o arredondamento certo).
        let x = calcular(c, &entrada(&venda, vec![(&i_filtro, "2", false), (&i_vela, "3", false)], "pix", "")).unwrap();
        assert_eq!(x.valor, "63,00");
        assert_eq!(x.partes[0].nome, "Pix ou cartão");
    }

    #[test]
    fn abate_do_crediario_e_o_resto_vira_vale() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        caixa::abrir(c, d, &u, "Caixa 1", "0").unwrap();
        let mut f = pessoas::ficha_nova(c, "cliente").unwrap();
        f.nome = "Maria".into();
        let cliente = pessoas::salvar(c, d, &u, &f).unwrap();
        let filtro = peca(&b, &u, "Filtro", "10"); // 20
                                                   // 3 filtros = 60: 20 de entrada e 40 no crediário.
        let e = VendaEntrada {
            itens: vec![item(&filtro, "3")],
            tabela: 1,
            cliente_id: cliente.clone(),
            pagamentos: vec![pg("dinheiro", "20"), pg("crediario", "40")],
            ..Default::default()
        };
        let venda = vendas::concluir(c, d, &u, &e).unwrap();
        let i = da_venda(c, &venda).unwrap().itens[0].venda_item_id.clone();
        assert_eq!(da_venda(c, &venda).unwrap().crediario_aberto, "40,00");
        // Devolve os 3 (60): 40 abatem o crediário e os outros 20 precisam de destino.
        let x = calcular(c, &entrada(&venda, vec![(&i, "3", false)], "crediario", "")).unwrap();
        assert!(x.precisa_resto && !x.erro.is_empty());
        devolver(c, d, &u, &entrada(&venda, vec![(&i, "3", false)], "crediario", "vale")).unwrap();
        assert_eq!(da_venda(c, &venda).unwrap().crediario_aberto, "0,00");
        let vales = vales_abertos(c, &cliente).unwrap();
        assert_eq!((vales[0].saldo.as_str(), vales[0].cliente.as_str()), ("20,00", "Maria"));
        let l = listar(c, "maria").unwrap();
        assert_eq!(l.len(), 1);
        assert!(l[0].como.contains("crediário") && l[0].como.contains("vale-troca"));
        assert!(crate::impressao::devolucao(c, &l[0].id).unwrap().contains("Vale-troca nº 1"));
    }
}
