//! Impressos em folha A4: comprovante de venda, orçamento e fechamento de
//! caixa. Aqui só se monta o HTML; a janela (Qt) transforma em PDF.
//!
//! O comprovante NÃO é documento fiscal (a nota vem na etapa 5) e diz isso.

use rusqlite::{Connection, OptionalExtension};

use crate::erro::Resultado;
use crate::{caixa, config, documento, numeros, receber, vendas};

/// Troca &, <, > e aspas para o texto não quebrar o HTML.
pub fn escapar(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn cabecalho_loja(conn: &Connection) -> Resultado<String> {
    let nome = config::nome_loja(conn)?;
    let cnpj = config::ler(conn, "loja_cnpj")?.unwrap_or_default();
    let endereco = config::ler(conn, "loja_endereco")?.unwrap_or_default();
    let telefone = config::ler(conn, "loja_telefone")?.unwrap_or_default();
    let mut linha2 = Vec::new();
    if !cnpj.is_empty() {
        linha2.push(format!("CNPJ {}", documento::escrever(&cnpj)));
    }
    if !endereco.is_empty() {
        linha2.push(escapar(&endereco));
    }
    if !telefone.is_empty() {
        linha2.push(escapar(&telefone));
    }
    Ok(format!(
        "<p style=\"font-family:'Cormorant Garamond Ajustada'; font-size:26pt; font-weight:600; margin:0\">{}</p>
         <p style=\"font-size:9pt; color:#5C534A; margin:0 0 10pt 0\">{}</p>",
        escapar(if nome.is_empty() { "Rustinho" } else { &nome }),
        linha2.join(" · ")
    ))
}

const ESTILO: &str = "<style>
 body { font-family: 'Source Sans 3'; font-size: 10pt; color: #2A2622; }
 h1 { font-family: 'Cormorant Garamond Ajustada'; font-size: 20pt; font-weight: 600; margin: 4pt 0 2pt 0; }
 table.itens { border-collapse: collapse; }
 table.itens th { font-size: 8pt; color: #857B70; text-align: left; border-bottom: 1px solid #A3927E; padding: 3pt 4pt; }
 table.itens td { padding: 4pt 4pt; border-bottom: 1px solid #E6DAC2; }
 .num { text-align: right; }
 .nota { font-size: 8.5pt; color: #5C534A; font-style: italic; }
 .total { font-size: 14pt; font-weight: 700; }
</style>";

fn data_hora(conn: &Connection, iso: &str) -> Resultado<String> {
    Ok(conn
        .query_row("SELECT strftime('%d/%m/%Y às %H:%M', ?1, 'localtime')", [iso], |l| l.get::<_, Option<String>>(0))?
        .unwrap_or_default())
}

/// Itens da venda. Com `locais`, mostra a prateleira de cada peça (ajuda a
/// separar no balcão).
fn tabela_itens(conn: &Connection, itens: &[vendas::ItemFicha], com_local: bool) -> Resultado<String> {
    let local_de = |produto_id: &str| -> Resultado<String> {
        if !com_local || produto_id.is_empty() {
            return Ok(String::new());
        }
        Ok(conn
            .query_row("SELECT local FROM produto WHERE id = ?1", [produto_id], |l| l.get::<_, String>(0))
            .optional()?
            .unwrap_or_default())
    };
    let mut locais = Vec::with_capacity(itens.len());
    for it in itens {
        locais.push(local_de(&it.produto_id)?);
    }
    let mostrar_local = locais.iter().any(|l| !l.is_empty());
    let mut h =
        String::from("<table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr><th>CÓDIGO</th><th>DESCRIÇÃO</th>");
    if mostrar_local {
        h.push_str("<th>LOCAL</th>");
    }
    h.push_str("<th class=\"num\">QTD.</th><th class=\"num\">PREÇO</th><th class=\"num\">DESCONTO</th><th class=\"num\">TOTAL</th></tr>");
    for (it, local) in itens.iter().zip(&locais) {
        h.push_str(&format!("<tr><td>{}</td><td>{}</td>", escapar(&it.codigo), escapar(&it.descricao)));
        if mostrar_local {
            h.push_str(&format!("<td>{}</td>", escapar(local)));
        }
        h.push_str(&format!(
            "<td class=\"num\">{} {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\"><b>{}</b></td></tr>",
            it.quantidade,
            escapar(&it.unidade),
            it.preco,
            if it.desconto.is_empty() { "—".into() } else { it.desconto.clone() },
            it.total
        ));
    }
    h.push_str("</table>");
    Ok(h)
}

/// Texto do pé do impresso (configurações), se houver.
fn rodape(conn: &Connection, chave: &str) -> Resultado<String> {
    let t = config::ler(conn, chave)?.unwrap_or_default();
    if t.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(format!("<p style=\"margin-top:14pt\">{}</p>", escapar(t.trim()).replace('\n', "<br/>")))
}

fn totais(v: &vendas::VendaFicha, pagamentos: bool) -> String {
    let mut linhas = vec![("Subtotal".to_string(), format!("R$ {}", v.subtotal))];
    if v.desconto != "0,00" {
        linhas.push(("Desconto".into(), format!("− R$ {}", v.desconto)));
    }
    let mut h = String::from("<table width=\"100%\" cellspacing=\"0\" style=\"margin-top:8pt\"><tr><td width=\"55%\"></td><td><table width=\"100%\">");
    for (r, val) in linhas {
        h.push_str(&format!("<tr><td>{r}</td><td class=\"num\">{val}</td></tr>"));
    }
    h.push_str(&format!("<tr><td class=\"total\">Total</td><td class=\"num total\">R$ {}</td></tr>", v.total));
    if pagamentos {
        for p in &v.pagamentos {
            h.push_str(&format!("<tr><td>{}</td><td class=\"num\">R$ {}</td></tr>", p.nome, p.valor));
        }
        if v.troco != "0,00" {
            h.push_str(&format!("<tr><td>Troco</td><td class=\"num\">R$ {}</td></tr>", v.troco));
        }
    }
    h.push_str("</table></td></tr></table>");
    h
}

/// Comprovante de venda (não fiscal).
pub fn venda(conn: &Connection, id: &str) -> Resultado<String> {
    let v = vendas::ficha(conn, id)?;
    let quando = data_hora(conn, &v.criado_em)?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    h.push_str(&format!(
        "<h1>Venda {} {}</h1><p>{} · {} · vendedor: {}</p>",
        v.numero,
        if v.situacao == "cancelada" { "— CANCELADA" } else { "" },
        quando,
        escapar(&v.caixa),
        escapar(&v.vendedor)
    ));
    if !v.cliente.is_empty() {
        h.push_str(&format!(
            "<p style=\"margin-bottom:8pt\">Cliente: <b>{}</b>{}</p>",
            escapar(&v.cliente),
            if v.cliente_documento.is_empty() { String::new() } else { format!(" · {}", v.cliente_documento) }
        ));
    }
    h.push_str(&tabela_itens(conn, &v.itens, config::ligado(conn, "imprimir_local")?)?);
    h.push_str(&totais(&v, true));
    if !v.observacoes.is_empty() {
        h.push_str(&format!("<p>Observações: {}</p>", escapar(&v.observacoes)));
    }
    if !v.cheques.is_empty() {
        h.push_str("<p style=\"margin-top:12pt\"><b>Cheques</b></p>");
        for c in &v.cheques {
            h.push_str(&format!("<p>{}</p>", escapar(c)));
        }
    }
    if !v.parcelas.is_empty() {
        h.push_str(&tabela_parcelas(&v.parcelas));
        h.push_str(&format!(
            "<p style=\"margin-top:28pt\">Reconheço a compra acima e pagarei as parcelas nos vencimentos.</p>
             <p style=\"margin-top:28pt\">______________________________<br/>{}</p>",
            escapar(&v.cliente)
        ));
    }
    h.push_str(&rodape(conn, "rodape_venda")?);
    h.push_str(
        "<p class=\"nota\" style=\"margin-top:18pt\">Comprovante de venda. Não é documento fiscal.</p></body></html>",
    );
    Ok(h)
}

fn tabela_parcelas(parcelas: &[receber::Parcela]) -> String {
    let mut h = String::from(
        "<p style=\"margin-top:12pt\"><b>Crediário</b></p><table class=\"itens\" width=\"70%\" cellspacing=\"0\"><tr>
           <th>PARCELA</th><th>VENCIMENTO</th><th class=\"num\">VALOR</th><th class=\"num\">PAGO</th><th class=\"num\">FALTA</th></tr>",
    );
    for p in parcelas {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\"><b>{}</b></td></tr>",
            p.parcela,
            p.vencimento,
            p.valor,
            if p.pago == "0,00" { "—".to_string() } else { p.pago.clone() },
            if p.situacao == "cancelada" { "cancelada".to_string() } else { p.saldo.clone() }
        ));
    }
    h.push_str("</table>");
    h
}

/// Recibo de pagamento do crediário.
pub fn recibo(conn: &Connection, id: &str) -> Resultado<String> {
    let r = receber::recibo(conn, id)?;
    let quando = data_hora(conn, &r.criado_em)?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    h.push_str(&format!(
        "<h1>Recibo {}{}</h1><p>{} · {} · recebido por {}</p>",
        r.numero,
        if r.estornado { " — ESTORNADO" } else { "" },
        quando,
        escapar(&r.caixa),
        escapar(&r.usuario)
    ));
    h.push_str(&format!(
        "<p style=\"margin-bottom:8pt\">Recebemos de <b>{}</b>{} a quantia de <b>R$ {}</b> ({}), referente a:</p>",
        escapar(&r.cliente),
        if r.cliente_documento.is_empty() { String::new() } else { format!(" ({})", r.cliente_documento) },
        r.valor,
        escapar(&r.forma)
    ));
    h.push_str(
        "<table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr><th>REFERENTE A</th><th>PARCELA</th><th>VENCIMENTO</th>
           <th class=\"num\">PAGO AGORA</th><th class=\"num\">AINDA FALTA</th></tr>",
    );
    for p in &r.partes {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            escapar(&p.descricao),
            p.parcela,
            p.vencimento,
            p.valor,
            if p.resta == "0,00" { "quitada".to_string() } else { p.resta.clone() }
        ));
    }
    h.push_str(
        "</table><table width=\"100%\" style=\"margin-top:8pt\"><tr><td width=\"55%\"></td><td><table width=\"100%\">",
    );
    if r.juros != "0,00" {
        h.push_str(&format!("<tr><td>Juros e multa</td><td class=\"num\">R$ {}</td></tr>", r.juros));
    }
    if r.desconto != "0,00" {
        h.push_str(&format!("<tr><td>Desconto</td><td class=\"num\">− R$ {}</td></tr>", r.desconto));
    }
    h.push_str(&format!("<tr><td class=\"total\">Pago</td><td class=\"num total\">R$ {}</td></tr>", r.valor));
    if r.troco != "0,00" {
        h.push_str(&format!("<tr><td>Troco</td><td class=\"num\">R$ {}</td></tr>", r.troco));
    }
    h.push_str(&format!(
        "<tr><td>Ainda deve (total)</td><td class=\"num\">R$ {}</td></tr></table></td></tr></table>",
        r.em_aberto
    ));
    h.push_str("<p style=\"margin-top:40pt\">______________________________<br/>Assinatura da loja</p></body></html>");
    Ok(h)
}

/// Extrato do crediário de um cliente (o que está em aberto).
pub fn extrato(conn: &Connection, cliente_id: &str) -> Resultado<String> {
    let c = receber::cliente(conn, cliente_id, false)?;
    let hoje: String = conn.query_row("SELECT strftime('%d/%m/%Y', 'now', 'localtime')", [], |l| l.get(0))?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    h.push_str(&format!(
        "<h1>Extrato do crediário</h1><p><b>{}</b>{} · em {}</p>",
        escapar(&c.nome),
        if c.documento.is_empty() { String::new() } else { format!(" · {}", c.documento) },
        hoje
    ));
    if c.parcelas.is_empty() {
        h.push_str("<p>Nada em aberto. Obrigado!</p>");
    } else {
        h.push_str(
            "<table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr><th>REFERENTE A</th><th>PARCELA</th><th>VENCIMENTO</th>
               <th class=\"num\">VALOR</th><th class=\"num\">PAGO</th><th class=\"num\">JUROS</th><th class=\"num\">FALTA</th></tr>",
        );
        for p in &c.parcelas {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\"><b>{}</b></td></tr>",
                escapar(&p.descricao),
                p.parcela,
                p.vencimento,
                if p.dias_atraso > 0 { format!(" <i>({} dias)</i>", p.dias_atraso) } else { String::new() },
                p.valor,
                if p.pago == "0,00" { "—".to_string() } else { p.pago.clone() },
                if p.juros.is_empty() { "—".to_string() } else { p.juros.clone() },
                p.saldo
            ));
        }
        h.push_str("</table>");
        h.push_str(&format!(
            "<table width=\"100%\" style=\"margin-top:8pt\"><tr><td width=\"55%\"></td><td><table width=\"100%\">
               <tr><td>Vencido</td><td class=\"num\">R$ {}</td></tr>
               <tr><td class=\"total\">Total em aberto</td><td class=\"num total\">R$ {}</td></tr></table></td></tr></table>",
            c.situacao.vencido, c.situacao.em_aberto
        ));
        h.push_str("<p class=\"nota\" style=\"margin-top:18pt\">Juros e multa calculados até hoje.</p>");
    }
    h.push_str("</body></html>");
    Ok(h)
}

