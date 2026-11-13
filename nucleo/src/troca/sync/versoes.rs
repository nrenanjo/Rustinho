//! Versões, linhagem e descendência; anotar e fechar conflitos.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// Uma versão de um registro no log de alterações.
#[derive(Debug, Clone)]
pub(super) struct Versao {
    pub(super) id: String,
    pub(super) dispositivo: String,
    pub(super) base: Option<String>,
    pub(super) dados: Option<String>,
    pub(super) ordem: i64,
    pub(super) historia: Option<String>,
}

impl Versao {
    /// Vale mais que `outra`? Maior ordem; empate, maior id. Não depende da
    /// ordem de chegada: todos os caixas escolhem a mesma.
    pub(super) fn vale_mais_que(&self, outra: &Versao) -> bool {
        (self.ordem, self.id.as_str()) > (outra.ordem, outra.id.as_str())
    }
}

/// Colunas que mudam sozinhas (hora da última entrada, carimbo de
/// alteração, cadeado do backup) e não contam como conflito.
/// (`dispositivo` diz só quem mexeu por último; `busca` sai do nome.)
pub(super) const SEM_CONFLITO: &[&str] = &["atualizado_em", "ultimo_acesso", "backup_acesso", "dispositivo", "busca"];

/// Colunas que diferem entre duas versões (sem as que mudam sozinhas e sem
/// as que não viajam: derivadas e locais do `dados::esquema`, passo 5.2).
pub fn diferencas(a: Option<&str>, b: Option<&str>) -> Vec<String> {
    let ler = |t: Option<&str>| -> Option<serde_json::Map<String, Value>> {
        t.and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|v| v.as_object().cloned())
    };
    match (ler(a), ler(b)) {
        (Some(x), Some(y)) => {
            let mut cols: Vec<String> = x
                .keys()
                .chain(y.keys())
                .filter(|k| {
                    !SEM_CONFLITO.contains(&k.as_str())
                        && !crate::dados::esquema::nome_que_nao_viaja(k)
                        && x.get(*k) != y.get(*k)
                })
                .cloned()
                .collect();
            cols.sort();
            cols.dedup();
            cols
        }
        // Um apagou e o outro mexeu (ou os dois apagaram).
        (None, None) => Vec::new(),
        _ => vec!["(apagado)".into()],
    }
}

/// História de uma alteração nova feita sobre `base`: a base e o que a base
/// conhecia, até [`HISTORIA_MAXIMA`]; "+" no fim se não chega ao começo. Sem
/// base (registro novo): `[]`. Usada pelos gatilhos (`rustinho_historia`).
pub fn historia_nova(base: Option<&str>, historia_base: Option<&str>, base_da_base: Option<&str>) -> String {
    let Some(base) = base else { return "[]".into() };
    let anterior = linhagem(base_da_base, historia_base);
    let mut ids = vec![base.to_string()];
    ids.extend(anterior.ids);
    let mut completa = anterior.completa;
    if ids.len() > HISTORIA_MAXIMA {
        ids.truncate(HISTORIA_MAXIMA);
        completa = false;
    }
    if !completa {
        ids.push(INCOMPLETA.into());
    }
    serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())
}

/// As versões anteriores de uma versão, e se a lista chega ao começo.
pub(super) struct Linhagem {
    pub(super) ids: Vec<String>,
    pub(super) completa: bool,
}

pub(super) fn linhagem(base: Option<&str>, historia: Option<&str>) -> Linhagem {
    match historia.and_then(|h| serde_json::from_str::<Vec<String>>(h).ok()) {
        Some(lista) => {
            let completa = !lista.iter().any(|x| x == INCOMPLETA);
            let mut ids: Vec<String> = lista.into_iter().filter(|x| x != INCOMPLETA).collect();
            if let Some(b) = base {
                if !ids.iter().any(|x| x == b) {
                    ids.insert(0, b.to_string());
                }
            }
            Linhagem { ids, completa }
        }
        // Sem história: registro novo (sem base) ou alteração de antes do
        // protocolo 3 (só a base; o resto não se sabe).
        None => match base {
            None => Linhagem { ids: Vec::new(), completa: true },
            Some(b) => Linhagem { ids: vec![b.to_string()], completa: false },
        },
    }
}

