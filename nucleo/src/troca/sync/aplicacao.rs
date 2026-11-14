//! O motor de aplicação: pendências, ligações, `aplicar_dentro`, reaplicar e verificar.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;
use crate::dados::esquema::Derivado;

/// A chave (`["loja_nome"]`) é de uma configuração da loja?
pub(super) fn config_da_loja(chave: &str) -> bool {
    match serde_json::from_str::<Vec<Value>>(chave).ok().as_deref() {
        Some([Value::String(c)]) => chave_da_loja(c),
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Modo {
    /// Lote que acabou de chegar de outro caixa.
    Normal,
    /// Pendências guardadas sendo tentadas de novo (da cópia guardada).
    Reaplicar,
}

pub(super) enum Falha {
    /// A regra do banco recusou uma alteração do lote (índice na lista): o
    /// lote inteiro é desfeito.
    Lote { indice: usize, erro: String },
    /// Outro problema (disco, banco ocupado...): nada muda, tenta depois.
    Outra(Erro),
}

impl From<Erro> for Falha {
    fn from(e: Erro) -> Self {
        Falha::Outra(e)
    }
}

impl From<rusqlite::Error> for Falha {
    fn from(e: rusqlite::Error) -> Self {
        Falha::Outra(e.into())
    }
}

/// O erro veio dos dados da alteração (uma regra do banco recusou), não do
/// disco ou do banco ocupado?
pub(super) fn erro_dos_dados(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _)
        if matches!(f.code, rusqlite::ErrorCode::ConstraintViolation | rusqlite::ErrorCode::TypeMismatch))
}

/// O que está pendente agora: registros, operações e motivos (para decidir
/// se uma alteração que chega depende de algo pendente).
#[derive(Default)]
pub(super) struct Pendentes {
    pub(super) registros: std::collections::HashSet<(String, String)>,
    pub(super) operacoes: std::collections::HashSet<String>,
}

impl Pendentes {
    pub(super) fn carregar(conn: &Connection) -> Resultado<Pendentes> {
        let mut p = Pendentes::default();
        let mut c = conn.prepare(
            "SELECT tabela, chave, operacao_id FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente'",
        )?;
        for l in
            c.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, Option<String>>(2)?)))?
        {
            let (t, k, o) = l?;
            p.registros.insert((t, k));
            if let Some(o) = o {
                p.operacoes.insert(o);
            }
        }
        Ok(p)
    }

    pub(super) fn juntar(&mut self, a: &Alteracao) {
        self.registros.insert((a.tabela.clone(), a.chave.clone()));
        if let Some(o) = &a.operacao_id {
            self.operacoes.insert(o.clone());
        }
    }

    pub(super) fn vazio(&self) -> bool {
        self.registros.is_empty() && self.operacoes.is_empty()
    }
}

/// Ligações (chaves estrangeiras) de uma tabela: (tabela pai, colunas daqui
/// na ordem da chave do pai).
pub(super) type Ligacoes = Vec<(String, Vec<String>)>;

pub(super) fn ligacoes(conn: &Connection, tabela: &str) -> Resultado<Ligacoes> {
    let mut c = conn.prepare(&format!("PRAGMA foreign_key_list(\"{tabela}\")"))?;
    let linhas: Vec<(i64, i64, String, String)> =
        c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?.collect::<Result<_, _>>()?;
    let mut por_id: std::collections::BTreeMap<i64, (String, Vec<(i64, String)>)> = Default::default();
    for (id, seq, pai, de) in linhas {
        por_id.entry(id).or_insert_with(|| (pai, Vec::new())).1.push((seq, de));
    }
    Ok(por_id
        .into_values()
        .map(|(pai, mut cols)| {
            cols.sort();
            (pai, cols.into_iter().map(|c| c.1).collect())
        })
        .collect())
}

/// A alteração depende de algo pendente? (o mesmo registro, a mesma
/// operação comercial ou um registro pai pendente.)
pub(super) fn depende_de_pendente(
    p: &Pendentes,
    a: &Alteracao,
    dados: Option<&serde_json::Map<String, Value>>,
    lig: &Ligacoes,
) -> Option<String> {
    if p.vazio() {
        return None;
    }
    if p.registros.contains(&(a.tabela.clone(), a.chave.clone())) {
        return Some("outra versão deste registro está pendente".into());
    }
    if a.operacao_id.as_ref().is_some_and(|o| p.operacoes.contains(o)) {
        return Some("outra parte da mesma operação está pendente".into());
    }
    if let Some(d) = dados {
        for (pai, cols) in lig {
            let valores: Option<Vec<Value>> = cols.iter().map(|c| d.get(c).filter(|v| !v.is_null()).cloned()).collect();
            if let Some(v) = valores {
                let chave = serde_json::to_string(&v).unwrap_or_default();
                if p.registros.contains(&(pai.clone(), chave)) {
                    return Some(format!("o registro de {pai} de que ela depende está pendente"));
                }
            }
        }
    }
    None
}

/// Dados estragados: a alteração não pode ser aplicada como veio.
pub(super) fn invalida(a: &Alteracao, chave_cols: &[String]) -> Option<String> {
    let valores: Vec<Value> = match serde_json::from_str::<Value>(&a.chave) {
        Ok(Value::Array(v)) => v,
        _ => return Some("a chave do registro não é uma lista".into()),
    };
    if valores.len() != chave_cols.len() {
        return Some(format!("a chave tem {} partes e a tabela {} pede {}", valores.len(), a.tabela, chave_cols.len()));
    }
    if a.operacao != "gravar" && a.operacao != "apagar" {
        return Some(format!("operação desconhecida: {}", a.operacao));
    }
    if a.operacao == "gravar" && !a.compactada {
        match a.dados.as_deref().map(serde_json::from_str::<Value>) {
            Some(Ok(Value::Object(m))) => {
                if let Some(k) = chave_cols.iter().find(|k| !m.contains_key(k.as_str())) {
                    return Some(format!("os dados não têm a coluna {k} da chave"));
                }
            }
            _ => return Some("os dados não são um registro (objeto JSON)".into()),
        }
    }
    None
}

