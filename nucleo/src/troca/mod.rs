//! Troca de dados entre caixas: transporte, rede local, caminhos e identidade da loja.

pub mod caminhos;
pub mod loja;
/// Protocolo 4 (D1/R-01/E5): identidade criptográfica de cada caixa (etapa
/// 1), registro assinado de caixas aprovados e retirados (etapa 2) e anúncio
/// assinado (etapa 3). Ainda não ligado à troca de dados (etapas 4 a 7).
pub mod protocolo4;
pub mod rede_local;
pub mod sync;
