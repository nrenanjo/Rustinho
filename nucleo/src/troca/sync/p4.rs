//! Protocolo 4 no motor da troca (0.9.19-dev.2, T008; `roadmap.md` §5.6).
//!
//! - O "4" é o **fio** (DT-P4-25): a conexão TLS 1.3 com as chaves cruas de cada
//!   caixa (`protocolo4::canal`). As mensagens da troca dentro dele seguem o
//!   formato de hoje, mais o recurso [`RECURSO_CHAVES`] e o campo `registro` (as
//!   linhas assinadas de caixas aprovados e retirados, que só viajam por aqui).
//! - O contexto ([`contexto`], DT-P4-26) é montado por conexão a partir do banco
//!   (loja, raiz fixada, registro) e da identidade **deste processo** (aberta uma
//!   vez, `cofre::identidade_do_processo`). Sem ele, o caixa ainda não fala o P4.
//! - A raiz da loja ([`garantir_raiz`], DT-P4-10) é conferida na puxada, de minuto
//!   em minuto, nunca a cada conexão (grava `config`).

use super::*;
use crate::troca::protocolo4::{aprovados, canal, cofre, hex, Chave};

/// Recurso da troca: este caixa fala o protocolo 4 (o fio autenticado).
pub const RECURSO_CHAVES: &str = "chaves-v1";

/// Só nos testes: quantas conexões o servidor atendeu pelo fio do protocolo 4
/// (prova de que a troca passou mesmo pelo canal autenticado).
#[cfg(test)]
pub(super) static ATENDIDAS_PELO_P4: AtomicUsize = AtomicUsize::new(0);

/// Prazo do aperto de mão TLS de uma conexão da troca.
pub(super) const PRAZO_DO_APERTO: Duration = Duration::from_secs(5);

/// O modo deste processo (o Rustinho portátil ou a instalação).
pub(super) fn modo() -> cofre::Modo {
    if crate::portatil::ligado() {
        cofre::Modo::portatil_deste_computador()
    } else {
        cofre::Modo::Instalado
    }
}

/// O que este caixa precisa para falar o protocolo 4 numa conexão.
pub(super) struct ContextoP4 {
    pub(super) identidade: Arc<cofre::IdentidadeLocal>,
    pub(super) raiz: Chave,
    pub(super) loja: String,
}

impl ContextoP4 {
    /// A chave pública deste caixa.
    pub(super) fn eu(&self) -> Chave {
        self.identidade.identidade.publica()
    }

    /// As chaves autorizadas agora nesta loja, sem a deste caixa (com quem ele fala).
    pub(super) fn outras_autorizadas(&self, conn: &Connection) -> Resultado<std::collections::HashSet<Chave>> {
        let av = aprovados::avaliar(conn, &self.raiz, &self.loja)?;
        let mut outras = av.autorizadas().clone();
        outras.remove(&self.eu());
        Ok(outras)
    }

    /// A chave autorizada do caixa `dispositivo` (I3), se houver.
    pub(super) fn chave_de(&self, conn: &Connection, dispositivo: &str) -> Resultado<Option<Chave>> {
        aprovados::chave_do_dispositivo(conn, &self.raiz, &self.loja, dispositivo)
    }
}

/// O contexto P4 deste caixa, ou `None` quando ele ainda não fala o P4: sem loja,
/// sem raiz fixada, identidade indisponível agora (o motivo vai para o registro
/// técnico; nunca gera identidade nova sozinho) ou a própria chave não autorizada.
/// A raiz é conferida antes da identidade: um caixa sem raiz não paga o Argon2.
pub(super) fn contexto(conn: &Connection, caminho: &Path) -> Option<ContextoP4> {
    // O caixa que acabou de entrar por convite ainda não recebeu a identidade da
    // loja pela troca: vale a loja escolhida na entrada (T009).
    let loja = crate::loja::id(conn)
        .ok()
        .filter(|l| !l.is_empty())
        .or_else(|| crate::loja::da_entrada(conn).ok().filter(|l| !l.is_empty()))?;
    let raiz = cofre::raiz_atual(conn, &loja).ok().flatten()?;
    let pasta = caminho.parent()?;
    let identidade = match cofre::identidade_do_processo(pasta, &modo()) {
        Ok(i) => i,
        Err(e) => {
            crate::registro::anotar(&format!("troca de dados (protocolo 4): identidade indisponível agora: {e}"));
            return None;
        }
    };
    let ctx = ContextoP4 { identidade, raiz, loja };
    aprovados::autorizada(conn, &ctx.raiz, &ctx.loja, &ctx.eu()).ok().filter(|ok| *ok)?;
    Some(ctx)
}

