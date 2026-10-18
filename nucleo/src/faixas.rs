//! Código interno das peças sem repetir entre caixas: faixas de números.
//!
//! Como funciona:
//! - Um caixa da loja é o "distribuidor" (`codigos_distribuidor`, vale para
//!   a loja toda). Só ele cria faixas, numa gravação só (transação que trava
//!   o banco para escrever), olhando todas as faixas e todos os códigos que
//!   ele conhece. Cada faixa nova começa depois da maior faixa e do maior
//!   código de números que existem, e depois do maior fim que ele já deu
//!   (guardado neste computador e num arquivo ao lado do banco, que um backup
//!   restaurado não apaga).
//! - Os outros caixas pedem a faixa ao distribuidor pela troca de dados
//!   (pedido "faixa"); a faixa chega como uma alteração normal do log e fica
//!   gravada antes de o caixa dar qualquer código dela.
//! - A faixa é do caixa e da geração de códigos dele. Restaurar um backup,
//!   ou o banco ir para outro lugar, dá geração nova: o caixa pede outra
//!   faixa e a antiga nunca mais é usada por ninguém (não dá para saber até
//!   onde ela foi usada). Nada é liberado de novo, nem quando um caixa fica
//!   muito tempo sem rede.
//! - Sem faixa (caixa novo antes da primeira troca, faixa acabou sem rede),
//!   a peça fica com "código a definir" (código vazio): não sai etiqueta e o
//!   código é dado quando a faixa chegar, sem mudar nenhum outro código.
//! - Códigos que já existiam antes das faixas ficam como estão. A primeira
//!   faixa do distribuidor (1 a 99.999, ou até o fim da faixa do maior código
//!   de números que já existe) é dada depois de conferir os códigos.
//! - Códigos repetidos entre peças (de antes das faixas, ou de duas lojas
//!   juntadas) não são trocados sozinhos: o gerente vê a lista, o leitor e a
//!   busca perguntam qual peça é, e a etiqueta não sai até resolver.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::erro::{Erro, Resultado};
use crate::{config, tempo};

/// Tamanho de cada faixa nova.
pub const TAMANHO_FAIXA: i64 = 100_000;
/// O maior código de números que sai de uma faixa (9 algarismos).
pub const MAIOR_CODIGO: i64 = 999_999_999;
/// Quantas faixas o caixa que passa a distribuir pula, por segurança (o
/// distribuidor antigo pode ter dado faixas que ainda não chegaram).
pub const SALTO_AO_ASSUMIR: i64 = 20;
/// Faltando menos que um décimo da faixa mais nova, o caixa pede a próxima.
fn limiar(faixa: &Faixa) -> i64 {
    ((faixa.fim - faixa.inicio + 1) / 10).max(1)
}
/// Um caixa de fora não recebe mais de uma faixa nova por minuto (protege
/// contra um caixa com defeito pedindo sem parar). Nos testes, um banco
/// pode diminuir (`codigos_espera_teste`, em segundos).
fn espera_entre_faixas(conn: &Connection) -> Resultado<Duration> {
    #[cfg(test)]
    {
        if let Some(t) = config::ler(conn, "codigos_espera_teste")?.and_then(|t| t.parse::<u64>().ok()) {
            return Ok(Duration::from_secs(t));
        }
    }
    let _ = conn;
    Ok(Duration::from_secs(60))
}

/// Condição SQL de um código "de números" (como os que as faixas dão):
/// só algarismos, sem zero na frente, até 9 algarismos.
const SQL_NUMERO: &str = "codigo GLOB '[1-9]*' AND codigo NOT GLOB '*[^0-9]*' AND length(codigo) <= 9";

/// O valor de um código de números (None para os outros formatos).
pub fn numero(codigo: &str) -> Option<i64> {
    let b = codigo.as_bytes();
    if b.is_empty() || b.len() > 9 || b[0] == b'0' || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    codigo.parse().ok()
}

/// Tamanho das faixas novas. Nos testes, um banco pode pedir faixas
/// pequenas (`codigos_tamanho_teste`) para testar o fim da faixa.
fn tamanho(conn: &Connection) -> Resultado<i64> {
    #[cfg(test)]
    {
        if let Some(t) = config::ler(conn, "codigos_tamanho_teste")?.and_then(|t| t.parse::<i64>().ok()) {
            if t >= 2 {
                return Ok(t);
            }
        }
    }
    let _ = conn;
    Ok(TAMANHO_FAIXA)
}

// ---------------------------------------------------------------------------
// Dono, geração e o arquivo ao lado do banco.
// ---------------------------------------------------------------------------

fn dispositivo(conn: &Connection) -> Resultado<String> {
    config::ler(conn, "dispositivo_id")?
        .filter(|d| !d.is_empty())
        .ok_or_else(|| Erro::Interno("sem dispositivo".into()))
}

/// Geração de códigos deste caixa (só deste computador).
pub fn geracao(conn: &Connection) -> Resultado<String> {
    if let Some(g) = config::ler(conn, "codigos_geracao")?.filter(|g| !g.is_empty()) {
        return Ok(g);
    }
    let g = tempo::novo_id();
    config::gravar(conn, "codigos_geracao", &g)?;
    Ok(g)
}

/// Geração nova: as faixas de antes não são mais usadas por este caixa.
pub fn nova_geracao(conn: &Connection, motivo: &str) -> Resultado<()> {
    crate::registro::anotar(&format!("códigos das peças: geração nova ({motivo})"));
    config::gravar(conn, "codigos_geracao", &tempo::novo_id())?;
    escrever_arquivo(conn)
}

/// O banco veio de outro lugar (ou voltou para trás) sem o arquivo que diz
/// até onde este caixa foi: geração nova e, se ele distribui, a próxima
/// faixa pula `SALTO_AO_ASSUMIR` (pode já ter dado faixas que este banco
/// não tem).
fn banco_de_outro_lugar(conn: &Connection, motivo: &str) -> Resultado<()> {
    if sou_distribuidor(conn)? {
        config::gravar(conn, "codigos_salto", "1")?;
    }
    nova_geracao(conn, motivo)
}

fn ler_numero(conn: &Connection, chave: &str) -> Resultado<i64> {
    Ok(config::ler(conn, chave)?.and_then(|t| t.parse().ok()).unwrap_or(0))
}

/// Último código dado por este caixa na geração atual.
fn ultimo(conn: &Connection, g: &str) -> Resultado<i64> {
    Ok(config::ler(conn, "codigos_ultimo")?
        .and_then(|t| t.split_once('|').filter(|(gg, _)| *gg == g).and_then(|(_, n)| n.parse().ok()))
        .unwrap_or(0))
}

/// O arquivo ao lado do banco ("rustinho.db.codigos").
fn caminho_do_arquivo(conn: &Connection) -> Resultado<Option<PathBuf>> {
    let banco: Option<String> =
        conn.query_row("SELECT file FROM pragma_database_list WHERE name = 'main'", [], |l| l.get(0)).optional()?;
    Ok(banco.filter(|b| !b.is_empty()).map(|b| PathBuf::from(format!("{b}.codigos"))))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Arquivo {
    dispositivo: String,
    geracao: String,
    ultimo: i64,
    maior: i64,
}

/// Grava o arquivo com o que este caixa já deu (só aumenta). Um banco que
/// volta para trás (cópia antiga por cima) continua depois disso.
fn escrever_arquivo(conn: &Connection) -> Resultado<()> {
    let Some(caminho) = caminho_do_arquivo(conn)? else { return Ok(()) };
    let g = geracao(conn)?;
    let a = Arquivo {
        dispositivo: dispositivo(conn)?,
        ultimo: ultimo(conn, &g)?,
        geracao: g,
        maior: ler_numero(conn, "codigos_maior")?,
    };
    let novo = caminho.with_extension("codigos-novo");
    let texto = serde_json::to_string(&a)?;
    let gravou = std::fs::write(&novo, texto).and_then(|_| std::fs::rename(&novo, &caminho));
    match gravou {
        Ok(()) => {
            crate::arquivos::so_do_usuario(&caminho, "arquivo dos códigos");
            config::gravar(conn, "codigos_arquivo", &caminho.display().to_string())?;
        }
        Err(e) => crate::registro::anotar(&format!("códigos das peças: não gravou o arquivo ({e})")),
    }
    Ok(())
}

/// Ao abrir o banco: confere o arquivo ao lado dele.
/// - O arquivo já existiu e sumiu, ou o banco está em outro lugar: o banco
///   foi copiado ou mudou de computador. Geração nova (duas cópias nunca
///   dão códigos da mesma faixa).
/// - O arquivo diz que este caixa já foi mais longe (cópia antiga por cima):
///   continua depois do que o arquivo diz. Se a geração do arquivo é outra,
///   a deste banco já tinha sido trocada: geração nova.
fn conferir_arquivo(conn: &Connection) -> Resultado<()> {
    let Some(caminho) = caminho_do_arquivo(conn)? else { return Ok(()) };
    let marca = config::ler(conn, "codigos_arquivo")?;
    let lido: Option<Arquivo> = std::fs::read(&caminho).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let eu = dispositivo(conn)?;
    let g = geracao(conn)?;
    match lido {
        None => {
            if marca.is_some() {
                banco_de_outro_lugar(conn, "o banco mudou de lugar ou o arquivo dos códigos sumiu")?;
            }
        }
        Some(a) if a.dispositivo == eu => {
            if ler_numero(conn, "codigos_maior")? < a.maior {
                config::gravar(conn, "codigos_maior", &a.maior.to_string())?;
            }
            if a.geracao == g {
                if ultimo(conn, &g)? < a.ultimo {
                    config::gravar(conn, "codigos_ultimo", &format!("{g}|{}", a.ultimo))?;
                }
                if marca.as_deref() != Some(&caminho.display().to_string()) {
                    banco_de_outro_lugar(conn, "o banco veio de outro lugar")?;
                }
            } else {
                nova_geracao(conn, "o banco voltou para trás")?;
            }
        }
        Some(_) => {
            // Arquivo de outro banco neste lugar: este banco começa a usar o
            // arquivo; se ele já tinha arquivo em outro lugar, veio de lá.
            if marca.is_some() {
                banco_de_outro_lugar(conn, "o banco veio de outro lugar")?;
            }
        }
    }
    escrever_arquivo(conn)
}

// ---------------------------------------------------------------------------
// Faixas.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Faixa {
    pub inicio: i64,
    pub fim: i64,
    pub dispositivo: String,
    pub geracao: String,
    pub caixa: String,
    pub distribuidor: String,
    pub criado_em: String,
}

