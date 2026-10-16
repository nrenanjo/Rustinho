//! Peças: cadastro completo (dados, preços, aplicações e equivalências).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::{auditoria, config, estoque, fiscal, gtin, numeros, tempo, texto, unidades};

/// Origem da mercadoria (tabela da NF-e), na ordem do código.
pub const ORIGENS: &[&str] = &[
    "0 · Nacional",
    "1 · Estrangeira, importação direta",
    "2 · Estrangeira, comprada no Brasil",
    "3 · Nacional, mais de 40% e até 70% importado",
    "4 · Nacional, processo produtivo básico",
    "5 · Nacional, até 40% importado",
    "6 · Estrangeira, importação direta, sem similar nacional",
    "7 · Estrangeira, comprada no Brasil, sem similar nacional",
    "8 · Nacional, mais de 70% importado",
];

const MAX_PRECO: i64 = 10_000_000_000; // R$ 100 milhões
const MAX_LINHAS: usize = 300;

// ---------------------------------------------------------------------------
// O que vem da tela (tudo como foi digitado) e o que volta para ela.
// ---------------------------------------------------------------------------

/// Ficha de uma peça, com os valores já escritos para os campos da tela.
/// A tela manda de volta este mesmo formato para salvar.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProdutoFicha {
    /// Vazio = peça nova.
    pub id: String,
    pub codigo: String,
    pub nome: String,
    pub marca: String,
    pub unidade: String,
    pub codigo_barras: String,
    pub codigo_original: String,
    pub categoria: String,
    pub ativo: bool,
    pub custo: String,
    pub precos: Vec<PrecoFicha>,
    /// Só para peça nova: quantidade que já existe na prateleira.
    pub estoque_inicial: String,
    pub estoque_minimo: String,
    pub local: String,
    pub ncm: String,
    pub cest: String,
    pub origem: i64,
    pub anp_codigo: String,
    pub anp_descricao: String,
    /// CSOSN escolhido à mão; vazio = automático.
    pub csosn: String,
    pub monofasico: bool,
    /// Peso em kg.
    pub peso: String,
    pub desconto_maximo: String,
    pub comissao: String,
    pub observacoes: String,
    pub aplicacoes: Vec<AplicacaoFicha>,
    pub equivalentes: Vec<EquivalenteFicha>,
    /// "produto" ou "servico" (serviço não tem estoque).
    pub tipo: String,
    /// Preço de oferta (troca o da primeira tabela) e até quando ("dd/mm/aaaa").
    pub oferta_preco: String,
    pub oferta_ate: String,
    /// O preço é digitado na hora da venda.
    pub preco_aberto: bool,
    /// Tamanho ou cor deste produto ("P", "Azul M"). Vazio = sem grade.
    pub variacao: String,
    /// IPI, PIS/Cofins, combustível e serviço (para a nota).
    pub fiscal: fiscal::FiscalPeca,

    // Só leitura (a tela não manda de volta).
    /// Os outros tamanhos e cores deste produto (com ele mesmo).
    #[serde(skip_deserializing)]
    pub grade_itens: Vec<ItemGrade>,
    /// A oferta vale hoje.
    #[serde(skip_deserializing)]
    pub oferta_valendo: bool,
    /// A unidade vende quebrado (KG, LT, MT...).
    #[serde(skip_deserializing)]
    pub fracionada: bool,
    #[serde(skip_deserializing)]
    pub estoque: String,
    #[serde(skip_deserializing)]
    pub estoque_valor: i64,
    #[serde(skip_deserializing)]
    pub criado_em: String,
    #[serde(skip_deserializing)]
    pub criado_por: String,
    #[serde(skip_deserializing)]
    pub atualizado_em: String,
    #[serde(skip_deserializing)]
    pub atualizado_por: String,
    #[serde(skip_deserializing)]
    pub historico: Vec<auditoria::Registro>,
    /// CSOSN e CFOP que a venda vai usar, com a explicação e o alerta.
    #[serde(skip_deserializing)]
    pub tributacao: Option<fiscal::Tributacao>,
    /// Fornecedores e códigos que eles usam para esta peça.
    #[serde(skip_deserializing)]
    pub fornecedores: Vec<CodigoFornecedor>,
}

/// Um tamanho ou cor da grade.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemGrade {
    pub id: String,
    pub variacao: String,
    pub codigo: String,
    pub estoque: String,
}

/// Código que um fornecedor usa para a peça.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodigoFornecedor {
    pub fornecedor: String,
    pub codigo: String,
    pub fator: String,
}

/// Preço de uma tabela.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PrecoFicha {
    pub tabela: i64,
    pub nome: String,
    pub lucro: String,
    pub preco: String,
}

/// Um carro em que a peça serve.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AplicacaoFicha {
    pub montadora: String,
    pub modelo: String,
    pub motor: String,
    pub ano_inicio: String,
    pub ano_fim: String,
    pub observacao: String,
}

/// A mesma peça em outra marca.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EquivalenteFicha {
    pub marca: String,
    pub codigo: String,
}

// ---------------------------------------------------------------------------
// Dados conferidos, do jeito que vão para o banco.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
struct Dados {
    codigo: String,
    nome: String,
    marca: String,
    unidade: String,
    codigo_barras: String,
    codigo_original: String,
    categoria: String,
    ativo: bool,
    custo: i64,
    /// (tabela, preço, lucro sobre o custo)
    precos: Vec<(i64, i64, Option<i64>)>,
    estoque_minimo: i64,
    local: String,
    ncm: String,
    cest: String,
    origem: i64,
    anp_codigo: String,
    anp_descricao: String,
    csosn: String,
    monofasico: bool,
    peso_gramas: i64,
    desconto_maximo: Option<i64>,
    comissao: Option<i64>,
    observacoes: String,
    aplicacoes: Vec<Aplicacao>,
    equivalentes: Vec<Equivalente>,
    tipo: String,
    oferta_preco: Option<i64>,
    /// "AAAA-MM-DD" ou vazio (sem prazo).
    oferta_ate: String,
    preco_aberto: bool,
    variacao: String,
    grade: String,
    fiscal: fiscal::FiscalPeca,
}

