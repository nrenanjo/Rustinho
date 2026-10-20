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
// Computador onde o banco está.
// ---------------------------------------------------------------------------

/// O banco foi criado em outro computador (a identidade do computador de
/// agora é diferente da que o banco guardou): é uma cópia.
pub const BLOQUEIO_COPIA: &str = "copia";
/// O sistema não informa a identidade deste computador.
pub const BLOQUEIO_SEM_IDENTIDADE: &str = "sem_identidade";
/// O banco é de antes da conferência do computador e o gerente ainda não
/// confirmou que este é o computador de sempre.
pub const BLOQUEIO_SEM_REGISTRO: &str = "sem_registro";

/// Por que este caixa está sem poder dar códigos nem faixas (None = livre).
/// Fica gravado no banco: só uma ação explícita do gerente tira o bloqueio
/// de uma cópia ou de um banco sem registro.
pub fn bloqueio(conn: &Connection) -> Resultado<Option<String>> {
    Ok(config::ler(conn, "codigos_bloqueio")?.filter(|b| !b.is_empty()))
}

/// O texto do bloqueio, para a tela do gerente.
pub fn texto_do_bloqueio(motivo: &str) -> &'static str {
    match motivo {
        BLOQUEIO_COPIA => {
            "Este banco de dados foi criado em outro computador (foi copiado para cá). Para não repetir códigos, \
             este caixa não dá códigos de peças, não distribui faixas e não troca dados com os outros caixas até \
             o gerente registrá-lo como caixa novo."
        }
        BLOQUEIO_SEM_IDENTIDADE => {
            "O Rustinho não conseguiu confirmar em que computador este banco está. Para não repetir códigos, \
             este caixa não dá códigos de peças até a identidade do computador poder ser lida."
        }
        _ => {
            "Este banco é de antes da conferência do computador. Para não repetir códigos, este caixa não dá \
             códigos de peças até o gerente confirmar que este é o computador de sempre, ou registrá-lo como caixa novo."
        }
    }
}

/// Este banco é uma cópia de outro computador (e ninguém o registrou ainda).
pub fn copia_detectada(conn: &Connection) -> Resultado<bool> {
    Ok(bloqueio(conn)?.as_deref() == Some(BLOQUEIO_COPIA))
}

fn ordem_do_bloqueio(motivo: &str) -> u8 {
    match motivo {
        BLOQUEIO_COPIA => 3,
        BLOQUEIO_SEM_REGISTRO => 2,
        _ => 1,
    }
}

/// Ao abrir o banco: compara a identidade do computador de agora com a que
/// o banco guardou. Não olha arquivo nenhum do Rustinho (uma cópia da pasta
/// dos dados leva todos eles), só a identidade que o sistema dá ao computador.
/// - igual: nada muda;
/// - diferente: o banco é uma cópia, bloqueio "copia";
/// - o sistema não informa a identidade: não dá para confirmar, bloqueio;
/// - banco sem identidade guardada: se é novo (sem usuários), guarda a de
///   agora; se já tem usuários (versão antiga), bloqueio até o gerente confirmar.
pub fn conferir_computador(conn: &Connection, atual: Option<&str>) -> Resultado<()> {
    let registrado = config::ler(conn, "computador_id")?.filter(|r| !r.is_empty());
    let motivo = match (registrado.as_deref(), atual) {
        (Some(r), Some(a)) if r == a => {
            // A causa de um bloqueio "sem identidade" passou.
            if bloqueio(conn)?.as_deref() == Some(BLOQUEIO_SEM_IDENTIDADE) {
                conn.execute("DELETE FROM config WHERE chave = 'codigos_bloqueio'", [])?;
            }
            return Ok(());
        }
        (Some(_), Some(_)) => BLOQUEIO_COPIA,
        (_, None) => BLOQUEIO_SEM_IDENTIDADE,
        (None, Some(a)) => {
            let tem_usuarios: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM usuario)", [], |l| l.get(0))?;
            if tem_usuarios {
                BLOQUEIO_SEM_REGISTRO
            } else {
                config::gravar(conn, "computador_id", a)?;
                return Ok(());
            }
        }
    };
    let ja = bloqueio(conn)?;
    if ja.as_deref().map(ordem_do_bloqueio).unwrap_or(0) < ordem_do_bloqueio(motivo) {
        crate::registro::anotar(&format!("códigos das peças: caixa bloqueado ({motivo})"));
        config::gravar(conn, "codigos_bloqueio", motivo)?;
    }
    Ok(())
}

pub fn texto_sem_poder_dar(conn: &Connection) -> Resultado<String> {
    Ok(match bloqueio(conn)? {
        Some(m) => texto_do_bloqueio(&m).to_string(),
        None => "Este caixa ainda não tem faixa de códigos (ou ela acabou). Faça a troca de dados com o caixa que \
                 distribui as faixas."
            .to_string(),
    })
}

/// O gerente confirma que este é o computador de sempre (banco de uma versão
/// sem a conferência do computador). Só vale para o bloqueio "sem_registro".
pub fn confirmar_computador(conn: &Connection, atual: Option<&str>) -> Resultado<()> {
    if bloqueio(conn)?.as_deref() != Some(BLOQUEIO_SEM_REGISTRO) {
        return Err(Erro::validacao("Este caixa não está esperando essa confirmação."));
    }
    let Some(atual) = atual else {
        return Err(Erro::validacao(texto_do_bloqueio(BLOQUEIO_SEM_IDENTIDADE)));
    };
    config::gravar(conn, "computador_id", atual)?;
    conn.execute("DELETE FROM config WHERE chave = 'codigos_bloqueio'", [])?;
    crate::registro::anotar("códigos das peças: o gerente confirmou o computador deste caixa");
    Ok(())
}

