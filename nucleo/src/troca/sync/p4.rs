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

#[cfg(test)]
thread_local! {
    /// Só nos testes (C2-7): quantas vezes esta linha de execução avaliou o registro.
    pub(super) static AVALIACOES_DO_REGISTRO: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// C2R3-7 (R3 da 2C): anota no registro técnico, mas o mesmo texto só uma vez a cada
/// 10 minutos (as leituras da tela e do motor repetem a cada poucos segundos).
pub(super) fn anotar_sem_repetir(texto: &str) {
    static VISTOS: std::sync::OnceLock<Mutex<HashMap<String, Instant>>> = std::sync::OnceLock::new();
    let mut m = VISTOS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if m.get(texto).is_some_and(|q| q.elapsed() < Duration::from_secs(600)) {
        return;
    }
    if m.len() > 256 {
        m.clear();
    }
    m.insert(texto.to_string(), Instant::now());
    drop(m);
    crate::registro::anotar(texto);
}

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
        #[cfg(test)]
        AVALIACOES_DO_REGISTRO.with(|c| c.set(c.get() + 1));
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

/// A loja deste caixa: a identidade dela ou, no caixa que acabou de entrar por
/// convite e ainda não a recebeu pela troca, a loja escolhida na entrada (T009).
fn loja_deste_caixa(conn: &Connection) -> Option<String> {
    crate::loja::id(conn)
        .ok()
        .filter(|l| !l.is_empty())
        .or_else(|| crate::loja::da_entrada(conn).ok().filter(|l| !l.is_empty()))
}

/// Texto para a tela: este caixa ainda não é aprovado na loja (o protocolo 4 não vale nele).
pub const TEXTO_CAIXA_SEM_APROVACAO: &str = "Este caixa ainda não foi aprovado na loja. O gerente aprova cada caixa \
     uma vez, com um convite gerado num caixa já aprovado (Configurações › Caixas da loja).";

/// O contexto P4 deste caixa, ou o motivo de ele ainda não falar o P4: sem loja,
/// sem raiz fixada, identidade indisponível agora (nunca gera identidade nova
/// sozinho) ou a própria chave não autorizada. A raiz é conferida antes da
/// identidade: um caixa sem raiz não paga o Argon2.
pub(super) fn contexto_ou_motivo(conn: &Connection, caminho: &Path) -> Resultado<ContextoP4> {
    let loja = loja_deste_caixa(conn).ok_or_else(|| Erro::validacao(TEXTO_CAIXA_SEM_APROVACAO))?;
    let raiz = cofre::raiz_atual(conn, &loja)?.ok_or_else(|| Erro::validacao(TEXTO_CAIXA_SEM_APROVACAO))?;
    let pasta = caminho.parent().ok_or_else(|| Erro::Interno("banco sem pasta".into()))?;
    let identidade = cofre::identidade_do_processo(pasta, &modo())?;
    let ctx = ContextoP4 { identidade, raiz, loja };
    if !aprovados::autorizada(conn, &ctx.raiz, &ctx.loja, &ctx.eu())? {
        return Err(Erro::validacao(TEXTO_CAIXA_SEM_APROVACAO));
    }
    Ok(ctx)
}

/// Depois de a identidade não abrir, quanto tempo o motor espera para tentar de novo
/// (C2-13: cada conexão repetiria o cofre do sistema e o Argon2).
const ESPERA_DEPOIS_DE_FALHA: Duration = Duration::from_secs(30);

fn falhas_do_contexto() -> &'static Mutex<HashMap<PathBuf, Instant>> {
    static M: std::sync::OnceLock<Mutex<HashMap<PathBuf, Instant>>> = std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

/// Só nos testes (C2-13): marca uma falha da identidade de `ha` atrás.
#[cfg(test)]
pub(super) fn marcar_falha_do_contexto(caminho: &Path, ha: Duration) {
    let quando = Instant::now().checked_sub(ha).unwrap_or_else(Instant::now);
    falhas_do_contexto().lock().unwrap_or_else(|e| e.into_inner()).insert(chave_dos_ouvidos(caminho), quando);
}

/// O contexto P4 deste caixa, ou `None` quando ele ainda não fala o P4 (veja
/// [`contexto_ou_motivo`]; a identidade indisponível vai para o registro técnico).
/// Para o motor, o servidor e o cliente da troca: depois de uma falha da identidade,
/// espera [`ESPERA_DEPOIS_DE_FALHA`] antes de tentar abrir de novo. Os comandos da
/// tela usam [`contexto_ou_motivo`] (tentam na hora e mostram o motivo).
pub(super) fn contexto(conn: &Connection, caminho: &Path) -> Option<ContextoP4> {
    let chave = chave_dos_ouvidos(caminho);
    if falhas_do_contexto()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&chave)
        .is_some_and(|quando| quando.elapsed() < ESPERA_DEPOIS_DE_FALHA)
    {
        return None;
    }
    // C2R2-12 (R2 da 2C): a espera vale só para a falha da **identidade** (cofre,
    // selo); um erro do banco ou "ainda não aprovado" não deixa o P4 parado.
    let loja = loja_deste_caixa(conn)?;
    let raiz = match cofre::raiz_atual(conn, &loja) {
        Ok(Some(r)) => r,
        Ok(None) => return None,
        Err(e) => {
            anotar_sem_repetir(&format!("troca de dados (protocolo 4): raiz da loja: {e}"));
            return None;
        }
    };
    let pasta = caminho.parent()?;
    let identidade = match cofre::identidade_do_processo(pasta, &modo()) {
        Ok(i) => i,
        Err(e) => {
            anotar_sem_repetir(&format!("troca de dados (protocolo 4): identidade indisponível agora: {e}"));
            falhas_do_contexto().lock().unwrap_or_else(|e| e.into_inner()).insert(chave, Instant::now());
            return None;
        }
    };
    let ctx = ContextoP4 { identidade, raiz, loja };
    match aprovados::autorizada(conn, &ctx.raiz, &ctx.loja, &ctx.eu()) {
        Ok(true) => Some(ctx),
        Ok(false) => None,
        Err(e) => {
            anotar_sem_repetir(&format!("troca de dados (protocolo 4): registro de caixas: {e}"));
            None
        }
    }
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
        // O identificador do usuário (o "gerente" assinado e da auditoria; a tela
        // mostra o nome pela tabela de usuários, que vai para todos os caixas).
        .map(|u| u.id)
        .unwrap_or_default();
    // C2-14 (R1 da 2C): quem já ouve caixas atualizados desta loja (anúncio assinado)
    // não cria raiz nova — a loja já tem uma; este caixa entra pelo convite.
    let distribuidor = crate::faixas::sou_distribuidor(conn)? && ouvidos(caminho, &loja).is_empty();
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

/// T025 (007): a junção do registro lê as linhas gravadas e depois grava; fora de transação (a conexão do cliente da
/// troca), o `SAVEPOINT` de `aprovados::juntar` era uma transação de leitura e outra conexão que gravasse no meio
/// fazia o SQLite recusar na hora ("database is locked"). Aqui ela já começa reservando a gravação (a conexão da troca
/// é `Immediate`); dentro de uma transação de quem chama, fica nela.
pub(super) fn juntar_na_cadeia_na_vez(
    conn: &Connection,
    raiz: &Chave,
    loja: &str,
    linhas: &aprovados::Linhas,
) -> Resultado<aprovados::Juntadas> {
    if !conn.is_autocommit() {
        return aprovados::juntar_na_cadeia(conn, raiz, loja, linhas);
    }
    let tx = conn.unchecked_transaction()?;
    let j = aprovados::juntar_na_cadeia(&tx, raiz, loja, linhas)?;
    tx.commit()?;
    Ok(j)
}

/// Junta o registro que veio numa mensagem P4 (acrescenta, nunca tira; cada linha
/// se confere sozinha). Mensagem sem registro não muda nada; registro estragado é
/// recusado inteiro.
pub(super) fn juntar_registro(conn: &Connection, ctx: &ContextoP4, mensagem: &Value) -> Resultado<()> {
    let Some(r) = mensagem.get("registro") else { return Ok(()) };
    let linhas: aprovados::Linhas = serde_json::from_value(r.clone())
        .map_err(|_| Erro::validacao("O registro de caixas que veio do outro caixa está estragado."))?;
    // C2R2-3: só o que entra na cadeia da raiz (lixo bem assinado fica de fora).
    let j = juntar_na_cadeia_na_vez(conn, &ctx.raiz, &ctx.loja, &linhas)?;
    if j.sem_lugar > 0 {
        // C2R5-4: só no registro técnico (sem repetir); o aviso do Início vem do estado.
        anotar_sem_repetir(
            "registro de caixas: aprovações ficaram de fora por falta de lugar (teto do registro ou limite por \
             aprovador)",
        );
    }
    Ok(())
}

/// O fio do protocolo 4: o canal TLS autenticado. Toda mensagem que sai leva o
/// registro deste caixa e o recurso [`RECURSO_CHAVES`].
///
/// C2-3 (R1 da 2C): o registro vai só nas mensagens do **servidor** (as respostas,
/// com o limite grande da resposta); o pedido do cliente fica pequeno (o limite do
/// pedido é de 64 KiB e o registro só cresce). Cada caixa recebe o registro dos
/// outros quando puxa deles — a puxada acontece nos dois sentidos.
pub(super) struct FioP4<C> {
    canal: canal::CanalAutenticado<C>,
    registro: Option<Value>,
}

impl<C> FioP4<C> {
    /// O fio do servidor (as respostas levam o registro deste caixa).
    pub(super) fn do_servidor(canal: canal::CanalAutenticado<C>, registro: Value) -> FioP4<C> {
        FioP4 { canal, registro: Some(registro) }
    }

    /// O fio do cliente (o pedido não leva o registro).
    pub(super) fn do_cliente(canal: canal::CanalAutenticado<C>) -> FioP4<C> {
        FioP4 { canal, registro: None }
    }

    /// Troca o registro que vai nas próximas mensagens (depois de juntar o do outro).
    pub(super) fn trocar_registro(&mut self, registro: Value) {
        if self.registro.is_some() {
            self.registro = Some(registro);
        }
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
            if let Some(registro) = &self.registro {
                obj.insert("registro".into(), registro.clone());
            }
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

/// T010 (A4) e C2-8 (R1 da 2C, M2): o anúncio cifrado com a senha da rede (`RST3`) só
/// sai com a senha e com o P3 valendo neste caixa (fora da convivência, nada tirado
/// da senha vai para a rede: R-01).
pub(super) fn anuncio_p3_permitido(conn: &Connection, a: &Ajustes) -> bool {
    !a.senha.is_empty() && p3_permitido(conn)
}

/// DT-P4-17 e C2-8 (M2, M7): os anúncios que este caixa manda agora — o `RST3`
/// cifrado (só com `k`, que existe só quando [`anuncio_p3_permitido`]) e o `RSA4`
/// assinado de quem fala o P4. (Nome para o registro técnico, pacote.)
/// Só nos testes (C2R2-4): os anúncios que o motor de cada banco mandou de fato,
/// gravados no ponto do envio (o laço do motor), na ordem.
#[cfg(test)]
pub(super) fn anuncios_mandados() -> &'static Mutex<HashMap<PathBuf, Vec<&'static str>>> {
    static M: std::sync::OnceLock<Mutex<HashMap<PathBuf, Vec<&'static str>>>> = std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

pub(super) fn pacotes_de_anuncio(
    k: Option<&Chaves>,
    ctx: Option<&ContextoP4>,
    a: &Ajustes,
    porta_tcp: u16,
) -> Vec<(&'static str, Vec<u8>)> {
    let mut pacotes = Vec::new();
    if let Some(k) = k {
        let ola = anuncio(&a.dispositivo, &a.nome, porta_tcp);
        if let Ok(p) = cifrar(k, ola.to_string().as_bytes()) {
            pacotes.push(("RST3", p));
        }
    }
    if let Some(ctx) = ctx {
        if let Ok(p) = pacote_de_anuncio(ctx, porta_tcp) {
            pacotes.push(("RSA4", p));
        }
    }
    pacotes
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
    // C2-7 (R1 da 2C): a conta barata primeiro — formato, janela de tempo, assinatura
    // e loja; só um anúncio que passou disso faz avaliar o registro.
    let previo = crate::troca::protocolo4::anuncio::abrir(bytes, agora_ms()).ok()?;
    if previo.loja_id != ctx.loja || previo.chave == ctx.eu() {
        return None;
    }
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

/// A4 (T010): a configuração com o prazo da convivência com os caixas do protocolo 3
/// (data e hora ISO-8601 UTC; vazia = desligada, o padrão). C2-1 (R1 da 2C): é
/// **deste caixa** (não vai para os outros): um caixa atualizado com a convivência
/// ligada já serve de ponte para os antigos, e os dados chegam aos outros pelo P4.
/// Assim nenhum outro caixa (nem um antigo com a senha da rede) muda o prazo daqui.
pub const CHAVE_CONVIVENCIA: &str = "p4_convivencia_p3_ate";
/// A4: o prazo máximo da convivência.
pub const CONVIVENCIA_MAXIMA_DIAS: i64 = 30;
/// Texto para a tela (dos dois lados): o caixa do protocolo 3 foi recusado.
pub const TEXTO_P3_RECUSADO: &str = "Este caixa troca dados só com caixas atualizados (versão 0.9.19 ou mais nova). \
     Atualize o outro caixa. Se precisar de um tempo, o gerente pode liberar os caixas antigos por alguns dias em \
     Configurações › Caixas da loja.";

/// Até quando os caixas do protocolo 3 ainda trocam dados (`None` = desligada ou
/// vencida).
///
/// C2-1: lida com desconfiança — só vale um prazo no formato de `tempo::agora`
/// (`AAAA-MM-DDTHH:MM:SS…Z`, só ASCII) que ainda não venceu e não passa de agora +
/// [`CONVIVENCIA_MAXIMA_DIAS`] (um valor fora disso conta como desligada).
pub fn convivencia_p3_ate(conn: &Connection) -> Resultado<Option<String>> {
    let ate = config::ler(conn, CHAVE_CONVIVENCIA)?.unwrap_or_default();
    let ate = ate.trim();
    let agora = crate::tempo::agora();
    let formato = ate.len() >= 20
        && ate.is_ascii()
        && ate.ends_with('Z')
        && ate.as_bytes().iter().enumerate().take(19).all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            _ => b.is_ascii_digit(),
        });
    if !formato || ate <= agora.as_str() {
        return Ok(None);
    }
    let teto = crate::tempo::somar_segundos(&agora, CONVIVENCIA_MAXIMA_DIAS * 24 * 60 * 60 + 60).unwrap_or_default();
    Ok((ate <= teto.as_str()).then(|| ate.to_string()))
}

/// C2R4-1: o registro de caixas desta loja está no teto (um caixa aprovou caixas demais).
/// O Início avisa enquanto for verdade.
pub fn registro_de_caixas_cheio(conn: &Connection) -> Resultado<bool> {
    // C2R5-4 (R5 da 2C): pelo estado gravado, na hora (uma conferência do registro).
    // C2R2-6: um registro estranho (duas raízes) não derruba o Início; o motivo vai para
    // o registro técnico.
    let cheio = (|| -> Resultado<bool> {
        let Some((loja, raiz)) = loja_e_raiz(conn)? else { return Ok(false) };
        aprovados::registro_cheio(conn, &raiz, &loja)
    })();
    Ok(cheio.unwrap_or_else(|e| {
        anotar_sem_repetir(&format!("troca de dados (protocolo 4): aviso do registro cheio sem o registro: {e}"));
        false
    }))
}

/// T010: o protocolo 3 vale com este caixa? O caixa que ainda não é do P4 (sem raiz
/// fixada) continua como antes; o caixa do P4, só durante a convivência (A4).
pub(super) fn p3_permitido(conn: &Connection) -> bool {
    !tem_raiz_fixada(conn) || convivencia_p3_ate(conn).ok().flatten().is_some()
}

/// A4 (T010): o gerente liga (`dias` de 1 a 30) ou desliga (`None`) a convivência
/// com os caixas do protocolo 3. Vale na loja inteira e vai para a auditoria.
/// Devolve até quando ficou ligada.
pub fn definir_convivencia_p3(conn: &Connection, dias: Option<i64>, gerente: &str) -> Resultado<Option<String>> {
    if gerente.trim().is_empty() {
        return Err(Erro::validacao("Só o gerente libera os caixas antigos."));
    }
    let ate = match dias {
        None => None,
        Some(d) if (1..=CONVIVENCIA_MAXIMA_DIAS).contains(&d) => Some(
            crate::tempo::somar_segundos(&crate::tempo::agora(), d * 24 * 60 * 60)
                .ok_or_else(|| Erro::Interno("não foi possível calcular o prazo".into()))?,
        ),
        Some(_) => {
            return Err(Erro::campo(
                "dias",
                format!("Os caixas antigos podem ser liberados por 1 a {CONVIVENCIA_MAXIMA_DIAS} dias."),
            ))
        }
    };
    config::gravar(conn, CHAVE_CONVIVENCIA, ate.as_deref().unwrap_or(""))?;
    let disp = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    let detalhe = match &ate {
        Some(a) => format!("Caixas antigos liberados até {a}"),
        None => "Caixas antigos recusados".to_string(),
    };
    crate::auditoria::registrar(
        conn,
        &disp,
        Some(gerente),
        "p4_convivencia_p3",
        "troca",
        CHAVE_CONVIVENCIA,
        &[detalhe],
    )?;
    Ok(ate)
}

/// A raiz do protocolo 4 está fixada neste caixa (pino `p4_raiz`)?
pub fn tem_raiz_fixada(conn: &Connection) -> bool {
    config::ler(conn, "p4_raiz").ok().flatten().is_some_and(|r| !r.trim().is_empty())
}

/// T009: a troca está ligada e tem como falar — com a senha da rede (protocolo 3)
/// ou, sem ela, como caixa do protocolo 4 (raiz fixada: o caixa que entrou por
/// convite não sabe a senha da rede, e não precisa).
pub fn troca_ativa(conn: &Connection, a: &Ajustes) -> bool {
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

/// T025 (007): consome o convite numa transação que já começa reservando a gravação (a conexão da troca é
/// `Immediate`). Sem ela, o `SAVEPOINT` do cofre abria uma transação de leitura que depois tentava gravar: se outra
/// conexão deste caixa (o motor, o balcão) gravasse no meio, o SQLite recusava na hora com "database is locked", sem
/// esperar a vez, e o caixa que entrava pelo convite recebia o erro (1 em 4 sob carga).
pub(super) fn consumir_convite_na_vez(
    conn: &Connection,
    aprovador: &crate::troca::protocolo4::Identidade,
    raiz: &Chave,
    loja: &str,
    convite_id: &str,
    pedido: &aprovados::Pedido,
    agora: &str,
) -> Resultado<aprovados::Aprovacao> {
    if !conn.is_autocommit() {
        return cofre::consumir_convite_e_aprovar(conn, aprovador, raiz, loja, convite_id, pedido, agora);
    }
    let tx = conn.unchecked_transaction()?;
    let a = cofre::consumir_convite_e_aprovar(&tx, aprovador, raiz, loja, convite_id, pedido, agora)?;
    tx.commit()?;
    Ok(a)
}

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
        consumir_convite_na_vez(conn, &ctx.identidade.identidade, &ctx.raiz, &ctx.loja, &c.id, pedido, &agora)
    {
        return recusa(e.to_string());
    }
    // C2R3-4 (R3 da 2C): a aprovação nova NÃO tira o bloqueio antigo do `dispositivo`
    // (ele é só declarado pelo caixa novo; o código que o gerente conferiu não o cobre).
    // A lista de caixas mostra o caso e o gerente usa "Readmitir".
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

/// A chave do mapa dos ouvidos: o banco pelo caminho verdadeiro (o motor e a
/// conexão do banco podem dizer o mesmo arquivo de jeitos diferentes).
fn chave_dos_ouvidos(caminho: &Path) -> PathBuf {
    std::fs::canonicalize(caminho).unwrap_or_else(|_| caminho.to_path_buf())
}

/// Guarda um anúncio `RSA4` ouvido (só o endereço e a loja; até 32, por 2 minutos).
pub(super) fn guardar_ouvido(caminho: &Path, ip: std::net::IpAddr, bytes: &[u8]) {
    let Ok(a) = crate::troca::protocolo4::anuncio::abrir(bytes, agora_ms()) else { return };
    let endereco = SocketAddr::new(ip, a.porta);
    let mut m = ouvidos_mapa().lock().unwrap_or_else(|e| e.into_inner());
    let lista = m.entry(chave_dos_ouvidos(caminho)).or_default();
    lista.retain(|o| o.endereco != endereco && o.visto.elapsed() < Duration::from_secs(120));
    lista.push(Ouvido { endereco, loja: a.loja_id, visto: Instant::now() });
    if lista.len() > 32 {
        lista.remove(0);
    }
}

/// Os endereços ouvidos recentemente (da `loja`, se ela é conhecida).
fn ouvidos(caminho: &Path, loja: &str) -> Vec<SocketAddr> {
    let m = ouvidos_mapa().lock().unwrap_or_else(|e| e.into_inner());
    m.get(&chave_dos_ouvidos(caminho))
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

/// Quem entra pelo convite (DT-P4-33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuemEntra {
    /// O caixa novo, sem usuários (T009): entra na loja e recebe tudo dela.
    CaixaNovo,
    /// O caixa que já é da loja (tem usuários) e ainda não foi aprovado no
    /// protocolo 4 (atualizou para a 0.9.19 e não é a raiz): só a aprovação.
    JaDaLoja,
}

/// O que a entrada pelo convite deixou: a loja e o nome que o gerente deu ao
/// caixa no convite (vem da aprovação assinada).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntradaPeloConvite {
    pub loja: String,
    pub nome: String,
}

/// Texto para a tela: o convite é de outra loja.
pub const TEXTO_CONVITE_DE_OUTRA_LOJA: &str =
    "Este convite é de outra loja. Peça ao gerente um convite gerado num caixa desta loja.";
/// Texto para a tela: o caixa ainda não recebeu a identidade da loja.
pub const TEXTO_SEM_IDENTIDADE_DA_LOJA: &str = "Este caixa ainda não recebeu a identidade da loja. Ligue a troca \
     de dados, espere a primeira troca com outro caixa e tente de novo.";
/// Texto para a tela: este caixa já é aprovado.
pub const TEXTO_JA_APROVADO: &str = "Este caixa já é aprovado na loja: não precisa de convite.";

/// Por quanto tempo, no máximo, o caixa procura a loja pelos anúncios quando não
/// tem endereço nenhum (os caixas anunciam a cada 10 segundos).
const PRAZO_DA_PROCURA: Duration = Duration::from_secs(12);

/// T009 (etapa 5, A6, D1): o caixa novo (sem usuários) entra na loja pelo convite
/// que o gerente gerou num caixa da loja. Procura a loja no endereço do QR, nos
/// `enderecos` digitados, nos anúncios `RSA4` ouvidos e, sem nenhum, escutando os
/// anúncios por alguns segundos; com cada um, abre o canal de entrada, prova o
/// convite e só aceita a resposta cuja prova do servidor confere (N-05).
/// Aprovado: junta o registro, confere que a própria chave ficou autorizada, fixa
/// a raiz e a loja da entrada, dá ao caixa o nome que o gerente escolheu e liga a
/// troca (sem a senha da rede) com o endereço de quem aprovou.
pub fn juntar_com_convite(caminho: &Path, convite: &str, enderecos: &[String]) -> Resultado<EntradaPeloConvite> {
    entrar_com_convite(caminho, convite, enderecos, QuemEntra::CaixaNovo)
}

/// T012 (DT-P4-33): o caixa que já é da loja e ainda não foi aprovado no
/// protocolo 4 recebe a aprovação pelo convite (o mesmo canal de entrada). O
/// convite tem de ser da loja dele; nada muda nos dados nem na troca, só passa a
/// falar o protocolo 4.
pub fn aprovar_este_caixa_com_convite(
    caminho: &Path,
    convite: &str,
    enderecos: &[String],
) -> Resultado<EntradaPeloConvite> {
    entrar_com_convite(caminho, convite, enderecos, QuemEntra::JaDaLoja)
}

fn entrar_com_convite(
    caminho: &Path,
    convite: &str,
    enderecos: &[String],
    quem: QuemEntra,
) -> Resultado<EntradaPeloConvite> {
    let _trava = TRAVA.read().unwrap_or_else(|e| e.into_inner());
    let mut conn = abrir_conexao(caminho)?;
    let usuarios = crate::usuarios::quantidade(&conn)?;
    let loja_daqui = match quem {
        QuemEntra::CaixaNovo => {
            if usuarios > 0 {
                return Err(Erro::validacao("Este caixa já tem usuários: ele já é de uma loja."));
            }
            String::new()
        }
        QuemEntra::JaDaLoja => {
            if usuarios == 0 {
                return Err(Erro::validacao(
                    "Este caixa ainda não é de uma loja: use Juntar este caixa, na tela de boas-vindas.",
                ));
            }
            let l = crate::loja::id(&conn)?;
            if l.is_empty() {
                return Err(Erro::validacao(TEXTO_SEM_IDENTIDADE_DA_LOJA));
            }
            if contexto(&conn, caminho).is_some() {
                return Err(Erro::validacao(TEXTO_JA_APROVADO));
            }
            l
        }
    };
    let qr = canal::interpretar_convite_entrada(convite)?;
    if quem == QuemEntra::JaDaLoja && !qr.loja_id.is_empty() && qr.loja_id != loja_daqui {
        return Err(Erro::campo("convite", TEXTO_CONVITE_DE_OUTRA_LOJA));
    }
    // A loja que a resposta tem de trazer (vazia = o convite em texto, sem a loja).
    let loja_esperada = if qr.loja_id.is_empty() { loja_daqui.clone() } else { qr.loja_id.clone() };
    let pasta = caminho.parent().ok_or_else(|| Erro::Interno("banco sem pasta".into()))?;
    let id = cofre::identidade_do_processo(pasta, &modo())?;
    let eu = id.identidade.publica();
    let disp = config::ler(&conn, "dispositivo_id")?.unwrap_or_default();
    let pedido = aprovados::Pedido::com_dispositivo(&id.identidade, &disp);
    let mut alvos: Vec<SocketAddr> = Vec::new();
    if let Ok(a) = endereco_completo(qr.endereco.trim()) {
        alvos.push(a);
    }
    // E6-1 (T002 da 009): um endereço DIGITADO que não dá certo (um nome que não está na rede, ou que não respondeu a
    // tempo) é dito no próprio campo, em vez de ser pulado em silêncio. Mas só se nenhum outro caminho (o QR, os
    // anúncios) funcionar: um nome errado não impede a entrada pelo endereço certo (R1 M4).
    let mut erro_do_digitado: Option<Erro> = None;
    for e in enderecos.iter().filter(|e| !e.trim().is_empty()) {
        match endereco_completo(e.trim()) {
            Ok(a) if !alvos.contains(&a) => alvos.push(a),
            Ok(_) => {}
            Err(erro) => {
                erro_do_digitado.get_or_insert_with(|| Erro::campo("enderecos", erro.to_string()));
            }
        }
    }
    for a in ouvidos(caminho, &loja_esperada) {
        if !alvos.contains(&a) {
            alvos.push(a);
        }
    }
    if alvos.is_empty() {
        alvos = procurar_pelos_anuncios(&loja_esperada, PRAZO_DA_PROCURA);
    }
    if alvos.is_empty() {
        return Err(erro_do_digitado.unwrap_or_else(|| Erro::validacao(TEXTO_SEM_ENDERECO)));
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
        if !loja_esperada.is_empty() && entrada.loja_id != loja_esperada {
            ultimo = Some(Erro::validacao(TEXTO_CONVITE_NAO_CONFERE));
            continue;
        }
        // Tudo o que a entrada grava, numa transação só: ou o caixa fica aprovado
        // e ligado à loja, ou nada muda aqui.
        let tx = conn.transaction()?;
        aprovados::juntar_na_cadeia(&tx, &entrada.raiz, &entrada.loja_id, &entrada.linhas)?;
        if !aprovados::autorizada(&tx, &entrada.raiz, &entrada.loja_id, &eu)? {
            ultimo = Some(Erro::validacao(TEXTO_CONVITE_NAO_CONFERE));
            continue;
        }
        cofre::fixar_raiz_recebida(&tx, &entrada.raiz, &entrada.loja_id)?;
        config::gravar(&tx, "p4_chave_publica", &hex(&eu))?;
        config::gravar(&tx, "p4_cofre_sistema", if id.cofre_sistema { "1" } else { "0" })?;
        let nome = aprovados::linhas(&tx, &entrada.loja_id)?
            .aprovacoes
            .into_iter()
            .filter(|a| a.chave == eu && a.assinatura_confere())
            .max_by(|a, b| a.aprovado_em.cmp(&b.aprovado_em))
            .map(|a| a.nome)
            .unwrap_or_default();
        if quem == QuemEntra::CaixaNovo {
            crate::loja::fixar_entrada_aprovada(&tx, &entrada.loja_id)?;
            if !nome.trim().is_empty() {
                config::gravar(&tx, "caixa_nome", &nome)?;
            }
            let mut digitados: Vec<String> = config::ler(&tx, "sync_enderecos")?
                .unwrap_or_default()
                .split([',', ' ', ';', '\n'])
                .filter(|e| !e.trim().is_empty())
                .map(str::to_string)
                .collect();
            if !digitados.contains(&alvo.to_string()) {
                digitados.push(alvo.to_string());
            }
            config::gravar(&tx, "sync_enderecos", &digitados.join(","))?;
            config::gravar(&tx, "sync_ligado", "1")?;
        }
        tx.commit()?;
        return Ok(EntradaPeloConvite { loja: entrada.loja_id, nome });
    }
    Err(erro_do_digitado.or(ultimo).unwrap_or_else(|| Erro::validacao(TEXTO_SEM_ENDERECO)))
}

/// T012: sem endereço nenhum, o caixa escuta os anúncios `RSA4` por até `prazo`
/// (só durante a entrada que a pessoa pediu; a porta fecha no fim). Assinatura
/// conferida; a autorização não dá para conferir sem o registro (quem confere é a
/// prova do servidor, N-05). Para pouco depois do primeiro achado, para juntar os
/// outros caixas que anunciam na mesma janela.
fn procurar_pelos_anuncios(loja: &str, prazo: Duration) -> Vec<SocketAddr> {
    procurar_pelos_anuncios_em(loja, prazo, ("0.0.0.0", crate::portatil::portas().1))
}

/// O mesmo, escutando em `onde` (os testes usam uma porta sorteada no loopback).
pub(super) fn procurar_pelos_anuncios_em(
    loja: &str,
    prazo: Duration,
    onde: impl std::net::ToSocketAddrs,
) -> Vec<SocketAddr> {
    let Ok(s) = UdpSocket::bind(onde) else { return Vec::new() };
    if s.set_read_timeout(Some(Duration::from_millis(250))).is_err() {
        return Vec::new();
    }
    let inicio = Instant::now();
    let mut primeiro: Option<Instant> = None;
    let mut achados: Vec<SocketAddr> = Vec::new();
    let mut buf = [0u8; 2048];
    while inicio.elapsed() < prazo && primeiro.is_none_or(|p| p.elapsed() < Duration::from_secs(3)) {
        let Ok((n, origem)) = s.recv_from(&mut buf) else { continue };
        let Ok(a) = crate::troca::protocolo4::anuncio::abrir(&buf[..n], agora_ms()) else { continue };
        if !loja.is_empty() && a.loja_id != loja {
            continue;
        }
        let e = SocketAddr::new(origem.ip(), a.porta);
        if !achados.contains(&e) {
            achados.push(e);
            primeiro.get_or_insert_with(Instant::now);
        }
    }
    achados
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

/// A loja e a raiz deste caixa, só pelo banco (sem abrir a identidade): `None`
/// quando ele ainda não fala o protocolo 4.
fn loja_e_raiz(conn: &Connection) -> Resultado<Option<(String, Chave)>> {
    let Some(loja) = loja_deste_caixa(conn) else { return Ok(None) };
    Ok(cofre::raiz_atual(conn, &loja)?.map(|r| (loja, r)))
}

/// A chave autorizada do caixa `dispositivo` no registro, só pelo banco.
fn chave_registrada_de(conn: &Connection, dispositivo: &str) -> Resultado<Option<(String, Chave, Chave)>> {
    let Some((loja, raiz)) = loja_e_raiz(conn)? else { return Ok(None) };
    Ok(aprovados::chave_do_dispositivo(conn, &raiz, &loja, dispositivo)?.map(|k| (loja, raiz, k)))
}

/// C2-5 (R1 da 2C): o caixa `dispositivo` tem chave no registro desta loja? Só pelo
/// banco (sem a identidade deste caixa, que pode não abrir agora). Um erro (registro
/// ambíguo, banco) conta como "tem": nunca rebaixa para o P3 por causa de um erro.
pub(super) fn tem_chave_no_registro(conn: &Connection, dispositivo: &str) -> bool {
    if dispositivo.trim().is_empty() {
        return false;
    }
    match loja_e_raiz(conn) {
        Ok(None) => false,
        Ok(Some((loja, raiz))) => !matches!(aprovados::chave_do_dispositivo(conn, &raiz, &loja, dispositivo), Ok(None)),
        Err(_) => true,
    }
}

/// Texto para a tela: o outro caixa tem chave e este não consegue falar o P4 agora.
pub const TEXTO_P4_INDISPONIVEL: &str = "Este caixa não conseguiu usar a identidade dele agora para falar com o \
     outro caixa (que só troca dados pela conexão segura). A troca com ele espera; o Rustinho tenta de novo sozinho.";

/// T009 (etapa 6, P4-4): os caixas que ficariam sem cadeia se o caixa `dispositivo`
/// fosse retirado (a tela lista antes de confirmar). Vazio quando ele não tem chave.
/// Só lê o banco (T012: pode ser chamado na linha da tela).
pub fn dependentes_da_retirada(conn: &Connection, dispositivo: &str) -> Resultado<Vec<String>> {
    let Some((loja, raiz, chave)) = chave_registrada_de(conn, dispositivo)? else { return Ok(Vec::new()) };
    Ok(aprovados::dependentes_de(conn, &raiz, &loja, &chave)?.into_iter().map(|a| a.nome).collect())
}

/// Texto para a tela: a retirada pela chave precisa da identidade deste caixa.
pub const TEXTO_RETIRADA_SEM_IDENTIDADE: &str = "Este caixa tem a chave de segurança aprovada na loja, mas não \
     deu para usar a identidade deste computador agora. Nada foi retirado. Tente de novo em instantes; se \
     continuar, faça a retirada em outro caixa aprovado.";

/// T009 (etapa 6; E5, P4-3, S-10): retira o caixa `dispositivo` da loja, com o
/// gerente (o identificador do usuário vai assinado e na auditoria; o nome, no
/// bloqueio de antes, que a tela mostra), numa transação só:
/// - sempre o bloqueio de antes (`caixa_retirado:<dispositivo>`), que os caixas do
///   protocolo 3 respeitam (S-10), com as conferências de sempre (motivo, a si
///   mesmo, pendências recebidas dele);
/// - e, quando ele tem chave no registro, a retirada **assinada por este caixa**,
///   que vale por máquina (mesmo que ele troque o identificador declarado) e
///   viaja pelo canal autenticado. A raiz não é retirada (P4-3): nada é gravado.
///
/// T012 (DT-P4-35): a chave do caixa é procurada só pelo banco; se ele tem chave e
/// este caixa não consegue assinar agora (identidade indisponível, ou este caixa
/// não aprovado), a retirada é **recusada** inteira: nunca fica só o bloqueio de
/// antes, que o protocolo 4 não respeita.
pub fn retirar_caixa_por_chave(
    conn: &Connection,
    caminho: &Path,
    dispositivo: &str,
    motivo: &str,
    gerente_id: &str,
    gerente_nome: &str,
) -> Resultado<()> {
    let chave = chave_registrada_de(conn, dispositivo)?.map(|(_, _, k)| k);
    let ctx = match chave {
        Some(_) => Some(contexto_ou_motivo(conn, caminho).map_err(|e| {
            crate::registro::anotar(&format!("troca de dados (protocolo 4): retirada pela chave recusada: {e}"));
            Erro::validacao(TEXTO_RETIRADA_SEM_IDENTIDADE)
        })?),
        None => None,
    };
    conn.execute_batch("SAVEPOINT p4_retirar_caixa")?;
    let r = (|| -> Resultado<()> {
        retirar_caixa(conn, dispositivo, motivo, gerente_nome)?;
        if let (Some(ctx), Some(chave)) = (&ctx, chave) {
            let motivo = crate::texto::limpar(motivo);
            aprovados::retirar(conn, &ctx.identidade.identidade, &ctx.raiz, &ctx.loja, &chave, &motivo, gerente_id)?;
        }
        Ok(())
    })();
    match r {
        Ok(()) => conn.execute_batch("RELEASE p4_retirar_caixa").map_err(Into::into),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO p4_retirar_caixa; RELEASE p4_retirar_caixa");
            Err(e)
        }
    }
}

/// C2R2-5 (R2 da 2C): retira **uma** identidade escolhida do caixa `dispositivo`
/// que tem mais de uma autorizada (o registro ambíguo não tem outra saída: a troca
/// dele falha fechada). Não grava o bloqueio antigo (o caixa continua, com a outra).
pub fn retirar_identidade_do_caixa(
    conn: &Connection,
    caminho: &Path,
    dispositivo: &str,
    chave_hex: &str,
    motivo: &str,
    gerente_id: &str,
) -> Resultado<()> {
    let motivo = crate::texto::limpar(motivo);
    if motivo.is_empty() {
        return Err(Erro::campo("motivo", "Diga por que esta identidade sai da loja."));
    }
    let chave = crate::troca::protocolo4::de_hex::<32>(chave_hex.trim())
        .ok_or_else(|| Erro::validacao("Identidade inválida."))?;
    let Some((loja, raiz)) = loja_e_raiz(conn)? else { return Err(Erro::validacao(TEXTO_CAIXA_SEM_APROVACAO)) };
    let desta = aprovados::situacoes_por_dispositivo(conn, &raiz, &loja)?
        .get(dispositivo.trim())
        // C2R5-1: também a chave já retirada (só a raiz a retira de novo, para o lugar voltar:
        // `aprovados::retirar` confere).
        .is_some_and(|l| {
            l.iter().any(|(k, s)| {
                *k == chave && matches!(s, aprovados::Situacao::Autorizada | aprovados::Situacao::Retirada)
            })
        });
    if !desta {
        return Err(Erro::validacao("Esta identidade não é uma das aprovadas deste caixa."));
    }
    let ctx = contexto_ou_motivo(conn, caminho).map_err(|_| Erro::validacao(TEXTO_RETIRADA_SEM_IDENTIDADE))?;
    aprovados::retirar(conn, &ctx.identidade.identidade, &ctx.raiz, &ctx.loja, &chave, &motivo, gerente_id)?;
    Ok(())
}

/// Texto para a tela: o caixa foi retirado pela chave (não volta pelo "Readmitir").
pub const TEXTO_RETIRADO_PELA_CHAVE: &str = "Este caixa foi retirado pela chave de segurança dele: a retirada vale \
     para sempre para aquela chave. Para ele voltar, o gerente aprova de novo com um convite novo, depois que ele \
     gerar uma identidade nova.";

/// T012 (DT-P4-35): readmitir um caixa retirado. O bloqueio de antes sai como
/// sempre; um caixa retirado **pela chave** não volta por aqui (a retirada
/// assinada vence qualquer aprovação, P4: a volta é por convite novo).
pub fn readmitir_caixa_por_chave(conn: &Connection, dispositivo: &str) -> Resultado<()> {
    if let Some((loja, raiz)) = loja_e_raiz(conn)? {
        let situacoes = aprovados::situacoes_por_dispositivo(conn, &raiz, &loja)?;
        let retirado = situacoes.get(dispositivo.trim()).is_some_and(|l| {
            l.iter().any(|(_, s)| *s == aprovados::Situacao::Retirada)
                && !l.iter().any(|(_, s)| *s == aprovados::Situacao::Autorizada)
        });
        if retirado {
            return Err(Erro::validacao(TEXTO_RETIRADO_PELA_CHAVE));
        }
    }
    readmitir_caixa(conn, dispositivo)
}

/// T012 (DT-P4-35): o que o registro diz de um caixa conhecido, para a tela.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChaveDoCaixa {
    /// Tem chave autorizada agora (troca pelo protocolo 4).
    pub tem_chave: bool,
    /// É a raiz da loja (não pode ser retirado, P4-3).
    pub raiz: bool,
    /// Foi retirado pela chave (e não tem outra autorizada).
    pub retirado_pela_chave: bool,
    /// Caixas que ficariam sem cadeia se ele fosse retirado (P4-4).
    pub dependentes: Vec<String>,
    /// O nome da aprovação assinada mais recente dele (C2-9: o caixa que só existe no
    /// registro, aprovado e ainda sem troca, também aparece na lista).
    pub nome: String,
    /// C2R2-5 (R2 da 2C): quando ele tem **mais de uma** identidade autorizada
    /// (aprovações ao mesmo tempo em caixas diferentes), cada uma, para o gerente
    /// retirar a que não vale mais: (chave em hexadecimal, código de pedido, quem cai
    /// junto se ela sair — C2R3-8).
    pub identidades: Vec<(String, String, Vec<String>)>,
    /// C2R5-1 (R5 da 2C): só no primeiro caixa e com o registro cheio, as chaves dele já
    /// retiradas por outro caixa e ainda não pela raiz ("Liberar o lugar"), em hexadecimal.
    pub liberar_lugar: Vec<String>,
}

/// C2R5-2: quantos nomes a lista de caixas mostra em "quem cai junto" (o resto vira "e
/// mais N caixas").
pub const QUEM_CAI_JUNTO_NA_LISTA: usize = 30;

/// T012: a situação no registro de cada `dispositivo` desta loja, só pelo banco.
pub(super) fn chaves_dos_caixas(conn: &Connection) -> Resultado<HashMap<String, ChaveDoCaixa>> {
    let mut m = HashMap::new();
    let Some((loja, raiz)) = loja_e_raiz(conn)? else { return Ok(m) };
    // C2R5-2 (R5 da 2C): o registro conferido uma vez só; quem cai junto com cada caixa
    // sai de uma conta só (antes: o registro inteiro conferido de novo para cada caixa).
    let reg = aprovados::RegistroConferido::carregar(conn, &raiz, &loja)?;
    let caem = reg.quem_cai_junto(QUEM_CAI_JUNTO_NA_LISTA);
    let nomes_que_caem = |k: &Chave| -> Vec<String> {
        let Some((lista, total)) = caem.get(k) else { return Vec::new() };
        let mut v: Vec<String> = lista.iter().map(|a| a.nome.clone()).collect();
        // C2R6-1: "e mais 1 caixa", no singular.
        match total.saturating_sub(v.len()) {
            0 => {}
            1 => v.push("e mais 1 caixa".to_string()),
            n => v.push(format!("e mais {n} caixas")),
        }
        v
    };
    let mut nomes: HashMap<String, (String, String)> = HashMap::new();
    let mut trocas: HashMap<Chave, Chave> = HashMap::new();
    for a in reg.aprovacoes() {
        let atual = nomes.entry(a.dispositivo.trim().to_string()).or_default();
        if a.aprovado_em > atual.0 {
            *atual = (a.aprovado_em.clone(), a.nome.clone());
        }
        trocas.insert(a.chave, a.chave_troca);
    }
    // C2R5-1: no primeiro caixa, com o registro cheio, o caixa já retirado por outro caixa
    // pode ser retirado também pela raiz, para o lugar voltar.
    let sou_raiz = config::ler(conn, "p4_chave_publica")?.is_some_and(|h| h.trim() == hex(&raiz));
    let pode_liberar = sou_raiz && reg.cheio();
    for (disp, chaves) in reg.situacoes_por_dispositivo() {
        let nome = nomes.get(&disp).map(|(_, n)| n.clone()).unwrap_or_default();
        let autorizadas: Vec<Chave> =
            chaves.iter().filter(|(_, s)| *s == aprovados::Situacao::Autorizada).map(|(k, _)| *k).collect();
        let identidades = if autorizadas.len() > 1 {
            autorizadas
                .iter()
                .map(|k| {
                    let codigo =
                        trocas.get(k).map(|t| crate::troca::protocolo4::codigo_de_pedido(k, t)).unwrap_or_default();
                    (hex(k), codigo, nomes_que_caem(k))
                })
                .collect()
        } else {
            Vec::new()
        };
        let autorizada = autorizadas.first().copied();
        let retirada = chaves.iter().any(|(_, s)| *s == aprovados::Situacao::Retirada);
        let dependentes = autorizada.as_ref().map(nomes_que_caem).unwrap_or_default();
        let liberar_lugar = if pode_liberar {
            chaves
                .iter()
                .filter(|(k, s)| *s == aprovados::Situacao::Retirada && !reg.retirada_pela_raiz(k))
                .map(|(k, _)| hex(k))
                .collect()
        } else {
            Vec::new()
        };
        m.insert(
            disp,
            ChaveDoCaixa {
                tem_chave: autorizada.is_some(),
                raiz: autorizada == Some(raiz),
                retirado_pela_chave: retirada && autorizada.is_none(),
                dependentes,
                nome,
                identidades,
                liberar_lugar,
            },
        );
    }
    Ok(m)
}

/// T012 (DT-P4-31, DT-P4-36): a situação do protocolo 4 neste caixa, só pelo
/// banco (nunca abre a identidade: pode ser lida na linha da tela).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SituacaoP4 {
    /// A raiz da loja está fixada aqui: este caixa fala o protocolo 4.
    pub tem_raiz: bool,
    /// Este caixa é aprovado na loja (a chave dele está autorizada no registro).
    pub aprovado: bool,
    /// Este caixa é a raiz da loja.
    pub raiz: bool,
    /// C2R2-1: a chave deste caixa foi retirada da loja (para voltar: identidade nova
    /// e convite novo).
    pub retirado: bool,
    /// Até quando os caixas antigos (protocolo 3) ainda trocam dados; `None` = recusados.
    pub convivencia_ate: Option<String>,
    /// A mesma data no horário daqui ("01/12/2026 15:42"); vazia quando recusados.
    pub convivencia_ate_texto: String,
    /// O prazo máximo da convivência (dias).
    pub convivencia_maxima_dias: i64,
    /// Convites ativos (não usados, dentro da validade) nesta loja.
    pub convites_ativos: usize,
    /// O segredo da identidade fica no cofre do sistema; `None` = ainda não se sabe.
    pub cofre_sistema: Option<bool>,
}

/// A situação do protocolo 4 neste caixa (veja [`SituacaoP4`]).
pub fn situacao(conn: &Connection) -> Resultado<SituacaoP4> {
    let minha = config::ler(conn, "p4_chave_publica")?.and_then(|h| crate::troca::protocolo4::de_hex::<32>(h.trim()));
    let cofre_sistema = config::ler(conn, "p4_cofre_sistema")?.and_then(|v| match v.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    });
    let convivencia_ate = convivencia_p3_ate(conn)?;
    let convivencia_ate_texto = match &convivencia_ate {
        Some(iso) => conn
            .query_row("SELECT strftime('%d/%m/%Y %H:%M', ?1, 'localtime')", [iso], |l| l.get::<_, Option<String>>(0))?
            .unwrap_or_else(|| iso.clone()),
        None => String::new(),
    };
    let mut s = SituacaoP4 {
        tem_raiz: tem_raiz_fixada(conn),
        convivencia_ate,
        convivencia_ate_texto,
        convivencia_maxima_dias: CONVIVENCIA_MAXIMA_DIAS,
        cofre_sistema,
        ..SituacaoP4::default()
    };
    // C2R2-6 (R2 da 2C): um registro estranho (por exemplo, duas raízes sem pino)
    // não derruba a leitura: o estado do P4 fica sem o resto, e o motivo vai para o
    // registro técnico (o Início e Configurações continuam abrindo).
    let p4 = (|| -> Resultado<()> {
        if let Some((loja, raiz)) = loja_e_raiz(conn)? {
            if let Some(k) = minha {
                let av = aprovados::avaliar(conn, &raiz, &loja)?;
                s.aprovado = av.situacao(&k) == aprovados::Situacao::Autorizada;
                s.retirado = av.situacao(&k) == aprovados::Situacao::Retirada;
                s.raiz = s.aprovado && k == raiz;
            }
            s.convites_ativos = cofre::convites_validos_agora(conn, &loja, &crate::tempo::agora())?
                .iter()
                .filter(|c| c.usado_em.is_none() && c.segredo_normalizado.is_some())
                .count();
        }
        Ok(())
    })();
    if let Err(e) = p4 {
        anotar_sem_repetir(&format!("troca de dados (protocolo 4): situação do registro: {e}"));
    }
    Ok(s)
}

