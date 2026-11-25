//! R12 (DP-41): o mesmo código do fornecedor `(fornecedor, código)` ligado a peças diferentes (ou com fatores
//! diferentes) em dois caixas sem rede. Exigido (roadmap da 007, desenho v3 e notas 5 e 6), com dois bancos de verdade:
//!
//! - a troca deixa a disputa nos dois caixas; enquanto ela é efetiva, ninguém grava nessa chave (ligar, trocar,
//!   desligar, a ficha) e as leituras a tratam como não ligada; a compra desse código vira pendência
//!   `ligacao_em_disputa` com as duas candidatas e o lançar não liga;
//! - as Diferenças mostram "Código do fornecedor em disputa" com "São a mesma peça: juntar"; "Deixar todas" não fecha;
//! - "Usar a do outro" fecha nos dois caixas; juntar as peças acaba com a disputa (pela raiz): "Corrigido";
//! - só o fator diferente também é disputa.

use crate::banco::Banco;
use crate::catalogo::codigos::{
    fornecedor_em_disputa, vincular_fornecedor, vinculo_do_fornecedor, JaLigado, LinhaCodigoFornecedor,
};
use crate::{compras, config, nfe_xml, pessoas, produtos, sync, texto, usuarios};

fn levar(de: &Banco, para: &mut Banco) -> usize {
    let (mut desde, mut total) = (0, 0);
    loop {
        let (lista, mais) = sync::alteracoes_desde(&de.conn, desde, 40).unwrap();
        if let Some(u) = lista.last() {
            desde = u.seq;
        }
        total += sync::aplicar(&mut para.conn, &lista).unwrap();
        if !mais {
            return total;
        }
    }
}

fn trocar(a: &mut Banco, b: &mut Banco) {
    for _ in 0..6 {
        if levar(a, b) + levar(b, a) == 0 {
            return;
        }
    }
    panic!("os caixas não pararam de trocar");
}

struct Dois {
    a: Banco,
    b: Banco,
    u: String,
    p: String,
    q: String,
    f: String,
}

/// Dois caixas iguais: as peças P e Q e o fornecedor do XML de exemplo (CNPJ 11.222.333/0001-81), sem ligações.
fn dois() -> Dois {
    let mut a = Banco::em_memoria().unwrap();
    let u = usuarios::criar(&a.conn, &a.dispositivo, "Ana", "1234", true).unwrap().id;
    usuarios::definir_gerente(&a.conn, &u, true).unwrap();
    config::gravar(&a.conn, "caixa_nome", "Caixa 1").unwrap();
    let mut ids = Vec::new();
    for nome in ["Rolamento P", "Rolamento Q"] {
        let mut fp = produtos::ficha_nova(&a.conn).unwrap();
        fp.nome = nome.into();
        fp.custo = "10".into();
        ids.push(produtos::salvar(&a.conn, &a.dispositivo, &u, &fp).unwrap());
    }
    let mut ff = pessoas::ficha_nova(&a.conn, "fornecedor").unwrap();
    ff.nome = "Distribuidora Exemplo".into();
    ff.documento = "11.222.333/0001-81".into();
    let f = pessoas::salvar(&a.conn, &a.dispositivo, &u, &ff).unwrap();
    let mut b = Banco::em_memoria().unwrap();
    config::gravar(&b.conn, "caixa_nome", "Caixa 2").unwrap();
    trocar(&mut a, &mut b);
    let q = ids.pop().unwrap();
    let p = ids.pop().unwrap();
    Dois { a, b, u, p, q, f }
}

const CODIGO: &str = "FAG-32206";

/// O caixa 1 liga o código à P e o caixa 2 à Q, sem rede; depois trocam.
fn em_disputa() -> Dois {
    let mut d = dois();
    assert!(vincular_fornecedor(&d.a.conn, &d.f, CODIGO, &d.p, 1000, JaLigado::Manter).unwrap());
    assert!(vincular_fornecedor(&d.b.conn, &d.f, CODIGO, &d.q, 1000, JaLigado::Manter).unwrap());
    trocar(&mut d.a, &mut d.b);
    d
}