/// O que o caixa de estrutura `esquema` não consegue gravar (matriz de
/// compatibilidade por versão). `None` = pode mandar.
pub fn incompatibilidade(esquema: i64, a: &Alteracao) -> Option<String> {
    if a.compactada {
        // Versão velha sem dados: só entra na cadeia de versões, nunca é aplicada.
        return None;
    }
    if !TABELAS.contains(&a.tabela.as_str()) {
        return Some(format!("a tabela {} não existe na versão dele", a.tabela));
    }
    // 0.9.18 (estrutura 24): os GTINs da peça viajam desde o passo 5.2; antes
    // da 24 a tabela não existe. (Com a regra de versão homogênea, um caixa
    // assim nem troca dados; a matriz fica certa para quem a consulta.)
    if esquema < ESQUEMA_GTINS && a.tabela == "produto_gtin" {
        return Some("GTINs da peça: o caixa precisa da versão 0.9.18 ou mais nova".into());
    }
    // 0.9.15 (estrutura 23): rascunho de OS. Na estrutura 22 o CHECK de
    // ordem_servico.situacao recusa 'rascunho'.
    if esquema < ESQUEMA_RASCUNHO && a.tabela == "ordem_servico" && a.operacao == "gravar" {
        let situacao = a
            .dados
            .as_deref()
            .and_then(|d| serde_json::from_str::<Value>(d).ok())
            .and_then(|v| v["situacao"].as_str().map(str::to_string));
        if situacao.as_deref() == Some("rascunho") {
            return Some("rascunho de ordem de serviço: o caixa precisa da versão 0.9.15 ou mais nova".into());
        }
    }
    None
}

/// Guarda (ou atualiza) uma alteração recebida em `sync_pendencia`, inteira.
pub(super) fn guardar_pendencia(
    conn: &Connection,
    origem: &str,
    a: &Alteracao,
    motivo: &str,
    detalhe: &str,
) -> Resultado<()> {
    let agora = tempo::agora();
    conn.execute(
        "INSERT INTO sync_pendencia (lado, par, alteracao_id, origem, seq, tabela, chave, operacao, dados, dispositivo, quando,
                                     base, ordem, historia, compactada, operacao_id, motivo, detalhe, estado, criado_em, atualizado_em)
         VALUES ('recebida', '', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'pendente', ?17, ?17)
         ON CONFLICT (lado, par, alteracao_id) DO UPDATE SET
             motivo = excluded.motivo, detalhe = excluded.detalhe, atualizado_em = excluded.atualizado_em,
             tentativas = sync_pendencia.tentativas + (sync_pendencia.estado = 'pendente')
           WHERE sync_pendencia.estado = 'pendente'",
        params![
            a.id,
            origem,
            a.seq,
            a.tabela,
            a.chave,
            a.operacao,
            a.dados,
            a.dispositivo,
            a.quando,
            a.base,
            a.ordem,
            a.historia,
            a.compactada as i64,
            a.operacao_id,
            motivo,
            detalhe,
            agora
        ],
    )?;
    Ok(())
}

pub(super) fn ja_pendente(conn: &Connection, id: &str) -> Resultado<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sync_pendencia WHERE lado = 'recebida' AND alteracao_id = ?1 AND estado IN ('pendente', 'arquivada_manual')",
            [id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false))
}

pub(super) fn avancar_cursor(tx: &Connection, cursor: Option<&AvancoCursor>) -> Resultado<bool> {
    let Some(c) = cursor else { return Ok(false) };
    if c.para == c.de && c.marca.is_empty() {
        tx.execute(
            "UPDATE sync_par SET geracao = ?2 WHERE dispositivo = ?1 AND (geracao IS NULL OR geracao = '')",
            params![c.dispositivo, c.geracao],
        )?;
        return Ok(true);
    }
    // Só avança se ninguém zerou o cursor no meio (restauração de backup).
    Ok(tx.execute(
        "UPDATE sync_par SET cursor = ?2, marca = ?4, geracao = ?5 WHERE dispositivo = ?1 AND cursor = ?3",
        params![c.dispositivo, c.para, c.de, c.marca, c.geracao],
    )? > 0)
}