/// T013 (DT-P4-41): este caixa é da loja (tem usuários e a troca ligada), não é
/// aprovado no protocolo 4 e ouviu há pouco o anúncio assinado de um caixa
/// atualizado da mesma loja: ele precisa da aprovação do gerente (convite) para
/// trocar dados com os caixas atualizados. Só lê o banco e a memória do motor.
pub fn precisa_de_aprovacao(conn: &Connection) -> Resultado<bool> {
    let s = situacao(conn)?;
    if !ajustes(conn)?.ligado || crate::usuarios::quantidade(conn)? == 0 || s.aprovado || s.retirado {
        return Ok(false);
    }
    let loja = crate::loja::id(conn)?;
    let Some(caminho) = conn.path().filter(|c| !c.is_empty()).map(PathBuf::from) else { return Ok(false) };
    Ok(!loja.is_empty() && !ouvidos(&caminho, &loja).is_empty())
}

/// T012 (DT-P4-31): a identidade deste caixa, para a tela.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentidadeNaTela {
    /// O código que o gerente confere (12 caracteres, ex.: `4F2K-9QXA-7MPC`).
    pub codigo_pedido: String,
    /// O segredo da identidade fica no cofre do sistema (senão, na reserva local, com aviso).
    pub cofre_sistema: bool,
    /// Este caixa já é aprovado na loja.
    pub aprovado: bool,
}

