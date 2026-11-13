//! Os textos e as telas de conflito; resolver e manter todos; pago a mais.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

pub(super) const TEXTO_CORRIGIDO: &str = "Corrigido: o que foi pago (ou usado) não passa mais do valor";

/// (tabela, tipo do conflito, as que passam do valor, as operações de uma).
pub(super) const CONTAS_A_MAIS: &[(&str, &str, &str, &str)] = &[
    (
        "conta_receber",
        "pago_a_mais",
        "SELECT id FROM conta_receber_saldo WHERE cancelado_em IS NULL AND pago > valor",
        "SELECT group_concat(id, ',') FROM (SELECT r.id FROM recebimento_parcela rp JOIN recebimento r ON r.id = rp.recebimento_id
          WHERE rp.conta_id = ?1 AND r.estornado_em IS NULL ORDER BY r.criado_em, r.id)",
    ),
    (
        "conta_pagar",
        "pago_a_mais",
        "SELECT id FROM conta_pagar_saldo WHERE cancelado_em IS NULL AND pago > valor",
        "SELECT group_concat(id, ',') FROM (SELECT id FROM pagamento WHERE conta_pagar_id = ?1 AND estornado_em IS NULL
          ORDER BY criado_em, id)",
    ),
    (
        "vale",
        "vale_a_mais",
        "SELECT id FROM vale_saldo WHERE cancelado_em IS NULL AND saldo < 0",
        "SELECT group_concat(id, ',') FROM (SELECT DISTINCT x.id FROM venda_pagamento p JOIN venda x ON x.id = p.venda_id
          WHERE p.vale_id = ?1 AND x.situacao = 'concluida' ORDER BY x.criado_em, x.id)",
    ),
];

/// R-03: a conta (ou o vale) continua passando do valor? (Só lê.)
pub(super) fn ainda_a_mais(conn: &Connection, tipo: &str, tabela: &str, chave: &str) -> Resultado<bool> {
    let sql = match (tipo, tabela) {
        ("pago_a_mais", "conta_receber") => {
            "SELECT EXISTS (SELECT 1 FROM conta_receber_saldo WHERE id = ?1 AND cancelado_em IS NULL AND pago > valor)"
        }
        ("pago_a_mais", "conta_pagar") => {
            "SELECT EXISTS (SELECT 1 FROM conta_pagar_saldo WHERE id = ?1 AND cancelado_em IS NULL AND pago > valor)"
        }
        ("vale_a_mais", _) => {
            "SELECT EXISTS (SELECT 1 FROM vale_saldo WHERE id = ?1 AND cancelado_em IS NULL AND saldo < 0)"
        }
        _ => return Ok(true),
    };
    Ok(conn.query_row(sql, [chave], |l| l.get(0))?)
}

