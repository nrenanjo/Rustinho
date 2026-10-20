//! Clientes e fornecedores (pessoas físicas e jurídicas).

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::documento::{self, UFS};
use crate::erro::{Erro, Resultado};
use crate::{auditoria, faixas, numeros, tempo, texto};

const MAX_VEICULOS: usize = 50;

/// Ficha da pessoa, com os campos como aparecem na tela.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PessoaFicha {
    pub id: String,
    pub codigo: String,
    /// O código com letras ainda espera a confirmação do caixa que distribui
    /// as faixas (PEND-07): não vale como único até lá.
    pub codigo_pendente: bool,
    pub cliente: bool,
    pub fornecedor: bool,
    /// "F" ou "J"
    pub tipo: String,
    pub nome: String,
    pub fantasia: String,
    pub documento: String,
    pub ie: String,
    pub indicador_ie: i64,
    pub telefone: String,
    pub celular: String,
    pub email: String,
    pub cep: String,
    pub logradouro: String,
    pub numero: String,
    pub complemento: String,
    pub bairro: String,
    pub municipio: String,
    pub municipio_ibge: String,
    pub uf: String,
    pub limite_credito: String,
    /// Convênio: dia do mês em que as parcelas vencem (vazio = 30 dias).
    pub dia_vencimento: String,
    pub observacoes: String,
    pub ativo: bool,
    pub veiculos: Vec<VeiculoFicha>,

    /// Ficha nova: o código que o próximo cadastro deste caixa vai receber.
    #[serde(skip_deserializing)]
    pub codigo_previsto: String,
    /// Aviso sobre o código (caixa bloqueado, ou sem faixa ainda).
    #[serde(skip_deserializing)]
    pub codigo_aviso: String,
    #[serde(skip_deserializing)]
    pub criado_em: String,
    #[serde(skip_deserializing)]
    pub atualizado_em: String,
    #[serde(skip_deserializing)]
    pub atualizado_por: String,
    #[serde(skip_deserializing)]
    pub compras: i64,
    #[serde(skip_deserializing)]
    pub historico: Vec<auditoria::Registro>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct VeiculoFicha {
    pub placa: String,
    pub descricao: String,
}

/// Uma linha da lista.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PessoaResumo {
    pub id: String,
    pub codigo: String,
    /// Código esperando a confirmação (PEND-07).
    pub codigo_pendente: bool,
    pub nome: String,
    pub fantasia: String,
    pub documento: String,
    pub telefone: String,
    pub cidade: String,
    pub cliente: bool,
    pub fornecedor: bool,
    pub ativo: bool,
    pub veiculos: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Dados {
    cliente: bool,
    fornecedor: bool,
    tipo: String,
    nome: String,
    fantasia: String,
    documento: String,
    ie: String,
    indicador_ie: i64,
    telefone: String,
    celular: String,
    email: String,
    cep: String,
    logradouro: String,
    numero: String,
    complemento: String,
    bairro: String,
    municipio: String,
    municipio_ibge: String,
    uf: String,
    limite_credito: Option<i64>,
    dia_vencimento: Option<i64>,
    observacoes: String,
    ativo: bool,
    veiculos: Vec<VeiculoFicha>,
}

fn curto(valor: &str, campo: &str, rotulo: &str, maximo: usize) -> Resultado<String> {
    let v = texto::limpar(valor);
    if texto::tamanho(&v) > maximo {
        return Err(Erro::campo(campo, format!("{rotulo}: use no máximo {maximo} caracteres.")));
    }
    Ok(v)
}