/// Como uma versão está para outra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Relacao {
    /// Foi feita depois (a outra está no caminho de versões dela).
    Depois,
    /// Foi feita antes (está no caminho da outra).
    Antes,
    /// Com certeza nenhuma está no caminho da outra: feitas sem uma ver a
    /// outra.
    Concorrente,
    /// O caminho de versões está incompleto neste caixa (alteração de antes
    /// do protocolo 3, por exemplo): não dá para saber. Conta como conflito
    /// (R-08: nada decidido em silêncio).
    Incerta,
}

/// `x` foi feita em cima de `alvo` (direto ou depois de outras)?
/// `Some(true)`: sim; `Some(false)`: com certeza não; `None`: não dá para
/// saber (falta uma versão no caminho).
///
/// Primeiro olha a história que vem na própria alteração; senão segue as
/// bases no log (as versões compactadas continuam lá, sem dados). A ordem
/// para a busca cedo: uma versão anterior sempre tem ordem menor.
pub(super) fn descende(conn: &Connection, x: &Versao, alvo: &Versao) -> Resultado<Option<bool>> {
    if linhagem(x.base.as_deref(), x.historia.as_deref()).ids.contains(&alvo.id) {
        return Ok(Some(true));
    }
    // Ordem 0 = versão de antes do protocolo 3 (sem contador): não serve.
    let ordem_vale = |o: i64| o > 0 && alvo.ordem > 0;
    if ordem_vale(x.ordem) && x.ordem <= alvo.ordem {
        return Ok(Some(false));
    }
    let mut c = conn.prepare_cached("SELECT base, ordem FROM alteracao WHERE id = ?1")?;
    let mut atual = x.base.clone();
    let mut passos = 0u32;
    while let Some(id) = atual {
        if id == alvo.id {
            return Ok(Some(true));
        }
        let Some((base, ordem)) =
            c.query_row([&id], |l| Ok((l.get::<_, Option<String>>(0)?, l.get::<_, i64>(1)?))).optional()?
        else {
            return Ok(None);
        };
        if ordem_vale(ordem) && ordem <= alvo.ordem {
            return Ok(Some(false));
        }
        passos += 1;
        if passos > 1_000_000 {
            return Ok(None);
        }
        atual = base;
    }
    // Chegou ao começo do registro sem passar por `alvo`.
    Ok(Some(false))
}

pub(super) fn relacao(conn: &Connection, x: &Versao, y: &Versao) -> Resultado<Relacao> {
    let depois = descende(conn, x, y)?;
    if depois == Some(true) {
        return Ok(Relacao::Depois);
    }
    let antes = descende(conn, y, x)?;
    if antes == Some(true) {
        return Ok(Relacao::Antes);
    }
    Ok(if depois == Some(false) && antes == Some(false) { Relacao::Concorrente } else { Relacao::Incerta })
}