/// DT-P4-10 (A4): o caixa que distribui as faixas de uma loja com usuários vira a
/// raiz do registro (uma vez; o pino `p4_raiz` nunca é trocado), com o gerente
/// ativo mais antigo no campo informativo. Grava a chave pública deste caixa em
/// `config` (local). Sem loja, sem usuários ou com a raiz já fixada e a chave já
/// anotada, não faz nada (nem abre a identidade).
pub(super) fn garantir_raiz(conn: &Connection, caminho: &Path) -> Resultado<()> {
    let loja = crate::loja::id(conn)?;
    if loja.is_empty() || crate::usuarios::quantidade(conn)? == 0 {
        return Ok(());
    }
    let anotada = config::ler(conn, "p4_chave_publica")?.is_some_and(|c| !c.trim().is_empty());
    if anotada && cofre::raiz_atual(conn, &loja)?.is_some() {
        return Ok(());
    }
    let Some(pasta) = caminho.parent() else { return Ok(()) };
    let id = cofre::identidade_do_processo(pasta, &modo())?;
    let gerente = crate::usuarios::listar(conn, true)?
        .into_iter()
        .filter(|u| u.gerente)
        .min_by(|a, b| a.criado_em.cmp(&b.criado_em))
        .map(|u| u.nome)
        .unwrap_or_default();
    let distribuidor = crate::faixas::sou_distribuidor(conn)?;
    cofre::garantir_raiz_se_aplicavel(
        conn,
        &id.identidade,
        id.cofre_sistema,
        &loja,
        &config::nome_caixa(conn)?,
        &gerente,
        distribuidor,
    )?;
    Ok(())
}

/// O registro de caixas desta loja, em JSON, para ir junto numa mensagem P4.
pub(super) fn registro_em_json(conn: &Connection, loja: &str) -> Resultado<Value> {
    Ok(serde_json::to_value(aprovados::linhas(conn, loja)?)?)
}

/// Junta o registro que veio numa mensagem P4 (acrescenta, nunca tira; cada linha
/// se confere sozinha). Mensagem sem registro não muda nada; registro estragado é
/// recusado inteiro.
pub(super) fn juntar_registro(conn: &Connection, loja: &str, mensagem: &Value) -> Resultado<()> {
    let Some(r) = mensagem.get("registro") else { return Ok(()) };
    let linhas: aprovados::Linhas = serde_json::from_value(r.clone())
        .map_err(|_| Erro::validacao("O registro de caixas que veio do outro caixa está estragado."))?;
    aprovados::juntar(conn, loja, &linhas)?;
    Ok(())
}

/// O fio do protocolo 4: o canal TLS autenticado. Toda mensagem que sai leva o
/// registro deste caixa e o recurso [`RECURSO_CHAVES`].
pub(super) struct FioP4<C> {
    canal: canal::CanalAutenticado<C>,
    registro: Value,
}

impl<C> FioP4<C> {
    pub(super) fn novo(canal: canal::CanalAutenticado<C>, registro: Value) -> FioP4<C> {
        FioP4 { canal, registro }
    }

    /// Troca o registro que vai nas próximas mensagens (depois de juntar o do outro).
    pub(super) fn trocar_registro(&mut self, registro: Value) {
        self.registro = registro;
    }
}

impl<C, Data> Fio for FioP4<C>
where
    Data: rustls::SideData,
    C: std::ops::DerefMut<Target = rustls::ConnectionCommon<Data>>,
{
    fn chave_do_par(&self) -> Option<Chave> {
        Some(self.canal.chave_do_par())
    }

    fn enviar(&mut self, v: &Value) -> Resultado<()> {
        let mut v = v.clone();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("registro".into(), self.registro.clone());
            if let Some(Value::Array(rec)) = obj.get_mut("recursos") {
                if !rec.iter().any(|r| r.as_str() == Some(RECURSO_CHAVES)) {
                    rec.push(Value::String(RECURSO_CHAVES.into()));
                }
            }
        }
        self.canal.enviar_json(&v, Duration::from_secs(30), None)
    }

    fn receber(&mut self, limite: usize, prazo: Duration, parar: Option<&AtomicBool>) -> Resultado<Value> {
        self.canal.receber_json(limite, prazo, parar)
    }

    fn origem(&self) -> Option<SocketAddr> {
        self.canal.peer_addr().ok()
    }
}

