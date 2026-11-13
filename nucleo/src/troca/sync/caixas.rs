//! Os caixas conhecidos, retirar e readmitir, as pendências e o histórico da troca.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// Um caixa que esta loja conhece, com a situação da versão dele.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaixaConhecido {
    pub dispositivo: String,
    pub nome: String,
    /// Estrutura do banco que ele informou (None = não informou: versão
    /// antiga ou nunca conversou direto com este caixa).
    pub esquema: Option<i64>,
    pub compativel_em: String,
    /// A mesma data no horário daqui ("19/10/2026 10:32").
    pub compativel_em_texto: String,
    pub dias_sem_confirmar: Option<i64>,
    /// "compativel", "vencido" (passou o prazo), "antigo", "desconhecido" ou "retirado".
    pub situacao: String,
    pub texto: String,
    /// Impede rascunhos de OS novos.
    pub bloqueia: bool,
    /// Pendências recebidas dele (impedem a retirada) e não enviadas a ele.
    pub pendencias_recebidas: i64,
    pub nao_enviadas: i64,
    pub retirado_em: String,
    pub retirado_motivo: String,
    pub retirado_por: String,
}

/// Todos os caixas que esta loja conhece: os que já conversaram com este
/// (`sync_par`), os que aparecem no log de alterações (algum caixa os
/// repassou) e os retirados. Este computador não entra.
pub fn caixas_conhecidos(conn: &Connection) -> Resultado<Vec<CaixaConhecido>> {
    let eu = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let ids: Vec<String> = conn
        .prepare(&format!(
            "SELECT dispositivo FROM sync_par
             UNION SELECT DISTINCT dispositivo FROM alteracao WHERE dispositivo <> ''
             UNION SELECT substr(chave, {}) FROM config WHERE chave LIKE '{PREFIXO_RETIRADO}%'",
            PREFIXO_RETIRADO.len() + 1
        ))?
        .query_map([], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for id in ids.into_iter().filter(|d| !d.is_empty() && *d != eu) {
        #[allow(clippy::type_complexity)]
        let par: Option<(String, Option<i64>, Option<String>, Option<f64>, Option<String>)> = conn
            .query_row(
                "SELECT nome, esquema, compativel_em, julianday('now') - julianday(compativel_em),
                        strftime('%d/%m/%Y %H:%M', compativel_em, 'localtime')
                   FROM sync_par WHERE dispositivo = ?1",
                [&id],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)),
            )
            .optional()?;
        let mut nome = par.as_ref().map(|p| p.0.clone()).unwrap_or_default();
        if nome.is_empty() {
            for sql in [
                "SELECT caixa_nome FROM caixa_sessao WHERE dispositivo = ?1 ORDER BY aberto_em DESC LIMIT 1",
                "SELECT caixa_nome FROM ordem_servico WHERE dispositivo = ?1 ORDER BY criado_em DESC LIMIT 1",
            ] {
                if let Some(n) = conn.query_row(sql, [&id], |l| l.get::<_, String>(0)).optional()? {
                    if !n.is_empty() {
                        nome = n;
                        break;
                    }
                }
            }
        }
        if nome.is_empty() {
            nome = format!(
                "Caixa sem nome ({})",
                id.chars().rev().take(6).collect::<String>().chars().rev().collect::<String>()
            );
        }
        let retirada: Option<Value> = config::ler(conn, &format!("{PREFIXO_RETIRADO}{id}"))?
            .filter(|v| !v.trim().is_empty())
            .map(|v| serde_json::from_str(&v).unwrap_or(json!({})));
        let (esquema, compativel_em, dias, compativel_em_texto) = match &par {
            Some((_, e, c, d, t)) => {
                (*e, c.clone().unwrap_or_default(), d.map(|d| d.floor() as i64), t.clone().unwrap_or_default())
            }
            None => (None, String::new(), None, String::new()),
        };
        let pendencias_recebidas: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'recebida' AND origem = ?1 AND estado = 'pendente'",
            [&id],
            |l| l.get(0),
        )?;
        let nao_enviadas: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'nao_enviada' AND par = ?1 AND estado = 'pendente'",
            [&id],
            |l| l.get(0),
        )?;
        let (situacao, texto) = if retirada.is_some() {
            ("retirado", "Retirado da loja: não troca dados nem conta para a compatibilidade.".to_string())
        } else if par.is_none() {
            ("desconhecido", "Nunca conversou direto com este caixa: a versão dele não é conhecida.".to_string())
        } else if esquema.is_none_or(|e| e < ESQUEMA_RASCUNHO) {
            ("antigo", "Está com uma versão antiga do Rustinho (antes da 0.9.15).".to_string())
        } else if dias.is_none_or(|d| d >= PRAZO_COMPATIBILIDADE_DIAS) {
            (
                "vencido",
                format!(
                    "Não confirma a versão há {} dias (o prazo é de {PRAZO_COMPATIBILIDADE_DIAS}).",
                    dias.unwrap_or(0)
                ),
            )
        } else {
            ("compativel", "Versão compatível, confirmada na última troca de dados.".to_string())
        };
        let txt = |k: &str| retirada.as_ref().and_then(|r| r[k].as_str().map(str::to_string)).unwrap_or_default();
        lista.push(CaixaConhecido {
            dispositivo: id,
            nome,
            esquema,
            compativel_em,
            compativel_em_texto,
            dias_sem_confirmar: dias,
            bloqueia: situacao != "compativel" && situacao != "retirado",
            situacao: situacao.into(),
            texto,
            pendencias_recebidas,
            nao_enviadas,
            retirado_em: txt("em"),
            retirado_motivo: txt("motivo"),
            retirado_por: txt("por"),
        });
    }
    lista.sort_by(|a, b| a.nome.cmp(&b.nome));
    Ok(lista)
}

