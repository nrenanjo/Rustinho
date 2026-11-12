//! As referências entre caixas, a consistência (`exigir_editavel`) e `compactar`.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// Quantos registros apontam para outro que ainda não chegou (ou foi
/// apagado). Normalmente é zero depois de trocar tudo com os outros caixas.
pub fn pendencias(conn: &Connection) -> Resultado<usize> {
    let mut c = conn.prepare("PRAGMA foreign_key_check")?;
    let n = c.query_map([], |_| Ok(()))?.count();
    Ok(n)
}

/// R-05: referências que não batem, separadas em dois tipos.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Referencias {
    /// Apontam para algo que ainda não chegou de outro caixa (normal no
    /// meio de uma troca; resolve sozinho quando o resto chegar).
    pub esperando: usize,
    /// Apontam para algo que foi apagado (aqui ou em outro caixa). Não
    /// resolve sozinho; nada é apagado para esconder: o gerente decide.
    pub quebradas: usize,
    /// Das que esperam, quantas esperam há 24 horas ou mais. É só um aviso:
    /// atraso não é inconsistência e não bloqueia nada.
    pub atrasadas: usize,
}

/// Uma referência que não bate, para a lista em "Caixas da loja".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Referencia {
    /// Tabela e chave (JSON) do registro que aponta.
    pub tabela: String,
    pub chave: String,
    /// "Venda", "Peça"... (para a tela).
    pub o_que: String,
    /// Tabela e chave do registro que falta.
    pub pai: String,
    pub falta: String,
    pub o_que_falta: String,
    /// "esperando", "apagado", "inconsistente" (com prova) ou "aceito" (o
    /// gerente aceitou como está).
    pub estado: String,
    pub desde: String,
    /// Prova da inconsistência: "log" ou "pares" (vazio nos outros estados).
    pub motivo: String,
    /// Esperando há 24 horas ou mais (aviso, não erro).
    pub atrasada: bool,
    /// Dá para o gerente "aceitar como está".
    pub pode_aceitar: bool,
    pub aceita_por: String,
    pub aceita_em: String,
    pub aceite_motivo: String,
    /// O que fazer.
    pub acao: String,
}

/// Depois de quanto tempo esperando uma referência passa a ser mostrada como
/// "atrasada". É só um aviso para o gerente olhar: **o tempo sozinho nunca a
/// torna inconsistente** (o dado pode chegar quando o outro caixa ligar).
pub(super) const ATRASO_AVISO_HORAS: i64 = 24;

