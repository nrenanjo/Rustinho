//! Vendas e orçamentos.
//!
//! O carrinho fica na tela. A cada mudança a tela pede `calcular` (preços,
//! descontos, estoque, total e troco); ao receber, `concluir` grava tudo de
//! uma vez: venda, itens, pagamentos e a saída do estoque.
//!
//! Um orçamento é a mesma coisa sem pagamento e sem mexer no estoque; depois
//! ele pode virar venda.

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::caixa::{self, FORMAS};
use crate::erro::{Erro, Resultado};
use crate::{auditoria, cheques, config, documento, encomendas, estoque, numeros, receber, tempo, texto};

const MAX_ITENS: usize = 500;

// ---------------------------------------------------------------------------
// O que vem da tela.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemEntrada {
    /// Vazio = serviço (mão de obra), com a descrição e o preço digitados.
    pub produto_id: String,
    /// Serviço: o que foi feito (ex.: "Troca de pastilhas").
    pub descricao: String,
    pub quantidade: String,
    /// Preço de cada unidade; vazio = preço da tabela.
    pub preco: String,
    /// Desconto do item: "5,00" (reais) ou "10%".
    pub desconto: String,
    /// Falta no estoque e o cliente quer encomendar.
    pub encomendar: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PagamentoEntrada {
    pub forma: String,
    pub valor: String,
    /// Vale-troca usado (forma "vale").
    pub vale_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VendaEntrada {
    pub itens: Vec<ItemEntrada>,
    pub tabela: i64,
    pub cliente_id: String,
    /// Nome digitado sem cadastro (ex.: "João mecânico").
    pub cliente_nome: String,
    /// Desconto no total: "5,00" ou "10%".
    pub desconto: String,
    pub pagamentos: Vec<PagamentoEntrada>,
    pub observacoes: String,
    /// Orçamento que está virando venda.
    pub orcamento_id: String,
    /// Orçamento: validade em dias.
    pub validade_dias: i64,
    /// Crediário: em quantas parcelas (0 ou 1 = uma só).
    pub parcelas: i64,
    /// Crediário: 1º vencimento "dd/mm/aaaa" (vazio = automático).
    pub primeiro_vencimento: String,
    /// Cheque: quantos, bom para, banco e número.
    pub cheques: cheques::ChequesEntrada,
    /// Encomenda que está sendo entregue (e paga) nesta venda.
    pub encomenda_id: String,
    /// Ordem de serviço que está sendo entregue (e paga) nesta venda.
    pub os_id: String,
}

// ---------------------------------------------------------------------------
// O que volta para a tela.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCalculado {
    pub produto_id: String,
    pub codigo: String,
    pub descricao: String,
    pub marca: String,
    pub unidade: String,
    pub quantidade: String,
    pub preco: String,
    pub preco_tabela: String,
    pub desconto: String,
    pub total: String,
    pub estoque: String,
    /// Vende mais do que o sistema diz que tem.
    pub falta_estoque: bool,
    /// Quanto falta no estoque (o que seria encomendado).
    pub falta: String,
    pub encomendar: bool,
    pub local: String,
    /// Serviço (mão de obra): não tem estoque.
    pub servico: bool,
    /// A unidade vende quebrado (KG, LT, MT...).
    pub fracionada: bool,
    /// Problema neste item (desconto acima do máximo, quantidade inválida...).
    pub erro: String,
    #[serde(skip)]
    pub v_quantidade: i64,
    #[serde(skip)]
    pub v_preco: i64,
    #[serde(skip)]
    pub v_desconto: i64,
    #[serde(skip)]
    pub v_total: i64,
    #[serde(skip)]
    pub v_custo: i64,
    #[serde(skip)]
    pub v_comissao: Option<i64>,
    #[serde(skip)]
    pub v_falta: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calculo {
    pub itens: Vec<ItemCalculado>,
    pub quantidade_itens: String,
    pub subtotal: String,
    pub desconto_itens: String,
    pub desconto: String,
    pub total: String,
    pub pago: String,
    pub falta: String,
    pub troco: String,
    pub erro_desconto: String,
    pub cliente: String,
    /// Itens marcados para encomendar.
    pub encomendas: i64,
    pub erro_encomenda: String,
    pub crediario: Crediario,
    pub cheques: ChequesCalculo,
    #[serde(skip)]
    pub v_subtotal: i64,
    #[serde(skip)]
    pub v_desconto: i64,
    #[serde(skip)]
    pub v_total: i64,
    #[serde(skip)]
    pub v_pago: i64,
    #[serde(skip)]
    pub v_troco: i64,
}

/// O que vai para o crediário nesta venda.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crediario {
    pub valor: String,
    pub parcelas: Vec<receber::ParcelaPrevia>,
    pub avisos: Vec<String>,
    pub erro: String,
    pub campo: String,
    #[serde(skip)]
    pub v_valor: i64,
}

/// O que vai em cheque nesta venda (ou recebimento).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChequesCalculo {
    pub valor: String,
    pub cheques: Vec<cheques::ChequePrevia>,
    pub erro: String,
    pub campo: String,
    #[serde(skip)]
    pub v_valor: i64,
}

pub fn calcular_cheques(conn: &Connection, valor: i64, e: &cheques::ChequesEntrada) -> Resultado<ChequesCalculo> {
    let mut c = ChequesCalculo { valor: numeros::escrever_dinheiro(valor), v_valor: valor, ..Default::default() };
    if valor == 0 {
        return Ok(c);
    }
    match cheques::previa(conn, valor, e).and_then(|l| cheques::conferir(e).map(|_| l)) {
        Ok(l) => c.cheques = l,
        Err(Erro::Validacao { campo, mensagem }) => {
            if let Ok(l) = cheques::previa(conn, valor, e) {
                c.cheques = l;
            }
            c.erro = mensagem;
            c.campo = campo.unwrap_or_default();
        }
        Err(outro) => return Err(outro),
    }
    Ok(c)
}

fn calcular_crediario(conn: &Connection, e: &VendaEntrada) -> Resultado<Crediario> {
    let valor = numeros::somar(
        e.pagamentos
            .iter()
            .filter(|p| p.forma == "crediario")
            .map(|p| numeros::ler_dinheiro(&p.valor).ok().flatten().unwrap_or(0).clamp(0, TOTAL_MAXIMO)),
    )
    .ok_or_else(|| Erro::validacao("O crediário ficou grande demais."))?;
    let mut c = Crediario { valor: numeros::escrever_dinheiro(valor), v_valor: valor, ..Default::default() };
    if valor == 0 {
        return Ok(c);
    }
    if e.cliente_id.is_empty() {
        c.erro = "Crediário só para cliente cadastrado. Escolha o cliente.".into();
        c.campo = "cliente".into();
        return Ok(c);
    }
    match receber::previa(conn, &e.cliente_id, valor, e.parcelas, &e.primeiro_vencimento) {
        Ok(p) => c.parcelas = p,
        Err(Erro::Validacao { campo, mensagem }) => {
            c.erro = mensagem;
            c.campo = campo.unwrap_or_default();
        }
        Err(outro) => return Err(outro),
    }
    c.avisos = receber::avisos_venda(conn, &e.cliente_id, valor)?;
    Ok(c)
}