pub(super) fn aplicar_dentro(
    conn: &mut Connection,
    lista: &[Alteracao],
    origem: &str,
    cursor: Option<&AvancoCursor>,
    modo: Modo,
) -> Result<Aplicacao, Falha> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE sync_estado SET aplicando = 1 WHERE id = 1", [])?;
    let mut novas = 0;
    let mut pendentes_agora = 0;
    let mut colunas_cache: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    let mut ligacoes_cache: HashMap<String, Ligacoes> = HashMap::new();
    // No lote novo, o que já está pendente; ao reaplicar, começa vazio (as
    // próprias pendências estão sendo tentadas, na ordem em que chegaram).
    let mut pend = if modo == Modo::Normal { Pendentes::carregar(&tx)? } else { Pendentes::default() };
    let mut gravados_no_lote: std::collections::HashSet<(String, String)> = Default::default();
    // Os derivados que as alterações aplicadas mudam (passo 5.2): refeitos
    // aqui, antes do COMMIT, a partir do que ficou gravado.
    let mut refazer: std::collections::BTreeSet<(Derivado, String)> = Default::default();
    let mut tabelas_tocadas: std::collections::BTreeSet<String> = Default::default();
    let ja_guardadas: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente' AND origem = ?1",
        [origem],
        |l| l.get(0),
    )?;
    let mut guardar = |tx: &Connection, a: &Alteracao, motivo: &str, detalhe: &str| -> Result<(), Falha> {
        if modo == Modo::Normal && ja_guardadas + pendentes_agora as i64 >= PENDENCIAS_MAXIMAS {
            return Err(Falha::Outra(Erro::validacao(format!(
                "Há {PENDENCIAS_MAXIMAS} alterações deste caixa guardadas sem conseguir aplicar. A troca com ele parou até \
                 o gerente olhar as pendências em Configurações › Caixas da loja."
            ))));
        }
        guardar_pendencia(tx, origem, a, motivo, detalhe)?;
        pendentes_agora += 1;
        Ok(())
    };
    for (indice, a) in lista.iter().enumerate() {
        if !TABELAS.contains(&a.tabela.as_str()) {
            guardar(
                &tx,
                a,
                "incompativel_esquema",
                &format!("a tabela {} não existe nesta versão do Rustinho", a.tabela),
            )?;
            pend.juntar(a);
            continue;
        }
        // D-16: da tabela de configurações só chegam as da loja (as mesmas
        // que os gatilhos mandam); as deste computador nunca vêm da rede.
        // Descarte de propósito: não vira pendência.
        if a.tabela == "config" && !config_da_loja(&a.chave) {
            continue;
        }
        // Já vista (reenvio, a própria alteração voltando, backup restaurado
        // em outro caixa): nada a fazer, nem aviso. Pelo id, não pela data
        // (o log guarda o id de toda versão, mesmo compactada). Uma alteração
        // deste caixa que ele não tem mais (perdeu ao restaurar um backup)
        // volta dos outros e é aplicada.
        let ja: bool =
            tx.query_row("SELECT 1 FROM alteracao WHERE id = ?1", [&a.id], |_| Ok(true)).optional()?.unwrap_or(false);
        if ja {
            if modo == Modo::Reaplicar {
                marcar_resolvida(&tx, &a.id)?;
            }
            continue;
        }
        // Já guardada como pendência (o mesmo lote chegando de novo).
        if modo == Modo::Normal && ja_pendente(&tx, &a.id)? {
            continue;
        }
        if let Some(m) = &a.incompativel {
            guardar(&tx, a, "incompativel_esquema", m)?;
            pend.juntar(a);
            continue;
        }
        if !colunas_cache.iter().any(|c| c.0 == a.tabela) {
            let (todas, chave) = colunas(&tx, &a.tabela)?;
            colunas_cache.push((a.tabela.clone(), todas, chave));
        }
        let Some((_, todas, chave)) = colunas_cache.iter().find(|c| c.0 == a.tabela) else { continue };
        if let Some(m) = invalida(a, chave) {
            guardar(&tx, a, "invalida", &m)?;
            pend.juntar(a);
            continue;
        }
        let dados: Option<serde_json::Map<String, Value>> = if a.operacao == "gravar" && !a.compactada {
            match a.dados.as_deref().map(serde_json::from_str::<Value>) {
                Some(Ok(Value::Object(m))) => Some(m),
                _ => None,
            }
        } else {
            None
        };
        if !ligacoes_cache.contains_key(&a.tabela) {
            let l = ligacoes(&tx, &a.tabela)?;
            ligacoes_cache.insert(a.tabela.clone(), l);
        }
        if let Some(m) = depende_de_pendente(&pend, a, dados.as_ref(), &ligacoes_cache[&a.tabela]) {
            guardar(&tx, a, "dependencia_suspensa", &m)?;
            pend.juntar(a);
            continue;
        }
        if modo == Modo::Reaplicar {
            marcar_resolvida(&tx, &a.id)?;
        }
        tabelas_tocadas.insert(a.tabela.clone());
        if a.operacao == "gravar" && !a.compactada {
            gravados_no_lote.insert((a.tabela.clone(), a.chave.clone()));
        }
        // Versão velha sem dados (o outro caixa compactou): entra só para a
        // cadeia de versões. Já perdeu lá para outra, que também vem.
        if a.compactada {
            tx.execute(
                "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia, compactada, operacao_id)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, NULL, 1, ?9)",
                params![a.id, a.tabela, a.chave, a.operacao, a.dispositivo, a.quando, a.base, a.ordem, a.operacao_id],
            )?;
            novas += 1;
            continue;
        }
        // A versão que vale agora deste registro, neste caixa.
        let ultima = versao_atual(&tx, &a.tabela, &a.chave)?;
        tx.execute(
            "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia, operacao_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![a.id, a.tabela, a.chave, a.operacao, a.dados, a.dispositivo, a.quando, a.base, a.ordem, a.historia, a.operacao_id],
        )?;
        novas += 1;
        let chegou = Versao {
            id: a.id.clone(),
            dispositivo: a.dispositivo.clone(),
            base: a.base.clone(),
            dados: a.dados.clone(),
            ordem: a.ordem,
            historia: a.historia.clone(),
        };
        if let Some(u) = &ultima {
            let ganhou = chegou.vale_mais_que(u);
            // Feitas sem uma ver a outra, ou sem como saber: fica anotado para
            // o gerente, com as duas guardadas. Uma feita depois da outra
            // (mesmo com o relógio atrasado) não é conflito.
            let r = relacao(&tx, &chegou, u)?;
            if matches!(r, Relacao::Concorrente | Relacao::Incerta) && a.tabela != "auditoria" {
                let (ficou, perdeu) = if ganhou { (&chegou, u) } else { (u, &chegou) };
                if !diferencas(ficou.dados.as_deref(), perdeu.dados.as_deref()).is_empty() {
                    anotar_conflito(&tx, &a.tabela, &a.chave, ficou, perdeu, r)?;
                }
            }
            if !ganhou {
                continue;
            }
        }
        let valores_chave: Vec<Value> = serde_json::from_str(&a.chave).map_err(Erro::from)?;
        if let Some((coluna, derivado)) = crate::dados::esquema::alimenta(&a.tabela) {
            for id in donos_do_derivado(&tx, &a.tabela, chave, &valores_chave, coluna, dados.as_ref())? {
                refazer.insert((derivado, id));
            }
        }
        let falha = |e: rusqlite::Error| -> Falha {
            if erro_dos_dados(&e) {
                Falha::Lote { indice, erro: e.to_string() }
            } else {
                Falha::Outra(e.into())
            }
        };
        if a.operacao == "apagar" {
            let onde = chave
                .iter()
                .enumerate()
                .map(|(i, c)| format!("\"{c}\" = ?{}", i + 1))
                .collect::<Vec<_>>()
                .join(" AND ");
            tx.execute(
                &format!("DELETE FROM \"{}\" WHERE {onde}", a.tabela),
                params_from_iter(valores_chave.iter().map(para_sql)),
            )
            .map_err(falha)?;
            continue;
        }
        let Some(dados) = dados else { continue };
        // Só as colunas que existem aqui (um caixa com versão mais nova pode ter
        // mais) e que viajam: a derivada e a local que vierem (de um log de
        // antes da 5.2) são ignoradas; a derivada é refeita aqui.
        let cols: Vec<&String> = todas
            .iter()
            .filter(|c| dados.contains_key(c.as_str()) && crate::dados::esquema::coluna_viaja(&a.tabela, c))
            .collect();
        let nomes = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let marcas = (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
        let atualizar: Vec<String> =
            cols.iter().filter(|c| !chave.contains(c)).map(|c| format!("\"{c}\" = excluded.\"{c}\"")).collect();
        let conflito = if atualizar.is_empty() {
            "DO NOTHING".to_string()
        } else {
            format!("DO UPDATE SET {}", atualizar.join(", "))
        };
        // A mesma linha (pela chave natural) já gravada aqui com outro id: fica
        // a daqui; a que chegou fica só no log (passo 5.2, PEND-35).
        let repetida =
            if crate::dados::esquema::chave_natural(&a.tabela).is_some() { " ON CONFLICT DO NOTHING" } else { "" };
        let sql = format!(
            "INSERT INTO \"{}\" ({nomes}) VALUES ({marcas}) ON CONFLICT ({}) {conflito}{repetida}",
            a.tabela,
            chave.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")
        );
        tx.execute(&sql, params_from_iter(cols.iter().map(|c| para_sql(&dados[c.as_str()])))).map_err(falha)?;
    }
    for (derivado, id) in &refazer {
        refazer_derivado(&tx, *derivado, id)?;
    }
    // Antes do COMMIT: um registro que aponta para outro gravado neste mesmo
    // lote tem de encontrá-lo (o pai que ainda não chegou é normal: R-05).
    for tabela in &tabelas_tocadas {
        let mut c = tx.prepare(&format!("PRAGMA foreign_key_check(\"{tabela}\")"))?;
        let violacoes: Vec<(String, Option<i64>, String, i64)> =
            c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?.collect::<Result<_, _>>()?;
        drop(c);
        for (_, linha, pai, fk) in violacoes {
            let Some(linha) = linha else { continue };
            let lig = ligacoes(&tx, tabela)?;
            let Some((_, cols)) = lig.get(fk as usize).or_else(|| lig.iter().find(|(p, _)| *p == pai)) else {
                continue;
            };
            let sel = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
            let falta: String =
                tx.query_row(&format!("SELECT json_array({sel}) FROM \"{tabela}\" WHERE rowid = ?1"), [linha], |l| {
                    l.get(0)
                })?;
            if gravados_no_lote.contains(&(pai.clone(), falta.clone())) {
                let indice = lista.iter().position(|a| a.tabela == *tabela).unwrap_or(0);
                return Err(Falha::Lote {
                    indice,
                    erro: format!("{tabela} aponta para {pai} {falta}, gravado neste lote, e não o encontra"),
                });
            }
        }
    }
    let avancou = avancar_cursor(&tx, cursor)?;
    limpar_conflitos(&tx)?;
    tx.execute("UPDATE sync_estado SET aplicando = 0 WHERE id = 1", [])?;
    tx.commit()?;
    Ok(Aplicacao { novas, pendentes: pendentes_agora, avancou, suspenso: false })
}

