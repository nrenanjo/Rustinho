//! "Trazer cadastros": o que a tela usa do núcleo e a regra da FI-1 na tela.
//!
//! O roteiro de tela não chega à prévia com notas de verdade: as notas de teste
//! são assinadas pela PKI de teste, e o programa de tela recusa abrir com as
//! âncoras de teste (PEND-09). Por isso estas duas travas leem o texto do QML.
//! A igualdade das duas cópias (computador e Android) é do `copias_android.rs`.

use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn ler(caminho: &str) -> String {
    std::fs::read_to_string(raiz().join(caminho)).unwrap_or_else(|e| panic!("{caminho}: {e}"))
}

/// Os textos entre aspas que vêm logo depois de `antes` (ex.: `cat("`).
fn citados(texto: &str, antes: &str) -> Vec<String> {
    texto
        .match_indices(antes)
        .filter_map(|(i, _)| texto[i + antes.len()..].split('"').next().map(String::from))
        .collect()
}

/// Todo código de categoria que a tela pede em `cat("…")` existe na lista do
/// núcleo (`CATEGORIAS` em `compras/importar.rs`). Um código que não existe
/// mostraria 0 sem avisar ninguém.
#[test]
fn as_categorias_que_a_tela_usa_existem_no_nucleo() {
    let qml = ler("app/qml/ImportarCadastros.qml");
    let rs = ler("nucleo/src/compras/importar.rs");
    let inicio = rs.find("const CATEGORIAS: &[(&str, &str)] = &[").expect("a lista CATEGORIAS");
    let lista = &rs[inicio..inicio + rs[inicio..].find("\n];").unwrap()];
    let do_nucleo = citados(lista, "(\"");
    assert!(do_nucleo.len() >= 15, "a leitura da lista achou poucas categorias: {do_nucleo:?}");
    let da_tela = citados(&qml, "cat(\"");
    assert!(!da_tela.is_empty(), "a tela não pede nenhuma categoria?");
    let faltam: Vec<&String> = da_tela.iter().filter(|c| !do_nucleo.contains(c)).collect();
    assert!(faltam.is_empty(), "categorias da tela que o núcleo não tem: {faltam:?}");
}

/// FI-1 (a), decisão do proprietário de 12/11: "Peças das notas de
/// fornecedores" leva "Fornecedores" junto (ao marcar e na primeira leitura), e
/// "Fornecedores" fica travada enquanto as peças estiverem marcadas, com o
/// porquê à vista. (O núcleo também recusa a combinação; veja
/// `importar::testes::fi1_as_16_combinacoes_das_opcoes`.)
#[test]
fn fi1_a_tela_prende_fornecedores_as_pecas_do_fornecedor() {
    let qml = ler("app/qml/ImportarCadastros.qml");
    let sem_espaco = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let q = sem_espaco(&qml);
    for trecho in [
        // a primeira leitura
        "if (painel.qPecasFornecedor) painel.qFornecedores = true",
        // ao marcar as peças do fornecedor
        "if (linha.modelData.chave === \"qPecasFornecedor\" && checked) painel.qFornecedores = true",
        // a opção dos fornecedores travada, e à vista, enquanto as peças estiverem marcadas
        "presa: painel.qPecasFornecedor",
        "enabled: !linha.modelData.presa",
        "|| !!linha.modelData.presa",
        // o porquê
        "linha.modelData.presa ? linha.modelData.porque",
    ] {
        assert!(q.contains(trecho), "falta na tela: {trecho}");
    }
}