/// "10%" → percentual sobre `base`; "5,00" → reais. Devolve centavos.
fn ler_desconto(texto: &str, base: i64) -> Result<i64, String> {
    let t = texto.trim();
    if t.is_empty() {
        return Ok(0);
    }
    if let Some(p) = t.strip_suffix('%') {
        let pct = numeros::ler_percentual(p)?.unwrap_or(0);
        if !(0..=10_000).contains(&pct) {
            return Err("Use um desconto entre 0 e 100%.".into());
        }
        return Ok(numeros::saturar((base as i128 * pct as i128 + 5_000) / 10_000));
    }
    let v = numeros::ler_dinheiro(t)?.unwrap_or(0);
    if v < 0 {
        return Err("O desconto não pode ser negativo.".into());
    }
    Ok(v)
}

/// Limites de uma venda de balcão (bem acima de qualquer venda real; evitam
/// contas que passariam do tamanho dos números).
const QUANTIDADE_MAXIMA: i64 = 1_000_000_000; // 1 milhão de unidades (em milésimos)
const PRECO_MAXIMO: i64 = 100_000_000_000; // R$ 1 bilhão (em centavos)
const TOTAL_MAXIMO: i64 = 10_000_000_000_000; // R$ 100 bilhões
/// Formas de pagamento numa venda só.
const PAGAMENTOS_MAXIMOS: usize = 50;

/// quantidade (milésimos) × preço (centavos) → centavos.
fn multiplicar(quantidade: i64, preco: i64) -> i64 {
    numeros::saturar((quantidade as i128 * preco as i128 + 500) / 1000)
}

struct Peca {
    codigo: String,
    nome: String,
    marca: String,
    unidade: String,
    custo: i64,
    local: String,
    desconto_maximo: Option<i64>,
    comissao: Option<i64>,
    preco: Option<i64>,
    estoque: i64,
    servico: bool,
    preco_aberto: bool,
    fracionada: bool,
}

fn peca(conn: &Connection, id: &str, tabela: i64) -> Resultado<Option<Peca>> {
    let sql = format!(
        "SELECT codigo, nome, marca, unidade, custo, local, desconto_maximo, comissao,
                CASE WHEN ?2 = 1 THEN {p1}
                     ELSE COALESCE((SELECT preco FROM produto_preco WHERE produto_id = p.id AND tabela = ?2), {p1}) END,
                tipo = 'servico', preco_aberto,
                COALESCE((SELECT fracionada FROM unidade WHERE sigla = p.unidade), 0)
           FROM produto p WHERE id = ?1",
        p1 = crate::produtos::SQL_PRECO_1
    );
    let p = conn
        .query_row(&sql, params![id, tabela], |l| {
            Ok(Peca {
                codigo: l.get(0)?,
                nome: l.get(1)?,
                marca: l.get(2)?,
                unidade: l.get(3)?,
                custo: l.get(4)?,
                local: l.get(5)?,
                desconto_maximo: l.get(6)?,
                comissao: l.get(7)?,
                preco: l.get(8)?,
                estoque: 0,
                servico: l.get(9)?,
                preco_aberto: l.get(10)?,
                fracionada: l.get(11)?,
            })
        })
        .optional()?;
    Ok(match p {
        Some(mut p) => {
            p.estoque = if p.servico { i64::MAX / 4 } else { estoque::saldo(conn, id)? };
            Some(p)
        }
        None => None,
    })
}