/// O dono refaz um derivado (passo 5.2, DP-15). Sem `_`: um derivado novo no
/// `dados::esquema` não compila sem dizer aqui quem o refaz.
pub(super) fn refazer_derivado(conn: &Connection, derivado: Derivado, id: &str) -> Resultado<()> {
    match derivado {
        // A peça pode ainda não ter chegado (R-05): refeito quando ela chegar.
        Derivado::IndiceDaPeca => {
            crate::catalogo::codigos::refazer_indice(conn, id)?;
        }
    }
    Ok(())
}

/// Refaz todos os derivados de todas as linhas donas (N-I1: depois de
/// restaurar um backup, que pode ser de antes da 5.2, com o índice velho do
/// D01; veja `backup::Pendente::aplicar`). Só mexe em colunas que não viajam:
/// não grava alteração. Devolve quantas linhas.
pub fn refazer_todos_os_derivados(conn: &Connection) -> Resultado<usize> {
    let mut derivados: Vec<Derivado> = crate::dados::esquema::ALIMENTAM.iter().map(|a| a.2).collect();
    derivados.sort();
    derivados.dedup();
    let mut n = 0;
    for d in derivados {
        n += refazer_derivado_de_todas(conn, d)?;
    }
    Ok(n)
}

// Só nos testes (RC2-5): quantas vezes um derivado foi refeito para todas as
// linhas, nesta linha de execução (o teste e o que ele chama rodam nela).
#[cfg(test)]
thread_local! {
    pub(crate) static REFEITOS_DE_TODAS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Refaz o derivado de todas as linhas donas (R-1: na primeira abertura com a
/// 5.2, veja `registro::registrar_as_que_passaram_a_viajar`). Sem `_`, como
/// [`refazer_derivado`].
pub(super) fn refazer_derivado_de_todas(conn: &Connection, derivado: Derivado) -> Resultado<usize> {
    #[cfg(test)]
    REFEITOS_DE_TODAS.with(|c| c.set(c.get() + 1));
    match derivado {
        Derivado::IndiceDaPeca => crate::catalogo::codigos::refazer_todos_os_indices(conn),
    }
}

/// As linhas donas do derivado que uma alteração de `tabela` muda: a da linha
/// como está aqui (antes de aplicar) e a da que chegou.
pub(super) fn donos_do_derivado(
    conn: &Connection,
    tabela: &str,
    chave: &[String],
    valores_chave: &[Value],
    coluna: &str,
    dados: Option<&serde_json::Map<String, Value>>,
) -> Resultado<Vec<String>> {
    let onde = chave.iter().enumerate().map(|(i, c)| format!("\"{c}\" = ?{}", i + 1)).collect::<Vec<_>>().join(" AND ");
    let antes: Option<Option<String>> = conn
        .query_row(
            &format!("SELECT \"{coluna}\" FROM \"{tabela}\" WHERE {onde}"),
            params_from_iter(valores_chave.iter().map(para_sql)),
            |l| l.get(0),
        )
        .optional()?;
    let mut ids: Vec<String> = antes.flatten().into_iter().collect();
    if let Some(Value::String(id)) = dados.and_then(|d| d.get(coluna)) {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    Ok(ids)
}

pub(super) fn marcar_resolvida(conn: &Connection, id: &str) -> Resultado<()> {
    conn.execute(
        "UPDATE sync_pendencia SET estado = 'resolvida', resolvido_em = ?2, atualizado_em = ?2
          WHERE lado = 'recebida' AND alteracao_id = ?1 AND estado = 'pendente'",
        params![id, tempo::agora()],
    )?;
    Ok(())
}

/// O lote foi desfeito (uma regra do banco recusou a alteração `indice`).
/// Numa transação nova: o lote inteiro vai para as pendências (a que falhou
/// com o erro; as outras como suspensas junto) e a numeração avança, para a
/// troca seguir com os lotes seguintes. Nada do lote entra nas tabelas.
pub(super) fn suspender_lote(
    conn: &mut Connection,
    lista: &[Alteracao],
    origem: &str,
    indice: usize,
    erro: &str,
    cursor: Option<&AvancoCursor>,
) -> Resultado<Aplicacao> {
    crate::registro::anotar(&format!("troca de dados: lote de {} alterações suspenso: {erro}", lista.len()));
    let tx = conn.transaction()?;
    let mut pendentes = 0;
    for (i, a) in lista.iter().enumerate() {
        if a.tabela == "config" && !config_da_loja(&a.chave) {
            continue;
        }
        let ja: bool =
            tx.query_row("SELECT 1 FROM alteracao WHERE id = ?1", [&a.id], |_| Ok(true)).optional()?.unwrap_or(false);
        if ja || ja_pendente(&tx, &a.id)? {
            continue;
        }
        if i == indice {
            guardar_pendencia(&tx, origem, a, "erro_aplicacao", erro)?;
            tx.execute(
                "UPDATE sync_pendencia SET ultimo_erro = ?2 WHERE lado = 'recebida' AND alteracao_id = ?1",
                params![a.id, erro],
            )?;
        } else {
            guardar_pendencia(
                &tx,
                origem,
                a,
                "dependencia_suspensa",
                "suspensa junto com o lote em que outra alteração falhou",
            )?;
        }
        pendentes += 1;
    }
    let avancou = avancar_cursor(&tx, cursor)?;
    if !origem.is_empty() {
        tx.execute(
            "UPDATE sync_par SET erro = ?2 WHERE dispositivo = ?1",
            params![
                origem,
                format!(
                    "Uma alteração vinda deste caixa não pôde ser aplicada ({erro}). O lote ficou guardado nas pendências; \
                     a troca continua com o resto."
                )
            ],
        )?;
    }
    tx.commit()?;
    Ok(Aplicacao { novas: 0, pendentes, avancou, suspenso: true })
}

/// Tenta de novo, a partir da cópia guardada, as pendências recebidas (na
/// ordem em que chegaram). Tudo numa transação: se uma regra do banco ainda
/// recusar alguma, nada muda e cada uma ganha uma tentativa a mais. As que
/// continuam sem poder (dados estragados, dependência) seguem pendentes.
/// Devolve quantas foram aplicadas.
pub fn reaplicar_pendencias(conn: &mut Connection) -> Resultado<usize> {
    let lista: Vec<(String, Alteracao)> = {
        let mut c = conn.prepare(
            "SELECT origem, seq, alteracao_id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia,
                    compactada, operacao_id
               FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente' ORDER BY criado_em, seq",
        )?;
        let l = c.query_map([], |l| {
            Ok((
                l.get::<_, String>(0)?,
                Alteracao {
                    seq: l.get(1)?,
                    id: l.get(2)?,
                    tabela: l.get(3)?,
                    chave: l.get(4)?,
                    operacao: l.get(5)?,
                    dados: l.get(6)?,
                    dispositivo: l.get(7)?,
                    quando: l.get(8)?,
                    base: l.get(9)?,
                    ordem: l.get(10)?,
                    historia: l.get(11)?,
                    compactada: l.get::<_, i64>(12)? != 0,
                    operacao_id: l.get(13)?,
                    incompativel: None,
                },
            ))
        })?;
        l.collect::<Result<_, _>>()?
    };
    if lista.is_empty() {
        return Ok(0);
    }
    let antes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente'",
        [],
        |l| l.get(0),
    )?;
    let alteracoes: Vec<Alteracao> = lista.iter().map(|(_, a)| a.clone()).collect();
    conn.pragma_update(None, "foreign_keys", false)?;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        aplicar_dentro(conn, &alteracoes, "", None, Modo::Reaplicar)
    }));
    if !conn.is_autocommit() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    let religou = conn.pragma_update(None, "foreign_keys", true);
    let r = match r {
        Ok(r) => r,
        Err(p) => std::panic::resume_unwind(p),
    };
    religou?;
    match r {
        Ok(_) => {
            let depois: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente'",
                [],
                |l| l.get(0),
            )?;
            Ok((antes - depois).max(0) as usize)
        }
        Err(Falha::Lote { indice, erro }) => {
            let agora = tempo::agora();
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE sync_pendencia SET tentativas = tentativas + 1, atualizado_em = ?1
                  WHERE lado = 'recebida' AND estado = 'pendente'",
                [&agora],
            )?;
            if let Some(a) = alteracoes.get(indice) {
                tx.execute(
                    "UPDATE sync_pendencia SET ultimo_erro = ?2 WHERE lado = 'recebida' AND alteracao_id = ?1",
                    params![a.id, erro],
                )?;
            }
            tx.commit()?;
            Ok(0)
        }
        Err(Falha::Outra(e)) => Err(e),
    }
}