#[derive(Debug, Clone, PartialEq)]
struct Aplicacao {
    montadora: String,
    modelo: String,
    motor: String,
    ano_inicio: Option<i32>,
    ano_fim: Option<i32>,
    observacao: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Equivalente {
    marca: String,
    codigo: String,
}

// ---------------------------------------------------------------------------
// Conferência do que foi digitado.
// ---------------------------------------------------------------------------

fn texto_curto(valor: &str, campo: &str, rotulo: &str, maximo: usize) -> Resultado<String> {
    let v = texto::limpar(valor);
    if texto::tamanho(&v) > maximo {
        return Err(Erro::campo(campo, format!("{rotulo}: use no máximo {maximo} caracteres.")));
    }
    Ok(v)
}

fn so_digitos(valor: &str) -> String {
    valor.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn validar(f: &ProdutoFicha) -> Resultado<(Dados, i64)> {
    let nome = texto_curto(&f.nome, "nome", "Nome", 120)?;
    if nome.is_empty() {
        return Err(Erro::campo("nome", "Digite o nome da peça."));
    }
    let codigo = texto_curto(&f.codigo, "codigo", "Código", 60)?;
    let marca = texto_curto(&f.marca, "marca", "Marca", 60)?;

    let unidade = if f.unidade.trim().is_empty() { "UN".to_string() } else { unidades::conferir_sigla(&f.unidade)? };
    let tipo = match f.tipo.trim() {
        "" | "produto" => "produto",
        "servico" => "servico",
        _ => return Err(Erro::campo("tipo", "Escolha se é produto ou serviço.")),
    }
    .to_string();

    let codigo_barras: String = f.codigo_barras.chars().filter(|c| !c.is_whitespace()).collect();
    if !codigo_barras.is_empty() {
        if !codigo_barras.chars().all(|c| c.is_ascii_digit()) {
            return Err(Erro::campo("codigoBarras", "O código de barras só tem números."));
        }
        if !gtin::gtin_valido(&codigo_barras) {
            return Err(Erro::campo(
                "codigoBarras",
                "Código de barras inválido. Confira os números (o último dígito não confere).",
            ));
        }
    }

    let codigo_original = texto_curto(&f.codigo_original, "codigoOriginal", "Código original", 60)?;
    let categoria = texto_curto(&f.categoria, "categoria", "Categoria", 60)?;
    let local = texto_curto(&f.local, "local", "Local", 40)?;
    let observacoes = f.observacoes.trim().to_string();
    if texto::tamanho(&observacoes) > 2000 {
        return Err(Erro::campo("observacoes", "Observações: use no máximo 2000 caracteres."));
    }

    let custo = numeros::ler_dinheiro(&f.custo).map_err(|m| Erro::campo("custo", m))?.unwrap_or(0);
    if custo < 0 {
        return Err(Erro::campo("custo", "O custo não pode ser negativo."));
    }
    if custo > MAX_PRECO {
        return Err(Erro::campo("custo", "Custo alto demais."));
    }

    let mut precos = Vec::new();
    for tabela in 1..=5_i64 {
        let vazio = PrecoFicha::default();
        let p = f.precos.iter().find(|p| p.tabela == tabela).unwrap_or(&vazio);
        let campo_preco = format!("preco{tabela}");
        let campo_lucro = format!("lucro{tabela}");
        let preco = numeros::ler_dinheiro(&p.preco).map_err(|m| Erro::campo(&campo_preco, m))?;
        let lucro = numeros::ler_percentual(&p.lucro).map_err(|m| Erro::campo(&campo_lucro, m))?;
        let preco = match (preco, lucro) {
            (Some(valor), _) => Some(valor),
            (None, Some(l)) if custo > 0 => Some(numeros::preco_por_lucro(custo, l)),
            _ => None,
        };
        match preco {
            Some(valor) if valor <= 0 => {
                return Err(Erro::campo(&campo_preco, "O preço precisa ser maior que zero."));
            }
            Some(valor) if valor > MAX_PRECO => {
                return Err(Erro::campo(&campo_preco, "Preço alto demais."));
            }
            Some(valor) => precos.push((tabela, valor, numeros::lucro_por_preco(custo, valor))),
            None if tabela == 1 && !f.preco_aberto => {
                return Err(Erro::campo("preco1", "Informe o preço de revenda (ou marque \"preço na hora da venda\")."));
            }
            None => {}
        }
    }

    let estoque_inicial = numeros::ler_quantidade(&f.estoque_inicial)
        .map_err(|m| Erro::campo("estoqueInicial", m))?
        .unwrap_or(0);
    if estoque_inicial < 0 {
        return Err(Erro::campo("estoqueInicial", "O estoque inicial não pode ser negativo."));
    }
    let estoque_minimo = numeros::ler_quantidade(&f.estoque_minimo)
        .map_err(|m| Erro::campo("estoqueMinimo", m))?
        .unwrap_or(0);
    if estoque_minimo < 0 {
        return Err(Erro::campo("estoqueMinimo", "O estoque mínimo não pode ser negativo."));
    }

    let ncm = so_digitos(&f.ncm);
    if !ncm.is_empty() && ncm.len() != 8 {
        return Err(Erro::campo("ncm", "O NCM tem 8 números (ex.: 8482.20.10)."));
    }
    let cest = so_digitos(&f.cest);
    if !cest.is_empty() && cest.len() != 7 {
        return Err(Erro::campo("cest", "O CEST tem 7 números (ex.: 01.999.00)."));
    }
    if !(0..ORIGENS.len() as i64).contains(&f.origem) {
        return Err(Erro::campo("origem", "Escolha a origem da mercadoria."));
    }
    let anp_codigo = so_digitos(&f.anp_codigo);
    if !anp_codigo.is_empty() && anp_codigo.len() != 9 {
        return Err(Erro::campo("anpCodigo", "O código ANP tem 9 números."));
    }
    let anp_descricao = texto_curto(&f.anp_descricao, "anpDescricao", "Descrição ANP", 95)?;
    if anp_codigo.is_empty() && !anp_descricao.is_empty() {
        return Err(Erro::campo("anpCodigo", "Informe o código ANP."));
    }

    let csosn = f.csosn.trim().to_string();
    if !fiscal::CSOSNS.iter().any(|(c, _)| *c == csosn) {
        return Err(Erro::campo("csosn", "Escolha o CSOSN."));
    }
    let peso_gramas = numeros::ler_quantidade(&f.peso).map_err(|m| Erro::campo("peso", m))?.unwrap_or(0);
    if peso_gramas < 0 {
        return Err(Erro::campo("peso", "O peso não pode ser negativo."));
    }
    let percentual = |valor: &str, campo: &str| -> Resultado<Option<i64>> {
        let v = numeros::ler_percentual(valor).map_err(|m| Erro::campo(campo, m))?;
        if let Some(x) = v {
            if !(0..=10_000).contains(&x) {
                return Err(Erro::campo(campo, "Use um percentual entre 0 e 100."));
            }
        }
        Ok(v)
    };
    let desconto_maximo = percentual(&f.desconto_maximo, "descontoMaximo")?;
    let comissao = percentual(&f.comissao, "comissao")?;

    let oferta_preco = numeros::ler_dinheiro(&f.oferta_preco).map_err(|m| Erro::campo("ofertaPreco", m))?;
    if let Some(v) = oferta_preco {
        if v <= 0 {
            return Err(Erro::campo("ofertaPreco", "O preço da oferta precisa ser maior que zero."));
        }
        if v > MAX_PRECO {
            return Err(Erro::campo("ofertaPreco", "Preço alto demais."));
        }
    }
    let oferta_ate = fiscal::ler_data(&f.oferta_ate).map_err(|m| Erro::campo("ofertaAte", m))?.unwrap_or_default();
    if oferta_preco.is_none() && !oferta_ate.is_empty() {
        return Err(Erro::campo("ofertaPreco", "Informe o preço da oferta."));
    }
    let variacao = texto_curto(&f.variacao, "variacao", "Tamanho ou cor", 30)?;
    let fiscal_peca = f.fiscal.conferir()?;

    let ano_atual = tempo::ano_atual();
    let mut aplicacoes = Vec::new();
    for (i, a) in f.aplicacoes.iter().enumerate() {
        let campo = |nome: &str| format!("aplicacoes.{i}.{nome}");
        let montadora = texto_curto(&a.montadora, &campo("montadora"), "Montadora", 40)?;
        let modelo = texto_curto(&a.modelo, &campo("modelo"), "Modelo", 40)?;
        let motor = texto_curto(&a.motor, &campo("motor"), "Motor", 30)?;
        let observacao = texto_curto(&a.observacao, &campo("observacao"), "Observação", 80)?;
        let ano_inicio = numeros::ler_ano(&a.ano_inicio, ano_atual).map_err(|m| Erro::campo(&campo("anoInicio"), m))?;
        let ano_fim = numeros::ler_ano(&a.ano_fim, ano_atual).map_err(|m| Erro::campo(&campo("anoFim"), m))?;
        if montadora.is_empty()
            && modelo.is_empty()
            && motor.is_empty()
            && observacao.is_empty()
            && ano_inicio.is_none()
            && ano_fim.is_none()
        {
            continue;
        }
        if montadora.is_empty() && modelo.is_empty() {
            return Err(Erro::campo(&campo("modelo"), "Informe o modelo do carro (ou a montadora)."));
        }
        if let (Some(ini), Some(fim)) = (ano_inicio, ano_fim) {
            if ini > fim {
                return Err(Erro::campo(&campo("anoFim"), "O ano final é anterior ao inicial."));
            }
        }
        aplicacoes.push(Aplicacao {
            montadora,
            modelo,
            motor,
            ano_inicio,
            ano_fim,
            observacao,
        });
    }
    if aplicacoes.len() > MAX_LINHAS {
        return Err(Erro::validacao(format!("Use no máximo {MAX_LINHAS} aplicações por peça.")));
    }

    let mut equivalentes: Vec<Equivalente> = Vec::new();
    for (i, q) in f.equivalentes.iter().enumerate() {
        let marca = texto_curto(&q.marca, &format!("equivalentes.{i}.marca"), "Marca", 40)?;
        let codigo = texto_curto(&q.codigo, &format!("equivalentes.{i}.codigo"), "Código", 60)?;
        if marca.is_empty() && codigo.is_empty() {
            continue;
        }
        if codigo.is_empty() {
            return Err(Erro::campo(&format!("equivalentes.{i}.codigo"), "Informe o código da peça equivalente."));
        }
        let repetido = equivalentes.iter().any(|e| {
            texto::normalizar(&e.marca) == texto::normalizar(&marca) && texto::compactar(&e.codigo) == texto::compactar(&codigo)
        });
        if !repetido {
            equivalentes.push(Equivalente { marca, codigo });
        }
    }
    if equivalentes.len() > MAX_LINHAS {
        return Err(Erro::validacao(format!("Use no máximo {MAX_LINHAS} equivalências por peça.")));
    }

    Ok((
        Dados {
            codigo,
            nome,
            marca,
            unidade,
            codigo_barras,
            codigo_original,
            categoria,
            ativo: f.ativo,
            custo,
            precos,
            estoque_minimo,
            local,
            ncm,
            cest,
            origem: f.origem,
            anp_codigo,
            anp_descricao,
            csosn,
            monofasico: f.monofasico,
            peso_gramas,
            desconto_maximo,
            comissao,
            observacoes,
            aplicacoes,
            equivalentes,
            tipo,
            oferta_preco,
            oferta_ate,
            preco_aberto: f.preco_aberto,
            variacao,
            grade: String::new(),
            fiscal: fiscal_peca,
        },
        if f.tipo == "servico" { 0 } else { estoque_inicial },
    ))
}

// ---------------------------------------------------------------------------
// Gravar e ler.
// ---------------------------------------------------------------------------

/// Próximo código interno livre (maior código numérico + 1).
pub fn proximo_codigo(conn: &Connection) -> Resultado<String> {
    let maior: i64 = conn.query_row(
        "SELECT COALESCE(MAX(CAST(codigo AS INTEGER)), 0) FROM produto
          WHERE codigo <> '' AND codigo NOT GLOB '*[^0-9]*' AND length(codigo) <= 15",
        [],
        |l| l.get(0),
    )?;
    Ok((maior + 1).to_string())
}

/// Salva a peça (nova ou alterada). Devolve o id.
pub fn salvar(conn: &Connection, dispositivo: &str, usuario_id: &str, ficha: &ProdutoFicha) -> Resultado<String> {
    let novo = ficha.id.trim().is_empty();
    let antes = if novo { None } else { Some(carregar(conn, &ficha.id)?) };
    let (mut dados, estoque_inicial) = validar(ficha)?;
    let id = if novo { tempo::novo_id() } else { ficha.id.clone() };
    if let Some(a) = &antes {
        dados.grade = a.grade.clone();
        if !a.grade.is_empty() && dados.variacao.is_empty() {
            return Err(Erro::campo("variacao", "Diga o tamanho ou a cor deste produto."));
        }
        if a.tipo != "servico" && dados.tipo == "servico" && estoque::saldo(conn, &id)? != 0 {
            return Err(Erro::campo("tipo", "Serviço não tem estoque. Zere o estoque antes de mudar para serviço."));
        }
    }
    unidades::garantir(conn, &dados.unidade)?;

    if dados.codigo.is_empty() {
        if novo {
            dados.codigo = proximo_codigo(conn)?;
        } else {
            return Err(Erro::campo("codigo", "O código interno não pode ficar vazio."));
        }
    }
    let mesmo_codigo: Option<String> = conn
        .query_row(
            "SELECT nome FROM produto WHERE codigo = ?1 AND id <> ?2",
            params![dados.codigo, id],
            |l| l.get(0),
        )
        .optional()?;
    if let Some(outro) = mesmo_codigo {
        return Err(Erro::campo("codigo", format!("O código {} já é da peça \"{outro}\".", dados.codigo)));
    }
    if !dados.codigo_barras.is_empty() {
        let mesmo_barras: Option<String> = conn
            .query_row(
                "SELECT nome FROM produto WHERE codigo_barras = ?1 AND id <> ?2",
                params![dados.codigo_barras, id],
                |l| l.get(0),
            )
            .optional()?;
        if let Some(outro) = mesmo_barras {
            return Err(Erro::campo("codigoBarras", format!("Este código de barras já é da peça \"{outro}\".")));
        }
    }

    let tabelas = config::tabelas_preco(conn)?;
    let detalhes = match &antes {
        Some(a) => {
            let d = diferencas(&tabelas, a, &dados);
            if d.is_empty() {
                return Ok(id); // nada mudou
            }
            d
        }
        None => {
            let mut d = Vec::new();
            if let Some((_, preco, _)) = dados.precos.iter().find(|p| p.0 == 1) {
                d.push(format!("{}: {}", nome_tabela(&tabelas, 1), numeros::escrever_dinheiro(*preco)));
            }
            if estoque_inicial != 0 {
                d.push(format!("Estoque inicial: {}", numeros::escrever_quantidade(estoque_inicial)));
            }
            d
        }
    };

    let (busca, codigos) = texto_busca(&dados);
    let agora = tempo::agora();
    if novo {
        conn.execute(
            "INSERT INTO produto (id, codigo, nome, marca, unidade, codigo_barras, codigo_original, categoria,
                                  custo, estoque_minimo, local, ncm, cest, origem, anp_codigo, anp_descricao,
                                  peso_gramas, desconto_maximo, comissao, observacoes, ativo, busca, codigos,
                                  criado_em, criado_por, atualizado_em, atualizado_por, dispositivo, csosn, monofasico,
                                  tipo, oferta_preco, oferta_ate, preco_aberto, variacao, grade, fiscal)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                     ?21, ?22, ?23, ?24, ?25, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35)",
            params![
                id,
                dados.codigo,
                dados.nome,
                dados.marca,
                dados.unidade,
                dados.codigo_barras,
                dados.codigo_original,
                dados.categoria,
                dados.custo,
                dados.estoque_minimo,
                dados.local,
                dados.ncm,
                dados.cest,
                dados.origem,
                dados.anp_codigo,
                dados.anp_descricao,
                dados.peso_gramas,
                dados.desconto_maximo,
                dados.comissao,
                dados.observacoes,
                dados.ativo,
                busca,
                codigos,
                agora,
                usuario_id,
                dispositivo,
                dados.csosn,
                dados.monofasico,
                dados.tipo,
                dados.oferta_preco,
                dados.oferta_ate,
                dados.preco_aberto,
                dados.variacao,
                dados.grade,
                dados.fiscal.para_json(),
            ],
        )?;
    } else {
        conn.execute(
            "UPDATE produto SET codigo = ?2, nome = ?3, marca = ?4, unidade = ?5, codigo_barras = ?6,
                    codigo_original = ?7, categoria = ?8, custo = ?9, estoque_minimo = ?10, local = ?11,
                    ncm = ?12, cest = ?13, origem = ?14, anp_codigo = ?15, anp_descricao = ?16,
                    peso_gramas = ?17, desconto_maximo = ?18, comissao = ?19, observacoes = ?20, ativo = ?21,
                    busca = ?22, codigos = ?23, atualizado_em = ?24, atualizado_por = ?25, dispositivo = ?26,
                    csosn = ?27, monofasico = ?28, tipo = ?29, oferta_preco = ?30, oferta_ate = ?31,
                    preco_aberto = ?32, variacao = ?33, grade = ?34, fiscal = ?35
              WHERE id = ?1",
            params![
                id,
                dados.codigo,
                dados.nome,
                dados.marca,
                dados.unidade,
                dados.codigo_barras,
                dados.codigo_original,
                dados.categoria,
                dados.custo,
                dados.estoque_minimo,
                dados.local,
                dados.ncm,
                dados.cest,
                dados.origem,
                dados.anp_codigo,
                dados.anp_descricao,
                dados.peso_gramas,
                dados.desconto_maximo,
                dados.comissao,
                dados.observacoes,
                dados.ativo,
                busca,
                codigos,
                agora,
                usuario_id,
                dispositivo,
                dados.csosn,
                dados.monofasico,
                dados.tipo,
                dados.oferta_preco,
                dados.oferta_ate,
                dados.preco_aberto,
                dados.variacao,
                dados.grade,
                dados.fiscal.para_json(),
            ],
        )?;
        conn.execute("DELETE FROM produto_preco WHERE produto_id = ?1", [&id])?;
        conn.execute("DELETE FROM produto_aplicacao WHERE produto_id = ?1", [&id])?;
        conn.execute("DELETE FROM produto_equivalente WHERE produto_id = ?1", [&id])?;
    }