fn compacto() -> String {
    texto::compactar(CODIGO)
}

fn ligada(b: &Banco, f: &str) -> String {
    b.conn
        .query_row(
            "SELECT produto_id FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
            [f, &compacto()],
            |l| l.get(0),
        )
        .unwrap()
}

#[test]
fn r12_disputa_trava_os_escritores_e_as_leituras() {
    let d = em_disputa();
    for x in [&d.a, &d.b] {
        let mut c = fornecedor_em_disputa(&x.conn, &d.f, &compacto()).unwrap().expect("disputa nos dois caixas");
        c.sort();
        let mut esperado = vec![d.p.clone(), d.q.clone()];
        esperado.sort();
        assert_eq!(c, esperado);
        assert_eq!(vinculo_do_fornecedor(&x.conn, &d.f, &compacto()).unwrap(), None, "lida como não ligada");
        assert_eq!(sync::conflitos_abertos(&x.conn).unwrap(), 1);
        let k = sync::conflitos(&x.conn).unwrap().into_iter().find(|k| k.tabela == "produto_fornecedor").unwrap();
        assert_eq!(k.o_que, "Código do fornecedor em disputa");
        assert_eq!(k.mesma_peca.as_array().map(|v| v.len()), Some(2));
    }
    let a = &d.a;
    let antes = ligada(a, &d.f);
    // Os escritores não gravam na chave em disputa.
    for modo in [JaLigado::Manter, JaLigado::PassarPeca, JaLigado::PassarPecaEFator, JaLigado::ManterPecaAtualizarFator]
    {
        assert!(!vincular_fornecedor(&a.conn, &d.f, CODIGO, &d.q, 6000, modo).unwrap());
    }
    assert!(!crate::catalogo::codigos::desvincular_fornecedor_se_produto(&a.conn, &d.f, CODIGO, &antes).unwrap());
    assert_eq!(ligada(a, &d.f), antes, "nada mudou na chave");
    // A ficha avisa e não grava a troca do código.
    let mut ficha = produtos::ficha(&a.conn, &antes).unwrap();
    ficha.codigos_fornecedor =
        vec![LinhaCodigoFornecedor { fornecedor_id: d.f.clone(), antes: CODIGO.into(), codigo: "FAG-99999".into() }];
    let e = produtos::salvar(&a.conn, &a.dispositivo, &d.u, &ficha).unwrap_err();
    assert!(e.to_string().contains("ligaram este código a peças diferentes"), "{e}");
    assert_eq!(ligada(a, &d.f), antes);
    // A busca do balcão não acha a peça por esse código (sem a disputa, acharia: a ligação é o único lugar dele).
    let achadas = crate::busca::buscar(&a.conn, CODIGO, false, 20).unwrap();
    assert!(achadas.iter().all(|x| x.id != antes), "a ligação em disputa não acha a peça na busca");
    // "Deixar todas" não fecha a disputa.
    assert_eq!(sync::manter_todos(&a.conn, &d.u).unwrap(), 0);
    assert_eq!(sync::conflitos_abertos(&a.conn).unwrap(), 1);
}