/// Calcula o carrinho. Não grava nada.
pub fn calcular(conn: &Connection, e: &VendaEntrada) -> Resultado<Calculo> {
    if e.itens.len() > MAX_ITENS {
        return Err(Erro::validacao(format!("Use no máximo {MAX_ITENS} itens por venda.")));
    }
    let tabela = if (1..=5).contains(&e.tabela) { e.tabela } else { 1 };
    let mut itens = Vec::new();
    // Quanto de cada peça já está no carrinho (a mesma peça pode aparecer 2 vezes).
    let mut usado: Vec<(String, i64)> = Vec::new();
    for it in &e.itens {
        let avulso = it.produto_id.is_empty();
        let p = if avulso {
            // Serviço: sem estoque nem custo; o preço é o digitado.
            Peca {
                codigo: String::new(),
                nome: texto::limpar(&it.descricao).chars().take(120).collect(),
                marca: String::new(),
                unidade: "SV".into(),
                custo: 0,
                local: String::new(),
                desconto_maximo: None,
                comissao: None,
                preco: None,
                estoque: i64::MAX / 4,
                servico: true,
                preco_aberto: true,
                fracionada: true,
            }
        } else {
            peca(conn, &it.produto_id, tabela)?.ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?
        };
        let servico = p.servico;
        let mut erro = String::new();
        if avulso && p.nome.is_empty() {
            erro = "Diga qual foi o serviço.".into();
        }
        let quantidade = match numeros::ler_quantidade(&it.quantidade) {
            Ok(Some(q)) if q > QUANTIDADE_MAXIMA => {
                erro = "Quantidade grande demais.".into();
                0
            }
            Ok(Some(q)) if q > 0 => q,
            Ok(_) => {
                erro = "Quantidade precisa ser maior que zero.".into();
                0
            }
            Err(m) => {
                erro = m;
                0
            }
        };
        if quantidade % 1000 != 0 && !p.fracionada && erro.is_empty() {
            erro = format!("{} é vendido inteiro (sem vírgula).", p.unidade);
        }
        let preco_tabela = if p.preco_aberto { 0 } else { p.preco.unwrap_or(0) };
        let preco = match numeros::ler_dinheiro(&it.preco) {
            Ok(Some(v)) if v > PRECO_MAXIMO => {
                erro = "Preço grande demais.".into();
                0
            }
            Ok(Some(v)) if v > 0 => v,
            Ok(Some(_)) => {
                erro = "O preço precisa ser maior que zero.".into();
                preco_tabela
            }
            Ok(None) => preco_tabela,
            Err(m) => {
                erro = m;
                preco_tabela
            }
        };
        if preco <= 0 && erro.is_empty() {
            erro = if servico {
                "Digite o valor do serviço.".into()
            } else if p.preco_aberto {
                "Digite o preço desta peça.".into()
            } else {
                "Esta peça não tem preço. Digite o preço.".into()
            };
        }
        let bruto = multiplicar(quantidade, preco);
        // Item que sozinho passa do total de uma venda: fica com erro e não conta.
        let (quantidade, preco, bruto) = if bruto > TOTAL_MAXIMO {
            if erro.is_empty() {
                erro = "O valor deste item ficou grande demais. Confira a quantidade e o preço.".into();
            }
            (0, 0, 0)
        } else {
            (quantidade, preco, bruto)
        };
        let desconto = match ler_desconto(&it.desconto, bruto) {
            Ok(d) if d > bruto => {
                erro = "O desconto é maior que o valor do item.".into();
                0
            }
            Ok(d) => d,
            Err(m) => {
                erro = m;
                0
            }
        };
        // Desconto máximo da peça: conta também o preço digitado abaixo da tabela.
        if let Some(max) = p.desconto_maximo {
            let cheio = multiplicar(quantidade, preco_tabela);
            let dado = cheio - (bruto - desconto);
            if cheio > 0 && dado > 0 && dado as i128 * 10_000 > cheio as i128 * max as i128 && erro.is_empty() {
                erro = format!("Desconto acima do máximo desta peça ({}%).", numeros::escrever_percentual(max));
            }
        }
        let total = bruto - desconto;
        let ja = if servico { 0 } else { usado.iter().find(|u| u.0 == it.produto_id).map(|u| u.1).unwrap_or(0) };
        let pedido = ja + quantidade;
        if !servico {
            match usado.iter_mut().find(|u| u.0 == it.produto_id) {
                Some(u) => u.1 = pedido,
                None => usado.push((it.produto_id.clone(), pedido)),
            }
        }
        let falta = if !servico && pedido > p.estoque { quantidade.min(pedido - p.estoque.max(0)) } else { 0 };
        itens.push(ItemCalculado {
            produto_id: it.produto_id.clone(),
            codigo: p.codigo,
            descricao: p.nome,
            marca: p.marca,
            unidade: p.unidade,
            quantidade: numeros::escrever_quantidade(quantidade),
            preco: numeros::escrever_dinheiro(preco),
            preco_tabela: numeros::escrever_dinheiro(preco_tabela),
            desconto: if desconto > 0 { numeros::escrever_dinheiro(desconto) } else { String::new() },
            total: numeros::escrever_dinheiro(total),
            estoque: if servico { String::new() } else { numeros::escrever_quantidade(p.estoque) },
            falta_estoque: !servico && pedido > p.estoque,
            falta: numeros::escrever_quantidade(falta),
            encomendar: !servico && it.encomendar && falta > 0,
            local: p.local,
            servico,
            fracionada: p.fracionada,
            erro,
            v_quantidade: quantidade,
            v_preco: preco,
            v_desconto: desconto,
            v_total: total,
            v_custo: p.custo,
            v_comissao: p.comissao,
            v_falta: falta,
        });
    }
    if e.pagamentos.len() > PAGAMENTOS_MAXIMOS {
        return Err(Erro::validacao(format!("Use no máximo {PAGAMENTOS_MAXIMOS} formas de pagamento.")));
    }
    // As somas são feitas num número maior (i128) e conferidas antes de
    // voltar: nenhuma conta "dá a volta" nem vira pânico.
    let grande = || Erro::validacao("O total da venda ficou grande demais. Confira as quantidades e os preços.");
    let subtotal = numeros::somar(itens.iter().map(|i| multiplicar(i.v_quantidade, i.v_preco))).ok_or_else(grande)?;
    let desconto_itens = numeros::somar(itens.iter().map(|i| i.v_desconto)).ok_or_else(grande)?;
    let depois_itens = subtotal - desconto_itens;
    let (desconto, erro_desconto) = match ler_desconto(&e.desconto, depois_itens) {
        Ok(d) if d > depois_itens => (0, "O desconto é maior que o total.".to_string()),
        Ok(d) => (d, String::new()),
        Err(m) => (0, m),
    };
    let total = depois_itens - desconto;
    let pago = numeros::somar(
        e.pagamentos.iter().map(|p| numeros::ler_dinheiro(&p.valor).ok().flatten().unwrap_or(0).clamp(0, TOTAL_MAXIMO)),
    )
    .ok_or_else(|| Erro::validacao("Os pagamentos somam mais do que uma venda pode ter."))?;
    let troco = (pago - total).max(0);
    let quantidade_itens = numeros::soma(itens.iter().filter(|i| !i.servico).map(|i| i.v_quantidade))?;
    let em_cheque = numeros::soma(
        e.pagamentos
            .iter()
            .filter(|p| p.forma == "cheque")
            .map(|p| numeros::ler_dinheiro(&p.valor).ok().flatten().unwrap_or(0).clamp(0, TOTAL_MAXIMO)),
    )?;
    let v_desconto = numeros::soma([desconto, desconto_itens])?;
    let cliente = if !e.cliente_id.is_empty() {
        conn.query_row("SELECT nome FROM pessoa WHERE id = ?1", [&e.cliente_id], |l| l.get(0))
            .optional()?
            .unwrap_or_default()
    } else {
        texto::limpar(&e.cliente_nome)
    };
    let encomendas = itens.iter().filter(|i| i.encomendar).count() as i64;
    let erro_encomenda = if encomendas > 0 && e.cliente_id.is_empty() {
        "Para encomendar, escolha o cliente (cadastrado), para avisar quando a peça chegar.".to_string()
    } else {
        String::new()
    };
    Ok(Calculo {
        encomendas,
        erro_encomenda,
        itens,
        quantidade_itens: numeros::escrever_quantidade(quantidade_itens),
        subtotal: numeros::escrever_dinheiro(subtotal),
        desconto_itens: numeros::escrever_dinheiro(desconto_itens),
        desconto: numeros::escrever_dinheiro(desconto),
        total: numeros::escrever_dinheiro(total),
        pago: numeros::escrever_dinheiro(pago),
        falta: numeros::escrever_dinheiro((total - pago).max(0)),
        troco: numeros::escrever_dinheiro(troco),
        erro_desconto,
        cliente,
        crediario: calcular_crediario(conn, e)?,
        cheques: calcular_cheques(conn, em_cheque, &e.cheques)?,
        v_subtotal: subtotal,
        v_desconto,
        v_total: total,
        v_pago: pago,
        v_troco: troco,
    })
}

fn proximo_numero(conn: &Connection, dispositivo: &str, tipo: &str) -> Resultado<i64> {
    let n: i64 = conn.query_row(
        "SELECT COALESCE(MAX(numero), 0) + 1 FROM venda WHERE dispositivo = ?1 AND tipo = ?2",
        params![dispositivo, tipo],
        |l| l.get(0),
    )?;
    Ok(n)
}

fn conferir_itens(c: &Calculo) -> Resultado<()> {
    if c.itens.is_empty() {
        return Err(Erro::validacao("Passe ou busque pelo menos uma peça."));
    }
    if let Some((i, it)) = c.itens.iter().enumerate().find(|(_, it)| !it.erro.is_empty()) {
        return Err(Erro::campo(&format!("itens.{i}"), format!("{}: {}", it.descricao, it.erro)));
    }
    if !c.erro_desconto.is_empty() {
        return Err(Erro::campo("desconto", c.erro_desconto.clone()));
    }
    Ok(())
}