/// Os caixas que impedem rascunhos de OS novos agora (vazio = pode).
pub fn bloqueios_de_rascunho(conn: &Connection) -> Resultado<Vec<CaixaConhecido>> {
    Ok(caixas_conhecidos(conn)?.into_iter().filter(|c| c.bloqueia).collect())
}

/// Retira da loja um caixa que deixou de pertencer a ela (gerente, com
/// motivo). Não conclui com alterações recebidas dele ainda pendentes (elas
/// ficariam sem dono). As que não iam para ele (versão antiga) deixam de
/// valer: ele não recebe mais nada. Vai para todos os caixas.
pub fn retirar_caixa(conn: &Connection, dispositivo: &str, motivo: &str, quem: &str) -> Resultado<()> {
    let eu = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    if dispositivo.is_empty() {
        return Err(Erro::validacao("Escolha o caixa."));
    }
    if dispositivo == eu {
        return Err(Erro::validacao("Este computador não pode retirar a si mesmo da loja."));
    }
    let motivo = crate::texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga por que o caixa está saindo da loja."));
    }
    let caixa = caixas_conhecidos(conn)?
        .into_iter()
        .find(|c| c.dispositivo == dispositivo)
        .ok_or_else(|| Erro::NaoEncontrado("Caixa".into()))?;
    if caixa.situacao == "retirado" {
        return Err(Erro::validacao("Este caixa já foi retirado da loja."));
    }
    if caixa.pendencias_recebidas > 0 {
        return Err(Erro::validacao(format!(
            "{} tem {} alteração(ões) recebida(s) e ainda pendente(s) neste caixa. Resolva ou arquive cada uma \
             (em Pendências da troca de dados) antes de retirá-lo.",
            caixa.nome, caixa.pendencias_recebidas
        )));
    }
    let agora = tempo::agora();
    let valor = json!({ "nome": caixa.nome, "motivo": motivo, "por": quem, "em": agora }).to_string();
    config::gravar(conn, &format!("{PREFIXO_RETIRADO}{dispositivo}"), &valor)?;
    conn.execute(
        "UPDATE sync_pendencia SET estado = 'resolvida', resolvido_em = ?2, atualizado_em = ?2,
                detalhe = detalhe || ' (caixa retirado da loja)'
          WHERE lado = 'nao_enviada' AND par = ?1 AND estado = 'pendente'",
        params![dispositivo, agora],
    )?;
    Ok(())
}

/// O gerente aceita de volta um caixa retirado.
pub fn readmitir_caixa(conn: &Connection, dispositivo: &str) -> Resultado<()> {
    if !retirado(conn, dispositivo)? {
        return Err(Erro::validacao("Este caixa não está retirado."));
    }
    conn.execute("DELETE FROM config WHERE chave = ?1", [format!("{PREFIXO_RETIRADO}{dispositivo}")])?;
    Ok(())
}