/// DT-P4-17: o anúncio assinado (`RSA4`) deste caixa: chave, loja, porta, momento
/// e nonce, assinados; nada tirado da senha da rede.
pub(super) fn pacote_de_anuncio(ctx: &ContextoP4, porta_tcp: u16) -> Resultado<Vec<u8>> {
    crate::troca::protocolo4::anuncio::montar(&ctx.identidade.identidade, &ctx.loja, porta_tcp, agora_ms())
}

/// DT-P4-17: um anúncio `RSA4` recebido vira par só se for desta loja, de uma chave
/// autorizada agora (não a deste caixa), dentro da janela de tempo e sem repetição.
/// O `dispositivo` e o nome vêm da aprovação assinada da chave (o anúncio não os
/// traz), e o `dispositivo` tem de apontar de volta para a mesma chave (I3).
/// Devolve `(dispositivo, nome, porta)`.
pub(super) fn ler_anuncio(
    conn: &Connection,
    ctx: &ContextoP4,
    bytes: &[u8],
    repeticoes: &mut crate::troca::protocolo4::anuncio::Repeticoes,
) -> Option<(String, String, u16)> {
    let outras = ctx.outras_autorizadas(conn).ok()?;
    let a = crate::troca::protocolo4::anuncio::verificar_anuncio(bytes, &ctx.loja, agora_ms(), repeticoes, |k| {
        outras.contains(k)
    })
    .ok()?;
    let linhas = aprovados::linhas(conn, &ctx.loja).ok()?;
    let aprovacao = linhas
        .aprovacoes
        .iter()
        .filter(|x| x.chave == a.chave && !x.dispositivo.is_empty() && x.assinatura_confere())
        .max_by(|x, y| x.aprovado_em.cmp(&y.aprovado_em))?;
    if ctx.chave_de(conn, &aprovacao.dispositivo).ok()? != Some(a.chave) {
        return None;
    }
    Some((aprovacao.dispositivo.clone(), aprovacao.nome.clone(), a.porta))
}

/// A raiz do protocolo 4 está fixada neste caixa (pino `p4_raiz`)?
pub(super) fn tem_raiz_fixada(conn: &Connection) -> bool {
    config::ler(conn, "p4_raiz").ok().flatten().is_some_and(|r| !r.trim().is_empty())
}

/// T009: a troca está ligada e tem como falar — com a senha da rede (protocolo 3)
/// ou, sem ela, como caixa do protocolo 4 (raiz fixada: o caixa que entrou por
/// convite não sabe a senha da rede, e não precisa).
pub(super) fn troca_ativa(conn: &Connection, a: &Ajustes) -> bool {
    a.ligado && (!a.senha.is_empty() || tem_raiz_fixada(conn))
}

/// Há convite ativo (não usado, com segredo, dentro da validade) nesta loja? Só
/// então o servidor aceita o aperto de mão de uma chave desconhecida, e só para o
/// canal de entrada.
pub(super) fn tem_convite_ativo(conn: &Connection, loja: &str) -> Resultado<bool> {
    Ok(cofre::convites_validos_agora(conn, loja, &crate::tempo::agora())?
        .iter()
        .any(|c| c.usado_em.is_none() && c.segredo_normalizado.is_some()))
}

/// Texto para a tela do caixa novo: o convite não confere.
pub const TEXTO_CONVITE_NAO_CONFERE: &str =
    "O convite não confere (vencido, já usado, de outra loja ou de outro caixa). Peça ao gerente um convite novo.";

/// T009 (etapa 5, A6): a resposta da loja a um pedido de entrada pelo convite. O
/// convite tem de casar com a prova amarrada a esta sessão; aí o caixa novo é
/// aprovado (assinado por este caixa, com o gerente do convite e auditoria) e
/// recebe a prova do servidor, a raiz, a loja e o registro. Repetir depois de uma
/// queda devolve a mesma aprovação (idempotente).
pub(super) fn responder_ingresso(
    conn: &Connection,
    ctx: &ContextoP4,
    chave_cliente: &Chave,
    exportador: &[u8; 32],
    pedido: &aprovados::Pedido,
    prova: &canal::ProvaIngressoCliente,
) -> canal::MensagemIngresso {
    let recusa = |motivo: String| {
        crate::registro::anotar(&format!(
            "troca de dados (protocolo 4): entrada recusada para a chave {}: {motivo}",
            hex(chave_cliente)
        ));
        canal::MensagemIngresso::Recusado { motivo }
    };
    let agora = crate::tempo::agora();
    let candidatos = match cofre::convites_validos_agora(conn, &ctx.loja, &agora) {
        Ok(c) => c,
        Err(e) => return recusa(e.to_string()),
    };
    let eu = ctx.eu();
    let Some(c) = canal::casar_convite_da_prova(&candidatos, exportador, chave_cliente, &eu, pedido, prova) else {
        return recusa(TEXTO_CONVITE_NAO_CONFERE.into());
    };
    if let Err(e) =
        cofre::consumir_convite_e_aprovar(conn, &ctx.identidade.identidade, &ctx.raiz, &ctx.loja, &c.id, pedido, &agora)
    {
        return recusa(e.to_string());
    }
    let prova_servidor =
        match canal::gerar_prova_servidor(&c.segredo_hash, exportador, chave_cliente, &eu, &ctx.raiz, &ctx.loja) {
            Ok(p) => p,
            Err(e) => return recusa(e.to_string()),
        };
    match aprovados::linhas(conn, &ctx.loja) {
        Ok(linhas) => {
            canal::MensagemIngresso::Aprovado { prova_servidor, raiz: ctx.raiz, loja_id: ctx.loja.clone(), linhas }
        }
        Err(e) => recusa(e.to_string()),
    }
}

