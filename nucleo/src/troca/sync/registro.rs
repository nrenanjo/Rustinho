//! As tabelas que viajam, os gatilhos, a fila de alterações, as faixas e `aplicar_de`.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// Tabelas que os caixas compartilham: as de classe `Viaja` do `dados::esquema`,
/// na ordem de lá (a mesma de antes; dev.3.3, passo 3.2).
pub const TABELAS: &[&str] = &crate::dados::esquema::QUE_VIAJAM;

/// Configurações que valem para a loja toda. As outras (nome deste caixa,
/// senha da rede...) são de cada computador.
pub const CONFIG_DA_LOJA: &[&str] = &[
    "loja_nome",
    "loja_uf",
    "loja_cnpj",
    "loja_endereco",
    "loja_telefone",
    "crediario_multa",
    "crediario_juros",
    "crediario_carencia",
    "orcamento_validade",
    "rodape_venda",
    "rodape_orcamento",
    "rodape_os",
    "imprimir_local",
    "estoque_dias_compra",
    "estoque_dias_parado",
    // "backup_senha" (até a 0.9.1) e "backup_chave" (rascunho da 0.9.11)
    // ficam na lista para o apagar chegar aos outros caixas.
    "backup_senha",
    "backup_chave",
    // Parte pública da chave do backup (a secreta só vai trancada, no usuário).
    "backup_publica",
    "gerente_livres",
    // O caixa que dá as faixas de códigos das peças.
    "codigos_distribuidor",
    // O gerente confirmou a classificação das categorias na DRE.
    "dre_classificacao_confirmada",
    // Identidade da loja (FALHA-01, veja `loja`).
    "loja_id",
];

// ---------------------------------------------------------------------------
// Gatilhos e registro das alterações.
// ---------------------------------------------------------------------------

/// (todas as colunas, colunas da chave primária) de uma tabela compartilhada.
pub(crate) fn colunas_da_tabela(conn: &Connection, tabela: &str) -> Resultado<(Vec<String>, Vec<String>)> {
    colunas(conn, tabela)
}

pub(super) fn colunas(conn: &Connection, tabela: &str) -> Resultado<(Vec<String>, Vec<String>)> {
    let mut c = conn.prepare(&format!("PRAGMA table_info(\"{tabela}\")"))?;
    let mut linhas: Vec<(String, i64)> = c.query_map([], |l| Ok((l.get(1)?, l.get(5)?)))?.collect::<Result<_, _>>()?;
    let todas: Vec<String> = linhas.iter().map(|l| l.0.clone()).collect();
    linhas.retain(|l| l.1 > 0);
    linhas.sort_by_key(|l| l.1);
    Ok((todas, linhas.into_iter().map(|l| l.0).collect()))
}

/// As colunas da tabela que viajam: todas, menos as que o `dados::esquema`
/// diz que não viajam (derivadas e locais; passo 5.2, DP-15).
pub(super) fn que_viajam(tabela: &str, todas: &[String]) -> Vec<String> {
    todas.iter().filter(|c| crate::dados::esquema::coluna_viaja(tabela, c)).cloned().collect()
}

pub(super) fn expressoes(tabela: &str, todas: &[String], chave: &[String], prefixo: &str) -> (String, String) {
    let chave =
        format!("json_array({})", chave.iter().map(|c| format!("{prefixo}.\"{c}\"")).collect::<Vec<_>>().join(", "));
    let dados = format!(
        "json_object({})",
        todas.iter().map(|c| format!("'{c}', {prefixo}.\"{c}\"")).collect::<Vec<_>>().join(", ")
    );
    let _ = tabela;
    (chave, dados)
}

pub(super) fn filtro_config(prefixo: &str) -> String {
    format!(
        " AND ({prefixo}.chave IN ({}) OR {prefixo}.chave LIKE '{PREFIXO_RETIRADO}%')",
        CONFIG_DA_LOJA.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ")
    )
}

