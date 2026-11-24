//! Puxar, o anúncio e os problemas da porta.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// F89-4: a pausa entre uma resposta e a próxima, mais que um passo da espera
/// do SQLite (100 ms). Custa ~0,12 s a cada 200 alterações recebidas.
pub(super) const VEZ_DO_BALCAO: Duration = Duration::from_millis(120);

/// Puxa de um endereço tudo o que há de novo. Devolve (dispositivo, quantas novas).
pub fn puxar(caminho: &Path, endereco: &str, porta_local: u16) -> Resultado<(String, usize)> {
    puxar_ate(caminho, endereco, porta_local, None)
}

/// Como `puxar`, mas para entre um lote e outro se pedirem (fechar o programa).
pub(super) fn puxar_ate(
    caminho: &Path,
    endereco: &str,
    porta_local: u16,
    parar: Option<&AtomicBool>,
) -> Resultado<(String, usize)> {
    puxar_por(caminho, endereco, porta_local, parar, None).map(|p| (p.dispositivo, p.total))
}

/// O endereço de um caixa como `SocketAddr` (sem porta = a porta da troca).
pub(super) fn endereco_completo(endereco: &str) -> Resultado<SocketAddr> {
    match endereco.parse::<SocketAddr>() {
        Ok(s) => Ok(s),
        Err(_) => format!("{endereco}:{}", crate::portatil::portas().0)
            .parse()
            .map_err(|_| Erro::validacao("Endereço inválido.")),
    }
}

/// Os endereços digitados, já com a porta.
pub(super) fn digitados_completos(a: &Ajustes) -> Vec<String> {
    a.enderecos.iter().filter_map(|e| endereco_completo(e).ok()).map(|s| s.to_string()).collect()
}

