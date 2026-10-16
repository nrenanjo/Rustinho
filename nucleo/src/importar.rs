//! Cadastros a partir de XMLs de notas (NF-e e NFC-e).
//!
//! Serve para os dois casos:
//! - **Notas da própria loja** (o sistema antigo exporta os XMLs das notas
//!   emitidas): o cliente vem do destinatário e a peça vem com o código
//!   interno da loja e o último preço de venda.
//! - **Notas de fornecedores**: o fornecedor vem do emitente e a peça vem
//!   com o código do fornecedor (fica ligado à peça) e o custo.
//!
//! Nada que já existe é mudado: só cadastra o que falta. A mesma pessoa é
//! achada pelo CPF/CNPJ; a mesma peça pelo código interno (notas da loja),
//! pelo código do fornecedor ou pelo código de barras.
//!
//! Aceita arquivos .xml, .zip e .rar com XMLs dentro e pastas (com
//! subpastas).

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::erro::{Erro, Resultado};
use crate::nfe_xml::{self, ItemXml, NotaXml};
use crate::pessoas::{self, Emitente};
use crate::{arquivos, auditoria, config, documento, gtin, numeros, produtos, tempo, texto};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entrada {
    pub arquivos: Vec<String>,
    pub clientes: bool,
    pub fornecedores: bool,
    pub pecas_loja: bool,
    pub pecas_fornecedor: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Previa {
    /// XMLs encontrados (soltos, nas pastas e nos .zip).
    pub arquivos: i64,
    /// Notas lidas (NF-e ou NFC-e).
    pub notas: i64,
    /// Notas emitidas pela loja e notas de fornecedores.
    pub notas_loja: i64,
    pub notas_fornecedor: i64,
    /// Arquivos que não eram nota (eventos, CT-e, outros) ou com erro.
    pub ignorados: i64,
    pub avisos: Vec<String>,
    pub clientes_novos: i64,
    pub clientes_ja: i64,
    pub fornecedores_novos: i64,
    pub fornecedores_ja: i64,
    pub pecas_loja_novas: i64,
    pub pecas_fornecedor_novas: i64,
    pub pecas_ja: i64,
    pub exemplos_clientes: Vec<String>,
    pub exemplos_fornecedores: Vec<String>,
    pub exemplos_pecas: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Importado {
    pub clientes: i64,
    pub fornecedores: i64,
    pub pecas: i64,
    pub codigos_ligados: i64,
}

/// O que foi lido das notas, já sem repetição.
#[derive(Default)]
struct Lido {
    arquivos: i64,
    ignorados: i64,
    avisos: Vec<String>,
    notas_loja: i64,
    notas_fornecedor: i64,
    /// documento → (pessoa, emissão da nota mais nova)
    clientes: HashMap<String, (Emitente, String)>,
    fornecedores: HashMap<String, (Emitente, String)>,
    /// código interno da loja → (item, emissão)
    pecas_loja: BTreeMap<String, (ItemXml, String)>,
    /// (documento do fornecedor, código do fornecedor) → (item, emissão)
    pecas_fornecedor: BTreeMap<(String, String), (ItemXml, String)>,
}

// ---------------------------------------------------------------------------
// Achar e ler os arquivos (soltos, pastas, .zip e .rar): ver `arquivos`.
// ---------------------------------------------------------------------------

/// Chama `cada` com (nome do arquivo, conteúdo) de todos os XMLs achados.
fn cada_xml(entradas: &[String], mut cada: impl FnMut(&str, &str)) -> Resultado<i64> {
    let lista = arquivos::xmls(entradas)?;
    let lidos = lista.len() as i64;
    for x in lista {
        cada(&x.nome, &x.texto);
    }
    Ok(lidos)
}

fn cnpj_loja(conn: &Connection) -> Resultado<String> {
    let cnpj = documento::limpar(&config::ler(conn, "loja_cnpj")?.unwrap_or_default());
    if cnpj.is_empty() {
        return Err(Erro::validacao(
            "Informe o CNPJ da loja em Configurações → Loja. É por ele que o Rustinho separa as notas da loja das notas de fornecedores.",
        ));
    }
    Ok(cnpj)
}

fn guardar_mais_nova<K: Ord + std::hash::Hash + Eq, V>(mapa: &mut HashMap<K, (V, String)>, chave: K, valor: V, emissao: &str) {
    match mapa.get(&chave) {
        Some((_, e)) if e.as_str() >= emissao => {}
        _ => {
            mapa.insert(chave, (valor, emissao.to_string()));
        }
    }
}

fn guardar_mais_nova_b<K: Ord, V>(mapa: &mut BTreeMap<K, (V, String)>, chave: K, valor: V, emissao: &str) {
    match mapa.get(&chave) {
        Some((_, e)) if e.as_str() >= emissao => {}
        _ => {
            mapa.insert(chave, (valor, emissao.to_string()));
        }
    }
}

fn ler_tudo(conn: &Connection, arquivos: &[String]) -> Resultado<Lido> {
    if arquivos.is_empty() {
        return Err(Erro::validacao("Escolha os arquivos XML, um .zip, um .rar ou uma pasta."));
    }
    let loja = cnpj_loja(conn)?;
    let mut lido = Lido::default();
    let mut chaves: HashSet<String> = HashSet::new();
    let total = cada_xml(arquivos, |nome, xml| {
        let nota: NotaXml = match nfe_xml::ler_cadastro(xml) {
            Ok(n) => n,
            Err(e) => {
                lido.ignorados += 1;
                if lido.avisos.len() < 5 && !e.contains("evento") && !e.contains("CT-e") {
                    lido.avisos.push(format!("{nome}: {e}"));
                }
                return;
            }
        };
        if !nota.chave.is_empty() && !chaves.insert(nota.chave.clone()) {
            return; // a mesma nota em dois arquivos
        }
        let emitente = documento::limpar(&nota.emitente.documento);
        let destinatario = documento::limpar(&nota.destinatario);
        if emitente == loja {
            lido.notas_loja += 1;
            // Devolução para fornecedor: o destinatário não é cliente.
            if nota.finalidade == "4" {
                return;
            }
            if !destinatario.is_empty() && destinatario != loja {
                guardar_mais_nova(&mut lido.clientes, destinatario, nota.destinatario_dados.clone(), &nota.emissao);
            }
            for it in &nota.itens {
                let cod = it.codigo.trim().to_string();
                if !cod.is_empty() {
                    guardar_mais_nova_b(&mut lido.pecas_loja, cod, it.clone(), &nota.emissao);
                }
            }
        } else {
            lido.notas_fornecedor += 1;
            if emitente.is_empty() || nota.finalidade == "4" {
                return;
            }
            guardar_mais_nova(&mut lido.fornecedores, emitente.clone(), nota.emitente.clone(), &nota.emissao);
            for it in &nota.itens {
                let cod = it.codigo.trim().to_string();
                if !cod.is_empty() {
                    guardar_mais_nova_b(&mut lido.pecas_fornecedor, (emitente.clone(), cod), it.clone(), &nota.emissao);
                }
            }
        }
    })?;
    lido.arquivos = total;
    Ok(lido)
}

// ---------------------------------------------------------------------------
// Conferir o que já existe
// ---------------------------------------------------------------------------

fn pessoa_existe(conn: &Connection, doc: &str) -> Resultado<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM pessoa WHERE documento = ?1", [doc], |l| l.get(0))
        .optional()?)
}

