//! O-3 (auditoria da 0.9.15; R-05 da auditoria da 0.9.14): supervisão das
//! tarefas da troca de dados.
//!
//! Antes, um erro interno grave (pânico) numa das três tarefas do motor
//! (servidor, anúncio na rede, puxada) matava a tarefa: a troca de dados
//! parava até fechar e abrir o programa, sem aviso nenhum.
//!
//! Agora cada tarefa roda dentro de `supervisionar`:
//! - o pânico é capturado; o registro anota qual tarefa caiu (o gancho de
//!   pânico do `registro` já anota o arquivo e a linha, sem dados);
//! - a tarefa volta sozinha depois de uma espera que dobra a cada queda
//!   (até um minuto), e uma só por vez (a nova só começa depois que a que
//!   caiu terminou: é a mesma linha de execução);
//! - RVI-02: falhar ao começar (porta ocupada) ou terminar sem ninguém pedir
//!   conta como queda, igual ao pânico, com o motivo guardado;
//! - a queda número `QUEDAS_MAXIMAS` dentro de `JANELA` para a tarefa de vez
//!   (sem ciclo sem fim), com aviso crítico no Início: no programa, 5 quedas
//!   em 10 minutos (4 voltas);
//! - o lote que estava sendo aplicado foi desfeito pela própria transação
//!   (veja `sync::aplicar_de`): nada fica pela metade e as pendências
//!   guardadas continuam guardadas.
//!
//! A situação fica na memória deste programa (por banco), para o aviso do
//! Início e para a tela da troca de dados não dizer que está tudo bem com o
//! motor parado.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(test))]
pub const QUEDAS_MAXIMAS: usize = 5;
#[cfg(test)]
pub const QUEDAS_MAXIMAS: usize = 3;
#[cfg(not(test))]
const JANELA: Duration = Duration::from_secs(10 * 60);
#[cfg(test)]
const JANELA: Duration = Duration::from_secs(60);
#[cfg(not(test))]
const PRIMEIRA_ESPERA: Duration = Duration::from_secs(2);
#[cfg(test)]
const PRIMEIRA_ESPERA: Duration = Duration::from_millis(100);
const ESPERA_MAXIMA: Duration = Duration::from_secs(60);
/// Depois de quanto tempo sem cair o aviso "voltou sozinha" sai.
const AVISO_DA_QUEDA: Duration = Duration::from_secs(30 * 60);

/// Situação de uma tarefa da troca de dados.
#[derive(Debug, Clone, Default)]
pub struct Tarefa {
    pub quedas: usize,
    pub ultima_queda: Option<Instant>,
    /// Parou de vez (quedas demais).
    pub parada: bool,
    /// Quantas cópias dela estão rodando agora (nunca mais que uma).
    pub rodando: usize,
    /// O maior número de cópias rodando ao mesmo tempo (para os testes).
    pub maximo_rodando: usize,
    /// Quantas vezes ela começou (a primeira e cada volta).
    pub inicios: usize,
    /// O motivo da última falha (para a tela; sem dados da loja).
    pub motivo: Option<String>,
    /// Falhou e está esperando para tentar de novo (não está rodando).
    pub esperando: bool,
}

fn mapa() -> &'static Mutex<HashMap<(PathBuf, &'static str), Tarefa>> {
    static M: OnceLock<Mutex<HashMap<(PathBuf, &'static str), Tarefa>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn mexer<R>(banco: &Path, nome: &'static str, f: impl FnOnce(&mut Tarefa) -> R) -> R {
    let mut m = mapa().lock().unwrap_or_else(|e| e.into_inner());
    f(m.entry((banco.to_path_buf(), nome)).or_default())
}

/// As tarefas da troca de dados deste banco, com a situação de cada uma.
pub fn tarefas(banco: &Path) -> Vec<(&'static str, Tarefa)> {
    let m = mapa().lock().unwrap_or_else(|e| e.into_inner());
    let mut v: Vec<_> = m.iter().filter(|((b, _), _)| b == banco).map(|((_, n), t)| (*n, t.clone())).collect();
    v.sort_by_key(|(n, _)| *n);
    v
}

/// Situação das tarefas da troca de dados, para o aviso do Início e para a
/// tela da troca de dados. Cada uma vem com o motivo da última falha.
#[derive(Debug, Clone, Default)]
pub struct Situacao {
    /// Pararam de vez (falharam demais).
    pub paradas: Vec<(&'static str, String)>,
    /// Falharam e estão esperando para tentar de novo: não estão rodando.
    pub tentando: Vec<(&'static str, String)>,
    /// Falharam há pouco e voltaram a rodar.
    pub voltaram: Vec<(&'static str, String)>,
}

impl Situacao {
    /// Alguma tarefa não está rodando agora por causa de uma falha.
    pub fn com_falha(&self) -> bool {
        !self.paradas.is_empty() || !self.tentando.is_empty()
    }
}

pub fn situacao(banco: &Path) -> Situacao {
    let mut s = Situacao::default();
    for (n, t) in tarefas(banco) {
        let motivo = t.motivo.clone().unwrap_or_else(|| "erro interno".into());
        if t.parada {
            s.paradas.push((n, motivo));
        } else if t.esperando {
            s.tentando.push((n, motivo));
        } else if t.ultima_queda.is_some_and(|q| q.elapsed() < AVISO_DA_QUEDA) {
            s.voltaram.push((n, motivo));
        }
    }
    s
}

/// O nome da tarefa para as pessoas.
pub fn nome_para_tela(nome: &str) -> &'static str {
    match nome {
        "servidor" => "responder aos outros caixas",
        "anuncio" => "achar os outros caixas na rede",
        _ => "buscar as alterações dos outros caixas",
    }
}

/// Só nos testes: a próxima volta da tarefa `nome` deste banco cai (pânico)
/// `vezes` vezes.
#[cfg(test)]
pub(crate) fn provocar_queda(banco: &Path, nome: &'static str, vezes: usize) {
    let mut m = provocadas().lock().unwrap_or_else(|e| e.into_inner());
    *m.entry((banco.to_path_buf(), nome)).or_default() += vezes;
}

#[cfg(test)]
fn provocadas() -> &'static Mutex<HashMap<(PathBuf, &'static str), usize>> {
    static M: OnceLock<Mutex<HashMap<(PathBuf, &'static str), usize>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Chamado pelas tarefas a cada volta do laço delas: nos testes, cai se pediram.
pub(crate) fn ponto_de_queda(banco: &Path, nome: &'static str) {
    #[cfg(test)]
    {
        let cair = {
            let mut m = provocadas().lock().unwrap_or_else(|e| e.into_inner());
            match m.get_mut(&(banco.to_path_buf(), nome)) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    true
                }
                _ => false,
            }
        };
        if cair {
            panic!("queda de teste da tarefa {nome}");
        }
    }
    let _ = (banco, nome);
}

/// Conta a cópia rodando (sai da conta mesmo se cair).
struct Rodando<'a> {
    banco: &'a Path,
    nome: &'static str,
}

impl Drop for Rodando<'_> {
    fn drop(&mut self) {
        mexer(self.banco, self.nome, |t| t.rodando = t.rodando.saturating_sub(1));
    }
}