/// Orçamento para entregar ao cliente.
pub fn orcamento(conn: &Connection, id: &str) -> Resultado<String> {
    let v = vendas::ficha(conn, id)?;
    let quando = data_hora(conn, &v.criado_em)?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    h.push_str(&format!(
        "<h1>Orçamento {}</h1><p>{} · atendido por {} · <b>válido até {}</b></p>",
        v.numero,
        quando,
        escapar(&v.vendedor),
        v.validade
    ));
    if !v.cliente.is_empty() {
        h.push_str(&format!("<p style=\"margin-bottom:8pt\">Para: <b>{}</b></p>", escapar(&v.cliente)));
    }
    h.push_str(&tabela_itens(conn, &v.itens, false)?);
    h.push_str(&totais(&v, false));
    if !v.observacoes.is_empty() {
        h.push_str(&format!("<p>Observações: {}</p>", escapar(&v.observacoes)));
    }
    h.push_str(&rodape(conn, "rodape_orcamento")?);
    h.push_str("<p class=\"nota\" style=\"margin-top:18pt\">Preços válidos até a data acima, sujeitos à disponibilidade em estoque.</p></body></html>");
    Ok(h)
}

/// Fechamento (ou parcial) do caixa.
pub fn caixa(conn: &Connection, sessao_id: &str) -> Resultado<String> {
    let r = caixa::resumo(conn, sessao_id)?;
    let aberto = data_hora(conn, &r.sessao.aberto_em)?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    let titulo = if r.sessao.fechado_em.is_some() { "Fechamento de caixa" } else { "Caixa (parcial)" };
    h.push_str(&format!(
        "<h1>{titulo} · {}</h1><p>Aberto em {} por {}</p>",
        escapar(&r.sessao.caixa_nome),
        aberto,
        escapar(&r.sessao.aberto_por)
    ));
    if let Some(f) = &r.sessao.fechado_em {
        h.push_str(&format!("<p>Fechado em {} por {}</p>", data_hora(conn, f)?, escapar(&r.fechado_por)));
    }
    h.push_str("<table class=\"itens\" width=\"60%\" cellspacing=\"0\">");
    let mut linha = |a: &str, b: &str| h.push_str(&format!("<tr><td>{a}</td><td class=\"num\">{b}</td></tr>"));
    linha("Troco inicial", &format!("R$ {}", r.valor_inicial));
    linha("Vendas", &format!("{} (R$ {})", r.vendas, r.total_vendido));
    for f in &r.por_forma {
        linha(&format!("&nbsp;&nbsp;{}", f.nome), &format!("R$ {}", f.valor));
    }
    if r.recebimentos > 0 {
        linha("Recebido do crediário", &format!("{} (R$ {})", r.recebimentos, r.total_recebido));
        for f in &r.recebido_por_forma {
            linha(&format!("&nbsp;&nbsp;{}", f.nome), &format!("R$ {}", f.valor));
        }
    }
    linha("Suprimentos", &format!("R$ {}", r.suprimentos));
    linha("Sangrias", &format!("− R$ {}", r.sangrias));
    linha("<b>Dinheiro esperado na gaveta</b>", &format!("<b>R$ {}</b>", r.esperado));
    if !r.contado.is_empty() {
        linha("Contado", &format!("R$ {}", r.contado));
        let dif = match r.diferenca_centavos {
            0 => "bateu".to_string(),
            d if d > 0 => format!("sobraram R$ {}", r.diferenca),
            _ => format!("faltaram R$ {}", r.diferenca.trim_start_matches('-')),
        };
        linha("<b>Diferença</b>", &format!("<b>{dif}</b>"));
    }
    linha("Vendas canceladas", &r.canceladas.to_string());
    h.push_str("</table>");
    if !r.movimentos.is_empty() {
        h.push_str("<p style=\"margin-top:10pt\"><b>Sangrias e suprimentos</b></p><table class=\"itens\" width=\"100%\" cellspacing=\"0\">");
        for m in &r.movimentos {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">R$ {}</td></tr>",
                data_hora(conn, &m.quando)?,
                if m.tipo == "sangria" { "Sangria" } else { "Suprimento" },
                escapar(&m.motivo),
                m.valor
            ));
        }
        h.push_str("</table>");
    }
    h.push_str("<p style=\"margin-top:40pt\">______________________________<br/>Assinatura</p></body></html>");
    Ok(h)
}