impl Faixa {
    fn cruza(&self, outra: &Faixa) -> bool {
        self.inicio <= outra.fim && outra.inicio <= self.fim
    }
    fn mesmo_dono(&self, outra: &Faixa) -> bool {
        self.dispositivo == outra.dispositivo && self.geracao == outra.geracao
    }
    fn contem(&self, n: i64) -> bool {
        self.inicio <= n && n <= self.fim
    }
}

pub fn faixas(conn: &Connection) -> Resultado<Vec<Faixa>> {
    let mut c = conn.prepare_cached(
        "SELECT inicio, fim, dispositivo, geracao, caixa, distribuidor, criado_em FROM faixa_codigo
          ORDER BY inicio, dispositivo, geracao",
    )?;
    let l = c.query_map([], |l| {
        Ok(Faixa {
            inicio: l.get(0)?,
            fim: l.get(1)?,
            dispositivo: l.get(2)?,
            geracao: l.get(3)?,
            caixa: l.get(4)?,
            distribuidor: l.get(5)?,
            criado_em: l.get(6)?,
        })
    })?;
    Ok(l.collect::<Result<_, _>>()?)
}

/// Faixas deste caixa (geração atual) que ninguém mais tem.
fn minhas_livres(conn: &Connection) -> Resultado<Vec<Faixa>> {
    let (eu, g) = (dispositivo(conn)?, geracao(conn)?);
    let todas = faixas(conn)?;
    Ok(todas
        .iter()
        .filter(|f| f.dispositivo == eu && f.geracao == g)
        .filter(|f| !todas.iter().any(|o| !o.mesmo_dono(f) && o.cruza(f)))
        .cloned()
        .collect())
}

/// Nome do caixa dono de uma faixa, para a tela.
fn nome_do_dono(conn: &Connection, f: &Faixa) -> Resultado<String> {
    let nome = crate::sync::nome_do_caixa(conn, &f.dispositivo)?;
    if nome == "outro caixa" && !f.caixa.is_empty() {
        return Ok(f.caixa.clone());
    }
    Ok(nome)
}

/// Pares de faixas de donos diferentes que se cruzam.
pub fn cruzadas(conn: &Connection) -> Resultado<Vec<(Faixa, Faixa)>> {
    let todas = faixas(conn)?;
    let mut r = Vec::new();
    for (i, a) in todas.iter().enumerate() {
        for b in &todas[i + 1..] {
            if !a.mesmo_dono(b) && a.cruza(b) {
                r.push((a.clone(), b.clone()));
            }
        }
    }
    Ok(r)
}

/// O maior código de números entre `a` e `b`, atual ou antigo (0 se não há).
fn maior_existente(conn: &Connection, a: i64, b: i64) -> Resultado<i64> {
    let mut maior = 0;
    for tabela in ["produto", "produto_codigo_antigo"] {
        let n: Option<i64> = conn.query_row(
            &format!(
                "SELECT MAX(CAST(codigo AS INTEGER)) FROM {tabela}
                  WHERE {SQL_NUMERO} AND CAST(codigo AS INTEGER) BETWEEN ?1 AND ?2"
            ),
            params![a, b],
            |l| l.get(0),
        )?;
        maior = maior.max(n.unwrap_or(0));
    }
    Ok(maior)
}

/// Próximo código livre numa faixa deste caixa: depois do último que ele
/// deu e depois de qualquer código que já exista nela (nunca preenche
/// buracos: um código que já foi usado e sumiu não volta).
fn proximo_em(conn: &Connection, f: &Faixa, ultimo: i64) -> Resultado<Option<i64>> {
    if f.fim <= ultimo {
        return Ok(None);
    }
    let c = (ultimo + 1).max(f.inicio).max(maior_existente(conn, f.inicio, f.fim)? + 1);
    Ok((c <= f.fim).then_some(c))
}

/// O código que a próxima peça deste caixa vai receber (só para mostrar;
/// não reserva nada).
pub fn previsto(conn: &Connection) -> Resultado<Option<String>> {
    let u = ultimo(conn, &geracao(conn)?)?;
    for f in minhas_livres(conn)? {
        if let Some(c) = proximo_em(conn, &f, u)? {
            return Ok(Some(c.to_string()));
        }
    }
    Ok(None)
}

/// Quantos códigos ainda restam nas faixas deste caixa.
pub fn restantes(conn: &Connection) -> Resultado<i64> {
    let u = ultimo(conn, &geracao(conn)?)?;
    let mut total = 0;
    for f in minhas_livres(conn)? {
        if let Some(c) = proximo_em(conn, &f, u)? {
            total += f.fim - c + 1;
        }
    }
    Ok(total)
}

/// Este caixa precisa de mais uma faixa (não tem, ou falta pouco).
pub fn precisa(conn: &Connection) -> Resultado<bool> {
    let livres = minhas_livres(conn)?;
    let Some(mais_nova) = livres.iter().max_by_key(|f| f.inicio) else { return Ok(true) };
    Ok(restantes(conn)? < limiar(mais_nova))
}

/// Dá o próximo código deste caixa e guarda que deu (no banco, na mesma
/// transação de quem chamou, e no arquivo ao lado). None = sem faixa.
pub fn gerar(conn: &Connection) -> Resultado<Option<String>> {
    let g = geracao(conn)?;
    let u = ultimo(conn, &g)?;
    for f in minhas_livres(conn)? {
        if let Some(c) = proximo_em(conn, &f, u)? {
            config::gravar(conn, "codigos_ultimo", &format!("{g}|{c}"))?;
            escrever_arquivo(conn)?;
            return Ok(Some(c.to_string()));
        }
    }
    Ok(None)
}

/// Código para uma peça nova (ou a definir): da faixa deste caixa. O
/// distribuidor sem faixa cria a dele na mesma transação. None = a peça
/// fica com código a definir.
pub fn gerar_para_peca(conn: &Connection) -> Resultado<Option<String>> {
    garantir_distribuidor(conn)?;
    if let Some(c) = gerar(conn)? {
        return Ok(Some(c));
    }
    if sou_distribuidor(conn)? {
        faixa_propria(conn)?;
        return gerar(conn);
    }
    Ok(None)
}

/// O distribuidor cria uma faixa nova para ele mesmo.
fn faixa_propria(conn: &Connection) -> Resultado<Faixa> {
    let (eu, g) = (dispositivo(conn)?, geracao(conn)?);
    nova_faixa(conn, &eu, &g, &config::nome_caixa(conn)?)
}

// ---------------------------------------------------------------------------
// Distribuidor.
// ---------------------------------------------------------------------------

/// Quem distribui as faixas na loja.
pub fn distribuidor(conn: &Connection) -> Resultado<Option<String>> {
    Ok(config::ler(conn, "codigos_distribuidor")?.filter(|d| !d.is_empty()))
}

pub fn sou_distribuidor(conn: &Connection) -> Resultado<bool> {
    Ok(distribuidor(conn)?.is_some_and(|d| Some(d) == config::ler(conn, "dispositivo_id").ok().flatten()))
}

/// Loja nova (primeira configuração): este caixa distribui e fica com a
/// primeira faixa (1 a 99.999).
pub fn comecar_loja(conn: &Connection) -> Resultado<Inventario> {
    comecar_distribuidor(conn)
}

/// Este caixa passa a distribuir: confere os códigos que existem e, se a
/// loja ainda não tem faixa nenhuma, fica com a primeira (de 1 até 99.999,
/// ou até o fim da faixa do maior código de números que já existe).
fn comecar_distribuidor(conn: &Connection) -> Resultado<Inventario> {
    let inv = inventario(conn)?;
    let eu = dispositivo(conn)?;
    let g = geracao(conn)?;
    let t = tamanho(conn)?;
    config::gravar(conn, "codigos_distribuidor", &eu)?;
    let nenhuma: bool = conn.query_row("SELECT NOT EXISTS (SELECT 1 FROM faixa_codigo)", [], |l| l.get(0))?;
    if nenhuma {
        let maior = maior_existente(conn, 1, MAIOR_CODIGO)?;
        let fim = if maior < t { t - 1 } else { (maior / t + 1) * t - 1 }.min(MAIOR_CODIGO);
        conn.execute(
            "INSERT INTO faixa_codigo (inicio, fim, dispositivo, geracao, caixa, distribuidor, criado_em)
             VALUES (1, ?1, ?2, ?3, ?4, ?2, ?5)",
            params![fim, eu, g, config::nome_caixa(conn)?, tempo::agora()],
        )?;
        let antes = ler_numero(conn, "codigos_maior")?;
        config::gravar(conn, "codigos_maior", &antes.max(fim).to_string())?;
    }
    config::gravar(conn, "codigos_inventario", &serde_json::to_string(&inv)?)?;
    conn.execute("DELETE FROM config WHERE chave = 'codigos_legado'", [])?;
    escrever_arquivo(conn)?;
    Ok(inv)
}

