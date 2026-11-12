//! Atender, a versão do par, retirado e `preparar_para`.
//!
//! Parte do `troca::sync` (passo 5.1: o texto veio do sync.rs sem mudança,
//! fora a visibilidade `pub(super)`, que tem o mesmo alcance de antes).

use super::*;

/// A3: este caixa está em modo seguro (banco com defeito): não troca dados.
pub(super) const TEXTO_MODO_SEGURO: &str =
    "O banco de dados deste caixa está com defeito (modo seguro): a troca de dados \
     está parada até restaurar um backup bom.";

/// Atende um pedido de outro caixa.
pub(super) fn atender(caminho: &Path, mut fluxo: TcpStream, parar: &AtomicBool) -> Resultado<()> {
    // Quem não manda o pedido em `PRAZO_DO_PEDIDO` é desligado.
    fluxo.set_read_timeout(Some(PRAZO_DO_PEDIDO)).ok();
    fluxo.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    // A3: banco com defeito (modo seguro): não entrega nada a ninguém.
    if banco::defeito(caminho).is_some() {
        return Ok(());
    }
    let mut conn = abrir_conexao(caminho)?;
    let a = ajustes(&conn)?;
    if !a.ligado || a.senha.is_empty() {
        return Ok(());
    }
    // Banco copiado de outro computador: tem o identificador de troca do
    // caixa de onde veio e não pode trocar dados com ele até ser registrado
    // como caixa novo (veja `faixas::registrar_caixa_novo`).
    if crate::faixas::copia_detectada(&conn)? {
        return Ok(());
    }
    let k = Chaves::de(&conn, &a.senha)?;
    let pedido = receber(&mut fluxo, &k, TAMANHO_PEDIDO, PRAZO_DO_PEDIDO, Some(parar))?;
    if pedido["tipo"] != "puxar" && pedido["tipo"] != "faixa" {
        return Ok(());
    }
    if pedido["protocolo"].as_i64() != Some(PROTOCOLO) {
        return Err(Erro::validacao(PROTOCOLO_DIFERENTE));
    }
    // FALHA-01: é da mesma loja? Antes de entregar qualquer coisa (e antes de
    // conhecê-lo como caixa da loja).
    let quem_pede = pedido["dispositivo"].as_str().unwrap_or_default().to_string();
    if !quem_pede.is_empty() && quem_pede != a.dispositivo {
        match crate::loja::decidir(&conn, &quem_pede, &pedido, false)? {
            crate::loja::Decisao::Recusa => {
                let origem = fluxo.peer_addr().map(|e| e.ip().to_string()).unwrap_or_default();
                crate::loja::anotar_outra_loja(&conn, pedido["nome"].as_str().unwrap_or(""), &origem)?;
                let (loja, novo) = crate::loja::campos(&conn)?;
                return enviar(
                    &mut fluxo,
                    &k,
                    &json!({ "protocolo": PROTOCOLO, "dispositivo": a.dispositivo, "nome": a.nome,
                             "esquema": banco::versao_da_estrutura() as i64, "recursos": RECURSOS,
                             "loja": loja, "novo": novo, "outraLoja": true,
                             "recusado": crate::loja::TEXTO_RECUSA_LA }),
                );
            }
            crate::loja::Decisao::Aceita => crate::loja::confirmar(&conn, &quem_pede)?,
            // Este caixa ainda não tem usuários (está entrando numa loja):
            // não confirma ninguém e não entrega nada (logo abaixo).
            crate::loja::Decisao::NadaAReceber => {}
        }
    }
    // Pedido de faixa de códigos das peças: só o distribuidor atende, numa
    // gravação só (veja `faixas::atender_pedido`).
    if pedido["tipo"] == "faixa" {
        let resposta = responder_faixa(&mut conn, &pedido)?;
        return enviar(&mut fluxo, &k, &resposta);
    }
    // Quem pediu também vira um par conhecido (com o endereço de onde veio).
    if let (Some(d), Ok(origem)) = (pedido["dispositivo"].as_str(), fluxo.peer_addr()) {
        let porta = pedido["porta"].as_u64().unwrap_or(crate::portatil::portas().0 as u64);
        if d != a.dispositivo {
            registrar_par(&conn, d, pedido["nome"].as_str().unwrap_or(""), &format!("{}:{porta}", origem.ip()))?;
            registrar_versao(&conn, d, &pedido)?;
        }
    }
    let quem = pedido["dispositivo"].as_str().unwrap_or_default().to_string();
    // Caixa retirado da loja: não recebe nada até o gerente readmitir.
    if !quem.is_empty() && retirado(&conn, &quem)? {
        return enviar(
            &mut fluxo,
            &k,
            &json!({ "protocolo": PROTOCOLO, "dispositivo": a.dispositivo, "nome": a.nome,
                     "esquema": banco::versao_da_estrutura() as i64, "recursos": RECURSOS,
                     "recusado": TEXTO_RETIRADO_LA }),
        );
    }
    let desde = pedido["desde"].as_i64().unwrap_or(0).max(0);
    let geracao = geracao(&conn)?;
    let (minha_loja, sou_novo) = crate::loja::campos(&conn)?;
    let seq_maxima = seq_maxima(&conn)?;
    // O contador de quem pediu ainda vale neste banco? A alteração número
    // `desde` aqui tem de ser a mesma que ele recebeu (a "marca"). Se não é
    // (este banco voltou de um backup, é outro computador, foi copiado à
    // mão), ele recomeça do zero.
    if desde > 0 {
        let aqui: Option<String> =
            conn.query_row("SELECT id FROM alteracao WHERE seq = ?1", [desde], |l| l.get(0)).optional()?;
        if aqui.is_none() || aqui.as_deref() != pedido["marca"].as_str() {
            return enviar(
                &mut fluxo,
                &k,
                &json!({ "protocolo": PROTOCOLO, "dispositivo": a.dispositivo, "nome": a.nome, "alteracoes": [],
                         "temMais": false, "agoraMs": agora_ms(), "geracao": geracao, "seqMaxima": seq_maxima,
                         "recomecar": true, "esquema": banco::versao_da_estrutura() as i64, "recursos": RECURSOS,
                         "loja": minha_loja, "novo": sou_novo }),
            );
        }
    }
    // Caixa sem usuários (entrando numa loja): não tem dado próprio e não é a
    // fonte de loja nenhuma; o que recebeu de outro caixa não é repassado.
    let (lista, tem_mais) = if sou_novo { (Vec::new(), false) } else { alteracoes_desde(&conn, desde, lote(&conn))? };
    let (lista, tem_mais) = preparar_para(&conn, &quem, &pedido, lista, tem_mais)?;
    enviar(
        &mut fluxo,
        &k,
        &json!({ "protocolo": PROTOCOLO, "dispositivo": a.dispositivo, "nome": a.nome, "alteracoes": lista,
                 "temMais": tem_mais, "agoraMs": agora_ms(), "geracao": geracao, "seqMaxima": seq_maxima,
                 "esquema": banco::versao_da_estrutura() as i64, "recursos": RECURSOS,
                 "loja": minha_loja, "novo": sou_novo }),
    )
}