fn peca_por_codigo(conn: &Connection, codigo: &str) -> Resultado<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM produto WHERE codigo = ?1", [codigo], |l| l.get(0))
        .optional()?)
}

fn peca_por_barras(conn: &Connection, gtin_: &str) -> Resultado<Option<String>> {
    if gtin_.is_empty() || !gtin::gtin_valido(gtin_) {
        return Ok(None);
    }
    Ok(conn
        .query_row("SELECT id FROM produto WHERE codigo_barras = ?1", [gtin_], |l| l.get(0))
        .optional()?)
}

fn peca_por_fornecedor(conn: &Connection, fornecedor_doc: &str, codigo: &str) -> Resultado<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT f.produto_id FROM produto_fornecedor f JOIN pessoa p ON p.id = f.fornecedor_id
              WHERE p.documento = ?1 AND f.codigo_compacto = ?2",
            params![fornecedor_doc, texto::compactar(codigo)],
            |l| l.get(0),
        )
        .optional()?)
}

pub fn previa(conn: &Connection, arquivos: &[String]) -> Resultado<Previa> {
    let lido = ler_tudo(conn, arquivos)?;
    let mut p = Previa {
        arquivos: lido.arquivos,
        notas: lido.notas_loja + lido.notas_fornecedor,
        notas_loja: lido.notas_loja,
        notas_fornecedor: lido.notas_fornecedor,
        ignorados: lido.ignorados,
        avisos: lido.avisos.clone(),
        ..Default::default()
    };
    let mut nomes: Vec<(String, String)> = Vec::new();
    for (doc, (e, _)) in &lido.clientes {
        if pessoa_existe(conn, doc)?.is_some() {
            p.clientes_ja += 1;
        } else {
            p.clientes_novos += 1;
            nomes.push((e.nome.clone(), doc.clone()));
        }
    }
    nomes.sort();
    p.exemplos_clientes = nomes.iter().take(6).map(|n| n.0.clone()).collect();
    let mut nomes: Vec<String> = Vec::new();
    for (doc, (e, _)) in &lido.fornecedores {
        if pessoa_existe(conn, doc)?.is_some() {
            p.fornecedores_ja += 1;
        } else {
            p.fornecedores_novos += 1;
            nomes.push(if e.fantasia.is_empty() { e.nome.clone() } else { e.fantasia.clone() });
        }
    }
    nomes.sort();
    p.exemplos_fornecedores = nomes.into_iter().take(6).collect();

    let mut barras_novas: HashSet<String> = HashSet::new();
    let mut exemplos: Vec<String> = Vec::new();
    for (codigo, (it, _)) in &lido.pecas_loja {
        if peca_por_codigo(conn, codigo)?.is_some() || peca_por_barras(conn, &it.gtin)?.is_some() {
            p.pecas_ja += 1;
        } else {
            p.pecas_loja_novas += 1;
            if !it.gtin.is_empty() {
                barras_novas.insert(it.gtin.clone());
            }
            if exemplos.len() < 6 {
                exemplos.push(texto::limpar(&it.descricao));
            }
        }
    }
    for ((forn, codigo), (it, _)) in &lido.pecas_fornecedor {
        if peca_por_fornecedor(conn, forn, codigo)?.is_some()
            || peca_por_barras(conn, &it.gtin)?.is_some()
            || (!it.gtin.is_empty() && barras_novas.contains(&it.gtin))
        {
            p.pecas_ja += 1;
        } else {
            p.pecas_fornecedor_novas += 1;
            if !it.gtin.is_empty() {
                barras_novas.insert(it.gtin.clone());
            }
            if exemplos.len() < 6 {
                exemplos.push(texto::limpar(&it.descricao));
            }
        }
    }
    p.exemplos_pecas = exemplos;
    Ok(p)
}