/// Confere cada referência quebrada: o registro que falta ainda não chegou
/// (espera), foi apagado em algum caixa (o log diz), ou é inconsistente (o
/// log diz que existe e a tabela não tem).
/// (tabela, chave, tabela que falta, chave que falta, estado).
pub(super) type ReferenciaAgora = (String, String, String, String, &'static str);

pub(super) fn referencias_agora(conn: &Connection) -> Resultado<Vec<ReferenciaAgora>> {
    let mut c = conn.prepare("PRAGMA foreign_key_check")?;
    let violacoes: Vec<(String, Option<i64>, String, i64)> =
        c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?.collect::<Result<_, _>>()?;
    let mut lista = Vec::new();
    for (tabela, linha, pai, fk) in violacoes {
        let limpa = |t: &str| t.replace('"', "");
        let mut ligacao = Vec::new();
        {
            let mut q = conn.prepare(&format!("PRAGMA foreign_key_list(\"{}\")", limpa(&tabela)))?;
            let l = q.query_map([], |l| Ok((l.get::<_, i64>(0)?, l.get::<_, i64>(1)?, l.get::<_, String>(3)?)))?;
            for item in l {
                let (id, seq, de) = item?;
                if id == fk {
                    ligacao.push((seq, de));
                }
            }
        }
        ligacao.sort();
        let Some(linha) = linha else { continue };
        let (_, pk) = colunas(conn, &tabela)?;
        let juntar = |cols: &[String]| cols.iter().map(|c| format!("\"{}\"", limpa(c))).collect::<Vec<_>>().join(", ");
        let de: Vec<String> = ligacao.iter().map(|(_, c)| c.clone()).collect();
        if de.is_empty() || pk.is_empty() {
            continue;
        }
        let (chave, falta): (String, String) = conn.query_row(
            &format!(
                "SELECT json_array({}), json_array({}) FROM \"{}\" WHERE rowid = ?1",
                juntar(&pk),
                juntar(&de),
                limpa(&tabela)
            ),
            [linha],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )?;
        let ultima: Option<String> = conn
            .query_row(
                "SELECT operacao FROM alteracao WHERE tabela = ?1 AND chave = ?2 AND compactada = 0
                  ORDER BY ordem DESC, id DESC LIMIT 1",
                params![pai, falta],
                |l| l.get(0),
            )
            .optional()?;
        let estado = match ultima.as_deref() {
            Some("apagar") => "apagado",
            Some(_) => "inconsistente",
            None => "esperando",
        };
        lista.push((tabela, chave, pai, falta, estado));
    }
    Ok(lista)
}

/// Contagem das referências (veja `registrar_referencias`).
pub fn conferir_referencias(conn: &Connection) -> Resultado<Referencias> {
    let mut r = Referencias::default();
    for (.., estado) in referencias_agora(conn)? {
        if estado == "esperando" {
            r.esperando += 1;
        } else {
            r.quebradas += 1;
        }
    }
    Ok(r)
}

/// Confere de novo e guarda a lista (cada referência com a data em que
/// apareceu). Refeita a cada conferência, sem estado "grudado":
/// - **apagado**: o log diz que o registro foi apagado;
/// - **inconsistente (log)**: o log diz que o registro existe e a tabela não
///   tem (contradição entre dois registros locais, prova por si);
/// - **inconsistente (pares)**: o log nunca o mencionou, existe ao menos um
///   outro caixa conhecido e **todos** eles fizeram uma troca completa depois
///   de a referência aparecer, e o registro continua faltando;
/// - **esperando**: todo o resto. Sem prova não se bloqueia, e o tempo
///   sozinho não é prova (passadas 24 horas só ganha o marcador "atrasada").
///
/// O que o gerente aceitou (`aceito`) continua aceito enquanto o que falta for
/// o mesmo. A que deixou de faltar (o registro chegou) sai da lista sozinha.
/// Nenhum dado da loja é apagado.
pub fn registrar_referencias(conn: &Connection) -> Resultado<Referencias> {
    let agora = referencias_agora(conn)?;
    let agora_txt = tempo::agora();
    conn.execute("CREATE TEMP TABLE IF NOT EXISTS ref_atual (tabela TEXT, chave TEXT, pai TEXT)", [])?;
    conn.execute("DELETE FROM temp.ref_atual", [])?;
    // O aceite do gerente vale para a mesma falta, desde que a prova continue
    // sendo de inconsistência (apagado volta a ser apagado).
    const MANTEM_ACEITE: &str =
        "(sync_referencia.estado = 'aceito' AND sync_referencia.falta = excluded.falta AND excluded.estado <> 'apagado')";
    let upsert = format!(
        "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (tabela, chave, pai) DO UPDATE SET
             -- A espera recomeça só se o registro que falta mudou; a mesma
             -- referência, conferida de novo, guarda a data do começo.
             desde = CASE WHEN sync_referencia.falta <> excluded.falta THEN excluded.desde ELSE sync_referencia.desde END,
             estado = CASE WHEN {MANTEM_ACEITE} THEN 'aceito' ELSE excluded.estado END,
             motivo = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.motivo ELSE excluded.motivo END,
             aceita_por = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.aceita_por END,
             aceita_em = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.aceita_em END,
             aceite_motivo = CASE WHEN {MANTEM_ACEITE} THEN sync_referencia.aceite_motivo END,
             falta = excluded.falta"
    );
    for (tabela, chave, pai, falta, estado) in &agora {
        conn.execute("INSERT INTO temp.ref_atual VALUES (?1, ?2, ?3)", params![tabela, chave, pai])?;
        let motivo = if *estado == "inconsistente" { "log" } else { "" };
        conn.execute(&upsert, params![tabela, chave, pai, falta, estado, agora_txt, motivo])?;
    }
    conn.execute(
        "DELETE FROM sync_referencia WHERE NOT EXISTS
            (SELECT 1 FROM temp.ref_atual a WHERE a.tabela = sync_referencia.tabela
                AND a.chave = sync_referencia.chave AND a.pai = sync_referencia.pai)",
        [],
    )?;
    // Prova dos pares: todos os caixas conhecidos já entregaram tudo (troca
    // completa) DEPOIS de a referência aparecer, e o registro não veio.
    // FALHA-03: não vale enquanto o registro que falta estiver guardado aqui,
    // numa pendência recebida ainda não resolvida (ele chegou, só não pôde
    // ser aplicado): a referência continua esperando. Resolvida a pendência,
    // a referência é conferida de novo (some, ou a prova volta a valer).
    conn.execute(
        "UPDATE sync_referencia SET estado = 'inconsistente', motivo = 'pares'
          WHERE estado = 'esperando'
            AND EXISTS (SELECT 1 FROM sync_par)
            AND NOT EXISTS (SELECT 1 FROM sync_par p
                             WHERE p.completo_em IS NULL OR p.completo_em <= sync_referencia.desde)
            AND NOT EXISTS (SELECT 1 FROM sync_pendencia g
                             WHERE g.lado = 'recebida' AND g.estado = 'pendente'
                               AND g.tabela = sync_referencia.pai
                               AND json_valid(g.chave) AND json_valid(sync_referencia.falta)
                               AND json(g.chave) = json(sync_referencia.falta))",
        [],
    )?;
    let (esperando, atrasadas, quebradas): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(estado = 'esperando'), 0),
                COALESCE(SUM(estado = 'esperando' AND (julianday('now') - julianday(desde)) * 24 >= ?1), 0),
                COALESCE(SUM(estado IN ('apagado', 'inconsistente')), 0)
           FROM sync_referencia",
        [ATRASO_AVISO_HORAS],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    config::gravar(conn, "sync_pendencias", &esperando.to_string())?;
    config::gravar(conn, "sync_atrasadas", &atrasadas.to_string())?;
    config::gravar(conn, "sync_quebradas", &quebradas.to_string())?;
    Ok(Referencias { esperando: esperando as usize, quebradas: quebradas as usize, atrasadas: atrasadas as usize })
}