/// T012 (DT-P4-31): a identidade deste caixa para a tela. Abre a identidade (custa
/// um Argon2 e o cofre): nunca na linha da tela, só pelo comando demorado
/// (DT-P4-30). Cria a identidade se ainda não existe (o caixa novo precisa do
/// código para pedir a entrada).
pub fn identidade_para_a_tela(conn: &Connection, caminho: &Path) -> Resultado<IdentidadeNaTela> {
    let pasta = caminho.parent().ok_or_else(|| Erro::Interno("banco sem pasta".into()))?;
    let id = cofre::identidade_do_processo(pasta, &modo())?;
    let aprovado = match loja_e_raiz(conn)? {
        Some((loja, raiz)) => aprovados::autorizada(conn, &raiz, &loja, &id.identidade.publica())?,
        None => false,
    };
    Ok(IdentidadeNaTela { codigo_pedido: id.identidade.codigo_de_pedido(), cofre_sistema: id.cofre_sistema, aprovado })
}

/// T013 (DT-P4-42): o erro ao abrir a identidade, para a tela. Quando o arquivo
/// dela existe e não abre (o segredo não veio do cofre, ou não confere), o campo
/// "identidade" deixa a tela oferecer uma identidade nova, com o gerente. A trava
/// ocupada e os erros internos seguem como estão (não são caso de identidade nova).
pub fn erro_da_identidade(caminho: &Path, e: Erro) -> Erro {
    let existe =
        caminho.parent().and_then(|pasta| cofre::caminho_identidade(pasta, &modo()).ok()).is_some_and(|c| c.exists());
    match e {
        Erro::Validacao { campo: None, mensagem } if existe && mensagem != cofre::TEXTO_IDENTIDADE_OCUPADA => {
            Erro::campo("identidade", mensagem)
        }
        outro => outro,
    }
}