    for (tabela, preco, lucro) in &dados.precos {
        conn.execute(
            "INSERT INTO produto_preco (produto_id, tabela, preco, lucro) VALUES (?1, ?2, ?3, ?4)",
            params![id, tabela, preco, lucro],
        )?;
    }
    for (ordem, a) in dados.aplicacoes.iter().enumerate() {
        conn.execute(
            "INSERT INTO produto_aplicacao (id, produto_id, ordem, montadora, modelo, motor, ano_inicio, ano_fim, observacao)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![tempo::novo_id(), id, ordem as i64, a.montadora, a.modelo, a.motor, a.ano_inicio, a.ano_fim, a.observacao],
        )?;
    }
    for (ordem, q) in dados.equivalentes.iter().enumerate() {
        conn.execute(
            "INSERT INTO produto_equivalente (id, produto_id, ordem, marca, codigo, codigo_compacto)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![tempo::novo_id(), id, ordem as i64, q.marca, q.codigo, texto::compactar(&q.codigo)],
        )?;
    }

    if novo && estoque_inicial != 0 {
        estoque::lancar(conn, dispositivo, Some(usuario_id), &id, estoque_inicial, "inicial", "Estoque inicial")?;
    }
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        if novo { "produto.criar" } else { "produto.alterar" },
        "produto",
        &id,
        &detalhes,
    )?;
    Ok(id)
}