/// Loja que já usava o Rustinho antes das faixas: escolhe um caixa só para
/// distribuir. Sozinho (nunca trocou dados com ninguém): este. Com outros
/// caixas: o que gravou o nome da loja que vale (todos os caixas escolhem o
/// mesmo, porque olham a mesma versão); só ele se marca. Os outros esperam.
pub fn garantir_distribuidor(conn: &Connection) -> Resultado<()> {
    if config::ler(conn, "codigos_legado")?.as_deref() != Some("1") {
        return Ok(());
    }
    if distribuidor(conn)?.is_some() {
        conn.execute("DELETE FROM config WHERE chave = 'codigos_legado'", [])?;
        return Ok(());
    }
    let eu = dispositivo(conn)?;
    let sozinho: bool = conn.query_row("SELECT NOT EXISTS (SELECT 1 FROM sync_par)", [], |l| l.get(0))?;
    let escolhido = if sozinho {
        Some(eu.clone())
    } else {
        conn.query_row(
            "SELECT dispositivo FROM alteracao WHERE tabela = 'config' AND chave = json_array('loja_nome') AND compactada = 0
              ORDER BY ordem DESC, id DESC LIMIT 1",
            [],
            |l| l.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
    };
    if escolhido.as_deref() == Some(eu.as_str()) {
        crate::registro::anotar(
            "códigos das peças: este caixa passou a distribuir as faixas (loja de antes das faixas)",
        );
        comecar_distribuidor(conn)?;
    }
    Ok(())
}

/// O gerente faz este caixa distribuir as faixas (o distribuidor de antes
/// não volta mais). Pula `SALTO_AO_ASSUMIR` faixas na próxima.
pub fn assumir(conn: &Connection) -> Resultado<()> {
    if sou_distribuidor(conn)? {
        return Ok(());
    }
    config::gravar(conn, "codigos_distribuidor", &dispositivo(conn)?)?;
    config::gravar(conn, "codigos_salto", "1")?;
    conn.execute("DELETE FROM config WHERE chave = 'codigos_legado'", [])?;
    Ok(())
}

/// Cria uma faixa nova para um dono. Só o distribuidor, dentro de uma
/// transação de escrita.
fn nova_faixa(conn: &Connection, dono: &str, geracao_dono: &str, nome_dono: &str) -> Resultado<Faixa> {
    let t = tamanho(conn)?;
    let maior_fim: i64 = conn.query_row("SELECT COALESCE(MAX(fim), 0) FROM faixa_codigo", [], |l| l.get(0))?;
    let base = maior_fim.max(ler_numero(conn, "codigos_maior")?).max(maior_existente(conn, 1, MAIOR_CODIGO)?);
    let mut inicio = (base / t + 1) * t;
    if config::ler(conn, "codigos_salto")?.as_deref() == Some("1") {
        inicio += SALTO_AO_ASSUMIR * t;
    }
    let fim = inicio + t - 1;
    if fim > MAIOR_CODIGO {
        return Err(Erro::validacao(
            "Acabaram as faixas de códigos de números. Use códigos com letras e chame quem cuida do sistema.",
        ));
    }
    let f = Faixa {
        inicio,
        fim,
        dispositivo: dono.to_string(),
        geracao: geracao_dono.to_string(),
        caixa: nome_dono.chars().take(60).collect(),
        distribuidor: dispositivo(conn)?,
        criado_em: tempo::agora(),
    };
    conn.execute(
        "INSERT INTO faixa_codigo (inicio, fim, dispositivo, geracao, caixa, distribuidor, criado_em)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![f.inicio, f.fim, f.dispositivo, f.geracao, f.caixa, f.distribuidor, f.criado_em],
    )?;
    config::gravar(conn, "codigos_maior", &fim.to_string())?;
    conn.execute("DELETE FROM config WHERE chave = 'codigos_salto'", [])?;
    escrever_arquivo(conn)?;
    crate::registro::anotar(&format!("códigos das peças: faixa {inicio} a {fim} dada a um caixa"));
    Ok(f)
}

fn ultima_faixa_dada() -> &'static Mutex<HashMap<String, Instant>> {
    static M: std::sync::OnceLock<Mutex<HashMap<String, Instant>>> = std::sync::OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Dentro de uma transação de escrita, para o pedido de outro caixa: as
/// faixas dele que ele ainda não tem (a resposta de antes se perdeu) ou, se
/// não há, uma faixa nova (no máximo uma por minuto para cada caixa).
/// Devolve os inícios.
pub fn distribuir_dentro(
    conn: &Connection,
    dono: &str,
    geracao_dono: &str,
    nome_dono: &str,
    tenho: &[i64],
) -> Resultado<Vec<i64>> {
    if !sou_distribuidor(conn)? {
        return Err(Erro::validacao("Este caixa não distribui as faixas de códigos."));
    }
    if dono.is_empty() || geracao_dono.is_empty() {
        return Err(Erro::validacao("Pedido de faixa sem caixa."));
    }
    let do_dono: Vec<i64> = conn
        .prepare_cached("SELECT inicio FROM faixa_codigo WHERE dispositivo = ?1 AND geracao = ?2 ORDER BY inicio")?
        .query_map(params![dono, geracao_dono], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    let faltam: Vec<i64> = do_dono.iter().filter(|i| !tenho.contains(i)).copied().collect();
    if !faltam.is_empty() {
        return Ok(faltam);
    }
    if dono == dispositivo(conn)? {
        return Err(Erro::validacao("Pedido de faixa do próprio distribuidor."));
    }
    let chave = format!("{dono}|{geracao_dono}");
    let espera = espera_entre_faixas(conn)?;
    if ultima_faixa_dada().lock().unwrap_or_else(|e| e.into_inner()).get(&chave).is_some_and(|t| t.elapsed() < espera) {
        return Ok(Vec::new());
    }
    let f = nova_faixa(conn, dono, geracao_dono, nome_dono)?;
    let mut m = ultima_faixa_dada().lock().unwrap_or_else(|e| e.into_inner());
    if m.len() > 1000 {
        m.clear();
    }
    m.insert(chave, Instant::now());
    Ok(vec![f.inicio])
}

/// Atende o pedido de faixa de outro caixa (numa transação de escrita só:
/// dois pedidos ao mesmo tempo nunca recebem a mesma faixa). Devolve as
/// chaves (inicio, dono, geração) das faixas que vão na resposta.
pub fn atender_pedido(
    conn: &mut Connection,
    dono: &str,
    geracao_dono: &str,
    nome_dono: &str,
    tenho: &[i64],
) -> Resultado<Vec<(i64, String, String)>> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let inicios = distribuir_dentro(&tx, dono, geracao_dono, nome_dono, tenho)?;
    tx.commit()?;
    Ok(inicios.into_iter().map(|i| (i, dono.to_string(), geracao_dono.to_string())).collect())
}

/// O que o caixa já tem (para o pedido): os inícios das faixas dele.
pub fn pedido(conn: &Connection) -> Resultado<Value> {
    let (eu, g) = (dispositivo(conn)?, geracao(conn)?);
    let tenho: Vec<i64> = conn
        .prepare_cached("SELECT inicio FROM faixa_codigo WHERE dispositivo = ?1 AND geracao = ?2")?
        .query_map(params![eu, g], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(json!({ "dispositivo": eu, "geracao": g, "tenho": tenho, "nome": config::nome_caixa(conn)? }))
}

/// Manutenção (a cada poucos segundos e depois de cada troca de dados): o
/// distribuidor cria a própria faixa quando falta, e as peças deste caixa
/// com código a definir recebem código. Devolve quantas receberam.
pub fn manter(conn: &mut Connection) -> Resultado<usize> {
    // Só trava o banco para escrever quando há o que fazer (a manutenção
    // roda a cada poucos segundos, junto com o uso normal do caixa).
    let legado = config::ler(conn, "codigos_legado")?.as_deref() == Some("1");
    if legado || (sou_distribuidor(conn)? && precisa(conn)?) {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        garantir_distribuidor(&tx)?;
        if sou_distribuidor(&tx)? && precisa(&tx)? {
            faixa_propria(&tx)?;
        }
        tx.commit()?;
    }
    atribuir_pendentes(conn)
}

/// As peças deste caixa que estão com código a definir recebem código da
/// faixa (as dos outros caixas, eles mesmos dão).
pub fn atribuir_pendentes(conn: &mut Connection) -> Resultado<usize> {
    let eu = dispositivo(conn)?;
    let ids: Vec<String> = conn
        .prepare_cached(
            "SELECT id FROM produto WHERE codigo = '' AND dispositivo = ?1 ORDER BY criado_em, id LIMIT 2000",
        )?
        .query_map([&eu], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    if ids.is_empty() {
        return Ok(0);
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut n = 0;
    for id in ids {
        let Some(c) = gerar_para_peca(&tx)? else { break };
        if crate::produtos::definir_codigo(&tx, &eu, &id, &c)? {
            n += 1;
        }
    }
    tx.commit()?;
    Ok(n)
}

/// O gerente dá código a todas as peças com código a definir, de qualquer
/// caixa, pela faixa deste (quando o caixa que criou não volta mais).
pub fn definir_todos_pendentes(conn: &Connection) -> Resultado<usize> {
    let eu = dispositivo(conn)?;
    let ids: Vec<String> = conn
        .prepare_cached("SELECT id FROM produto WHERE codigo = '' ORDER BY criado_em, id")?
        .query_map([], |l| l.get(0))?
        .collect::<Result<_, _>>()?;
    let mut n = 0;
    for id in ids {
        let Some(c) = gerar_para_peca(conn)? else {
            return Err(Erro::validacao(
                "Este caixa ainda não tem faixa de códigos (ou ela acabou). Faça a troca de dados com o caixa que distribui as faixas.",
            ));
        };
        if crate::produtos::definir_codigo(conn, &eu, &id, &c)? {
            n += 1;
        }
    }
    Ok(n)
}

/// Ao abrir o banco: confere o arquivo, escolhe o distribuidor (loja de
/// antes das faixas) e faz a manutenção.
pub fn preparar(conn: &mut Connection) -> Resultado<()> {
    // Nada disto impede o banco de abrir: o que falhar fica no registro e a
    // manutenção roda de novo a cada poucos segundos.
    if let Err(e) = conferir_arquivo(conn) {
        crate::registro::anotar(&format!("códigos das peças: arquivo ao lado do banco: {e}"));
    }
    if let Err(e) = manter(conn) {
        crate::registro::anotar(&format!("códigos das peças: {e}"));
    }
    Ok(())
}

/// Depois de restaurar um backup: geração nova (a faixa de antes não é mais
/// usada; até onde ela foi, só o banco perdido sabia). Backup de antes das
/// faixas: a loja escolhe o distribuidor de novo.
pub fn depois_de_restaurar(conn: &Connection) -> Resultado<()> {
    nova_geracao(conn, "backup restaurado")?;
    let tem_usuarios: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM usuario)", [], |l| l.get(0))?;
    if distribuidor(conn)?.is_none() && tem_usuarios {
        config::gravar(conn, "codigos_legado", "1")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Código digitado à mão (ou vindo de uma importação).
// ---------------------------------------------------------------------------

/// Confere um código escrito à mão para uma peça. Códigos de números só
/// dentro das faixas deste caixa (ou, no distribuidor, fora de todas as
/// faixas: a próxima faixa começa depois dele). Nunca um código atual ou
/// antigo de outra peça.
pub fn conferir_digitado(conn: &Connection, produto_id: &str, codigo: &str) -> Resultado<()> {
    if let Some(outro) = conn
        .query_row(
            "SELECT p.nome FROM produto_codigo_antigo a JOIN produto p ON p.id = a.produto_id
              WHERE a.codigo = ?1 AND a.produto_id <> ?2 LIMIT 1",
            params![codigo, produto_id],
            |l| l.get::<_, String>(0),
        )
        .optional()?
    {
        return Err(Erro::campo("codigo", format!("O código {codigo} já foi da peça \"{outro}\". Use outro código.")));
    }
    let Some(n) = numero(codigo) else { return Ok(()) };
    let (eu, g) = (dispositivo(conn)?, geracao(conn)?);
    let todas = faixas(conn)?;
    let com_ele: Vec<&Faixa> = todas.iter().filter(|f| f.contem(n)).collect();
    let em_branco = "Deixe o código em branco para o Rustinho dar um, ou use um código com letras (ex.: A-123).";
    if let Some(f) = com_ele.iter().find(|f| !(f.dispositivo == eu && f.geracao == g)) {
        let dono = if f.dispositivo == eu {
            "deste caixa antes de restaurar o backup (ou de o banco mudar de lugar) e não são mais usados".to_string()
        } else {
            format!("do {}", nome_do_dono(conn, f)?)
        };
        return Err(Erro::campo("codigo", format!("Os códigos de {} a {} são {dono}. {em_branco}", f.inicio, f.fim)));
    }
    if com_ele.is_empty() && !sou_distribuidor(conn)? {
        return Err(Erro::campo(
            "codigo",
            format!(
                "Códigos só com números saem da faixa de cada caixa, e o {codigo} não é da faixa deste. {em_branco}"
            ),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Conferência dos códigos (inventário) e repetições.
// ---------------------------------------------------------------------------

/// Retrato dos códigos das peças.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Inventario {
    pub quando: String,
    pub pecas: i64,
    /// Código a definir (vazio).
    pub sem_codigo: i64,
    /// Só números, sem zero na frente, até 9 algarismos (como as faixas dão).
    pub de_numeros: i64,
    pub maior: i64,
    /// Limite da primeira faixa (99.999) e quantos ficam dentro e acima.
    pub limite: i64,
    pub ate_limite: i64,
    pub acima_do_limite: i64,
    /// Só números, mas com zero na frente ou mais de 9 algarismos.
    pub fora_do_padrao: i64,
    /// Com letras, traço, espaço...
    pub com_letras: i64,
    /// Códigos que aparecem em mais de uma peça, e quantas peças.
    pub repetidos: i64,
    pub pecas_repetidas: i64,
    pub exemplos_acima: Vec<String>,
    pub exemplos_fora: Vec<String>,
    pub exemplos_letras: Vec<String>,
    pub exemplos_repetidos: Vec<String>,
}

pub fn inventario(conn: &Connection) -> Resultado<Inventario> {
    let limite = tamanho(conn)? - 1;
    let mut inv = conn.query_row(
        &format!(
            "SELECT COUNT(*), TOTAL(codigo = ''), TOTAL({SQL_NUMERO}),
                    COALESCE(MAX(CASE WHEN {SQL_NUMERO} THEN CAST(codigo AS INTEGER) END), 0),
                    TOTAL(({SQL_NUMERO}) AND CAST(codigo AS INTEGER) <= ?1),
                    TOTAL(({SQL_NUMERO}) AND CAST(codigo AS INTEGER) > ?1),
                    TOTAL(codigo <> '' AND codigo NOT GLOB '*[^0-9]*' AND NOT ({SQL_NUMERO})),
                    TOTAL(codigo GLOB '*[^0-9]*')
               FROM produto"
        ),
        [limite],
        |l| {
            Ok(Inventario {
                pecas: l.get(0)?,
                sem_codigo: l.get::<_, f64>(1)? as i64,
                de_numeros: l.get::<_, f64>(2)? as i64,
                maior: l.get(3)?,
                ate_limite: l.get::<_, f64>(4)? as i64,
                acima_do_limite: l.get::<_, f64>(5)? as i64,
                fora_do_padrao: l.get::<_, f64>(6)? as i64,
                com_letras: l.get::<_, f64>(7)? as i64,
                ..Default::default()
            })
        },
    )?;
    inv.quando = tempo::agora();
    inv.limite = limite;
    let (repetidos, pecas): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(n), 0) FROM
           (SELECT COUNT(*) n FROM produto WHERE codigo <> '' GROUP BY codigo HAVING COUNT(*) > 1)",
        [],
        |l| Ok((l.get(0)?, l.get(1)?)),
    )?;
    inv.repetidos = repetidos;
    inv.pecas_repetidas = pecas;
    let exemplos = |sql: &str| -> Resultado<Vec<String>> {
        let mut c = conn.prepare(sql)?;
        let l = c.query_map([limite], |l| l.get(0))?;
        Ok(l.collect::<Result<_, _>>()?)
    };
    inv.exemplos_acima = exemplos(&format!(
        "SELECT codigo FROM produto WHERE {SQL_NUMERO} AND CAST(codigo AS INTEGER) > ?1
          ORDER BY CAST(codigo AS INTEGER) DESC LIMIT 5"
    ))?;
    inv.exemplos_fora = exemplos(&format!(
        "SELECT codigo FROM produto WHERE codigo <> '' AND codigo NOT GLOB '*[^0-9]*' AND NOT ({SQL_NUMERO}) AND ?1 = ?1
          ORDER BY codigo LIMIT 5"
    ))?;
    inv.exemplos_letras =
        exemplos("SELECT codigo FROM produto WHERE codigo GLOB '*[^0-9]*' AND ?1 = ?1 ORDER BY codigo LIMIT 5")?;
    inv.exemplos_repetidos = exemplos(
        "SELECT codigo FROM produto WHERE codigo <> '' AND ?1 = ?1 GROUP BY codigo HAVING COUNT(*) > 1
          ORDER BY codigo LIMIT 5",
    )?;
    Ok(inv)
}

/// Uma peça numa repetição.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PecaRepetida {
    pub id: String,
    pub nome: String,
    pub codigo: String,
    /// O código repetido é um código antigo desta peça.
    pub antigo: bool,
}

/// Um código que leva a mais de uma peça.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repetido {
    /// "interno" (código da peça) ou "barras".
    pub tipo: String,
    pub codigo: String,
    pub pecas: Vec<PecaRepetida>,
}

/// Códigos que levam a mais de uma peça: o mesmo código interno em duas
/// peças, o código antigo de uma igual ao atual de outra, ou o mesmo código
/// de barras. Nada é trocado sozinho: o gerente decide.
pub fn repetidos(conn: &Connection) -> Resultado<Vec<Repetido>> {
    let mut grupos: BTreeMap<(String, String), Vec<PecaRepetida>> = BTreeMap::new();
    let mut juntar = |tipo: &str, codigo: String, p: PecaRepetida| {
        let g = grupos.entry((tipo.to_string(), codigo)).or_default();
        if !g.iter().any(|x| x.id == p.id && x.antigo == p.antigo) {
            g.push(p);
        }
    };
    {
        let mut c = conn.prepare(
            "SELECT codigo, id, nome FROM produto
              WHERE codigo IN (SELECT codigo FROM produto WHERE codigo <> '' GROUP BY codigo HAVING COUNT(*) > 1)
              ORDER BY codigo, nome",
        )?;
        for l in c.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?)))? {
            let (codigo, id, nome) = l?;
            juntar("interno", codigo.clone(), PecaRepetida { id, nome, codigo, antigo: false });
        }
    }
    {
        let mut c = conn.prepare(
            "SELECT a.codigo, pa.id, pa.nome, pa.codigo, p.id, p.nome
               FROM produto_codigo_antigo a
               JOIN produto p ON p.codigo = a.codigo AND p.id <> a.produto_id
               JOIN produto pa ON pa.id = a.produto_id
              ORDER BY a.codigo",
        )?;
        let linhas = c.query_map([], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, String>(3)?,
                l.get::<_, String>(4)?,
                l.get::<_, String>(5)?,
            ))
        })?;
        for l in linhas {
            let (codigo, id_antigo, nome_antigo, codigo_atual, id, nome) = l?;
            juntar("interno", codigo.clone(), PecaRepetida { id, nome, codigo: codigo.clone(), antigo: false });
            juntar(
                "interno",
                codigo,
                PecaRepetida { id: id_antigo, nome: nome_antigo, codigo: codigo_atual, antigo: true },
            );
        }
    }
    {
        let mut c = conn.prepare(
            "SELECT codigo_barras, id, nome, codigo FROM produto
              WHERE codigo_barras IN (SELECT codigo_barras FROM produto WHERE codigo_barras <> ''
                                       GROUP BY codigo_barras HAVING COUNT(*) > 1)
              ORDER BY codigo_barras, nome",
        )?;
        let linhas = c.query_map([], |l| {
            Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?, l.get::<_, String>(2)?, l.get::<_, String>(3)?))
        })?;
        for l in linhas {
            let (barras, id, nome, codigo) = l?;
            juntar("barras", barras, PecaRepetida { id, nome, codigo, antigo: false });
        }
    }
    Ok(grupos
        .into_iter()
        .filter(|(_, p)| p.len() > 1)
        .map(|((tipo, codigo), pecas)| Repetido { tipo, codigo, pecas })
        .collect())
}

