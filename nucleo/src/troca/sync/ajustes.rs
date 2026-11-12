//! Os ajustes da rede, as interfaces, os pares e abrir a conexão.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

#[derive(Debug, Clone, Default)]
pub struct Ajustes {
    pub ligado: bool,
    pub senha: String,
    /// Endereços digitados (ex.: "192.168.0.12").
    pub enderecos: Vec<String>,
    pub dispositivo: String,
    pub nome: String,
    /// Interfaces de VPN/virtual/celular que o gerente liberou pelo nome
    /// (`sync_interfaces`, só deste computador).
    pub interfaces: Vec<String>,
}

pub fn ajustes(conn: &Connection) -> Resultado<Ajustes> {
    Ok(Ajustes {
        ligado: config::ler(conn, "sync_ligado")?.as_deref() == Some("1"),
        senha: config::ler(conn, "sync_senha")?.unwrap_or_default(),
        enderecos: config::ler(conn, "sync_enderecos")?
            .unwrap_or_default()
            .split([',', ' ', ';', '\n'])
            .filter(|e| !e.trim().is_empty())
            .map(|e| e.trim().to_string())
            .collect(),
        dispositivo: config::ler(conn, "dispositivo_id")?.unwrap_or_default(),
        nome: config::nome_caixa(conn)?,
        interfaces: lista_de_interfaces(&config::ler(conn, "sync_interfaces")?.unwrap_or_default()),
    })
}

pub(super) fn lista_de_interfaces(texto: &str) -> Vec<String> {
    texto.split([',', ';', '\n']).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect()
}

/// Nomes das interfaces deste computador que **não** anunciam o caixa (VPN,
/// virtuais, dados móveis sem liberação). Só para a tela de Configurações em
/// modo completo ajudar o gerente a escolher.
pub fn interfaces_ignoradas(liberadas: &[String]) -> Vec<String> {
    let mut nomes: Vec<String> = rede_local::listar()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| i.ativa && !rede_local::anunciavel(i, liberadas))
        .map(|i| i.nome)
        .collect();
    nomes.sort();
    nomes.dedup();
    nomes
}

/// Interfaces (VPN, virtuais, dados móveis) que também anunciam este caixa e
/// servem de caminho, pelo nome, separados por vírgula. Só deste computador.
pub fn salvar_interfaces(conn: &Connection, nomes: &str) -> Resultado<()> {
    let lista = lista_de_interfaces(nomes);
    if let Some(n) = lista.iter().find(|n| n.chars().count() > 64 || n.chars().any(|c| c.is_control())) {
        return Err(Erro::campo("interfaces", format!("Nome de interface inválido: {n}")));
    }
    config::gravar(conn, "sync_interfaces", &lista.join(", "))?;
    Ok(())
}

/// A senha da rede gravada está fora da política? (só informa; não força troca)
pub fn senha_fora_da_politica(conn: &Connection) -> Resultado<bool> {
    let senha = config::ler(conn, "sync_senha")?.unwrap_or_default();
    if senha.is_empty() {
        return Ok(false);
    }
    let loja = config::nome_loja(conn)?;
    let caixa = config::nome_caixa(conn)?;
    let ctx = crate::senhas::Contexto { usuario: "", loja: &loja, caixa: &caixa, gerente: false };
    Ok(crate::senhas::avaliar(&senha, &ctx).is_some())
}