/// Lista de compra (sugestão), separada por fornecedor.
pub fn lista_compra(conn: &Connection) -> Resultado<String> {
    let quando =
        conn.query_row("SELECT strftime('%d/%m/%Y às %H:%M', 'now', 'localtime')", [], |l| l.get::<_, String>(0))?;
    Ok(format!(
        "<html><head>{ESTILO}</head><body>{}{}<p class=\"nota\" style=\"margin-top:14pt\">Feita em {quando}. Confira o estoque antes de pedir.</p></body></html>",
        cabecalho_loja(conn)?,
        crate::reposicao::lista_compra_html(conn)?
    ))
}

fn caixa_vale(conn: &Connection, vale_id: &str) -> Resultado<String> {
    let v: (i64, String, String, i64, i64, String) = conn.query_row(
        "SELECT numero, caixa_nome, cliente_nome, valor, saldo, criado_em FROM vale_saldo WHERE id = ?1",
        [vale_id],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)),
    )?;
    let quando = data_hora(conn, &v.5)?;
    let saldo = if v.4 != v.3 {
        format!(
            "<p style=\"margin:2pt 0\">Valor original R$ {} · <b>ainda tem R$ {}</b></p>",
            numeros::escrever_dinheiro(v.3),
            numeros::escrever_dinheiro(v.4)
        )
    } else {
        String::new()
    };
    Ok(format!(
        "<table width=\"100%\" cellspacing=\"0\" cellpadding=\"12\" style=\"margin-top:14pt; background-color:#DFE4D6\"><tr><td>
           <p style=\"font-family:'Cormorant Garamond Ajustada'; font-size:22pt; font-weight:600; margin:0\">Vale-troca nº {} · R$ {}</p>
           {saldo}
           <p style=\"margin:2pt 0\">{}{} · emitido em {}</p>
           <p class=\"nota\" style=\"margin:4pt 0 0 0\">Vale para qualquer compra na loja, de uma vez ou aos poucos. Traga este papel ou diga o número no caixa.</p>
         </td></tr></table>",
        v.0,
        numeros::escrever_dinheiro(v.4),
        escapar(&v.1),
        if v.2.is_empty() { String::new() } else { format!(" · {}", escapar(&v.2)) },
        quando
    ))
}

