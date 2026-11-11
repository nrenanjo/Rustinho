//! Núcleo do Rustinho: regras da loja e banco de dados, sem interface.
//!
//! A interface (Qt/QML) conversa com o núcleo só pelo [`Servico`], mandando
//! comandos com dados em JSON. Assim tudo aqui pode ser testado sem abrir
//! nenhuma janela (`cargo test`).

#[cfg(test)]
#[path = "../tests/apoio/chave.rs"]
pub(crate) mod apoio_chave;
#[cfg(test)]
pub(crate) mod teste_rede;
#[cfg(test)]
mod testes_0915;
#[cfg(test)]
mod testes_avisos;
#[cfg(test)]
mod testes_chave;
#[cfg(test)]
mod testes_estab;
#[cfg(test)]
mod testes_loja;
#[cfg(test)]
mod testes_modo_seguro;
#[cfg(test)]
mod testes_nav;
#[cfg(test)]
mod testes_r03;
#[cfg(test)]
mod testes_rvi03;
#[cfg(test)]
mod testes_supervisao;

pub mod cadastros;
pub mod catalogo;
pub mod compras;
pub mod dados;
pub mod integracoes;
pub mod operacao;
pub mod servico;
pub mod troca;

// Os módulos nos mesmos caminhos de antes das pastas de domínio (dev.3.3,
// passo 3.1): `crate::banco`, `rustinho_nucleo::backup`...
pub use cadastros::{pessoas, usuarios};
pub use catalogo::{busca, estoque, etiquetas, faixas, fiscal, produtos, unidades};
pub use compras::{importar, nfe_xml, reposicao};
pub use dados::{auditoria, banco, computador, config, migracao, portatil, registro, tempo};
pub use integracoes::{arquivos, backup, codigo, impressao, nfe_assinatura};
pub use operacao::{
    caixa, cheques, comissoes, devolucoes, dre, encomendas, financeiro, oficina, pagar, painel, receber, vendas,
};
pub use servico::{api, avisos, despacho, menu, requisitos, senhas, supervisao};
pub use troca::{caminhos, loja, rede_local, sync};

// Da base (crate `rustinho-base`, etapa 1 da modularização), nos mesmos
// caminhos de antes: `crate::erro::Erro`, `rustinho_nucleo::texto`...
pub use rustinho_base::{documento, erro, gtin, numeros, texto};

pub use api::Servico;
pub use banco::caminho_padrao;
pub use erro::{Erro, Resultado};
pub use rustinho_base::VERSAO;