/// O gerente registra este banco como um caixa novo (é uma cópia, ou o
/// computador não pôde ser confirmado): identificador novo do caixa, geração
/// de códigos nova e troca de dados do zero com os outros caixas. As faixas
/// de antes (do caixa de onde veio a cópia) nunca são usadas por este caixa;
/// ele pede uma faixa nova ao distribuidor. Devolve o identificador novo.
pub fn registrar_caixa_novo(conn: &Connection, atual: Option<&str>, nome: Option<&str>) -> Resultado<String> {
    if bloqueio(conn)?.is_none() {
        return Err(Erro::validacao("Este caixa não precisa ser registrado de novo."));
    }
    let Some(atual) = atual else {
        return Err(Erro::validacao(
            "Este computador não informa a própria identidade, então não dá para registrá-lo como caixa novo aqui.",
        ));
    };
    let velho = dispositivo(conn)?;
    let novo = tempo::novo_id();
    // A troca de dados recomeça do zero com cada caixa (o banco continua
    // com tudo o que já tinha) e com um identificador de troca novo.
    let seq: i64 = conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM alteracao", [], |l| l.get(0))?;
    config::gravar(conn, "dispositivo_id", &novo)?;
    if let Some(n) = nome.map(str::trim).filter(|n| !n.is_empty()) {
        config::gravar(conn, "caixa_nome", &n.chars().take(60).collect::<String>())?;
    }
    crate::sync::depois_de_restaurar(conn, seq)?;
    crate::registro::anotar(&format!("códigos das peças: banco registrado como caixa novo (antes {velho})"));
    // `depois_de_restaurar` marcou o banco como "de antes das faixas" se não
    // havia distribuidor; aqui o distribuidor continua sendo quem era.
    nova_geracao(conn, "registrado como caixa novo")?;
    // Por último (a restauração acima também mexe nisto): a identidade deste
    // computador fica registrada e o bloqueio sai.
    config::gravar(conn, "computador_id", atual)?;
    conn.execute("DELETE FROM config WHERE chave = 'codigos_bloqueio'", [])?;
    Ok(novo)
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
    if bloqueio(conn)?.is_some() {
        return Ok(None);
    }
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
    if bloqueio(conn)?.is_some() {
        return Ok(0);
    }
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
    if bloqueio(conn)?.is_some() {
        return Ok(false);
    }
    let livres = minhas_livres(conn)?;
    let Some(mais_nova) = livres.iter().max_by_key(|f| f.inicio) else { return Ok(true) };
    Ok(restantes(conn)? < limiar(mais_nova))
}

/// Dá o próximo código deste caixa e guarda que deu (no banco, na mesma
/// transação de quem chamou, e no arquivo ao lado). None = sem faixa.
pub fn gerar(conn: &Connection) -> Resultado<Option<String>> {
    if bloqueio(conn)?.is_some() {
        return Ok(None);
    }
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
    if bloqueio(conn)?.is_some() {
        return Ok(None);
    }
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
    if let Some(m) = bloqueio(conn)? {
        return Err(Erro::validacao(texto_do_bloqueio(&m)));
    }
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
    if config::ler(conn, "codigos_legado")?.as_deref() != Some("1") || bloqueio(conn)?.is_some() {
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
    if let Some(m) = bloqueio(conn)? {
        return Err(Erro::validacao(texto_do_bloqueio(&m)));
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
    if let Some(m) = bloqueio(conn)? {
        return Err(Erro::validacao(texto_do_bloqueio(&m)));
    }
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
    if tem_esperando(conn)? && posso_confirmar(conn)? {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        confirmar_esperando(&tx)?;
        tx.commit()?;
    }
    atribuir_pendentes(conn)
}

/// As peças deste caixa que estão com código a definir recebem código da
/// faixa (as dos outros caixas, eles mesmos dão).
pub fn atribuir_pendentes(conn: &mut Connection) -> Resultado<usize> {
    if bloqueio(conn)?.is_some() {
        return Ok(0);
    }
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
            return Err(Erro::validacao(texto_sem_poder_dar(conn)?));
        };
        if crate::produtos::definir_codigo(conn, &eu, &id, &c)? {
            n += 1;
        }
    }
    Ok(n)
}

/// Ao abrir o banco: confere o arquivo, escolhe o distribuidor (loja de
/// antes das faixas) e faz a manutenção.
pub fn preparar(conn: &mut Connection, computador: Option<&str>) -> Resultado<()> {
    // Nada disto impede o banco de abrir: o que falhar fica no registro e a
    // manutenção roda de novo a cada poucos segundos.
    if let Err(e) = conferir_computador(conn, computador) {
        crate::registro::anotar(&format!("códigos das peças: identidade do computador: {e}"));
    }
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
    // O banco restaurado pode ser de outro computador (backup de um caixa
    // que quebrou): quem restaura é o gerente, e a geração nova já impede o
    // uso da faixa de antes; este computador passa a ser o do banco.
    conn.execute("DELETE FROM config WHERE chave IN ('codigos_bloqueio', 'computador_id')", [])?;
    match crate::computador::atual() {
        Some(a) => config::gravar(conn, "computador_id", &a)?,
        None => config::gravar(conn, "codigos_bloqueio", BLOQUEIO_SEM_IDENTIDADE)?,
    }
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
    if let Some(m) = bloqueio(conn)? {
        return Err(Erro::campo("codigo", format!("{} Use um código com letras (ex.: A-123).", texto_do_bloqueio(&m))));
    }
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
// Confirmação dos códigos que não são de números (letras, traço, zero na frente).
// ---------------------------------------------------------------------------
//
// Um código de números sai da faixa do caixa e, por isso, nunca se repete.
// Um código com letras escrito à mão não tem essa garantia: dois caixas sem
// rede podem escolher o mesmo. Por isso ele só vale depois de confirmado pelo
// caixa que distribui as faixas (o único que decide, numa gravação só):
// - o distribuidor confirma na hora, se ninguém mais tem o código;
// - os outros caixas deixam a peça "esperando" (`produto.codigo_validado = 0`);
//   quando a peça chega ao distribuidor pela troca de dados, ele confirma (se
//   o código é livre) ou deixa esperando (se já é de outra peça: conflito,
//   que o gerente vê). Nada é renumerado nem trocado sozinho;
// - esperando: a peça vende e é achada pelo nome, mas a etiqueta não sai e o
//   leitor não acha a peça por esse código;
// - os códigos que já existiam antes desta versão valem como confirmados.

/// Este código precisa de confirmação (não é de números).
pub fn exige_confirmacao(codigo: &str) -> bool {
    !codigo.is_empty() && numero(codigo).is_none()
}

/// Este caixa pode confirmar códigos: é o distribuidor e não está bloqueado.
pub fn posso_confirmar(conn: &Connection) -> Resultado<bool> {
    Ok(bloqueio(conn)?.is_none() && sou_distribuidor(conn)?)
}

/// Nenhuma outra peça tem este código confirmado (nem o teve: código antigo).
fn codigo_livre(conn: &Connection, produto_id: &str, codigo: &str) -> Resultado<bool> {
    let ocupado: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM produto WHERE codigo = ?1 AND id <> ?2 AND codigo_validado = 1)
             OR EXISTS (SELECT 1 FROM produto_codigo_antigo WHERE codigo = ?1 AND produto_id <> ?2)",
        params![codigo, produto_id],
        |l| l.get(0),
    )?;
    Ok(!ocupado)
}