/// Por que esta peça não pode ter etiqueta agora (código a definir, ou um
/// código dela leva também a outra peça). None = pode.
pub fn problema_da_peca(conn: &Connection, produto_id: &str) -> Resultado<Option<String>> {
    let (codigo, barras): (String, String) = conn
        .query_row("SELECT codigo, codigo_barras FROM produto WHERE id = ?1", [produto_id], |l| {
            Ok((l.get(0)?, l.get(1)?))
        })
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
    if codigo.is_empty() {
        return Ok(Some(
            "Esta peça está com código a definir: o código sai quando este caixa receber a faixa de códigos.".into(),
        ));
    }
    let outra = |sql: &str, valor: &str| -> Resultado<Option<String>> {
        Ok(conn.query_row(sql, params![valor, produto_id], |l| l.get::<_, String>(0)).optional()?)
    };
    if let Some(n) = outra("SELECT nome FROM produto WHERE codigo = ?1 AND id <> ?2 LIMIT 1", &codigo)? {
        return Ok(Some(format!("O código {codigo} também é da peça \"{n}\".")));
    }
    if let Some(n) = outra(
        "SELECT p.nome FROM produto_codigo_antigo a JOIN produto p ON p.id = a.produto_id
          WHERE a.codigo = ?1 AND a.produto_id <> ?2 LIMIT 1",
        &codigo,
    )? {
        return Ok(Some(format!("O código {codigo} já foi da peça \"{n}\" (etiquetas antigas).")));
    }
    if !barras.is_empty() {
        if let Some(n) = outra("SELECT nome FROM produto WHERE codigo_barras = ?1 AND id <> ?2 LIMIT 1", &barras)? {
            return Ok(Some(format!("O código de barras {barras} também é da peça \"{n}\".")));
        }
    }
    Ok(None)
}