/// A configuração é da loja (vai para todos os caixas)?
pub fn chave_da_loja(chave: &str) -> bool {
    CONFIG_DA_LOJA.contains(&chave) || chave.starts_with(PREFIXO_RETIRADO)
}

/// (Re)cria os gatilhos que registram as alterações. Roda ao abrir o banco.
///
/// Gerados do `dados::esquema` (passo 5.2, ARQ-10): as tabelas que viajam e,
/// de cada uma, só as colunas que viajam. Numa tabela com coluna que não
/// viaja, a alteração só é registrada quando alguma coluna que viaja está no
/// `UPDATE` (`UPDATE OF`), e o conteúdo dela não leva a que não viaja.
///
/// O-1 (passo 5.2): um `UPDATE` que regrava a linha com o mesmo conteúdo (as
/// colunas que viajam iguais) não grava alteração: nada viaja à toa, e a
/// regravação não vira conflito com a mudança feita em outro caixa.
pub fn criar_gatilhos(conn: &Connection) -> Resultado<()> {
    // 0.9.15: cada alteração leva a operação comercial (a transação) em que
    // foi feita. Banco de estrutura antiga (só nos testes) não tem a coluna.
    let com_operacao: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('alteracao') WHERE name = 'operacao_id'",
        [],
        |l| l.get(0),
    )?;
    let (col_op, val_op) = if com_operacao { (", operacao_id", ", rustinho_operacao()") } else { ("", "") };
    for tabela in TABELAS {
        let (todas, chave) = colunas(conn, tabela)?;
        if chave.is_empty() {
            return Err(Erro::Interno(format!("a tabela {tabela} não tem chave primária")));
        }
        let viajam = que_viajam(tabela, &todas);
        // Sem coluna que não viaja, o gatilho fica como sempre foi.
        let de_update = if viajam.len() < todas.len() {
            format!("UPDATE OF {}", viajam.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", "))
        } else {
            "UPDATE".to_string()
        };
        let quando = "WHEN (SELECT aplicando FROM sync_estado WHERE id = 1) = 0";
        let disp = "(SELECT valor FROM config WHERE chave = 'dispositivo_id')";
        // A versão que vale agora deste registro (a base da nova).
        let ultima = |k: &str, coluna: &str| {
            format!(
                "(SELECT b.{coluna} FROM alteracao b WHERE b.tabela = '{tabela}' AND b.chave = {k} AND b.compactada = 0
                   ORDER BY b.ordem DESC, b.id DESC LIMIT 1)"
            )
        };
        let mut sql = String::new();
        for (sufixo, evento, prefixo, operacao) in
            [("i", "INSERT", "NEW", "gravar"), ("u", "UPDATE", "NEW", "gravar"), ("d", "DELETE", "OLD", "apagar")]
        {
            let (k, d) = expressoes(tabela, &viajam, &chave, prefixo);
            let dados = if operacao == "apagar" { "NULL".to_string() } else { d };
            let evento = if evento == "UPDATE" { de_update.as_str() } else { evento };
            let mudou = if sufixo == "u" {
                let cols: Vec<String> = viajam.iter().map(|c| format!("OLD.\"{c}\" IS NOT NEW.\"{c}\"")).collect();
                format!(" AND ({})", cols.join(" OR "))
            } else {
                String::new()
            };
            let extra = if *tabela == "config" { filtro_config(prefixo) } else { String::new() };
            let (base, ordem, historia, base_da_base) =
                (ultima(&k, "id"), ultima(&k, "ordem"), ultima(&k, "historia"), ultima(&k, "base"));
            sql.push_str(&format!(
                "DROP TRIGGER IF EXISTS sync_{tabela}_{sufixo};
                 CREATE TRIGGER sync_{tabela}_{sufixo} AFTER {evento} ON \"{tabela}\" {quando}{extra}{mudou}
                 BEGIN
                   INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia{col_op})
                   VALUES (rustinho_id(), '{tabela}', {k}, '{operacao}', {dados}, {disp}, rustinho_agora(),
                           {base}, COALESCE({ordem}, 0) + 1, rustinho_historia({base}, {historia}, {base_da_base}){val_op});
                 END;"
            ));
        }
        conn.execute_batch(&sql)?;
    }
    Ok(())
}

