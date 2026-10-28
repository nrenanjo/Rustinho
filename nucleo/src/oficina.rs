//! Ordem de serviço da oficina.
//!
//! Rascunho (0.9.15): ao sair da Nova OS sem terminar, "Salvar rascunho"
//! guarda o que foi preenchido e os itens, sem número e sem exigir cliente
//! ou carro. O rascunho aparece em "Abertas" e "Todas" e, ao salvar de
//! verdade, vira orçamento com número (o mesmo registro, sem duplicar).
//! Rascunho novo só quando todos os caixas da loja confirmaram uma versão
//! que entende rascunho (veja `sync::bloqueios_de_rascunho`).
//!
//! A OS tem o cliente, o carro (placa, modelo e km), o que o cliente
//! reclamou, as peças e os serviços (mão de obra). Caminho:
//! **orçamento → aprovada (em serviço) → pronta → entregue**.
//!
//! As peças só saem do estoque na entrega: aí a OS vira uma venda (no
//! Vender, com todas as formas de pagamento). Se a venda for cancelada, a
//! OS volta para "pronta".

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::vendas::ItemEntrada;
use crate::{auditoria, config, documento, estoque, fiscal, numeros, tempo, texto};

pub const SITUACOES: &[(&str, &str)] = &[
    ("rascunho", "Rascunho"),
    ("orcamento", "Orçamento"),
    ("aprovada", "Em serviço"),
    ("pronta", "Pronta"),
    ("entregue", "Entregue"),
    ("cancelada", "Cancelada"),
];