/// Tudo sobre os códigos, para a pílula Gerente.
pub fn situacao(conn: &Connection) -> Resultado<Value> {
    let (eu, g) = (dispositivo(conn)?, geracao(conn)?);
    let dist = distribuidor(conn)?;
    let u = ultimo(conn, &g)?;
    let mut minhas = Vec::new();
    for f in minhas_livres(conn)? {
        let proximo = proximo_em(conn, &f, u)?;
        minhas.push(json!({
            "inicio": f.inicio, "fim": f.fim,
            "proximo": proximo,
            "restantes": proximo.map(|c| f.fim - c + 1).unwrap_or(0),
        }));
    }
    let mut todas = Vec::new();
    for f in faixas(conn)? {
        todas.push(json!({
            "inicio": f.inicio, "fim": f.fim,
            "caixa": nome_do_dono(conn, &f)?,
            "atual": f.dispositivo == eu && f.geracao == g,
            "antiga": f.dispositivo == eu && f.geracao != g,
        }));
    }
    let mut cruzam = Vec::new();
    for (a, b) in cruzadas(conn)? {
        cruzam.push(json!({
            "inicio": a.inicio.max(b.inicio), "fim": a.fim.min(b.fim),
            "caixas": [nome_do_dono(conn, &a)?, nome_do_dono(conn, &b)?],
        }));
    }
    let (pendentes, deste): (i64, i64) =
        conn.query_row("SELECT COUNT(*), TOTAL(dispositivo = ?1) FROM produto WHERE codigo = ''", [&eu], |l| {
            Ok((l.get(0)?, l.get::<_, f64>(1)? as i64))
        })?;
    let inventario: Option<Inventario> =
        config::ler(conn, "codigos_inventario")?.and_then(|t| serde_json::from_str(&t).ok());
    Ok(json!({
        "distribuidor": match &dist {
            Some(d) => json!({ "nome": crate::sync::nome_do_caixa(conn, d)?, "esteCaixa": *d == eu }),
            None => Value::Null,
        },
        "minhas": minhas,
        "restantes": restantes(conn)?,
        "faixas": todas,
        "cruzadas": cruzam,
        "pendentes": pendentes,
        "pendentesDeste": deste,
        "repetidos": repetidos(conn)?,
        "inventario": inventario,
        "agora": self::inventario(conn)?,
    }))
}

/// Quantas coisas sobre os códigos o gerente precisa ver (para o aviso do
/// Início): códigos que levam a mais de uma peça e faixas cruzadas.
pub fn para_ver(conn: &Connection) -> Resultado<i64> {
    Ok(repetidos(conn)?.len() as i64 + cruzadas(conn)?.len() as i64)
}

#[cfg(test)]
pub(crate) fn tamanho_de_teste(conn: &Connection, t: i64) {
    config::gravar(conn, "codigos_tamanho_teste", &t.to_string()).unwrap();
}

/// Ajudantes dos testes: lojas e caixas em memória e a troca de dados entre
/// eles (o mesmo log e o mesmo pedido de faixa da troca pela rede, sem a
/// rede).
#[cfg(test)]
pub(crate) mod para_testes {
    use super::*;
    use crate::banco::Banco;
    use crate::{produtos, sync, usuarios};

    /// Loja configurada como na primeira tela: gerente, nome, e este caixa
    /// distribui as faixas. `tamanho` = tamanho das faixas (0 = o normal).
    pub fn loja_com(nome: &str, tamanho: i64) -> (Banco, String) {
        let b = Banco::em_memoria().unwrap();
        let u = configurar(&b, nome, tamanho);
        (b, u)
    }

    pub fn loja(nome: &str) -> (Banco, String) {
        loja_com(nome, 0)
    }

    /// Configura `b` como uma loja nova (usuário gerente "Ana").
    pub fn configurar(b: &Banco, nome: &str, tamanho: i64) -> String {
        if tamanho > 0 {
            tamanho_de_teste(&b.conn, tamanho);
        }
        config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
        config::gravar(&b.conn, "loja_nome", "Loja Teste").unwrap();
        config::gravar(&b.conn, "caixa_nome", nome).unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Ana", "1234", true).unwrap().id;
        usuarios::definir_gerente(&b.conn, &u, true).unwrap();
        comecar_loja(&b.conn).unwrap();
        u
    }

    /// Caixa novo que vai entrar na loja (sem usuários: recebe tudo).
    pub fn caixa_novo(nome: &str) -> Banco {
        let b = Banco::em_memoria().unwrap();
        config::gravar(&b.conn, "caixa_nome", nome).unwrap();
        config::gravar(&b.conn, "codigos_espera_teste", "0").unwrap();
        b
    }

    /// `para` recebe tudo o que `de` tem no log (como na troca de dados).
    pub fn receber(para: &mut Banco, de: &Banco) -> usize {
        let (lista, _) = sync::alteracoes_desde(&de.conn, 0, 10_000_000).unwrap();
        sync::aplicar(&mut para.conn, &lista).unwrap()
    }

    pub fn trocar(a: &mut Banco, b: &mut Banco) {
        receber(a, b);
        receber(b, a);
        manter(&mut a.conn).unwrap();
        manter(&mut b.conn).unwrap();
    }

    /// Como depois de puxar do distribuidor: recebe tudo dele e, se precisa,
    /// pede a faixa; depois a manutenção dá código às peças a definir.
    pub fn pedir_faixa(caixa: &mut Banco, dist: &mut Banco) -> usize {
        receber(caixa, dist);
        let mut n = 0;
        if precisa(&caixa.conn).unwrap() {
            let mut pedido = pedido(&caixa.conn).unwrap();
            pedido["tipo"] = json!("faixa");
            pedido["protocolo"] = json!(sync::PROTOCOLO);
            let r = sync::responder_faixa(&mut dist.conn, &pedido).unwrap();
            n = sync::gravar_faixa_recebida(&mut caixa.conn, &r).unwrap();
        }
        manter(&mut caixa.conn).unwrap();
        n
    }

    /// Cadastra uma peça com o código em branco (o Rustinho dá).
    pub fn peca(b: &Banco, u: &str, nome: &str) -> String {
        peca_com(b, u, nome, "").unwrap()
    }

