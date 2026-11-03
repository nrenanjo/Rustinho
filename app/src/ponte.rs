//! Ponte entre as telas (QML) e o núcleo em Rust.
//!
//! As telas chamam `Sistema.chamar("comando", "{...json...}")` e recebem a
//! resposta em JSON. Toda a lógica fica no núcleo (`rustinho_nucleo`).
//!
//! Funções demoradas (0.9.15): `Sistema.chamarDemorado(...)` roda o comando
//! numa tarefa à parte e avisa no sinal `terminou(pedido, resposta)`; a tela
//! mostra a espera com a barra andando. Enquanto isso, qualquer outra
//! chamada responde "aguarde" na hora, sem esperar (a janela nunca trava e
//! nada roda duas vezes). A abertura do banco (que pode atualizar a
//! estrutura) também roda assim: `abrirEmSegundoPlano()` e o sinal `aberto`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("rustinho/cpp/pdf.h");
        fn rustinho_gerar_pdf(html: &QString, caminho: &QString) -> bool;
        fn rustinho_abrir_arquivo(caminho: &QString) -> bool;
        fn rustinho_gerar_etiquetas(json: &QString, caminho: &QString) -> bool;
        fn rustinho_imprimir_html(html: &QString) -> bool;
        fn rustinho_imprimir_etiquetas(json: &QString) -> bool;

        include!("rustinho/cpp/janela.h");
        fn rustinho_preparar_janela();
        fn rustinho_arquivo_local(endereco: &QString) -> QString;
        fn rustinho_identidade_do_computador() -> QString;
    }

    extern "RustQt" {
        /// Objeto único, visível em todas as telas como `Sistema`.
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Sistema = super::SistemaRust;

        /// Executa um comando do núcleo e devolve a resposta em JSON.
        #[qinvokable]
        fn chamar(self: Pin<&mut Self>, comando: &QString, dados: &QString) -> QString;

        /// Gera o PDF A4 (venda, orçamento ou caixa) e abre ou imprime.
        /// Devolve JSON: {ok, caminho, impresso} ou {ok: false, erro}.
        #[qinvokable]
        fn imprimir(self: Pin<&mut Self>, tipo: &QString, id: &QString) -> QString;

        /// Arquivo escolhido na tela → caminho que o núcleo consegue abrir.
        /// No Android, o "content://" vira uma cópia na pasta do app.
        #[qinvokable]
        fn arquivo_local(self: &Self, endereco: &QString) -> QString;

        /// Roda um comando demorado numa tarefa à parte. Devolve o número do
        /// pedido (0 = não começou: já há outro rodando).
        #[qinvokable]
        fn chamar_demorado(self: Pin<&mut Self>, comando: &QString, dados: &QString) -> i32;

        /// Um comando demorado terminou.
        #[qsignal]
        fn terminou(self: Pin<&mut Self>, pedido: i32, resposta: QString);

        /// Há um comando demorado (ou a abertura do banco) rodando.
        #[qinvokable]
        fn ocupado(self: &Self) -> bool;

        /// O que está sendo feito agora (para a tela de espera).
        #[qinvokable]
        fn fazendo(self: &Self) -> QString;

        /// Abre o banco numa tarefa à parte (pode atualizar a estrutura de um
        /// banco antigo, com cópia e conferência). Avisa em `aberto`.
        #[qinvokable]
        fn abrir_em_segundo_plano(self: Pin<&mut Self>);

        /// O banco abriu ("" = deu certo; senão, o erro).
        #[qsignal]
        fn aberto(self: Pin<&mut Self>, erro: QString);

        /// Depois de uma atualização da estrutura que falhou (o banco de
        /// antes já voltou): libera uma nova tentativa ao abrir de novo.
        #[qinvokable]
        fn tentar_atualizacao_de_novo(self: Pin<&mut Self>) -> bool;
    }

    impl cxx_qt::Threading for Sistema {}
}

use core::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use rustinho_nucleo::Servico;

type Guardado = Arc<Mutex<Option<Result<Servico, String>>>>;