/// Grava a venda (e o que foi pago) e tira as peças do estoque.
pub fn concluir(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &VendaEntrada) -> Resultado<String> {
    let sessao =
        caixa::sessao_aberta(conn, dispositivo)?.ok_or_else(|| Erro::validacao("Abra o caixa antes de vender."))?;
    let c = calcular(conn, e)?;
    conferir_itens(&c)?;
    // Pagamentos
    let mut pagamentos: Vec<(String, i64, Option<String>)> = Vec::new();
    for (i, p) in e.pagamentos.iter().enumerate() {
        let valor =
            numeros::ler_dinheiro(&p.valor).map_err(|m| Erro::campo(&format!("pagamentos.{i}"), m))?.unwrap_or(0);
        if valor == 0 {
            continue;
        }
        if valor < 0 {
            return Err(Erro::campo(&format!("pagamentos.{i}"), "O valor não pode ser negativo."));
        }
        match FORMAS.iter().find(|f| f.0 == p.forma) {
            None => return Err(Erro::Interno(format!("forma de pagamento desconhecida: {}", p.forma))),
            Some((_, nome, false)) => return Err(Erro::validacao(format!("{nome} chega na próxima etapa."))),
            _ => {}
        }
        let vale = if p.forma == "vale" {
            if p.vale_id.is_empty() {
                return Err(Erro::campo(&format!("pagamentos.{i}"), "Escolha qual vale-troca o cliente está usando."));
            }
            let usado =
                numeros::soma(pagamentos.iter().filter(|x| x.2.as_deref() == Some(p.vale_id.as_str())).map(|x| x.1))?;
            let saldo = crate::devolucoes::saldo_vale(conn, &p.vale_id)?;
            if numeros::soma([usado, valor])? > saldo {
                return Err(Erro::campo(
                    &format!("pagamentos.{i}"),
                    format!("Este vale-troca só tem R$ {}.", numeros::escrever_dinheiro((saldo - usado).max(0))),
                ));
            }
            Some(p.vale_id.clone())
        } else {
            None
        };
        pagamentos.push((p.forma.clone(), valor, vale));
    }
    if c.v_total > 0 && pagamentos.is_empty() {
        return Err(Erro::campo("pagamentos", "Informe como o cliente pagou."));
    }
    if c.v_pago < c.v_total {
        return Err(Erro::campo(
            "pagamentos",
            format!("Faltam {} para fechar a venda.", numeros::escrever_dinheiro(c.v_total - c.v_pago)),
        ));
    }
    let dinheiro = numeros::soma(pagamentos.iter().filter(|p| p.0 == "dinheiro").map(|p| p.1))?;
    if c.v_troco > dinheiro {
        return Err(Erro::campo("pagamentos", "Pagamento a mais só em dinheiro (para dar troco)."));
    }
    if !c.crediario.erro.is_empty() {
        let campo = if c.crediario.campo.is_empty() { "pagamentos" } else { &c.crediario.campo };
        return Err(Erro::campo(campo, c.crediario.erro.clone()));
    }
    if !c.cheques.erro.is_empty() {
        let campo = if c.cheques.campo.is_empty() { "pagamentos" } else { &c.cheques.campo };
        return Err(Erro::campo(campo, c.cheques.erro.clone()));
    }
    if !c.erro_encomenda.is_empty() {
        return Err(Erro::campo("cliente", c.erro_encomenda.clone()));
    }
    let cliente_id = if e.cliente_id.is_empty() { None } else { Some(e.cliente_id.clone()) };
    conferir_cliente(conn, &cliente_id)?;
    let cliente_documento = match &cliente_id {
        Some(id) => conn
            .query_row("SELECT documento FROM pessoa WHERE id = ?1", [id], |l| l.get::<_, String>(0))
            .optional()?
            .unwrap_or_default(),
        None => String::new(),
    };
    let id = tempo::novo_id();
    let numero = proximo_numero(conn, dispositivo, "venda")?;
    let agora = tempo::agora();
    conn.execute(
        "INSERT INTO venda (id, tipo, numero, dispositivo, caixa_nome, situacao, sessao_id, cliente_id, cliente_nome,
                            cliente_documento, vendedor_id, tabela, subtotal, desconto, total, troco, observacoes,
                            orcamento_id, criado_em)
         VALUES (?1, 'venda', ?2, ?3, ?4, 'concluida', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            id,
            numero,
            dispositivo,
            sessao.caixa_nome,
            sessao.id,
            cliente_id,
            c.cliente,
            cliente_documento,
            usuario_id,
            e.tabela.clamp(1, 5),
            c.v_subtotal,
            c.v_desconto,
            c.v_total,
            c.v_troco,
            e.observacoes.trim(),
            if e.orcamento_id.is_empty() { None } else { Some(e.orcamento_id.clone()) },
            agora
        ],
    )?;
    let motivo = format!("Venda {numero} · {}", sessao.caixa_nome);
    gravar_itens(conn, &id, &c)?;
    for it in c.itens.iter().filter(|i| !i.servico) {
        estoque::lancar(conn, dispositivo, Some(usuario_id), &it.produto_id, -it.v_quantidade, "venda", &motivo)?;
    }
    for (forma, valor, vale) in &pagamentos {
        conn.execute(
            "INSERT INTO venda_pagamento (id, venda_id, forma, valor, vale_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![tempo::novo_id(), id, forma, valor, vale],
        )?;
    }
    if c.crediario.v_valor > 0 {
        receber::criar_da_venda(
            conn,
            dispositivo,
            usuario_id,
            &id,
            &format!("Venda {numero} · {}", sessao.caixa_nome),
            &e.cliente_id,
            c.crediario.v_valor,
            e.parcelas,
            &e.primeiro_vencimento,
        )?;
    }
    if c.cheques.v_valor > 0 {
        let cliente = if e.cliente_id.is_empty() { None } else { Some(e.cliente_id.as_str()) };
        cheques::criar(conn, dispositivo, usuario_id, cliente, Some(&id), None, c.cheques.v_valor, &e.cheques)?;
    }
    if !e.orcamento_id.is_empty() {
        let n = conn.execute(
            "UPDATE venda SET situacao = 'vendido' WHERE id = ?1 AND tipo = 'orcamento' AND situacao = 'aberto'",
            [&e.orcamento_id],
        )?;
        // Já vendido (aqui ou em outro caixa) ou cancelado: a venda não entra.
        if n != 1 {
            return Err(Erro::validacao(
                "Este orçamento já foi vendido ou não está mais aberto. Abra o orçamento de novo.",
            ));
        }
    }
    let pedir: Vec<(String, String, i64)> = c
        .itens
        .iter()
        .filter(|i| i.encomendar)
        .map(|i| {
            let d = if i.marca.is_empty() { i.descricao.clone() } else { format!("{} · {}", i.descricao, i.marca) };
            (i.produto_id.clone(), d, i.v_falta)
        })
        .collect();
    encomendas::criar_da_venda(conn, dispositivo, usuario_id, &id, &e.cliente_id, &pedir)?;
    if !e.encomenda_id.is_empty() {
        encomendas::entregue_na_venda(conn, &e.encomenda_id, &id)?;
    }
    if !e.os_id.is_empty() {
        crate::oficina::entregue_na_venda(conn, dispositivo, usuario_id, &e.os_id, &id)?;
    }
    Ok(id)
}

fn gravar_itens(conn: &Connection, venda_id: &str, c: &Calculo) -> Resultado<()> {
    for (ordem, it) in c.itens.iter().enumerate() {
        conn.execute(
            "INSERT INTO venda_item (id, venda_id, ordem, produto_id, codigo, descricao, unidade, quantidade, preco,
                                     desconto, total, custo, comissao)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                tempo::novo_id(),
                venda_id,
                ordem as i64,
                if it.produto_id.is_empty() { None } else { Some(&it.produto_id) },
                it.codigo,
                it.descricao,
                it.unidade,
                it.v_quantidade,
                it.v_preco,
                it.v_desconto,
                it.v_total,
                it.v_custo,
                it.v_comissao
            ],
        )?;
    }
    Ok(())
}

/// Grava um orçamento (não mexe no estoque).
/// O cliente escolhido ainda existe? (Pode ter sido apagado em outro caixa.)
fn conferir_cliente(conn: &Connection, cliente_id: &Option<String>) -> Resultado<()> {
    if let Some(id) = cliente_id {
        let existe: bool =
            conn.query_row("SELECT 1 FROM pessoa WHERE id = ?1", [id], |_| Ok(true)).optional()?.unwrap_or(false);
        if !existe {
            return Err(Erro::campo("cliente", "Esse cliente não existe mais. Escolha o cliente de novo."));
        }
    }
    Ok(())
}