    pub fn peca_com(b: &Banco, u: &str, nome: &str, codigo: &str) -> Resultado<String> {
        let mut f = produtos::ficha_nova(&b.conn)?;
        f.nome = nome.into();
        f.codigo = codigo.into();
        f.custo = "10".into();
        if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
            p.preco = "20,00".into();
            p.lucro = String::new();
        }
        produtos::salvar(&b.conn, &b.dispositivo, u, &f)
    }

    pub fn codigo(b: &Banco, id: &str) -> String {
        b.conn.query_row("SELECT codigo FROM produto WHERE id = ?1", [id], |l| l.get(0)).unwrap()
    }

    /// (id, código) de todas as peças, em ordem de id.
    pub fn todas(b: &Banco) -> Vec<(String, String)> {
        let mut c = b.conn.prepare("SELECT id, codigo FROM produto ORDER BY id").unwrap();
        let l = c.query_map([], |l| Ok((l.get(0)?, l.get(1)?))).unwrap();
        l.collect::<Result<_, _>>().unwrap()
    }

    /// Códigos (não vazios) que estão em mais de uma peça.
    pub fn codigos_repetidos(b: &Banco) -> Vec<String> {
        let mut c = b
            .conn
            .prepare(
                "SELECT codigo FROM produto WHERE codigo <> '' GROUP BY codigo HAVING COUNT(*) > 1 ORDER BY codigo",
            )
            .unwrap();
        let l = c.query_map([], |l| l.get(0)).unwrap();
        l.collect::<Result<_, _>>().unwrap()
    }

    /// As faixas livres deste caixa: (início, fim).
    pub fn minhas(b: &Banco) -> Vec<(i64, i64)> {
        minhas_livres(&b.conn).unwrap().into_iter().map(|f| (f.inicio, f.fim)).collect()
    }
}

#[cfg(test)]
mod testes {
    use super::para_testes::*;
    use super::*;
    use crate::banco::Banco;
    use crate::{busca, etiquetas, produtos};

    fn temp(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rustinho-faixas-{nome}-{}", tempo::novo_id()));
        std::fs::create_dir_all(&p).unwrap();
        p.join("rustinho.db")
    }

    fn erro_do_codigo(r: Resultado<String>) -> String {
        match r {
            Err(Erro::Validacao { campo: Some(c), mensagem }) if c == "codigo" => mensagem,
            outro => panic!("esperava erro no código, veio {outro:?}"),
        }
    }

    /// a) Dois caixas sem rede cadastram peças; um terceiro entrou na loja e
    /// caiu da rede antes de receber a faixa. Depois da troca, os três têm as
    /// mesmas peças com os mesmos códigos, nenhum código repetido, e nenhum
    /// código que já existia mudou.
    #[test]
    fn a_dois_caixas_sem_rede_criam_pecas() {
        let (mut a, u) = loja("Caixa A");
        let mut b = caixa_novo("Caixa B");
        pedir_faixa(&mut b, &mut a);
        assert_eq!(minhas(&a), vec![(1, 99_999)]);
        assert_eq!(minhas(&b), vec![(100_000, 199_999)]);

        // Sem rede: cada um cadastra 30 peças.
        let mut de_a = Vec::new();
        let mut de_b = Vec::new();
        for i in 0..30 {
            de_a.push(peca(&a, &u, &format!("Peça A{i}")));
            de_b.push(peca(&b, &u, &format!("Peça B{i}")));
        }
        let codigos_a: Vec<String> = de_a.iter().map(|id| codigo(&a, id)).collect();
        let codigos_b: Vec<String> = de_b.iter().map(|id| codigo(&b, id)).collect();
        assert_eq!(codigos_a, (1..=30).map(|n| n.to_string()).collect::<Vec<_>>());
        assert_eq!(codigos_b, (100_000..100_030).map(|n| n.to_string()).collect::<Vec<_>>());

        // C entrou, recebeu o cadastro e caiu antes de pedir a faixa.
        let mut c = caixa_novo("Caixa C");
        receber(&mut c, &a);
        let f = produtos::ficha_nova(&c.conn).unwrap();
        assert_eq!((f.codigo.as_str(), f.codigo_previsto.as_str()), ("", ""));
        assert!(f.codigo_aviso.contains("código a definir"), "{}", f.codigo_aviso);
        let de_c: Vec<String> = (0..3).map(|i| peca(&c, &u, &format!("Peça C{i}"))).collect();
        assert!(de_c.iter().all(|id| codigo(&c, id).is_empty()), "sem faixa: código a definir");
        // Escrever um código de números à mão também não passa.
        assert!(erro_do_codigo(peca_com(&c, &u, "À mão", "100005")).contains("de 100000 a 199999 são do Caixa B"));
        assert!(erro_do_codigo(peca_com(&c, &u, "À mão", "777")).contains("de 1 a 99999 são do Caixa A"));
        assert!(erro_do_codigo(peca_com(&c, &u, "À mão", "250000")).contains("não é da faixa deste"));

        // A rede volta: todos trocam com todos.
        trocar(&mut a, &mut b);
        trocar(&mut a, &mut c);
        pedir_faixa(&mut c, &mut a);
        trocar(&mut b, &mut c);
        trocar(&mut a, &mut b);
        trocar(&mut a, &mut c);

        // Convergem: as mesmas peças, com os mesmos códigos, nos três.
        let ta = todas(&a);
        assert_eq!(ta.len(), 63);
        assert_eq!(ta, todas(&b));
        assert_eq!(ta, todas(&c));
        // Nenhum repetido, nenhum a definir, os de antes iguais.
        for x in [&a, &b, &c] {
            assert!(codigos_repetidos(x).is_empty());
            assert!(repetidos(&x.conn).unwrap().is_empty());
            assert!(cruzadas(&x.conn).unwrap().is_empty());
            assert_eq!(de_a.iter().map(|id| codigo(x, id)).collect::<Vec<_>>(), codigos_a);
            assert_eq!(de_b.iter().map(|id| codigo(x, id)).collect::<Vec<_>>(), codigos_b);
        }
        let codigos_c: Vec<String> = de_c.iter().map(|id| codigo(&a, id)).collect();
        assert_eq!(codigos_c, ["200000", "200001", "200002"], "as de C receberam código da faixa de C");
    }

    /// b) Um computador novo entra numa loja com milhares de peças.
    #[test]
    fn b_computador_novo_entra_numa_loja_com_milhares_de_pecas() {
        let (mut a, u) = loja("Caixa A");
        {
            let tx = a.conn.unchecked_transaction().unwrap();
            for i in 0..3000 {
                let mut f = produtos::ficha_nova(&tx).unwrap();
                f.nome = format!("Peça {i}");
                f.custo = "10".into();
                produtos::salvar(&tx, &a.dispositivo, &u, &f).unwrap();
            }
            tx.commit().unwrap();
        }
        assert_eq!(previsto(&a.conn).unwrap().as_deref(), Some("3001"));

        let mut n = caixa_novo("Computador novo");
        // Recebeu o cadastro, mas a rede caiu antes da faixa: nada de "1".
        receber(&mut n, &a);
        let p = peca(&n, &u, "Feita antes da faixa");
        assert_eq!(codigo(&n, &p), "");
        assert!(problema_da_peca(&n.conn, &p).unwrap().unwrap().contains("a definir"));
        assert!(!etiquetas::etiqueta(&n.conn, &p).unwrap().bloqueio.is_empty(), "sem código, sem etiqueta");
        // Recebe a faixa: a peça a definir ganha o primeiro código dela.
        pedir_faixa(&mut n, &mut a);
        assert_eq!(minhas(&n), vec![(100_000, 199_999)]);
        assert_eq!(codigo(&n, &p), "100000");
        let q = peca(&n, &u, "Feita depois");
        assert_eq!(codigo(&n, &q), "100001");
        assert!(etiquetas::etiqueta(&n.conn, &q).unwrap().bloqueio.is_empty());
        receber(&mut a, &n);
        assert_eq!(todas(&a), todas(&n));
        assert_eq!(todas(&a).len(), 3002);
        assert!(codigos_repetidos(&a).is_empty());
        // O inventário ficou guardado quando a loja começou.
        let inv: Inventario =
            serde_json::from_str(&config::ler(&a.conn, "codigos_inventario").unwrap().unwrap()).unwrap();
        assert_eq!(inv.pecas, 0);
        assert_eq!(inventario(&a.conn).unwrap().maior, 100_001);
    }

    /// c) Caixa sem rede quase acaba a faixa, e depois acaba (faixas de 100
    /// números neste teste). Nada sai da faixa, nada repete; quando a rede
    /// volta, ele recebe outra faixa e as peças a definir ganham código.
    #[test]
    fn c_caixa_sem_rede_esgota_a_faixa() {
        let (mut a, u) = loja_com("Caixa A", 100);
        assert_eq!(minhas(&a), vec![(1, 99)]);
        let mut b = caixa_novo("Caixa B");
        pedir_faixa(&mut b, &mut a);
        assert_eq!(minhas(&b), vec![(100, 199)]);
        let mut ids = Vec::new();
        for i in 0..89 {
            ids.push(peca(&b, &u, &format!("P{i}")));
        }
        assert_eq!(restantes(&b.conn).unwrap(), 11);
        assert!(!precisa(&b.conn).unwrap());
        for i in 89..91 {
            ids.push(peca(&b, &u, &format!("P{i}")));
        }
        assert!(precisa(&b.conn).unwrap(), "faltando menos de um décimo, pede a próxima na troca");
        // Continua sem rede até acabar.
        for i in 91..100 {
            ids.push(peca(&b, &u, &format!("P{i}")));
        }
        assert_eq!(restantes(&b.conn).unwrap(), 0);
        let a_definir: Vec<String> = (0..5).map(|i| peca(&b, &u, &format!("Sem faixa {i}"))).collect();
        let codigos: Vec<i64> = ids.iter().map(|id| codigo(&b, id).parse().unwrap()).collect();
        assert_eq!(codigos, (100..200).collect::<Vec<_>>(), "todos dentro da faixa, em ordem, sem repetir");
        assert!(a_definir.iter().all(|id| codigo(&b, id).is_empty()));
        // O distribuidor também acaba a dele e cria a próxima sozinho.
        let de_a: Vec<String> = (0..100).map(|i| peca(&a, &u, &format!("A{i}"))).collect();
        assert_eq!(codigo(&a, &de_a[98]), "99");
        assert_eq!(codigo(&a, &de_a[99]), "200");
        // A rede volta.
        pedir_faixa(&mut b, &mut a);
        assert_eq!(minhas(&b), vec![(100, 199), (300, 399)], "a de antes (acabada) e a nova");
        let novos: Vec<String> = a_definir.iter().map(|id| codigo(&b, id)).collect();
        assert_eq!(novos, ["300", "301", "302", "303", "304"]);
        assert_eq!(ids.iter().map(|id| codigo(&b, id).parse::<i64>().unwrap()).collect::<Vec<_>>(), codigos);
        trocar(&mut a, &mut b);
        assert_eq!(todas(&a), todas(&b));
        assert!(codigos_repetidos(&a).is_empty());
        assert!(!precisa(&b.conn).unwrap());
    }