fn validar(f: &PessoaFicha) -> Resultado<Dados> {
    if !f.cliente && !f.fornecedor {
        return Err(Erro::campo("cliente", "Marque se é cliente, fornecedor ou os dois."));
    }
    let doc = documento::limpar(&f.documento);
    // O tipo segue o documento quando ele foi digitado.
    let tipo = match doc.len() {
        11 => "F".to_string(),
        14 => "J".to_string(),
        _ => {
            if f.tipo == "J" {
                "J".into()
            } else {
                "F".into()
            }
        }
    };
    if !doc.is_empty() {
        let ok = match doc.len() {
            11 => documento::cpf_valido(&doc),
            14 => documento::cnpj_valido(&doc),
            _ => false,
        };
        if !ok {
            let o_que = if tipo == "J" || doc.len() > 11 { "CNPJ" } else { "CPF" };
            return Err(Erro::campo("documento", format!("{o_que} inválido. Confira os números.")));
        }
    }
    let nome = curto(&f.nome, "nome", "Nome", 100)?;
    if nome.is_empty() {
        let o_que = if tipo == "J" { "a razão social" } else { "o nome" };
        return Err(Erro::campo("nome", format!("Digite {o_que}.")));
    }
    let fantasia = curto(&f.fantasia, "fantasia", "Nome fantasia", 60)?;
    let ie = documento::limpar(&f.ie);
    let ie = if ie == "ISENTO" { String::new() } else { ie };
    if ie.len() > 14 {
        return Err(Erro::campo("ie", "A inscrição estadual tem no máximo 14 caracteres."));
    }
    let indicador_ie = if !ie.is_empty() {
        1
    } else if [1, 2, 9].contains(&f.indicador_ie) {
        if f.indicador_ie == 1 {
            9
        } else {
            f.indicador_ie
        }
    } else {
        9
    };
    let telefone = documento::escrever_telefone(&curto(&f.telefone, "telefone", "Telefone", 20)?);
    let celular = documento::escrever_telefone(&curto(&f.celular, "celular", "Celular", 20)?);
    let email = f.email.trim().to_lowercase();
    if !email.is_empty() {
        let partes: Vec<&str> = email.split('@').collect();
        if partes.len() != 2 || partes[0].is_empty() || !partes[1].contains('.') || email.contains(' ') {
            return Err(Erro::campo("email", "E-mail inválido (ex.: nome@empresa.com.br)."));
        }
        if email.len() > 80 {
            return Err(Erro::campo("email", "E-mail: use no máximo 80 caracteres."));
        }
    }
    let cep = documento::digitos(&f.cep);
    if !cep.is_empty() && cep.len() != 8 {
        return Err(Erro::campo("cep", "O CEP tem 8 números (ex.: 18320-095)."));
    }
    let uf = f.uf.trim().to_uppercase();
    if !uf.is_empty() && !UFS.contains(&uf.as_str()) {
        return Err(Erro::campo("uf", "Escolha o estado (UF)."));
    }
    let municipio_ibge = documento::digitos(&f.municipio_ibge);
    if !municipio_ibge.is_empty() && municipio_ibge.len() != 7 {
        return Err(Erro::campo("municipioIbge", "O código IBGE da cidade tem 7 números."));
    }
    let limite_credito = numeros::ler_dinheiro(&f.limite_credito).map_err(|m| Erro::campo("limiteCredito", m))?;
    if limite_credito.is_some_and(|v| v < 0) {
        return Err(Erro::campo("limiteCredito", "O limite não pode ser negativo."));
    }
    let dia_vencimento = match f.dia_vencimento.trim() {
        "" => None,
        t => match t.parse::<i64>() {
            Ok(d) if (1..=31).contains(&d) => Some(d),
            _ => return Err(Erro::campo("diaVencimento", "Use um dia de 1 a 31 (ou deixe vazio).")),
        },
    };
    let observacoes = f.observacoes.trim().to_string();
    if texto::tamanho(&observacoes) > 2000 {
        return Err(Erro::campo("observacoes", "Observações: use no máximo 2000 caracteres."));
    }

    let mut veiculos: Vec<VeiculoFicha> = Vec::new();
    for (i, v) in f.veiculos.iter().enumerate() {
        let placa = documento::ler_placa(&v.placa).map_err(|m| Erro::campo(&format!("veiculos.{i}.placa"), m))?;
        let descricao = curto(&v.descricao, &format!("veiculos.{i}.descricao"), "Carro", 60)?;
        if placa.is_empty() && descricao.is_empty() {
            continue;
        }
        if !placa.is_empty() && veiculos.iter().any(|x| x.placa == placa) {
            continue;
        }
        veiculos.push(VeiculoFicha { placa, descricao });
    }
    if veiculos.len() > MAX_VEICULOS {
        return Err(Erro::validacao(format!("Use no máximo {MAX_VEICULOS} carros por cliente.")));
    }

    Ok(Dados {
        cliente: f.cliente,
        fornecedor: f.fornecedor,
        tipo,
        nome,
        fantasia,
        documento: doc,
        ie,
        indicador_ie,
        telefone,
        celular,
        email,
        cep,
        logradouro: curto(&f.logradouro, "logradouro", "Endereço", 80)?,
        numero: curto(&f.numero, "numero", "Número", 20)?,
        complemento: curto(&f.complemento, "complemento", "Complemento", 60)?,
        bairro: curto(&f.bairro, "bairro", "Bairro", 60)?,
        municipio: curto(&f.municipio, "municipio", "Cidade", 60)?,
        municipio_ibge,
        uf,
        limite_credito,
        dia_vencimento,
        observacoes,
        ativo: f.ativo,
        veiculos,
    })
}

/// Código para um cliente ou fornecedor novo: o próximo da faixa deste caixa
/// (veja `faixas`). Vazio = "código a definir": o caixa ainda não tem faixa
/// (ou está bloqueado); o código sai quando a faixa chegar, sem mudar nenhum
/// outro dado da pessoa.
fn codigo_novo(conn: &Connection) -> Resultado<String> {
    Ok(faixas::gerar_para_pessoa(conn)?.unwrap_or_default())
}

/// Dá o código da faixa deste caixa a uma pessoa que estava com código a
/// definir (não muda nenhum outro dado). false = a pessoa já tinha código.
pub fn definir_codigo(conn: &Connection, dispositivo: &str, id: &str, codigo: &str) -> Resultado<bool> {
    let (_, dados) = carregar(conn, id)?;
    let busca = texto_busca(codigo, &dados);
    let n = conn.execute(
        "UPDATE pessoa SET codigo = ?2, busca = ?3, atualizado_em = ?4, dispositivo = ?5, codigo_validado = 1
          WHERE id = ?1 AND codigo = ''",
        params![id, codigo, busca, tempo::agora(), dispositivo],
    )?;
    if n == 1 {
        auditoria::registrar(conn, dispositivo, None, "pessoa.codigo", "pessoa", id, &[format!("Código: {codigo}")])?;
    }
    Ok(n == 1)
}