/// Guarda o serviço do núcleo (aberto na primeira chamada ou em segundo plano).
#[derive(Default)]
pub struct SistemaRust {
    servico: Guardado,
    /// Um comando demorado (ou a abertura) está rodando.
    ocupado: Arc<AtomicBool>,
    pedidos: i32,
}

const AGUARDE: &str = "Aguarde: o Rustinho ainda está terminando a operação anterior.";

/// Abre o serviço do núcleo (só uma vez).
fn abrir_servico() -> Result<Servico, String> {
    // PEND-09: programa montado com a raiz de TESTE da NF-e não abre.
    if rustinho_nucleo::nfe_assinatura::ANCORAS_DE_TESTE_COMPILADAS {
        return Err("Este Rustinho foi montado com as âncoras de TESTE da NF-e e não pode ser usado.".into());
    }
    let caminho = rustinho_nucleo::caminho_padrao();
    Servico::abrir(&caminho).map_err(|e| format!("Não consegui abrir o banco de dados em {}. {e}", caminho.display()))
}

/// Antes de abrir o banco (na linha da janela): o núcleo compara a
/// identidade deste computador com a que o banco guardou (cópia do banco).
fn definir_computador_se_preciso(g: &Guardado) {
    if travar(g).is_none() {
        rustinho_nucleo::computador::definir(&qobject::rustinho_identidade_do_computador().to_string());
    }
}

fn travar(g: &Guardado) -> MutexGuard<'_, Option<Result<Servico, String>>> {
    g.lock().unwrap_or_else(|e| e.into_inner())
}

/// Roda `f` com o serviço (aberto agora, se ainda não estava).
fn com_servico<T>(g: &Guardado, f: impl FnOnce(&mut Servico) -> T, erro: impl FnOnce(&str) -> T) -> T {
    let mut s = travar(g);
    match s.get_or_insert_with(abrir_servico) {
        Ok(servico) => f(servico),
        Err(m) => erro(m),
    }
}

impl qobject::Sistema {
    fn guardado(&self) -> Guardado {
        self.rust().servico.clone()
    }

    pub fn arquivo_local(&self, endereco: &QString) -> QString {
        qobject::rustinho_arquivo_local(endereco)
    }

    pub fn ocupado(&self) -> bool {
        self.rust().ocupado.load(Ordering::SeqCst)
    }

    pub fn fazendo(&self) -> QString {
        let m = rustinho_nucleo::migracao::andamento();
        QString::from(&if m.is_empty() { rustinho_nucleo::backup::fazendo() } else { m })
    }

    pub fn chamar(self: Pin<&mut Self>, comando: &QString, dados: &QString) -> QString {
        if self.ocupado() {
            return QString::from(&rustinho_nucleo::api::resposta_de_erro(AGUARDE));
        }
        let (comando, dados) = (comando.to_string(), dados.to_string());
        definir_computador_se_preciso(&self.guardado());
        let resposta =
            com_servico(&self.guardado(), |s| s.chamar(&comando, &dados), rustinho_nucleo::api::resposta_de_erro);
        QString::from(&resposta)
    }