/// A versão que vale agora de um registro, neste caixa.
pub(super) fn versao_atual(conn: &Connection, tabela: &str, chave: &str) -> Resultado<Option<Versao>> {
    Ok(conn
        .query_row(
            "SELECT id, dispositivo, base, dados, ordem, historia FROM alteracao
              WHERE tabela = ?1 AND chave = ?2 AND compactada = 0 ORDER BY ordem DESC, id DESC LIMIT 1",
            params![tabela, chave],
            |l| {
                Ok(Versao {
                    id: l.get(0)?,
                    dispositivo: l.get(1)?,
                    base: l.get(2)?,
                    dados: l.get(3)?,
                    ordem: l.get(4)?,
                    historia: l.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Alguma versão deste registro que está no log foi feita em cima de
/// `id` (ele está na base ou na história dela)? Então `id` não se perdeu: o
/// caixa que a fez (ou outro) mexeu de novo depois.
pub(super) fn alguem_partiu_de(conn: &Connection, tabela: &str, chave: &str, id: &str) -> Resultado<bool> {
    let mut c = conn.prepare_cached(
        "SELECT base, historia FROM alteracao WHERE tabela = ?1 AND chave = ?2 AND id <> ?3
            AND (base = ?3 OR instr(historia, ?4) > 0)",
    )?;
    let l = c.query_map(params![tabela, chave, id, format!("\"{id}\"")], |l| {
        Ok((l.get::<_, Option<String>>(0)?, l.get::<_, Option<String>>(1)?))
    })?;
    for v in l {
        let (base, historia) = v?;
        if linhagem(base.as_deref(), historia.as_deref()).ids.iter().any(|i| i == id) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Anota o conflito (uma vez por versão que perdeu: reenvio não repete).
/// Só a última versão de cada caminho conta: se outra versão já foi feita
/// em cima da que perdeu, não é conflito (veja `alguem_partiu_de`).
/// Se o gerente já decidiu em outro caixa, já entra resolvido.
pub(super) fn anotar_conflito(
    conn: &Connection,
    tabela: &str,
    chave: &str,
    ficou: &Versao,
    perdeu: &Versao,
    certeza: Relacao,
) -> Resultado<()> {
    let ja: bool = conn
        .query_row("SELECT 1 FROM sync_conflito WHERE perdeu = ?1", [&perdeu.id], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if ja || alguem_partiu_de(conn, tabela, chave, &perdeu.id)? {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo, dados_perdeu, dispositivo_ficou,
                                    base_perdeu, historia_perdeu, certeza)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'registro', ?7, ?8, ?9, ?10, ?11)",
        params![
            tabela,
            chave,
            ficou.id,
            perdeu.id,
            perdeu.dispositivo,
            tempo::agora(),
            perdeu.dados,
            ficou.dispositivo,
            perdeu.base,
            perdeu.historia,
            if certeza == Relacao::Incerta { "incerta" } else { "concorrente" }
        ],
    )?;
    fechar_decididos(conn)?;
    // Guarda os últimos 5000 já resolvidos, com a versão que perdeu (os
    // abertos ficam sempre).
    conn.execute(
        "DELETE FROM sync_conflito WHERE resolvido_em IS NOT NULL
            AND id <= (SELECT MAX(id) - 5000 FROM sync_conflito)",
        [],
    )?;
    Ok(())
}

/// Fecha os conflitos que um gerente já decidiu (aqui ou em outro caixa) e
/// escreve no conflito o que cada gerente decidiu.
///
/// Dois gerentes podem decidir diferente sobre o mesmo conflito, com os
/// caixas sem rede. A regra (igual em todos os caixas, sem relógio): **vale
/// "usar a que perdeu"**. Essa escolha gravou uma alteração nova feita em
/// cima da versão que valia, e uma alteração feita depois sempre vale (pela
/// ordem). "Manter" não grava nada no registro, então não tem como valer
/// contra ela. A divergência fica registrada num evento "aplicada", com o
/// mesmo id em todos os caixas (feito a partir das decisões), e a tela diz
/// quem decidiu o quê e qual versão vale agora.
pub(super) fn fechar_decididos(conn: &Connection) -> Resultado<()> {
    #[allow(clippy::type_complexity)]
    let decisoes: Vec<(String, String, String, String, String, Option<String>, String)> = {
        let mut c = conn.prepare(
            "SELECT r.id, r.tabela, r.chave, r.perdeu, r.escolha, u.nome, r.quando FROM sync_resolucao r
               LEFT JOIN usuario u ON u.id = r.usuario_id
              WHERE r.escolha IN ('usar', 'manter') ORDER BY r.tabela, r.chave, r.perdeu, r.id",
        )?;
        let l =
            c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)))?;
        l.collect::<Result<_, _>>()?
    };
    // (tabela, chave, versão que perdeu) → decisões (id, escolha, quem, quando).
    type Decisao = (String, String, String, String);
    let mut grupos: Vec<((String, String, String), Vec<Decisao>)> = Vec::new();
    for (id, tabela, chave, perdeu, escolha, nome, quando) in decisoes {
        let k = (tabela, chave, perdeu);
        let d = (id, escolha, nome.unwrap_or_else(|| "um gerente".into()), quando);
        match grupos.last_mut() {
            Some((g, lista)) if *g == k => lista.push(d),
            _ => grupos.push((k, vec![d])),
        }
    }
    for ((tabela, chave, perdeu), lista) in grupos {
        let usou = lista.iter().any(|d| d.1 == "usar");
        let manteve = lista.iter().any(|d| d.1 == "manter");
        let quem = |e: &str| lista.iter().filter(|d| d.1 == e).map(|d| d.2.clone()).collect::<Vec<_>>().join(", ");
        let texto = if usou && manteve {
            // Evento da divergência: mesmo id e mesmo conteúdo em todos os caixas.
            let ids: Vec<&str> = lista.iter().map(|d| d.0.as_str()).collect();
            let resumo: [u8; 32] =
                Sha256::digest(format!("{tabela}|{chave}|{perdeu}|{}", ids.join(",")).as_bytes()).into();
            let id_evento =
                format!("aplicada-{}", resumo.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>());
            let quando = lista.iter().map(|d| d.3.as_str()).max().unwrap_or_default().to_string();
            conn.execute(
                "INSERT OR IGNORE INTO sync_resolucao (id, tabela, chave, ficou, perdeu, escolha, usuario_id, quando, decisoes)
                 VALUES (?1, ?2, ?3, '', ?4, 'aplicada', NULL, ?5, ?6)",
                params![id_evento, tabela, chave, perdeu, quando, serde_json::to_string(&ids)?],
            )?;
            format!(
                "decisões diferentes: {} manteve, {} usou a que tinha perdido. Valeu a de usar a que tinha perdido \
                 (ela gravou uma alteração nova); a decisão de manter ficou registrada",
                quem("manter"),
                quem("usar")
            )
        } else if usou {
            format!("voltou a versão que tinha perdido ({})", quem("usar"))
        } else {
            format!("ficou como está ({})", quem("manter"))
        };
        let primeiro = conn
            .query_row(
                "SELECT usuario_id FROM sync_resolucao WHERE tabela = ?1 AND chave = ?2 AND perdeu = ?3
                    AND escolha IN ('usar', 'manter') ORDER BY id LIMIT 1",
                params![tabela, chave, perdeu],
                |l| l.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        conn.execute(
            "UPDATE sync_conflito SET resolvido_em = COALESCE(resolvido_em, ?4), resolvido_por = COALESCE(resolvido_por, ?5),
                    resolucao = ?6
              WHERE tabela = ?1 AND chave = ?2 AND perdeu = ?3 AND (resolucao IS NULL OR resolucao <> ?6)",
            params![tabela, chave, perdeu, tempo::agora(), primeiro, texto],
        )?;
    }
    Ok(())
}

/// Conflitos abertos que deixaram de ser conflito (o resultado é o mesmo
/// em qualquer ordem de chegada):
/// - a versão que vale (ou outra que perdeu) foi feita em cima da que
///   perdeu: ela não se perdeu, alguém a viu e mexeu depois;
/// - no fim, a versão que vale ficou igual à que perdeu.
///
/// Esses saem da lista; os decididos pelo gerente ficam como resolvidos.
pub(super) fn limpar_conflitos(conn: &Connection) -> Resultado<()> {
    fechar_decididos(conn)?;
    #[allow(clippy::type_complexity)]
    let abertos: Vec<(i64, String, String, String, Option<String>, Option<String>, Option<String>)> = {
        let mut c = conn.prepare(
            "SELECT id, tabela, chave, perdeu, dados_perdeu, base_perdeu, historia_perdeu FROM sync_conflito
              WHERE tipo = 'registro' AND resolvido_em IS NULL",
        )?;
        let l =
            c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?, l.get(5)?, l.get(6)?)))?;
        l.collect::<Result<_, _>>()?
    };
    for (id, tabela, chave, perdeu, dados, _, _) in &abertos {
        let Some(atual) = versao_atual(conn, tabela, chave)? else { continue };
        let partiu =
            |base: Option<&str>, historia: Option<&str>| linhagem(base, historia).ids.iter().any(|i| i == perdeu);
        // Outra que perdeu foi feita em cima desta (vale mesmo se o log já
        // não tem mais a versão: a história fica no conflito).
        let outra_partiu = abertos
            .iter()
            .any(|o| o.0 != *id && o.1 == *tabela && o.2 == *chave && partiu(o.5.as_deref(), o.6.as_deref()));
        let sem_diferenca = atual.id != *perdeu && diferencas(atual.dados.as_deref(), dados.as_deref()).is_empty();
        if outra_partiu || sem_diferenca || alguem_partiu_de(conn, tabela, chave, perdeu)? {
            conn.execute("DELETE FROM sync_conflito WHERE id = ?1", [id])?;
        }
    }
    Ok(())
}