/// T013 (DT-P4-42, N-04): o gerente gera uma identidade nova para este caixa,
/// **só** quando a atual não abre (a função recusa a que abre). A antiga fica
/// arquivada com o segredo dela; a chave anotada passa a ser a nova; a auditoria
/// guarda quem fez. Depois, este caixa precisa de uma aprovação nova (convite) — e,
/// se a chave antiga estava aprovada, o gerente a retira num caixa aprovado.
pub fn gerar_identidade_nova(
    conn: &Connection,
    caminho: &Path,
    gerente_id: &str,
    confirmar_raiz: bool,
    confirmar_retirado: bool,
) -> Resultado<IdentidadeNaTela> {
    if gerente_id.trim().is_empty() {
        return Err(Erro::validacao("Só o gerente gera uma identidade nova para o caixa."));
    }
    let s = situacao(conn)?;
    // C2R2-2 (R2 da 2C): na raiz, a identidade nova tira da loja o caixa que aprovou
    // todos: só com a confirmação reforçada (a tela diz como a loja segue).
    if s.raiz && !confirmar_raiz {
        return Err(Erro::campo("confirmarRaiz", TEXTO_IDENTIDADE_NOVA_NA_RAIZ));
    }
    let pasta = caminho.parent().ok_or_else(|| Erro::Interno("banco sem pasta".into()))?;
    // C2R2-1: a chave retirada abre, mas não serve mais — a identidade nova vale
    // também para ela; nos outros casos, só quando a identidade não abre (N-04).
    // O caixa retirado não fica sabendo (os outros recusam a conexão dele): o gerente
    // pode dizer que ele foi retirado (`confirmar_retirado`); a antiga fica arquivada.
    let id = if s.retirado || confirmar_retirado {
        cofre::regerar_identidade_retirada(pasta, &modo())?
    } else {
        cofre::regerar_identidade_local(pasta, &modo())?
    };
    let nova = IdentidadeNaTela {
        codigo_pedido: id.identidade.codigo_de_pedido(),
        cofre_sistema: id.cofre_sistema,
        aprovado: false,
    };
    config::gravar(conn, "p4_chave_publica", &hex(&id.identidade.publica()))?;
    config::gravar(conn, "p4_cofre_sistema", if id.cofre_sistema { "1" } else { "0" })?;
    let disp = config::ler(conn, "dispositivo_id")?.unwrap_or_default();
    crate::auditoria::registrar(
        conn,
        &disp,
        Some(gerente_id),
        "p4_identidade_nova",
        "caixa",
        &disp,
        &[format!("Código novo deste caixa: {}", nova.codigo_pedido)],
    )?;
    Ok(nova)
}