/// Depois de juntar os dados dos caixas, confere o que nenhuma alteração
/// sozinha mostra:
/// - o mesmo orçamento virou venda em dois caixas sem rede (F-12): fica
///   anotado para o gerente cancelar a que sobrou;
/// - a mesma nota (chave da NF-e) entrou em mais de um caixa sem rede (G05,
///   passo 5.4): fica anotado para o gerente, do mesmo jeito;
/// - a loja ficou sem gerente (cada caixa tirou um, sem ver o outro, C-06):
///   o usuário mais antigo (quem configurou a loja) volta a ser gerente.
///   Todos os caixas escolhem o mesmo, então o resultado é igual em todos.
pub fn verificar_depois(conn: &Connection) -> Resultado<()> {
    let repetidos: Vec<(String, String)> = {
        let mut c = conn.prepare(
            "SELECT orcamento_id, group_concat(id, ',') FROM
               (SELECT orcamento_id, id FROM venda
                 WHERE tipo = 'venda' AND situacao = 'concluida' AND orcamento_id IS NOT NULL ORDER BY id)
              GROUP BY orcamento_id HAVING COUNT(*) > 1",
        )?;
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?;
        l.collect::<Result<_, _>>()?
    };
    for (orcamento, vendas) in repetidos {
        // Numa gravação só (N-F54-2): duas conferências ao mesmo tempo no
        // mesmo banco (o motor e uma troca pedida à mão) não anotam duas vezes.
        let primeira = vendas.split(',').next().unwrap_or_default().to_string();
        conn.execute(
            "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
             SELECT 'venda', ?1, ?2, ?3, '', ?4, 'orcamento'
              WHERE NOT EXISTS (SELECT 1 FROM sync_conflito WHERE tipo = 'orcamento' AND chave = ?1 AND perdeu = ?3)",
            params![orcamento, primeira, vendas, tempo::agora()],
        )?;
    }
    detectar_nota_repetida(conn)?;
    // R-03: parcela recebida (ou conta paga) mais de uma vez e vale-troca usado
    // além do valor, por caixas sem rede.
    detectar_pago_a_mais(conn)?;
    let (usuarios, gerentes): (i64, i64) =
        conn.query_row("SELECT COUNT(*), COALESCE(SUM(gerente = 1), 0) FROM usuario WHERE ativo = 1", [], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })?;
    if usuarios > 0 && gerentes == 0 {
        let id: String =
            conn.query_row("SELECT id FROM usuario WHERE ativo = 1 ORDER BY criado_em, id LIMIT 1", [], |l| l.get(0))?;
        // RC3-1: a volta do gerente só acontece se ainda não há nenhum (numa
        // instrução só), e o item só se ela aconteceu: duas conferências ao
        // mesmo tempo no mesmo banco não anotam duas vezes. RC4-4: e só se o
        // escolhido continua ativo (a troca pode tê-lo desativado no meio).
        let mudou = conn.execute(
            "UPDATE usuario SET gerente = 1, atualizado_em = ?2
              WHERE id = ?1 AND ativo = 1 AND NOT EXISTS (SELECT 1 FROM usuario WHERE ativo = 1 AND gerente = 1)",
            params![id, tempo::agora()],
        )?;
        if mudou == 1 {
            conn.execute(
                "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
                 VALUES ('usuario', ?1, '', '', '', ?2, 'gerente')",
                params![id, tempo::agora()],
            )?;
        }
    }
    Ok(())
}