/// Registra como alteração tudo que já existia antes da sincronização (só
/// uma vez). Num caixa novo, sem usuários, não precisa: ele vai receber
/// tudo dos outros.
pub fn registrar_existentes(conn: &Connection) -> Resultado<()> {
    if config::ler(conn, "sync_inicio")?.is_some() {
        return registrar_as_que_passaram_a_viajar(conn);
    }
    let usuarios: i64 = conn.query_row("SELECT COUNT(*) FROM usuario", [], |l| l.get(0))?;
    if usuarios > 0 {
        for tabela in TABELAS {
            registrar_linhas(conn, tabela)?;
        }
    }
    config::gravar(conn, "sync_inicio", &tempo::agora())?;
    for tabela in crate::dados::esquema::PASSARAM_A_VIAJAR {
        config::gravar(conn, &format!("sync_inicio:{tabela}"), &tempo::agora())?;
    }
    Ok(())
}

/// Registra como alteração cada linha que a tabela tem agora.
fn registrar_linhas(conn: &Connection, tabela: &str) -> Resultado<()> {
    let (todas, chave) = colunas(conn, tabela)?;
    let (k, d) = expressoes(tabela, &que_viajam(tabela, &todas), &chave, "t");
    let filtro = if tabela == "config" { format!(" WHERE 1 = 1{}", filtro_config("t")) } else { String::new() };
    conn.execute(
        &format!(
            "INSERT INTO alteracao (id, tabela, chave, operacao, dados, dispositivo, quando)
             SELECT rustinho_id(), '{tabela}', {k}, 'gravar', {d},
                    (SELECT valor FROM config WHERE chave = 'dispositivo_id'), rustinho_agora()
               FROM \"{tabela}\" t{filtro}"
        ),
        [],
    )?;
    Ok(())
}

/// A marca de que as linhas de uma tabela que passou a viajar já viraram
/// alteração neste banco (`sync_inicio:<tabela>`, passo 5.2)? Ela é do banco,
/// e não do caixa (N-I1, integração 2: um backup de antes da 5.2 restaurado
/// volta sem ela, e a abertura seguinte registra as linhas dele; veja
/// `backup::Pendente`).
pub fn marca_das_que_passaram_a_viajar(chave: &str) -> bool {
    crate::dados::esquema::PASSARAM_A_VIAJAR.iter().any(|t| chave.strip_prefix("sync_inicio:") == Some(*t))
}