/// Texto para a tela (C2R2-2): a identidade nova no primeiro caixa da loja.
pub const TEXTO_IDENTIDADE_NOVA_NA_RAIZ: &str = "Este é o primeiro caixa da loja: foi ele que aprovou os outros. Com \
     a identidade nova, ele precisa ser aprovado por outro caixa aprovado da loja (com um convite). Se a loja só tem \
     este caixa, chame o suporte antes: a identidade antiga fica guardada nesta pasta e pode ser recuperada.";

/// Texto para a tela: o convite precisa da troca ligada.
pub const TEXTO_CONVITE_SEM_TROCA: &str = "Ligue a troca de dados deste caixa antes de aprovar um caixa novo: é \
     por ela que o caixa novo vai entrar.";

/// T012 (DT-P4-32): o gerente gera, neste caixa aprovado, o convite de uso único
/// para o caixa do `codigo_pedido` (10 minutos). O QR leva o endereço deste caixa
/// (`endereco`, "IP:porta"). Recusa com a troca desligada e com este caixa ainda
/// não aprovado. A autenticação do gerente é da camada de cima (classe do comando).
pub fn gerar_convite(
    conn: &Connection,
    caminho: &Path,
    codigo_pedido: &str,
    nome: &str,
    gerente: &str,
    endereco: &str,
) -> Resultado<cofre::ConviteGerado> {
    if !troca_ativa(conn, &ajustes(conn)?) {
        return Err(Erro::validacao(TEXTO_CONVITE_SEM_TROCA));
    }
    let ctx = contexto_ou_motivo(conn, caminho)?;
    cofre::gerar_convite(conn, &ctx.identidade.identidade, &ctx.raiz, &ctx.loja, codigo_pedido, nome, gerente, endereco)
}