/// G05 (passo 5.4): a mesma nota (chave da NF-e) em mais de uma compra não
/// estornada. No mesmo caixa a segunda importação é recusada
/// (`compras::importar_xml`); em caixas sem rede as duas entram, porque a
/// 0003 tirou a unicidade do banco de propósito (para os caixas trabalharem
/// sem rede). Vira um item para o gerente, como o orçamento vendido duas
/// vezes (F-12): sem "usar a do outro caixa", e nada é estornado nem
/// descartado sozinho. O gerente corrige pela tela de Compras (estornar a
/// lançada que sobrou, descartar o rascunho), o que já fica no histórico. O
/// mesmo conjunto de compras não volta a ser anotado; um conjunto novo (uma
/// terceira compra com a mesma chave) vira um item novo.
pub(super) fn detectar_nota_repetida(conn: &Connection) -> Resultado<()> {
    let repetidas: Vec<(String, String)> = {
        let mut c = conn.prepare(
            "SELECT chave, group_concat(id, ',') FROM
               (SELECT chave, id FROM compra WHERE chave <> '' AND situacao <> 'estornada' ORDER BY id)
              GROUP BY chave HAVING COUNT(*) > 1",
        )?;
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?;
        l.collect::<Result<_, _>>()?
    };
    for (chave, compras) in repetidas {
        // Numa gravação só (N-F54-2): duas conferências ao mesmo tempo no
        // mesmo banco não anotam duas vezes.
        let primeira = compras.split(',').next().unwrap_or_default().to_string();
        conn.execute(
            "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
             SELECT 'compra', ?1, ?2, ?3, '', ?4, 'nota'
              WHERE NOT EXISTS (SELECT 1 FROM sync_conflito WHERE tipo = 'nota' AND chave = ?1 AND perdeu = ?3)",
            params![chave, primeira, compras, tempo::agora()],
        )?;
    }
    Ok(())
}

