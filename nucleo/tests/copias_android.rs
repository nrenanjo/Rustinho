//! As cópias do Android (`android/rust`) têm de ser iguais às do computador
//! (`app/`): telas, componentes, ponte e montagem. Elas são mantidas à mão
//! (não são links): este teste avisa, na CI, quando uma ficou para trás.
//! Some quando houver uma ponte única para os dois (modularização, etapa 7).

use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Os arquivos da pasta (caminhos relativos, em ordem).
fn arquivos(pasta: &Path) -> Vec<PathBuf> {
    let mut lista = Vec::new();
    let mut pendentes = vec![pasta.to_path_buf()];
    while let Some(p) = pendentes.pop() {
        for item in std::fs::read_dir(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())) {
            let caminho = item.unwrap().path();
            if caminho.is_dir() {
                pendentes.push(caminho);
            } else {
                lista.push(caminho.strip_prefix(pasta).unwrap().to_path_buf());
            }
        }
    }
    lista.sort();
    lista
}

/// O conteúdo, sem diferença de fim de linha (CRLF × LF): o `.gitattributes`
/// guarda a licença da fonte do computador com CRLF e a do Android com LF.
fn conteudo(caminho: &Path) -> Vec<u8> {
    let bytes = std::fs::read(caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
    let texto = !bytes.contains(&0);
    if texto {
        bytes.into_iter().filter(|&b| b != b'\r').collect()
    } else {
        bytes
    }
}

#[test]
fn copias_do_android_iguais_as_do_computador() {
    let r = raiz();
    let mut diferentes = Vec::new();
    for pasta in ["qml", "qml_android", "base", "cpp", "fontes", "icone"] {
        let (a, b) = (r.join("app").join(pasta), r.join("android/rust").join(pasta));
        let (la, lb) = (arquivos(&a), arquivos(&b));
        if la != lb {
            let so_a: Vec<_> = la.iter().filter(|x| !lb.contains(x)).collect();
            let so_b: Vec<_> = lb.iter().filter(|x| !la.contains(x)).collect();
            diferentes.push(format!("{pasta}: só no computador {so_a:?}; só no Android {so_b:?}"));
            continue;
        }
        for f in la {
            if conteudo(&a.join(&f)) != conteudo(&b.join(&f)) {
                diferentes.push(format!("{pasta}/{}", f.display()));
            }
        }
    }
    for (a, b) in [("app/src/ponte.rs", "android/rust/src/ponte.rs"), ("app/build.rs", "android/rust/build.rs")] {
        if conteudo(&r.join(a)) != conteudo(&r.join(b)) {
            diferentes.push(format!("{a} × {b}"));
        }
    }
    assert!(
        diferentes.is_empty(),
        "cópias do Android diferentes das do computador (copie a mudança para as duas): {diferentes:#?}"
    );
}