/// Puxa de **um caminho** (`endereco`), saindo pela `saida` planejada quando
/// houver. Quem o caminho é — qual caixa — só se sabe pela resposta: o contador,
/// a marca e a geração são do caixa, nunca do endereço. Um caminho novo de um
/// caixa conhecido continua do contador dele.
pub(crate) fn puxar_por(
    caminho: &Path,
    endereco: &str,
    porta_local: u16,
    parar: Option<&AtomicBool>,
    saida: Option<&Saida>,
) -> Resultado<Puxada> {
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    // A3: banco com defeito (modo seguro): não aplica nada de ninguém.
    if banco::defeito(caminho).is_some() {
        return Err(Erro::validacao(TEXTO_MODO_SEGURO));
    }
    let mut conn = abrir_conexao(caminho)?;
    if crate::faixas::copia_detectada(&conn)? {
        return Err(Erro::validacao(crate::faixas::texto_do_bloqueio(crate::faixas::BLOQUEIO_COPIA)));
    }
    let a = ajustes(&conn)?;
    // A chave da senha da rede (protocolo 3) só existe com a senha (T009).
    let k = if a.senha.is_empty() { None } else { Some(Chaves::de(&conn, &a.senha)?) };
    // DT-P4-26: o contexto do protocolo 4 deste caixa (None = ainda não fala o P4).
    let ctx = p4::contexto(&conn, caminho);
    let alvo = endereco_completo(endereco)?;
    let mut total = 0;
    let mut dispositivo = String::new();
    let mut recomecos = 0;
    let mut sondou = false;
    let mut lotes = 0usize;
    let mut confirmacao: Option<rede_local::Confirmacao>;
    // Passo 5.4: o resumo das tabelas deste caixa, mandado no primeiro pedido
    // da troca (de tempos em tempos; veja `hora_do_resumo`).
    let mut resumo_mandado = false;
    loop {
        // Contador, marca e geração guardados deste outro caixa. Na primeira
        // volta o caixa só se conhece pelo endereço: pelo caminho já aprendido
        // (outro caminho do mesmo caixa) ou, antes, pelo par que tinha este endereço.
        #[allow(clippy::type_complexity)]
        let guardado: Option<(String, i64, Option<String>, Option<String>)> = if dispositivo.is_empty() {
            let dono = caminhos::dono_do_endereco(&conn, &alvo.to_string())?;
            match dono {
                Some(d) => conn
                    .query_row(
                        "SELECT dispositivo, cursor, marca, geracao FROM sync_par WHERE dispositivo = ?1",
                        [d],
                        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
                    )
                    .optional()?,
                None => conn
                    .query_row(
                        "SELECT dispositivo, cursor, marca, geracao FROM sync_par WHERE endereco = ?1 ORDER BY visto_em DESC LIMIT 1",
                        [alvo.to_string()],
                        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
                    )
                    .optional()?,
            }
        } else {
            conn.query_row(
                "SELECT dispositivo, cursor, marca, geracao FROM sync_par WHERE dispositivo = ?1",
                [&dispositivo],
                |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
            )
            .optional()?
        };
        let (dono, cursor, marca, geracao_guardada) = guardado.unwrap_or_default();
        // Endereço de caixa ainda desconhecido, numa loja que já tem outros
        // caixas: pode ser outro caminho de um deles (ex.: endereço digitado).
        // Uma pergunta curta, com um contador que não existe em nenhum banco,
        // faz o outro lado responder só quem é (é o jeito que o protocolo já
        // tem de dizer "recomece"); depois se pede de verdade, do contador certo.
        let sonda = dono.is_empty() && !sondou && {
            let existe: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM sync_par)", [], |l| l.get(0))?;
            existe
        };
        let (mut fio, conf) = abrir_fio(&conn, k.as_ref(), &alvo, saida, ctx.as_ref(), &dono, parar)?;
        confirmacao = Some(conf);
        let saiu = agora_ms();
        let (minha_loja, sou_novo) = crate::loja::campos(&conn)?;
        let mut pedido = json!({ "tipo": "puxar", "protocolo": PROTOCOLO, "desde": if sonda { i64::MAX } else { cursor },
                                 "marca": if sonda { Some("sonda".to_string()) } else { marca },
                                 "dispositivo": a.dispositivo, "nome": a.nome, "porta": porta_local,
                                 "esquema": banco::versao_da_estrutura() as i64, "recursos": RECURSOS,
                                 "loja": minha_loja, "novo": sou_novo });
        // Passo 5.4 (PEND-35): a troca anterior com este caixa terminou
        // completa — de tempos em tempos, o primeiro pedido leva o resumo das
        // tabelas daqui. Se ele também estiver em dia, devolve o dele e os dois
        // comparam (nenhuma conexão a mais; erro aqui não atrapalha a troca).
        let meu_resumo = if !resumo_mandado
            && !sonda
            && !dono.is_empty()
            && completo_antes(&conn, &dono)?
            && hora_do_resumo(&conn, caminho, &dono)
        {
            resumo_mandado = true;
            resumo_comparado(caminho, &dono);
            match resumo_agora(&conn, &dono) {
                Ok(r) => Some(r),
                Err(e) => {
                    crate::registro::anotar(&format!("troca de dados: resumo das tabelas: {e}"));
                    None
                }
            }
        } else {
            None
        };
        if let Some(r) = &meu_resumo {
            pedido["resumo"] = r.clone();
        }
        fio.enviar(&pedido)?;
        let resposta = fio.receber(TAMANHO_MAXIMO, Duration::from_secs(30), parar)?;
        // Outro protocolo: recusa antes de ler qualquer alteração.
        if resposta["protocolo"].as_i64() != Some(PROTOCOLO) {
            return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
        }
        let voltou = agora_ms();
        dispositivo = resposta["dispositivo"].as_str().unwrap_or_default().to_string();
        if dispositivo.is_empty() || dispositivo == a.dispositivo {
            return Err(Erro::validacao(caminhos::ESTE_MESMO_CAIXA));
        }
        // DT-P4-28: pelo P4, o registro do outro caixa entra, e o `dispositivo` que ele
        // diz ser tem de ser o da chave do canal (I3), antes de aplicar qualquer coisa.
        if let (Some(par), Some(ctx)) = (fio.chave_do_par(), ctx.as_ref()) {
            p4::juntar_registro(&conn, &ctx.loja, &resposta)?;
            if ctx.chave_de(&conn, &dispositivo)? != Some(par) {
                p4::anotar_recusa_de_identidade(&dispositivo, &par);
                return Err(Erro::validacao(p4::TEXTO_CAIXA_NAO_CONFERE));
            }
        }
        // FALHA-01: é da mesma loja? Antes de conhecê-lo como caixa da loja e
        // de aplicar qualquer alteração dele.
        let decisao = if resposta["outraLoja"].as_bool() == Some(true) {
            crate::loja::Decisao::Recusa
        } else {
            crate::loja::decidir(&conn, &dispositivo, &resposta, true)?
        };
        match decisao {
            crate::loja::Decisao::Recusa => {
                crate::loja::anotar_outra_loja(&conn, resposta["nome"].as_str().unwrap_or(""), &alvo.ip().to_string())?;
                return Err(Erro::validacao(crate::loja::TEXTO_RECUSA_AQUI));
            }
            crate::loja::Decisao::NadaAReceber => {
                // Caixa novo entrando na loja: ele é que recebe daqui.
                registrar_par(&conn, &dispositivo, resposta["nome"].as_str().unwrap_or(""), &alvo.to_string())?;
                let confirmacao = confirmacao.unwrap_or(rede_local::Confirmacao {
                    preso_ao_ip: false,
                    preso_a_interface: None,
                    local: None,
                });
                return Ok(Puxada { dispositivo, total: 0, confirmacao });
            }
            crate::loja::Decisao::Aceita => crate::loja::confirmar(&conn, &dispositivo)?,
        }
        registrar_par(&conn, &dispositivo, resposta["nome"].as_str().unwrap_or(""), &alvo.to_string())?;
        registrar_versao(&conn, &dispositivo, &resposta)?;
        // Caixa retirado da loja: não troca dados até o gerente readmitir.
        if let Some(m) = resposta["recusado"].as_str() {
            return Err(Erro::validacao(m));
        }
        if retirado(&conn, &dispositivo)? {
            return Err(Erro::validacao(TEXTO_RETIRADO_AQUI));
        }
        // Passo 5.2, regra de versão homogênea (veja `RECURSO_CLASSES`): nada
        // de um caixa de versão anterior é aplicado aqui.
        if !mesmas_classes(&resposta) {
            return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
        }
        if sonda {
            // Só queria saber quem é; a resposta (sem alterações) não se aplica.
            sondou = true;
            continue;
        }
        // Neste endereço agora está outro caixa (computador trocado): pede de
        // novo com o contador dele, sem aplicar o que veio.
        if !dono.is_empty() && dono != dispositivo {
            recomecos += 1;
            if recomecos > 3 {
                return Err(Erro::validacao("A troca de dados não conseguiu recomeçar com este caixa."));
            }
            continue;
        }
        let geracao_dele = resposta["geracao"].as_str().unwrap_or_default().to_string();
        let seq_dele = resposta["seqMaxima"].as_i64().unwrap_or(i64::MAX);
        let motivo = if resposta["recomecar"].as_bool().unwrap_or(false) {
            Some("o contador não bate com o banco dele")
        } else if geracao_guardada.as_deref().is_some_and(|g| !g.is_empty() && g != geracao_dele) {
            Some("o banco dele voltou de um backup")
        } else if seq_dele < cursor {
            Some("a numeração dele voltou para trás")
        } else {
            None
        };
        if let Some(motivo) = motivo {
            // Recomeça do zero com ele: recebe tudo de novo; o que já estava
            // aqui é reconhecido pelo id e não é aplicado outra vez.
            crate::registro::anotar(&format!("troca de dados: recomeçando do zero com um caixa ({motivo})"));
            conn.execute(
                "UPDATE sync_par SET cursor = 0, marca = NULL, geracao = ?2, completo_em = NULL WHERE dispositivo = ?1",
                params![dispositivo, geracao_dele],
            )?;
            recomecos += 1;
            if recomecos > 3 {
                return Err(Erro::validacao("A troca de dados não conseguiu recomeçar com este caixa."));
            }
            continue;
        }
        let lista: Vec<Alteracao> = serde_json::from_value(resposta["alteracoes"].clone())?;
        let ultima = lista.iter().max_by_key(|x| x.seq).map(|x| (x.seq, x.id.clone()));
        let novo_cursor = ultima.as_ref().map_or(cursor, |(m, _)| *m);
        // O lote e o novo contador numa transação só (marca d'água: tudo até
        // o contador está gravado ou guardado em `sync_pendencia`).
        let marca_nova = ultima.as_ref().map(|(_, id)| id.clone()).unwrap_or_default();
        let r = aplicar_de(
            &mut conn,
            &lista,
            &dispositivo,
            Some(AvancoCursor {
                dispositivo: &dispositivo,
                de: cursor,
                para: novo_cursor,
                marca: &marca_nova,
                geracao: &geracao_dele,
            }),
        )?;
        total += r.novas;
        let avancou = r.avancou;
        // Passo 5.4: o outro caixa devolveu o resumo dele (estava em dia).
        if let Some(meu) = &meu_resumo {
            if dono == dispositivo {
                if let Err(e) = comparar_resumos(&conn, &dispositivo, meu, &resposta["resumo"]) {
                    crate::registro::anotar(&format!("troca de dados: resumo das tabelas: {e}"));
                }
            }
        }
        // Troca COMPLETA com este caixa (R-05): não há mais nada a receber e o
        // contador chegou ao máximo dele. É a prova de que ele já entregou tudo.
        if avancou && !resposta["temMais"].as_bool().unwrap_or(false) && novo_cursor >= seq_dele {
            conn.execute(
                "UPDATE sync_par SET completo_em = ?2 WHERE dispositivo = ?1",
                params![dispositivo, tempo::agora()],
            )?;
        }
        // Relógio do outro caixa (descontando o tempo da viagem pela rede).
        let aviso = match resposta["agoraMs"].as_i64() {
            Some(dele) => {
                let diferenca = dele - (saiu + voltou) / 2;
                if diferenca.abs() > RELOGIO_TOLERANCIA_MS {
                    let minutos = (diferenca.abs() + 30_000) / 60_000;
                    format!(
                        "O relógio dele está {minutos} min {} em relação a este. Acerte a data e a hora dos dois.",
                        if diferenca > 0 { "adiantado" } else { "atrasado" }
                    )
                } else {
                    String::new()
                }
            }
            None => String::new(),
        };
        conn.execute(
            "UPDATE sync_par SET sincronizado_em = ?2, erro = ?3 WHERE dispositivo = ?1",
            params![dispositivo, tempo::agora(), aviso],
        )?;
        lotes += 1;
        if !resposta["temMais"].as_bool().unwrap_or(false)
            || parar.is_some_and(|p| p.load(Ordering::SeqCst))
            || interrompida(caminho)
            || lotes_maximos(&conn).is_some_and(|n| lotes >= n)
        {
            break;
        }
        // F89-4 (proposta, decisão técnica com veto): dá a vez ao balcão antes
        // de pedir a resposta seguinte. Cada resposta é aplicada numa transação
        // e segura a escrita; com as respostas uma atrás da outra, a escrita
        // fica livre só por milissegundos, e a espera do SQLite (que acorda de
        // 100 em 100 ms) pode não achar a brecha (achado do t2_5).
        thread::sleep(VEZ_DO_BALCAO);
    }
    // Códigos das peças: depois de receber tudo do caixa que distribui as
    // faixas (assim este caixa já conhece todos os códigos que ele conhece),
    // pede a faixa se não tem ou se está acabando (veja `faixas`).
    if !parar.is_some_and(|p| p.load(Ordering::SeqCst))
        && !interrompida(caminho)
        && crate::faixas::distribuidor(&conn)?.as_deref() == Some(dispositivo.as_str())
        && crate::faixas::precisa(&conn)?
    {
        match pedir_faixa(&mut conn, k.as_ref(), &alvo, saida, ctx.as_ref(), &dispositivo) {
            Ok(n) => total += n,
            Err(e) => crate::registro::anotar(&format!("troca de dados: pedido de faixa de códigos: {e}")),
        }
    }
    // Depois de tudo: algum registro ficou apontando para outro que não veio?
    registrar_referencias(&conn)?;
    if total > 0 {
        crate::usuarios::resolver_nomes_repetidos(&conn)?;
        verificar_depois(&conn)?;
    }
    // Peças deste caixa com código a definir recebem código da faixa.
    if let Err(e) = crate::faixas::manter(&mut conn) {
        crate::registro::anotar(&format!("códigos das peças: {e}"));
    }
    let confirmacao =
        confirmacao.unwrap_or(rede_local::Confirmacao { preso_ao_ip: false, preso_a_interface: None, local: None });
    Ok(Puxada { dispositivo, total, confirmacao })
}

