//! Configurações da loja, deste computador e das tabelas de preço.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::numeros;
use crate::{documento, texto};

/// Lê uma configuração.
pub fn ler(conn: &Connection, chave: &str) -> Resultado<Option<String>> {
    Ok(conn.query_row("SELECT valor FROM config WHERE chave = ?1", [chave], |l| l.get(0)).optional()?)
}

/// Grava (ou troca) uma configuração.
pub fn gravar(conn: &Connection, chave: &str, valor: &str) -> Resultado<()> {
    conn.execute(
        "INSERT INTO config (chave, valor) VALUES (?1, ?2)
         ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor",
        params![chave, valor],
    )?;
    Ok(())
}

/// Nome da loja (vazio se ainda não foi informado).
pub fn nome_loja(conn: &Connection) -> Resultado<String> {
    Ok(ler(conn, "loja_nome")?.unwrap_or_default())
}

/// Nome deste computador, como "Caixa 1".
pub fn nome_caixa(conn: &Connection) -> Resultado<String> {
    Ok(ler(conn, "caixa_nome")?.unwrap_or_else(|| "Caixa 1".to_string()))
}

/// Uma tabela de preço (Revenda, Tabela 2...).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabelaPreco {
    pub numero: i64,
    pub nome: String,
    /// Percentual de lucro sugerido para peças novas, já escrito ("100").
    pub lucro_padrao: String,
    #[serde(skip)]
    pub lucro_padrao_valor: Option<i64>,
}