/// R-03: o texto do conflito, com o que o gerente precisa para achar as
/// operações (quem, quanto, em que caixas).
pub(super) fn texto_a_mais(
    conn: &Connection,
    tipo: &str,
    tabela: &str,
    chave: &str,
    perdeu: &str,
) -> Resultado<String> {
    let ops: Vec<&str> = perdeu.split(',').filter(|x| !x.is_empty()).collect();
    let dinheiro = |v: i64| format!("R$ {}", crate::numeros::escrever_dinheiro(v));
    let lista = |sql: &str| -> Resultado<Vec<String>> {
        let mut v = Vec::new();
        for id in &ops {
            if let Some((numero, valor, disp, estornado)) = conn
                .query_row(sql, [id], |l| {
                    Ok((l.get::<_, i64>(0)?, l.get::<_, i64>(1)?, l.get::<_, String>(2)?, l.get::<_, bool>(3)?))
                })
                .optional()?
            {
                let caixa = nome_do_caixa(conn, &disp)?;
                let quem = if caixa.is_empty() { "outro caixa".to_string() } else { caixa };
                let n = if numero > 0 { format!("Nº {numero} ") } else { String::new() };
                v.push(format!("{n}no {quem}: {}{}", dinheiro(valor), if estornado { " (já estornado)" } else { "" }));
            }
        }
        Ok(v)
    };
    Ok(match (tipo, tabela) {
        ("pago_a_mais", "conta_receber") => {
            let (cliente, valor, pago, venc, parcela, parcelas): (String, i64, i64, String, i64, i64) = conn
                .query_row(
                    "SELECT COALESCE(p.nome, ''), s.valor, s.pago, s.vencimento, s.parcela, s.parcelas
                   FROM conta_receber_saldo s LEFT JOIN pessoa p ON p.id = s.cliente_id WHERE s.id = ?1",
                    [chave],
                    |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?)),
                )?;
            let recebimentos = lista(
                "SELECT r.numero, rp.valor, r.dispositivo, r.estornado_em IS NOT NULL
                   FROM recebimento r JOIN recebimento_parcela rp ON rp.recebimento_id = r.id WHERE r.id = ?1",
            )?;
            format!(
                "A parcela {parcela}/{parcelas} de {cliente} ({}, vencimento {}) foi recebida mais de uma vez em caixas \
                 sem rede: {}. Recebido no total: {}. Confira com o cliente e estorne o recebimento que sobrou em \
                 Receber (o estorno fica no histórico). Nada foi apagado sozinho.",
                dinheiro(valor),
                crate::fiscal::data_br(&venc),
                recebimentos.join(" e "),
                dinheiro(pago)
            )
        }
        ("pago_a_mais", "conta_pagar") => {
            let (fornecedor, descricao, valor, pago, venc): (String, String, i64, i64, String) = conn.query_row(
                "SELECT COALESCE(p.nome, ''), s.descricao, s.valor, s.pago, s.vencimento
                   FROM conta_pagar_saldo s LEFT JOIN pessoa p ON p.id = s.fornecedor_id WHERE s.id = ?1",
                [chave],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
            )?;
            let pagamentos =
                lista("SELECT 0, valor, dispositivo, estornado_em IS NOT NULL FROM pagamento WHERE id = ?1")?;
            format!(
                "A conta de {fornecedor}{} ({}, vencimento {}) foi paga mais de uma vez em caixas sem rede: {}. Pago no \
                 total: {}. Confira com o fornecedor e estorne o pagamento que sobrou em Pagar (o estorno fica no \
                 histórico). Nada foi apagado sozinho.",
                if descricao.is_empty() { String::new() } else { format!(" — {descricao}") },
                dinheiro(valor),
                crate::fiscal::data_br(&venc),
                pagamentos.join(" e "),
                dinheiro(pago)
            )
        }
        _ => {
            let (numero, cliente, valor, saldo): (i64, String, i64, i64) = conn.query_row(
                "SELECT numero, cliente_nome, valor, saldo FROM vale_saldo WHERE id = ?1",
                [chave],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
            )?;
            let vendas = lista(
                "SELECT x.numero, COALESCE((SELECT SUM(valor) FROM venda_pagamento WHERE venda_id = x.id AND vale_id IS NOT NULL), 0),
                        x.dispositivo, x.situacao <> 'concluida'
                   FROM venda x WHERE x.id = ?1",
            )?;
            format!(
                "O vale-troca Nº {numero}{} ({}) foi usado além do valor em caixas sem rede: vendas {}. Usado no total: \
                 {}. Confira com o cliente e corrija pela venda que sobrou (o cancelamento fica no histórico). Nada foi \
                 apagado sozinho.",
                if cliente.is_empty() { String::new() } else { format!(" de {cliente}") },
                dinheiro(valor),
                vendas.join(" e "),
                dinheiro(valor - saldo)
            )
        }
    })
}