/// Uma pendência da troca de dados, para a tela.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pendencia {
    pub lado: String,
    pub par: String,
    pub alteracao_id: String,
    pub origem: String,
    pub caixa: String,
    pub tabela: String,
    pub o_que: String,
    pub motivo: String,
    pub texto_motivo: String,
    pub detalhe: String,
    pub estado: String,
    pub tentativas: i64,
    pub ultimo_erro: String,
    pub criado_em: String,
    pub pode_arquivar: bool,
    pub arquivada_por: String,
    pub arquivada_motivo: String,
}

pub fn texto_do_motivo(motivo: &str) -> &'static str {
    match motivo {
        "incompativel_esquema" => "Versão incompatível",
        "invalida" => "Dados estragados",
        "erro_aplicacao" => "Recusada ao gravar",
        "dependencia_suspensa" => "Depende de outra pendente",
        _ => "Pendente",
    }
}

/// Pendências (as pendentes e as arquivadas; até 300, as mais antigas primeiro).
pub fn pendencias_da_troca(conn: &Connection) -> Resultado<Vec<Pendencia>> {
    let caixas: HashMap<String, String> =
        caixas_conhecidos(conn)?.into_iter().map(|c| (c.dispositivo, c.nome)).collect();
    let mut c = conn.prepare(
        "SELECT lado, par, alteracao_id, origem, tabela, motivo, detalhe, estado, tentativas, ultimo_erro, criado_em,
                COALESCE((SELECT nome FROM usuario WHERE id = arquivada_por), ''), arquivada_motivo
           FROM sync_pendencia WHERE estado IN ('pendente', 'arquivada_manual')
          ORDER BY estado = 'arquivada_manual', criado_em LIMIT 300",
    )?;
    let lista = c
        .query_map([], |l| {
            let lado: String = l.get(0)?;
            let par: String = l.get(1)?;
            let origem: String = l.get(3)?;
            let tabela: String = l.get(4)?;
            let motivo: String = l.get(5)?;
            let estado: String = l.get(7)?;
            let quem = if lado == "nao_enviada" { &par } else { &origem };
            Ok(Pendencia {
                caixa: caixas.get(quem).cloned().unwrap_or_else(|| {
                    if quem.is_empty() {
                        "Este caixa".into()
                    } else {
                        quem.clone()
                    }
                }),
                o_que: nome_da_tabela(&tabela).to_string(),
                texto_motivo: texto_do_motivo(&motivo).into(),
                pode_arquivar: estado == "pendente" && lado == "recebida" && motivo != "incompativel_esquema",
                lado,
                par,
                alteracao_id: l.get(2)?,
                origem,
                tabela,
                motivo,
                detalhe: l.get(6)?,
                estado,
                tentativas: l.get(8)?,
                ultimo_erro: l.get(9)?,
                criado_em: l.get(10)?,
                arquivada_por: l.get(11)?,
                arquivada_motivo: l.get(12)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(lista)
}

/// Histórico da troca de dados (Configurações › Simultaneidade › Histórico):
/// como foi a última troca com cada caixa e as pendências que já saíram da
/// lista (aplicadas depois ou arquivadas pelo gerente). Só leitura.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricoDaTroca {
    pub caixas: Vec<TrocaComCaixa>,
    pub encerradas: Vec<PendenciaEncerrada>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrocaComCaixa {
    pub dispositivo: String,
    pub nome: String,
    /// "19/10/2026 10:32" no horário daqui; vazio = nunca.
    pub sincronizado_em: String,
    pub visto_em: String,
    pub erro: String,
    pub situacao: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendenciaEncerrada {
    pub alteracao_id: String,
    pub lado: String,
    pub caixa: String,
    pub o_que: String,
    pub texto_motivo: String,
    /// "resolvida" (aplicada ou não precisou mais) ou "arquivada_manual".
    pub estado: String,
    pub criado_em: String,
    pub encerrada_em: String,
    pub tentativas: i64,
    pub arquivada_por: String,
    pub arquivada_motivo: String,
}

pub fn historico_da_troca(conn: &Connection, limite: i64) -> Resultado<HistoricoDaTroca> {
    let conhecidos = caixas_conhecidos(conn)?;
    let nomes: HashMap<String, String> = conhecidos.iter().map(|c| (c.dispositivo.clone(), c.nome.clone())).collect();
    let mut caixas = Vec::new();
    for c in &conhecidos {
        let linha: Option<(String, String, String)> = conn
            .query_row(
                "SELECT COALESCE(strftime('%d/%m/%Y %H:%M', sincronizado_em, 'localtime'), ''),
                        COALESCE(strftime('%d/%m/%Y %H:%M', visto_em, 'localtime'), ''),
                        erro
                   FROM sync_par WHERE dispositivo = ?1",
                [&c.dispositivo],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
            )
            .optional()?;
        let (sincronizado_em, visto_em, erro) = linha.unwrap_or_default();
        caixas.push(TrocaComCaixa {
            dispositivo: c.dispositivo.clone(),
            nome: c.nome.clone(),
            sincronizado_em,
            visto_em,
            erro,
            situacao: c.situacao.clone(),
        });
    }
    let mut consulta = conn.prepare(
        "SELECT alteracao_id, lado, par, origem, tabela, motivo, estado,
                COALESCE(strftime('%d/%m/%Y %H:%M', criado_em, 'localtime'), ''),
                COALESCE(strftime('%d/%m/%Y %H:%M', COALESCE(resolvido_em, atualizado_em), 'localtime'), ''),
                tentativas,
                COALESCE((SELECT nome FROM usuario WHERE id = arquivada_por), ''), arquivada_motivo
           FROM sync_pendencia WHERE estado IN ('resolvida', 'arquivada_manual')
          ORDER BY COALESCE(resolvido_em, atualizado_em) DESC LIMIT ?1",
    )?;
    let encerradas = consulta
        .query_map([limite], |l| {
            let lado: String = l.get(1)?;
            let par: String = l.get(2)?;
            let origem: String = l.get(3)?;
            let tabela: String = l.get(4)?;
            let motivo: String = l.get(5)?;
            let quem = if lado == "nao_enviada" { &par } else { &origem };
            Ok(PendenciaEncerrada {
                alteracao_id: l.get(0)?,
                caixa: nomes.get(quem).cloned().unwrap_or_else(|| {
                    if quem.is_empty() {
                        "Este caixa".into()
                    } else {
                        quem.clone()
                    }
                }),
                o_que: nome_da_tabela(&tabela).to_string(),
                texto_motivo: texto_do_motivo(&motivo).into(),
                lado,
                estado: l.get(6)?,
                criado_em: l.get(7)?,
                encerrada_em: l.get(8)?,
                tentativas: l.get(9)?,
                arquivada_por: l.get(10)?,
                arquivada_motivo: l.get(11)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(HistoricoDaTroca { caixas, encerradas })
}

/// O gerente arquiva uma pendência recebida que não tem como ser aplicada
/// (com motivo). A cópia inteira continua guardada; nunca aparece como
/// aplicada. Incompatibilidade de versão não se arquiva: só sai com a
/// atualização do caixa.
pub fn arquivar_pendencia(conn: &Connection, alteracao_id: &str, motivo: &str, quem: &str) -> Resultado<()> {
    let motivo = crate::texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga por que esta pendência vai ser arquivada."));
    }
    let atual: Option<(String, String)> = conn
        .query_row(
            "SELECT motivo, estado FROM sync_pendencia WHERE lado = 'recebida' AND alteracao_id = ?1",
            [alteracao_id],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .optional()?;
    let Some((tipo, estado)) = atual else { return Err(Erro::NaoEncontrado("Pendência".into())) };
    if estado != "pendente" {
        return Err(Erro::validacao("Esta pendência não está mais pendente."));
    }
    if tipo == "incompativel_esquema" {
        return Err(Erro::validacao(
            "Incompatibilidade de versão não se arquiva: ela sai sozinha quando o caixa for atualizado.",
        ));
    }
    conn.execute(
        "UPDATE sync_pendencia SET estado = 'arquivada_manual', arquivada_por = ?2, arquivada_motivo = ?3, atualizado_em = ?4
          WHERE lado = 'recebida' AND alteracao_id = ?1",
        params![alteracao_id, quem, motivo, tempo::agora()],
    )?;
    Ok(())
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
pub(super) fn seq_maxima(conn: &Connection) -> Resultado<i64> {
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