pub fn nome_situacao(s: &str) -> &'static str {
    SITUACOES.iter().find(|x| x.0 == s).map(|x| x.1).unwrap_or("")
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemEntradaOs {
    /// peca ou servico
    pub tipo: String,
    pub produto_id: String,
    pub descricao: String,
    pub quantidade: String,
    pub preco: String,
    pub desconto: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entrada {
    /// Vazio = OS nova.
    pub id: String,
    pub cliente_id: String,
    pub cliente_nome: String,
    pub telefone: String,
    pub placa: String,
    pub veiculo: String,
    pub km: String,
    pub defeito: String,
    pub diagnostico: String,
    pub observacoes: String,
    pub mecanico_id: String,
    /// "dd/mm/aaaa"
    pub previsao: String,
    pub desconto: String,
    pub itens: Vec<ItemEntradaOs>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOs {
    pub tipo: String,
    pub produto_id: String,
    pub codigo: String,
    pub descricao: String,
    pub unidade: String,
    pub quantidade: String,
    pub preco: String,
    pub desconto: String,
    pub total: String,
    /// Peça: quanto tem no estoque e se falta.
    pub estoque: String,
    pub falta: bool,
    pub erro: String,
    #[serde(skip)]
    v_quantidade: i64,
    #[serde(skip)]
    v_preco: i64,
    #[serde(skip)]
    v_desconto: i64,
    #[serde(skip)]
    v_total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calculo {
    pub itens: Vec<ItemOs>,
    pub pecas: String,
    pub servicos: String,
    pub desconto: String,
    pub total: String,
    pub erro: String,
    pub campo: String,
    #[serde(skip)]
    v_desconto: i64,
}

fn multiplicar(quantidade: i64, preco: i64) -> i64 {
    crate::numeros::saturar((quantidade as i128 * preco as i128 + 500) / 1000)
}

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
        return Ok(crate::numeros::saturar((base as i128 * pct as i128 + 5_000) / 10_000));
    }
    let v = numeros::ler_dinheiro(t)?.unwrap_or(0);
    if v < 0 {
        return Err("O desconto não pode ser negativo.".into());
    }
    Ok(v)
}

/// Confere e soma os itens. Não grava nada.
pub fn calcular(conn: &Connection, e: &Entrada) -> Resultado<Calculo> {
    let mut itens = Vec::new();
    let mut falha: Option<(String, String)> = None;
    let mut falhou = |c: String, m: String| {
        if falha.is_none() {
            falha = Some((c, m));
        }
    };
    if e.itens.len() > 200 {
        return Err(Erro::validacao("Use no máximo 200 itens por OS."));
    }
    for (i, it) in e.itens.iter().enumerate() {
        let campo = format!("itens.{i}");
        let peca = it.tipo != "servico";
        let (codigo, mut descricao, unidade, preco_tabela, estoque_atual) = if peca {
            let p: Option<(String, String, String, Option<i64>)> = conn
                .query_row(
                    "SELECT codigo, nome, unidade, (SELECT preco FROM produto_preco WHERE produto_id = p.id AND tabela = 1)
                       FROM produto p WHERE id = ?1",
                    [&it.produto_id],
                    |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
                )
                .optional()?;
            let p = p.ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
            (p.0, p.1, p.2, p.3.unwrap_or(0), estoque::saldo(conn, &it.produto_id)?)
        } else {
            (String::new(), String::new(), "SV".to_string(), 0, 0)
        };
        if !peca {
            descricao = texto::limpar(&it.descricao).chars().take(120).collect();
        }
        let mut erro = String::new();
        if descricao.is_empty() {
            erro = "Diga qual é o serviço.".into();
        }
        let quantidade = match numeros::ler_quantidade(&it.quantidade) {
            Ok(Some(q)) if q > 1_000_000_000 => {
                erro = "Quantidade grande demais.".into();
                0
            }
            Ok(Some(q)) if q > 0 => q,
            Ok(None) if !peca => 1000,
            Ok(_) => {
                erro = "Quantidade precisa ser maior que zero.".into();
                0
            }
            Err(m) => {
                erro = m;
                0
            }
        };
        let preco = match numeros::ler_dinheiro(&it.preco) {
            Ok(Some(v)) if v > 100_000_000_000 => {
                erro = "Preço grande demais.".into();
                0
            }
            Ok(Some(v)) if v >= 0 => v,
            Ok(Some(_)) => {
                erro = "O preço não pode ser negativo.".into();
                0
            }
            Ok(None) => preco_tabela,
            Err(m) => {
                erro = m;
                0
            }
        };
        let bruto = multiplicar(quantidade, preco);
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
        if !erro.is_empty() {
            falhou(campo, format!("{}: {erro}", if descricao.is_empty() { "Serviço" } else { &descricao }));
        }
        // A mesma peça duas vezes conta junto para o estoque.
        let ja = numeros::soma(
            itens
                .iter()
                .filter(|x: &&ItemOs| x.tipo == "peca" && x.produto_id == it.produto_id)
                .map(|x| x.v_quantidade),
        )?;
        itens.push(ItemOs {
            tipo: if peca { "peca".into() } else { "servico".into() },
            produto_id: if peca { it.produto_id.clone() } else { String::new() },
            codigo,
            descricao,
            unidade,
            quantidade: numeros::escrever_quantidade(quantidade),
            preco: numeros::escrever_dinheiro(preco),
            desconto: if desconto > 0 { numeros::escrever_dinheiro(desconto) } else { String::new() },
            total: numeros::escrever_dinheiro(bruto - desconto),
            estoque: if peca { numeros::escrever_quantidade(estoque_atual) } else { String::new() },
            falta: peca && ja + quantidade > estoque_atual,
            erro,
            v_quantidade: quantidade,
            v_preco: preco,
            v_desconto: desconto,
            v_total: bruto - desconto,
        });
    }
    let pecas = numeros::somar(itens.iter().filter(|i| i.tipo == "peca").map(|i| i.v_total))
        .ok_or_else(numeros::grande_demais)?;
    let servicos = numeros::somar(itens.iter().filter(|i| i.tipo == "servico").map(|i| i.v_total))
        .ok_or_else(numeros::grande_demais)?;
    let _ = numeros::somar([pecas, servicos]).ok_or_else(numeros::grande_demais)?;
    let desconto = match ler_desconto(&e.desconto, pecas + servicos) {
        Ok(d) if d > pecas + servicos => {
            falhou("desconto".into(), "O desconto é maior que o total.".into());
            0
        }
        Ok(d) => d,
        Err(m) => {
            falhou("desconto".into(), m);
            0
        }
    };
    let (campo, erro) = falha.unwrap_or_default();
    Ok(Calculo {
        itens,
        pecas: numeros::escrever_dinheiro(pecas),
        servicos: numeros::escrever_dinheiro(servicos),
        desconto: numeros::escrever_dinheiro(desconto),
        total: numeros::escrever_dinheiro(pecas + servicos - desconto),
        erro,
        campo,
        v_desconto: desconto,
    })
}

fn situacao_de(conn: &Connection, id: &str) -> Resultado<(String, i64)> {
    conn.query_row("SELECT situacao, numero FROM ordem_servico WHERE id = ?1", [id], |l| Ok((l.get(0)?, l.get(1)?)))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Ordem de serviço".into()))
}

/// Grava a OS (nova ou alterada). Devolve o id.
pub fn salvar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada) -> Resultado<String> {
    let c = calcular(conn, e)?;
    if !c.erro.is_empty() {
        return Err(Erro::campo(&c.campo, c.erro));
    }
    let cliente_id = if e.cliente_id.is_empty() { None } else { Some(e.cliente_id.clone()) };
    let mut cliente_nome = texto::limpar(&e.cliente_nome);
    if let Some(id) = &cliente_id {
        let nome: Option<String> =
            conn.query_row("SELECT nome FROM pessoa WHERE id = ?1", [id], |l| l.get(0)).optional()?;
        cliente_nome = nome.ok_or_else(|| Erro::NaoEncontrado("Cliente".into()))?;
    }
    if cliente_nome.is_empty() {
        return Err(Erro::campo("cliente", "Diga de quem é o carro (o cliente)."));
    }
    let placa = documento::ler_placa(&e.placa).map_err(|m| Erro::campo("placa", m))?;
    let veiculo: String = texto::limpar(&e.veiculo).chars().take(60).collect();
    if placa.is_empty() && veiculo.is_empty() {
        return Err(Erro::campo("placa", "Digite a placa ou o carro."));
    }
    let previsao = fiscal::ler_data(&e.previsao).map_err(|m| Erro::campo("previsao", m))?;
    let corta = |t: &str, n: usize| -> String { t.trim().chars().take(n).collect() };
    let mecanico = if e.mecanico_id.is_empty() { None } else { Some(e.mecanico_id.clone()) };
    let agora = tempo::agora();
    let novo = e.id.is_empty();
    let id = if novo { tempo::novo_id() } else { e.id.clone() };
    if novo {
        let numero: i64 = conn.query_row(
            "SELECT COALESCE(MAX(numero), 0) + 1 FROM ordem_servico WHERE dispositivo = ?1",
            [dispositivo],
            |l| l.get(0),
        )?;
        conn.execute(
            "INSERT INTO ordem_servico (id, numero, dispositivo, caixa_nome, situacao, cliente_id, cliente_nome, telefone, placa,
                                        veiculo, km, defeito, diagnostico, observacoes, mecanico_id, previsao, desconto,
                                        criado_em, criado_por, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, 'orcamento', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?17)",
            params![
                id,
                numero,
                dispositivo,
                config::nome_caixa(conn)?,
                cliente_id,
                cliente_nome,
                corta(&e.telefone, 40),
                placa,
                veiculo,
                corta(&e.km, 12),
                corta(&e.defeito, 1000),
                corta(&e.diagnostico, 1000),
                corta(&e.observacoes, 1000),
                mecanico,
                previsao,
                c.v_desconto,
                agora,
                usuario_id
            ],
        )?;
        auditoria::registrar(
            conn,
            dispositivo,
            Some(usuario_id),
            "os.criar",
            "ordem_servico",
            &id,
            &[format!("OS {numero} aberta")],
        )?;
    } else {
        let (situacao, _) = situacao_de(conn, &id)?;
        if situacao == "entregue" || situacao == "cancelada" {
            return Err(Erro::validacao("Esta OS já foi entregue ou cancelada; não dá para mudar."));
        }
        // O rascunho salvo de verdade vira orçamento, com número (o mesmo
        // registro: nada é duplicado).
        if situacao == "rascunho" {
            let numero: i64 = conn.query_row(
                "SELECT COALESCE(MAX(numero), 0) + 1 FROM ordem_servico WHERE dispositivo = ?1",
                [dispositivo],
                |l| l.get(0),
            )?;
            conn.execute(
                "UPDATE ordem_servico SET situacao = 'orcamento', numero = ?2, dispositivo = ?3, caixa_nome = ?4,
                        criado_por = COALESCE(criado_por, ?5)
                  WHERE id = ?1 AND situacao = 'rascunho'",
                params![id, numero, dispositivo, config::nome_caixa(conn)?, usuario_id],
            )?;
            auditoria::registrar(
                conn,
                dispositivo,
                Some(usuario_id),
                "os.criar",
                "ordem_servico",
                &id,
                &[format!("OS {numero} aberta (era rascunho)")],
            )?;
        }
        conn.execute(
            "UPDATE ordem_servico SET cliente_id = ?2, cliente_nome = ?3, telefone = ?4, placa = ?5, veiculo = ?6, km = ?7,
                    defeito = ?8, diagnostico = ?9, observacoes = ?10, mecanico_id = ?11, previsao = ?12, desconto = ?13,
                    atualizado_em = ?14
              WHERE id = ?1",
            params![
                id,
                cliente_id,
                cliente_nome,
                corta(&e.telefone, 40),
                placa,
                veiculo,
                corta(&e.km, 12),
                corta(&e.defeito, 1000),
                corta(&e.diagnostico, 1000),
                corta(&e.observacoes, 1000),
                mecanico,
                previsao,
                c.v_desconto,
                agora
            ],
        )?;
        conn.execute("DELETE FROM ordem_servico_item WHERE ordem_id = ?1", [&id])?;
    }
    for (ordem, it) in c.itens.iter().enumerate() {
        conn.execute(
            "INSERT INTO ordem_servico_item (id, ordem_id, ordem, tipo, produto_id, codigo, descricao, unidade, quantidade, preco, desconto)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tempo::novo_id(),
                id,
                ordem as i64,
                it.tipo,
                if it.produto_id.is_empty() { None } else { Some(&it.produto_id) },
                it.codigo,
                it.descricao,
                it.unidade,
                it.v_quantidade,
                it.v_preco,
                it.v_desconto
            ],
        )?;
    }
    // Carro novo de cliente cadastrado: fica no cadastro dele.
    if let Some(cid) = &cliente_id {
        if !placa.is_empty() {
            let tem: bool = conn
                .query_row(
                    "SELECT 1 FROM pessoa_veiculo WHERE pessoa_id = ?1 AND placa = ?2",
                    params![cid, placa],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !tem {
                let ordem: i64 = conn.query_row(
                    "SELECT COALESCE(MAX(ordem), -1) + 1 FROM pessoa_veiculo WHERE pessoa_id = ?1",
                    [cid],
                    |l| l.get(0),
                )?;
                conn.execute(
                    "INSERT INTO pessoa_veiculo (id, pessoa_id, ordem, placa, descricao) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![tempo::novo_id(), cid, ordem, placa, veiculo],
                )?;
            }
        }
    }
    Ok(id)
}