/// O gerente resolve um código repetido: a pessoa recebe um código novo da
/// faixa deste caixa. Não muda mais nada.
pub fn trocar_codigo(conn: &Connection, dispositivo: &str, usuario_id: &str, id: &str) -> Resultado<String> {
    let (velho, dados) = carregar(conn, id)?;
    let Some(novo) = faixas::gerar_para_pessoa(conn)? else {
        return Err(Erro::validacao(faixas::texto_sem_poder_dar(conn)?));
    };
    let busca = texto_busca(&novo, &dados);
    conn.execute(
        "UPDATE pessoa SET codigo = ?2, busca = ?3, atualizado_em = ?4, atualizado_por = ?5, dispositivo = ?6,
                codigo_validado = 1
          WHERE id = ?1",
        params![id, novo, busca, tempo::agora(), usuario_id, dispositivo],
    )?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        "pessoa.alterar",
        "pessoa",
        id,
        &[format!(
            "Código: {} → {novo} (código repetido resolvido pelo gerente)",
            if velho.is_empty() { "a definir" } else { &velho }
        )],
    )?;
    Ok(novo)
}

fn texto_busca(codigo: &str, d: &Dados) -> String {
    let mut partes = vec![
        d.nome.clone(),
        d.fantasia.clone(),
        codigo.to_string(),
        d.documento.clone(),
        documento::digitos(&d.telefone),
        documento::digitos(&d.celular),
        d.email.clone(),
        d.municipio.clone(),
        d.bairro.clone(),
    ];
    for v in &d.veiculos {
        partes.push(v.placa.clone());
        partes.push(v.descricao.clone());
    }
    texto::normalizar(&partes.join(" "))
}

/// O código que a pessoa tem agora (vazio para pessoa nova).
fn antes_codigo(conn: &Connection, id: &str, novo: bool) -> Resultado<String> {
    if novo {
        return Ok(String::new());
    }
    Ok(conn.query_row("SELECT codigo FROM pessoa WHERE id = ?1", [id], |l| l.get(0))?)
}

/// Salva a pessoa. Devolve o id.
pub fn salvar(conn: &Connection, dispositivo: &str, usuario_id: &str, ficha: &PessoaFicha) -> Resultado<String> {
    let novo = ficha.id.trim().is_empty();
    let antes = if novo { None } else { Some(carregar(conn, &ficha.id)?.1) };
    let dados = validar(ficha)?;
    let id = if novo { tempo::novo_id() } else { ficha.id.clone() };
    let codigo_antes = antes_codigo(conn, &id, novo)?;
    let codigo = {
        let c = texto::limpar(&ficha.codigo);
        if c.is_empty() {
            if novo || codigo_antes.is_empty() {
                // Vazio: o Rustinho dá o código ao salvar, da faixa deste
                // caixa (ou deixa "a definir" se o caixa ainda não tem faixa).
                codigo_novo(conn)?
            } else {
                return Err(Erro::campo("codigo", "O código não pode ficar vazio."));
            }
        } else if texto::tamanho(&c) > 20 {
            return Err(Erro::campo("codigo", "Código: use no máximo 20 caracteres."));
        } else {
            c
        }
    };
    // Só confere o código quando ele é novo ou foi trocado: uma pessoa que
    // veio da troca de dados com o código igual ao de outra (veja
    // `faixas::repetidos_pessoas`) continua podendo ser alterada.
    if !codigo.is_empty() && codigo != codigo_antes {
        faixas::conferir_digitado_pessoa(conn, &codigo)?;
        let mesmo_codigo: Option<String> = conn
            .query_row("SELECT nome FROM pessoa WHERE codigo = ?1 AND id <> ?2", params![codigo, id], |l| l.get(0))
            .optional()?;
        if let Some(outro) = mesmo_codigo {
            return Err(Erro::campo("codigo", format!("O código {codigo} já é de \"{outro}\".")));
        }
    }
    if !dados.documento.is_empty() {
        let mesmo_doc: Option<String> = conn
            .query_row("SELECT nome FROM pessoa WHERE documento = ?1 AND id <> ?2", params![dados.documento, id], |l| {
                l.get(0)
            })
            .optional()?;
        if let Some(outro) = mesmo_doc {
            return Err(Erro::campo("documento", format!("Este documento já está cadastrado em \"{outro}\".")));
        }
    }
    // O código vale já (de números, ou confirmado aqui pelo caixa que
    // distribui as faixas) ou fica esperando a confirmação dele (veja
    // `faixas`). Com o código igual ao de antes, continua como estava, e o
    // distribuidor confirma se já pode.
    let validado: i64 = if !novo && codigo == codigo_antes {
        let atual: i64 = conn.query_row("SELECT codigo_validado FROM pessoa WHERE id = ?1", [&id], |l| l.get(0))?;
        if atual == 1 {
            1
        } else {
            faixas::confirmado_ao_salvar_pessoa(conn, &id, &codigo)?
        }
    } else {
        faixas::confirmado_ao_salvar_pessoa(conn, &id, &codigo)?
    };
    let detalhes = match &antes {
        Some(a) => {
            let mut d = diferencas(a, &dados);
            if codigo != codigo_antes {
                d.push(format!(
                    "Código: {} → {codigo}",
                    if codigo_antes.is_empty() { "a definir" } else { &codigo_antes }
                ));
            }
            if d.is_empty() {
                return Ok(id);
            }
            d
        }
        None => vec![],
    };
    gravar(conn, dispositivo, usuario_id, &id, Codigo { texto: &codigo, validado }, &dados, novo)?;
    auditoria::registrar(
        conn,
        dispositivo,
        Some(usuario_id),
        if novo { "pessoa.criar" } else { "pessoa.alterar" },
        "pessoa",
        &id,
        &detalhes,
    )?;
    Ok(id)
}