/// Um anúncio `RSA4` ouvido por um caixa que ainda não é da loja (assinatura
/// conferida; a autorização não dá para conferir sem o registro): candidato a
/// endereço da loja para a entrada por convite.
#[derive(Clone, Debug)]
struct Ouvido {
    endereco: SocketAddr,
    loja: String,
    visto: Instant,
}

fn ouvidos_mapa() -> &'static Mutex<HashMap<PathBuf, Vec<Ouvido>>> {
    static M: std::sync::OnceLock<Mutex<HashMap<PathBuf, Vec<Ouvido>>>> = std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

/// Guarda um anúncio `RSA4` ouvido (só o endereço e a loja; até 32, por 2 minutos).
pub(super) fn guardar_ouvido(caminho: &Path, ip: std::net::IpAddr, bytes: &[u8]) {
    let Ok(a) = crate::troca::protocolo4::anuncio::abrir(bytes, agora_ms()) else { return };
    let endereco = SocketAddr::new(ip, a.porta);
    let mut m = ouvidos_mapa().lock().unwrap_or_else(|e| e.into_inner());
    let lista = m.entry(caminho.to_path_buf()).or_default();
    lista.retain(|o| o.endereco != endereco && o.visto.elapsed() < Duration::from_secs(120));
    lista.push(Ouvido { endereco, loja: a.loja_id, visto: Instant::now() });
    if lista.len() > 32 {
        lista.remove(0);
    }
}

/// Os endereços ouvidos recentemente (da `loja`, se ela é conhecida).
fn ouvidos(caminho: &Path, loja: &str) -> Vec<SocketAddr> {
    let m = ouvidos_mapa().lock().unwrap_or_else(|e| e.into_inner());
    m.get(caminho)
        .map(|l| {
            l.iter()
                .filter(|o| o.visto.elapsed() < Duration::from_secs(120) && (loja.is_empty() || o.loja == loja))
                .map(|o| o.endereco)
                .collect()
        })
        .unwrap_or_default()
}

/// Texto para a tela: não há onde procurar a loja.
pub const TEXTO_SEM_ENDERECO: &str = "Não foi encontrado nenhum caixa da loja na rede. Use o QR do convite, ou digite \
     o endereço do caixa onde o gerente gerou o convite.";