/// Placa do rascunho: a lida, se já estiver completa; senão o que foi
/// digitado (letras e números, até 8), para continuar depois.
fn placa_do_rascunho(texto: &str) -> String {
    match documento::ler_placa(texto) {
        Ok(p) => p,
        Err(_) => texto.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).take(8).collect(),
    }
}

/// Guarda o que foi preenchido como rascunho (novo ou o mesmo rascunho de
/// antes). Não pede cliente nem carro. Devolve o id.
pub fn salvar_rascunho(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada) -> Resultado<String> {
    let c = calcular(conn, e)?;
    if !c.erro.is_empty() {
        return Err(Erro::campo(&c.campo, c.erro));
    }
    let novo = e.id.is_empty();
    if novo {
        let bloqueios = crate::sync::bloqueios_de_rascunho(conn)?;
        if !bloqueios.is_empty() {
            let nomes = bloqueios.iter().map(|b| b.nome.clone()).collect::<Vec<_>>().join(", ");
            return Err(Erro::campo(
                "rascunho",
                format!(
                    "Rascunho novo está bloqueado: {nomes} ainda não confirmou uma versão do Rustinho que entende \
                     rascunho. Atualize o Rustinho nesse caixa e deixe trocar dados, ou retire-o da loja em \
                     Configurações › Caixas da loja. Você pode salvar a OS normalmente."
                ),
            ));
        }
    } else {
        let (situacao, _) = situacao_de(conn, &e.id)?;
        if situacao != "rascunho" {
            return Err(Erro::validacao("Esta OS já foi salva; use Salvar."));
        }
    }
    let mut cliente_id = if e.cliente_id.is_empty() { None } else { Some(e.cliente_id.clone()) };
    let mut cliente_nome = texto::limpar(&e.cliente_nome);
    if let Some(id) = &cliente_id {
        match conn.query_row("SELECT nome FROM pessoa WHERE id = ?1", [id], |l| l.get::<_, String>(0)).optional()? {
            Some(n) => cliente_nome = n,
            None => cliente_id = None,
        }
    }
    let placa = placa_do_rascunho(&e.placa);
    let veiculo: String = texto::limpar(&e.veiculo).chars().take(60).collect();
    let previsao = fiscal::ler_data(&e.previsao)
        .map_err(|m| Erro::campo("previsao", format!("{m} Corrija ou apague a previsão para salvar o rascunho.")))?;
    let corta = |t: &str, n: usize| -> String { t.trim().chars().take(n).collect() };
    let mecanico = if e.mecanico_id.is_empty() { None } else { Some(e.mecanico_id.clone()) };
    let agora = tempo::agora();
    let id = if novo { tempo::novo_id() } else { e.id.clone() };
    if novo {
        conn.execute(
            "INSERT INTO ordem_servico (id, numero, dispositivo, caixa_nome, situacao, cliente_id, cliente_nome, telefone, placa,
                                        veiculo, km, defeito, diagnostico, observacoes, mecanico_id, previsao, desconto,
                                        criado_em, criado_por, atualizado_em)
             VALUES (?1, 0, ?2, ?3, 'rascunho', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?16)",
            params![
                id,
                dispositivo,
                config::nome_caixa(conn)?,
                cliente_id,
                cliente_nome,
                corta(&e.telefone, 40),
                placa,
                veiculo,
                corta(&e.km, 12),
                corta(&e.defeito, 1000),
                corta(&e.diagnostico, 1000),
                corta(&e.observacoes, 1000),
                mecanico,
                previsao,
                c.v_desconto,
                agora,
                usuario_id
            ],
        )?;
    } else {
        conn.execute(
            "UPDATE ordem_servico SET cliente_id = ?2, cliente_nome = ?3, telefone = ?4, placa = ?5, veiculo = ?6, km = ?7,
                    defeito = ?8, diagnostico = ?9, observacoes = ?10, mecanico_id = ?11, previsao = ?12, desconto = ?13,
                    atualizado_em = ?14
              WHERE id = ?1 AND situacao = 'rascunho'",
            params![
                id,
                cliente_id,
                cliente_nome,
                corta(&e.telefone, 40),
                placa,
                veiculo,
                corta(&e.km, 12),
                corta(&e.defeito, 1000),
                corta(&e.diagnostico, 1000),
                corta(&e.observacoes, 1000),
                mecanico,
                previsao,
                c.v_desconto,
                agora
            ],
        )?;
        conn.execute("DELETE FROM ordem_servico_item WHERE ordem_id = ?1", [&id])?;
    }
    for (ordem, it) in c.itens.iter().enumerate() {
        conn.execute(
            "INSERT INTO ordem_servico_item (id, ordem_id, ordem, tipo, produto_id, codigo, descricao, unidade, quantidade, preco, desconto)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tempo::novo_id(),
                id,
                ordem as i64,
                it.tipo,
                if it.produto_id.is_empty() { None } else { Some(&it.produto_id) },
                it.codigo,
                it.descricao,
                it.unidade,
                it.v_quantidade,
                it.v_preco,
                it.v_desconto
            ],
        )?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "os.rascunho",
        "ordem_servico",
        &id,
        &[format!("Rascunho de OS guardado ({} itens)", c.itens.len())],
    )?;
    Ok(id)
}