/// As 5 tabelas de preço, em ordem.
pub fn tabelas_preco(conn: &Connection) -> Resultado<Vec<TabelaPreco>> {
    let mut consulta = conn.prepare_cached("SELECT numero, nome, lucro_padrao FROM tabela_preco ORDER BY numero")?;
    let linhas = consulta.query_map([], |l| {
        let lucro: Option<i64> = l.get(2)?;
        Ok(TabelaPreco {
            numero: l.get(0)?,
            nome: l.get(1)?,
            lucro_padrao: lucro.map(numeros::escrever_percentual).unwrap_or_default(),
            lucro_padrao_valor: lucro,
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// O que vem da tela de configurações.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigEntrada {
    pub loja: String,
    pub caixa: String,
    pub uf: String,
    pub cnpj: String,
    pub endereco: String,
    pub telefone: String,
    pub imprimir_direto: bool,
    pub tabelas: Vec<TabelaEntrada>,
    /// Crediário: multa e juros ao mês por atraso ("2" = 2%).
    pub crediario_multa: String,
    pub crediario_juros: String,
    /// Dias de atraso sem multa nem juros.
    pub crediario_carencia: String,
    /// Orçamento: validade padrão em dias.
    pub orcamento_validade: String,
    /// Impressão: texto no pé do comprovante e do orçamento.
    pub rodape_venda: String,
    pub rodape_orcamento: String,
    pub rodape_os: String,
    /// Impressão: mostrar o local da peça (prateleira) no comprovante.
    pub imprimir_local: bool,
}

/// Validade padrão do orçamento, em dias.
pub const VALIDADE_PADRAO: i64 = 7;

/// Validade do orçamento escolhida nas configurações (em dias).
pub fn validade_orcamento(conn: &Connection) -> Resultado<i64> {
    Ok(ler(conn, "orcamento_validade")?
        .and_then(|v| v.parse().ok())
        .filter(|v| (1..=365).contains(v))
        .unwrap_or(VALIDADE_PADRAO))
}

/// Liga/desliga gravado como "1"/"0".
pub fn ligado(conn: &Connection, chave: &str) -> Resultado<bool> {
    Ok(ler(conn, chave)?.as_deref() == Some("1"))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TabelaEntrada {
    pub numero: i64,
    pub nome: String,
    pub lucro_padrao: String,
}

/// Confere e grava as configurações.
pub fn salvar(conn: &Connection, entrada: &ConfigEntrada) -> Resultado<()> {
    let loja = texto::limpar(&entrada.loja);
    let caixa = texto::limpar(&entrada.caixa);
    if texto::tamanho(&loja) > 80 {
        return Err(Erro::campo("loja", "O nome da loja pode ter no máximo 80 letras."));
    }
    validar_nome_caixa(&caixa)?;
    let uf = entrada.uf.trim().to_uppercase();
    if !uf.is_empty() && !documento::UFS.contains(&uf.as_str()) {
        return Err(Erro::campo("uf", "Escolha o estado da loja."));
    }
    let cnpj = documento::limpar(&entrada.cnpj);
    if !cnpj.is_empty() && !documento::cnpj_valido(&cnpj) && !documento::cpf_valido(&cnpj) {
        return Err(Erro::campo("cnpj", "CNPJ inválido. Confira os números e as letras."));
    }

    let mut nomes_usados: Vec<String> = Vec::new();
    let mut tabelas = Vec::new();
    for t in &entrada.tabelas {
        if !(1..=5).contains(&t.numero) {
            return Err(Erro::Interno(format!("tabela de preço {} não existe", t.numero)));
        }
        let campo_nome = format!("tabela{}Nome", t.numero);
        let campo_lucro = format!("tabela{}Lucro", t.numero);
        let nome = texto::limpar(&t.nome);
        if nome.is_empty() {
            return Err(Erro::campo(&campo_nome, "Dê um nome para a tabela de preço."));
        }
        if texto::tamanho(&nome) > 30 {
            return Err(Erro::campo(&campo_nome, "Use no máximo 30 letras."));
        }
        let normalizado = texto::normalizar(&nome);
        if nomes_usados.contains(&normalizado) {
            return Err(Erro::campo(&campo_nome, "Já existe outra tabela com esse nome."));
        }
        nomes_usados.push(normalizado);
        let lucro = numeros::ler_percentual(&t.lucro_padrao).map_err(|m| Erro::campo(&campo_lucro, m))?;
        if let Some(l) = lucro {
            if !(0..=1_000_000).contains(&l) {
                return Err(Erro::campo(&campo_lucro, "Use um percentual entre 0 e 10.000."));
            }
        }
        tabelas.push((t.numero, nome, lucro));
    }

    gravar(conn, "loja_nome", &loja)?;
    gravar(conn, "caixa_nome", &caixa)?;
    gravar(conn, "loja_uf", &uf)?;
    gravar(conn, "loja_cnpj", &cnpj)?;
    let endereco: String = texto::limpar(&entrada.endereco).chars().take(150).collect();
    let telefone: String = texto::limpar(&entrada.telefone).chars().take(40).collect();
    gravar(conn, "loja_endereco", &endereco)?;
    gravar(conn, "loja_telefone", &telefone)?;
    gravar(conn, "imprimir_direto", if entrada.imprimir_direto { "1" } else { "0" })?;
    gravar(conn, "imprimir_local", if entrada.imprimir_local { "1" } else { "0" })?;
    let validade = entrada.orcamento_validade.trim();
    let dias: i64 = if validade.is_empty() {
        VALIDADE_PADRAO
    } else {
        validade
            .parse()
            .ok()
            .filter(|v| (1..=365).contains(v))
            .ok_or_else(|| Erro::campo("orcamentoValidade", "Use de 1 a 365 dias."))?
    };
    gravar(conn, "orcamento_validade", &dias.to_string())?;
    for (chave, campo, texto_rodape) in [
        ("rodape_venda", "rodapeVenda", &entrada.rodape_venda),
        ("rodape_orcamento", "rodapeOrcamento", &entrada.rodape_orcamento),
        ("rodape_os", "rodapeOs", &entrada.rodape_os),
    ] {
        let t = texto_rodape.trim();
        if texto::tamanho(t) > 300 {
            return Err(Erro::campo(campo, "Use no máximo 300 letras."));
        }
        gravar(conn, chave, t)?;
    }
    for (numero, nome, lucro) in tabelas {
        conn.execute(
            "UPDATE tabela_preco SET nome = ?2, lucro_padrao = ?3 WHERE numero = ?1",
            params![numero, nome, lucro],
        )?;
    }
    Ok(())
}

/// O nome do caixa é obrigatório e curto.
pub fn validar_nome_caixa(caixa: &str) -> Resultado<()> {
    if caixa.is_empty() {
        return Err(Erro::campo("caixa", "Dê um nome para este computador, como Caixa 1."));
    }
    if texto::tamanho(caixa) > 30 {
        return Err(Erro::campo("caixa", "Use no máximo 30 letras."));
    }
    Ok(())
}