/// Tabelas em que o gerente pode escolher "usar a versão que perdeu"
/// (cadastros). Nas de dinheiro e movimento (vendas, caixa, contas), mexer
/// assim poderia desequilibrar as contas: lá o gerente só fica ciente e
/// corrige pela tela de cada coisa.
pub(super) const PODE_USAR: &[&str] = &[
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

pub(super) fn nome_da_tabela(t: &str) -> &'static str {
    match t {
        "produto"
        | "produto_preco"
        | "produto_aplicacao"
        | "produto_equivalente"
        | "produto_fornecedor"
        | "produto_codigo_antigo"
        | "produto_gtin" => "Peça",
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
pub(super) fn nome_do_campo(campo: &str) -> String {
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
pub(super) const DINHEIRO: &[&str] =
    &["preco", "custo", "valor", "total", "subtotal", "desconto", "limite_credito", "saldo_inicial", "oferta_preco"];

pub(super) fn valor_curto(v: Option<&Value>, campo: &str) -> String {
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
            "nota" => {
                // G05 (passo 5.4): a mesma nota em mais de um caixa.
                let mut c = conn.prepare(
                    "SELECT c.numero, c.situacao, c.dispositivo, COALESCE(p.nome, '') FROM compra c
                       LEFT JOIN pessoa p ON p.id = c.fornecedor_id
                      WHERE c.id IN (SELECT value FROM json_each('[\"' || replace(?1, ',', '\",\"') || '\"]')) ORDER BY c.id",
                )?;
                let compras: Vec<(String, String, String, String)> = c
                    .query_map([&perdeu], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?
                    .collect::<Result<_, _>>()?;
                let mut onde = Vec::new();
                for (_, situacao, dispositivo, _) in &compras {
                    let como = match situacao.as_str() {
                        "lancada" => "lançada",
                        "rascunho" => "em rascunho",
                        _ => "estornada",
                    };
                    let caixa = nome_do_caixa(conn, dispositivo)?;
                    onde.push(format!("{como} no {}", if caixa.is_empty() { "outro caixa".into() } else { caixa }));
                }
                let (numero, fornecedor) = compras.first().map(|x| (x.0.clone(), x.3.clone())).unwrap_or_default();
                k.o_que = "Nota de compra".into();
                k.texto = format!(
                    "A nota Nº {numero}{} (chave {chave}) entrou em mais de um caixa sem rede: {}. Se é a mesma compra, \
                     estorne a que sobrou (ou descarte, se estiver em rascunho) em Compras.",
                    if fornecedor.is_empty() { String::new() } else { format!(" de {fornecedor}") },
                    onde.join(" e ")
                );
            }
            "pago_a_mais" | "vale_a_mais" => {
                k.o_que = match tabela.as_str() {
                    "conta_receber" => "Parcela recebida a mais",
                    "conta_pagar" => "Conta paga a mais",
                    _ => "Vale-troca usado a mais",
                }
                .into();
                k.texto = texto_a_mais(conn, &tipo, &tabela, &chave, &perdeu)?;
                // Já corrigido pelo gerente (estorno, cancelamento): sai da lista
                // de abertos na hora, sem esperar a próxima troca de dados.
                if k.resolvido.is_empty() && !ainda_a_mais(conn, &tipo, &tabela, &chave)? {
                    k.resolvido = TEXTO_CORRIGIDO.into();
                }
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
    let outros: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_conflito WHERE resolvido_em IS NULL AND tipo NOT IN ('pago_a_mais', 'vale_a_mais')",
        [],
        |l| l.get(0),
    )?;
    // R-03: os de valor pago a mais que o gerente já corrigiu não contam.
    let a_mais: Vec<(String, String, String)> = conn
        .prepare("SELECT tipo, tabela, chave FROM sync_conflito WHERE resolvido_em IS NULL AND tipo IN ('pago_a_mais', 'vale_a_mais')")?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut abertos = outros;
    for (tipo, tabela, chave) in a_mais {
        if ainda_a_mais(conn, &tipo, &tabela, &chave)? {
            abertos += 1;
        }
    }
    Ok(abertos)
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
        // D05 (passo 5.2): os derivados da linha que volta são refeitos
        // depois, a partir do que ficou gravado (a versão que perdeu pode
        // trazer um índice velho).
        let derivado = crate::dados::esquema::alimenta(&tabela);
        let mut refazer: Vec<(crate::dados::esquema::Derivado, String)> = Vec::new();
        match dados.as_deref().map(serde_json::from_str::<Value>) {
            None => {
                if let Some((coluna, d)) = derivado {
                    for id in donos_do_derivado(conn, &tabela, &pk, &valores_chave, coluna, None)? {
                        refazer.push((d, id));
                    }
                }
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
                if let Some((coluna, d)) = derivado {
                    for id in donos_do_derivado(conn, &tabela, &pk, &valores_chave, coluna, Some(&m))? {
                        refazer.push((d, id));
                    }
                }
                // Só as colunas que viajam (a derivada e a local da versão
                // guardada ficam como estão aqui).
                let cols: Vec<&String> = todas
                    .iter()
                    .filter(|c| {
                        m.contains_key(c.as_str())
                            && c.as_str() != "atualizado_em"
                            && crate::dados::esquema::coluna_viaja(&tabela, c)
                    })
                    .collect();
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
        for (d, id) in &refazer {
            refazer_derivado(conn, *d, id)?;
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