#[test]
fn r12_compra_do_codigo_em_disputa_vira_pendencia_e_nao_liga() {
    let d = em_disputa();
    let a = &d.a;
    let compra = compras::importar_xml(&a.conn, &a.dispositivo, &d.u, nfe_xml::EXEMPLO).unwrap();
    let (acao, produto): (String, Option<String>) = a
        .conn
        .query_row(
            "SELECT acao, produto_id FROM compra_item WHERE compra_id = ?1 AND codigo_fornecedor = ?2",
            [&compra, CODIGO],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .unwrap();
    assert_eq!((acao.as_str(), produto), ("", None), "o item não liga sozinho");
    let (motivo, candidatos): (String, String) = a
        .conn
        .query_row(
            "SELECT motivo, candidatos_json FROM xml_pendencia_identificacao WHERE codigo_compacto = ?1",
            [compacto()],
            |l| Ok((l.get(0)?, l.get(1)?)),
        )
        .unwrap();
    assert_eq!(motivo, "ligacao_em_disputa");
    assert!(candidatos.contains(&d.p) && candidatos.contains(&d.q), "{candidatos}");
}

#[test]
fn r12_usar_a_do_outro_fecha_nos_dois_caixas() {
    let mut d = em_disputa();
    // Controle da busca: sem disputa, o código acha a peça ligada (veja o r12_disputa_trava...).
    let livre = dois();
    assert!(vincular_fornecedor(&livre.a.conn, &livre.f, CODIGO, &livre.p, 1000, JaLigado::Manter).unwrap());
    assert!(crate::busca::buscar(&livre.a.conn, CODIGO, false, 20).unwrap().iter().any(|x| x.id == livre.p));
    let k = sync::conflitos(&d.a.conn).unwrap().into_iter().find(|k| k.tabela == "produto_fornecedor").unwrap();
    sync::resolver_conflito(&d.a.conn, k.id, true, &d.u).unwrap();
    trocar(&mut d.a, &mut d.b);
    for x in [&d.a, &d.b] {
        assert_eq!(fornecedor_em_disputa(&x.conn, &d.f, &compacto()).unwrap(), None);
        assert_eq!(sync::conflitos_abertos(&x.conn).unwrap(), 0);
    }
    assert_eq!(ligada(&d.a, &d.f), ligada(&d.b, &d.f), "a mesma ligação nos dois");
    // Livre de novo: a ligação volta a ser lida.
    assert!(vinculo_do_fornecedor(&d.a.conn, &d.f, &compacto()).unwrap().is_some());
}

#[test]
fn r12_juntar_as_pecas_acaba_com_a_disputa() {
    let mut d = em_disputa();
    {
        let tx = d.a.conn.transaction().unwrap();
        let disp = d.a.dispositivo.clone();
        crate::api::juntar_pecas(&tx, &disp, &d.u, &d.q, &d.p, false).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(fornecedor_em_disputa(&d.a.conn, &d.f, &compacto()).unwrap(), None, "a mesma raiz: sem disputa");
    trocar(&mut d.a, &mut d.b);
    for x in [&d.a, &d.b] {
        assert_eq!(sync::conflitos_abertos(&x.conn).unwrap(), 0);
        // A diferença sai (as duas versões ficaram iguais) ou, se ainda está na lista, aparece como corrigida.
        for k in sync::conflitos(&x.conn).unwrap().into_iter().filter(|k| k.tabela == "produto_fornecedor") {
            assert!(!k.resolvido.is_empty(), "{}", k.texto);
        }
    }
}

#[test]
fn r12_so_o_fator_diferente_tambem_e_disputa() {
    let mut d = dois();
    assert!(vincular_fornecedor(&d.a.conn, &d.f, CODIGO, &d.p, 1000, JaLigado::Manter).unwrap());
    assert!(vincular_fornecedor(&d.b.conn, &d.f, CODIGO, &d.p, 6000, JaLigado::Manter).unwrap());
    trocar(&mut d.a, &mut d.b);
    let c = fornecedor_em_disputa(&d.a.conn, &d.f, &compacto()).unwrap().expect("fator diferente é disputa");
    assert_eq!(c, vec![d.p.clone()]);
    assert_eq!(sync::conflitos_abertos(&d.a.conn).unwrap(), 1);
}

/// Nota 5 do desenho v3: varredura. Quem lê `produto_fornecedor` pela chave tem de passar pelo ponto único de
/// `catalogo/codigos.rs` (que trata a chave em disputa como não ligada). Fora dele, só as leituras listadas aqui, cada
/// uma conferida: mostrar (a ficha, o "O que comprar"), contar pela peça (apagar a peça, a Duplicatas) e a busca do
/// balcão (que já filtra a disputa). Um leitor novo faz o teste reprovar até ser conferido e listado.
#[test]
fn r12_varredura_ninguem_le_a_ligacao_fora_do_ponto_unico() {
    use std::collections::BTreeMap;
    let conferidos: BTreeMap<&str, usize> = [
        ("src/catalogo/busca.rs", 2), // a busca do balcão (filtra a disputa) e a lista de fornecedores (mostrar)
        ("src/catalogo/codigos.rs", 8), // o ponto único (escritores e leituras com a trava)
        ("src/catalogo/faixas.rs", 1), // a Duplicatas: quantas ligações a peça juntada ainda tem (pela peça)
        ("src/catalogo/produtos.rs", 2), // apagar a peça (existe alguma?) e a ficha (mostrar)
        ("src/compras/reposicao.rs", 1), // o "O que comprar" (mostrar o código mais recente)
    ]
    .into_iter()
    .collect();
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let padrao = regex_simples();
    let mut achados: BTreeMap<String, usize> = BTreeMap::new();
    let mut pilha = vec![raiz.join("src")];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                pilha.push(p);
                continue;
            }
            let nome = p.file_name().unwrap().to_string_lossy().to_string();
            if !nome.ends_with(".rs") || nome.starts_with("testes") {
                continue;
            }
            let texto = std::fs::read_to_string(&p).unwrap();
            // Só a parte de produção: até o primeiro módulo de testes.
            let producao =
                match texto.find("#[cfg(test)]\nmod ").or_else(|| texto.find("#[cfg(test)]\npub(crate) mod ")) {
                    Some(i) => &texto[..i],
                    None => &texto[..],
                };
            let n = padrao(producao);
            if n > 0 {
                let rel = p.strip_prefix(raiz).unwrap().to_string_lossy().replace('\\', "/");
                achados.insert(rel, n);
            }
        }
    }
    let esperado: BTreeMap<String, usize> = conferidos.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    assert_eq!(achados, esperado, "leitor de produto_fornecedor novo ou a menos: confira a regra do R12 e liste aqui");
}