/// R-03: o que só aparece depois de juntar os dados de caixas que ficaram sem
/// rede — cada um conferiu o saldo com o que sabia:
/// - parcela a receber recebida mais de uma vez (o pago passa do valor);
/// - conta a pagar paga mais de uma vez;
/// - vale-troca usado além do valor.
///
/// Vira um conflito para o gerente, com as operações envolvidas (nada é
/// apagado nem estornado sozinho: os pagamentos de cada caixa ficam). O
/// gerente corrige pela tela de cada coisa (estorno no Receber ou no Pagar,
/// cancelamento da venda), o que já fica no histórico; quando a conta deixa
/// de passar do valor, o conflito é dado por corrigido.
pub(super) fn detectar_pago_a_mais(conn: &Connection) -> Resultado<()> {
    let agora = tempo::agora();
    for (tabela, tipo, achar, operacoes) in CONTAS_A_MAIS {
        let ids: Vec<String> = conn.prepare(achar)?.query_map([], |l| l.get(0))?.collect::<Result<_, _>>()?;
        for id in ids {
            // Numa gravação só (RC3-1, como a N-F54-2): duas conferências ao
            // mesmo tempo no mesmo banco não anotam a mesma conta duas vezes.
            let ops: String = conn.query_row(operacoes, [&id], |l| l.get::<_, Option<String>>(0))?.unwrap_or_default();
            conn.execute(
                "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
                 SELECT ?1, ?2, '', ?3, '', ?4, ?5
                  WHERE NOT EXISTS (SELECT 1 FROM sync_conflito
                                     WHERE tipo = ?5 AND tabela = ?1 AND chave = ?2 AND resolvido_em IS NULL)",
                params![tabela, id, ops, agora, tipo],
            )?;
        }
    }
    // Os que o gerente já corrigiu (a conta não passa mais do valor).
    let abertos: Vec<(i64, String, String, String)> = conn
        .prepare("SELECT id, tipo, tabela, chave FROM sync_conflito WHERE tipo IN ('pago_a_mais', 'vale_a_mais') AND resolvido_em IS NULL")?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)))?
        .collect::<Result<_, _>>()?;
    for (id, tipo, tabela, chave) in abertos {
        if !ainda_a_mais(conn, &tipo, &tabela, &chave)? {
            conn.execute(
                "UPDATE sync_conflito SET resolvido_em = ?2, resolvido_por = NULL, resolucao = ?3 WHERE id = ?1",
                params![id, agora, TEXTO_CORRIGIDO],
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Passo 5.4, parte 1 (PEND-35): resumo por tabela, a conferência entre dois
// caixas em dia um com o outro.
// ---------------------------------------------------------------------------

/// Versão do cálculo do resumo: só se comparam resumos da mesma versão e das
/// mesmas classes ([`RECURSO_CLASSES`]); um caixa que não manda o resumo (de
/// versão anterior) simplesmente não compara (não troca menos dados).
pub const RESUMO_VERSAO: &str = "resumo-v1";

/// De quanto em quanto tempo um caixa compara o resumo com cada outro (o
/// cálculo lê inteiras as tabelas que viajam: medido em 0,37 s na loja de 20
/// mil peças, em release, sem segurar a escrita; frente F54).
pub(super) const RESUMO_INTERVALO: Duration = Duration::from_secs(10 * 60);

/// Linhas que cada banco cria sozinho ao nascer (pela estrutura, sem passar
/// pela troca), com o mesmo id em todos e a hora de cada computador: (tabela,
/// chave como no log). Só entram no resumo depois de viajar uma vez (alguém
/// mexeu nelas); antes, a diferença é só a hora em que cada banco nasceu.
pub(super) const NASCEM_EM_CADA_BANCO: &[(&str, &str)] = &[("conta_loja", "[\"caixa-geral\"]")];

/// Quando este caixa comparou o resumo com cada outro (banco, outro caixa).
static ULTIMO_RESUMO: Mutex<Vec<(PathBuf, String, Instant)>> = Mutex::new(Vec::new());

/// Está na hora de comparar o resumo com este outro caixa? (Uma vez a cada
/// [`RESUMO_INTERVALO`]; só nos testes, `sync_resumo_teste` = "sempre".)
pub(super) fn hora_do_resumo(conn: &Connection, caminho: &Path, outro: &str) -> bool {
    #[cfg(any(test, feature = "ancoras-de-teste"))]
    if config::ler(conn, "sync_resumo_teste").ok().flatten().as_deref() == Some("sempre") {
        return true;
    }
    let _ = conn;
    let lista = ULTIMO_RESUMO.lock().unwrap_or_else(|e| e.into_inner());
    !lista.iter().any(|(c, o, quando)| c == caminho && o == outro && quando.elapsed() < RESUMO_INTERVALO)
}

/// Anota que o resumo foi comparado agora com este outro caixa.
pub(super) fn resumo_comparado(caminho: &Path, outro: &str) {
    let mut lista = ULTIMO_RESUMO.lock().unwrap_or_else(|e| e.into_inner());
    lista.retain(|(c, o, _)| !(c == caminho && o == outro));
    lista.push((caminho.to_path_buf(), outro.to_string(), Instant::now()));
}

/// Contagem e soma de verificação de cada tabela que viaja: as colunas que
/// viajam em JSON (na ordem da tabela; na `config`, só as da loja), SHA-256 de
/// cada linha e a soma dos 8 primeiros bytes, módulo 2^64 (não depende da
/// ordem; uma linha a mais, a menos ou diferente muda a soma).
///
/// Na tabela com chave natural, só as colunas da chave natural (RC2-1): o
/// mesmo dado pode estar gravado em cada caixa com o seu id, a sua hora e a
/// sua ordem (o mesmo GTIN gravado pelas notas de cada caixa; ao receber, a
/// linha daqui fica, veja `dados::esquema::CHAVES_NATURAIS`), e só a chave
/// natural é igual nos caixas em dia.
pub fn resumo_das_tabelas(conn: &Connection) -> Resultado<Value> {
    let mut tabelas = serde_json::Map::new();
    for tabela in TABELAS {
        let (todas, chave) = colunas(conn, tabela)?;
        let cols: Vec<String> = match crate::dados::esquema::chave_natural(tabela) {
            Some(natural) => natural.iter().map(|c| format!("\"{c}\"")).collect(),
            None => que_viajam(tabela, &todas).into_iter().map(|c| format!("\"{c}\"")).collect(),
        };
        let chave_sql = chave.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let mut c = conn.prepare(&format!(
            "SELECT json_array({}), json_array({chave_sql}){} FROM \"{tabela}\"",
            cols.join(", "),
            if *tabela == "config" { ", chave" } else { ", ''" }
        ))?;
        let mut linhas = c.query([])?;
        let (mut n, mut soma) = (0u64, 0u64);
        while let Some(l) = linhas.next()? {
            if *tabela == "config" && !chave_da_loja(&l.get::<_, String>(2)?) {
                continue;
            }
            let k: String = l.get(1)?;
            if NASCEM_EM_CADA_BANCO.iter().any(|(t, ch)| t == tabela && *ch == k) {
                let viajou: bool = conn
                    .query_row(
                        "SELECT 1 FROM alteracao WHERE tabela = ?1 AND chave = ?2 LIMIT 1",
                        params![tabela, k],
                        |_| Ok(true),
                    )
                    .optional()?
                    .unwrap_or(false);
                if !viajou {
                    continue;
                }
            }
            let h = Sha256::digest(l.get::<_, String>(0)?.as_bytes());
            let mut oito = [0u8; 8];
            oito.copy_from_slice(&h[..8]);
            soma = soma.wrapping_add(u64::from_le_bytes(oito));
            n += 1;
        }
        tabelas.insert(tabela.to_string(), json!([n, format!("{soma:016x}")]));
    }
    Ok(Value::Object(tabelas))
}

/// O resumo deste caixa e onde ele está em relação ao `outro` caixa, lidos
/// juntos numa leitura (no modo WAL a leitura não segura a escrita: o balcão
/// grava enquanto isso): a numeração deste caixa, até onde ele já recebeu do
/// outro e quantas alterações recebidas estão guardadas sem aplicar.
pub(super) fn resumo_agora(conn: &Connection, outro: &str) -> Resultado<Value> {
    let dentro = !conn.is_autocommit();
    if !dentro {
        conn.execute_batch("BEGIN")?;
    }
    let r = (|| -> Resultado<Value> {
        let seq = seq_maxima(conn)?;
        let cursor: i64 = conn
            .query_row("SELECT cursor FROM sync_par WHERE dispositivo = ?1", [outro], |l| l.get(0))
            .optional()?
            .unwrap_or(0);
        let pendentes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_pendencia WHERE lado = 'recebida' AND estado = 'pendente'",
            [],
            |l| l.get(0),
        )?;
        Ok(json!({ "versao": RESUMO_VERSAO, "classes": RECURSO_CLASSES, "seqMaxima": seq, "cursorDoOutro": cursor,
                   "pendencias": pendentes, "tabelas": resumo_das_tabelas(conn)? }))
    })();
    if !dentro {
        let _ = conn.execute_batch("COMMIT");
    }
    r
}

/// Dá para comparar os dois resumos? Mesma versão e mesmas classes, os dois
/// caixas em dia um com o outro (cada um já recebeu tudo o que o outro tinha
/// quando tirou o resumo) e nada guardado sem aplicar (uma pendência é uma
/// diferença conhecida, já mostrada ao gerente).
pub(super) fn resumos_comparaveis(meu: &Value, dele: &Value) -> bool {
    meu["versao"] == dele["versao"]
        && meu["classes"] == dele["classes"]
        && meu["seqMaxima"].is_i64()
        && meu["cursorDoOutro"] == dele["seqMaxima"]
        && dele["cursorDoOutro"] == meu["seqMaxima"]
        && meu["pendencias"].as_i64() == Some(0)
        && dele["pendencias"].as_i64() == Some(0)
}

/// Compara o resumo deste caixa com o do `outro` (só quando dá: veja
/// [`resumos_comparaveis`]). Cada tabela diferente vira um item para o gerente
/// em Simultaneidade › Diferenças ("Conferência entre caixas"), uma vez para
/// cada diferença. Não decide nada: nenhum dado é mudado. Devolve quantas
/// tabelas diferem.
pub(super) fn comparar_resumos(conn: &Connection, outro: &str, meu: &Value, dele: &Value) -> Resultado<usize> {
    if !resumos_comparaveis(meu, dele) {
        return Ok(0);
    }
    let mut diferentes = 0;
    for tabela in TABELAS {
        let (aqui, la) = (&meu["tabelas"][*tabela], &dele["tabelas"][*tabela]);
        if la.is_null() || aqui == la {
            continue;
        }
        diferentes += 1;
        let marca = format!(
            "{}|{}|{}|{}",
            aqui[0].as_u64().unwrap_or(0),
            aqui[1].as_str().unwrap_or(""),
            la[0].as_u64().unwrap_or(0),
            la[1].as_str().unwrap_or("")
        );
        // Numa gravação só: este caixa pode comparar com o mesmo outro como
        // quem pede e como quem responde ao mesmo tempo.
        conn.execute(
            "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)
             SELECT ?1, ?2, '', ?3, ?2, ?4, 'resumo'
              WHERE NOT EXISTS (SELECT 1 FROM sync_conflito
                                 WHERE tipo = 'resumo' AND tabela = ?1 AND chave = ?2 AND perdeu = ?3)",
            params![tabela, outro, marca, tempo::agora()],
        )?;
    }
    Ok(diferentes)
}