/// Dorme aos pouquinhos, acordando logo se pedirem para parar.
fn cochilar(parar: &AtomicBool, tempo: Duration) {
    let fim = Instant::now() + tempo;
    while !parar.load(Ordering::SeqCst) && Instant::now() < fim {
        thread::sleep(Duration::from_millis(20));
    }
}

/// Roda a tarefa `nome` numa linha de execução própria, de novo depois de
/// cada falha (veja o começo deste arquivo).
///
/// `corpo` devolve `Ok(())` quando pedirem para parar e `Err(motivo)` quando
/// não consegue funcionar (como uma porta ocupada). RVI-02: tudo o que não for
/// "pediram para parar" é falha — o erro devolvido, o pânico e também a volta
/// sem erro com ninguém pedindo — e conta para o limite, com o motivo na
/// situação (para o Início e para a tela da troca de dados).
pub fn supervisionar(
    banco: PathBuf,
    nome: &'static str,
    parar: Arc<AtomicBool>,
    corpo: impl Fn() -> Result<(), String> + Send + 'static,
) -> thread::JoinHandle<()> {
    mexer(&banco, nome, |t| {
        t.parada = false;
        t.esperando = false;
    });
    thread::Builder::new()
        .name(format!("sync-{nome}"))
        .spawn(move || {
            let mut quedas: VecDeque<Instant> = VecDeque::new();
            let mut espera = PRIMEIRA_ESPERA;
            while !parar.load(Ordering::SeqCst) {
                mexer(&banco, nome, |t| {
                    t.rodando += 1;
                    t.maximo_rodando = t.maximo_rodando.max(t.rodando);
                    t.inicios += 1;
                    t.esperando = false;
                });
                let r = {
                    let _rodando = Rodando { banco: &banco, nome };
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(&corpo))
                };
                // Pediram para parar: não é falha, mesmo que tenha caído no caminho.
                if parar.load(Ordering::SeqCst) {
                    break;
                }
                let motivo = match r {
                    Ok(Err(motivo)) => motivo,
                    Ok(Ok(())) => "a tarefa terminou sem ninguém pedir".to_string(),
                    Err(_) => "erro interno".to_string(),
                };
                let agora = Instant::now();
                quedas.push_back(agora);
                while quedas.front().is_some_and(|q| agora.duration_since(*q) > JANELA) {
                    quedas.pop_front();
                }
                let total = mexer(&banco, nome, |t| {
                    t.quedas += 1;
                    t.ultima_queda = Some(agora);
                    t.motivo = Some(motivo.clone());
                    t.quedas
                });
                // RVI-02: a queda número `QUEDAS_MAXIMAS` dentro da janela já
                // para (antes, só a seguinte).
                if quedas.len() >= QUEDAS_MAXIMAS {
                    mexer(&banco, nome, |t| t.parada = true);
                    crate::registro::anotar(&format!(
                        "troca de dados: a tarefa \"{nome}\" falhou {} vezes em pouco tempo e ficou parada ({motivo}); \
                         feche e abra o Rustinho; se continuar, chame o suporte",
                        quedas.len()
                    ));
                    break;
                }
                crate::registro::anotar(&format!(
                    "troca de dados: a tarefa \"{nome}\" parou ({motivo}; falha {total}); tenta de novo em {} s",
                    espera.as_secs_f32()
                ));
                mexer(&banco, nome, |t| t.esperando = true);
                cochilar(&parar, espera);
                mexer(&banco, nome, |t| t.esperando = false);
                espera = (espera * 2).min(ESPERA_MAXIMA);
            }
            mexer(&banco, nome, |t| t.esperando = false);
        })
        .expect("criar a tarefa da troca de dados")
}
