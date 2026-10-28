//! Núcleo do Rustinho: regras da loja e banco de dados, sem interface.
//!
//! A interface (Qt/QML) conversa com o núcleo só pelo [`Servico`], mandando
//! comandos com dados em JSON. Assim tudo aqui pode ser testado sem abrir
//! nenhuma janela (`cargo test`).

pub mod api;
#[cfg(test)]
#[path = "../tests/apoio/chave.rs"]
pub(crate) mod apoio_chave;
pub mod arquivos;
pub mod auditoria;
pub mod avisos;
pub mod backup;
pub mod banco;
pub mod busca;
pub mod caixa;
pub mod caminhos;
pub mod cheques;
pub mod codigo;
pub mod comissoes;
pub mod compras;
pub mod computador;
pub mod config;
pub mod devolucoes;
pub mod documento;
pub mod dre;
pub mod encomendas;
pub mod erro;
pub mod estoque;
pub mod etiquetas;
pub mod faixas;
pub mod financeiro;
pub mod fiscal;
pub mod gtin;
pub mod importar;
pub mod impressao;
pub mod loja;
pub mod menu;
pub mod migracao;
pub mod nfe_assinatura;
pub mod nfe_xml;
pub mod numeros;
pub mod oficina;
pub mod pagar;
pub mod painel;
pub mod pessoas;
pub mod portatil;
pub mod produtos;
pub mod receber;
pub mod rede_local;
pub mod registro;
pub mod reposicao;
pub mod senhas;
pub mod supervisao;
pub mod sync;
pub mod tempo;
#[cfg(test)]
pub(crate) mod teste_rede;
#[cfg(test)]
mod testes_0915;
#[cfg(test)]
mod testes_chave;
#[cfg(test)]
mod testes_loja;
#[cfg(test)]
mod testes_modo_seguro;
#[cfg(test)]
mod testes_r03;
#[cfg(test)]
mod testes_rvi03;
#[cfg(test)]
mod testes_supervisao;
pub mod texto;
pub mod unidades;
pub mod usuarios;
pub mod vendas;

pub use api::{Servico, VERSAO};
pub use banco::caminho_padrao;
pub use erro::{Erro, Resultado};
