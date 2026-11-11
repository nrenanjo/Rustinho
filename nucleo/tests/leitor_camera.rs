//! Leitor de código pela câmera do celular (dev.3.3, passo 2.3: F03).
//!
//! `codigo_ler_foto` roda sem ninguém conectado (a câmera abre na busca).
//! Antes, ele lia e APAGAVA o caminho que a tela mandasse, sem conferir nada:
//! qualquer arquivo que o app pudesse apagar. Agora o quadro da câmera fica
//! num lugar só, escolhido pelo núcleo (`codigo_caminho_foto`, na pasta
//! privada do Rustinho); outro caminho é recusado e nada é apagado; e o
//! quadro só é apagado depois de conferido que é uma imagem.
//!
//! Dados de mentira.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use comum::*;
use serde_json::json;

/// Um quadro PGM pequeno, todo cinza (é imagem; não tem código).
fn quadro_cinza() -> Vec<u8> {
    let mut d = b"P5\n8 8\n255\n".to_vec();
    d.extend(std::iter::repeat_n(128u8, 64));
    d
}

/// F03 (caracterização em 4b253c6): um caminho de fora do leitor é recusado e
/// o arquivo continua lá.
#[test]
fn f03_caminho_de_fora_e_recusado_e_nada_e_apagado() {
    let mut l = Loja::nova("leitor-fora");
    let pasta = pasta_nova("leitor-fora-arquivos");
    for (nome, conteudo) in [("documento.txt", b"importante".to_vec()), ("outra-foto.pgm", quadro_cinza())] {
        let alvo = pasta.join(nome);
        std::fs::write(&alvo, &conteudo).unwrap();
        let r = l.chamar("codigo_ler_foto", json!({ "caminho": alvo.display().to_string() }));
        assert_eq!(r["ok"], false, "{nome}: aceitou um caminho de fora do leitor: {r}");
        assert!(alvo.exists(), "{nome}: o arquivo de fora do leitor foi APAGADO");
    }
}

/// F03: o quadro fica no lugar do núcleo; é apagado depois de lido; um
/// arquivo que não é imagem nesse lugar é recusado e NÃO é apagado.
#[test]
fn f03_quadro_do_leitor_so_e_apagado_depois_de_conferido() {
    let mut l = Loja::nova("leitor-quadro");
    let caminho = l.ok("codigo_caminho_foto", json!({}))["caminho"].as_str().unwrap().to_string();
    let quadro = std::path::PathBuf::from(&caminho);
    assert!(quadro.is_absolute(), "{caminho}");

    std::fs::write(&quadro, b"isto nao e uma imagem").unwrap();
    let r = l.chamar("codigo_ler_foto", json!({}));
    assert_eq!(r["ok"], false, "aceitou um quadro que não é imagem: {r}");
    assert!(quadro.exists(), "apagou o arquivo antes de conferir que era imagem");

    std::fs::write(&quadro, quadro_cinza()).unwrap();
    let r = l.ok("codigo_ler_foto", json!({}));
    assert_eq!(r["codigo"], "", "{r}");
    assert!(!quadro.exists(), "o quadro lido não foi apagado");

    // O caminho do próprio leitor, mandado pela tela, também vale.
    std::fs::write(&quadro, quadro_cinza()).unwrap();
    l.ok("codigo_ler_foto", json!({ "caminho": caminho }));
    assert!(!quadro.exists());
}