    /// e) Duas lojas que já tinham os mesmos códigos passam a trocar dados.
    /// Nenhuma peça é renumerada; a repetição aparece para o gerente; a faixa
    /// cruzada não é mais usada por ninguém; os dois continuam dando códigos
    /// sem criar repetição nova.
    #[test]
    fn e_dois_caixas_com_codigos_que_ja_coincidiam() {
        let (mut a, ua) = loja("Caixa A");
        let (mut b, ub) = loja("Caixa B");
        let de_a: Vec<String> = (0..5).map(|i| peca(&a, &ua, &format!("A{i}"))).collect();
        let de_b: Vec<String> = (0..5).map(|i| peca(&b, &ub, &format!("B{i}"))).collect();
        trocar(&mut a, &mut b);
        trocar(&mut a, &mut b);
        assert_eq!(todas(&a), todas(&b));
        for x in [&a, &b] {
            // Códigos de antes ficaram como estavam.
            assert_eq!(de_a.iter().map(|id| codigo(x, id)).collect::<Vec<_>>(), ["1", "2", "3", "4", "5"]);
            assert_eq!(de_b.iter().map(|id| codigo(x, id)).collect::<Vec<_>>(), ["1", "2", "3", "4", "5"]);
            let r = repetidos(&x.conn).unwrap();
            assert_eq!(r.len(), 5);
            assert!(r.iter().all(|g| g.pecas.len() == 2 && g.tipo == "interno"));
            assert_eq!(cruzadas(&x.conn).unwrap().len(), 1, "as duas faixas 1 a 99.999 se cruzam");
            assert!(minhas(x).iter().all(|(i, _)| *i > 99_999), "ninguém dá código na faixa cruzada");
            assert!(para_ver(&x.conn).unwrap() >= 6);
        }
        // Os dois caixas escolhem o mesmo distribuidor.
        let d = distribuidor(&a.conn).unwrap();
        assert_eq!(d, distribuidor(&b.conn).unwrap());
        let (mut dono, mut outro, u_dono, u_outro) =
            if d.as_deref() == Some(a.dispositivo.as_str()) { (a, b, ua, ub) } else { (b, a, ub, ua) };
        let n1 = peca(&dono, &u_dono, "Nova no distribuidor");
        assert_eq!(codigo(&dono, &n1), "100000");
        let n2 = peca(&outro, &u_outro, "Nova no outro");
        assert_eq!(codigo(&outro, &n2), "", "sem faixa livre: a definir");
        pedir_faixa(&mut outro, &mut dono);
        assert_eq!(codigo(&outro, &n2), "200000");
        trocar(&mut dono, &mut outro);
        assert_eq!(todas(&dono), todas(&outro));
        assert_eq!(codigos_repetidos(&dono), ["1", "2", "3", "4", "5"], "nenhuma repetição nova");
        // A peça repetida continua podendo ser alterada sem trocar o código.
        let mut f = produtos::ficha(&dono.conn, &de_a[2]).unwrap();
        assert!(f.codigo_aviso.contains("também é da peça"), "{}", f.codigo_aviso);
        f.observacoes = "conferida".into();
        produtos::salvar(&dono.conn, &dono.dispositivo, &u_dono, &f).unwrap();
        assert_eq!(codigo(&dono, &de_a[2]), "3");
        // Escrever um código repetido à mão não passa.
        assert!(erro_do_codigo(peca_com(&dono, &u_dono, "X", "3")).contains("já é da peça"));
        // Busca exata: as duas; nenhuma é escolhida sozinha (veja `f_...`).
        assert_eq!(busca::pelo_codigo_exato(&dono.conn, "3").unwrap().len(), 2);
    }