/// Lê os dados gravados de uma peça.
fn carregar(conn: &Connection, id: &str) -> Resultado<Dados> {
    let mut d = conn
        .query_row(
            "SELECT codigo, nome, marca, unidade, codigo_barras, codigo_original, categoria, ativo, custo,
                    estoque_minimo, local, ncm, cest, origem, anp_codigo, anp_descricao, peso_gramas,
                    desconto_maximo, comissao, observacoes, csosn, monofasico,
                    tipo, oferta_preco, oferta_ate, preco_aberto, variacao, grade, fiscal
               FROM produto WHERE id = ?1",
            [id],
            |l| {
                Ok(Dados {
                    codigo: l.get(0)?,
                    nome: l.get(1)?,
                    marca: l.get(2)?,
                    unidade: l.get(3)?,
                    codigo_barras: l.get(4)?,
                    codigo_original: l.get(5)?,
                    categoria: l.get(6)?,
                    ativo: l.get(7)?,
                    custo: l.get(8)?,
                    estoque_minimo: l.get(9)?,
                    local: l.get(10)?,
                    ncm: l.get(11)?,
                    cest: l.get(12)?,
                    origem: l.get(13)?,
                    anp_codigo: l.get(14)?,
                    anp_descricao: l.get(15)?,
                    peso_gramas: l.get(16)?,
                    desconto_maximo: l.get(17)?,
                    comissao: l.get(18)?,
                    observacoes: l.get(19)?,
                    csosn: l.get(20)?,
                    monofasico: l.get(21)?,
                    tipo: l.get(22)?,
                    oferta_preco: l.get(23)?,
                    oferta_ate: l.get(24)?,
                    preco_aberto: l.get(25)?,
                    variacao: l.get(26)?,
                    grade: l.get(27)?,
                    fiscal: fiscal::FiscalPeca::de_json(&l.get::<_, String>(28)?),
                    ..Default::default()
                })
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;

    let mut consulta = conn.prepare_cached(
        "SELECT tabela, preco, lucro FROM produto_preco WHERE produto_id = ?1 ORDER BY tabela",
    )?;
    d.precos = consulta
        .query_map([id], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?
        .collect::<Result<_, _>>()?;

    let mut consulta = conn.prepare_cached(
        "SELECT montadora, modelo, motor, ano_inicio, ano_fim, observacao
           FROM produto_aplicacao WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    d.aplicacoes = consulta
        .query_map([id], |l| {
            Ok(Aplicacao {
                montadora: l.get(0)?,
                modelo: l.get(1)?,
                motor: l.get(2)?,
                ano_inicio: l.get(3)?,
                ano_fim: l.get(4)?,
                observacao: l.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut consulta = conn.prepare_cached(
        "SELECT marca, codigo FROM produto_equivalente WHERE produto_id = ?1 ORDER BY ordem",
    )?;
    d.equivalentes = consulta
        .query_map([id], |l| Ok(Equivalente { marca: l.get(0)?, codigo: l.get(1)? }))?
        .collect::<Result<_, _>>()?;
    Ok(d)
}

/// Ficha completa de uma peça para a tela.
pub fn ficha(conn: &Connection, id: &str) -> Resultado<ProdutoFicha> {
    let d = carregar(conn, id)?;
    let tabelas = config::tabelas_preco(conn)?;
    let (criado_em, criado_por, atualizado_em, atualizado_por): (String, String, String, String) = conn.query_row(
        "SELECT p.criado_em, COALESCE(c.nome, ''), p.atualizado_em, COALESCE(a.nome, '')
           FROM produto p
           LEFT JOIN usuario c ON c.id = p.criado_por
           LEFT JOIN usuario a ON a.id = p.atualizado_por
          WHERE p.id = ?1",
        [id],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?, l.get(3)?)),
    )?;
    let saldo = estoque::saldo(conn, id)?;
    let hoje = fiscal::hoje(conn)?;
    let grade_itens = if d.grade.is_empty() { Vec::new() } else { itens_grade(conn, &d.grade)? };
    let fracionada = unidades::fracionada(conn, &d.unidade)?;
    let tributacao = fiscal::tributacao(conn, &fiscal::uf_loja(conn)?, &d.cest, &d.csosn, &fiscal::hoje(conn)?)?;
    let mut consulta = conn.prepare_cached(
        "SELECT p.nome, f.codigo, f.fator FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
          WHERE f.produto_id = ?1 ORDER BY f.atualizado_em DESC",
    )?;
    let fornecedores = consulta
        .query_map([id], |l| {
            Ok(CodigoFornecedor {
                fornecedor: l.get(0)?,
                codigo: l.get(1)?,
                fator: numeros::escrever_quantidade(l.get(2)?),
            })
        })?
        .collect::<Result<_, _>>()?;

    let precos = tabelas
        .iter()
        .map(|t| {
            let p = d.precos.iter().find(|p| p.0 == t.numero);
            PrecoFicha {
                tabela: t.numero,
                nome: t.nome.clone(),
                lucro: p
                    .and_then(|p| p.2)
                    .map(numeros::escrever_percentual)
                    .unwrap_or_default(),
                preco: p.map(|p| numeros::escrever_dinheiro(p.1)).unwrap_or_default(),
            }
        })
        .collect();

    Ok(ProdutoFicha {
        id: id.to_string(),
        codigo: d.codigo,
        nome: d.nome,
        marca: d.marca,
        unidade: d.unidade,
        codigo_barras: d.codigo_barras,
        codigo_original: d.codigo_original,
        categoria: d.categoria,
        ativo: d.ativo,
        custo: if d.custo > 0 { numeros::escrever_dinheiro(d.custo) } else { String::new() },
        precos,
        estoque_inicial: String::new(),
        estoque_minimo: if d.estoque_minimo > 0 { numeros::escrever_quantidade(d.estoque_minimo) } else { String::new() },
        local: d.local,
        ncm: d.ncm,
        cest: d.cest,
        origem: d.origem,
        anp_codigo: d.anp_codigo,
        anp_descricao: d.anp_descricao,
        csosn: d.csosn,
        monofasico: d.monofasico,
        peso: if d.peso_gramas > 0 { numeros::escrever_quantidade(d.peso_gramas) } else { String::new() },
        desconto_maximo: d.desconto_maximo.map(numeros::escrever_percentual).unwrap_or_default(),
        comissao: d.comissao.map(numeros::escrever_percentual).unwrap_or_default(),
        observacoes: d.observacoes,
        aplicacoes: d
            .aplicacoes
            .into_iter()
            .map(|a| AplicacaoFicha {
                montadora: a.montadora,
                modelo: a.modelo,
                motor: a.motor,
                ano_inicio: a.ano_inicio.map(|x| x.to_string()).unwrap_or_default(),
                ano_fim: a.ano_fim.map(|x| x.to_string()).unwrap_or_default(),
                observacao: a.observacao,
            })
            .collect(),
        equivalentes: d
            .equivalentes
            .into_iter()
            .map(|q| EquivalenteFicha { marca: q.marca, codigo: q.codigo })
            .collect(),
        estoque: numeros::escrever_quantidade(saldo),
        estoque_valor: saldo,
        criado_em,
        criado_por,
        atualizado_em,
        atualizado_por,
        historico: auditoria::historico(conn, "produto", id, 30)?,
        tributacao: Some(tributacao),
        fornecedores,
        oferta_valendo: oferta_vale(d.oferta_preco, &d.oferta_ate, &hoje),
        oferta_preco: d.oferta_preco.map(numeros::escrever_dinheiro).unwrap_or_default(),
        oferta_ate: if d.oferta_ate.is_empty() { String::new() } else { fiscal::data_br(&d.oferta_ate) },
        preco_aberto: d.preco_aberto,
        tipo: d.tipo,
        variacao: d.variacao,
        fiscal: d.fiscal,
        grade_itens,
        fracionada,
    })
}

/// A oferta vale nesta data ("AAAA-MM-DD")?
pub fn oferta_vale(preco: Option<i64>, ate: &str, hoje: &str) -> bool {
    preco.is_some() && (ate.is_empty() || ate >= hoje)
}

/// Preço da primeira tabela já com a oferta (SQL, para a peça `p`).
pub const SQL_PRECO_1: &str = "COALESCE(CASE WHEN p.oferta_preco IS NOT NULL AND (p.oferta_ate = '' OR p.oferta_ate >= date('now', 'localtime'))
                                    THEN p.oferta_preco END,
                               (SELECT preco FROM produto_preco WHERE produto_id = p.id AND tabela = 1))";

/// A oferta vale hoje (SQL, para a peça `p`).
pub const SQL_EM_OFERTA: &str = "(p.oferta_preco IS NOT NULL AND (p.oferta_ate = '' OR p.oferta_ate >= date('now', 'localtime')))";

fn itens_grade(conn: &Connection, grade: &str) -> Resultado<Vec<ItemGrade>> {
    let mut c = conn.prepare_cached(
        "SELECT id, variacao, codigo FROM produto WHERE grade = ?1 ORDER BY criado_em, rowid",
    )?;
    let lista: Vec<(String, String, String)> =
        c.query_map([grade], |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)))?.collect::<Result<_, _>>()?;
    lista
        .into_iter()
        .map(|(id, variacao, codigo)| {
            Ok(ItemGrade { estoque: numeros::escrever_quantidade(estoque::saldo(conn, &id)?), id, variacao, codigo })
        })
        .collect()
}

/// Cria mais um tamanho ou cor a partir deste produto: copia tudo (preços,
/// fiscal, carros) com código novo e estoque zero. `atual` = o tamanho ou cor
/// deste, se ainda não tiver. Devolve o id do novo.
pub fn criar_variacao(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str, atual: &str, nova: &str) -> Resultado<String> {
    let mut base = ficha(conn, id)?;
    let nova = texto_curto(nova, "nova", "Tamanho ou cor", 30)?;
    if nova.is_empty() {
        return Err(Erro::campo("nova", "Diga o novo tamanho ou cor (ex.: M, Azul G)."));
    }
    if base.variacao.is_empty() {
        let atual = texto_curto(atual, "atual", "Tamanho ou cor", 30)?;
        if atual.is_empty() {
            return Err(Erro::campo("atual", "Diga o tamanho ou a cor deste produto (ex.: P)."));
        }
        base.variacao = atual;
        base.nome = format!("{} {}", base.nome, base.variacao).chars().take(120).collect();
        salvar(conn, dispositivo, usuario_id, &base)?;
    }
    let grade: String = conn.query_row("SELECT grade FROM produto WHERE id = ?1", [id], |l| l.get(0))?;
    let grade = if grade.is_empty() { id.to_string() } else { grade };
    conn.execute("UPDATE produto SET grade = ?1 WHERE id = ?2 AND grade = ''", params![grade, id])?;
    let repetida: bool = conn
        .query_row(
            "SELECT 1 FROM produto WHERE grade = ?1 AND normalizar(variacao) = normalizar(?2)",
            params![grade, nova],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if repetida {
        return Err(Erro::campo("nova", format!("Já existe \"{nova}\" neste produto.")));
    }
    // O nome sem o tamanho/cor deste, mais o novo.
    let sufixo = format!(" {}", base.variacao);
    let raiz = base.nome.strip_suffix(&sufixo).unwrap_or(&base.nome).to_string();
    let mut novo = base;
    novo.id = String::new();
    novo.codigo = String::new();
    novo.codigo_barras = String::new();
    novo.estoque_inicial = String::new();
    novo.nome = format!("{raiz} {nova}").chars().take(120).collect();
    novo.variacao = nova;
    let novo_id = salvar(conn, dispositivo, usuario_id, &novo)?;
    conn.execute("UPDATE produto SET grade = ?1 WHERE id = ?2", params![grade, novo_id])?;
    Ok(novo_id)
}

/// Ficha em branco para cadastrar uma peça nova.
pub fn ficha_nova(conn: &Connection) -> Resultado<ProdutoFicha> {
    let precos = config::tabelas_preco(conn)?
        .into_iter()
        .map(|t| PrecoFicha {
            tabela: t.numero,
            nome: t.nome,
            lucro: t.lucro_padrao,
            preco: String::new(),
        })
        .collect();
    Ok(ProdutoFicha {
        codigo: proximo_codigo(conn)?,
        unidade: "UN".into(),
        tipo: "produto".into(),
        ativo: true,
        precos,
        estoque: "0".into(),
        tributacao: Some(fiscal::tributacao(conn, &fiscal::uf_loja(conn)?, "", "", &fiscal::hoje(conn)?)?),
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// Ajudantes.
// ---------------------------------------------------------------------------

/// Calcula o preço pelo percentual, ou o percentual pelo preço, conforme o
/// campo que a pessoa mexeu (`de` = "custo", "lucro" ou "preco").
/// Devolve (lucro, preço) já escritos. Enquanto o número está incompleto,
/// devolve o que já estava.
pub fn calcular_preco(custo: &str, lucro: &str, preco: &str, de: &str) -> (String, String) {
    let c = numeros::ler_dinheiro(custo).ok().flatten().filter(|c| *c > 0);
    let l = numeros::ler_percentual(lucro).ok().flatten();
    let p = numeros::ler_dinheiro(preco).ok().flatten().filter(|p| *p > 0);
    let pelo_preco = || match (c, p) {
        (Some(c), Some(p)) => numeros::lucro_por_preco(c, p).map(numeros::escrever_percentual).unwrap_or_default(),
        _ => lucro.to_string(),
    };
    let pelo_lucro = || match (c, l) {
        (Some(c), Some(l)) => numeros::escrever_dinheiro(numeros::preco_por_lucro(c, l)),
        _ => preco.to_string(),
    };
    match de {
        "preco" => (pelo_preco(), preco.to_string()),
        "lucro" => (lucro.to_string(), pelo_lucro()),
        _ => {
            if l.is_some() {
                (lucro.to_string(), pelo_lucro())
            } else {
                (pelo_preco(), preco.to_string())
            }
        }
    }
}

/// Valores já usados, para sugerir enquanto a pessoa digita.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sugestoes {
    pub marcas: Vec<String>,
    pub categorias: Vec<String>,
    pub montadoras: Vec<String>,
    pub modelos: Vec<String>,
    pub locais: Vec<String>,
    pub unidades: Vec<String>,
    pub origens: Vec<String>,
}

pub fn sugestoes(conn: &Connection) -> Resultado<Sugestoes> {
    let valores = |sql: &str| -> Resultado<Vec<String>> {
        let mut consulta = conn.prepare(sql)?;
        let linhas = consulta.query_map([], |l| l.get(0))?;
        Ok(linhas.collect::<Result<_, _>>()?)
    };
    let unidades = valores(
        "SELECT u.sigla FROM unidade u ORDER BY (SELECT COUNT(*) FROM produto p WHERE p.unidade = u.sigla) DESC, u.sigla",
    )?;
    Ok(Sugestoes {
        marcas: valores(
            "SELECT marca FROM (SELECT marca FROM produto UNION ALL SELECT marca FROM produto_equivalente)
              WHERE marca <> '' GROUP BY marca ORDER BY COUNT(*) DESC, marca LIMIT 300",
        )?,
        categorias: valores(
            "SELECT categoria FROM produto WHERE categoria <> '' GROUP BY categoria ORDER BY COUNT(*) DESC, categoria LIMIT 300",
        )?,
        montadoras: valores(
            "SELECT montadora FROM produto_aplicacao WHERE montadora <> '' GROUP BY montadora ORDER BY COUNT(*) DESC, montadora LIMIT 300",
        )?,
        modelos: valores(
            "SELECT modelo FROM produto_aplicacao WHERE modelo <> '' GROUP BY modelo ORDER BY COUNT(*) DESC, modelo LIMIT 500",
        )?,
        locais: valores("SELECT local FROM produto WHERE local <> '' GROUP BY local ORDER BY local LIMIT 500")?,
        unidades,
        origens: ORIGENS.iter().map(|o| o.to_string()).collect(),
    })
}

fn nome_tabela(tabelas: &[config::TabelaPreco], numero: i64) -> String {
    let nome = tabelas
        .iter()
        .find(|t| t.numero == numero)
        .map(|t| t.nome.clone())
        .unwrap_or_else(|| format!("Tabela {numero}"));
    format!("Preço {nome}")
}

/// Texto de busca da peça: nome primeiro (para a ordenação), depois o resto.
fn texto_busca(d: &Dados) -> (String, String) {
    let mut partes: Vec<String> = vec![
        d.nome.clone(),
        d.marca.clone(),
        d.codigo.clone(),
        d.codigo_barras.clone(),
        d.codigo_original.clone(),
        d.categoria.clone(),
        d.local.clone(),
        d.variacao.clone(),
    ];
    for a in &d.aplicacoes {
        partes.push(a.montadora.clone());
        for s in texto::sinonimos_montadora(&texto::normalizar(&a.montadora)) {
            partes.push(s.to_string());
        }
        partes.push(a.modelo.clone());
        partes.push(a.motor.clone());
        partes.push(a.observacao.clone());
    }
    for q in &d.equivalentes {
        partes.push(q.marca.clone());
        partes.push(q.codigo.clone());
    }
    let mut codigos: Vec<String> = Vec::new();
    for c in [&d.codigo, &d.codigo_barras, &d.codigo_original]
        .into_iter()
        .chain(d.equivalentes.iter().map(|q| &q.codigo))
    {
        let compacto = texto::compactar(c);
        if !compacto.is_empty() && !codigos.contains(&compacto) {
            codigos.push(compacto);
        }
    }
    let busca = format!("{} {}", texto::normalizar(&partes.join(" ")), codigos.join(" "));
    (busca.trim().to_string(), format!(" {} ", codigos.join(" ")))
}

/// Frases com o que mudou, para o histórico.
fn diferencas(tabelas: &[config::TabelaPreco], a: &Dados, b: &Dados) -> Vec<String> {
    fn ou_traco(s: &str) -> String {
        if s.is_empty() {
            "—".into()
        } else {
            s.to_string()
        }
    }
    let mut v = Vec::new();
    let mut campo = |rotulo: &str, antes: String, depois: String| {
        if antes != depois {
            v.push(format!("{rotulo}: {} → {}", ou_traco(&antes), ou_traco(&depois)));
        }
    };
    let dinheiro = |x: i64| if x > 0 { numeros::escrever_dinheiro(x) } else { String::new() };
    let quantidade = |x: i64| if x > 0 { numeros::escrever_quantidade(x) } else { String::new() };
    let pct = |x: Option<i64>| x.map(numeros::escrever_percentual).unwrap_or_default();

    campo("Código", a.codigo.clone(), b.codigo.clone());
    campo("Nome", a.nome.clone(), b.nome.clone());
    campo("Marca", a.marca.clone(), b.marca.clone());
    campo("Unidade", a.unidade.clone(), b.unidade.clone());
    campo("Código de barras", a.codigo_barras.clone(), b.codigo_barras.clone());
    campo("Código original", a.codigo_original.clone(), b.codigo_original.clone());
    campo("Categoria", a.categoria.clone(), b.categoria.clone());
    campo("Custo", dinheiro(a.custo), dinheiro(b.custo));
    for t in tabelas {
        let preco = |d: &Dados| {
            d.precos
                .iter()
                .find(|p| p.0 == t.numero)
                .map(|p| numeros::escrever_dinheiro(p.1))
                .unwrap_or_default()
        };
        campo(&nome_tabela(tabelas, t.numero), preco(a), preco(b));
    }
    campo("Estoque mínimo", quantidade(a.estoque_minimo), quantidade(b.estoque_minimo));
    campo("Local", a.local.clone(), b.local.clone());
    campo("NCM", a.ncm.clone(), b.ncm.clone());
    campo("CEST", a.cest.clone(), b.cest.clone());
    campo("Origem", a.origem.to_string(), b.origem.to_string());
    campo("Código ANP", a.anp_codigo.clone(), b.anp_codigo.clone());
    campo("Descrição ANP", a.anp_descricao.clone(), b.anp_descricao.clone());
    let cs = |c: &str| if c.is_empty() { "automático".to_string() } else { c.to_string() };
    campo("CSOSN", cs(&a.csosn), cs(&b.csosn));
    let mono = |m: bool| if m { "sim".to_string() } else { "não".to_string() };
    campo("Monofásica (PIS/Cofins)", mono(a.monofasico), mono(b.monofasico));
    campo("Peso (kg)", quantidade(a.peso_gramas), quantidade(b.peso_gramas));
    campo("Desconto máximo (%)", pct(a.desconto_maximo), pct(b.desconto_maximo));
    campo("Comissão (%)", pct(a.comissao), pct(b.comissao));
    let tipo = |t: &str| if t == "servico" { "serviço".to_string() } else { "produto".to_string() };
    campo("Tipo", tipo(&a.tipo), tipo(&b.tipo));
    let oferta = |d: &Dados| match d.oferta_preco {
        Some(p) if d.oferta_ate.is_empty() => numeros::escrever_dinheiro(p),
        Some(p) => format!("{} até {}", numeros::escrever_dinheiro(p), fiscal::data_br(&d.oferta_ate)),
        None => String::new(),
    };
    campo("Oferta", oferta(a), oferta(b));
    campo("Preço na hora da venda", mono(a.preco_aberto), mono(b.preco_aberto));
    campo("Tamanho ou cor", a.variacao.clone(), b.variacao.clone());
    if a.observacoes != b.observacoes {
        v.push("Observações alteradas".into());
    }
    if a.aplicacoes != b.aplicacoes {
        if a.aplicacoes.len() != b.aplicacoes.len() {
            v.push(format!("Aplicações: {} → {} carros", a.aplicacoes.len(), b.aplicacoes.len()));
        } else {
            v.push("Aplicações alteradas".into());
        }
    }
    if a.equivalentes != b.equivalentes {
        if a.equivalentes.len() != b.equivalentes.len() {
            v.push(format!("Equivalências: {} → {}", a.equivalentes.len(), b.equivalentes.len()));
        } else {
            v.push("Equivalências alteradas".into());
        }
    }
    if a.ativo != b.ativo {
        v.push(if b.ativo { "Peça reativada".into() } else { "Peça desativada".into() });
    }
    v.extend(a.fiscal.diferencas(&b.fiscal));
    // Se só o percentual mudou por causa do custo, o preço já aparece acima.
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn calcula_preco_e_lucro() {
        assert_eq!(calcular_preco("58", "100", "", "lucro"), ("100".into(), "116,00".into()));
        assert_eq!(calcular_preco("58,00", "100", "85,00", "preco"), ("46,55".into(), "85,00".into()));
        // mudou o custo: mantém o percentual e recalcula o preço
        assert_eq!(calcular_preco("60", "100", "116,00", "custo"), ("100".into(), "120,00".into()));
        // sem percentual: recalcula o percentual pelo preço
        assert_eq!(calcular_preco("50", "", "85,00", "custo"), ("70".into(), "85,00".into()));
        // número incompleto: não mexe
        assert_eq!(calcular_preco("", "100", "85,00", "lucro"), ("100".into(), "85,00".into()));
        assert_eq!(calcular_preco("58", "abc", "85,00", "lucro"), ("abc".into(), "85,00".into()));
    }
}