// ---------------------------------------------------------------------------
// Cadastrar
// ---------------------------------------------------------------------------

/// Valor de uma unidade do item (em centavos): o que a nota cobrou por
/// unidade, já com frete, IPI e ST e menos o desconto.
fn valor_unitario(it: &ItemXml, com_impostos: bool) -> i64 {
    let total = if com_impostos {
        it.valor_produtos + it.frete + it.seguro + it.outras + it.ipi + it.icms_st - it.desconto
    } else {
        it.valor_produtos - it.desconto
    };
    if it.quantidade <= 0 {
        return 0;
    }
    ((total as i128 * 1000 + it.quantidade as i128 / 2) / it.quantidade as i128) as i64
}

fn ficha_do_item(conn: &Connection, it: &ItemXml) -> Resultado<produtos::ProdutoFicha> {
    let mut f = produtos::ficha_nova(conn)?;
    f.nome = texto::limpar(&it.descricao).chars().take(120).collect();
    if f.nome.is_empty() {
        f.nome = "Peça sem nome".into();
    }
    if !it.gtin.is_empty() && gtin::gtin_valido(&it.gtin) && peca_por_barras(conn, &it.gtin)?.is_none() {
        f.codigo_barras = it.gtin.clone();
    }
    f.unidade = crate::unidades::de_fora(&it.unidade);
    if it.ncm.len() == 8 && it.ncm.chars().all(|c| c.is_ascii_digit()) {
        f.ncm = it.ncm.clone();
    }
    if it.cest.len() == 7 && it.cest.chars().all(|c| c.is_ascii_digit()) {
        f.cest = it.cest.clone();
    }
    if (0..produtos::ORIGENS.len() as i64).contains(&it.origem) {
        f.origem = it.origem;
    }
    f.monofasico = it.cst_pis == "04";
    Ok(f)
}