    pub fn chamar_demorado(mut self: Pin<&mut Self>, comando: &QString, dados: &QString) -> i32 {
        if self.rust().ocupado.swap(true, Ordering::SeqCst) {
            return 0;
        }
        let pedido = {
            let mut r = self.as_mut().rust_mut();
            r.pedidos = r.pedidos.wrapping_add(1).max(1);
            r.pedidos
        };
        let (comando, dados) = (comando.to_string(), dados.to_string());
        let guardado = self.guardado();
        let ocupado = self.rust().ocupado.clone();
        let linha = self.qt_thread();
        std::thread::spawn(move || {
            let resposta = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                com_servico(&guardado, |s| s.chamar(&comando, &dados), rustinho_nucleo::api::resposta_de_erro)
            }))
            .unwrap_or_else(|_| rustinho_nucleo::api::resposta_de_erro("Erro interno inesperado na operação."));
            ocupado.store(false, Ordering::SeqCst);
            let _ = linha.queue(move |obj| obj.terminou(pedido, QString::from(&resposta)));
        });
        pedido
    }

    pub fn abrir_em_segundo_plano(self: Pin<&mut Self>) {
        if self.rust().ocupado.swap(true, Ordering::SeqCst) {
            return;
        }
        // Antes de abrir o banco: o núcleo compara a identidade deste
        // computador com a que o banco guardou (cópia do banco). Lida aqui,
        // na linha da janela (vem do Qt).
        rustinho_nucleo::computador::definir(&qobject::rustinho_identidade_do_computador().to_string());
        let guardado = self.guardado();
        let ocupado = self.rust().ocupado.clone();
        let linha = self.qt_thread();
        std::thread::spawn(move || {
            let erro = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut s = travar(&guardado);
                match s.get_or_insert_with(abrir_servico) {
                    Ok(_) => String::new(),
                    Err(m) => m.clone(),
                }
            }))
            .unwrap_or_else(|_| "Erro interno inesperado ao abrir o banco.".to_string());
            ocupado.store(false, Ordering::SeqCst);
            let _ = linha.queue(move |obj| obj.aberto(QString::from(&erro)));
        });
    }

    pub fn tentar_atualizacao_de_novo(self: Pin<&mut Self>) -> bool {
        if self.ocupado() {
            return false;
        }
        let caminho = rustinho_nucleo::caminho_padrao();
        let liberou = rustinho_nucleo::migracao::liberar_nova_tentativa(&caminho).unwrap_or(false);
        if liberou {
            // O erro guardado sai: a próxima abertura tenta de novo.
            *travar(&self.guardado()) = None;
        }
        liberou
    }

    pub fn imprimir(self: Pin<&mut Self>, tipo: &QString, id: &QString) -> QString {
        if self.ocupado() {
            return QString::from(&rustinho_nucleo::api::resposta_de_erro(AGUARDE));
        }
        let guardado = self.guardado();
        definir_computador_se_preciso(&guardado);
        let mut guarda = travar(&guardado);
        let servico = match guarda.get_or_insert_with(abrir_servico) {
            Ok(s) => s,
            Err(m) => return QString::from(&rustinho_nucleo::api::resposta_de_erro(m)),
        };
        let pedido = serde_json::json!({ "tipo": tipo.to_string(), "id": id.to_string() }).to_string();
        let resposta: serde_json::Value =
            serde_json::from_str(&servico.chamar("impressao", &pedido)).unwrap_or_default();
        if resposta["ok"] != serde_json::Value::Bool(true) {
            return QString::from(&resposta.to_string());
        }
        let dados = &resposta["dados"];
        let caminho = dados["caminho"].as_str().unwrap_or_default();
        let direto = dados["direto"].as_bool().unwrap_or(false);
        let gerou = if dados["etiquetas"].is_object() {
            // Etiquetas: desenhadas direto no PDF (medidas da folha de etiquetas).
            qobject::rustinho_gerar_etiquetas(&QString::from(&dados["etiquetas"].to_string()), &QString::from(caminho))
        } else {
            let html = dados["html"].as_str().unwrap_or_default();
            qobject::rustinho_gerar_pdf(&QString::from(html), &QString::from(caminho))
        };
        if !gerou {
            return QString::from(&rustinho_nucleo::api::resposta_de_erro("Não consegui criar o PDF."));
        }
        // Impressão direta pela impressora padrão, se configurada: no Windows
        // pelo Qt; no Linux pelo CUPS ("lp").
        let impresso = direto && {
            if cfg!(windows) {
                if dados["etiquetas"].is_object() {
                    qobject::rustinho_imprimir_etiquetas(&QString::from(&dados["etiquetas"].to_string()))
                } else {
                    qobject::rustinho_imprimir_html(&QString::from(dados["html"].as_str().unwrap_or_default()))
                }
            } else {
                std::process::Command::new("lp").arg(caminho).output().map(|s| s.status.success()).unwrap_or(false)
            }
        };
        if !impresso {
            qobject::rustinho_abrir_arquivo(&QString::from(caminho));
        }
        let r = serde_json::json!({ "ok": true, "dados": { "caminho": caminho, "impresso": impresso } });
        QString::from(&r.to_string())
    }
}