pub fn salvar_orcamento(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    caixa_nome: &str,
    e: &VendaEntrada,
) -> Resultado<String> {
    let c = calcular(conn, e)?;
    conferir_itens(&c)?;
    let dias = if e.validade_dias <= 0 { config::validade_orcamento(conn)? } else { e.validade_dias.min(365) };
    let validade: String =
        conn.query_row("SELECT date('now', 'localtime', ?1)", [format!("+{dias} days")], |l| l.get(0))?;
    let id = tempo::novo_id();
    let numero = proximo_numero(conn, dispositivo, "orcamento")?;
    let cliente_id = if e.cliente_id.is_empty() { None } else { Some(e.cliente_id.clone()) };
    conferir_cliente(conn, &cliente_id)?;
    conn.execute(
        "INSERT INTO venda (id, tipo, numero, dispositivo, caixa_nome, situacao, cliente_id, cliente_nome, vendedor_id,
                            tabela, subtotal, desconto, total, validade, observacoes, criado_em)
         VALUES (?1, 'orcamento', ?2, ?3, ?4, 'aberto', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            id,
            numero,
            dispositivo,
            caixa_nome,
            cliente_id,
            c.cliente,
            usuario_id,
            e.tabela.clamp(1, 5),
            c.v_subtotal,
            c.v_desconto,
            c.v_total,
            validade,
            e.observacoes.trim(),
            tempo::agora()
        ],
    )?;
    gravar_itens(conn, &id, &c)?;
    Ok(id)
}