/// A lista para a tela (as mais antigas primeiro, até 100).
pub fn referencias(conn: &Connection) -> Resultado<Vec<Referencia>> {
    #[allow(clippy::type_complexity)]
    let linhas: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        bool,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .prepare(
            "SELECT r.tabela, r.chave, r.pai, r.falta, r.estado, r.desde, r.motivo,
                    r.estado = 'esperando' AND (julianday('now') - julianday(r.desde)) * 24 >= ?1,
                    (SELECT nome FROM usuario WHERE id = r.aceita_por), r.aceita_em, r.aceite_motivo
               FROM sync_referencia r
              ORDER BY r.estado = 'esperando', r.desde LIMIT 100",
        )?
        .query_map([ATRASO_AVISO_HORAS], |l| {
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
    let mut lista = Vec::new();
    for (tabela, chave, pai, falta, estado, desde, motivo, atrasada, aceita_por, aceita_em, aceite_motivo) in linhas {
        let o_que_falta = nome_da_tabela(&pai).to_string();
        let acao = match (estado.as_str(), motivo.as_str()) {
            ("esperando", _) if atrasada => "Está esperando há mais de 24 horas. Isso não é erro e não bloqueia nada: o dado pode \
                  chegar quando o outro caixa ligar. Confira se os outros caixas estão ligados, na mesma rede e na mesma \
                  versão, e se algum aparece acima com erro."
                .to_string(),
            ("esperando", _) => {
                "Ainda não chegou de outro caixa. Deixe os caixas ligados na mesma rede: resolve sozinho.".to_string()
            }
            ("apagado", _) => format!(
                "{o_que_falta} foi apagado em um caixa, mas este registro ainda aponta para ele. Abra o registro e \
                 corrija (escolha outro), ou cadastre de novo o que foi apagado. Nada foi apagado por isso."
            ),
            ("aceito", _) => "O gerente aceitou este registro como está. As operações estão liberadas.".to_string(),
            (_, "pares") => "Todos os caixas conhecidos já entregaram tudo o que têm e nenhum tem o que falta (o caixa que \
                  criou isto pode ter se perdido). Restaure o backup desse caixa, ou o gerente pode aceitar como está."
                .to_string(),
            _ => "O registro de alterações diz que o que falta existe, mas o banco não tem. Faça um backup e deixe os \
                  caixas trocarem dados de novo; se continuar, o gerente pode aceitar como está."
                .to_string(),
        };
        lista.push(Referencia {
            o_que: nome_da_tabela(&tabela).into(),
            tabela,
            chave,
            pai,
            falta,
            o_que_falta,
            pode_aceitar: estado == "inconsistente",
            estado,
            desde: crate::fiscal::data_local_br(conn, &desde).unwrap_or(desde),
            motivo,
            atrasada,
            aceita_por: aceita_por.unwrap_or_default(),
            aceita_em: aceita_em.map(|q| crate::fiscal::data_local_br(conn, &q).unwrap_or(q)).unwrap_or_default(),
            aceite_motivo: aceite_motivo.unwrap_or_default(),
            acao,
        });
    }
    Ok(lista)
}

/// O gerente aceita um registro **comprovadamente inconsistente** como está
/// (R-05). Exige o motivo; só vale para o que está `inconsistente`; não
/// inventa o registro que falta, só libera as operações. Quem chama grava a
/// auditoria. Devolve quantas referências foram aceitas.
pub fn aceitar_inconsistencia(
    conn: &Connection,
    usuario_id: &str,
    tabela: &str,
    chave: &str,
    motivo: &str,
) -> Resultado<usize> {
    let motivo = crate::texto::limpar(motivo);
    if motivo.chars().count() < 5 {
        return Err(Erro::campo("motivo", "Escreva o motivo (pelo menos 5 letras): fica no histórico."));
    }
    let n = conn.execute(
        "UPDATE sync_referencia SET estado = 'aceito', aceita_por = ?3, aceita_em = ?4, aceite_motivo = ?5
          WHERE tabela = ?1 AND chave = ?2 AND estado = 'inconsistente'",
        params![tabela, chave, usuario_id, tempo::agora(), motivo],
    )?;
    if n == 0 {
        return Err(Erro::validacao(
            "Este registro não está inconsistente (pode já ter sido resolvido). Atualize a lista.",
        ));
    }
    Ok(n)
}

/// R-05: recusa mexer no dinheiro de um registro **comprovadamente
/// inconsistente** (`sync_referencia.estado = 'inconsistente'`). Registro
/// esperando, apagado ou aceito pelo gerente passa; ver e imprimir nunca
/// passam por aqui. `id` é a chave do registro em `tabela`.
pub fn exigir_editavel(conn: &Connection, tabela: &str, id: &str) -> Resultado<()> {
    exigir_sem_inconsistencia(
        conn,
        "r.tabela = ?1 AND EXISTS (SELECT 1 FROM json_each(r.chave) WHERE value = ?2)",
        params![tabela, id],
    )
}

/// O mesmo para os registros "filhos" de um registro (os itens de uma compra,
/// os itens de uma devolução...): `filha.coluna = id`.
pub fn exigir_filhos_editaveis(conn: &Connection, filha: &str, coluna: &str, id: &str) -> Resultado<()> {
    let nome_ok = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    if !nome_ok(filha) || !nome_ok(coluna) {
        return Err(Erro::Interno("exigir_filhos_editaveis: nome de tabela ou coluna inválido".into()));
    }
    exigir_sem_inconsistencia(
        conn,
        &format!("r.tabela = ?1 AND r.chave IN (SELECT json_array(f.id) FROM \"{filha}\" f WHERE f.\"{coluna}\" = ?2)"),
        params![filha, id],
    )
}

pub(super) fn exigir_sem_inconsistencia(
    conn: &Connection,
    onde: &str,
    valores: impl rusqlite::Params,
) -> Resultado<()> {
    let achada: Option<(String, String, String)> = conn
        .query_row(
            &format!(
                "SELECT r.tabela, r.pai, r.motivo FROM sync_referencia r
                  WHERE r.estado = 'inconsistente' AND {onde} LIMIT 1"
            ),
            valores,
            |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
        )
        .optional()?;
    let Some((tabela, pai, motivo)) = achada else { return Ok(()) };
    let prova = if motivo == "pares" {
        "todos os outros caixas já entregaram tudo e nenhum tem esse dado"
    } else {
        "o registro de alterações diz que ele existe e o banco não tem"
    };
    Err(Erro::validacao(format!(
        "Este registro ({}) ficou inconsistente: falta {} ({prova}). Por segurança o Rustinho não deixa mexer no \
         dinheiro dele até o gerente resolver. Configurações › Caixas da loja mostra o que fazer: restaurar o backup do \
         caixa de origem, ou o gerente aceitar como está.",
        nome_da_tabela(&tabela).to_lowercase(),
        nome_da_tabela(&pai).to_lowercase(),
    )))
}

/// Tira os dados das versões velhas de cada registro (fica a que vale).
/// A versão velha continua no log, sem dados: id, base e ordem bastam para
/// os caixas seguirem a cadeia de versões (R-08). Devolve quantas apagou. Pode rodar a qualquer hora: quem
/// pedir depois recebe só a versão que vale.
pub fn compactar(conn: &Connection) -> Resultado<usize> {
    let mut total = 0;
    loop {
        // Fica a versão que vale (maior ordem, depois maior id) e as que
        // perderam um conflito ainda aberto (para irem aos outros caixas).
        let n = conn.execute(
            "UPDATE alteracao SET dados = NULL, historia = NULL, compactada = 1 WHERE seq IN (
                SELECT a.seq FROM alteracao a
                 WHERE a.compactada = 0
                   AND EXISTS (SELECT 1 FROM alteracao b
                                WHERE b.tabela = a.tabela AND b.chave = a.chave AND b.compactada = 0
                                  AND (b.ordem > a.ordem OR (b.ordem = a.ordem AND b.id > a.id)))
                   AND a.id NOT IN (SELECT perdeu FROM sync_conflito WHERE resolvido_em IS NULL)
                 LIMIT 5000)",
            [],
        )?;
        total += n;
        if n < 5000 {
            return Ok(total);
        }
    }
}