/// Apaga um rascunho (só rascunho; OS de verdade se cancela com motivo).
pub fn excluir_rascunho(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<()> {
    let (situacao, _) = situacao_de(conn, id)?;
    if situacao != "rascunho" {
        return Err(Erro::validacao("Só um rascunho pode ser excluído. Uma OS salva se cancela, com o motivo."));
    }
    conn.execute("DELETE FROM ordem_servico_item WHERE ordem_id = ?1", [id])?;
    conn.execute("DELETE FROM ordem_servico WHERE id = ?1 AND situacao = 'rascunho'", [id])?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "os.rascunho_excluir",
        "ordem_servico",
        id,
        &["Rascunho de OS excluído".into()],
    )?;
    Ok(())
}

/// Muda a situação: aprovada (o cliente autorizou), pronta, ou volta.
pub fn mudar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, nova: &str) -> Resultado<()> {
    let (atual, numero) = situacao_de(conn, id)?;
    if atual == "rascunho" {
        return Err(Erro::validacao("É um rascunho: salve a OS antes de mudar a situação."));
    }
    let pode = matches!(
        (atual.as_str(), nova),
        ("orcamento", "aprovada") | ("aprovada", "pronta") | ("aprovada", "orcamento") | ("pronta", "aprovada")
    );
    if !pode {
        return Err(Erro::validacao(format!(
            "Não dá para passar de “{}” para “{}”.",
            nome_situacao(&atual),
            nome_situacao(nova)
        )));
    }
    let agora = tempo::agora();
    let coluna = match nova {
        "aprovada" => "aprovada_em",
        "pronta" => "pronta_em",
        _ => "atualizado_em",
    };
    conn.execute(
        &format!("UPDATE ordem_servico SET situacao = ?2, {coluna} = ?3, atualizado_em = ?3 WHERE id = ?1"),
        params![id, nova, agora],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "os.situacao",
        "ordem_servico",
        id,
        &[format!("OS {numero}: {} → {}", nome_situacao(&atual), nome_situacao(nova))],
    )?;
    Ok(())
}