pub(super) const TEXTO_RETIRADO_AQUI: &str =
    "Este caixa foi retirado da loja. Para ele voltar a trocar dados, o gerente precisa readmiti-lo em \
     Configurações › Caixas da loja.";
pub(super) const TEXTO_RETIRADO_LA: &str =
    "Este caixa foi retirado da loja pelo gerente. Para voltar a trocar dados, o gerente \
     precisa readmiti-lo em Configurações › Caixas da loja, num caixa da loja.";

/// Guarda a versão que o outro caixa informou (estrutura do banco e
/// recursos). Caixa que não informa é de antes da 0.9.15: fica sem
/// compatibilidade confirmada (um caixa que voltou a uma versão antiga perde
/// a confirmação na hora).
pub(crate) fn registrar_versao(conn: &Connection, dispositivo: &str, mensagem: &Value) -> Resultado<()> {
    let esquema = mensagem["esquema"].as_i64();
    let recursos: Vec<String> = mensagem["recursos"]
        .as_array()
        .map(|l| l.iter().filter_map(|v| v.as_str().map(str::to_string)).take(32).collect())
        .unwrap_or_default();
    let compativel = esquema.is_some_and(|e| e >= ESQUEMA_RASCUNHO);
    conn.execute(
        "UPDATE sync_par SET esquema = ?2, recursos = ?3,
                compativel_em = CASE WHEN ?4 THEN ?5 ELSE NULL END
          WHERE dispositivo = ?1",
        params![dispositivo, esquema, recursos.join(","), compativel, tempo::agora()],
    )?;
    if compativel {
        // O que não ia para ele por ser de versão antiga agora vai (a
        // numeração dele ficou antes dessas alterações).
        conn.execute(
            "UPDATE sync_pendencia SET estado = 'resolvida', resolvido_em = ?2, atualizado_em = ?2
              WHERE lado = 'nao_enviada' AND par = ?1 AND estado = 'pendente'",
            params![dispositivo, tempo::agora()],
        )?;
    }
    Ok(())
}