/// A última troca com este outro caixa terminou completa (R-05: recebeu tudo
/// o que ele tinha)?
pub(super) fn completo_antes(conn: &Connection, dispositivo: &str) -> Resultado<bool> {
    Ok(conn
        .query_row("SELECT completo_em IS NOT NULL FROM sync_par WHERE dispositivo = ?1", [dispositivo], |l| l.get(0))
        .optional()?
        .unwrap_or(false))
}

/// Pede ao distribuidor uma faixa de códigos das peças. A faixa chega como
/// uma alteração do log (com o mesmo id que o distribuidor gravou) e fica
/// gravada aqui antes de qualquer código dela ser dado. Devolve quantas
/// alterações eram novas.
pub(super) fn pedir_faixa(
    conn: &mut Connection,
    k: Option<&Chaves>,
    alvo: &SocketAddr,
    saida: Option<&Saida>,
    ctx: Option<&p4::ContextoP4>,
    dono: &str,
) -> Resultado<usize> {
    let mut pedido = crate::faixas::pedido(conn)?;
    pedido["tipo"] = json!("faixa");
    pedido["protocolo"] = json!(PROTOCOLO);
    let (loja, novo) = crate::loja::campos(conn)?;
    pedido["loja"] = json!(loja);
    pedido["novo"] = json!(novo);
    let (mut fio, _) = abrir_fio(conn, k, alvo, saida, ctx, dono, None)?;
    fio.enviar(&pedido)?;
    let resposta = fio.receber(TAMANHO_PEDIDO, Duration::from_secs(30), None)?;
    gravar_faixa_recebida(conn, &resposta)
}