/// Passo 5.2 (PEND-35): num banco de antes, as linhas das tabelas que
/// passaram a viajar nunca viraram alteração. Viram agora, uma vez só (a
/// marca `sync_inicio:<tabela>` fica na configuração deste caixa, junto, numa
/// gravação só). O mesmo dado já gravado nos outros caixas com outro id é
/// reconhecido lá pela chave natural (veja `dados::esquema::CHAVES_NATURAIS`).
///
/// R-1 (revisão cruzada de 13/11, 7.5 × 5.2): se a tabela alimenta um
/// derivado (o `produto_gtin` alimenta o índice da peça), o derivado de todas
/// as linhas donas é refeito junto, uma vez. Antes da 5.2 a troca
/// sobrescrevia o índice da peça com o do outro caixa (D01): o GTIN só deste
/// caixa ficava fora dele, e com a 7.5 o pedaço do GTIN só acha pelo índice.
/// Refazer só mexe nas colunas derivadas (não viajam): não grava alteração.
fn registrar_as_que_passaram_a_viajar(conn: &Connection) -> Resultado<()> {
    for tabela in crate::dados::esquema::PASSARAM_A_VIAJAR {
        let marca = format!("sync_inicio:{tabela}");
        if config::ler(conn, &marca)?.is_some() {
            continue;
        }
        conn.execute_batch("SAVEPOINT passou_a_viajar")?;
        let r = registrar_linhas(conn, tabela)
            .and_then(|_| match crate::dados::esquema::alimenta(tabela) {
                Some((_, derivado)) => super::aplicacao::refazer_derivado_de_todas(conn, derivado).map(|_| ()),
                None => Ok(()),
            })
            .and_then(|_| config::gravar(conn, &marca, &tempo::agora()));
        match r {
            Ok(()) => conn.execute_batch("RELEASE passou_a_viajar")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK TO passou_a_viajar; RELEASE passou_a_viajar");
                return Err(e);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Ler e aplicar alterações.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alteracao {
    pub seq: i64,
    pub id: String,
    pub tabela: String,
    pub chave: String,
    pub operacao: String,
    pub dados: Option<String>,
    pub dispositivo: String,
    pub quando: String,
    /// A versão do registro que o caixa conhecia quando mexeu (id da
    /// alteração). Caixas de antes da 0.9.11 não mandam.
    #[serde(default)]
    pub base: Option<String>,
    /// Contador do registro (base + 1). Vale a de maior ordem.
    #[serde(default)]
    pub ordem: i64,
    /// Versões anteriores conhecidas (JSON, a base primeiro; "+" no fim =
    /// não chega ao começo).
    #[serde(default)]
    pub historia: Option<String>,
    /// Versão velha sem os dados (o log foi compactado): serve só para a
    /// cadeia de versões; nunca é aplicada.
    #[serde(default)]
    pub compactada: bool,
    /// A operação comercial (transação) em que foi feita: as alterações de
    /// uma mesma venda, com itens e pagamentos, têm o mesmo. Vazio nas de
    /// antes da 0.9.15. Caixa antigo ignora o campo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operacao_id: Option<String>,
    /// Posto por quem manda: o caixa que vai receber não consegue gravar
    /// esta alteração (versão antiga). Ele a guarda em `sync_pendencia`, sem
    /// aplicar. Nunca vem de um caixa antigo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incompativel: Option<String>,
}

pub(super) const COLUNAS_ALTERACAO: &str =
    "seq, id, tabela, chave, operacao, dados, dispositivo, quando, base, ordem, historia, compactada, operacao_id";

pub(super) fn ler_alteracao(l: &rusqlite::Row) -> rusqlite::Result<Alteracao> {
    Ok(Alteracao {
        seq: l.get(0)?,
        id: l.get(1)?,
        tabela: l.get(2)?,
        chave: l.get(3)?,
        operacao: l.get(4)?,
        dados: l.get(5)?,
        dispositivo: l.get(6)?,
        quando: l.get(7)?,
        base: l.get(8)?,
        ordem: l.get(9)?,
        historia: l.get(10)?,
        compactada: l.get::<_, i64>(11)? != 0,
        operacao_id: l.get(12)?,
        incompativel: None,
    })
}

/// Alterações deste banco depois da número `desde`, no máximo `limite`.
///
/// Passo 5.4 (PEND-35): a lista termina no fim de uma operação comercial (as
/// alterações gravadas numa mesma transação: a venda com itens, pagamentos e
/// movimentos). Quem recebe aplica cada resposta numa transação; se ela
/// cortasse uma venda no meio, o outro caixa a mostraria pela metade até a
/// resposta seguinte. A lista nunca passa do `limite` (o achado do t2_5:
/// cada resposta segura a escrita de quem recebe): a operação que não cabe
/// inteira fica para a resposta seguinte. Uma operação sozinha maior que o
/// `limite` vai em partes, como antes (decisão D54-2).
pub fn alteracoes_desde(conn: &Connection, desde: i64, limite: i64) -> Resultado<(Vec<Alteracao>, bool)> {
    let mut c = conn
        .prepare_cached(&format!("SELECT {COLUNAS_ALTERACAO} FROM alteracao WHERE seq > ?1 ORDER BY seq LIMIT ?2"))?;
    let mut lista: Vec<Alteracao> =
        c.query_map(params![desde, limite + 1], ler_alteracao)?.collect::<Result<_, _>>()?;
    let tem_mais = lista.len() as i64 > limite;
    lista.truncate(limite.max(0) as usize);
    if tem_mais {
        let ate = sem_operacao_pela_metade(conn, &lista)?;
        lista.truncate(ate);
    }
    Ok((lista, tem_mais))
}

/// Quantas alterações do começo da `lista` podem ir sem deixar operação
/// comercial pela metade: nenhuma operação que está na lista tem alteração
/// depois da última que vai. Se nem a primeira operação cabe inteira (uma
/// operação maior que a resposta), vai a lista toda (D54-2).
pub(super) fn sem_operacao_pela_metade(conn: &Connection, lista: &[Alteracao]) -> Resultado<usize> {
    // A última alteração de cada operação da lista, neste log.
    let mut ultima: HashMap<&str, i64> = HashMap::new();
    let mut c = conn.prepare_cached("SELECT MAX(seq) FROM alteracao WHERE operacao_id = ?1")?;
    for a in lista {
        if let Some(o) = a.operacao_id.as_deref() {
            if !ultima.contains_key(o) {
                let fim: Option<i64> = c.query_row([o], |l| l.get(0))?;
                ultima.insert(o, fim.unwrap_or(a.seq));
            }
        }
    }
    let mut ate = lista.len();
    while ate > 0 {
        let fim = lista[ate - 1].seq;
        let cortada = lista[..ate].iter().position(|a| a.operacao_id.as_deref().is_some_and(|o| ultima[o] > fim));
        match cortada {
            Some(i) => ate = i,
            None => return Ok(ate),
        }
    }
    Ok(lista.len())
}

/// Resposta do distribuidor a um pedido de faixa de códigos das peças: as
/// alterações do log com as faixas (as mesmas que ele gravou), ou o motivo
/// da recusa.
pub fn responder_faixa(conn: &mut Connection, pedido: &Value) -> Resultado<Value> {
    let eu = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let dono = pedido["dispositivo"].as_str().unwrap_or_default();
    let geracao_dono = pedido["geracao"].as_str().unwrap_or_default();
    let nome_dono = pedido["nome"].as_str().unwrap_or_default();
    let tenho: Vec<i64> = pedido["tenho"]
        .as_array()
        .map(|l| l.iter().filter_map(|v| v.as_i64()).take(10_000).collect())
        .unwrap_or_default();
    Ok(match crate::faixas::atender_pedido(conn, dono, geracao_dono, nome_dono, &tenho) {
        Ok(chaves) => json!({ "protocolo": PROTOCOLO, "dispositivo": eu,
                              "alteracoes": alteracoes_das_faixas(conn, &chaves)? }),
        Err(Erro::Validacao { mensagem, .. }) => {
            json!({ "protocolo": PROTOCOLO, "dispositivo": eu, "recusado": mensagem })
        }
        Err(e) => return Err(e),
    })
}

/// O caixa que pediu grava as faixas que vieram (como alterações do log,
/// com o mesmo id). Devolve quantas eram novas.
pub fn gravar_faixa_recebida(conn: &mut Connection, resposta: &Value) -> Resultado<usize> {
    if resposta["protocolo"].as_i64() != Some(PROTOCOLO) {
        return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
    }
    if let Some(m) = resposta["recusado"].as_str() {
        return Err(Erro::validacao(m));
    }
    let lista: Vec<Alteracao> = serde_json::from_value(resposta["alteracoes"].clone())?;
    let lista: Vec<Alteracao> = lista.into_iter().filter(|x| x.tabela == "faixa_codigo").collect();
    aplicar(conn, &lista)
}

/// As alterações (com dados) das faixas de códigos pedidas, para a
/// resposta do pedido de faixa.
pub(super) fn alteracoes_das_faixas(conn: &Connection, chaves: &[(i64, String, String)]) -> Resultado<Vec<Alteracao>> {
    let mut c = conn.prepare_cached(&format!(
        "SELECT {COLUNAS_ALTERACAO} FROM alteracao
          WHERE tabela = 'faixa_codigo' AND chave = json_array(?1, ?2, ?3) AND compactada = 0 ORDER BY seq"
    ))?;
    let mut lista = Vec::new();
    for (inicio, dono, geracao) in chaves {
        for a in c.query_map(params![inicio, dono, geracao], ler_alteracao)? {
            lista.push(a?);
        }
    }
    Ok(lista)
}

pub(super) fn para_sql(v: &Value) -> SqlValor {
    match v {
        Value::Null => SqlValor::Null,
        Value::Bool(b) => SqlValor::Integer(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => SqlValor::Integer(i),
            None => SqlValor::Real(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => SqlValor::Text(s.clone()),
        outro => SqlValor::Text(outro.to_string()),
    }
}

/// Aplica alterações vindas de outro caixa. Devolve quantas eram novas.
pub fn aplicar(conn: &mut Connection, lista: &[Alteracao]) -> Resultado<usize> {
    Ok(aplicar_de(conn, lista, "", None)?.novas)
}

/// Onde a numeração do outro caixa vai parar depois deste lote (atualizada
/// na mesma transação que grava o lote).
pub struct AvancoCursor<'a> {
    pub dispositivo: &'a str,
    /// O contador lido antes de pedir (só avança se ninguém o zerou no meio).
    pub de: i64,
    pub para: i64,
    pub marca: &'a str,
    pub geracao: &'a str,
}

/// O que aconteceu com um lote.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Aplicacao {
    /// Alterações novas gravadas no log deste caixa.
    pub novas: usize,
    /// Alterações guardadas em `sync_pendencia` (não entraram nas tabelas).
    pub pendentes: usize,
    /// A numeração do outro caixa avançou.
    pub avancou: bool,
    /// O lote todo foi desfeito e guardado como pendência (erro ao aplicar).
    pub suspenso: bool,
}

/// Aplica um lote vindo do caixa `origem`. Regras (0.9.15):
/// - o lote inteiro numa transação, com a numeração do outro caixa (`cursor`)
///   avançada **dentro** dela: ou fica tudo, ou nada;
/// - alteração que este caixa não consegue gravar (tabela que não conhece,
///   marcada incompatível por quem mandou, dados estragados) não é jogada
///   fora: fica inteira em `sync_pendencia`, e a numeração avança sem perdê-la;
/// - alteração que depende de outra pendente (o mesmo registro, a mesma
///   operação comercial ou um registro pai pendente) também fica pendente;
/// - erro ao gravar (regra do banco): o lote inteiro é desfeito; depois, numa
///   transação nova, o lote todo é guardado como pendência e a numeração
///   avança. A troca continua com os lotes seguintes (os que dependem do
///   suspenso ficam pendentes também). Nada é aplicado pela metade.
/// - registro cujo pai ainda não chegou continua gravado e acompanhado como
///   antes (R-05).
pub fn aplicar_de(
    conn: &mut Connection,
    lista: &[Alteracao],
    origem: &str,
    cursor: Option<AvancoCursor>,
) -> Resultado<Aplicacao> {
    // Na ordem em que chegam, um item pode vir antes da peça (ou a peça ter
    // sido apagada depois). As chaves estrangeiras ficam desligadas aqui e
    // são conferidas no fim da troca (veja `pendencias`).
    conn.pragma_update(None, "foreign_keys", false)?;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match aplicar_dentro(conn, lista, origem, cursor.as_ref(), Modo::Normal) {
            Ok(a) => Ok(a),
            Err(Falha::Lote { indice, erro }) => suspender_lote(conn, lista, origem, indice, &erro, cursor.as_ref()),
            Err(Falha::Outra(e)) => Err(e),
        }
    }));
    // Religa em qualquer caminho (deu certo, deu erro ou pânico).
    if !conn.is_autocommit() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    let religou = conn.pragma_update(None, "foreign_keys", true);
    let r = match r {
        Ok(r) => r,
        Err(p) => std::panic::resume_unwind(p),
    };
    religou?;
    r
}
