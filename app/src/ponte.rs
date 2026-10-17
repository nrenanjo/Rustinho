//! Ponte entre as telas (QML) e o núcleo em Rust.
//!
//! As telas chamam `Sistema.chamar("comando", "{...json...}")` e recebem a
//! resposta em JSON. Toda a lógica fica no núcleo (`rustinho_nucleo`).

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
    }
}

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use rustinho_nucleo::Servico;

/// Guarda o serviço do núcleo (aberto na primeira chamada).
#[derive(Default)]
pub struct SistemaRust {
    servico: Option<Result<Servico, String>>,
}

impl qobject::Sistema {
    fn servico(self: Pin<&mut Self>) -> &mut Result<Servico, String> {
        let estado = self.rust_mut().get_mut();
        estado.servico.get_or_insert_with(|| {
            let caminho = rustinho_nucleo::caminho_padrao();
            Servico::abrir(&caminho)
                .map_err(|e| format!("Não consegui abrir o banco de dados em {}. {e}", caminho.display()))
        })
    }

    pub fn arquivo_local(&self, endereco: &QString) -> QString {
        qobject::rustinho_arquivo_local(endereco)
    }

    pub fn chamar(self: Pin<&mut Self>, comando: &QString, dados: &QString) -> QString {
        let resposta = match self.servico() {
            Ok(s) => s.chamar(&comando.to_string(), &dados.to_string()),
            Err(mensagem) => rustinho_nucleo::api::resposta_de_erro(mensagem),
        };
        QString::from(&resposta)
    }

    pub fn imprimir(self: Pin<&mut Self>, tipo: &QString, id: &QString) -> QString {
        let servico = match self.servico() {
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
