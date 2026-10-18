//! Compras: entrada de mercadoria pelo XML da NF-e do fornecedor.
//!
//! Caminho:
//! 1. `importar_xml` lê a nota, cadastra o fornecedor (se for novo) e cria um
//!    rascunho. Cada item já tenta se ligar sozinho a uma peça: pelo código
//!    que o fornecedor usou da última vez, pelo código de barras ou pelo
//!    código original/equivalente.
//! 2. Na tela, cada item fica "ligado" a uma peça, "nova" (cadastrar) ou
//!    "ignorar" (não entra no estoque). Dá para informar o frete pago à parte.
//! 3. `lancar` põe tudo no estoque, atualiza o custo (e o preço, se pedido)
//!    e guarda o código do fornecedor para a próxima nota.
//! 4. `estornar` desfaz a entrada no estoque (o custo não volta).
//!
//! Custo de cada peça = (valor dos produtos − desconto + frete + seguro +
//! outras despesas + IPI + ICMS-ST + parte do frete pago à parte) ÷
//! quantidade de peças. O Simples Nacional não aproveita crédito de IPI nem
//! de ICMS-ST, por isso eles entram no custo.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::nfe_xml::{self, ItemXml, NotaXml};
use crate::{auditoria, config, documento, estoque, fiscal, gtin, numeros, pessoas, produtos, tempo, texto};

// ---------------------------------------------------------------------------
// Importar.
// ---------------------------------------------------------------------------

/// Lê o XML e cria a compra em rascunho. Devolve o id da compra.
/// Aviso quando a nota de compra não é da loja (sem bloquear). Sem os
/// números do CNPJ, para não aparecer na tela com o olho fechado.
pub fn aviso_cnpj(conn: &Connection, destinatario: &str) -> Resultado<String> {
    let loja_cnpj = config::ler(conn, "loja_cnpj")?.unwrap_or_default();
    let destinatario = documento::limpar(destinatario);
    if destinatario.is_empty() {
        return Ok(String::new());
    }
    if loja_cnpj.is_empty() {
        return Ok(
            "Preencha o CNPJ da loja em Configurações para o Rustinho conferir se a nota é mesmo da loja.".into()
        );
    }
    if destinatario == loja_cnpj {
        return Ok(String::new());
    }
    let loja = config::nome_loja(conn)?;
    let loja = if loja.is_empty() { "da loja".to_string() } else { format!("da {loja}") };
    Ok(format!("Esta nota não é {loja}: foi emitida para outro CNPJ. Confira antes de lançar."))
}

/// A nota serve para entrada de compra? (Autorizada pela SEFAZ, de saída do
/// fornecedor, normal, e para o CNPJ da loja quando ele está preenchido.)
pub fn conferir_nota(conn: &Connection, nota: &NotaXml) -> Resultado<()> {
    if !nota.autorizada {
        return Err(Erro::validacao(
            "Este XML não tem a autorização da SEFAZ (ou a nota foi negada). Peça ao fornecedor o XML autorizado, o que vem com o protocolo.",
        ));
    }
    if !nota.incoerencia.is_empty() {
        return Err(Erro::validacao(format!(
            "Este XML parece alterado ou montado à mão: {}. Baixe de novo o XML no portal da SEFAZ ou peça ao fornecedor.",
            nota.incoerencia
        )));
    }
    if nota.tipo == "0" {
        return Err(Erro::validacao("Esta é uma nota de entrada, não uma venda do fornecedor para a loja."));
    }
    match nota.finalidade.as_str() {
        "" | "1" => {}
        "4" => return Err(Erro::validacao("Esta é uma nota de devolução, não de compra.")),
        _ => {
            return Err(Erro::validacao(
                "Esta é uma nota complementar ou de ajuste: ela não traz mercadoria para dar entrada.",
            ))
        }
    }
    let loja = documento::limpar(&config::ler(conn, "loja_cnpj")?.unwrap_or_default());
    let destinatario = documento::limpar(&nota.destinatario);
    if !loja.is_empty() && !destinatario.is_empty() && destinatario != loja {
        let nome = config::nome_loja(conn)?;
        let nome = if nome.is_empty() { "da loja".to_string() } else { format!("da {nome}") };
        return Err(Erro::validacao(format!(
            "Esta nota não é {nome}: foi emitida para outro CNPJ. Confira o CNPJ em Configurações ou peça a nota certa ao fornecedor."
        )));
    }
    Ok(())
}

pub fn importar_xml(conn: &Connection, dispositivo: &str, usuario_id: &str, xml: &str) -> Resultado<String> {
    let nota = nfe_xml::ler(xml).map_err(Erro::validacao)?;
    conferir_nota(conn, &nota)?;
    if nota.chave.len() == 44 {
        let ja: Option<(String, String)> = conn
            .query_row(
                "SELECT situacao, criado_em FROM compra WHERE chave = ?1 AND situacao <> 'estornada'",
                [&nota.chave],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .optional()?;
        if let Some((situacao, quando)) = ja {
            let como = if situacao == "lancada" { "lançada" } else { "importada (em rascunho)" };
            return Err(Erro::validacao(format!(
                "A nota {} deste fornecedor já foi {como} em {}.",
                nota.numero,
                fiscal::data_local_br(conn, &quando)?
            )));
        }
    }
    let (fornecedor_id, _) = pessoas::garantir_fornecedor(conn, dispositivo, usuario_id, &nota.emitente)?;
    let id = tempo::novo_id();
    let NotaXml { itens, .. } = nota.clone();
    let itens = ratear_do_cabecalho(&nota, itens)?;

    conn.execute(
        "INSERT INTO compra (id, fornecedor_id, chave, numero, serie, emissao, natureza, total_produtos, frete, seguro,
                             desconto, outras, ipi, icms_st, total, destinatario, xml, criado_em, criado_por, dispositivo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            id, fornecedor_id, nota.chave, nota.numero, nota.serie, nota.emissao, nota.natureza, nota.total_produtos,
            nota.frete, nota.seguro, nota.desconto, nota.outras, nota.ipi, nota.icms_st, nota.total,
            nota.destinatario, xml, tempo::agora(), usuario_id, dispositivo
        ],
    )?;
    for (ordem, item) in itens.iter().enumerate() {
        let (acao, produto_id, achado_por, fator) = ligar_sozinho(conn, &fornecedor_id, item)?;
        conn.execute(
            "INSERT INTO compra_item (id, compra_id, ordem, codigo_fornecedor, gtin, descricao, ncm, cest, cfop, unidade,
                                      origem, cst_icms, cst_pis, quantidade, valor_produtos, frete, seguro, desconto,
                                      outras, ipi, icms_st, acao, produto_id, achado_por, fator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21,
                     ?22, ?23, ?24, ?25)",
            params![
                tempo::novo_id(), id, ordem as i64, item.codigo, item.gtin, item.descricao, item.ncm, item.cest,
                item.cfop, item.unidade, item.origem, item.cst_icms, item.cst_pis, item.quantidade,
                item.valor_produtos, item.frete, item.seguro, item.desconto, item.outras, item.ipi, item.icms_st,
                acao, produto_id, achado_por, fator
            ],
        )?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.importar",
        "compra",
        &id,
        &[format!("NF {} · {} itens · {}", nota.numero, itens.len(), numeros::escrever_dinheiro(nota.total))],
    )?;
    Ok(id)
}