/// O código que vai para o cadastro e se ele já vale (`pessoa.codigo_validado`).
#[derive(Clone, Copy)]
struct Codigo<'a> {
    texto: &'a str,
    validado: i64,
}

fn gravar(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    id: &str,
    codigo: Codigo,
    d: &Dados,
    novo: bool,
) -> Resultado<()> {
    let Codigo { texto: codigo, validado } = codigo;
    let agora = tempo::agora();
    let busca = texto_busca(codigo, d);
    if novo {
        conn.execute(
            "INSERT INTO pessoa (id, codigo, cliente, fornecedor, tipo, nome, fantasia, documento, ie, indicador_ie,
                                 telefone, celular, email, cep, logradouro, numero, complemento, bairro, municipio,
                                 municipio_ibge, uf, limite_credito, observacoes, ativo, busca,
                                 criado_em, criado_por, atualizado_em, atualizado_por, dispositivo, codigo_validado)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                     ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?26, ?27, ?28, ?29)",
            params![
                id,
                codigo,
                d.cliente,
                d.fornecedor,
                d.tipo,
                d.nome,
                d.fantasia,
                d.documento,
                d.ie,
                d.indicador_ie,
                d.telefone,
                d.celular,
                d.email,
                d.cep,
                d.logradouro,
                d.numero,
                d.complemento,
                d.bairro,
                d.municipio,
                d.municipio_ibge,
                d.uf,
                d.limite_credito,
                d.observacoes,
                d.ativo,
                busca,
                agora,
                usuario_id,
                dispositivo,
                validado
            ],
        )?;
    } else {
        conn.execute(
            "UPDATE pessoa SET codigo = ?2, cliente = ?3, fornecedor = ?4, tipo = ?5, nome = ?6, fantasia = ?7,
                    documento = ?8, ie = ?9, indicador_ie = ?10, telefone = ?11, celular = ?12, email = ?13,
                    cep = ?14, logradouro = ?15, numero = ?16, complemento = ?17, bairro = ?18, municipio = ?19,
                    municipio_ibge = ?20, uf = ?21, limite_credito = ?22, observacoes = ?23, ativo = ?24,
                    busca = ?25, atualizado_em = ?26, atualizado_por = ?27, dispositivo = ?28, codigo_validado = ?29
              WHERE id = ?1",
            params![
                id,
                codigo,
                d.cliente,
                d.fornecedor,
                d.tipo,
                d.nome,
                d.fantasia,
                d.documento,
                d.ie,
                d.indicador_ie,
                d.telefone,
                d.celular,
                d.email,
                d.cep,
                d.logradouro,
                d.numero,
                d.complemento,
                d.bairro,
                d.municipio,
                d.municipio_ibge,
                d.uf,
                d.limite_credito,
                d.observacoes,
                d.ativo,
                busca,
                agora,
                usuario_id,
                dispositivo,
                validado
            ],
        )?;
        conn.execute("DELETE FROM pessoa_veiculo WHERE pessoa_id = ?1", [id])?;
    }
    conn.execute("UPDATE pessoa SET dia_vencimento = ?2 WHERE id = ?1", params![id, d.dia_vencimento])?;
    for (ordem, v) in d.veiculos.iter().enumerate() {
        conn.execute(
            "INSERT INTO pessoa_veiculo (id, pessoa_id, ordem, placa, descricao) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![tempo::novo_id(), id, ordem as i64, v.placa, v.descricao],
        )?;
    }
    Ok(())
}