/// Comprovante de devolução (com o vale-troca, se houver).
pub fn devolucao(conn: &Connection, id: &str) -> Resultado<String> {
    #[allow(clippy::type_complexity)]
    let d: (i64, String, String, String, String, i64, i64, i64, i64, i64, Option<String>, String, Option<String>, String) = conn.query_row(
        "SELECT d.numero, d.caixa_nome, d.criado_em, d.cliente_nome, d.motivo, d.valor, d.dinheiro, d.pix, d.vale, d.crediario,
                d.vale_id, COALESCE(u.nome, ''), d.cancelado_em, d.venda_id
           FROM devolucao d LEFT JOIN usuario u ON u.id = d.usuario_id WHERE d.id = ?1",
        [id],
        |l| {
            Ok((
                l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?, l.get(7)?, l.get(8)?, l.get(9)?,
                l.get(10)?, l.get(11)?, l.get(12)?, l.get(13)?,
            ))
        },
    )?;
    let (venda_numero, venda_caixa, venda_quando): (i64, String, String) =
        conn.query_row("SELECT numero, caixa_nome, criado_em FROM venda WHERE id = ?1", [&d.13], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?))
        })?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    h.push_str(&format!(
        "<h1>Devolução {}{}</h1><p>{} · {} · atendido por {}</p>
         <p style=\"margin-bottom:8pt\">Da venda {} ({}) de {}{}</p>",
        d.0,
        if d.12.is_some() { " — DESFEITA" } else { "" },
        data_hora(conn, &d.2)?,
        escapar(&d.1),
        escapar(&d.11),
        venda_numero,
        escapar(&venda_caixa),
        data_hora(conn, &venda_quando)?,
        if d.3.is_empty() { String::new() } else { format!(" · cliente: <b>{}</b>", escapar(&d.3)) }
    ));
    h.push_str(
        "<table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr><th>PEÇA</th><th class=\"num\">QTD.</th>
           <th class=\"num\">VALOR</th><th>OBSERVAÇÃO</th></tr>",
    );
    let mut c = conn.prepare(
        "SELECT di.descricao, di.quantidade, di.unidade, di.valor, di.volta_estoque, COALESCE(vi.codigo, '')
           FROM devolucao_item di LEFT JOIN venda_item vi ON vi.id = di.venda_item_id WHERE di.devolucao_id = ?1",
    )?;
    let itens: Vec<(String, i64, String, i64, bool, String)> = c
        .query_map([id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)))?
        .collect::<Result<_, _>>()?;
    for it in itens {
        h.push_str(&format!(
            "<tr><td>{}{}</td><td class=\"num\">{} {}</td><td class=\"num\"><b>{}</b></td><td>{}</td></tr>",
            if it.5.is_empty() { String::new() } else { format!("{} · ", escapar(&it.5)) },
            escapar(&it.0),
            numeros::escrever_quantidade(it.1),
            escapar(&it.2),
            numeros::escrever_dinheiro(it.3),
            if it.4 { "" } else { "com defeito" }
        ));
    }
    h.push_str("</table>");
    h.push_str("<table width=\"100%\" cellspacing=\"0\" style=\"margin-top:8pt\"><tr><td width=\"55%\"></td><td><table width=\"100%\">");
    h.push_str(&format!(
        "<tr><td class=\"total\">Devolvido</td><td class=\"num total\">R$ {}</td></tr>",
        numeros::escrever_dinheiro(d.5)
    ));
    for (nome, v) in [("Abateu do crediário", d.9), ("Vale-troca", d.8), ("Em dinheiro", d.6), ("Pix ou cartão", d.7)]
    {
        if v > 0 {
            h.push_str(&format!("<tr><td>{nome}</td><td class=\"num\">R$ {}</td></tr>", numeros::escrever_dinheiro(v)));
        }
    }
    h.push_str("</table></td></tr></table>");
    h.push_str(&format!("<p>Motivo: {}</p>", escapar(&d.4)));
    if let Some(vale) = &d.10 {
        h.push_str(&caixa_vale(conn, vale)?);
    }
    if d.6 + d.7 > 0 {
        h.push_str(&format!(
            "<p style=\"margin-top:30pt\">Recebi a quantia acima.</p>
             <p style=\"margin-top:26pt\">______________________________<br/>{}</p>",
            if d.3.is_empty() { "Cliente".to_string() } else { escapar(&d.3) }
        ));
    }
    h.push_str("<p class=\"nota\" style=\"margin-top:18pt\">Comprovante de devolução. Não é documento fiscal.</p></body></html>");
    Ok(h)
}