/// Cancela uma venda (as peças voltam ao estoque) ou um orçamento.
pub fn cancelar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    let (tipo, situacao, numero, caixa_nome): (String, String, i64, String) = conn
        .query_row("SELECT tipo, situacao, numero, caixa_nome FROM venda WHERE id = ?1", [id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?))
        })
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Venda".into()))?;
    let motivo = texto::limpar(motivo);
    if tipo == "venda" {
        if situacao != "concluida" {
            return Err(Erro::validacao("Esta venda já foi cancelada."));
        }
        if motivo.is_empty() {
            return Err(Erro::campo("motivo", "Diga o motivo do cancelamento."));
        }
        if crate::devolucoes::tem_devolucao(conn, id)? {
            return Err(Erro::validacao(
                "Esta venda tem devolução. Desfaça a devolução (em Trocas e devoluções) antes de cancelar a venda.",
            ));
        }
        cheques::cancelar_de(conn, usuario_id, "venda_id", id)?;
        encomendas::cancelar_da_venda(conn, id)?;
        crate::oficina::cancelar_da_venda(conn, id)?;
        receber::cancelar_da_venda(conn, usuario_id, id, &motivo)?;
        let itens: Vec<(String, i64)> = conn
            .prepare("SELECT produto_id, quantidade FROM venda_item WHERE venda_id = ?1 AND produto_id IS NOT NULL")?
            .query_map([id], |l| Ok((l.get(0)?, l.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (pid, q) in itens {
            estoque::lancar(
                conn,
                dispositivo,
                Some(usuario_id),
                &pid,
                q,
                "cancelamento",
                &format!("Cancelamento da venda {numero} · {caixa_nome} · {motivo}"),
            )?;
        }
        conn.execute(
            "UPDATE venda SET situacao = 'cancelada', cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4 WHERE id = ?1",
            params![id, tempo::agora(), usuario_id, motivo],
        )?;
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "venda.cancelar",
            "venda",
            id,
            &[format!("Motivo: {motivo}")],
        )?;
    } else {
        if situacao != "aberto" {
            return Err(Erro::validacao("Só dá para cancelar um orçamento em aberto."));
        }
        conn.execute(
            "UPDATE venda SET situacao = 'cancelado', cancelado_em = ?2, cancelado_por = ?3, motivo_cancelamento = ?4 WHERE id = ?1",
            params![id, tempo::agora(), usuario_id, motivo],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Ler.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendaFicha {
    pub id: String,
    pub tipo: String,
    pub numero: i64,
    pub caixa: String,
    pub situacao: String,
    pub cliente_id: String,
    pub cliente: String,
    pub cliente_documento: String,
    pub vendedor: String,
    pub tabela: i64,
    pub subtotal: String,
    pub desconto: String,
    pub total: String,
    pub troco: String,
    pub validade: String,
    pub vencido: bool,
    pub observacoes: String,
    pub criado_em: String,
    pub cancelado_em: String,
    pub motivo_cancelamento: String,
    pub itens: Vec<ItemFicha>,
    pub pagamentos: Vec<PagamentoFicha>,
    /// Parcelas do crediário (se houver).
    pub parcelas: Vec<receber::Parcela>,
    /// Cheques: "nº 000123 · Itaú · bom para 15/11/2026 · R$ 100,00 · Em carteira".
    pub cheques: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemFicha {
    pub produto_id: String,
    pub codigo: String,
    pub descricao: String,
    pub unidade: String,
    pub quantidade: String,
    pub preco: String,
    pub desconto: String,
    pub total: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagamentoFicha {
    pub forma: String,
    pub nome: String,
    pub valor: String,
}

pub fn ficha(conn: &Connection, id: &str) -> Resultado<VendaFicha> {
    #[allow(clippy::type_complexity)]
    let v: (
        String,
        i64,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<String>,
        String,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT v.tipo, v.numero, v.caixa_nome, v.situacao, v.cliente_id, v.cliente_nome, v.cliente_documento,
                    COALESCE(u.nome, ''), v.tabela, v.subtotal, v.desconto, v.total, v.troco, v.validade,
                    v.observacoes, v.criado_em, v.cancelado_em, v.motivo_cancelamento
               FROM venda v LEFT JOIN usuario u ON u.id = v.vendedor_id WHERE v.id = ?1",
            [id],
            |l| {
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
                    l.get(11)?,
                    l.get(12)?,
                    l.get(13)?,
                    l.get(14)?,
                    l.get(15)?,
                    l.get(16)?,
                    l.get(17)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Venda".into()))?;
    let hoje: String = conn.query_row("SELECT date('now', 'localtime')", [], |l| l.get(0))?;
    let itens = conn
        .prepare_cached(
            "SELECT COALESCE(produto_id, ''), codigo, descricao, unidade, quantidade, preco, desconto, total
               FROM venda_item WHERE venda_id = ?1 ORDER BY ordem",
        )?
        .query_map([id], |l| {
            let desconto: i64 = l.get(6)?;
            Ok(ItemFicha {
                produto_id: l.get(0)?,
                codigo: l.get(1)?,
                descricao: l.get(2)?,
                unidade: l.get(3)?,
                quantidade: numeros::escrever_quantidade(l.get(4)?),
                preco: numeros::escrever_dinheiro(l.get(5)?),
                desconto: if desconto > 0 { numeros::escrever_dinheiro(desconto) } else { String::new() },
                total: numeros::escrever_dinheiro(l.get(7)?),
            })
        })?
        .collect::<Result<_, _>>()?;
    let pagamentos = conn
        .prepare_cached("SELECT forma, valor FROM venda_pagamento WHERE venda_id = ?1")?
        .query_map([id], |l| {
            let forma: String = l.get(0)?;
            Ok(PagamentoFicha {
                nome: caixa::nome_forma(&forma).to_string(),
                forma,
                valor: numeros::escrever_dinheiro(l.get(1)?),
            })
        })?
        .collect::<Result<_, _>>()?;
    let validade = v.13.clone().unwrap_or_default();
    Ok(VendaFicha {
        id: id.to_string(),
        tipo: v.0,
        numero: v.1,
        caixa: v.2,
        situacao: v.3.clone(),
        cliente_id: v.4.unwrap_or_default(),
        cliente: v.5,
        cliente_documento: documento::escrever(&v.6),
        vendedor: v.7,
        tabela: v.8,
        subtotal: numeros::escrever_dinheiro(v.9),
        desconto: numeros::escrever_dinheiro(v.10),
        total: numeros::escrever_dinheiro(v.11),
        troco: numeros::escrever_dinheiro(v.12),
        vencido: !validade.is_empty() && validade < hoje && v.3 == "aberto",
        validade: crate::fiscal::data_br(&validade),
        observacoes: v.14,
        criado_em: v.15,
        cancelado_em: v.16.unwrap_or_default(),
        motivo_cancelamento: v.17,
        itens,
        pagamentos,
        parcelas: receber::parcelas_da_venda(conn, id)?,
        cheques: cheques_da(conn, "venda_id", id)?,
    })
}

/// Cheques de uma venda ou recebimento, em uma linha cada.
pub fn cheques_da(conn: &Connection, coluna: &str, id: &str) -> Resultado<Vec<String>> {
    let mut c = conn.prepare(&format!(
        "SELECT numero, banco, bom_para, valor, situacao FROM cheque WHERE {coluna} = ?1 ORDER BY bom_para, numero"
    ))?;
    let linhas = c.query_map([id], |l| {
        let banco: String = l.get(1)?;
        Ok([
            format!("nº {}", l.get::<_, String>(0)?),
            banco,
            format!("bom para {}", crate::fiscal::data_br(&l.get::<_, String>(2)?)),
            format!("R$ {}", numeros::escrever_dinheiro(l.get(3)?)),
            cheques::nome_situacao(&l.get::<_, String>(4)?).to_string(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · "))
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// O orçamento no formato do carrinho, para virar venda.
pub fn orcamento_para_venda(conn: &Connection, id: &str) -> Resultado<VendaEntrada> {
    let (tipo, situacao, cliente_id, cliente_nome, tabela, desconto, observacoes): (
        String,
        String,
        Option<String>,
        String,
        i64,
        i64,
        String,
    ) = conn
        .query_row(
            "SELECT tipo, situacao, cliente_id, cliente_nome, tabela,
                    desconto - (SELECT COALESCE(SUM(desconto), 0) FROM venda_item WHERE venda_id = v.id), observacoes
               FROM venda v WHERE id = ?1",
            [id],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)),
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Orçamento".into()))?;
    if tipo != "orcamento" {
        return Err(Erro::Interno("não é orçamento".into()));
    }
    if situacao != "aberto" {
        return Err(Erro::validacao("Este orçamento não está mais em aberto."));
    }
    let itens = conn
        .prepare("SELECT COALESCE(produto_id, ''), quantidade, preco, desconto, descricao FROM venda_item WHERE venda_id = ?1 ORDER BY ordem")?
        .query_map([id], |l| {
            let desconto: i64 = l.get(3)?;
            let produto_id: String = l.get(0)?;
            Ok(ItemEntrada {
                descricao: if produto_id.is_empty() { l.get(4)? } else { String::new() },
                produto_id,
                quantidade: numeros::escrever_quantidade(l.get(1)?),
                preco: numeros::escrever_dinheiro(l.get(2)?),
                desconto: if desconto > 0 { numeros::escrever_dinheiro(desconto) } else { String::new() },
                encomendar: false,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(VendaEntrada {
        itens,
        tabela,
        cliente_id: cliente_id.unwrap_or_default(),
        cliente_nome,
        desconto: if desconto > 0 { numeros::escrever_dinheiro(desconto) } else { String::new() },
        pagamentos: vec![],
        observacoes,
        orcamento_id: id.to_string(),
        ..Default::default()
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendaResumo {
    pub id: String,
    pub tipo: String,
    pub numero: i64,
    pub caixa: String,
    pub situacao: String,
    pub cliente: String,
    pub vendedor: String,
    pub total: String,
    pub itens: i64,
    pub criado_em: String,
    pub validade: String,
    pub vencido: bool,
    pub formas: String,
}

/// Lista vendas ou orçamentos. `sessao` filtra um caixa.
pub fn listar(conn: &Connection, tipo: &str, busca: &str, sessao: &str, limite: i64) -> Resultado<Vec<VendaResumo>> {
    let hoje: String = conn.query_row("SELECT date('now', 'localtime')", [], |l| l.get(0))?;
    let mut sql = String::from(
        "SELECT v.id, v.tipo, v.numero, v.caixa_nome, v.situacao, v.cliente_nome, COALESCE(u.nome, ''), v.total,
                (SELECT COUNT(*) FROM venda_item i WHERE i.venda_id = v.id), v.criado_em, COALESCE(v.validade, ''),
                (SELECT group_concat(forma, ',') FROM venda_pagamento p WHERE p.venda_id = v.id)
           FROM venda v LEFT JOIN usuario u ON u.id = v.vendedor_id
          WHERE v.tipo = ?1",
    );
    let mut valores: Vec<Value> = vec![Value::Text(tipo.to_string())];
    if !sessao.is_empty() {
        valores.push(Value::Text(sessao.to_string()));
        sql.push_str(&format!(" AND v.sessao_id = ?{}", valores.len()));
    }
    let t = texto::normalizar(busca);
    if !t.is_empty() {
        valores.push(Value::Text(format!("%{t}%")));
        let n = valores.len();
        sql.push_str(&format!(
            " AND (normalizar(v.cliente_nome) LIKE ?{n} OR CAST(v.numero AS TEXT) = ?{m}
                   OR EXISTS (SELECT 1 FROM venda_item i WHERE i.venda_id = v.id AND normalizar(i.descricao) LIKE ?{n}))",
            m = n + 1
        ));
        valores.push(Value::Text(busca.trim().to_string()));
    }
    sql.push_str(&format!(" ORDER BY v.criado_em DESC LIMIT {}", limite.clamp(1, 1000)));
    let mut c = conn.prepare(&sql)?;
    let linhas = c.query_map(params_from_iter(valores), |l| {
        let validade: String = l.get(10)?;
        let situacao: String = l.get(4)?;
        let formas: Option<String> = l.get(11)?;
        Ok(VendaResumo {
            id: l.get(0)?,
            tipo: l.get(1)?,
            numero: l.get(2)?,
            caixa: l.get(3)?,
            vencido: !validade.is_empty() && validade < hoje && situacao == "aberto",
            situacao,
            cliente: l.get(5)?,
            vendedor: l.get(6)?,
            total: numeros::escrever_dinheiro(l.get(7)?),
            itens: l.get(8)?,
            criado_em: l.get(9)?,
            validade: crate::fiscal::data_br(&validade),
            formas: formas
                .unwrap_or_default()
                .split(',')
                .filter(|f| !f.is_empty())
                .map(caixa::nome_forma)
                .collect::<Vec<_>>()
                .join(" + "),
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Nome do caixa (para o orçamento feito fora de um caixa aberto).
pub fn nome_caixa(conn: &Connection) -> Resultado<String> {
    config::nome_caixa(conn)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{produtos, usuarios};

    fn base() -> (Banco, String, String, String) {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        f.estoque_inicial = "5".into();
        f.desconto_maximo = "10".into();
        let p1 = produtos::salvar(&b.conn, &b.dispositivo, &u.id, &f).unwrap(); // preço 80,00
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = "Óleo 1 L".into();
        f.custo = "21,50".into();
        f.estoque_inicial = "2".into();
        let p2 = produtos::salvar(&b.conn, &b.dispositivo, &u.id, &f).unwrap(); // preço 43,00
        (b, u.id, p1, p2)
    }

    fn item(id: &str, q: &str) -> ItemEntrada {
        ItemEntrada { produto_id: id.into(), quantidade: q.into(), ..Default::default() }
    }

    #[test]
    fn calcula_descontos_estoque_e_troco() {
        let (b, _, p1, p2) = base();
        let mut e = VendaEntrada { itens: vec![item(&p1, "2"), item(&p2, "3")], tabela: 1, ..Default::default() };
        let c = calcular(&b.conn, &e).unwrap();
        assert_eq!(c.subtotal, "289,00");
        assert!(c.itens[1].falta_estoque, "3 óleos, só tem 2");
        assert!(!c.itens[0].falta_estoque);

        e.itens[0].desconto = "10%".into();
        e.desconto = "5,00".into();
        e.pagamentos = vec![PagamentoEntrada { forma: "dinheiro".into(), valor: "300".into(), ..Default::default() }];
        let c = calcular(&b.conn, &e).unwrap();
        // 160 - 16 + 129 - 5 = 268
        assert_eq!(c.total, "268,00");
        assert_eq!(c.troco, "32,00");

        // desconto acima do máximo da peça (10%)
        e.itens[0].desconto = "11%".into();
        assert!(!calcular(&b.conn, &e).unwrap().itens[0].erro.is_empty());
        // preço digitado abaixo da tabela também conta como desconto
        e.itens[0].desconto = String::new();
        e.itens[0].preco = "70".into();
        assert!(!calcular(&b.conn, &e).unwrap().itens[0].erro.is_empty());
        e.itens[0].preco = "75".into();
        assert!(calcular(&b.conn, &e).unwrap().itens[0].erro.is_empty());
    }

    /// C-05: valores na fronteira e além. Nada "dá a volta", nada é gravado.
    #[test]
    fn valores_extremos_sao_recusados_sem_gravar() {
        let (b, u, p1, _) = base();
        let c = &b.conn;
        caixa::abrir(c, &b.dispositivo, &u, "Caixa 1", "0").unwrap();
        let contar = |t: &str| -> i64 { c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |l| l.get(0)).unwrap() };
        let antes = (contar("venda"), contar("venda_item"), contar("estoque_movimento"), contar("caixa_movimento"));
        let avulso = |q: &str, p: &str| ItemEntrada {
            produto_id: String::new(),
            descricao: "Serviço".into(),
            quantidade: q.into(),
            preco: p.into(),
            ..Default::default()
        };
        // Número com mais de 11 dígitos inteiros: recusado na leitura.
        assert!(numeros::ler_dinheiro("999999999999").is_err());
        assert_eq!(numeros::ler_dinheiro("99999999999,99"), Ok(Some(9_999_999_999_999)));
        // Quantidade e preço no máximo: o item sozinho já passa do total da venda.
        let e = VendaEntrada { itens: vec![avulso("1000000", "1000000000")], tabela: 1, ..Default::default() };
        let r = calcular(c, &e).unwrap();
        assert!(r.itens[0].erro.contains("grande"), "{}", r.itens[0].erro);
        // Muitos itens grandes: a soma é conferida e recusada, sem pânico.
        let e = VendaEntrada { itens: vec![avulso("1", "1000000000"); 200], tabela: 1, ..Default::default() };
        assert!(calcular(c, &e).unwrap_err().to_string().contains("grande"));
        // Pagamentos enormes: recusados.
        let e = VendaEntrada {
            itens: vec![item(&p1, "1")],
            tabela: 1,
            pagamentos: vec![
                PagamentoEntrada {
                    forma: "dinheiro".into(),
                    valor: "99999999999".into(),
                    ..Default::default()
                };
                20
            ],
            ..Default::default()
        };
        assert!(concluir(c, &b.dispositivo, &u, &e).is_err());
        // Formas de pagamento demais.
        let e = VendaEntrada {
            itens: vec![item(&p1, "1")],
            tabela: 1,
            pagamentos: vec![PagamentoEntrada { forma: "pix".into(), valor: "1".into(), ..Default::default() }; 51],
            ..Default::default()
        };
        assert!(concluir(c, &b.dispositivo, &u, &e).is_err());
        // Desconto negativo e acima de 100%.
        let mut e = VendaEntrada { itens: vec![item(&p1, "1")], tabela: 1, ..Default::default() };
        e.desconto = "-5".into();
        assert!(!calcular(c, &e).unwrap().erro_desconto.is_empty());
        e.desconto = "101%".into();
        assert!(!calcular(c, &e).unwrap().erro_desconto.is_empty());
        // Nenhuma das tentativas gravou nada.
        let depois = (contar("venda"), contar("venda_item"), contar("estoque_movimento"), contar("caixa_movimento"));
        assert_eq!(antes, depois);
        assert_eq!(estoque::saldo(c, &p1).unwrap(), 5000);
    }

    #[test]
    fn vende_cancela_e_fecha_o_caixa() {
        let (b, u, p1, p2) = base();
        let c = &b.conn;
        let mut e = VendaEntrada {
            itens: vec![item(&p1, "1"), item(&p2, "1")],
            tabela: 1,
            pagamentos: vec![PagamentoEntrada { forma: "dinheiro".into(), valor: "150".into(), ..Default::default() }],
            ..Default::default()
        };
        // sem caixa aberto não vende
        assert!(concluir(c, &b.dispositivo, &u, &e).is_err());
        caixa::abrir(c, &b.dispositivo, &u, "Caixa 1", "100").unwrap();
        assert!(caixa::abrir(c, &b.dispositivo, &u, "Caixa 1", "0").is_err());

        let v1 = concluir(c, &b.dispositivo, &u, &e).unwrap();
        assert_eq!(estoque::saldo(c, &p1).unwrap(), 4000);
        let f = ficha(c, &v1).unwrap();
        assert_eq!((f.numero, f.total.as_str(), f.troco.as_str()), (1, "123,00", "27,00"));

        // pago a menos
        e.pagamentos = vec![PagamentoEntrada { forma: "pix".into(), valor: "100".into(), ..Default::default() }];
        assert_eq!(concluir(c, &b.dispositivo, &u, &e).unwrap_err().nome_do_campo(), Some("pagamentos"));
        // troco só em dinheiro
        e.pagamentos = vec![PagamentoEntrada { forma: "pix".into(), valor: "130".into(), ..Default::default() }];
        assert!(concluir(c, &b.dispositivo, &u, &e).is_err());
        // dividido em Pix e cartão
        e.pagamentos = vec![
            PagamentoEntrada { forma: "pix".into(), valor: "100".into(), ..Default::default() },
            PagamentoEntrada { forma: "debito".into(), valor: "23".into(), ..Default::default() },
        ];
        let v2 = concluir(c, &b.dispositivo, &u, &e).unwrap();
        assert_eq!(ficha(c, &v2).unwrap().numero, 2);
        // crediário só com cliente cadastrado
        e.pagamentos = vec![PagamentoEntrada { forma: "crediario".into(), valor: "123".into(), ..Default::default() }];
        assert_eq!(concluir(c, &b.dispositivo, &u, &e).unwrap_err().nome_do_campo(), Some("cliente"));

        // cancelar devolve ao estoque
        assert!(cancelar(c, &b.dispositivo, &u, &v2, "").is_err());
        cancelar(c, &b.dispositivo, &u, &v2, "Cliente desistiu").unwrap();
        assert_eq!(estoque::saldo(c, &p1).unwrap(), 4000);

        caixa::movimentar(c, &b.dispositivo, &u, "sangria", "50", "Cofre").unwrap();
        caixa::movimentar(c, &b.dispositivo, &u, "suprimento", "10", "Troco").unwrap();
        assert!(caixa::movimentar(c, &b.dispositivo, &u, "sangria", "1000", "Demais").is_err());
        let s = caixa::sessao_aberta(c, &b.dispositivo).unwrap().unwrap();
        let r = caixa::resumo(c, &s.id).unwrap();
        // 100 inicial + 150 recebido − 27 troco − 50 + 10 = 183
        assert_eq!(r.esperado, "183,00");
        assert_eq!(r.vendas, 1);
        assert_eq!(r.canceladas, 1);
        assert_eq!(r.por_forma[0].valor, "123,00");
        caixa::fechar(c, &b.dispositivo, &u, "180", "").unwrap();
        let r = caixa::resumo(c, &s.id).unwrap();
        assert_eq!(r.diferenca, "-3,00");
        assert!(caixa::sessao_aberta(c, &b.dispositivo).unwrap().is_none());

        assert_eq!(listar(c, "venda", "", "", 50).unwrap().len(), 2);
        assert_eq!(listar(c, "venda", "óleo", "", 50).unwrap().len(), 2);
    }

    #[test]
    fn orcamento_vira_venda() {
        let (b, u, p1, _) = base();
        let c = &b.conn;
        let e = VendaEntrada {
            itens: vec![ItemEntrada {
                descricao: String::new(),
                produto_id: p1.clone(),
                quantidade: "2".into(),
                preco: "78".into(),
                desconto: String::new(),
                encomendar: false,
            }],
            tabela: 1,
            cliente_nome: "Oficina do Zé".into(),
            desconto: "6".into(),
            validade_dias: 10,
            ..Default::default()
        };
        let o = salvar_orcamento(c, &b.dispositivo, &u, "Caixa 1", &e).unwrap();
        assert_eq!(estoque::saldo(c, &p1).unwrap(), 5000, "orçamento não mexe no estoque");
        let f = ficha(c, &o).unwrap();
        assert_eq!((f.tipo.as_str(), f.situacao.as_str(), f.total.as_str()), ("orcamento", "aberto", "150,00"));

        let mut venda = orcamento_para_venda(c, &o).unwrap();
        assert_eq!(venda.desconto, "6,00");
        caixa::abrir(c, &b.dispositivo, &u, "Caixa 1", "0").unwrap();
        venda.pagamentos =
            vec![PagamentoEntrada { forma: "credito".into(), valor: "150".into(), ..Default::default() }];
        concluir(c, &b.dispositivo, &u, &venda).unwrap();
        assert_eq!(ficha(c, &o).unwrap().situacao, "vendido");
        assert!(orcamento_para_venda(c, &o).is_err());
        assert_eq!(listar(c, "orcamento", "zé", "", 10).unwrap().len(), 1);
    }

    #[test]
    fn servico_oferta_preco_aberto_e_unidade_inteira() {
        let (b, u, p1, _) = base();
        let c = &b.conn;
        // Oferta vale para a primeira tabela; vencida, volta o preço normal.
        let mut f = produtos::ficha(c, &p1).unwrap();
        f.oferta_preco = "70".into();
        produtos::salvar(c, &b.dispositivo, &u, &f).unwrap();
        let e = VendaEntrada { itens: vec![item(&p1, "1")], tabela: 1, ..Default::default() };
        assert_eq!(calcular(c, &e).unwrap().total, "70,00");
        f.oferta_ate = "01/01/2020".into();
        produtos::salvar(c, &b.dispositivo, &u, &f).unwrap();
        assert_eq!(calcular(c, &e).unwrap().total, "80,00");
        // UN não vende quebrado; KG vende.
        let e = VendaEntrada { itens: vec![item(&p1, "1,5")], tabela: 1, ..Default::default() };
        assert!(!calcular(c, &e).unwrap().itens[0].erro.is_empty());
        let mut f = produtos::ficha(c, &p1).unwrap();
        f.unidade = "KG".into();
        produtos::salvar(c, &b.dispositivo, &u, &f).unwrap();
        assert!(calcular(c, &e).unwrap().itens[0].erro.is_empty());

        // Serviço cadastrado: sem estoque, preço digitado na hora.
        let mut s = produtos::ficha_nova(c).unwrap();
        s.nome = "Instalação".into();
        s.tipo = "servico".into();
        s.preco_aberto = true;
        s.estoque_inicial = "3".into();
        s.precos.iter_mut().for_each(|p| p.lucro = String::new());
        let sid = produtos::salvar(c, &b.dispositivo, &u, &s).unwrap();
        assert_eq!(estoque::saldo(c, &sid).unwrap(), 0);
        let mut e = VendaEntrada { itens: vec![item(&sid, "1")], tabela: 1, ..Default::default() };
        let r = calcular(c, &e).unwrap();
        assert!(r.itens[0].servico && !r.itens[0].falta_estoque);
        assert!(!r.itens[0].erro.is_empty(), "precisa digitar o preço");
        e.itens[0].preco = "50".into();
        assert!(calcular(c, &e).unwrap().itens[0].erro.is_empty());
        estoque::lancar(c, &b.dispositivo, Some(&u), &sid, -1000, "venda", "x").unwrap();
        assert_eq!(estoque::saldo(c, &sid).unwrap(), 0);
        assert!(estoque::ajustar(c, &b.dispositivo, &u, &sid, "2", "contagem").is_err());
        // Produto com estoque não vira serviço.
        let mut f = produtos::ficha(c, &p1).unwrap();
        f.tipo = "servico".into();
        assert!(produtos::salvar(c, &b.dispositivo, &u, &f).is_err());
    }

    #[test]
    fn tamanhos_e_cores() {
        let (b, u, p1, _) = base();
        let c = &b.conn;
        assert!(produtos::criar_variacao(c, &b.dispositivo, &u, &p1, "", "M").is_err(), "falta o deste");
        let m = produtos::criar_variacao(c, &b.dispositivo, &u, &p1, "P", "M").unwrap();
        let g = produtos::criar_variacao(c, &b.dispositivo, &u, &m, "", "G").unwrap();
        assert!(produtos::criar_variacao(c, &b.dispositivo, &u, &g, "", "m").is_err(), "repetido");
        let fp = produtos::ficha(c, &p1).unwrap();
        let fg = produtos::ficha(c, &g).unwrap();
        assert_eq!(fp.nome, "Pastilha P");
        assert_eq!(fg.nome, "Pastilha G");
        assert_eq!(fg.precos[0].preco, fp.precos[0].preco);
        assert_eq!(fg.estoque, "0");
        assert_eq!(fp.estoque, "5");
        let nomes: Vec<String> = fg.grade_itens.iter().map(|i| i.variacao.clone()).collect();
        assert_eq!(nomes, ["P", "M", "G"]);
        // Na grade, o tamanho/cor não pode ficar vazio.
        let mut f = produtos::ficha(c, &m).unwrap();
        f.variacao = String::new();
        assert!(produtos::salvar(c, &b.dispositivo, &u, &f).is_err());
    }
}