fn carregar(conn: &Connection, id: &str) -> Resultado<(String, Dados)> {
    let (codigo, mut d) = conn
        .query_row(
            "SELECT codigo, cliente, fornecedor, tipo, nome, fantasia, documento, ie, indicador_ie, telefone, celular,
                    email, cep, logradouro, numero, complemento, bairro, municipio, municipio_ibge, uf,
                    limite_credito, observacoes, ativo, dia_vencimento
               FROM pessoa WHERE id = ?1",
            [id],
            |l| {
                Ok((
                    l.get::<_, String>(0)?,
                    Dados {
                        cliente: l.get(1)?,
                        fornecedor: l.get(2)?,
                        tipo: l.get(3)?,
                        nome: l.get(4)?,
                        fantasia: l.get(5)?,
                        documento: l.get(6)?,
                        ie: l.get(7)?,
                        indicador_ie: l.get(8)?,
                        telefone: l.get(9)?,
                        celular: l.get(10)?,
                        email: l.get(11)?,
                        cep: l.get(12)?,
                        logradouro: l.get(13)?,
                        numero: l.get(14)?,
                        complemento: l.get(15)?,
                        bairro: l.get(16)?,
                        municipio: l.get(17)?,
                        municipio_ibge: l.get(18)?,
                        uf: l.get(19)?,
                        limite_credito: l.get(20)?,
                        dia_vencimento: l.get(23)?,
                        observacoes: l.get(21)?,
                        ativo: l.get(22)?,
                        veiculos: vec![],
                    },
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Cadastro".into()))?;
    let mut consulta =
        conn.prepare_cached("SELECT placa, descricao FROM pessoa_veiculo WHERE pessoa_id = ?1 ORDER BY ordem")?;
    d.veiculos = consulta
        .query_map([id], |l| Ok(VeiculoFicha { placa: l.get(0)?, descricao: l.get(1)? }))?
        .collect::<Result<_, _>>()?;
    Ok((codigo, d))
}

/// Ficha para a tela.
pub fn ficha(conn: &Connection, id: &str) -> Resultado<PessoaFicha> {
    let (codigo, d) = carregar(conn, id)?;
    let (criado_em, atualizado_em, atualizado_por): (String, String, String) = conn.query_row(
        "SELECT p.criado_em, p.atualizado_em, COALESCE(u.nome, '')
           FROM pessoa p LEFT JOIN usuario u ON u.id = p.atualizado_por WHERE p.id = ?1",
        [id],
        |l| Ok((l.get(0)?, l.get(1)?, l.get(2)?)),
    )?;
    let compras: i64 =
        conn.query_row("SELECT COUNT(*) FROM compra WHERE fornecedor_id = ?1 AND situacao = 'lancada'", [id], |l| {
            l.get(0)
        })?;
    let codigo_pendente = faixas::pessoa_esperando_confirmacao(conn, id)?;
    let codigo_aviso = if codigo.is_empty() {
        "Cadastro com código a definir: o código sai quando este caixa receber a faixa de códigos.".to_string()
    } else {
        let outro: Option<String> = conn
            .query_row("SELECT nome FROM pessoa WHERE codigo = ?1 AND id <> ?2 LIMIT 1", params![codigo, id], |l| {
                l.get(0)
            })
            .optional()?;
        match (outro, codigo_pendente) {
            (Some(n), _) => {
                format!("O código {codigo} também é de \"{n}\". O gerente pode dar um código novo a um dos dois.")
            }
            (None, true) => format!(
                "O código {codigo} ainda não foi confirmado pelo caixa que distribui os códigos. Até lá, ache este cadastro pelo nome: o código pode ser mudado se outro caixa já tiver escolhido o mesmo."
            ),
            (None, false) => String::new(),
        }
    };
    Ok(PessoaFicha {
        id: id.to_string(),
        codigo,
        codigo_pendente,
        codigo_aviso,
        cliente: d.cliente,
        fornecedor: d.fornecedor,
        tipo: d.tipo,
        nome: d.nome,
        fantasia: d.fantasia,
        documento: documento::escrever(&d.documento),
        ie: d.ie,
        indicador_ie: d.indicador_ie,
        telefone: d.telefone,
        celular: d.celular,
        email: d.email,
        cep: documento::escrever_cep(&d.cep),
        logradouro: d.logradouro,
        numero: d.numero,
        complemento: d.complemento,
        bairro: d.bairro,
        municipio: d.municipio,
        municipio_ibge: d.municipio_ibge,
        uf: d.uf,
        limite_credito: d.limite_credito.map(numeros::escrever_dinheiro).unwrap_or_default(),
        dia_vencimento: d.dia_vencimento.map(|x| x.to_string()).unwrap_or_default(),
        observacoes: d.observacoes,
        ativo: d.ativo,
        veiculos: d
            .veiculos
            .into_iter()
            .map(|v| VeiculoFicha { placa: documento::escrever_placa(&v.placa), descricao: v.descricao })
            .collect(),
        criado_em,
        atualizado_em,
        atualizado_por,
        compras,
        historico: auditoria::historico(conn, "pessoa", id, 30)?,
        codigo_previsto: String::new(),
    })
}

/// Ficha em branco. `papel` = "cliente" ou "fornecedor".
pub fn ficha_nova(conn: &Connection, papel: &str) -> Resultado<PessoaFicha> {
    let fornecedor = papel == "fornecedor";
    let codigo_previsto = faixas::previsto_de(conn, faixas::Uso::Pessoa)?.unwrap_or_default();
    let codigo_aviso = if let Some(m) = faixas::bloqueio(conn)? {
        faixas::texto_do_bloqueio(&m).to_string()
    } else if codigo_previsto.is_empty() && !faixas::sou_distribuidor(conn)? {
        "Este caixa ainda não recebeu a faixa de códigos: o cadastro fica com código a definir até a troca de dados \
         com o caixa que distribui as faixas."
            .to_string()
    } else {
        String::new()
    };
    Ok(PessoaFicha {
        codigo: String::new(),
        codigo_previsto,
        codigo_aviso,
        cliente: !fornecedor,
        fornecedor,
        tipo: if fornecedor { "J".into() } else { "F".into() },
        indicador_ie: 9,
        ativo: true,
        ..Default::default()
    })
}

/// Busca por nome, fantasia, código, CPF/CNPJ, telefone, cidade ou placa.
/// `papel`: "cliente", "fornecedor" ou "" (todos).
pub fn buscar(
    conn: &Connection,
    texto_busca: &str,
    papel: &str,
    inativos: bool,
    limite: i64,
) -> Resultado<Vec<PessoaResumo>> {
    let limite = limite.clamp(1, 1000);
    let normalizado = texto::normalizar(texto_busca);
    let tokens: Vec<String> = normalizado
        .split(' ')
        .filter(|t| !t.is_empty())
        .take(10)
        .map(|t| {
            // CPF/CNPJ/telefone com pontuação: compara só os números e letras
            let limpo = documento::limpar(t).to_lowercase();
            if limpo.len() >= 4 && limpo.chars().any(|c| c.is_ascii_digit()) {
                limpo
            } else {
                t.to_string()
            }
        })
        .collect();
    let mut sql = String::from(
        "SELECT p.id, p.codigo, p.nome, p.fantasia, p.documento, CASE WHEN p.celular <> '' THEN p.celular ELSE p.telefone END,
                CASE WHEN p.municipio <> '' THEN p.municipio || CASE WHEN p.uf <> '' THEN ' · ' || p.uf ELSE '' END ELSE '' END,
                p.cliente, p.fornecedor, p.ativo,
                (SELECT group_concat(placa, ' · ') FROM pessoa_veiculo v WHERE v.pessoa_id = p.id AND placa <> ''),
                p.codigo_validado = 0 AND p.codigo <> ''
           FROM pessoa p WHERE 1 = 1",
    );
    let mut valores: Vec<Value> = Vec::new();
    match papel {
        "cliente" => sql.push_str(" AND p.cliente = 1"),
        "fornecedor" => sql.push_str(" AND p.fornecedor = 1"),
        _ => {}
    }
    if !inativos {
        sql.push_str(" AND p.ativo = 1");
    }
    for t in &tokens {
        valores.push(Value::Text(format!("%{}%", t.to_lowercase())));
        sql.push_str(&format!(" AND lower(p.busca) LIKE ?{}", valores.len()));
    }
    if tokens.is_empty() {
        sql.push_str(" ORDER BY p.atualizado_em DESC");
    } else {
        sql.push_str(" ORDER BY p.ativo DESC, p.nome COLLATE NOCASE");
    }
    sql.push_str(&format!(" LIMIT {limite}"));
    let mut consulta = conn.prepare(&sql)?;
    let linhas = consulta.query_map(params_from_iter(valores), |l| {
        let placas: Option<String> = l.get(10)?;
        Ok(PessoaResumo {
            id: l.get(0)?,
            codigo: l.get(1)?,
            codigo_pendente: l.get(11)?,
            nome: l.get(2)?,
            fantasia: l.get(3)?,
            documento: documento::escrever(&l.get::<_, String>(4)?),
            telefone: l.get(5)?,
            cidade: l.get(6)?,
            cliente: l.get(7)?,
            fornecedor: l.get(8)?,
            ativo: l.get(9)?,
            veiculos: placas
                .unwrap_or_default()
                .split(" · ")
                .filter(|p| !p.is_empty())
                .map(documento::escrever_placa)
                .collect::<Vec<_>>()
                .join(" · "),
        })
    })?;
    Ok(linhas.collect::<Result<_, _>>()?)
}

/// Quantas pessoas existem com esse papel.
pub fn total(conn: &Connection, papel: &str) -> Resultado<i64> {
    let sql = match papel {
        "cliente" => "SELECT COUNT(*) FROM pessoa WHERE cliente = 1 AND ativo = 1",
        "fornecedor" => "SELECT COUNT(*) FROM pessoa WHERE fornecedor = 1 AND ativo = 1",
        _ => "SELECT COUNT(*) FROM pessoa WHERE ativo = 1",
    };
    Ok(conn.query_row(sql, [], |l| l.get(0))?)
}

/// Dados do emitente de uma nota de compra.
#[derive(Debug, Default, Clone)]
pub struct Emitente {
    pub documento: String,
    pub nome: String,
    pub fantasia: String,
    pub ie: String,
    pub email: String,
    pub telefone: String,
    pub cep: String,
    pub logradouro: String,
    pub numero: String,
    pub complemento: String,
    pub bairro: String,
    pub municipio: String,
    pub municipio_ibge: String,
    pub uf: String,
}

/// Acha o fornecedor pelo CNPJ/CPF ou cadastra com os dados da nota.
/// Se já existe só como cliente, passa a ser fornecedor também.
/// Devolve (id, foi_criado).
pub fn garantir_fornecedor(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    e: &Emitente,
) -> Resultado<(String, bool)> {
    garantir_pela_nota(conn, dispositivo, usuario_id, e, false, "Cadastrado pela nota de compra")
}

/// Acha a pessoa pelo CPF/CNPJ ou cadastra com os dados da nota, como
/// cliente (`cliente` = true) ou fornecedor. Se já existe com o outro papel,
/// passa a ter os dois. Devolve (id, foi_criado).
pub fn garantir_pela_nota(
    conn: &Connection,
    dispositivo: &str,
    usuario_id: &str,
    e: &Emitente,
    cliente: bool,
    origem: &str,
) -> Resultado<(String, bool)> {
    let doc = documento::limpar(&e.documento);
    if !doc.is_empty() {
        let achou: Option<(String, bool, bool)> = conn
            .query_row("SELECT id, cliente, fornecedor FROM pessoa WHERE documento = ?1", [&doc], |l| {
                Ok((l.get(0)?, l.get(1)?, l.get(2)?))
            })
            .optional()?;
        if let Some((id, eh_cliente, eh_fornecedor)) = achou {
            if cliente && !eh_cliente {
                conn.execute("UPDATE pessoa SET cliente = 1 WHERE id = ?1", [&id])?;
                auditoria::registrar(
                    conn,
                    dispositivo,
                    Some(usuario_id),
                    "pessoa.alterar",
                    "pessoa",
                    &id,
                    &["Passou a ser cliente".into()],
                )?;
            }
            if !cliente && !eh_fornecedor {
                conn.execute("UPDATE pessoa SET fornecedor = 1 WHERE id = ?1", [&id])?;
                auditoria::registrar(
                    conn,
                    dispositivo,
                    Some(usuario_id),
                    "pessoa.alterar",
                    "pessoa",
                    &id,
                    &["Passou a ser fornecedor".into()],
                )?;
            }
            return Ok((id, false));
        }
    }
    let dados = Dados {
        cliente,
        fornecedor: !cliente,
        tipo: if doc.len() == 11 { "F".into() } else { "J".into() },
        nome: corta(&texto::limpar(&e.nome), 100),
        fantasia: corta(&texto::limpar(&e.fantasia), 60),
        documento: doc,
        ie: corta(&documento::limpar(&e.ie), 14),
        indicador_ie: if documento::limpar(&e.ie).is_empty() { 9 } else { 1 },
        telefone: corta(&documento::escrever_telefone(&e.telefone), 20),
        celular: String::new(),
        email: corta(e.email.trim(), 100),
        cep: documento::digitos(&e.cep),
        logradouro: corta(&texto::limpar(&e.logradouro), 80),
        numero: corta(&texto::limpar(&e.numero), 20),
        complemento: corta(&texto::limpar(&e.complemento), 60),
        bairro: corta(&texto::limpar(&e.bairro), 60),
        municipio: corta(&texto::limpar(&e.municipio), 60),
        municipio_ibge: documento::digitos(&e.municipio_ibge),
        uf: e.uf.to_uppercase(),
        limite_credito: None,
        dia_vencimento: None,
        observacoes: String::new(),
        ativo: true,
        veiculos: vec![],
    };
    let nome = if !dados.nome.is_empty() {
        dados.nome.clone()
    } else if cliente {
        "Cliente sem nome".to_string()
    } else {
        "Fornecedor sem nome".to_string()
    };
    let dados = Dados { nome, ..dados };
    let id = tempo::novo_id();
    let codigo = codigo_novo(conn)?;
    gravar(conn, dispositivo, usuario_id, &id, Codigo { texto: &codigo, validado: 1 }, &dados, true)?;
    auditoria::registrar(conn, dispositivo, Some(usuario_id), "pessoa.criar", "pessoa", &id, &[origem.to_string()])?;
    Ok((id, true))
}

fn corta(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn diferencas(a: &Dados, b: &Dados) -> Vec<String> {
    let mut v = Vec::new();
    let mut campo = |rotulo: &str, x: &str, y: &str| {
        if x != y {
            let t = |s: &str| if s.is_empty() { "—".to_string() } else { s.to_string() };
            v.push(format!("{rotulo}: {} → {}", t(x), t(y)));
        }
    };
    campo("Nome", &a.nome, &b.nome);
    campo("Fantasia", &a.fantasia, &b.fantasia);
    campo("CPF/CNPJ", &documento::escrever(&a.documento), &documento::escrever(&b.documento));
    campo("IE", &a.ie, &b.ie);
    campo("Telefone", &a.telefone, &b.telefone);
    campo("Celular", &a.celular, &b.celular);
    campo("E-mail", &a.email, &b.email);
    campo("CEP", &a.cep, &b.cep);
    campo("Endereço", &a.logradouro, &b.logradouro);
    campo("Número", &a.numero, &b.numero);
    campo("Complemento", &a.complemento, &b.complemento);
    campo("Bairro", &a.bairro, &b.bairro);
    campo("Cidade", &a.municipio, &b.municipio);
    campo("UF", &a.uf, &b.uf);
    let lim = |x: Option<i64>| x.map(numeros::escrever_dinheiro).unwrap_or_default();
    campo("Limite de crédito", &lim(a.limite_credito), &lim(b.limite_credito));
    let dia = |x: Option<i64>| x.map(|d| d.to_string()).unwrap_or_default();
    campo("Dia do vencimento", &dia(a.dia_vencimento), &dia(b.dia_vencimento));
    if a.cliente != b.cliente {
        v.push(if b.cliente { "Passou a ser cliente".into() } else { "Deixou de ser cliente".into() });
    }
    if a.fornecedor != b.fornecedor {
        v.push(if b.fornecedor { "Passou a ser fornecedor".into() } else { "Deixou de ser fornecedor".into() });
    }
    if a.observacoes != b.observacoes {
        v.push("Observações alteradas".into());
    }
    if a.veiculos != b.veiculos {
        v.push(format!("Carros: {} → {}", a.veiculos.len(), b.veiculos.len()));
    }
    if a.ativo != b.ativo {
        v.push(if b.ativo { "Reativado".into() } else { "Desativado".into() });
    }
    if v.is_empty() && (a.tipo != b.tipo || a.indicador_ie != b.indicador_ie || a.municipio_ibge != b.municipio_ibge) {
        v.push("Dados fiscais alterados".into());
    }
    v
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    fn base() -> (Banco, String) {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap();
        (b, u.id)
    }

    #[test]
    fn cadastra_busca_e_altera() {
        let (b, u) = base();
        let mut f = ficha_nova(&b.conn, "cliente").unwrap();
        f.nome = "  José   da Silva ".into();
        f.documento = "529.982.247-25".into();
        f.celular = "(15) 99999-1234".into();
        f.municipio = "Avaré".into();
        f.uf = "sp".into();
        f.veiculos = vec![VeiculoFicha { placa: "abc-1234".into(), descricao: "Gol G5 1.6".into() }];
        let id = salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
        let lida = ficha(&b.conn, &id).unwrap();
        assert_eq!(lida.nome, "José da Silva");
        assert_eq!(lida.documento, "529.982.247-25");
        assert_eq!(lida.tipo, "F");
        assert_eq!(lida.uf, "SP");
        assert_eq!(lida.veiculos[0].placa, "ABC-1234");

        for termo in ["jose", "52998224725", "529.982", "abc1234", "avare", "99999"] {
            let r = buscar(&b.conn, termo, "cliente", false, 50).unwrap();
            assert_eq!(r.len(), 1, "busca {termo}");
        }
        assert!(buscar(&b.conn, "jose", "fornecedor", false, 50).unwrap().is_empty());

        // documento repetido
        let mut outra = ficha_nova(&b.conn, "cliente").unwrap();
        outra.nome = "Outro".into();
        outra.documento = "52998224725".into();
        let e = salvar(&b.conn, &b.dispositivo, &u, &outra).unwrap_err();
        assert_eq!(e.nome_do_campo(), Some("documento"));

        // alterar registra no histórico
        let mut f2 = lida.clone();
        f2.telefone = "(15) 3552-0000".into();
        salvar(&b.conn, &b.dispositivo, &u, &f2).unwrap();
        let h = ficha(&b.conn, &id).unwrap().historico;
        assert!(h[0].detalhes.iter().any(|d| d.starts_with("Telefone")));
    }

    #[test]
    fn confere_campos() {
        let (b, u) = base();
        let mut f = ficha_nova(&b.conn, "fornecedor").unwrap();
        f.nome = "Distribuidora".into();
        f.documento = "11.222.333/0001-82".into();
        assert_eq!(salvar(&b.conn, &b.dispositivo, &u, &f).unwrap_err().nome_do_campo(), Some("documento"));
        f.documento = "11.222.333/0001-81".into();
        f.email = "errado@".into();
        assert_eq!(salvar(&b.conn, &b.dispositivo, &u, &f).unwrap_err().nome_do_campo(), Some("email"));
        f.email = "vendas@dist.com.br".into();
        f.cliente = false;
        f.fornecedor = false;
        assert_eq!(salvar(&b.conn, &b.dispositivo, &u, &f).unwrap_err().nome_do_campo(), Some("cliente"));
        f.fornecedor = true;
        let id = salvar(&b.conn, &b.dispositivo, &u, &f).unwrap();
        assert_eq!(ficha(&b.conn, &id).unwrap().tipo, "J");
    }

    #[test]
    fn fornecedor_da_nota() {
        let (b, u) = base();
        let e = Emitente {
            documento: "11222333000181".into(),
            nome: "Distribuidora Teste Ltda".into(),
            uf: "SP".into(),
            ..Default::default()
        };
        let (id, criado) = garantir_fornecedor(&b.conn, &b.dispositivo, &u, &e).unwrap();
        assert!(criado);
        let (id2, criado2) = garantir_fornecedor(&b.conn, &b.dispositivo, &u, &e).unwrap();
        assert_eq!(id, id2);
        assert!(!criado2);
        assert!(ficha(&b.conn, &id).unwrap().fornecedor);
    }
}