pub fn cancelar(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, motivo: &str) -> Resultado<()> {
    let (atual, numero) = situacao_de(conn, id)?;
    if atual == "entregue" {
        return Err(Erro::validacao("OS entregue não se cancela: cancele a venda dela (no Caixa)."));
    }
    if atual == "cancelada" {
        return Err(Erro::validacao("Esta OS já foi cancelada."));
    }
    if atual == "rascunho" {
        return Err(Erro::validacao("Rascunho não se cancela: use Excluir rascunho."));
    }
    let motivo = texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga o motivo."));
    }
    conn.execute(
        "UPDATE ordem_servico SET situacao = 'cancelada', cancelada_em = ?2, motivo_cancelamento = ?3, atualizado_em = ?2 WHERE id = ?1",
        params![id, tempo::agora(), motivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "os.cancelar",
        "ordem_servico",
        id,
        &[format!("OS {numero} cancelada · {motivo}")],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ler
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ficha {
    pub id: String,
    pub numero: i64,
    pub caixa: String,
    pub situacao: String,
    pub nome_situacao: String,
    pub cliente_id: String,
    pub cliente: String,
    pub telefone: String,
    pub placa: String,
    pub veiculo: String,
    pub km: String,
    pub defeito: String,
    pub diagnostico: String,
    pub observacoes: String,
    pub mecanico_id: String,
    pub mecanico: String,
    pub previsao: String,
    pub atrasada: bool,
    pub desconto_texto: String,
    pub calculo: Calculo,
    pub venda_id: String,
    pub venda_numero: i64,
    pub criado_em: String,
    pub criado_por: String,
    pub aprovada_em: String,
    pub pronta_em: String,
    pub entregue_em: String,
    pub motivo_cancelamento: String,
    /// Outras OS do mesmo carro (histórico).
    pub historico: Vec<Resumo>,
}

pub fn ficha(conn: &Connection, id: &str) -> Resultado<Ficha> {
    #[allow(clippy::type_complexity)]
    let o: (
        (i64, String, String, Option<String>, String, String, String, String),
        (String, String, String, String, Option<String>, String, Option<String>, i64),
        (Option<String>, i64, String, String, Option<String>, Option<String>, Option<String>, String),
    ) = conn
        .query_row(
            "SELECT o.numero, o.caixa_nome, o.situacao, o.cliente_id, o.cliente_nome, o.telefone, o.placa, o.veiculo,
                    o.km, o.defeito, o.diagnostico, o.observacoes, o.mecanico_id, COALESCE(m.nome, ''), o.previsao, o.desconto,
                    o.venda_id, COALESCE(v.numero, 0), o.criado_em, COALESCE(u.nome, ''), o.aprovada_em, o.pronta_em, o.entregue_em,
                    o.motivo_cancelamento
               FROM ordem_servico o
               LEFT JOIN usuario m ON m.id = o.mecanico_id
               LEFT JOIN usuario u ON u.id = o.criado_por
               LEFT JOIN venda v ON v.id = o.venda_id
              WHERE o.id = ?1",
            [id],
            |l| {
                Ok((
                    (l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?, l.get(7)?),
                    (l.get(8)?, l.get(9)?, l.get(10)?, l.get(11)?, l.get(12)?, l.get(13)?, l.get(14)?, l.get(15)?),
                    (l.get(16)?, l.get(17)?, l.get(18)?, l.get(19)?, l.get(20)?, l.get(21)?, l.get(22)?, l.get(23)?),
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Ordem de serviço".into()))?;
    let (a, b, c) = o;
    let entrada = Entrada {
        id: id.to_string(),
        itens: itens_gravados(conn, id)?,
        desconto: if b.7 > 0 { numeros::escrever_dinheiro(b.7) } else { String::new() },
        ..Default::default()
    };
    let calculo = calcular(conn, &entrada)?;
    let hoje = fiscal::hoje(conn)?;
    let previsao = b.6.clone().unwrap_or_default();
    let quando = |iso: &Option<String>| -> Resultado<String> {
        Ok(match iso {
            Some(i) => conn
                .query_row("SELECT strftime('%d/%m/%Y %H:%M', ?1, 'localtime')", [i], |l| {
                    l.get::<_, Option<String>>(0)
                })?
                .unwrap_or_default(),
            None => String::new(),
        })
    };
    Ok(Ficha {
        id: id.to_string(),
        numero: a.0,
        caixa: a.1,
        nome_situacao: nome_situacao(&a.2).to_string(),
        atrasada: !previsao.is_empty() && previsao < hoje && matches!(a.2.as_str(), "orcamento" | "aprovada"),
        situacao: a.2,
        cliente_id: a.3.unwrap_or_default(),
        cliente: a.4,
        telefone: a.5,
        historico: if a.6.is_empty() {
            vec![]
        } else {
            listar_onde(conn, "o.placa = ?1 AND o.id <> ?2", &a.6, id, 20)?
        },
        placa: a.6,
        veiculo: a.7,
        km: b.0,
        defeito: b.1,
        diagnostico: b.2,
        observacoes: b.3,
        mecanico_id: b.4.unwrap_or_default(),
        mecanico: b.5,
        previsao: fiscal::data_br(&previsao),
        desconto_texto: if b.7 > 0 { numeros::escrever_dinheiro(b.7) } else { String::new() },
        calculo,
        venda_id: c.0.unwrap_or_default(),
        venda_numero: c.1,
        criado_em: quando(&Some(c.2))?,
        criado_por: c.3,
        aprovada_em: quando(&c.4)?,
        pronta_em: quando(&c.5)?,
        entregue_em: quando(&c.6)?,
        motivo_cancelamento: c.7,
    })
}

fn itens_gravados(conn: &Connection, id: &str) -> Resultado<Vec<ItemEntradaOs>> {
    let mut c = conn.prepare(
        "SELECT tipo, COALESCE(produto_id, ''), descricao, quantidade, preco, desconto
           FROM ordem_servico_item WHERE ordem_id = ?1 ORDER BY ordem",
    )?;
    let linhas = c.query_map([id], |l| {
        let desconto: i64 = l.get(5)?;
        Ok(ItemEntradaOs {
            tipo: l.get(0)?,
            produto_id: l.get(1)?,
            descricao: l.get(2)?,
            quantidade: numeros::escrever_quantidade(l.get(3)?),
            preco: numeros::escrever_dinheiro(l.get(4)?),
            desconto: if desconto > 0 { numeros::escrever_dinheiro(desconto) } else { String::new() },
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resumo {
    pub id: String,
    pub numero: i64,
    pub caixa: String,
    pub situacao: String,
    pub nome_situacao: String,
    pub cliente: String,
    pub placa: String,
    pub veiculo: String,
    pub defeito: String,
    pub mecanico: String,
    pub total: String,
    pub previsao: String,
    pub atrasada: bool,
    pub quando: String,
}

fn listar_onde(conn: &Connection, filtro: &str, a: &str, b: &str, limite: i64) -> Resultado<Vec<Resumo>> {
    let hoje = fiscal::hoje(conn)?;
    let sql = format!(
        "SELECT o.id, o.numero, o.caixa_nome, o.situacao, o.cliente_nome, o.placa, o.veiculo, o.defeito, COALESCE(m.nome, ''),
                COALESCE((SELECT SUM((i.quantidade * i.preco + 500) / 1000 - i.desconto) FROM ordem_servico_item i WHERE i.ordem_id = o.id), 0) - o.desconto,
                COALESCE(o.previsao, ''), strftime('%d/%m/%Y', o.criado_em, 'localtime')
           FROM ordem_servico o LEFT JOIN usuario m ON m.id = o.mecanico_id
          WHERE {filtro}
          ORDER BY CASE o.situacao WHEN 'pronta' THEN 0 WHEN 'aprovada' THEN 1 WHEN 'orcamento' THEN 2 WHEN 'rascunho' THEN 3 ELSE 4 END,
                   o.criado_em DESC
          LIMIT {limite}"
    );
    let mut c = conn.prepare(&sql)?;
    let linhas = c.query_map(params![a, b], |l| {
        let situacao: String = l.get(3)?;
        let previsao: String = l.get(10)?;
        let placa: String = l.get(5)?;
        Ok(Resumo {
            id: l.get(0)?,
            numero: l.get(1)?,
            caixa: l.get(2)?,
            nome_situacao: nome_situacao(&situacao).to_string(),
            atrasada: !previsao.is_empty() && previsao < hoje && matches!(situacao.as_str(), "orcamento" | "aprovada"),
            situacao,
            cliente: l.get(4)?,
            placa: documento::escrever_placa(&placa),
            veiculo: l.get(6)?,
            defeito: l.get(7)?,
            mecanico: l.get(8)?,
            total: numeros::escrever_dinheiro(l.get(9)?),
            previsao: fiscal::data_br(&previsao),
            quando: l.get::<_, Option<String>>(11)?.unwrap_or_default(),
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Lista as OS. `quais`: abertas (orçamento e em serviço), prontas,
/// entregues ou todas. A busca procura no número, placa, cliente e carro.
pub fn listar(conn: &Connection, quais: &str, busca: &str) -> Resultado<Vec<Resumo>> {
    let situacao = match quais {
        // O rascunho aparece em Abertas (e em Todas), com a situação "Rascunho".
        "abertas" => "o.situacao IN ('rascunho', 'orcamento', 'aprovada')",
        "prontas" => "o.situacao = 'pronta'",
        "entregues" => "o.situacao = 'entregue'",
        _ => "1 = 1",
    };
    let t = texto::normalizar(busca);
    let placa = documento::limpar(busca);
    if t.is_empty() {
        return listar_onde(conn, &format!("{situacao} AND ?1 = ?2"), "", "", 300);
    }
    listar_onde(
        conn,
        &format!(
            "{situacao} AND (normalizar(o.cliente_nome) LIKE '%' || ?1 || '%' OR normalizar(o.veiculo) LIKE '%' || ?1 || '%'
              OR (?2 <> '' AND o.placa LIKE '%' || ?2 || '%') OR CAST(o.numero AS TEXT) = ?1)"
        ),
        &t,
        &placa,
        300,
    )
}

/// Quantas OS em cada situação (para as pílulas e o Início).
pub fn contagem(conn: &Connection) -> Resultado<serde_json::Value> {
    let (abertas, prontas, atrasadas): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(situacao IN ('rascunho', 'orcamento', 'aprovada')), 0), COALESCE(SUM(situacao = 'pronta'), 0),
                COALESCE(SUM(situacao IN ('orcamento', 'aprovada') AND previsao IS NOT NULL AND previsao < date('now', 'localtime')), 0)
           FROM ordem_servico",
        [],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    Ok(serde_json::json!({ "abertas": abertas, "prontas": prontas, "atrasadas": atrasadas }))
}

// ---------------------------------------------------------------------------
// Entrega (vira venda)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaVenda {
    pub numero: i64,
    /// O caixa da OS (FALHA-02: o número é por caixa).
    pub caixa: String,
    pub os_id: String,
    pub cliente_id: String,
    pub cliente_nome: String,
    pub desconto: String,
    pub itens: Vec<ItemEntrada>,
}

pub fn para_venda(conn: &Connection, id: &str) -> Resultado<ParaVenda> {
    let f = ficha(conn, id)?;
    if f.situacao == "rascunho" {
        return Err(Erro::validacao("É um rascunho: salve a OS antes de entregar."));
    }
    if f.situacao == "entregue" {
        return Err(Erro::validacao("Esta OS já foi entregue."));
    }
    if f.situacao == "cancelada" {
        return Err(Erro::validacao("Esta OS foi cancelada."));
    }
    if f.calculo.itens.is_empty() {
        return Err(Erro::validacao("A OS não tem peças nem serviços."));
    }
    let itens = f
        .calculo
        .itens
        .iter()
        .map(|i| ItemEntrada {
            produto_id: i.produto_id.clone(),
            descricao: if i.tipo == "servico" { i.descricao.clone() } else { String::new() },
            quantidade: i.quantidade.clone(),
            preco: i.preco.clone(),
            desconto: i.desconto.clone(),
            encomendar: false,
        })
        .collect();
    Ok(ParaVenda {
        numero: f.numero,
        caixa: f.caixa,
        os_id: id.to_string(),
        cliente_id: f.cliente_id,
        cliente_nome: f.cliente,
        desconto: f.desconto_texto,
        itens,
    })
}

/// A venda da OS foi concluída: OS entregue.
pub fn entregue_na_venda(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    os_id: &str,
    venda_id: &str,
) -> Resultado<()> {
    let (atual, numero) = situacao_de(conn, os_id)?;
    if atual == "entregue" || atual == "cancelada" || atual == "rascunho" {
        return Err(Erro::validacao("Esta OS já foi entregue ou cancelada."));
    }
    let agora = tempo::agora();
    conn.execute(
        "UPDATE ordem_servico SET situacao = 'entregue', venda_id = ?2, entregue_em = ?3, atualizado_em = ?3 WHERE id = ?1",
        params![os_id, venda_id, agora],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "os.entregar",
        "ordem_servico",
        os_id,
        &[format!("OS {numero} entregue")],
    )?;
    Ok(())
}

/// A venda da OS foi cancelada: a OS volta para "pronta".
pub fn cancelar_da_venda(conn: &Connection, venda_id: &str) -> Resultado<()> {
    conn.execute(
        "UPDATE ordem_servico SET situacao = 'pronta', venda_id = NULL, entregue_em = NULL, atualizado_em = ?2
          WHERE venda_id = ?1 AND situacao = 'entregue'",
        params![venda_id, tempo::agora()],
    )?;
    Ok(())
}

/// Carros conhecidos pela placa (para preencher a OS): o cliente e o carro.
pub fn por_placa(conn: &Connection, placa: &str) -> Resultado<Option<serde_json::Value>> {
    let p = documento::limpar(placa);
    if p.len() < 7 {
        return Ok(None);
    }
    let r: Option<(String, String, String, String, String)> = conn
        .query_row(
            "SELECT p.id, p.nome, COALESCE(NULLIF(p.celular, ''), p.telefone), v.descricao, v.placa
               FROM pessoa_veiculo v JOIN pessoa p ON p.id = v.pessoa_id WHERE v.placa = ?1 LIMIT 1",
            [&p],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
        )
        .optional()?;
    if let Some(r) = r {
        return Ok(Some(serde_json::json!({
            "clienteId": r.0, "cliente": r.1, "telefone": r.2, "veiculo": r.3, "placa": documento::escrever_placa(&r.4),
        })));
    }
    // Sem cadastro: a última OS desse carro.
    let o: Option<(String, String, String)> = conn
        .query_row(
            "SELECT cliente_nome, telefone, veiculo FROM ordem_servico WHERE placa = ?1 AND situacao <> 'rascunho'
              ORDER BY criado_em DESC LIMIT 1",
            [&p],
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
        )
        .optional()?;
    Ok(o.map(|o| {
        serde_json::json!({ "clienteId": "", "cliente": o.0, "telefone": o.1, "veiculo": o.2, "placa": documento::escrever_placa(&p) })
    }))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::vendas::{self, PagamentoEntrada, VendaEntrada};
    use crate::{caixa, pessoas, produtos, usuarios};

    #[test]
    fn os_do_orcamento_a_entrega() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        caixa::abrir(c, d, &u, "Caixa 1", "0").unwrap();
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = "Pastilha".into();
        f.custo = "40".into();
        f.estoque_inicial = "4".into();
        let pastilha = produtos::salvar(c, d, &u, &f).unwrap(); // preço 80
        let mut p = pessoas::ficha_nova(c, "cliente").unwrap();
        p.nome = "Carlos".into();
        let cliente = pessoas::salvar(c, d, &u, &p).unwrap();

        let mut e = Entrada {
            cliente_id: cliente.clone(),
            placa: "abc1d23".into(),
            veiculo: "Gol G5 1.6".into(),
            km: "85000".into(),
            defeito: "Barulho ao frear".into(),
            itens: vec![
                ItemEntradaOs {
                    tipo: "peca".into(),
                    produto_id: pastilha.clone(),
                    quantidade: "1".into(),
                    ..Default::default()
                },
                ItemEntradaOs {
                    tipo: "servico".into(),
                    descricao: "Troca de pastilhas".into(),
                    preco: "60".into(),
                    ..Default::default()
                },
            ],
            desconto: "10".into(),
            ..Default::default()
        };
        // Serviço sem descrição não passa.
        let mut sem = e.clone();
        sem.itens[1].descricao = String::new();
        assert!(salvar(c, d, &u, &sem).is_err());

        let id = salvar(c, d, &u, &e).unwrap();
        let o = ficha(c, &id).unwrap();
        assert_eq!(
            (o.situacao.as_str(), o.calculo.pecas.as_str(), o.calculo.servicos.as_str()),
            ("orcamento", "80,00", "60,00")
        );
        assert_eq!(o.calculo.total, "130,00");
        assert_eq!(o.placa, "ABC1D23");
        // O carro foi para o cadastro do cliente e a placa acha o cliente.
        let achou = por_placa(c, "ABC-1D23").unwrap().unwrap();
        assert_eq!(achou["cliente"], "Carlos");

        // Muda e passa pelas situações; o estoque ainda não mexeu.
        e.id = id.clone();
        e.itens[0].quantidade = "2".into();
        salvar(c, d, &u, &e).unwrap();
        assert!(mudar(c, d, &u, &id, "pronta").is_err());
        mudar(c, d, &u, &id, "aprovada").unwrap();
        mudar(c, d, &u, &id, "pronta").unwrap();
        assert_eq!(crate::estoque::saldo(c, &pastilha).unwrap(), 4_000);
        assert_eq!(listar(c, "prontas", "").unwrap().len(), 1);
        let avisos = crate::avisos::do_dia(c).unwrap();
        assert!(avisos.iter().any(|a| a.id == "oficina" && a.texto == "1 carro pronto para entregar"));
        assert_eq!(listar(c, "todas", "abc1").unwrap().len(), 1);
        assert_eq!(listar(c, "todas", "carlos").unwrap().len(), 1);

        // Entrega: vira venda (peças saem do estoque, o serviço não).
        let pv = para_venda(c, &id).unwrap();
        assert_eq!(pv.itens.len(), 2);
        let venda = VendaEntrada {
            itens: pv.itens,
            tabela: 1,
            cliente_id: pv.cliente_id,
            desconto: pv.desconto,
            pagamentos: vec![PagamentoEntrada { forma: "pix".into(), valor: "210".into(), ..Default::default() }],
            os_id: id.clone(),
            ..Default::default()
        };
        let calc = vendas::calcular(c, &venda).unwrap();
        assert_eq!(calc.total, "210,00"); // 160 + 60 − 10
        assert!(calc.itens[1].servico);
        let venda_id = vendas::concluir(c, d, &u, &venda).unwrap();
        assert_eq!(crate::estoque::saldo(c, &pastilha).unwrap(), 2_000);
        let o = ficha(c, &id).unwrap();
        assert_eq!((o.situacao.as_str(), o.venda_id.as_str()), ("entregue", venda_id.as_str()));
        assert!(salvar(c, d, &u, &e).is_err());
        assert!(crate::impressao::ordem_servico(c, &id).unwrap().contains("Troca de pastilhas"));

        // Cancelou a venda: a OS volta para pronta e as peças voltam.
        vendas::cancelar(c, d, &u, &venda_id, "teste").unwrap();
        assert_eq!(ficha(c, &id).unwrap().situacao, "pronta");
        assert_eq!(crate::estoque::saldo(c, &pastilha).unwrap(), 4_000);
        cancelar(c, d, &u, &id, "cliente desistiu").unwrap();
        assert!(para_venda(c, &id).is_err());
    }
}