/// O valor de `produto.codigo_validado` para uma peça que está sendo salva
/// com este código (código novo ou trocado): 1 = vale, 0 = espera a
/// confirmação do distribuidor.
pub fn confirmado_ao_salvar(conn: &Connection, produto_id: &str, codigo: &str) -> Resultado<i64> {
    if !exige_confirmacao(codigo) {
        return Ok(1);
    }
    Ok((posso_confirmar(conn)? && codigo_livre(conn, produto_id, codigo)?) as i64)
}

/// Confirma (no distribuidor, dentro de uma transação de escrita) os códigos
/// que estão esperando e são livres, na ordem em que as peças foram criadas:
/// se duas peças esperam o mesmo código, a mais antiga fica com ele e a outra
/// continua esperando, em conflito. Devolve quantos foram confirmados.
pub fn confirmar_esperando(conn: &Connection) -> Resultado<usize> {
    if !posso_confirmar(conn)? {
        return Ok(0);
    }
    let esperando: Vec<(String, String)> = conn
        .prepare_cached(
            "SELECT id, codigo FROM produto WHERE codigo_validado = 0 AND codigo <> '' ORDER BY criado_em, id LIMIT 2000",
        )?
        .query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut n = 0;
    for (id, codigo) in esperando {
        if !exige_confirmacao(&codigo) || codigo_livre(conn, &id, &codigo)? {
            // Só o valor da coluna muda (a peça em si não é alterada).
            conn.execute("UPDATE produto SET codigo_validado = 1 WHERE id = ?1 AND codigo_validado = 0", [&id])?;
            n += 1;
        }
    }
    Ok(n)
}

fn tem_esperando(conn: &Connection) -> Resultado<bool> {
    Ok(conn.query_row("SELECT EXISTS (SELECT 1 FROM produto WHERE codigo_validado = 0 AND codigo <> '')", [], |l| {
        l.get(0)
    })?)
}

/// A peça espera a confirmação do código.
pub fn esperando_confirmacao(conn: &Connection, produto_id: &str) -> Resultado<bool> {
    Ok(conn
        .query_row("SELECT codigo_validado = 0 AND codigo <> '' FROM produto WHERE id = ?1", [produto_id], |l| {
            l.get::<_, bool>(0)
        })
        .optional()?
        .unwrap_or(false))
}