/// O caixa foi retirado da loja (configuração da loja, vale em todos)?
pub fn retirado(conn: &Connection, dispositivo: &str) -> Resultado<bool> {
    Ok(config::ler(conn, &format!("{PREFIXO_RETIRADO}{dispositivo}"))?.is_some_and(|v| !v.trim().is_empty()))
}

/// Prepara o lote para o caixa `quem`:
/// - caixa que entende pendências (0.9.15+): o que a versão dele não
///   consegue gravar vai marcado ("incompativel") e ele guarda sem aplicar;
/// - caixa antigo (não informa recursos): o lote **para** na primeira
///   alteração que ele não consegue gravar com segurança (incompatível ou
///   com dados estragados). Ele não pode pular nada: a numeração dele
///   avançaria e a alteração se perderia para ele. Fica anotado aqui (lado
///   "nao_enviada") e o gerente vê o motivo.
pub(crate) fn preparar_para(
    conn: &Connection,
    quem: &str,
    pedido: &Value,
    mut lista: Vec<Alteracao>,
    tem_mais: bool,
) -> Resultado<(Vec<Alteracao>, bool)> {
    let recursos: Vec<&str> =
        pedido["recursos"].as_array().map(|l| l.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    let esquema = pedido["esquema"].as_i64().unwrap_or(ESQUEMA_LEGADO);
    if recursos.contains(&RECURSO_PENDENCIAS) {
        for a in &mut lista {
            a.incompativel = incompatibilidade(esquema, a);
        }
        return Ok((lista, tem_mais));
    }
    let mut cache: Vec<(String, Vec<String>)> = Vec::new();
    let mut corte = None;
    for (i, a) in lista.iter().enumerate() {
        let motivo = if let Some(m) = incompatibilidade(esquema, a) {
            Some(m)
        } else if a.tabela == "config" && !config_da_loja(&a.chave) {
            None
        } else {
            if !cache.iter().any(|c| c.0 == a.tabela) {
                let (_, chave) = colunas(conn, &a.tabela)?;
                cache.push((a.tabela.clone(), chave));
            }
            let chave = &cache.iter().find(|c| c.0 == a.tabela).expect("acabou de entrar").1;
            invalida(a, chave)
        };
        if let Some(m) = motivo {
            corte = Some((i, m));
            break;
        }
    }
    let Some((i, motivo)) = corte else {
        if let Some(ultima) = lista.last() {
            conn.execute(
                "UPDATE sync_pendencia SET estado = 'resolvida', resolvido_em = ?3, atualizado_em = ?3
                  WHERE lado = 'nao_enviada' AND par = ?1 AND seq <= ?2 AND estado = 'pendente'",
                params![quem, ultima.seq, tempo::agora()],
            )?;
        }
        return Ok((lista, tem_mais));
    };
    let a = &lista[i];
    if !quem.is_empty() {
        let agora = tempo::agora();
        conn.execute(
            "INSERT INTO sync_pendencia (lado, par, alteracao_id, origem, seq, tabela, chave, operacao, dispositivo, quando,
                                         motivo, detalhe, criado_em, atualizado_em)
             VALUES ('nao_enviada', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?3, ?8, 'incompativel_esquema', ?9, ?10, ?10)
             ON CONFLICT (lado, par, alteracao_id) DO UPDATE SET atualizado_em = excluded.atualizado_em,
                 tentativas = sync_pendencia.tentativas + 1
               WHERE sync_pendencia.estado = 'pendente'",
            params![quem, a.id, a.dispositivo, a.seq, a.tabela, a.chave, a.operacao, a.quando, motivo, agora],
        )?;
    }
    lista.truncate(i);
    // `temMais` falso: ele não pede de novo na hora (pediria o mesmo).
    Ok((lista, false))
}
