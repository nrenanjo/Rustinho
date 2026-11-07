//! Base do Rustinho: o que todo o resto usa e que não conhece nenhuma regra
//! da loja (modularização, etapa 1).
//!
//! - [`erro`]: o erro do Rustinho, com as mensagens prontas para a tela;
//! - [`numeros`]: dinheiro, percentuais e quantidades no jeito brasileiro;
//! - [`texto`]: texto para busca (sem acentos, minúsculas, códigos compactos);
//! - [`documento`]: CPF, CNPJ, CEP e placa;
//! - [`gtin`]: dígito verificador do código de barras;
//! - [`VERSAO`]: a versão do programa.
//!
//! A seta é só para baixo: a base não conhece o núcleo, e o núcleo reexporta
//! tudo nos mesmos caminhos de antes (`rustinho_nucleo::texto`,
//! `crate::erro::Erro`, `rustinho_nucleo::VERSAO`...), então quem chama não
//! muda. Nada aqui abre banco nem faz SQL; do SQLite só aparece o tipo do erro
//! (`erro.rs`).

pub mod documento;
pub mod erro;
pub mod gtin;
pub mod numeros;
pub mod texto;

/// Versão do Rustinho. Fica aqui, embaixo de tudo, para os módulos que gravam
/// a versão (registro, backup) não dependerem do serviço (`api`). É a mesma
/// versão do workspace que o núcleo usava (`version.workspace`).
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");