fn ligar_fornecedor(conn: &Connection, fornecedor_id: &str, codigo: &str, produto_id: &str) -> Resultado<bool> {
    let compacto = texto::compactar(codigo);
    if compacto.is_empty() {
        return Ok(false);
    }
    let n = conn.execute(
        "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, fator, atualizado_em)
         VALUES (?1, ?2, ?3, ?4, 1000, ?5)
         ON CONFLICT (fornecedor_id, codigo_compacto) DO NOTHING",
        params![fornecedor_id, compacto, codigo, produto_id, tempo::agora()],
    )?;
    Ok(n > 0)
}

pub fn importar(conn: &Connection, dispositivo: &str, usuario_id: &str, e: &Entrada) -> Resultado<Importado> {
    let lido = ler_tudo(conn, &e.arquivos)?;
    let mut r = Importado::default();

    if e.clientes {
        let mut lista: Vec<_> = lido.clientes.iter().collect();
        lista.sort_by(|a, b| a.0.cmp(b.0));
        for (doc, (pessoa, _)) in lista {
            if pessoa_existe(conn, doc)?.is_none() {
                pessoas::garantir_pela_nota(conn, dispositivo, usuario_id, pessoa, true, "Cadastrado pelo XML de uma nota da loja")?;
                r.clientes += 1;
            }
        }
    }

    // Fornecedores: cadastra os que faltam (sempre que for cadastrar as
    // peças deles também, para ligar o código).
    let mut ids_fornecedor: HashMap<String, String> = HashMap::new();
    let mut lista: Vec<_> = lido.fornecedores.iter().collect();
    lista.sort_by(|a, b| a.0.cmp(b.0));
    for (doc, (pessoa, _)) in lista {
        match pessoa_existe(conn, doc)? {
            Some(id) => {
                ids_fornecedor.insert(doc.clone(), id);
            }
            None if e.fornecedores || e.pecas_fornecedor => {
                let (id, _) =
                    pessoas::garantir_pela_nota(conn, dispositivo, usuario_id, pessoa, false, "Cadastrado pelo XML de uma nota de compra")?;
                ids_fornecedor.insert(doc.clone(), id);
                r.fornecedores += 1;
            }
            None => {}
        }
    }

    if e.pecas_loja {
        for (codigo, (it, _)) in &lido.pecas_loja {
            if peca_por_codigo(conn, codigo)?.is_some() || peca_por_barras(conn, &it.gtin)?.is_some() {
                continue;
            }
            let mut f = ficha_do_item(conn, it)?;
            // O código da loja só se usar letras, números e traço, sem repetir.
            let cod = texto::limpar(codigo);
            if !cod.is_empty() && cod.chars().count() <= 20 && peca_por_codigo(conn, &cod)?.is_none() {
                f.codigo = cod;
            }
            let preco = valor_unitario(it, false);
            if preco > 0 {
                if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                    p.preco = numeros::escrever_dinheiro(preco);
                    p.lucro = String::new();
                }
            }
            let id = salvar_peca(conn, dispositivo, usuario_id, f)?;
            auditoria::registrar(conn, dispositivo, Some(usuario_id), "produto.importar", "produto", &id,
                &["Cadastrada pelo XML de uma nota da loja".into()])?;
            r.pecas += 1;
        }
    }

    if e.pecas_fornecedor {
        for ((forn, codigo), (it, _)) in &lido.pecas_fornecedor {
            let Some(fornecedor_id) = ids_fornecedor.get(forn) else { continue };
            if peca_por_fornecedor(conn, forn, codigo)?.is_some() {
                continue;
            }
            // Já existe pelo código de barras: só liga o código do fornecedor.
            if let Some(pid) = peca_por_barras(conn, &it.gtin)? {
                if ligar_fornecedor(conn, fornecedor_id, codigo, &pid)? {
                    r.codigos_ligados += 1;
                }
                continue;
            }
            let mut f = ficha_do_item(conn, it)?;
            let custo = valor_unitario(it, true);
            if custo > 0 {
                f.custo = numeros::escrever_dinheiro(custo);
                if let Some(p) = f.precos.iter_mut().find(|p| p.tabela == 1) {
                    if p.lucro.is_empty() {
                        p.lucro = "100".into();
                    }
                }
            }
            let id = salvar_peca(conn, dispositivo, usuario_id, f)?;
            ligar_fornecedor(conn, fornecedor_id, codigo, &id)?;
            auditoria::registrar(conn, dispositivo, Some(usuario_id), "produto.importar", "produto", &id,
                &["Cadastrada pelo XML de uma nota de compra".into()])?;
            r.pecas += 1;
        }
    } else {
        // Mesmo sem cadastrar peças novas, liga o código do fornecedor às
        // peças que já existem (pelo código de barras): ajuda na próxima compra.
        for ((forn, codigo), (it, _)) in &lido.pecas_fornecedor {
            let Some(fornecedor_id) = ids_fornecedor.get(forn) else { continue };
            if peca_por_fornecedor(conn, forn, codigo)?.is_some() {
                continue;
            }
            if let Some(pid) = peca_por_barras(conn, &it.gtin)? {
                if ligar_fornecedor(conn, fornecedor_id, codigo, &pid)? {
                    r.codigos_ligados += 1;
                }
            }
        }
    }
    Ok(r)
}

