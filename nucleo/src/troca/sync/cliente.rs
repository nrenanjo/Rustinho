//! Puxar, o anúncio e os problemas da porta.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

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
    let k = Chaves::de(&conn, &a.senha)?;
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
        let (mut fluxo, conf) = rede_local::conectar(alvo, saida, Duration::from_secs(3)).map_err(rede)?;
        confirmacao = Some(conf);
        fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok();
        fluxo.set_write_timeout(Some(Duration::from_secs(10))).ok();
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
        enviar(&mut fluxo, &k, &pedido)?;
        let resposta = receber(&mut fluxo, &k, TAMANHO_MAXIMO, Duration::from_secs(30), parar)?;
        // Outro protocolo: recusa antes de ler qualquer alteração.
        if resposta["protocolo"].as_i64() != Some(PROTOCOLO) {
            return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
        }
        let voltou = agora_ms();
        dispositivo = resposta["dispositivo"].as_str().unwrap_or_default().to_string();
        if dispositivo.is_empty() || dispositivo == a.dispositivo {
            return Err(Erro::validacao(caminhos::ESTE_MESMO_CAIXA));
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
            || lotes_maximos(&conn).is_some_and(|n| lotes >= n)
        {
            break;
        }
    }
    // Códigos das peças: depois de receber tudo do caixa que distribui as
    // faixas (assim este caixa já conhece todos os códigos que ele conhece),
    // pede a faixa se não tem ou se está acabando (veja `faixas`).
    if !parar.is_some_and(|p| p.load(Ordering::SeqCst))
        && crate::faixas::distribuidor(&conn)?.as_deref() == Some(dispositivo.as_str())
        && crate::faixas::precisa(&conn)?
    {
        match pedir_faixa(&mut conn, &k, &alvo, saida) {
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
    k: &Chaves,
    alvo: &SocketAddr,
    saida: Option<&Saida>,
) -> Resultado<usize> {
    let mut pedido = crate::faixas::pedido(conn)?;
    pedido["tipo"] = json!("faixa");
    pedido["protocolo"] = json!(PROTOCOLO);
    let (loja, novo) = crate::loja::campos(conn)?;
    pedido["loja"] = json!(loja);
    pedido["novo"] = json!(novo);
    let (mut fluxo, _) = rede_local::conectar(*alvo, saida, Duration::from_secs(3)).map_err(rede)?;
    fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(10))).ok();
    enviar(&mut fluxo, k, &pedido)?;
    let resposta = receber(&mut fluxo, k, TAMANHO_PEDIDO, Duration::from_secs(30), None)?;
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
pub(super) fn texto_da_porta(protocolo: &str, porta: u16, erro: &str) -> String {
    format!("troca de dados: {}", motivo_da_porta(protocolo, porta, erro))
}

/// Guarda o problema do motor uma vez só (as tentativas repetem o mesmo).
pub(super) fn guardar_problema(problemas: &Mutex<Vec<String>>, texto: String) {
    let mut p = problemas.lock().unwrap_or_else(|e| e.into_inner());
    if !p.contains(&texto) {
        p.push(texto);
    }
}