/// Só o vale-troca (para imprimir de novo).
pub fn vale(conn: &Connection, id: &str) -> Resultado<String> {
    Ok(format!("<html><head>{ESTILO}</head><body>{}{}</body></html>", cabecalho_loja(conn)?, caixa_vale(conn, id)?))
}

/// Ordem de serviço (ou orçamento de serviço), com lugar para o cliente
/// assinar.
pub fn ordem_servico(conn: &Connection, id: &str) -> Resultado<String> {
    let o = crate::oficina::ficha(conn, id)?;
    let mut h = format!("<html><head>{ESTILO}</head><body>{}", cabecalho_loja(conn)?);
    let titulo = if o.situacao == "orcamento" { "Orçamento de serviço" } else { "Ordem de serviço" };
    h.push_str(&format!(
        "<h1>{titulo} {}{}</h1><p>Aberta em {} · {}{}{}</p>",
        o.numero,
        if o.situacao == "cancelada" { " — CANCELADA" } else { "" },
        o.criado_em,
        escapar(&o.caixa),
        if o.mecanico.is_empty() { String::new() } else { format!(" · mecânico: {}", escapar(&o.mecanico)) },
        if o.previsao.is_empty() { String::new() } else { format!(" · <b>previsão: {}</b>", o.previsao) }
    ));
    h.push_str(&format!(
        "<table width=\"100%\" cellspacing=\"0\" cellpadding=\"6\" style=\"margin:6pt 0 8pt 0; background-color:#EFE5D1\"><tr>
           <td width=\"50%\"><span class=\"nota\">Cliente</span><br/><b>{}</b>{}</td>
           <td><span class=\"nota\">Carro</span><br/><b>{}</b>{}{}</td></tr></table>",
        escapar(&o.cliente),
        if o.telefone.is_empty() { String::new() } else { format!(" · {}", escapar(&o.telefone)) },
        escapar(&documento::escrever_placa(&o.placa)),
        if o.veiculo.is_empty() { String::new() } else { format!(" · {}", escapar(&o.veiculo)) },
        if o.km.is_empty() { String::new() } else { format!(" · {} km", escapar(&o.km)) }
    ));
    if !o.defeito.is_empty() {
        h.push_str(&format!("<p><b>O cliente relatou:</b> {}</p>", escapar(&o.defeito).replace('\n', "<br/>")));
    }
    if !o.diagnostico.is_empty() {
        h.push_str(&format!("<p><b>Diagnóstico:</b> {}</p>", escapar(&o.diagnostico).replace('\n', "<br/>")));
    }
    let pecas: Vec<_> = o.calculo.itens.iter().filter(|i| i.tipo == "peca").collect();
    let servicos: Vec<_> = o.calculo.itens.iter().filter(|i| i.tipo == "servico").collect();
    if !pecas.is_empty() {
        h.push_str(
            "<p style=\"margin-top:8pt\"><b>Peças</b></p><table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr>
               <th width=\"50%\">PEÇA</th><th width=\"12%\" class=\"num\">QTD.</th><th width=\"13%\" class=\"num\">PREÇO</th>
               <th width=\"12%\" class=\"num\">DESCONTO</th><th width=\"13%\" class=\"num\">TOTAL</th></tr>",
        );
        for i in pecas {
            h.push_str(&format!(
                "<tr><td>{}</td><td class=\"num\">{} {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\"><b>{}</b></td></tr>",
                escapar(&i.descricao),
                i.quantidade,
                escapar(&i.unidade),
                i.preco,
                if i.desconto.is_empty() { "—".to_string() } else { i.desconto.clone() },
                i.total
            ));
        }
        h.push_str("</table>");
    }
    if !servicos.is_empty() {
        h.push_str(
            "<p style=\"margin-top:8pt\"><b>Serviços</b></p><table class=\"itens\" width=\"100%\" cellspacing=\"0\"><tr>
               <th width=\"50%\">SERVIÇO</th><th width=\"12%\" class=\"num\">QTD.</th><th width=\"13%\" class=\"num\">VALOR</th>
               <th width=\"12%\" class=\"num\">DESCONTO</th><th width=\"13%\" class=\"num\">TOTAL</th></tr>",
        );
        for i in servicos {
            h.push_str(&format!(
                "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\"><b>{}</b></td></tr>",
                escapar(&i.descricao),
                i.quantidade,
                i.preco,
                if i.desconto.is_empty() { "—".to_string() } else { i.desconto.clone() },
                i.total
            ));
        }
        h.push_str("</table>");
    }
    h.push_str("<table width=\"100%\" cellspacing=\"0\" style=\"margin-top:8pt\"><tr><td width=\"55%\"></td><td><table width=\"100%\">");
    h.push_str(&format!("<tr><td>Peças</td><td class=\"num\">R$ {}</td></tr>", o.calculo.pecas));
    h.push_str(&format!("<tr><td>Serviços</td><td class=\"num\">R$ {}</td></tr>", o.calculo.servicos));
    if o.calculo.desconto != "0,00" {
        h.push_str(&format!("<tr><td>Desconto</td><td class=\"num\">− R$ {}</td></tr>", o.calculo.desconto));
    }
    h.push_str(&format!("<tr><td class=\"total\">Total</td><td class=\"num total\">R$ {}</td></tr>", o.calculo.total));
    h.push_str("</table></td></tr></table>");
    if !o.observacoes.is_empty() {
        h.push_str(&format!("<p>Observações: {}</p>", escapar(&o.observacoes).replace('\n', "<br/>")));
    }
    h.push_str(&rodape(conn, "rodape_os")?);
    let frase = if o.situacao == "entregue" {
        "Recebi o carro com os serviços acima feitos."
    } else {
        "Autorizo os serviços e as peças acima."
    };
    h.push_str(&format!(
        "<p style=\"margin-top:26pt\">{frase}</p><p style=\"margin-top:26pt\">______________________________<br/>{}</p>",
        escapar(&o.cliente)
    ));
    h.push_str("<p class=\"nota\" style=\"margin-top:18pt\">Não é documento fiscal.</p></body></html>");
    Ok(h)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escapa_html() {
        assert_eq!(escapar("<b>R&D \"x\"</b>"), "&lt;b&gt;R&amp;D &quot;x&quot;&lt;/b&gt;");
    }
}