/// Se a nota trouxe frete/seguro/desconto/outras só no total, divide entre
/// os itens pelo valor de cada um.
fn ratear_do_cabecalho(nota: &NotaXml, mut itens: Vec<ItemXml>) -> Resultado<Vec<ItemXml>> {
    let pesos: Vec<i64> = itens.iter().map(|i| i.valor_produtos.max(0)).collect();
    let mut aplicar = |total: i64, pegar: fn(&mut ItemXml) -> &mut i64| -> Resultado<()> {
        let soma = numeros::soma(itens.iter_mut().map(|i| *pegar(i)))?;
        if soma == 0 && total > 0 {
            for (i, parte) in ratear(total, &pesos).into_iter().enumerate() {
                *pegar(&mut itens[i]) = parte;
            }
        }
        Ok(())
    };
    aplicar(nota.frete, |i| &mut i.frete)?;
    aplicar(nota.seguro, |i| &mut i.seguro)?;
    aplicar(nota.desconto, |i| &mut i.desconto)?;
    aplicar(nota.outras, |i| &mut i.outras)?;
    Ok(itens)
}

/// Divide `total` em partes proporcionais aos pesos, sem perder centavos
/// (as sobras vão para os maiores restos).
pub fn ratear(total: i64, pesos: &[i64]) -> Vec<i64> {
    // Somas em i128: nenhuma quantidade de itens "dá a volta" (R-10).
    let soma: i128 = pesos.iter().map(|p| *p as i128).sum();
    if pesos.is_empty() {
        return vec![];
    }
    if soma <= 0 {
        // sem valores: divide igual
        let n = pesos.len() as i64;
        return (0..n).map(|i| total / n + if i < total % n { 1 } else { 0 }).collect();
    }
    let mut partes: Vec<i64> =
        pesos.iter().map(|p| crate::numeros::saturar(total as i128 * *p as i128 / soma)).collect();
    let mut sobra = total as i128 - partes.iter().map(|p| *p as i128).sum::<i128>();
    let mut restos: Vec<(usize, i128)> =
        pesos.iter().enumerate().map(|(i, p)| (i, (total as i128 * *p as i128) % soma)).collect();
    restos.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (i, _) in restos {
        if sobra <= 0 {
            break;
        }
        partes[i] += 1;
        sobra -= 1;
    }
    partes
}