    /// f) Busca, leitor, etiquetas e venda diante de uma colisão: código
    /// atual repetido, código antigo de uma igual ao atual de outra, e código
    /// de barras repetido. Nenhuma peça é escolhida sozinha.
    #[test]
    fn f_busca_leitor_etiqueta_e_venda_com_colisao() {
        let (mut a, ua) = loja("Caixa A");
        let (b, ub) = loja("Caixa B");
        let p = peca(&a, &ua, "Filtro de óleo"); // 1
        let q = peca(&b, &ub, "Pastilha de freio"); // 1 em outra loja
        receber(&mut a, &b);
        assert_eq!((codigo(&a, &p), codigo(&a, &q)), ("1".into(), "1".into()));
        // Leitor: as duas, nunca uma só.
        let achou = busca::pelo_codigo_exato(&a.conn, "1").unwrap();
        let mut ids: Vec<&str> = achou.iter().map(|x| x.id.as_str()).collect();
        ids.sort();
        let mut esperado = vec![p.as_str(), q.as_str()];
        esperado.sort();
        assert_eq!(ids, esperado);
        // Etiqueta: não sai para nenhuma das duas.
        for id in [&p, &q] {
            let e = etiquetas::etiqueta(&a.conn, id).unwrap();
            assert!(e.bloqueio.contains("também é da peça"), "{}", e.bloqueio);
            let pedido = etiquetas::Pedido {
                itens: vec![etiquetas::Item { produto_id: id.clone(), quantidade: 1 }],
                modelo: String::new(),
                inicio: 1,
                barras: true,
                loja: false,
                ajuste_x: 0,
                ajuste_y: 0,
            };
            assert!(etiquetas::montar(&a.conn, &pedido).is_err(), "impressão recusada");
        }
        // O gerente resolve trocando o código de uma (controlado, à mão):
        // o código antigo fica guardado e o leitor continua perguntando.
        let mut f = produtos::ficha(&a.conn, &q).unwrap();
        f.codigo = "PF-1".into();
        produtos::salvar(&a.conn, &a.dispositivo, &ua, &f).unwrap();
        assert_eq!(busca::pelo_codigo_exato(&a.conn, "1").unwrap().len(), 2, "etiqueta velha da pastilha");
        assert_eq!(busca::pelo_codigo_exato(&a.conn, "PF-1").unwrap().len(), 1);
        let r = repetidos(&a.conn).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].pecas.iter().any(|x| x.id == q && x.antigo));
        assert!(etiquetas::etiqueta(&a.conn, &q).unwrap().bloqueio.is_empty(), "o código novo da pastilha é só dela");
        assert!(etiquetas::etiqueta(&a.conn, &p).unwrap().bloqueio.contains("já foi da peça"));
        // O código antigo nunca vai para outra peça.
        assert!(erro_do_codigo(peca_com(&a, &ua, "Outra", "1")).contains("já é da peça"));
        let mut g = produtos::ficha(&a.conn, &p).unwrap();
        g.codigo = "FO-1".into();
        produtos::salvar(&a.conn, &a.dispositivo, &ua, &g).unwrap();
        assert!(erro_do_codigo(peca_com(&a, &ua, "Outra", "1")).contains("já foi da peça"));
        // O gerador também não volta a dar o 1 (nem os antigos): a faixa 1 a
        // 99.999 está cruzada com a da outra loja. (Se este caixa não ficou
        // como distribuidor, a peça fica a definir.)
        let nova = peca(&a, &ua, "Nova");
        let c = codigo(&a, &nova);
        assert!(c.is_empty() || c.parse::<i64>().unwrap() >= 100_000, "{c}");

        // Código de barras repetido (duas lojas cadastraram a mesma peça).
        a.conn.execute("UPDATE produto SET codigo_barras = '7891234567895' WHERE id IN (?1, ?2)", [&p, &nova]).unwrap();
        assert!(etiquetas::etiqueta(&a.conn, &p).unwrap().bloqueio.contains("código de barras"));
        assert!(repetidos(&a.conn).unwrap().iter().any(|g| g.tipo == "barras" && g.pecas.len() == 2));

        // Venda: a peça entra pelo id escolhido; o item guarda o código dela.
        crate::caixa::abrir(&a.conn, &a.dispositivo, &ua, "Caixa A", "0").unwrap();
        let e = crate::vendas::VendaEntrada {
            itens: vec![
                crate::vendas::ItemEntrada { produto_id: p.clone(), quantidade: "1".into(), ..Default::default() },
                crate::vendas::ItemEntrada { produto_id: q.clone(), quantidade: "1".into(), ..Default::default() },
            ],
            tabela: 1,
            pagamentos: vec![crate::vendas::PagamentoEntrada {
                forma: "dinheiro".into(),
                valor: "100".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let venda = crate::vendas::concluir(&a.conn, &a.dispositivo, &ua, &e).unwrap();
        let mut c =
            a.conn.prepare("SELECT produto_id, codigo FROM venda_item WHERE venda_id = ?1 ORDER BY codigo").unwrap();
        let itens: Vec<(String, String)> =
            c.query_map([&venda], |l| Ok((l.get(0)?, l.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(itens, vec![(p.clone(), "FO-1".to_string()), (q.clone(), "PF-1".to_string())]);
    }

    /// Banco de uma versão de antes das faixas, com códigos repetidos, acima
    /// de 99.999, com letras e com zero na frente: a conversão confere tudo,
    /// não renumera nada e a primeira faixa cobre o maior código que existe.
    #[test]
    fn migracao_com_codigos_repetidos_e_acima_de_99999_sem_renumerar() {
        let caminho = temp("migracao");
        {
            let mut conn = rusqlite::Connection::open(&caminho).unwrap();
            crate::banco::registrar_funcoes(&conn).unwrap();
            crate::banco::migrar_ate(&mut conn, 12).unwrap();
            conn.execute(
                "INSERT INTO usuario (id, nome, senha_hash, criado_em, atualizado_em, dispositivo, gerente)
                 VALUES ('u1', 'Ana', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'velho', 1)",
                [],
            )
            .unwrap();
            for (id, codigo, nome) in [
                ("p1", "7", "Filtro A"),
                ("p2", "7", "Filtro B"),
                ("p3", "150000", "Correia"),
                ("p4", "A-12", "Vela"),
                ("p5", "007", "Junta"),
                ("p6", "12", "Pastilha"),
                ("p7", "1234567890", "Óleo"),
            ] {
                conn.execute(
                    "INSERT INTO produto (id, codigo, nome, busca, codigos, criado_em, atualizado_em, dispositivo)
                     VALUES (?1, ?2, ?3, lower(?3) || ' ' || lower(?2), ' ' || lower(replace(?2, '-', '')) || ' ',
                             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'velho')",
                    [id, codigo, nome],
                )
                .unwrap();
            }
        }
        let mut a = Banco::abrir(&caminho).unwrap();
        // Conferiu antes de dar a primeira faixa.
        let inv: Inventario =
            serde_json::from_str(&config::ler(&a.conn, "codigos_inventario").unwrap().unwrap()).unwrap();
        assert_eq!(inv.pecas, 7);
        assert_eq!((inv.repetidos, inv.pecas_repetidas), (1, 2));
        assert_eq!(inv.de_numeros, 4);
        assert_eq!((inv.limite, inv.ate_limite, inv.acima_do_limite), (99_999, 3, 1));
        assert_eq!(inv.exemplos_acima, ["150000"]);
        assert_eq!(inv.fora_do_padrao, 2);
        assert_eq!(inv.com_letras, 1);
        assert_eq!(inv.maior, 150_000);
        assert_eq!(inv.exemplos_repetidos, ["7"]);
        // Sozinho (nunca trocou dados): este caixa distribui; a primeira faixa
        // vai até o fim da faixa do 150.000.
        assert!(sou_distribuidor(&a.conn).unwrap());
        assert_eq!(minhas(&a), vec![(1, 199_999)]);
        // Nada renumerado.
        let antes = [
            ("p1", "7"),
            ("p2", "7"),
            ("p3", "150000"),
            ("p4", "A-12"),
            ("p5", "007"),
            ("p6", "12"),
            ("p7", "1234567890"),
        ];
        let ver = |b: &Banco| -> Vec<(String, String)> {
            antes.iter().map(|(id, _)| (id.to_string(), codigo(b, id))).collect()
        };
        let esperado: Vec<(String, String)> = antes.iter().map(|(i, c)| (i.to_string(), c.to_string())).collect();
        assert_eq!(ver(&a), esperado);
        assert_eq!(repetidos(&a.conn).unwrap().len(), 1);
        // A próxima peça vai depois do maior código da faixa.
        let u = "u1";
        let nova = peca(&a, u, "Nova");
        assert_eq!(codigo(&a, &nova), "150001");
        // Peça repetida pode ser alterada sem mudar o código.
        let mut f = produtos::ficha(&a.conn, "p1").unwrap();
        f.custo = "12".into();
        f.precos[0].preco = "25,00".into();
        produtos::salvar(&a.conn, &a.dispositivo, u, &f).unwrap();
        assert_eq!(codigo(&a, "p1"), "7");
        // Outro caixa entra: a faixa dele não tem nenhum código que já existe.
        let mut b = caixa_novo("Caixa B");
        pedir_faixa(&mut b, &mut a);
        assert_eq!(minhas(&b), vec![(200_000, 299_999)]);
        assert_eq!(ver(&b), esperado, "o outro caixa recebe os mesmos códigos");
        // Abrir de novo não muda nada.
        drop(a);
        let a = Banco::abrir(&caminho).unwrap();
        assert_eq!(ver(&a), esperado);
        assert_eq!(minhas(&a), vec![(1, 199_999)]);
    }

    /// Pedidos de faixa ao mesmo tempo (várias conexões no mesmo banco do
    /// distribuidor, como os atendimentos da rede), junto com o distribuidor
    /// criando peças e a própria faixa: nenhuma faixa se cruza.
    #[test]
    fn reserva_concorrente() {
        let caminho = temp("concorrente");
        let a = Banco::abrir(&caminho).unwrap();
        let u = configurar(&a, "Caixa A", 100);
        drop(a);
        let mut tarefas = Vec::new();
        for t in 0..8 {
            let caminho = caminho.clone();
            tarefas.push(std::thread::spawn(move || {
                let mut conn = rusqlite::Connection::open(&caminho).unwrap();
                conn.busy_timeout(Duration::from_secs(30)).unwrap();
                crate::banco::registrar_funcoes(&conn).unwrap();
                for i in 0..5 {
                    let dono = format!("caixa-{t}-{i}");
                    let r = atender_pedido(&mut conn, &dono, "g1", "", &[]).unwrap();
                    assert_eq!(r.len(), 1);
                    // Resposta perdida: pedir de novo devolve a mesma faixa.
                    let de_novo = atender_pedido(&mut conn, &dono, "g1", "", &[]).unwrap();
                    assert_eq!(de_novo, r);
                }
            }));
        }
        {
            let caminho = caminho.clone();
            let u = u.clone();
            tarefas.push(std::thread::spawn(move || {
                let a = Banco::abrir(&caminho).unwrap();
                for i in 0..250 {
                    peca(&a, &u, &format!("A{i}"));
                }
            }));
        }
        for t in tarefas {
            t.join().unwrap();
        }
        let a = Banco::abrir(&caminho).unwrap();
        let todas_faixas = faixas(&a.conn).unwrap();
        assert_eq!(todas_faixas.iter().filter(|f| f.dispositivo.starts_with("caixa-")).count(), 40);
        assert!(cruzadas(&a.conn).unwrap().is_empty(), "nenhuma faixa se cruza");
        assert!(codigos_repetidos(&a).is_empty());
        // As peças do distribuidor ficaram nas faixas dele.
        let dele: Vec<&Faixa> = todas_faixas.iter().filter(|f| f.dispositivo == a.dispositivo).collect();
        for (_, c) in todas(&a) {
            let n: i64 = c.parse().unwrap();
            assert!(dele.iter().any(|f| f.contem(n)), "{n} é de uma faixa do distribuidor");
        }
    }

    /// Tentativas de usar uma faixa já usada: o distribuidor que volta para
    /// trás (cópia antiga do banco por cima), com e sem o arquivo ao lado do
    /// banco; e uma faixa de outro caixa que chega cruzando a de um caixa.
    #[test]
    fn faixa_ja_usada_nao_volta() {
        let caminho = temp("ja-usada");
        let mut a = Banco::abrir(&caminho).unwrap();
        let u = configurar(&a, "Caixa A", 0);
        let copia = caminho.with_file_name("copia.db");
        a.conn.backup(rusqlite::MAIN_DB, &copia, None).unwrap();
        let mut b = caixa_novo("Caixa B");
        pedir_faixa(&mut b, &mut a);
        assert_eq!(minhas(&b), vec![(100_000, 199_999)]);
        let pb = peca(&b, &u, "De B");
        assert_eq!(codigo(&b, &pb), "100000");

        // 1. A volta para trás: a cópia de antes de dar a faixa de B, por cima.
        drop(a);
        for fim in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{fim}", caminho.display()));
        }
        std::fs::copy(&copia, &caminho).unwrap();
        let mut a = Banco::abrir(&caminho).unwrap();
        assert!(faixas(&a.conn).unwrap().iter().all(|f| f.dispositivo != b.dispositivo), "a cópia não tem a de B");
        let mut c = caixa_novo("Caixa C");
        pedir_faixa(&mut c, &mut a);
        assert_eq!(minhas(&c), vec![(200_000, 299_999)], "o arquivo ao lado lembra até onde A foi");

        // 2. A mesma cópia antiga em outro lugar, sem o arquivo: geração nova e
        // o distribuidor pula 20 faixas (não sabe até onde já deu).
        drop(a);
        let outro = temp("ja-usada-outro-lugar");
        std::fs::copy(&copia, &outro).unwrap();
        let mut a2 = Banco::abrir(&outro).unwrap();
        let propria = minhas(&a2);
        assert_eq!(propria, vec![(2_100_000, 2_199_999)], "a faixa de antes ficou para trás; pulou 20");
        let mut d = caixa_novo("Caixa D");
        pedir_faixa(&mut d, &mut a2);
        assert_eq!(minhas(&d), vec![(2_200_000, 2_299_999)]);
        assert!(faixas(&a2.conn).unwrap().iter().all(|f| f.fim < 100_000 || f.inicio > 299_999), "nada cruza B e C");

        // 3. Uma faixa de outro caixa chega cruzando a de B (dois
        // distribuidores ao mesmo tempo): ninguém mais usa o pedaço cruzado.
        let (x, _) = loja("Outra loja");
        x.conn
            .execute(
                "INSERT INTO faixa_codigo (inicio, fim, dispositivo, geracao, distribuidor, criado_em)
                 VALUES (150000, 249999, 'caixa-x', 'g', ?1, '2026-10-17T00:00:00Z')",
                [&x.dispositivo],
            )
            .unwrap();
        receber(&mut b, &x);
        assert!(!cruzadas(&b.conn).unwrap().is_empty());
        assert!(minhas(&b).iter().all(|(i, f)| *f < 150_000 || *i > 249_999), "a faixa cruzada não é usada");
        assert!(erro_do_codigo(peca_com(&b, &u, "À mão", "150500")).contains("de 150000 a 249999"));
        let p = peca(&b, &u, "Depois de cruzar");
        assert_ne!(codigo(&b, &p), "100001");
    }
}
