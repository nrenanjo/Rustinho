//! Núcleo do Rustinho: regras da loja e banco de dados, sem interface.
//!
//! A interface (Qt/QML) conversa com o núcleo só pelo [`Servico`], mandando
//! comandos com dados em JSON. Assim tudo aqui pode ser testado sem abrir
//! nenhuma janela (`cargo test`).

pub mod api;
pub mod arquivos;
pub mod auditoria;
pub mod avisos;
pub mod backup;
pub mod banco;
pub mod busca;
pub mod caixa;
pub mod cheques;
pub mod codigo;
pub mod comissoes;
pub mod compras;
pub mod config;
pub mod devolucoes;
pub mod documento;
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
pub mod menu;
pub mod nfe_xml;
pub mod numeros;
pub mod oficina;
pub mod pagar;
pub mod painel;
pub mod pessoas;
pub mod produtos;
pub mod receber;
pub mod registro;
pub mod reposicao;
pub mod sync;
pub mod tempo;
pub mod texto;
pub mod unidades;
pub mod usuarios;
pub mod vendas;

pub use api::{Servico, VERSAO};
pub use banco::caminho_padrao;
pub use erro::{Erro, Resultado};