/// Tenta achar a peça do item: código do fornecedor → código de barras →
/// código original/equivalente. Devolve (ação, peça, como achou, fator).
fn ligar_sozinho(
    conn: &Connection,
    fornecedor_id: &str,
    item: &ItemXml,
) -> Resultado<(String, Option<String>, String, i64)> {
    let compacto = texto::compactar(&item.codigo);
    if !compacto.is_empty() {
        let r: Option<(String, i64)> = conn
            .query_row(
                "SELECT f.produto_id, f.fator FROM produto_fornecedor f JOIN produto p ON p.id = f.produto_id
                  WHERE f.fornecedor_id = ?1 AND f.codigo_compacto = ?2",
                params![fornecedor_id, compacto],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .optional()?;
        if let Some((pid, fator)) = r {
            return Ok(("ligar".into(), Some(pid), "fornecedor".into(), fator));
        }
    }
    if !item.gtin.is_empty() {
        // Só se o código de barras for de uma peça só (dois caixas podem ter
        // cadastrado o mesmo): com duas, a pessoa escolhe na tela.
        let r: Vec<String> = conn
            .prepare_cached("SELECT id FROM produto WHERE codigo_barras = ?1 LIMIT 2")?
            .query_map([&item.gtin], |l| l.get(0))?
            .collect::<Result<_, _>>()?;
        if r.len() == 1 {
            return Ok(("ligar".into(), Some(r[0].clone()), "barras".into(), 1000));
        }
    }
    if compacto.len() >= 3 {
        let r: Vec<String> = conn
            .prepare_cached("SELECT id FROM produto WHERE instr(codigos, ' ' || ?1 || ' ') > 0 LIMIT 2")?
            .query_map([&compacto], |l| l.get(0))?
            .collect::<Result<_, _>>()?;
        if r.len() == 1 {
            return Ok(("ligar".into(), Some(r[0].clone()), "codigo".into(), 1000));
        }
    }
    Ok((String::new(), None, String::new(), 1000))
}

// ---------------------------------------------------------------------------
// Ler para a tela.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompraFicha {
    pub id: String,
    pub situacao: String,
    pub fornecedor_id: String,
    pub fornecedor: String,
    pub fornecedor_documento: String,
    pub fornecedor_cidade: String,
    pub chave: String,
    pub numero: String,
    pub serie: String,
    pub emissao: String,
    pub natureza: String,
    pub total_produtos: String,
    pub frete: String,
    pub ipi: String,
    pub icms_st: String,
    pub desconto: String,
    pub outras: String,
    pub total: String,
    pub frete_extra: String,
    pub atualizar_precos: bool,
    pub observacoes: String,
    /// Aviso quando a nota não é para o CNPJ da loja.
    pub aviso_destinatario: String,
    pub itens: Vec<ItemFicha>,
    pub pendentes: i64,
    pub novas: i64,
    pub ignorados: i64,
    pub custo_total: String,
    pub lancada_em: String,
    pub lancada_por: String,
    pub historico: Vec<auditoria::Registro>,
    /// Parcelas da nota (duplicatas do XML): "001 · vence 07/11/2026 · R$ 550,25".
    pub duplicatas: Vec<String>,
    /// Lançar as duplicatas no A pagar ao lançar a compra.
    pub gerar_pagar: bool,
    /// Já lançada: parcelas no A pagar (quantas).
    pub parcelas_pagar: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemFicha {
    pub id: String,
    pub ordem: i64,
    pub codigo_fornecedor: String,
    pub gtin: String,
    pub descricao: String,
    pub ncm: String,
    pub cest: String,
    pub cfop: String,
    pub unidade: String,
    pub quantidade: String,
    pub valor_unitario: String,
    pub valor_produtos: String,
    pub despesas: String,
    pub acao: String,
    pub achado_por: String,
    pub fator: String,
    /// Quantas peças entram no estoque.
    pub quantidade_pecas: String,
    /// Custo de cada peça, já com frete, IPI e ST.
    pub custo_unitario: String,
    pub monofasico: bool,
    pub peca: Option<PecaLigada>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PecaLigada {
    pub id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub estoque: String,
    pub custo_atual: String,
    pub preco_atual: String,
    /// Preço de revenda depois da compra (se "atualizar preços" estiver marcado).
    pub preco_novo: String,
    /// Variação do custo em % ("+12,5" / "-3").
    pub variacao_custo: String,
    pub custo_subiu: bool,
    /// O custo novo é menor: a tela pergunta se o preço baixa junto.
    pub custo_baixou: bool,
    /// Escolha da tela: baixar o preço pelo % (true) ou manter (false).
    pub baixar_preco: bool,
    /// Preço se baixar pelo % (para mostrar na pergunta).
    pub preco_se_baixar: String,
}

struct ItemBanco {
    id: String,
    ordem: i64,
    codigo_fornecedor: String,
    gtin: String,
    descricao: String,
    ncm: String,
    cest: String,
    cfop: String,
    unidade: String,
    origem: i64,
    cst_pis: String,
    quantidade: i64,
    valor_produtos: i64,
    frete: i64,
    seguro: i64,
    desconto: i64,
    outras: i64,
    ipi: i64,
    icms_st: i64,
    acao: String,
    produto_id: Option<String>,
    achado_por: String,
    fator: i64,
    baixar_preco: bool,
}

impl ItemBanco {
    /// Valor que a loja pagou pelo item, sem o frete pago à parte.
    fn custo_nota(&self) -> Resultado<i64> {
        numeros::soma([
            self.valor_produtos,
            -self.desconto,
            self.frete,
            self.seguro,
            self.outras,
            self.ipi,
            self.icms_st,
        ])
    }
    /// Quantidade de peças (milésimos).
    fn pecas(&self) -> i64 {
        crate::numeros::saturar((self.quantidade as i128 * self.fator as i128 + 500) / 1000)
    }
}

fn itens_banco(conn: &Connection, compra_id: &str) -> Resultado<Vec<ItemBanco>> {
    let mut c = conn.prepare_cached(
        "SELECT id, ordem, codigo_fornecedor, gtin, descricao, ncm, cest, cfop, unidade, origem, cst_pis, quantidade,
                valor_produtos, frete, seguro, desconto, outras, ipi, icms_st, acao, produto_id, achado_por, fator,
                baixar_preco
           FROM compra_item WHERE compra_id = ?1 ORDER BY ordem",
    )?;
    let linhas = c.query_map([compra_id], |l| {
        Ok(ItemBanco {
            id: l.get(0)?,
            ordem: l.get(1)?,
            codigo_fornecedor: l.get(2)?,
            gtin: l.get(3)?,
            descricao: l.get(4)?,
            ncm: l.get(5)?,
            cest: l.get(6)?,
            cfop: l.get(7)?,
            unidade: l.get(8)?,
            origem: l.get(9)?,
            cst_pis: l.get(10)?,
            quantidade: l.get(11)?,
            valor_produtos: l.get(12)?,
            frete: l.get(13)?,
            seguro: l.get(14)?,
            desconto: l.get(15)?,
            outras: l.get(16)?,
            ipi: l.get(17)?,
            icms_st: l.get(18)?,
            acao: l.get(19)?,
            produto_id: l.get(20)?,
            achado_por: l.get(21)?,
            fator: l.get(22)?,
            baixar_preco: l.get(23)?,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Frete pago à parte dividido entre os itens que entram no estoque.
fn frete_extra_por_item(itens: &[ItemBanco], frete_extra: i64) -> Vec<i64> {
    let pesos: Vec<i64> = itens.iter().map(|i| if i.acao == "ignorar" { 0 } else { i.valor_produtos.max(0) }).collect();
    if frete_extra == 0 || pesos.iter().all(|p| *p == 0) {
        return vec![0; itens.len()];
    }
    ratear(frete_extra, &pesos)
}

/// Custo de uma peça (centavos), arredondado.
fn custo_unitario(custo_total: i64, pecas_milesimos: i64) -> i64 {
    if pecas_milesimos <= 0 {
        return 0;
    }
    crate::numeros::saturar((custo_total as i128 * 1000 + pecas_milesimos as i128 / 2) / pecas_milesimos as i128)
}

struct Cabecalho {
    situacao: String,
    fornecedor_id: String,
    frete_extra: i64,
    atualizar_precos: bool,
    numero: String,
}

fn cabecalho(conn: &Connection, id: &str) -> Resultado<Cabecalho> {
    conn.query_row(
        "SELECT situacao, COALESCE(fornecedor_id, ''), frete_extra, atualizar_precos, numero FROM compra WHERE id = ?1",
        [id],
        |l| {
            Ok(Cabecalho {
                situacao: l.get(0)?,
                fornecedor_id: l.get(1)?,
                frete_extra: l.get(2)?,
                atualizar_precos: l.get(3)?,
                numero: l.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Erro::NaoEncontrado("Compra".into()))
}

/// Ficha completa da compra.
pub fn ficha(conn: &Connection, id: &str) -> Resultado<CompraFicha> {
    #[allow(clippy::type_complexity)]
    let c: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        String,
        String,
        Option<String>,
        String,
    ) = conn
        .query_row(
            "SELECT c.situacao, COALESCE(c.fornecedor_id, ''), COALESCE(p.nome, ''), COALESCE(p.documento, ''),
                    COALESCE(p.municipio || CASE WHEN p.uf <> '' THEN ' · ' || p.uf ELSE '' END, ''),
                    c.chave, c.numero, c.serie, c.total_produtos, c.frete, c.ipi, c.icms_st, c.desconto, c.outras,
                    c.total, c.frete_extra, c.atualizar_precos, c.emissao, c.natureza, c.lancada_em,
                    c.destinatario
               FROM compra c LEFT JOIN pessoa p ON p.id = c.fornecedor_id WHERE c.id = ?1",
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
                    l.get(18)?,
                    l.get(19)?,
                    l.get(20)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Compra".into()))?;
    let (
        situacao,
        fornecedor_id,
        fornecedor,
        fdoc,
        cidade,
        chave,
        numero,
        serie,
        tprod,
        frete,
        ipi,
        st,
        desc,
        outras,
        total,
        frete_extra,
        atualizar,
        emissao,
        natureza,
        lancada_em,
        destinatario,
    ) = c;
    let atualizar_precos = atualizar != 0;
    let (observacoes, lancada_por): (String, String) = conn.query_row(
        "SELECT c.observacoes, COALESCE(u.nome, '') FROM compra c LEFT JOIN usuario u ON u.id = c.lancada_por WHERE c.id = ?1",
        [id],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;

    let itens = itens_banco(conn, id)?;
    let extras = frete_extra_por_item(&itens, frete_extra);
    let mut lista = Vec::new();
    let (mut pendentes, mut novas, mut ignorados, mut custo_total) = (0, 0, 0, 0_i64);
    for (i, it) in itens.iter().enumerate() {
        let custo_item = numeros::soma([it.custo_nota()?, extras[i]])?;
        if it.acao != "ignorar" {
            custo_total = numeros::somar([custo_total, custo_item]).ok_or_else(numeros::grande_demais)?;
        }
        match it.acao.as_str() {
            "" => pendentes += 1,
            "nova" => novas += 1,
            "ignorar" => ignorados += 1,
            _ => {}
        }
        let pecas = it.pecas();
        let cu = custo_unitario(custo_item, pecas);
        let peca = match (&it.acao[..], &it.produto_id) {
            ("ligar", Some(pid)) => peca_ligada(conn, pid, cu, atualizar_precos, it.baixar_preco)?,
            _ => None,
        };
        lista.push(ItemFicha {
            id: it.id.clone(),
            ordem: it.ordem + 1,
            codigo_fornecedor: it.codigo_fornecedor.clone(),
            gtin: it.gtin.clone(),
            descricao: it.descricao.clone(),
            ncm: it.ncm.clone(),
            cest: it.cest.clone(),
            cfop: it.cfop.clone(),
            unidade: it.unidade.clone(),
            quantidade: numeros::escrever_quantidade(it.quantidade),
            valor_unitario: numeros::escrever_dinheiro(custo_unitario(it.valor_produtos, it.quantidade)),
            valor_produtos: numeros::escrever_dinheiro(it.valor_produtos),
            despesas: numeros::escrever_dinheiro(custo_item - it.valor_produtos),
            acao: it.acao.clone(),
            achado_por: it.achado_por.clone(),
            fator: numeros::escrever_quantidade(it.fator),
            quantidade_pecas: numeros::escrever_quantidade(pecas),
            custo_unitario: numeros::escrever_dinheiro(cu),
            monofasico: it.cst_pis == "04",
            peca,
        });
    }
    let aviso_destinatario = aviso_cnpj(conn, &destinatario)?;
    Ok(CompraFicha {
        id: id.to_string(),
        situacao,
        fornecedor_id,
        fornecedor,
        fornecedor_documento: documento::escrever(&fdoc),
        fornecedor_cidade: cidade,
        chave,
        numero,
        serie,
        emissao: fiscal::data_br(&emissao),
        natureza,
        total_produtos: numeros::escrever_dinheiro(tprod),
        frete: numeros::escrever_dinheiro(frete),
        ipi: numeros::escrever_dinheiro(ipi),
        icms_st: numeros::escrever_dinheiro(st),
        desconto: numeros::escrever_dinheiro(desc),
        outras: numeros::escrever_dinheiro(outras),
        total: numeros::escrever_dinheiro(total),
        frete_extra: if frete_extra > 0 { numeros::escrever_dinheiro(frete_extra) } else { String::new() },
        atualizar_precos,
        observacoes,
        aviso_destinatario,
        itens: lista,
        pendentes,
        novas,
        ignorados,
        custo_total: numeros::escrever_dinheiro(custo_total),
        lancada_em: lancada_em.unwrap_or_default(),
        lancada_por,
        historico: auditoria::historico(conn, "compra", id, 20)?,
        duplicatas: crate::pagar::duplicatas_da_compra(conn, id)?
            .iter()
            .map(|d| {
                [
                    d.numero.clone(),
                    format!("vence {}", fiscal::data_br(&d.vencimento)),
                    format!("R$ {}", numeros::escrever_dinheiro(d.valor)),
                ]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
            })
            .collect(),
        gerar_pagar: conn.query_row("SELECT gerar_pagar FROM compra WHERE id = ?1", [id], |l| l.get(0))?,
        parcelas_pagar: conn.query_row(
            "SELECT COUNT(*) FROM conta_pagar WHERE compra_id = ?1 AND cancelado_em IS NULL",
            [id],
            |l| l.get(0),
        )?,
    })
}

fn peca_ligada(
    conn: &Connection,
    produto_id: &str,
    custo_novo: i64,
    atualizar_precos: bool,
    baixar_preco: bool,
) -> Resultado<Option<PecaLigada>> {
    let p: Option<(String, String, String, String, i64)> = conn
        .query_row("SELECT codigo, nome, marca, unidade, custo FROM produto WHERE id = ?1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?))
        })
        .optional()?;
    let Some((codigo, nome, marca, unidade, custo)) = p else { return Ok(None) };
    let preco: Option<(i64, Option<i64>)> = conn
        .query_row("SELECT preco, lucro FROM produto_preco WHERE produto_id = ?1 AND tabela = 1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?;
    let preco_atual = preco.map(|p| p.0).unwrap_or(0);
    let custo_baixou = custo > 0 && custo_novo > 0 && custo_novo < custo;
    let pelo_lucro = match preco {
        Some((_, Some(lucro))) if custo_novo > 0 => numeros::preco_por_lucro(custo_novo, lucro),
        _ => preco_atual,
    };
    let preco_novo = if !atualizar_precos || (custo_baixou && !baixar_preco) { preco_atual } else { pelo_lucro };
    let variacao = if custo > 0 && custo_novo > 0 {
        let v = (custo_novo - custo) as f64 * 100.0 / custo as f64;
        let v = (v * 10.0).round() / 10.0;
        if v == 0.0 {
            String::new()
        } else {
            let t = format!("{v:+.1}").replace('.', ",");
            t.trim_end_matches(",0").to_string()
        }
    } else {
        String::new()
    };
    Ok(Some(PecaLigada {
        id: produto_id.to_string(),
        codigo,
        nome,
        marca,
        unidade,
        estoque: numeros::escrever_quantidade(estoque::saldo(conn, produto_id)?),
        custo_atual: if custo > 0 { numeros::escrever_dinheiro(custo) } else { String::new() },
        preco_atual: if preco_atual > 0 { numeros::escrever_dinheiro(preco_atual) } else { String::new() },
        preco_novo: if preco_novo > 0 { numeros::escrever_dinheiro(preco_novo) } else { String::new() },
        custo_subiu: custo > 0 && custo_novo > custo,
        variacao_custo: variacao,
        custo_baixou,
        baixar_preco,
        preco_se_baixar: if pelo_lucro > 0 { numeros::escrever_dinheiro(pelo_lucro) } else { String::new() },
    }))
}

// ---------------------------------------------------------------------------
// Mudar o rascunho.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompraEntrada {
    pub id: String,
    pub frete_extra: String,
    pub atualizar_precos: bool,
    pub observacoes: String,
    pub itens: Vec<ItemEntrada>,
    pub gerar_pagar: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemEntrada {
    pub id: String,
    pub acao: String,
    pub produto_id: String,
    pub fator: String,
    /// Quando o custo baixou: baixar o preço junto?
    pub baixar_preco: bool,
}

/// Grava as escolhas da tela (só em rascunho).
pub fn atualizar(conn: &Connection, e: &CompraEntrada) -> Resultado<()> {
    let cab = cabecalho(conn, &e.id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Esta compra já foi lançada e não pode mais ser mudada."));
    }
    let frete_extra = numeros::ler_dinheiro(&e.frete_extra).map_err(|m| Erro::campo("freteExtra", m))?.unwrap_or(0);
    if frete_extra < 0 {
        return Err(Erro::campo("freteExtra", "O frete não pode ser negativo."));
    }
    let observacoes = e.observacoes.trim().to_string();
    if texto::tamanho(&observacoes) > 1000 {
        return Err(Erro::campo("observacoes", "Use no máximo 1000 caracteres."));
    }
    conn.execute(
        "UPDATE compra SET frete_extra = ?2, atualizar_precos = ?3, observacoes = ?4, gerar_pagar = ?5 WHERE id = ?1",
        params![e.id, frete_extra, e.atualizar_precos, observacoes, e.gerar_pagar],
    )?;
    for (i, it) in e.itens.iter().enumerate() {
        let acao = it.acao.as_str();
        if !["", "ligar", "nova", "ignorar"].contains(&acao) {
            return Err(Erro::Interno(format!("ação desconhecida: {acao}")));
        }
        let fator = numeros::ler_quantidade(&it.fator)
            .map_err(|m| Erro::campo(&format!("itens.{i}.fator"), m))?
            .unwrap_or(1000);
        if fator <= 0 || fator > 10_000_000 {
            return Err(Erro::campo(
                &format!("itens.{i}.fator"),
                "Quantas peças vêm em cada unidade da nota? (1 ou mais)",
            ));
        }
        let produto_id = if acao == "ligar" {
            if it.produto_id.is_empty() {
                return Err(Erro::campo(&format!("itens.{i}.produto"), "Escolha a peça."));
            }
            let existe: bool = conn
                .query_row("SELECT 1 FROM produto WHERE id = ?1", [&it.produto_id], |_| Ok(true))
                .optional()?
                .unwrap_or(false);
            if !existe {
                return Err(Erro::NaoEncontrado("Peça".into()));
            }
            Some(it.produto_id.clone())
        } else {
            None
        };
        let n = conn.execute(
            "UPDATE compra_item SET acao = ?3, produto_id = ?4, fator = ?5, baixar_preco = ?6,
                    achado_por = CASE WHEN ?3 = 'ligar' AND produto_id IS ?4 THEN achado_por ELSE '' END
              WHERE id = ?1 AND compra_id = ?2",
            params![it.id, e.id, acao, produto_id, fator, it.baixar_preco],
        )?;
        if n == 0 {
            return Err(Erro::Interno("item não pertence à compra".into()));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Lançar e estornar.
// ---------------------------------------------------------------------------

/// Resumo do que o lançamento fez.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lancamento {
    pub pecas_novas: i64,
    pub itens_estoque: i64,
    pub precos_atualizados: i64,
    /// Parcelas lançadas no A pagar.
    pub parcelas_pagar: i64,
    /// Encomendas de clientes que chegaram nesta compra.
    pub encomendas: i64,
}

/// Põe a compra no estoque.
pub fn lancar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<Lancamento> {
    let cab = cabecalho(conn, id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Esta compra já foi lançada."));
    }
    let itens = itens_banco(conn, id)?;
    if let Some(p) = itens.iter().find(|i| i.acao.is_empty()) {
        return Err(Erro::validacao(format!(
            "Falta decidir o item {} ({}): ligar a uma peça, cadastrar como nova ou não lançar.",
            p.ordem + 1,
            p.descricao
        )));
    }
    if itens.iter().all(|i| i.acao == "ignorar") {
        return Err(Erro::validacao("Nenhum item vai entrar no estoque."));
    }
    let fornecedor: String = conn
        .query_row("SELECT nome FROM pessoa WHERE id = ?1", [&cab.fornecedor_id], |l| l.get(0))
        .optional()?
        .unwrap_or_default();
    let extras = frete_extra_por_item(&itens, cab.frete_extra);
    let motivo = format!("Compra NF {} · {}", cab.numero, fornecedor);
    let tabelas = config::tabelas_preco(conn)?;
    let mut resumo =
        Lancamento { pecas_novas: 0, itens_estoque: 0, precos_atualizados: 0, parcelas_pagar: 0, encomendas: 0 };
    let mut entraram: Vec<(String, i64)> = Vec::new();

    for (i, it) in itens.iter().enumerate() {
        if it.acao == "ignorar" {
            continue;
        }
        let pecas = it.pecas();
        let custo = custo_unitario(numeros::soma([it.custo_nota()?, extras[i]])?, pecas);
        let mut detalhes = Vec::new();
        let produto_id = if it.acao == "nova" {
            resumo.pecas_novas += 1;
            let pid = cadastrar_peca(conn, dispositivo, usuario_id, it, custo)?;
            conn.execute("UPDATE compra_item SET produto_id = ?2 WHERE id = ?1", params![it.id, pid])?;
            pid
        } else {
            let pid = it.produto_id.clone().ok_or_else(|| Erro::Interno("item ligado sem peça".into()))?;
            let (d, mudou_preco) = atualizar_custo(
                conn,
                dispositivo,
                usuario_id,
                &pid,
                custo,
                cab.atualizar_precos,
                it.baixar_preco,
                &tabelas,
            )?;
            if mudou_preco {
                resumo.precos_atualizados += 1;
            }
            detalhes = d;
            pid
        };
        let antes = estoque::saldo(conn, &produto_id)?;
        estoque::lancar(conn, dispositivo, Some(usuario_id), &produto_id, pecas, "compra", &motivo)?;
        detalhes.insert(
            0,
            format!(
                "Estoque: {} → {} (+{})",
                numeros::escrever_quantidade(antes),
                numeros::escrever_quantidade(antes + pecas),
                numeros::escrever_quantidade(pecas)
            ),
        );
        detalhes.push(motivo.clone());
        auditoria::registrar(conn, dispositivo, Some(usuario_id), "compra.entrada", "produto", &produto_id, &detalhes)?;
        resumo.itens_estoque += 1;
        match entraram.iter_mut().find(|e| e.0 == produto_id) {
            Some(e) => e.1 += pecas,
            None => entraram.push((produto_id.clone(), pecas)),
        }
        let compacto = texto::compactar(&it.codigo_fornecedor);
        if !cab.fornecedor_id.is_empty() && !compacto.is_empty() {
            conn.execute(
                "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (fornecedor_id, codigo_compacto) DO UPDATE
                   SET codigo = excluded.codigo, produto_id = excluded.produto_id, fator = excluded.fator,
                       atualizado_em = excluded.atualizado_em",
                params![cab.fornecedor_id, compacto, it.codigo_fornecedor, produto_id, it.fator, tempo::agora()],
            )?;
        }
    }
    conn.execute(
        "UPDATE compra SET situacao = 'lancada', lancada_em = ?2, lancada_por = ?3 WHERE id = ?1",
        params![id, tempo::agora(), usuario_id],
    )?;
    resumo.parcelas_pagar = crate::pagar::criar_da_compra(conn, dispositivo, usuario_id, id)? as i64;
    resumo.encomendas = crate::encomendas::chegou_na_compra(conn, id, &entraram)?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.lancar",
        "compra",
        id,
        &[format!(
            "{} itens no estoque · {} peças novas · {} preços atualizados",
            resumo.itens_estoque, resumo.pecas_novas, resumo.precos_atualizados
        )],
    )?;
    Ok(resumo)
}

/// Troca o custo da peça e, se pedido, o preço das tabelas que têm % de
/// lucro. Se o custo baixou, o preço só baixa quando `baixar_preco`.
/// Devolve as frases do que mudou e se algum preço mudou.
#[allow(clippy::too_many_arguments)]
fn atualizar_custo(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    produto_id: &str,
    custo: i64,
    atualizar_precos: bool,
    baixar_preco: bool,
    tabelas: &[config::TabelaPreco],
) -> Resultado<(Vec<String>, bool)> {
    let antes: i64 = conn.query_row("SELECT custo FROM produto WHERE id = ?1", [produto_id], |l| l.get(0))?;
    let baixou = antes > 0 && custo < antes;
    let atualizar_precos = atualizar_precos && (!baixou || baixar_preco);
    let mut detalhes = Vec::new();
    if antes != custo {
        detalhes.push(format!(
            "Custo: {} → {}",
            if antes > 0 { numeros::escrever_dinheiro(antes) } else { "—".into() },
            numeros::escrever_dinheiro(custo)
        ));
    }
    let mut mudou_preco = false;
    if atualizar_precos && custo > 0 {
        let precos: Vec<(i64, i64, Option<i64>)> = conn
            .prepare_cached("SELECT tabela, preco, lucro FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela")?
            .query_map([produto_id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (tabela, preco, lucro) in precos {
            let Some(lucro) = lucro else { continue };
            let novo = numeros::preco_por_lucro(custo, lucro);
            if novo != preco {
                conn.execute(
                    "UPDATE produto_preco SET preco = ?3 WHERE produto_id = ?1 AND tabela = ?2",
                    params![produto_id, tabela, novo],
                )?;
                let nome = tabelas.iter().find(|t| t.numero == tabela).map(|t| t.nome.clone()).unwrap_or_default();
                detalhes.push(format!(
                    "Preço {nome}: {} → {}",
                    numeros::escrever_dinheiro(preco),
                    numeros::escrever_dinheiro(novo)
                ));
                mudou_preco = true;
            }
        }
    } else if custo > 0 {
        // Sem atualizar o preço, o % de lucro muda (o preço fica).
        conn.execute(
            "UPDATE produto_preco SET lucro = CASE WHEN ?2 > 0 THEN ((preco - ?2) * 10000 + ?2 / 2) / ?2 ELSE lucro END
              WHERE produto_id = ?1",
            params![produto_id, custo],
        )?;
    }
    if !detalhes.is_empty() {
        conn.execute(
            "UPDATE produto SET custo = ?2, atualizado_em = ?3, atualizado_por = ?4, dispositivo = ?5 WHERE id = ?1",
            params![produto_id, custo, tempo::agora(), usuario_id, dispositivo],
        )?;
    }
    Ok((detalhes, mudou_preco))
}

/// Cadastra uma peça nova com os dados do item da nota.
fn cadastrar_peca(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    it: &ItemBanco,
    custo: i64,
) -> Resultado<String> {
    let mut f = produtos::ficha_nova(conn)?;
    f.nome = texto::limpar(&it.descricao).chars().take(120).collect();
    let barras_livre = !it.gtin.is_empty()
        && gtin::gtin_valido(&it.gtin)
        && conn
            .query_row("SELECT 1 FROM produto WHERE codigo_barras = ?1", [&it.gtin], |_| Ok(()))
            .optional()?
            .is_none();
    if barras_livre {
        f.codigo_barras = it.gtin.clone();
    }
    f.unidade = if it.fator == 1000 { crate::unidades::de_fora(&it.unidade) } else { "UN".into() };
    if it.ncm.len() == 8 && it.ncm.chars().all(|c| c.is_ascii_digit()) {
        f.ncm = it.ncm.clone();
    }
    if it.cest.len() == 7 && it.cest.chars().all(|c| c.is_ascii_digit()) {
        f.cest = it.cest.clone();
    }
    if (0..produtos::ORIGENS.len() as i64).contains(&it.origem) {
        f.origem = it.origem;
    }
    f.monofasico = it.cst_pis == "04";
    f.custo = numeros::escrever_dinheiro(custo);
    if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
        if p.lucro.is_empty() {
            p.lucro = "100".into();
        }
    }
    let id = produtos::salvar(conn, dispositivo, usuario_id, &f)?;
    Ok(id)
}

/// Desfaz a entrada no estoque. O custo e os preços não voltam (podem ter
/// mudado de novo depois).
pub fn estornar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    let cab = cabecalho(conn, id)?;
    if cab.situacao != "lancada" {
        return Err(Erro::validacao("Só dá para estornar uma compra lançada."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo do estorno."));
    }
    crate::pagar::cancelar_da_compra(conn, usuario_id, id)?;
    crate::encomendas::desfazer_da_compra(conn, id)?;
    let itens = itens_banco(conn, id)?;
    for it in &itens {
        if it.acao == "ignorar" {
            continue;
        }
        if let Some(pid) = &it.produto_id {
            let texto_motivo = format!("Estorno da compra NF {} · {motivo}", cab.numero);
            let antes = estoque::saldo(conn, pid)?;
            estoque::lancar(conn, dispositivo, Some(usuario_id), pid, -it.pecas(), "estorno_compra", &texto_motivo)?;
            auditoria::registrar(
                conn,
                dispositivo,
                Some(usuario_id),
                "compra.estornar",
                "produto",
                pid,
                &[
                    format!(
                        "Estoque: {} → {} (−{})",
                        numeros::escrever_quantidade(antes),
                        numeros::escrever_quantidade(antes - it.pecas()),
                        numeros::escrever_quantidade(it.pecas())
                    ),
                    texto_motivo,
                ],
            )?;
        }
    }
    conn.execute("UPDATE compra SET situacao = 'estornada' WHERE id = ?1", [id])?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "compra.estornar",
        "compra",
        id,
        &[format!("Motivo: {motivo}")],
    )?;
    Ok(())
}

/// Apaga uma compra em rascunho (nada foi para o estoque).
pub fn excluir_rascunho(conn: &Connection, id: &str) -> Resultado<()> {
    let cab = cabecalho(conn, id)?;
    if cab.situacao != "rascunho" {
        return Err(Erro::validacao("Só dá para descartar uma compra que ainda não foi lançada."));
    }
    conn.execute("DELETE FROM compra WHERE id = ?1", [id])?;
    conn.execute("DELETE FROM auditoria WHERE entidade = 'compra' AND entidade_id = ?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lista.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompraResumo {
    pub id: String,
    pub fornecedor: String,
    pub numero: String,
    pub emissao: String,
    pub itens: i64,
    pub pendentes: i64,
    pub total: String,
    pub situacao: String,
}

pub fn listar(conn: &Connection, busca: &str, limite: i64) -> Resultado<Vec<CompraResumo>> {
    let termo = format!("%{}%", texto::normalizar(busca));
    let mut c = conn.prepare(
        "SELECT c.id, COALESCE(p.nome, ''), c.numero, c.emissao,
                (SELECT COUNT(*) FROM compra_item i WHERE i.compra_id = c.id),
                (SELECT COUNT(*) FROM compra_item i WHERE i.compra_id = c.id AND i.acao = ''),
                c.total, c.situacao
           FROM compra c LEFT JOIN pessoa p ON p.id = c.fornecedor_id
          WHERE ?1 = '%%' OR lower(COALESCE(p.busca, '')) LIKE ?1 OR c.numero LIKE ?1 OR c.chave LIKE ?1
             OR EXISTS (SELECT 1 FROM compra_item i WHERE i.compra_id = c.id AND normalizar(i.descricao) LIKE ?1)
          ORDER BY CASE c.situacao WHEN 'rascunho' THEN 0 ELSE 1 END, c.emissao DESC, c.criado_em DESC
          LIMIT ?2",
    )?;
    let linhas = c.query_map(params![termo, limite.clamp(1, 1000)], |l| {
        Ok(CompraResumo {
            id: l.get(0)?,
            fornecedor: l.get(1)?,
            numero: l.get(2)?,
            emissao: fiscal::data_br(&l.get::<_, String>(3)?),
            itens: l.get(4)?,
            pendentes: l.get(5)?,
            total: numeros::escrever_dinheiro(l.get(6)?),
            situacao: l.get(7)?,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    /// R-10: pesos enormes no rateio não estouram e não perdem centavo.
    #[test]
    fn rateio_com_pesos_enormes() {
        let partes = ratear(4000, &[i64::MAX, i64::MAX, 1]);
        assert_eq!(partes.iter().sum::<i64>(), 4000);
        let partes = ratear(numeros::SOMA_MAXIMA, &vec![i64::MAX / 2; 1000]);
        assert_eq!(partes.iter().map(|p| *p as i128).sum::<i128>(), numeros::SOMA_MAXIMA as i128);
    }

    #[test]
    fn so_entra_nota_autorizada_de_compra_para_a_loja() {
        let b = Banco::em_memoria().unwrap();
        let ex = nfe_xml::EXEMPLO;
        let ler = |x: &str| nfe_xml::ler(x).unwrap();
        assert_eq!(ler(ex).incoerencia, "");
        assert!(conferir_nota(&b.conn, &ler(ex)).is_ok());
        let sem_protocolo = ex.replace("<cStat>100</cStat>", "<cStat>110</cStat>");
        assert!(conferir_nota(&b.conn, &ler(&sem_protocolo)).unwrap_err().to_string().contains("autorização"));
        let entrada = ex.replace("<tpNF>1</tpNF>", "<tpNF>0</tpNF>");
        assert!(conferir_nota(&b.conn, &ler(&entrada)).unwrap_err().to_string().contains("entrada"));
        let devolucao = ex.replace("<tpNF>1</tpNF>", "<tpNF>1</tpNF><finNFe>4</finNFe>");
        // XML alterado ou montado: o que não bate é recusado (C-04).
        for (troca, por, motivo) in [
            ("<nNF>12345</nNF>", "<nNF>12346</nNF>", "número"),
            ("<CNPJ>11222333000181</CNPJ>", "<CNPJ>11222333000262</CNPJ>", "CNPJ do fornecedor"),
            (
                "<chNFe>35261011222333000181550010000123451000000014",
                "<chNFe>35261011222333000181550010000123461000000010",
                "outra nota",
            ),
            ("<digVal>ZXhl", "<digVal>AAAA", "resumo"),
            ("<nProt>135260000000001</nProt>", "", "sem número"),
            ("1000000014\" versao", "1000000015\" versao", "dígito"),
        ] {
            let x = ex.replace(troca, por);
            assert_ne!(x, ex, "{troca}");
            let e = conferir_nota(&b.conn, &ler(&x)).unwrap_err().to_string();
            assert!(e.contains("alterado") && e.contains(motivo), "{motivo}: {e}");
        }
        let inicio = ex.find("<Signature").unwrap();
        let fim = ex.find("</Signature>").unwrap() + "</Signature>".len();
        let sem_assinatura = format!("{}{}", &ex[..inicio], &ex[fim..]);
        assert!(conferir_nota(&b.conn, &ler(&sem_assinatura)).unwrap_err().to_string().contains("assinatura"));
        assert_eq!(nfe_xml::digito_da_chave("3526101122233300018155001000012345100000001"), Some(4));
        assert!(conferir_nota(&b.conn, &ler(&devolucao)).unwrap_err().to_string().contains("devolução"));
        // CNPJ da loja preenchido e diferente do destinatário.
        config::gravar(&b.conn, "loja_cnpj", "11222333000181").unwrap();
        assert!(conferir_nota(&b.conn, &ler(ex)).unwrap_err().to_string().contains("outro CNPJ"));
        config::gravar(&b.conn, "loja_cnpj", "11444777000161").unwrap();
        assert!(conferir_nota(&b.conn, &ler(ex)).is_ok());
    }

    #[test]
    fn rateio_sem_perder_centavos() {
        assert_eq!(ratear(100, &[1, 1, 1]), vec![34, 33, 33]);
        assert_eq!(ratear(4000, &[52000, 16000, 36000]).iter().sum::<i64>(), 4000);
        assert_eq!(ratear(1000, &[52000, 16000, 36000]), vec![500, 154, 346]);
        assert_eq!(ratear(10, &[0, 0]), vec![5, 5]);
        assert!(ratear(10, &[]).is_empty());
    }

    #[test]
    fn custo_por_peca() {
        assert_eq!(custo_unitario(54050, 10_000), 5405);
        assert_eq!(custo_unitario(1000, 3000), 333);
        assert_eq!(custo_unitario(1000, 0), 0);
    }

    fn base() -> (Banco, String) {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        (b, u.id)
    }

    fn peca(b: &Banco, u: &str, nome: &str, barras: &str, original: &str, custo: &str) -> String {
        let mut f = produtos::ficha_nova(&b.conn).unwrap();
        f.nome = nome.into();
        f.codigo_barras = barras.into();
        f.codigo_original = original.into();
        f.custo = custo.into();
        produtos::salvar(&b.conn, &b.dispositivo, u, &f).unwrap()
    }

    #[test]
    fn importa_liga_lanca_e_estorna() {
        let (b, u) = base();
        let c = &b.conn;
        let rolamento = peca(&b, &u, "Rolamento roda traseira", "7891234567895", "32206", "50,00");
        let pastilha = peca(&b, &u, "Pastilha dianteira Cobreq", "", "N-1100", "38,00");

        let id = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        // a mesma nota de novo é recusada
        let e = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap_err();
        assert!(e.to_string().contains("já foi"), "{e}");

        let f = ficha(c, &id).unwrap();
        assert_eq!(f.situacao, "rascunho");
        assert_eq!(f.fornecedor, "DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA");
        assert_eq!(f.itens.len(), 3);
        assert_eq!(f.itens[0].acao, "ligar");
        assert_eq!(f.itens[0].achado_por, "barras");
        assert_eq!(f.itens[1].acao, "ligar");
        assert_eq!(f.itens[1].achado_por, "codigo");
        assert_eq!(f.itens[2].acao, "");
        assert_eq!(f.pendentes, 1);
        // rolamento: (520 + 20 frete + 20,50 IPI) / 10 = 56,05
        assert_eq!(f.itens[0].custo_unitario, "56,05");
        assert_eq!(f.itens[0].peca.as_ref().unwrap().preco_novo, "112,10");
        assert!(f.itens[0].peca.as_ref().unwrap().custo_subiu);
        assert!(f.itens[2].monofasico);

        // não lança com item pendente
        assert!(lancar(c, &b.dispositivo, &u, &id).is_err());

        // óleo vira peça nova; caixa da pastilha vem com 2 jogos; frete à parte de R$ 10,40
        atualizar(
            c,
            &CompraEntrada {
                id: id.clone(),
                frete_extra: "10,40".into(),
                atualizar_precos: true,
                observacoes: String::new(),
                gerar_pagar: true,
                itens: vec![
                    ItemEntrada {
                        id: f.itens[0].id.clone(),
                        acao: "ligar".into(),
                        produto_id: rolamento.clone(),
                        fator: "1".into(),
                        baixar_preco: false,
                    },
                    ItemEntrada {
                        id: f.itens[1].id.clone(),
                        acao: "ligar".into(),
                        produto_id: pastilha.clone(),
                        fator: "2".into(),
                        baixar_preco: false,
                    },
                    ItemEntrada {
                        id: f.itens[2].id.clone(),
                        acao: "nova".into(),
                        produto_id: String::new(),
                        fator: "1".into(),
                        baixar_preco: false,
                    },
                ],
            },
        )
        .unwrap();
        let f = ficha(c, &id).unwrap();
        assert_eq!(f.itens[1].quantidade_pecas, "8");
        // frete extra 10,40 rateado: 520/1040 → 5,20 ; 160/1040 → 1,60 ; 360/1040 → 3,60
        assert_eq!(f.itens[0].custo_unitario, "56,57");
        assert_eq!(f.itens[1].custo_unitario, "21,45");
        assert_eq!(f.custo_total, "1.110,90");

        assert_eq!(f.duplicatas, ["001 · vence 07/11/2026 · R$ 550,25", "002 · vence 07/12/2026 · R$ 550,25"]);
        assert!(f.gerar_pagar);
        let r = lancar(c, &b.dispositivo, &u, &id).unwrap();
        assert_eq!((r.itens_estoque, r.pecas_novas), (3, 1));
        // as duplicatas foram para o A pagar
        assert_eq!(r.parcelas_pagar, 2);
        let a_pagar = crate::pagar::painel(c, "12345", "abertas").unwrap();
        assert_eq!(a_pagar.parcelas.len(), 2);
        assert_eq!(a_pagar.parcelas[0].documento, "12345/001");
        assert_eq!(a_pagar.parcelas[0].categoria, "Fornecedores (peças)");
        assert_eq!(a_pagar.parcelas[1].parcela, "2/2");
        // com parcela paga, não estorna a compra
        let pago = crate::pagar::pagar(
            c,
            &b.dispositivo,
            &u,
            &crate::pagar::PagamentoEntrada {
                conta_id: a_pagar.parcelas[0].id.clone(),
                forma: "boleto".into(),
                origem: "caixa-geral".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(estornar(c, &b.dispositivo, &u, &id, "Nota devolvida")
            .unwrap_err()
            .to_string()
            .contains("parcela paga"));
        crate::pagar::estornar(c, &b.dispositivo, &u, &pago, "engano").unwrap();
        assert_eq!(estoque::saldo(c, &rolamento).unwrap(), 10_000);
        assert_eq!(estoque::saldo(c, &pastilha).unwrap(), 8_000);
        let ficha_rol = produtos::ficha(c, &rolamento).unwrap();
        assert_eq!(ficha_rol.custo, "56,57");
        assert_eq!(ficha_rol.precos[0].preco, "113,14");
        // a pastilha ficou mais barata (38,00 → 21,45) e o preço foi mantido
        let ficha_pas = produtos::ficha(c, &pastilha).unwrap();
        assert_eq!(ficha_pas.custo, "21,45");
        assert_eq!(ficha_pas.precos[0].preco, "76,00");
        assert!(ficha_rol.historico[0].detalhes.iter().any(|d| d.contains("Compra NF 12345")));
        assert!(ficha_rol.historico[0].detalhes[0].starts_with("Estoque: 0 → 10"));
        assert_eq!(ficha_rol.fornecedores.len(), 1);

        // peça nova do óleo: monofásica, NCM e CEST da nota, custo e preço pelo lucro padrão (100%)
        let oleo_id: String = c
            .query_row("SELECT produto_id FROM compra_item WHERE compra_id = ?1 AND ordem = 2", [&id], |l| l.get(0))
            .unwrap();
        let oleo = produtos::ficha(c, &oleo_id).unwrap();
        assert!(oleo.monofasico);
        assert_eq!(oleo.ncm, "27101932");
        // (360 + 10 frete + 3,60 do frete à parte) / 12 = 31,13
        assert_eq!(oleo.custo, "31,13");
        assert_eq!(oleo.precos[0].preco, "62,26");
        assert_eq!(oleo.estoque, "12");

        // a próxima nota do mesmo fornecedor liga sozinha pelo código dele
        estornar(c, &b.dispositivo, &u, &id, "Nota devolvida").unwrap();
        assert_eq!(estoque::saldo(c, &pastilha).unwrap(), 0);
        assert!(crate::pagar::painel(c, "12345", "abertas").unwrap().parcelas.is_empty(), "parcelas saíram");
        let id2 = importar_xml(c, &b.dispositivo, &u, nfe_xml::EXEMPLO).unwrap();
        let f2 = ficha(c, &id2).unwrap();
        assert!(f2.itens.iter().all(|i| i.acao == "ligar" && i.achado_por == "fornecedor"));
        assert_eq!(f2.itens[1].fator, "2");
        assert_eq!(listar(c, "", 50).unwrap().len(), 2);
        assert_eq!(listar(c, "pastilha", 50).unwrap().len(), 2);
        excluir_rascunho(c, &id2).unwrap();
        assert_eq!(listar(c, "", 50).unwrap().len(), 1);
    }

    #[test]
    fn aviso_de_cnpj_de_outra_empresa() {
        let (b, _) = base();
        assert!(aviso_cnpj(&b.conn, "11.222.333/0001-81").unwrap().starts_with("Preencha"));
        config::gravar(&b.conn, "loja_cnpj", "11222333000181").unwrap();
        config::gravar(&b.conn, "loja_nome", "LOJA MODELO").unwrap();
        assert_eq!(aviso_cnpj(&b.conn, "11.222.333/0001-81").unwrap(), "");
        let a = aviso_cnpj(&b.conn, "99888777000100").unwrap();
        assert!(a.contains("não é da LOJA MODELO") && !a.contains("9988"), "{a}");
        assert_eq!(aviso_cnpj(&b.conn, "").unwrap(), "");
    }
}