/// T009 (etapa 5, A6, D1): o caixa novo (sem usuários) entra na loja pelo convite
/// que o gerente gerou num caixa da loja. Procura a loja no endereço do QR, nos
/// `enderecos` digitados e nos anúncios `RSA4` ouvidos; com cada um, abre o canal de
/// entrada, prova o convite e só aceita a resposta cuja prova do servidor confere
/// (N-05). Aprovado: junta o registro, confere que a própria chave ficou
/// autorizada, fixa a raiz e a loja da entrada, liga a troca (sem a senha da rede)
/// com o endereço de quem aprovou. Devolve a loja.
pub fn juntar_com_convite(caminho: &Path, convite: &str, enderecos: &[String]) -> Resultado<String> {
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    let conn = abrir_conexao(caminho)?;
    if crate::usuarios::quantidade(&conn)? > 0 {
        return Err(Erro::validacao("Este caixa já tem usuários: ele já é de uma loja."));
    }
    let qr = canal::interpretar_convite_entrada(convite)?;
    let pasta = caminho.parent().ok_or_else(|| Erro::Interno("banco sem pasta".into()))?;
    let id = cofre::identidade_do_processo(pasta, &modo())?;
    let eu = id.identidade.publica();
    let disp = config::ler(&conn, "dispositivo_id")?.unwrap_or_default();
    let pedido = aprovados::Pedido::com_dispositivo(&id.identidade, &disp);
    let mut alvos: Vec<SocketAddr> = Vec::new();
    let candidatos = std::iter::once(qr.endereco.clone()).chain(enderecos.iter().cloned());
    for e in candidatos.filter(|e| !e.trim().is_empty()) {
        if let Ok(a) = endereco_completo(e.trim()) {
            if !alvos.contains(&a) {
                alvos.push(a);
            }
        }
    }
    for a in ouvidos(caminho, &qr.loja_id) {
        if !alvos.contains(&a) {
            alvos.push(a);
        }
    }
    if alvos.is_empty() {
        return Err(Erro::validacao(TEXTO_SEM_ENDERECO));
    }
    let mut ultimo: Option<Erro> = None;
    for alvo in alvos {
        let entrada = match tentar_entrar(&id.identidade, &eu, &qr, &pedido, &alvo) {
            Ok(e) => e,
            Err(e) => {
                ultimo = Some(e);
                continue;
            }
        };
        if !qr.loja_id.is_empty() && entrada.loja_id != qr.loja_id {
            ultimo = Some(Erro::validacao(TEXTO_CONVITE_NAO_CONFERE));
            continue;
        }
        aprovados::juntar(&conn, &entrada.loja_id, &entrada.linhas)?;
        if !aprovados::autorizada(&conn, &entrada.raiz, &entrada.loja_id, &eu)? {
            ultimo = Some(Erro::validacao(TEXTO_CONVITE_NAO_CONFERE));
            continue;
        }
        cofre::fixar_raiz_recebida(&conn, &entrada.raiz, &entrada.loja_id)?;
        config::gravar(&conn, "p4_chave_publica", &hex(&eu))?;
        config::gravar(&conn, "p4_cofre_sistema", if id.cofre_sistema { "1" } else { "0" })?;
        crate::loja::fixar_entrada_aprovada(&conn, &entrada.loja_id)?;
        let mut digitados: Vec<String> = config::ler(&conn, "sync_enderecos")?
            .unwrap_or_default()
            .split([',', ' ', ';', '\n'])
            .filter(|e| !e.trim().is_empty())
            .map(str::to_string)
            .collect();
        if !digitados.contains(&alvo.to_string()) {
            digitados.push(alvo.to_string());
        }
        config::gravar(&conn, "sync_enderecos", &digitados.join(","))?;
        config::gravar(&conn, "sync_ligado", "1")?;
        return Ok(entrada.loja_id);
    }
    Err(ultimo.unwrap_or_else(|| Erro::validacao(TEXTO_SEM_ENDERECO)))
}

/// Uma tentativa de entrada pelo convite no caixa em `alvo`.
fn tentar_entrar(
    eu_id: &crate::troca::protocolo4::Identidade,
    eu: &Chave,
    qr: &canal::ConviteQr,
    pedido: &aprovados::Pedido,
    alvo: &SocketAddr,
) -> Resultado<canal::EntradaAprovada> {
    let (fluxo, _) = rede_local::conectar(*alvo, None, Duration::from_secs(3)).map_err(rede)?;
    let mut c = canal::conectar_para_ingresso(fluxo, eu_id, qr.aprovador_chave, PRAZO_DO_APERTO, None)?;
    let prova = canal::gerar_prova_cliente(&qr.segredo, c.exportador_convite(), eu, &c.chave_do_par(), pedido)?;
    c.pedir_entrada(pedido, &prova, Duration::from_secs(10), None)?;
    let r = c.receber_aprovado(&qr.segredo, eu, TAMANHO_MAXIMO, Duration::from_secs(30), None);
    c.encerrar();
    r
}

/// Texto para a tela: o caixa do outro lado não é o que diz ser (I3).
pub(super) const TEXTO_CAIXA_NAO_CONFERE: &str =
    "O outro computador disse ser um caixa desta loja que não confere com a identidade dele. A troca foi recusada.";

/// Anota a recusa de um caixa cuja identidade não confere com o `dispositivo`
/// declarado (DT-P4-14).
pub(super) fn anotar_recusa_de_identidade(dispositivo: &str, chave: &Chave) {
    crate::registro::anotar(&format!(
        "troca de dados (protocolo 4): recusado o caixa que declarou o dispositivo {dispositivo:?} com a chave {}",
        hex(chave)
    ));
}
