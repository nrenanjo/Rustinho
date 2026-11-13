//! A contagem de peças guardada por conexão (PEND-39, passo 7.5): o total do
//! cabeçalho de Peças, refeito só quando o banco mudou. É da busca
//! (`busca::total_guardado`), num módulo próprio e sem dependência de outro
//! módulo do núcleo: assim o backup a esquece ao trocar o banco (R-3) sem
//! criar ciclo entre os módulos (`backup → busca` fecharia `backup → busca →
//! produtos → faixas → sync → banco → backup`; veja `tests/dependencias.rs`).

use std::sync::Mutex;

use rusqlite::Connection;

use crate::erro::Resultado;

/// ((conexão, com as inativas), (data_version, total_changes), peças).
type Guardada = ((i64, bool), (i64, u64), i64);
static GUARDADAS: Mutex<Vec<Guardada>> = Mutex::new(Vec::new());

/// O número que `banco::registrar_funcoes` deu à conexão (`rustinho_conexao()`).
fn numero_da_conexao(conn: &Connection) -> Option<i64> {
    conn.prepare_cached("SELECT rustinho_conexao()").and_then(|mut c| c.query_row([], |l| l.get(0))).ok()
}

/// A contagem guardada desta conexão, se o banco não mudou desde ela: nesta
/// conexão (o número de mudanças dela) ou em outra (`PRAGMA data_version`: a
/// troca de dados, o backup). Senão, `contar` e guarda. Sem o número da
/// conexão, conta sempre.
pub(crate) fn guardada(
    conn: &Connection,
    incluir_inativas: bool,
    contar: impl FnOnce() -> Resultado<i64>,
) -> Resultado<i64> {
    let Some(conexao) = numero_da_conexao(conn) else {
        return contar();
    };
    let chave = (conexao, incluir_inativas);
    let versao: i64 = conn.prepare_cached("PRAGMA data_version")?.query_row([], |l| l.get(0))?;
    let marca = (versao, conn.total_changes());
    let ja =
        GUARDADAS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|g| g.0 == chave && g.1 == marca).map(|g| g.2);
    if let Some(n) = ja {
        return Ok(n);
    }
    let n = contar()?;
    let mut guardadas = GUARDADAS.lock().unwrap_or_else(|e| e.into_inner());
    guardadas.retain(|g| g.0 != chave);
    if guardadas.len() >= 32 {
        guardadas.remove(0);
    }
    guardadas.push((chave, marca, n));
    Ok(n)
}

/// R-3 (revisão cruzada de 13/11): esquece a contagem guardada da conexão. O
/// backup chama logo depois de trocar o banco nela (`conn.restore`): a troca
/// pela API de backup do SQLite não muda o `data_version` nem o
/// `total_changes` desta conexão, e a marca não perceberia.
pub fn esquecer(conn: &Connection) {
    if let Some(conexao) = numero_da_conexao(conn) {
        GUARDADAS.lock().unwrap_or_else(|e| e.into_inner()).retain(|g| g.0 .0 != conexao);
    }
}