/// As peças que esperam a confirmação do código, para o gerente: com o
/// conflito (outra peça com o mesmo código) quando há.
pub fn esperando(conn: &Connection) -> Resultado<Vec<Value>> {
    let mut c = conn.prepare(
        "SELECT p.id, p.nome, p.codigo, p.dispositivo,
                (SELECT o.nome FROM produto o WHERE o.codigo = p.codigo AND o.id <> p.id
                  ORDER BY o.codigo_validado DESC, o.criado_em, o.id LIMIT 1)
           FROM produto p WHERE p.codigo_validado = 0 AND p.codigo <> ''
          ORDER BY p.criado_em, p.id LIMIT 200",
    )?;
    let linhas: Vec<(String, String, String, String, Option<String>)> =
        c.query_map([], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?, l.get(4)?)))?.collect::<Result<_, _>>()?;
    let mut r = Vec::new();
    for (id, nome, codigo, disp, outra) in linhas {
        r.push(json!({
            "id": id, "nome": nome, "codigo": codigo,
            "caixa": crate::sync::nome_do_caixa(conn, &disp)?,
            "conflito": outra.is_some(),
            "outraPeca": outra.unwrap_or_default(),
        }));
    }
    Ok(r)
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
    if esperando_confirmacao(conn, produto_id)? {
        return Ok(Some(format!(
            "O código {codigo} ainda não foi confirmado pelo caixa que distribui as faixas. Enquanto isso a peça vende \
             e é achada pelo nome, mas a etiqueta não sai e o leitor não a acha por este código."
        )));
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
    let bloqueio_json = match bloqueio(conn)? {
        Some(m) => {
            let tem_identidade = crate::computador::atual().is_some();
            json!({
                "motivo": m,
                "texto": texto_do_bloqueio(&m),
                "podeConfirmar": m == BLOQUEIO_SEM_REGISTRO && tem_identidade,
                "podeRegistrar": tem_identidade,
            })
        }
        None => Value::Null,
    };
    Ok(json!({
        "bloqueio": bloqueio_json,
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
        "esperando": esperando(conn)?,
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
        // (Duas lojas juntadas: a configuração da outra loja trocou quem
        // distribui; o gerente escolhe este caixa, que confirma os códigos
        // com letras.)
        assumir(&a.conn).unwrap();
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
        // Banco de antes da conferência do computador: nenhum código novo, e
        // a primeira faixa só sai depois de o gerente confirmar o computador.
        assert_eq!(bloqueio(&a.conn).unwrap().as_deref(), Some(BLOQUEIO_SEM_REGISTRO));
        assert_eq!(gerar_para_peca(&a.conn).unwrap(), None);
        assert!(config::ler(&a.conn, "codigos_inventario").unwrap().is_none());
        confirmar_computador(&a.conn, crate::computador::atual().as_deref()).unwrap();
        manter(&mut a.conn).unwrap();
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
        // Os códigos com letras (e os outros) de antes valem como confirmados.
        let esperando_n: i64 =
            a.conn.query_row("SELECT COUNT(*) FROM produto WHERE codigo_validado = 0", [], |l| l.get(0)).unwrap();
        assert_eq!(esperando_n, 0);
        assert_eq!(busca::pelo_codigo_exato(&a.conn, "A-12").unwrap().len(), 1);
        assert!(etiquetas::etiqueta(&a.conn, "p4").unwrap().bloqueio.is_empty());
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

    // -----------------------------------------------------------------------
    // Cópia do banco para outro computador (identidade do computador).
    // -----------------------------------------------------------------------

    fn copiar_pasta(de: &std::path::Path, para: &std::path::Path) {
        std::fs::create_dir_all(para).unwrap();
        for e in std::fs::read_dir(de).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_file() {
                std::fs::copy(e.path(), para.join(e.file_name())).unwrap();
            }
        }
    }

    fn esvaziar_pasta(p: &std::path::Path) {
        for e in std::fs::read_dir(p).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_file() {
                std::fs::remove_file(e.path()).unwrap();
            }
        }
    }

    fn pasta_de(caminho: &std::path::Path) -> PathBuf {
        caminho.parent().unwrap().to_path_buf()
    }

    fn eu_pc(nome: &str) -> String {
        crate::computador::impressao(nome)
    }

    /// g) O banco e o arquivo `rustinho.db.codigos` são copiados para outro
    /// computador e ficam no mesmo caminho, com o mesmo identificador de
    /// caixa e a mesma geração de códigos. Só a identidade do computador
    /// mostra que é uma cópia: o caixa copiado não dá código nenhum (as
    /// peças ficam com código a definir), não distribui faixa, não aceita
    /// código só de números nem troca dados, e continua assim depois de
    /// fechar e abrir.
    #[test]
    fn g_banco_copiado_com_o_arquivo_dos_codigos_para_outro_computador_nao_da_codigo() {
        let caminho = temp("copia-integral");
        let pasta = pasta_de(&caminho);
        let (u, g_a, d_a);
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            u = configurar(&a, "Caixa A", 0);
            let p1 = peca(&a, &u, "Filtro");
            assert_eq!(codigo(&a, &p1), "1");
            assert_eq!(bloqueio(&a.conn).unwrap(), None);
            g_a = geracao(&a.conn).unwrap();
            d_a = a.dispositivo.clone();
        }
        assert!(std::path::Path::new(&format!("{}.codigos", caminho.display())).exists());
        // A pasta inteira vai para o outro computador.
        let levada = pasta_de(&temp("copia-integral-levada"));
        copiar_pasta(&pasta, &levada);

        // O caixa de sempre continua a dar códigos (o outro computador não vê).
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            assert_eq!(bloqueio(&a.conn).unwrap(), None);
            assert_eq!(geracao(&a.conn).unwrap(), g_a);
            let p2 = peca(&a, &u, "Vela");
            assert_eq!(codigo(&a, &p2), "2");
        }

        // No outro computador, a cópia está no mesmo caminho.
        esvaziar_pasta(&pasta);
        copiar_pasta(&levada, &pasta);
        let b = Banco::abrir_em(&caminho, Some("pc-b")).unwrap();
        assert_eq!(b.dispositivo, d_a, "a cópia tem o mesmo identificador de caixa");
        assert_eq!(geracao(&b.conn).unwrap(), g_a, "e a mesma geração de códigos (o arquivo ao lado bate)");
        assert_eq!(bloqueio(&b.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
        assert!(copia_detectada(&b.conn).unwrap());
        // Nada de código: nem da faixa que a cópia herdou. E nenhuma faixa
        // nova é criada para si enquanto bloqueado.
        let faixas_antes = faixas(&b.conn).unwrap().len();
        assert_eq!(gerar(&b.conn).unwrap(), None);
        assert_eq!(gerar_para_peca(&b.conn).unwrap(), None);
        assert_eq!(previsto(&b.conn).unwrap(), None);
        assert_eq!(restantes(&b.conn).unwrap(), 0);
        assert!(!precisa(&b.conn).unwrap(), "não pede faixa");
        let nova = peca(&b, &u, "Correia");
        assert_eq!(codigo(&b, &nova), "", "fica com código a definir");
        let mut b = b;
        assert_eq!(manter(&mut b.conn).unwrap(), 0);
        assert_eq!(codigo(&b, &nova), "");
        assert_eq!(faixas(&b.conn).unwrap().len(), faixas_antes, "bloqueado não cria faixa");
        // Código só de números, nem o que seria o próximo da faixa herdada.
        let erro = erro_do_codigo(peca_com(&b, &u, "À mão", "2"));
        assert!(erro.contains("criado em outro computador"), "{erro}");
        // Não distribui faixa (esta cópia era o distribuidor).
        assert!(sou_distribuidor(&b.conn).unwrap());
        let r = distribuir_dentro(&b.conn, "outro-caixa", "g", "Caixa X", &[]);
        assert!(matches!(r, Err(Erro::Validacao { .. })), "{r:?}");
        let r = assumir(&b.conn);
        assert!(r.is_ok(), "já era o distribuidor");
        // A ficha da peça nova explica.
        let ficha = produtos::ficha_nova(&b.conn).unwrap();
        assert!(ficha.codigo_aviso.contains("outro computador"), "{}", ficha.codigo_aviso);
        // O gerente vê o bloqueio.
        let sit = situacao(&b.conn).unwrap();
        assert_eq!(sit["bloqueio"]["motivo"], "copia");
        assert_eq!(sit["bloqueio"]["podeRegistrar"], true);
        assert_eq!(sit["bloqueio"]["podeConfirmar"], false);
        // Não troca dados: a troca de dados recusa antes de abrir a rede.
        let r = crate::sync::puxar(&caminho, "127.0.0.1:9", 0);
        assert!(matches!(r, Err(Erro::Validacao { .. })), "{r:?}");
        // Fechou e abriu de novo: continua bloqueado (fica gravado).
        drop(b);
        let b = Banco::abrir_em(&caminho, Some("pc-b")).unwrap();
        assert_eq!(bloqueio(&b.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
        assert_eq!(gerar_para_peca(&b.conn).unwrap(), None);
    }

    /// g) Identidade diferente, ausente ou nunca registrada: o caixa não dá
    /// código; só o registro explícito libera (e só se o computador sabe
    /// dizer quem é). Voltar ao computador de origem não desfaz sozinho o
    /// bloqueio de uma cópia.
    #[test]
    fn g_identidade_divergente_ou_nao_confirmada_bloqueia_e_so_o_registro_libera() {
        let caminho = temp("identidade");
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            configurar(&a, "Caixa A", 0);
            assert_eq!(config::ler(&a.conn, "computador_id").unwrap(), Some(eu_pc("pc-a")));
            // Só a impressão digital fica no banco, nunca o identificador.
            assert!(!config::ler(&a.conn, "computador_id").unwrap().unwrap().contains("pc-a"));
        }
        // O sistema não informa a identidade: não dá para confirmar.
        {
            let a = Banco::abrir_em(&caminho, None).unwrap();
            assert_eq!(bloqueio(&a.conn).unwrap().as_deref(), Some(BLOQUEIO_SEM_IDENTIDADE));
            assert_eq!(gerar_para_peca(&a.conn).unwrap(), None);
            assert!(registrar_caixa_novo(&a.conn, None, None).is_err());
            assert!(confirmar_computador(&a.conn, None).is_err());
            // Sem identidade a troca de dados não é recusada (não há prova de cópia).
            assert!(!copia_detectada(&a.conn).unwrap());
        }
        // O computador de sempre voltou a informar: o bloqueio por falta de
        // identidade passa sozinho.
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            assert_eq!(bloqueio(&a.conn).unwrap(), None);
            assert!(gerar_para_peca(&a.conn).unwrap().is_some());
            // Sem bloqueio, não há o que registrar.
            assert!(registrar_caixa_novo(&a.conn, Some(&eu_pc("pc-a")), None).is_err());
        }
        // Outro computador: cópia. Sem identidade depois, continua cópia.
        {
            let a = Banco::abrir_em(&caminho, Some("pc-b")).unwrap();
            assert_eq!(bloqueio(&a.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
        }
        {
            let a = Banco::abrir_em(&caminho, None).unwrap();
            assert_eq!(bloqueio(&a.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
        }
        // O banco voltar ao computador de origem não libera sozinho.
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            assert_eq!(bloqueio(&a.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
            assert_eq!(gerar_para_peca(&a.conn).unwrap(), None);
            // "Confirmar o computador de sempre" é só para banco sem registro:
            // uma cópia só se libera registrando como caixa novo.
            assert!(confirmar_computador(&a.conn, Some(&eu_pc("pc-a"))).is_err());
            assert_eq!(bloqueio(&a.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
        }
    }

    /// g) O caixa copiado é registrado como caixa novo: identificador e
    /// geração novos, sem faixa; pede uma ao distribuidor e nunca repete um
    /// código do caixa de onde veio a cópia (que continua a dar os dele).
    #[test]
    fn g_caixa_registrado_como_novo_recebe_faixa_nova_e_nao_repete_codigos_do_original() {
        let caminho = temp("registrar-novo");
        let pasta = pasta_de(&caminho);
        let u;
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            u = configurar(&a, "Caixa A", 10);
            for n in ["Filtro", "Vela", "Correia"] {
                peca(&a, &u, n);
            }
        }
        let levada = pasta_de(&temp("registrar-novo-levada"));
        copiar_pasta(&pasta, &levada);
        // O caixa A segue: mais duas peças.
        {
            let a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
            peca(&a, &u, "Pastilha");
            peca(&a, &u, "Disco");
            assert_eq!(todas(&a).iter().filter(|(_, c)| !c.is_empty()).count(), 5);
        }
        let a_final = pasta_de(&temp("registrar-novo-a"));
        copiar_pasta(&pasta, &a_final);

        // A cópia, no mesmo caminho, no outro computador.
        esvaziar_pasta(&pasta);
        copiar_pasta(&levada, &pasta);
        let (novo, g_antes, d_a);
        {
            let b = Banco::abrir_em(&caminho, Some("pc-b")).unwrap();
            d_a = b.dispositivo.clone();
            g_antes = geracao(&b.conn).unwrap();
            assert_eq!(bloqueio(&b.conn).unwrap().as_deref(), Some(BLOQUEIO_COPIA));
            novo = registrar_caixa_novo(&b.conn, Some(&eu_pc("pc-b")), Some("Caixa B")).unwrap();
            assert_ne!(novo, d_a);
            assert_eq!(config::ler(&b.conn, "dispositivo_id").unwrap().as_deref(), Some(novo.as_str()));
            assert_eq!(config::ler(&b.conn, "caixa_nome").unwrap().as_deref(), Some("Caixa B"));
            assert_eq!(config::ler(&b.conn, "computador_id").unwrap(), Some(eu_pc("pc-b")));
            assert_ne!(geracao(&b.conn).unwrap(), g_antes);
            assert_eq!(bloqueio(&b.conn).unwrap(), None);
            // A faixa de antes (do caixa de onde veio a cópia) não é dele.
            assert!(minhas(&b).is_empty());
            assert!(!sou_distribuidor(&b.conn).unwrap());
            assert_eq!(distribuidor(&b.conn).unwrap().as_deref(), Some(d_a.as_str()));
            assert_eq!(gerar_para_peca(&b.conn).unwrap(), None);
            // Troca de dados do zero com todos os caixas.
            let cursores: i64 =
                b.conn.query_row("SELECT COALESCE(SUM(cursor), 0) FROM sync_par", [], |l| l.get(0)).unwrap();
            assert_eq!(cursores, 0);
        }
        // Do outro computador, B continua de onde está (outro caminho: tanto faz).
        let caminho_b = temp("registrar-novo-b");
        copiar_pasta(&pasta, &pasta_de(&caminho_b));
        // O banco de B é o rustinho.db da pasta; o nome do arquivo é o mesmo.
        let mut b = Banco::abrir_em(&caminho_b, Some("pc-b")).unwrap();
        assert_eq!(b.dispositivo, novo);
        assert_eq!(bloqueio(&b.conn).unwrap(), None);
        // O caixa A (onde o original continuou) volta a abrir.
        esvaziar_pasta(&pasta);
        copiar_pasta(&a_final, &pasta);
        let mut a = Banco::abrir_em(&caminho, Some("pc-a")).unwrap();
        assert_eq!(a.dispositivo, d_a);
        assert!(sou_distribuidor(&a.conn).unwrap());

        // B cadastra uma peça antes de ter faixa: código a definir.
        let sem = peca(&b, &u, "Junta");
        assert_eq!(codigo(&b, &sem), "");
        // B pede a faixa nova; A segue dando os códigos dele.
        pedir_faixa(&mut b, &mut a);
        let minhas_b = minhas(&b);
        assert_eq!(minhas_b.len(), 1);
        let depois_a = peca(&a, &u, "Amortecedor");
        let cod_b = codigo(&b, &sem);
        assert!(!cod_b.is_empty(), "recebeu código da faixa nova");
        let n_b: i64 = cod_b.parse().unwrap();
        assert!(n_b >= minhas_b[0].0 && n_b <= minhas_b[0].1);
        // Nenhum código de B está na faixa de A, onde A deu os códigos 1 a 5 e deu mais um.
        let n_a: i64 = codigo(&a, &depois_a).parse().unwrap();
        assert!(!(minhas_b[0].0..=minhas_b[0].1).contains(&n_a));
        assert!(n_b > 5, "não repete os códigos que o original deu depois da cópia");
        trocar(&mut a, &mut b);
        assert!(codigos_repetidos(&a).is_empty());
        assert!(codigos_repetidos(&b).is_empty());
        assert_eq!(todas(&a), todas(&b));
        assert!(cruzadas(&a.conn).unwrap().is_empty());
    }

    // -----------------------------------------------------------------------
    // Códigos com letras: só valem depois de confirmados pelo distribuidor.
    // -----------------------------------------------------------------------

    fn confirmado(b: &Banco, id: &str) -> bool {
        b.conn.query_row("SELECT codigo_validado FROM produto WHERE id = ?1", [id], |l| l.get::<_, i64>(0)).unwrap()
            == 1
    }

    fn vender(b: &Banco, u: &str, ids: &[&str]) -> String {
        crate::caixa::abrir(&b.conn, &b.dispositivo, u, "Caixa", "0").ok();
        let e = crate::vendas::VendaEntrada {
            itens: ids
                .iter()
                .map(|id| crate::vendas::ItemEntrada {
                    produto_id: id.to_string(),
                    quantidade: "1".into(),
                    ..Default::default()
                })
                .collect(),
            tabela: 1,
            pagamentos: vec![crate::vendas::PagamentoEntrada {
                forma: "dinheiro".into(),
                valor: "100".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        crate::vendas::concluir(&b.conn, &b.dispositivo, u, &e).unwrap()
    }

    /// h) Loja de um caixa só: o caixa que distribui é a única autoridade e
    /// o código com letras vale na hora (leitor, etiqueta). Os códigos que
    /// já existiam continuam valendo quando um segundo caixa entra.
    #[test]
    fn h_loja_de_um_caixa_so_confirma_na_hora_e_o_segundo_caixa_recebe_os_codigos_validos() {
        let (mut a, ua) = loja("Caixa A");
        let p = peca_com(&a, &ua, "Filtro", "AB-1").unwrap();
        assert!(confirmado(&a, &p));
        assert_eq!(busca::pelo_codigo_exato(&a.conn, "AB-1").unwrap().len(), 1);
        assert!(etiquetas::etiqueta(&a.conn, &p).unwrap().bloqueio.is_empty());
        assert!(esperando(&a.conn).unwrap().is_empty());
        // Código de números da faixa: sempre valeu.
        let n = peca(&a, &ua, "Vela");
        assert!(confirmado(&a, &n));
        // Um segundo caixa entra: recebe a loja e os códigos continuam válidos.
        let mut b = caixa_novo("Caixa B");
        pedir_faixa(&mut b, &mut a);
        assert!(confirmado(&b, &p));
        assert_eq!(busca::pelo_codigo_exato(&b.conn, "AB-1").unwrap().len(), 1);
        assert!(etiquetas::etiqueta(&b.conn, &p).unwrap().bloqueio.is_empty());
        assert_eq!(para_ver(&b.conn).unwrap(), 0);
    }

    /// h) Um caixa que não distribui deixa o código com letras esperando.
    /// Enquanto espera: vende e acha pelo nome, mas não sai etiqueta e o
    /// leitor não acha a peça por esse código. Quando a peça chega ao
    /// distribuidor, ele confirma (só a coluna muda) e a confirmação volta.
    #[test]
    fn h_codigo_com_letras_espera_a_confirmacao_do_distribuidor() {
        let (mut a, ua) = loja("Caixa A");
        let mut b = caixa_novo("Caixa B");
        pedir_faixa(&mut b, &mut a);
        let p = peca_com(&b, &ua, "Correia dentada", "CD-2").unwrap();
        assert_eq!(codigo(&b, &p), "CD-2", "o código não é trocado");
        assert!(!confirmado(&b, &p));
        // Leitor: não acha por esse código, e a venda avisa que espera.
        assert!(busca::pelo_codigo_exato(&b.conn, "CD-2").unwrap().is_empty());
        assert_eq!(busca::esperando_pelo_codigo(&b.conn, "cd 2").unwrap(), 1);
        // Pelo nome acha; pelo texto do código lista, mas nunca como "exata".
        assert_eq!(busca::buscar(&b.conn, "correia", false, 10).unwrap().len(), 1);
        let lista = busca::buscar(&b.conn, "CD-2", false, 10).unwrap();
        assert!(lista.iter().all(|x| !x.codigo_exato));
        // Etiqueta não sai (nem pela impressão direta).
        let e = etiquetas::etiqueta(&b.conn, &p).unwrap();
        assert!(e.bloqueio.contains("ainda não foi confirmado"), "{}", e.bloqueio);
        let pedido = etiquetas::Pedido {
            itens: vec![etiquetas::Item { produto_id: p.clone(), quantidade: 1 }],
            modelo: String::new(),
            inicio: 1,
            barras: true,
            loja: false,
            ajuste_x: 0,
            ajuste_y: 0,
        };
        assert!(etiquetas::montar(&b.conn, &pedido).is_err());
        assert!(produtos::ficha(&b.conn, &p).unwrap().codigo_aviso.contains("ainda não foi confirmado"));
        // Vende normalmente.
        let venda = vender(&b, &ua, &[&p]);
        let guardado: String =
            b.conn.query_row("SELECT codigo FROM venda_item WHERE venda_id = ?1", [&venda], |l| l.get(0)).unwrap();
        assert_eq!(guardado, "CD-2");
        // O gerente vê a peça esperando, sem conflito.
        let sit = situacao(&b.conn).unwrap();
        assert_eq!(sit["esperando"].as_array().unwrap().len(), 1);
        assert_eq!(sit["esperando"][0]["conflito"], false);
        assert_eq!(para_ver(&b.conn).unwrap(), 0);

        // A peça chega ao distribuidor: ele confirma, sem mexer na peça.
        receber(&mut a, &b);
        let atualizado_antes: String =
            a.conn.query_row("SELECT atualizado_em FROM produto WHERE id = ?1", [&p], |l| l.get(0)).unwrap();
        manter(&mut a.conn).unwrap();
        assert!(confirmado(&a, &p));
        let atualizado_depois: String =
            a.conn.query_row("SELECT atualizado_em FROM produto WHERE id = ?1", [&p], |l| l.get(0)).unwrap();
        assert_eq!(atualizado_antes, atualizado_depois);
        assert_eq!(codigo(&a, &p), "CD-2");
        // A confirmação volta ao caixa B pela troca de dados.
        assert!(!confirmado(&b, &p));
        receber(&mut b, &a);
        assert!(confirmado(&b, &p));
        assert_eq!(busca::pelo_codigo_exato(&b.conn, "CD-2").unwrap().len(), 1);
        assert!(etiquetas::etiqueta(&b.conn, &p).unwrap().bloqueio.is_empty());
        assert!(situacao(&b.conn).unwrap()["esperando"].as_array().unwrap().is_empty());
    }

    /// h) Dois caixas sem rede cadastram o mesmo código com letras: só um
    /// confirma (o distribuidor decide, numa gravação só); o outro continua
    /// esperando, com o conflito visível ao gerente. Nenhum código é trocado
    /// sozinho. O gerente resolve dando um código novo à que esperava, e o
    /// código que ela nunca teve confirmado não fica guardado como antigo.
    #[test]
    fn h_dois_caixas_offline_com_o_mesmo_codigo_so_um_confirma() {
        let (mut a, ua) = loja("Caixa A");
        let mut b = caixa_novo("Caixa B");
        let mut c = caixa_novo("Caixa C");
        pedir_faixa(&mut b, &mut a);
        pedir_faixa(&mut c, &mut a);
        trocar(&mut a, &mut b);
        trocar(&mut a, &mut c);
        // Sem rede: B e C cadastram peças diferentes com o mesmo código.
        let pb = peca_com(&b, &ua, "Vela de ignição", "AB-7").unwrap();
        let pc = peca_com(&c, &ua, "Bomba de combustível", "AB-7").unwrap();
        assert!(!confirmado(&b, &pb) && !confirmado(&c, &pc));
        // Também o próprio distribuidor, sem saber, cadastra outra com ele.
        // (Tem de ser antes de receber as deles: depois, o cadastro recusaria.)
        let pa = peca_com(&a, &ua, "Palheta", "AB-7").unwrap();
        assert!(confirmado(&a, &pa), "o distribuidor confirma na hora");
        // A troca de dados junta tudo.
        receber(&mut a, &b);
        receber(&mut a, &c);
        manter(&mut a.conn).unwrap();
        // Só a do distribuidor tem o código confirmado.
        assert!(confirmado(&a, &pa));
        assert!(!confirmado(&a, &pb) && !confirmado(&a, &pc));
        receber(&mut b, &a);
        receber(&mut c, &a);
        receber(&mut b, &c);
        receber(&mut c, &b);
        for banco in [&a, &b, &c] {
            // Nenhum código foi trocado.
            assert_eq!(codigo(banco, &pa), "AB-7");
            assert_eq!(codigo(banco, &pb), "AB-7");
            assert_eq!(codigo(banco, &pc), "AB-7");
            // O leitor acha só a confirmada.
            let achou = busca::pelo_codigo_exato(&banco.conn, "AB-7").unwrap();
            assert_eq!(achou.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec![pa.as_str()]);
            // As duas que esperam não têm etiqueta, e dizem por quê.
            for id in [&pb, &pc] {
                let bloqueio = etiquetas::etiqueta(&banco.conn, id).unwrap().bloqueio;
                assert!(bloqueio.contains("também é da peça"), "{bloqueio}");
            }
            assert!(etiquetas::etiqueta(&banco.conn, &pa).unwrap().bloqueio.contains("também é da peça"));
            // O gerente vê o conflito.
            let sit = situacao(&banco.conn).unwrap();
            let esp = sit["esperando"].as_array().unwrap();
            assert_eq!(esp.len(), 2);
            assert!(esp.iter().all(|e| e["conflito"] == true && e["outraPeca"] == "Palheta"));
            assert!(para_ver(&banco.conn).unwrap() >= 1);
            assert_eq!(repetidos(&banco.conn).unwrap().len(), 1);
        }
        // Vende normalmente enquanto espera.
        vender(&b, &ua, &[&pb]);

        // O gerente, no distribuidor, resolve as duas que esperavam.
        for id in [&pb, &pc] {
            let novo = produtos::trocar_codigo(&a.conn, &a.dispositivo, &ua, id).unwrap();
            assert!(numero(&novo).is_some());
        }
        let antigos: i64 = a.conn.query_row("SELECT COUNT(*) FROM produto_codigo_antigo", [], |l| l.get(0)).unwrap();
        assert_eq!(antigos, 0, "um código que nunca foi confirmado não é guardado como antigo");
        assert!(confirmado(&a, &pb) && confirmado(&a, &pc));
        assert!(etiquetas::etiqueta(&a.conn, &pa).unwrap().bloqueio.is_empty());
        assert!(repetidos(&a.conn).unwrap().is_empty());
        assert_eq!(para_ver(&a.conn).unwrap(), 0);
        receber(&mut b, &a);
        receber(&mut c, &a);
        assert!(codigos_repetidos(&b).is_empty() && codigos_repetidos(&c).is_empty());
        assert_eq!(todas(&a), todas(&b));
        assert_eq!(todas(&a), todas(&c));
    }

    /// h) Desta vez o caixa B cadastra primeiro e pede a confirmação antes de
    /// o distribuidor ter a mesma ideia: a mais antiga confirma; a outra
    /// espera. E dois pedidos ao mesmo tempo (duas peças esperando o mesmo
    /// código, chegando juntas) confirmam exatamente uma.
    #[test]
    fn h_confirmacao_e_atomica_no_distribuidor() {
        let (mut a, ua) = loja("Caixa A");
        let mut b = caixa_novo("Caixa B");
        let mut c = caixa_novo("Caixa C");
        pedir_faixa(&mut b, &mut a);
        pedir_faixa(&mut c, &mut a);
        let pb = peca_com(&b, &ua, "Peça de B", "ZZ-9").unwrap();
        let pc = peca_com(&c, &ua, "Peça de C", "ZZ-9").unwrap();
        // A de C foi criada antes (a de B, depois): é ela que fica com o código.
        b.conn.execute("UPDATE produto SET criado_em = '2026-10-18T10:00:00Z' WHERE id = ?1", [&pb]).unwrap();
        c.conn.execute("UPDATE produto SET criado_em = '2026-10-18T09:00:00Z' WHERE id = ?1", [&pc]).unwrap();
        // As duas chegam juntas ao distribuidor, na mesma troca.
        receber(&mut a, &b);
        receber(&mut a, &c);
        manter(&mut a.conn).unwrap();
        manter(&mut a.conn).unwrap(); // repetir não muda nada
        let confirmadas = [&pb, &pc].iter().filter(|id| confirmado(&a, id)).count();
        assert_eq!(confirmadas, 1, "exatamente uma confirma");
        // Fica com a que foi criada primeiro.
        assert!(confirmado(&a, &pc) && !confirmado(&a, &pb));
        // Um terceiro cadastro do mesmo código no distribuidor é recusado:
        // já é de outra peça confirmada.
        let r = peca_com(&a, &ua, "Terceira", "ZZ-9");
        assert!(matches!(r, Err(Erro::Validacao { .. })), "{r:?}");
    }

    /// i) O distribuidor se perde (computador quebrado) com uma faixa dada
    /// que os outros caixas ainda não receberam. O gerente faz outro caixa
    /// distribuir: a faixa nova pula 20 faixas, então não cruza com nenhuma
    /// que o distribuidor perdido tenha dado. Os códigos que já existem não
    /// mudam, e os caixas continuam sem repetir.
    #[test]
    fn i_distribuidor_perdido_outro_caixa_assume_sem_repetir() {
        let (mut a, ua) = loja("Caixa A");
        let mut b = caixa_novo("Caixa B");
        let mut c = caixa_novo("Caixa C");
        pedir_faixa(&mut b, &mut a);
        pedir_faixa(&mut c, &mut a);
        peca(&b, &ua, "Peça de B");
        let pc = peca(&c, &ua, "Peça de C");
        trocar(&mut b, &mut c);
        // A ainda deu uma faixa a D, que nunca chegou a B nem a C; depois A se perdeu.
        let mut d = caixa_novo("Caixa D");
        pedir_faixa(&mut d, &mut a);
        let faixa_de_d = minhas(&d);
        assert_eq!(faixa_de_d.len(), 1);
        let pd = peca(&d, &ua, "Peça de D");
        let codigo_de_d: i64 = codigo(&d, &pd).parse().unwrap();
        drop(a);
        // B assume (não é o distribuidor ainda; C continua o dono da faixa dele).
        assert!(!sou_distribuidor(&b.conn).unwrap());
        assumir(&b.conn).unwrap();
        assert!(sou_distribuidor(&b.conn).unwrap());
        manter(&mut b.conn).unwrap();
        // C pede uma faixa nova a B: não cruza com a que A deu a D.
        let tinha_c = minhas(&c);
        receber(&mut c, &b);
        let r = {
            let mut pedido = pedido(&c.conn).unwrap();
            pedido["tipo"] = json!("faixa");
            pedido["protocolo"] = json!(crate::sync::PROTOCOLO);
            pedido["tenho"] = json!(tinha_c.iter().map(|f| f.0).collect::<Vec<_>>());
            crate::sync::responder_faixa(&mut b.conn, &pedido).unwrap()
        };
        crate::sync::gravar_faixa_recebida(&mut c.conn, &r).unwrap();
        let nova_de_c = minhas(&c).into_iter().find(|f| !tinha_c.contains(f)).expect("C recebeu faixa nova");
        assert!(
            nova_de_c.0 > faixa_de_d[0].1 + 10 * TAMANHO_FAIXA,
            "pulou faixas: {nova_de_c:?} depois de {faixa_de_d:?}"
        );
        // D chega depois a B e a C: nada cruza e nada se repete.
        receber(&mut b, &d);
        receber(&mut c, &d);
        receber(&mut b, &c);
        receber(&mut c, &b);
        assert!(cruzadas(&b.conn).unwrap().is_empty());
        assert!(cruzadas(&c.conn).unwrap().is_empty());
        let p_novo = peca(&c, &ua, "Peça nova de C");
        let n_novo: i64 = codigo(&c, &p_novo).parse().unwrap();
        assert_ne!(n_novo, codigo_de_d);
        assert!(codigos_repetidos(&b).is_empty() && codigos_repetidos(&c).is_empty());
        assert_eq!(codigo(&c, &pc), codigo(&b, &pc), "os códigos de antes não mudaram");
        // Os dois (B e D) ainda sem trocar entre si: faixas diferentes.
        trocar(&mut b, &mut d);
        assert!(cruzadas(&d.conn).unwrap().is_empty());
        assert!(codigos_repetidos(&d).is_empty());
    }
}