/// O pacote de anúncio: continua `{dispositivo, nome, porta}` (o protocolo não
/// muda); quem lê aceita campos a mais.
pub(super) fn anuncio(dispositivo: &str, nome: &str, porta: u16) -> Value {
    json!({ "dispositivo": dispositivo, "nome": nome, "porta": porta })
}

/// (dispositivo, nome, porta) de um anúncio; `None` se não diz quem é.
pub(super) fn ler_anuncio(v: &Value) -> Option<(String, String, u16)> {
    let d = v["dispositivo"].as_str().filter(|d| !d.is_empty())?;
    let porta = v["porta"].as_u64().and_then(|p| u16::try_from(p).ok()).unwrap_or(crate::portatil::portas().0);
    Some((d.to_string(), v["nome"].as_str().unwrap_or("").to_string(), porta))
}

/// Um anúncio recebido vira caminho? Não o do próprio caixa (por outra de
/// suas interfaces) e não o que vem de VPN/virtual/celular sem liberação.
pub(super) fn aceitar_anuncio(
    dispositivo: &str,
    a: &Ajustes,
    interfaces: Option<&[Interface]>,
    endereco: &str,
) -> bool {
    dispositivo != a.dispositivo && rede_local::caminho_aprendido_vale(interfaces, endereco, &a.interfaces)
}