/// Grava as configurações da sincronização (só deste computador).
pub fn salvar_ajustes(conn: &Connection, ligado: bool, senha: &str, enderecos: &str) -> Resultado<()> {
    let senha = senha.trim();
    // Senha da rede nova ou trocada segue a política (F-17: só o tamanho),
    // com a rede ligada ou desligada (A2: uma senha curta gravada com a rede
    // desligada valeria depois, ao ligar). Ligar a rede também confere a
    // senha que vai valer, mesmo já gravada. Com a rede já ligada, manter a
    // senha gravada (o campo vazio da tela) não exige nada: nada é forçado.
    let gravada = config::ler(conn, "sync_senha")?.unwrap_or_default();
    let estava_ligado = config::ler(conn, "sync_ligado")?.as_deref() == Some("1");
    let troca = !senha.is_empty() && senha != gravada;
    let liga = ligado && !estava_ligado;
    if troca || liga || (ligado && senha.is_empty()) {
        if senha.chars().count() < 4 {
            return Err(Erro::campo(
                "senha",
                "A senha da rede precisa ter pelo menos 4 caracteres. Use a mesma em todos os caixas.",
            ));
        }
        crate::usuarios::exigir_politica(conn, senha, "", false, "senha")?;
    }
    for e in enderecos.split([',', ' ', ';', '\n']).filter(|e| !e.trim().is_empty()) {
        let e = e.trim();
        let ok = e.parse::<std::net::IpAddr>().is_ok() || e.parse::<SocketAddr>().is_ok();
        if !ok {
            return Err(Erro::campo("enderecos", format!("Endereço inválido: {e} (ex.: 192.168.0.12)")));
        }
    }
    config::gravar(conn, "sync_ligado", if ligado { "1" } else { "0" })?;
    config::gravar(conn, "sync_senha", senha)?;
    config::gravar(conn, "sync_enderecos", enderecos.trim())?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Par {
    pub dispositivo: String,
    pub nome: String,
    /// O caminho em uso (o último que funcionou) ou, sem nenhum, o melhor conhecido.
    pub endereco: String,
    pub visto_em: String,
    pub sincronizado_em: String,
    pub erro: String,
    /// Por onde a última conexão **confirmou** ter saído ("cabo", "Wi-Fi",
    /// "rede"); vazio quando não se sabe (nunca se inventa).
    pub via: String,
    pub caminhos: Vec<CaminhoPar>,
}

pub fn pares(conn: &Connection) -> Resultado<Vec<Par>> {
    let mut c = conn.prepare(
        "SELECT dispositivo, nome, endereco, COALESCE(visto_em, ''), COALESCE(sincronizado_em, ''), erro
           FROM sync_par ORDER BY nome",
    )?;
    let linhas: Vec<Par> = c
        .query_map([], |l| {
            Ok(Par {
                dispositivo: l.get(0)?,
                nome: l.get(1)?,
                endereco: l.get(2)?,
                visto_em: l.get(3)?,
                sincronizado_em: l.get(4)?,
                erro: l.get(5)?,
                via: String::new(),
                caminhos: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    linhas
        .into_iter()
        .map(|mut p| {
            p.caminhos = caminhos::caminhos_do_par(conn, &p.dispositivo)?;
            if let Some(em_uso) = p.caminhos.iter().find(|x| x.em_uso) {
                p.endereco = em_uso.endereco.clone();
                p.via = em_uso.via.clone();
            }
            Ok(p)
        })
        .collect()
}

/// Um outro caixa se apresentou vindo de `endereco` (veja `caminhos::aprender`).
pub(crate) fn registrar_par(conn: &Connection, dispositivo: &str, nome: &str, endereco: &str) -> Resultado<()> {
    caminhos::aprender(conn, dispositivo, nome, endereco, agora_ms())
}

// ---------------------------------------------------------------------------
// Servidor, descoberta e puxada.
// ---------------------------------------------------------------------------

pub(crate) fn abrir_conexao(caminho: &Path) -> Resultado<Connection> {
    // 0.9.18: banco trocado por uma restauração que não terminou (ela
    // termina ao abrir o Rustinho de novo): a troca de dados espera.
    if banco::restauracao_pela_metade(caminho) {
        return Err(Erro::validacao(banco::RESTAURACAO_PELA_METADE));
    }
    let mut conn = Connection::open(caminho)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    // A3: banco com defeito de integridade (modo seguro): só leitura.
    if banco::defeito(caminho).is_some() {
        conn.pragma_update(None, "query_only", true)?;
    }
    // D-05: espera a vez de gravar em vez de falhar (veja `Banco::preparar`).
    conn.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
    banco::registrar_funcoes(&conn)?;
    Ok(conn)
}