/// T012 (DT-P4-32): um convite ativo, para a tela do gerente. **Sem o segredo**:
/// quem perdeu o convite cancela e gera outro.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConviteNaTela {
    pub id: String,
    pub codigo_pedido: String,
    pub nome_caixa: String,
    pub gerente: String,
    pub expira_em: String,
}

/// Os convites ativos desta loja (não usados, dentro da validade), do que vence
/// primeiro ao último. Só lê o banco.
pub fn convites_ativos(conn: &Connection) -> Resultado<Vec<ConviteNaTela>> {
    let Some(loja) = loja_deste_caixa(conn) else { return Ok(Vec::new()) };
    let mut lista: Vec<ConviteNaTela> = cofre::convites_validos_agora(conn, &loja, &crate::tempo::agora())?
        .into_iter()
        .filter(|c| c.usado_em.is_none() && c.segredo_normalizado.is_some())
        .map(|c| ConviteNaTela {
            id: c.id.clone(),
            codigo_pedido: c.codigo_pedido.clone(),
            nome_caixa: c.nome_caixa.clone(),
            gerente: c.gerente.clone(),
            expira_em: c.expira_em.clone(),
        })
        .collect();
    lista.sort_by(|a, b| a.expira_em.cmp(&b.expira_em));
    Ok(lista)
}

// S-10 (C2-2, R1 da 2C): o bloqueio de antes (`caixa_retirado:<dispositivo>`)
// continua valendo para o protocolo 3, mas **nunca** vira retirada assinada: ele
// pode ter chegado de um caixa antigo, só com a senha da rede (não diz quem o
// fez). O caixa com chave é retirado pela chave, num caixa atualizado
// (`retirar_caixa_por_chave`).

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