/// O motivo, para a tela, de uma porta da troca de dados que não abre.
pub(super) fn motivo_da_porta(protocolo: &str, porta: u16, erro: &str) -> String {
    format!("não foi possível abrir a porta {protocolo} {porta} ({erro}); outro programa pode estar usando essa porta")
}

/// O que vai para o registro quando uma porta da troca de dados não abre.
#[cfg(test)]
pub(super) fn texto_da_porta(protocolo: &str, porta: u16, erro: &str) -> String {
    format!("troca de dados: {}", motivo_da_porta(protocolo, porta, erro))
}

/// RD-B (regra 5): identificação de qual instância do Rustinho neste computador
/// está ocupando uma porta da troca de dados.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcupanteDaPorta {
    pub protocolo: String,
    pub porta: u16,
    pub pid: u32,
    pub banco: String,
    pub pasta: String,
    pub loja: String,
    pub caixa: String,
}

fn mapa_ocupantes() -> &'static Mutex<HashMap<(String, u16), OcupanteDaPorta>> {
    static M: std::sync::OnceLock<Mutex<HashMap<(String, u16), OcupanteDaPorta>>> = std::sync::OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pasta_ocupantes() -> PathBuf {
    let p = std::env::temp_dir().join("rustinho-portas");
    let _ = std::fs::create_dir_all(&p);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700));
    }
    p
}