/// Salva a peça; se o código de barras ou o código já existirem (corrida
/// com outra peça da mesma importação), tenta sem eles.
fn salvar_peca(conn: &Connection, dispositivo: &str, usuario_id: &str, mut f: produtos::ProdutoFicha) -> Resultado<String> {
    match produtos::salvar(conn, dispositivo, usuario_id, &f) {
        Ok(id) => Ok(id),
        Err(Erro::Validacao { campo: Some(c), .. }) if c == "codigoBarras" || c == "codigo" => {
            f.codigo_barras = String::new();
            f.codigo = produtos::proximo_codigo(conn)?;
            produtos::salvar(conn, dispositivo, usuario_id, &f)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::banco::Banco;
    use crate::usuarios;

    const NOTA_LOJA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<nfeProc><NFe><infNFe Id="NFe35261011222333000181550010000001231000001230">
 <ide><mod>55</mod><serie>1</serie><nNF>123</nNF><dhEmi>2026-09-10T10:00:00-03:00</dhEmi><tpNF>1</tpNF><finNFe>1</finNFe><natOp>VENDA</natOp></ide>
 <emit><CNPJ>11444777000161</CNPJ><xNome>LOJA MODELO AUTO PECAS</xNome><enderEmit><UF>SP</UF></enderEmit></emit>
 <dest><CPF>52998224725</CPF><xNome>JOAO DA SILVA</xNome><enderDest><xLgr>RUA A</xLgr><nro>10</nro><xBairro>CENTRO</xBairro><xMun>AVARE</xMun><UF>SP</UF><fone>15999990000</fone></enderDest><email>joao@exemplo.com</email></dest>
 <det nItem="1"><prod><cProd>4455</cProd><cEAN>SEM GTIN</cEAN><xProd>FILTRO DE OLEO PSL55</xProd><NCM>84212300</NCM><CFOP>5102</CFOP><uCom>UN</uCom><qCom>2.0000</qCom><vProd>60.00</vProd></prod>
  <imposto><ICMS><ICMSSN102><orig>0</orig><CSOSN>102</CSOSN></ICMSSN102></ICMS></imposto></det>
</infNFe></NFe></nfeProc>"#;

    #[test]
    fn importa_clientes_pecas_e_fornecedores() {
        let b = Banco::em_memoria().unwrap();
        let (c, d) = (&b.conn, b.dispositivo.as_str());
        let u = usuarios::criar(c, d, "Maria", "1234", true).unwrap().id;
        let pasta = std::env::temp_dir().join(format!("rustinho-importar-{}", tempo::novo_id()));
        std::fs::create_dir_all(pasta.join("sub")).unwrap();
        std::fs::write(pasta.join("venda.xml"), NOTA_LOJA).unwrap();
        std::fs::write(pasta.join("sub").join("compra.xml"), nfe_xml::EXEMPLO).unwrap();
        std::fs::write(pasta.join("evento.xml"), "<procEventoNFe><evento/></procEventoNFe>").unwrap();
        let arquivos = vec![pasta.display().to_string()];

        // Sem o CNPJ da loja não dá para separar as notas.
        assert!(previa(c, &arquivos).is_err());
        config::gravar(c, "loja_cnpj", "11444777000161").unwrap();

        let p = previa(c, &arquivos).unwrap();
        assert_eq!((p.arquivos, p.notas, p.notas_loja, p.notas_fornecedor, p.ignorados), (3, 2, 1, 1, 1));
        assert_eq!((p.clientes_novos, p.fornecedores_novos), (1, 1));
        assert_eq!(p.pecas_loja_novas, 1);
        assert_eq!(p.pecas_fornecedor_novas, 3);
        assert_eq!(p.exemplos_clientes, vec!["JOAO DA SILVA".to_string()]);

        let e = Entrada { arquivos: arquivos.clone(), clientes: true, fornecedores: true, pecas_loja: true, pecas_fornecedor: true };
        let r = importar(c, d, &u, &e).unwrap();
        assert_eq!((r.clientes, r.fornecedores, r.pecas), (1, 1, 4));

        // A peça da loja ficou com o código e o preço da nota.
        let (codigo, preco): (String, i64) = c
            .query_row(
                "SELECT p.codigo, pp.preco FROM produto p JOIN produto_preco pp ON pp.produto_id = p.id AND pp.tabela = 1
                  WHERE p.nome = 'FILTRO DE OLEO PSL55'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert_eq!((codigo.as_str(), preco), ("4455", 3000));
        // O cliente veio com o e-mail e a cidade.
        let (cliente, cidade, email): (bool, String, String) = c
            .query_row("SELECT cliente, municipio, email FROM pessoa WHERE documento = '52998224725'", [], |l| {
                Ok((l.get(0)?, l.get(1)?, l.get(2)?))
            })
            .unwrap();
        assert!(cliente);
        assert_eq!((cidade.as_str(), email.as_str()), ("AVARE", "joao@exemplo.com"));

        // De novo: nada para cadastrar.
        let p = previa(c, &arquivos).unwrap();
        assert_eq!((p.clientes_novos, p.fornecedores_novos, p.pecas_loja_novas, p.pecas_fornecedor_novas), (0, 0, 0, 0));
        assert_eq!(p.pecas_ja, 4);
        let r = importar(c, d, &u, &e).unwrap();
        assert_eq!((r.clientes, r.fornecedores, r.pecas), (0, 0, 0));

        // Os mesmos XMLs dentro de um .zip (e a mesma nota não conta duas vezes).
        let zip_caminho = pasta.join("notas.zip");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_caminho).unwrap());
            let op = zip::write::SimpleFileOptions::default();
            z.start_file("2026/09/venda.xml", op).unwrap();
            z.write_all(NOTA_LOJA.as_bytes()).unwrap();
            z.start_file("leia-me.txt", op).unwrap();
            z.write_all(b"nada").unwrap();
            z.finish().unwrap();
        }
        let p = previa(c, &[format!("file://{}", zip_caminho.display())]).unwrap();
        assert_eq!((p.arquivos, p.notas_loja, p.clientes_ja, p.pecas_ja), (1, 1, 1, 1));
        let p = previa(c, &arquivos).unwrap();
        assert_eq!((p.arquivos, p.notas), (4, 2));
        std::fs::remove_dir_all(&pasta).ok();
    }
}