/// Conta "FROM produto_fornecedor" e "JOIN produto_fornecedor" (com espaço ou quebra de linha no meio).
fn regex_simples() -> impl Fn(&str) -> usize {
    |t: &str| {
        let palavras: Vec<&str> = t.split_whitespace().collect();
        palavras
            .windows(2)
            .filter(|w| {
                (w[0] == "FROM" || w[0] == "JOIN")
                    && w[1].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_') == "produto_fornecedor"
            })
            .count()
    }
}

/// Mutação R12-raiz (sobreviveu): a junção chega pela troca e a ligação que perdeu ainda aponta para a peça juntada
/// (nada regravou a ligação). Pela raiz, as duas versões são a mesma peça: a disputa acaba e aparece "Corrigido".
#[test]
fn r12_juncao_que_chega_pela_troca_acaba_com_a_disputa_pela_raiz() {
    let d = em_disputa();
    let x = &d.a;
    let atual = ligada(x, &d.f);
    let outra = if atual == d.p { d.q.clone() } else { d.p.clone() };
    // A junção (outra → atual) chega como a troca de dados a grava: só a linha da junção.
    x.conn
        .execute(
            "INSERT INTO produto_juncao (produto_id, para_id, em, por, motivo) VALUES (?1, ?2, '2026-11-25T10:00:00Z', NULL, 'Peça cadastrada em dobro')",
            [&outra, &atual],
        )
        .unwrap();
    assert_eq!(fornecedor_em_disputa(&x.conn, &d.f, &compacto()).unwrap(), None);
    assert_eq!(sync::conflitos_abertos(&x.conn).unwrap(), 0);
    let k = sync::conflitos(&x.conn).unwrap().into_iter().find(|k| k.tabela == "produto_fornecedor").unwrap();
    assert!(k.resolvido.starts_with("Corrigido"), "{}", k.resolvido);
}