fn arquivo_ocupante(protocolo: &str, porta: u16) -> PathBuf {
    pasta_ocupantes().join(format!("{}-{porta}.json", protocolo.to_ascii_lowercase()))
}

fn ler_loja_e_caixa(caminho_banco: &Path) -> (String, String) {
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        caminho_banco,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return (String::new(), String::new());
    };
    let _ = conn.busy_timeout(Duration::from_millis(150));
    let loja = config::nome_loja(&conn).unwrap_or_default();
    let caixa = config::nome_caixa(&conn).unwrap_or_default();
    (loja, caixa)
}

fn pid_vivo(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        // SAFETY: kill com sinal 0 só verifica se o processo existe no sistema.
        let r = unsafe { libc::kill(pid as i32, 0) };
        r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub(super) fn registrar_ocupante_porta(protocolo: &str, porta: u16, caminho_banco: &Path) {
    if porta == 0 || caminho_banco.as_os_str().is_empty() {
        return;
    }
    let (loja, caixa) = ler_loja_e_caixa(caminho_banco);
    let banco = caminho_banco.display().to_string();
    let pasta = caminho_banco.parent().unwrap_or(caminho_banco).display().to_string();
    let info = OcupanteDaPorta {
        protocolo: protocolo.to_ascii_uppercase(),
        porta,
        pid: std::process::id(),
        banco,
        pasta,
        loja,
        caixa,
    };
    if let Ok(bytes) = serde_json::to_vec(&info) {
        let arq = arquivo_ocupante(protocolo, porta);
        let tmp = arq.with_extension(format!("tmp-{}", std::process::id()));
        if std::fs::write(&tmp, &bytes).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            }
            let _ = std::fs::rename(&tmp, &arq);
        }
    }
    let mut m = mapa_ocupantes().lock().unwrap_or_else(|e| e.into_inner());
    m.insert((protocolo.to_ascii_uppercase(), porta), info);
}

pub(super) fn remover_ocupante_porta(protocolo: &str, porta: u16, caminho_banco: &Path) {
    if porta == 0 {
        return;
    }
    let chave = (protocolo.to_ascii_uppercase(), porta);
    let meu = caminho_banco.display().to_string();
    {
        let mut m = mapa_ocupantes().lock().unwrap_or_else(|e| e.into_inner());
        if m.get(&chave).is_some_and(|o| o.banco == meu) {
            m.remove(&chave);
        }
    }
    let arq = arquivo_ocupante(protocolo, porta);
    if let Ok(txt) = std::fs::read_to_string(&arq) {
        if let Ok(o) = serde_json::from_str::<OcupanteDaPorta>(&txt) {
            if o.banco == meu || !pid_vivo(o.pid) {
                let _ = std::fs::remove_file(&arq);
            }
        }
    }
}

/// Descobre se outra instância do Rustinho neste computador registrou que ocupa
/// a `porta`, atualizando loja e caixa diretamente do banco dela quando acessível.
pub fn ocupante_da_porta(protocolo: &str, porta: u16, meu_banco: &Path) -> Option<OcupanteDaPorta> {
    if porta == 0 {
        return None;
    }
    let meu = meu_banco.display().to_string();
    let chave = (protocolo.to_ascii_uppercase(), porta);
    let cand = {
        let m = mapa_ocupantes().lock().unwrap_or_else(|e| e.into_inner());
        m.get(&chave).filter(|o| o.banco != meu && pid_vivo(o.pid)).cloned()
    }
    .or_else(|| {
        let arq = arquivo_ocupante(protocolo, porta);
        let txt = std::fs::read_to_string(&arq).ok()?;
        let o: OcupanteDaPorta = serde_json::from_str(&txt).ok()?;
        (o.banco != meu && pid_vivo(o.pid)).then_some(o)
    })?;
    let mut atualizado = cand;
    let p_banco = Path::new(&atualizado.banco);
    if p_banco.is_file() {
        let (loja, caixa) = ler_loja_e_caixa(p_banco);
        if !loja.is_empty() {
            atualizado.loja = loja;
        }
        if !caixa.is_empty() {
            atualizado.caixa = caixa;
        }
    }
    Some(atualizado)
}

/// Resumo legível do outro Rustinho que ocupa a porta ("loja \"X\", Caixa 1").
pub fn resumo_ocupante(o: &OcupanteDaPorta) -> String {
    let loja = if o.loja.trim().is_empty() {
        "loja ainda não configurada".to_string()
    } else {
        format!("loja \"{}\"", o.loja.trim())
    };
    let caixa = if o.caixa.trim().is_empty() { "Caixa 1" } else { o.caixa.trim() };
    format!("{loja}, {caixa}")
}

/// Ocupante em conflito com este banco (quando há falha ativa de porta na supervisão).
pub fn ocupante_em_conflito(meu_banco: &Path, porta_tcp: u16, porta_udp: u16) -> Option<OcupanteDaPorta> {
    let s = crate::supervisao::situacao(meu_banco);
    if !s.com_falha() {
        return None;
    }
    ocupante_da_porta("TCP", porta_tcp, meu_banco).or_else(|| ocupante_da_porta("UDP", porta_udp, meu_banco))
}

pub(super) fn motivo_da_porta_com_ocupante(protocolo: &str, porta: u16, erro: &str, meu_banco: &Path) -> String {
    if let Some(o) = ocupante_da_porta(protocolo, porta, meu_banco) {
        let quem = resumo_ocupante(&o);
        format!(
            "não foi possível abrir a porta {protocolo} {porta} ({erro}): já está em uso por outro Rustinho aberto \
             neste computador ({quem}, em {})",
            o.pasta
        )
    } else {
        motivo_da_porta(protocolo, porta, erro)
    }
}

/// Guarda o problema do motor uma vez só (as tentativas repetem o mesmo).
pub(super) fn guardar_problema(problemas: &Mutex<Vec<String>>, texto: String) {
    let mut p = problemas.lock().unwrap_or_else(|e| e.into_inner());
    if !p.contains(&texto) {
        p.push(texto);
    }
}
