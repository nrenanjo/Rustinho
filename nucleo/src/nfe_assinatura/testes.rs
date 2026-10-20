//! Testes de aceitação do PEND-09 (docs/PEND-09-criterios.md §8), escritos
//! antes do verificador. Usam a infraestrutura FICTÍCIA de `tests/pki`
//! (SEM VALIDADE – SÓ TESTE). Os testes com o acervo real ficam no fim,
//! ignorados por padrão, e leem uma pasta fora do repositório
//! (`RUSTINHO_ACERVO_XML`).

use super::*;
use std::time::Instant;

fn fx(nome: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/pki/xml/{nome}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn der(nome: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/pki/{nome}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn teste() -> &'static Ancoras {
    Ancoras::de_teste()
}

fn motivo(xml: &str) -> Motivo {
    match verificar(xml, teste()) {
        Ok(_) => panic!("deveria recusar"),
        Err(r) => r.motivo,
    }
}

fn aceita(xml: &str) -> Prova {
    match verificar(xml, teste()) {
        Ok(v) => v.prova,
        Err(r) => panic!("deveria aceitar: {r}"),
    }
}

/// Troca a primeira ocorrência (o teste falha se o trecho não existir).
fn trocar(xml: &str, de: &str, para: &str) -> String {
    assert!(xml.contains(de), "trecho não encontrado: {de}");
    xml.replacen(de, para, 1)
}

// ---------------------------------------------------------------------------
// A-04: nota da PKI de teste
// ---------------------------------------------------------------------------

#[test]
fn a04_nota_assinada_pela_pki_de_teste_e_aceita() {
    let p = aceita(&fx("ok.xml"));
    assert_eq!(p.titular, "11222333000181");
    assert_eq!(p.algoritmo, "rsa-sha1");
    assert_eq!(p.tipo, "A1");
    assert!(p.cadeia.iter().any(|c| c.contains("AC de TESTE")), "{:?}", p.cadeia);
    assert!(p.raiz.contains("Raiz de TESTE"));
    assert!(p.avisos.is_empty(), "{:?}", p.avisos);
    // O exemplo usado pelos outros testes do núcleo é a mesma nota.
    assert!(verificar(EXEMPLO_TESTE, teste()).is_ok());
}

#[test]
fn a04_par_sha256_tambem_e_aceito() {
    assert_eq!(aceita(&fx("ok_sha256.xml")).algoritmo, "rsa-sha256");
}

#[test]
fn bom_e_espacos_antes_da_declaracao_sao_tolerados() {
    let xml = format!("\u{feff}  \n{}", fx("ok.xml"));
    aceita(&xml);
}

// ---------------------------------------------------------------------------
// A-05 a A-08: adulterações mantendo DigestValue, digVal, chave e protocolo
// ---------------------------------------------------------------------------

#[test]
fn a05_a08_qualquer_mudanca_no_conteudo_assinado_e_recusada() {
    let ok = fx("ok.xml");
    let inf = &ok[ok.find("<infNFe").unwrap()..ok.find("</infNFe>").unwrap()];
    // Quantidade, valores, descrição, código, NCM, CFOP, IE, duplicata e 1 byte qualquer.
    let campos = ["qCom", "vUnCom", "vProd", "vNF", "xProd", "cProd", "NCM", "CFOP", "IE", "vDup", "xNome"];
    let mut testados = 0;
    for campo in campos {
        let abre = format!("<{campo}>");
        let Some(i) = inf.find(&abre) else { continue };
        let ini = i + abre.len();
        let original = &inf[ini..ini + inf[ini..].find('<').unwrap()];
        let novo = if original.chars().next().unwrap().is_ascii_digit() {
            format!("9{original}")
        } else {
            format!("X{original}")
        };
        let adulterado = ok.replacen(
            &format!("{abre}{original}</{campo}>"),
            &format!("{abre}{novo}</{campo}>"),
            1,
        );
        assert_ne!(adulterado, ok);
        assert_eq!(motivo(&adulterado), Motivo::Resumo, "campo {campo}");
        testados += 1;
    }
    assert!(testados >= 9, "só {testados} campos encontrados no exemplo");
    // Um espaço a mais dentro do infNFe também muda o resumo.
    assert_eq!(motivo(&trocar(&ok, "<ide>", "<ide> ")), Motivo::Resumo);
}

#[test]
fn a09_resumo_recalculado_sem_a_assinatura_e_recusado() {
    let ok = fx("ok.xml");
    let adulterado = trocar(&ok, "<qCom>", "<qCom>9");
    let doc = roxmltree::Document::parse(&adulterado).unwrap();
    let inf = doc.descendants().find(|n| n.has_tag_name((NS_NFE, "infNFe"))).unwrap();
    let novo = BASE64.encode(ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        &c14n::canonicalizar(inf).unwrap(),
    ));
    let antigo = texto_de(&ok, "DigestValue");
    let refeito = adulterado.replace(&antigo, &novo);
    assert_eq!(motivo(&refeito), Motivo::Assinatura);
}

#[test]
fn a10_assinatura_de_outra_nota_e_recusada() {
    let ok = fx("ok.xml");
    let outra = texto_de(&fx("vencido_hoje.xml"), "SignatureValue");
    let x = ok.replace(&texto_de(&ok, "SignatureValue"), &outra);
    assert_eq!(motivo(&x), Motivo::Assinatura);
}

#[test]
fn a11_certificado_de_outra_empresa_no_xml_e_recusado() {
    assert_eq!(motivo(&fx("certificado_trocado.xml")), Motivo::Assinatura);
}

#[test]
fn a12_reassinada_com_certificado_proprio_e_recusada() {
    assert_eq!(motivo(&fx("adulterada_reassinada_propria.xml")), Motivo::Cadeia);
    assert_eq!(motivo(&fx("autoassinado.xml")), Motivo::Cadeia);
}

#[test]
fn a13_raiz_falsa_com_o_mesmo_nome_e_recusada() {
    assert_eq!(motivo(&fx("raiz_falsa_mesmo_nome.xml")), Motivo::Cadeia);
}

#[test]
fn a14_cadeia_quebrada() {
    let ok = fx("ok.xml");
    // AC intermediária ausente.
    let sem_ac = Ancoras::montar(vec![(der("raiz_teste.der"), Categoria::Vigente)], vec![]).unwrap();
    assert_eq!(verificar(&ok, &sem_ac).unwrap_err().motivo, Motivo::Cadeia);
    // AC com a assinatura corrompida (último byte da assinatura trocado).
    let mut ac = der("ac_teste.der");
    let n = ac.len() - 1;
    ac[n] ^= 0x55;
    let corrompida = Ancoras::montar(vec![(der("raiz_teste.der"), Categoria::Vigente)], vec![ac]).unwrap();
    assert_eq!(verificar(&ok, &corrompida).unwrap_err().motivo, Motivo::Cadeia);
    // AC de outra raiz (a raiz dela não é âncora).
    let so_teste = Ancoras::montar(
        vec![(der("raiz_teste.der"), Categoria::Vigente)],
        vec![der("ac_teste.der"), der("ac_raiz_historica.der")],
    )
    .unwrap();
    assert_eq!(verificar(&fx("raiz_vence_no_recebimento.xml"), &so_teste).unwrap_err().motivo, Motivo::Cadeia);
}

// ---------------------------------------------------------------------------
// A-15, A-16: validade em dhEmi E em dhRecbto, sem tolerância (§3.5)
// ---------------------------------------------------------------------------

#[test]
fn a15_fora_da_validade_em_qualquer_dos_dois_instantes_e_recusado() {
    for caso in ["vencido_antes_emissao.xml", "comeca_depois_emissao.xml", "vence_antes_recebimento.xml"] {
        assert_eq!(motivo(&fx(caso)), Motivo::Validade, "{caso}");
    }
}

#[test]
fn a16_vencido_hoje_mas_valido_na_emissao_e_no_recebimento_e_aceito() {
    aceita(&fx("vencido_hoje.xml"));
}

#[test]
fn a16b_ac_ou_raiz_que_vence_antes_do_recebimento_e_recusada() {
    assert_eq!(motivo(&fx("ac_vence_no_recebimento.xml")), Motivo::Validade);
    assert_eq!(motivo(&fx("raiz_vence_no_recebimento.xml")), Motivo::Validade);
}

#[test]
fn a16c_fuso_errado_no_recebimento_e_aceito_com_aviso() {
    // Como a nota do Paraná do acervo: o recebimento carimbado com -02:00.
    let x = trocar(&fx("ok.xml"), "2026-10-08T10:16:30-03:00", "2026-10-08T10:16:30-02:00");
    let p = aceita(&x);
    assert!(p.avisos.iter().any(|a| a.contains("recebimento")), "{:?}", p.avisos);
}

#[test]
fn protocolo_sem_data_de_recebimento_e_recusado() {
    let x = trocar(&fx("ok.xml"), "<dhRecbto>2026-10-08T10:16:30-03:00</dhRecbto>", "");
    assert_eq!(motivo(&x), Motivo::Estrutura);
}

// ---------------------------------------------------------------------------
// A-17, A-49 a A-54: uso do certificado (§3.6)
// ---------------------------------------------------------------------------

#[test]
fn a49_key_usage_fora_do_perfil_e_recusado() {
    for caso in ["ku_ausente.xml", "ku_nao_critica.xml", "ku_sem_ds.xml", "ku_sem_nr.xml"] {
        assert_eq!(motivo(&fx(caso)), Motivo::Uso, "{caso}");
    }
}

#[test]
fn a50_sem_key_encipherment_e_aceito_com_registro() {
    let p = aceita(&fx("ku_sem_ke.xml"));
    assert!(p.avisos.iter().any(|a| a.contains("keyEncipherment")), "{:?}", p.avisos);
}

#[test]
fn a51_eku_e_conferido_como_perfil_nao_como_tls() {
    for caso in ["eku_ausente.xml", "eku_so_servidor.xml"] {
        assert_eq!(motivo(&fx(caso)), Motivo::Uso, "{caso}");
    }
    // Só e-mail: aceito (prova que a regra de TLS, clientAuth, NÃO é usada).
    for caso in ["eku_so_email.xml", "eku_so_cliente.xml", "eku_com_cartao.xml"] {
        aceita(&fx(caso));
    }
}

#[test]
fn a52_politica_fora_de_a1_a3_e_recusada() {
    assert_eq!(aceita(&fx("politica_a3.xml")).tipo, "A3");
    for caso in ["politica_a4.xml", "politica_ausente.xml", "politica_ssl.xml"] {
        assert_eq!(motivo(&fx(caso)), Motivo::Tipo, "{caso}");
    }
}

#[test]
fn a53_certificado_de_usuario_com_ca_true_e_recusado() {
    assert_eq!(motivo(&fx("folha_ca.xml")), Motivo::Uso);
    aceita(&fx("sem_basic_constraints.xml"));
}

#[test]
fn a54_ac_sem_key_cert_sign_e_recusada_pelo_rustinho() {
    assert_eq!(motivo(&fx("ac_sem_keycertsign.xml")), Motivo::Cadeia);
}

#[test]
fn rsa_1024_e_recusado() {
    assert_eq!(motivo(&fx("rsa1024.xml")), Motivo::Certificado);
}

// ---------------------------------------------------------------------------
// A-18: titular; A-19: homologação
// ---------------------------------------------------------------------------

#[test]
fn a18_titular_do_certificado() {
    assert_eq!(motivo(&fx("cnpj_outra_raiz.xml")), Motivo::Titular);
    assert_eq!(motivo(&fx("sem_cnpj.xml")), Motivo::Titular);
    aceita(&fx("cnpj_filial.xml"));
    aceita(&fx("cnpj_printable.xml"));
    assert_eq!(aceita(&fx("cpf_ok.xml")).titular, "12345678909");
    assert_eq!(motivo(&fx("cpf_outro.xml")), Motivo::Titular);
}

#[test]
fn a19_homologacao_e_recusada() {
    assert_eq!(motivo(&fx("homologacao.xml")), Motivo::Homologacao);
}

// ---------------------------------------------------------------------------
// A-21: "wrapping"; A-22: algoritmos; A-23: malformados
// ---------------------------------------------------------------------------

#[test]
fn a21_estruturas_que_enganariam_a_leitura_sao_recusadas() {
    let ok = fx("ok.xml");
    let inf_ini = ok.find("<infNFe").unwrap();
    let inf_fim = ok.find("</infNFe>").unwrap() + "</infNFe>".len();
    let inf = &ok[inf_ini..inf_fim];
    let assinatura = &ok[ok.find("<Signature").unwrap()..ok.find("</Signature>").unwrap() + 12];
    let casos = [
        // Segundo infNFe (com outro Id) depois do assinado.
        ("segundo infNFe", ok.replacen("</infNFe>", &format!("</infNFe>{}", inf.replace("Id=\"NFe", "Id=\"NFeX")), 1)),
        // Dois elementos com o mesmo Id.
        ("Id repetido", trocar(&ok, "<emit>", &format!("<emit Id=\"{}\">", id_de(&ok)))),
        // Reference para outro elemento.
        ("Reference para outro Id", ok.replacen(&format!("URI=\"#{}\"", id_de(&ok)), "URI=\"#outro\"", 1)),
        // Assinatura fora do NFe.
        ("Signature no nfeProc", ok.replacen(assinatura, "", 1).replacen("</nfeProc>", &format!("{assinatura}</nfeProc>"), 1)),
        // Duas assinaturas.
        ("duas Signature", ok.replacen(assinatura, &format!("{assinatura}{assinatura}"), 1)),
        // infNFe embrulhado em outro elemento.
        ("infNFe fora do NFe", ok.replacen(inf, &format!("<x>{inf}</x>"), 1)),
        // Assinatura dentro do infNFe.
        ("Signature dentro do infNFe", ok.replacen("</infNFe>", &format!("{assinatura}</infNFe>"), 1)),
        // Prefixo em elemento da região assinada.
        ("prefixo", ok.replacen("<ide>", "<n:ide xmlns:n=\"http://www.portalfiscal.inf.br/nfe\">", 1).replacen("</ide>", "</n:ide>", 1)),
        // Comentário e instrução de processamento na região assinada.
        ("comentário", trocar(&ok, "<ide>", "<ide><!-- x -->")),
        ("instrução", trocar(&ok, "<ide>", "<ide><?x y?>")),
        // Raiz que não é nfeProc.
        ("raiz", format!("<lote>{ok}</lote>").replacen("<?xml version=\"1.0\" encoding=\"UTF-8\"?>", "", 1)),
        // KeyInfo com KeyValue (chave fora do certificado).
        ("KeyValue", trocar(&ok, "<KeyInfo>", "<KeyInfo><KeyValue/>")),
        ("dois certificados", {
            let c = texto_de(&ok, "X509Certificate");
            trocar(&ok, "</X509Certificate>", &format!("</X509Certificate><X509Certificate>{c}</X509Certificate>"))
        }),
    ];
    for (nome, xml) in casos {
        assert_ne!(xml, ok, "{nome}: não mudou");
        let m = motivo(&xml);
        assert!(matches!(m, Motivo::Estrutura | Motivo::Malformado), "{nome}: {m:?}");
    }
}

#[test]
fn a21_dados_lidos_sao_do_no_verificado() {
    let ok = fx("ok.xml");
    let v = verificar(&ok, teste()).unwrap();
    let inf = v.doc.get_node(v.inf).unwrap();
    assert!(inf.has_tag_name((NS_NFE, "infNFe")));
    assert_eq!(inf.attribute("Id"), Some(id_de(&ok).as_str()));
    assert_eq!(inf.parent_element().unwrap().tag_name().name(), "NFe");
}

#[test]
fn a22_algoritmos_e_transformacoes_fora_do_padrao_sao_recusados() {
    let ok = fx("ok.xml");
    let casos = [
        ok.replacen("xmldsig#rsa-sha1", "xmldsig-more#rsa-sha256", 1).replacen("xmldsig#rsa-sha1", "x", 0),
        ok.replacen("http://www.w3.org/2000/09/xmldsig#rsa-sha1", "http://www.w3.org/2000/09/xmldsig#hmac-sha1", 1),
        ok.replacen("http://www.w3.org/2000/09/xmldsig#sha1", "http://www.w3.org/2001/04/xmlenc#sha256", 1),
        ok.replacen(
            "<Transform Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"/>",
            "<Transform Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"/><Transform Algorithm=\"http://www.w3.org/TR/1999/REC-xpath-19991116\"/>",
            1,
        ),
        ok.replacen(
            "<CanonicalizationMethod Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"/>",
            "<CanonicalizationMethod Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments\"/>",
            1,
        ),
        ok.replacen(
            "<Transform Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"/>",
            "<Transform Algorithm=\"http://www.w3.org/2001/10/xml-exc-c14n#\"/>",
            1,
        ),
        ok.replacen("<Transform Algorithm=\"http://www.w3.org/2000/09/xmldsig#enveloped-signature\"/>", "", 1),
    ];
    for (i, xml) in casos.iter().enumerate() {
        assert_ne!(xml, &ok, "caso {i}: não mudou");
        let m = motivo(xml);
        assert!(matches!(m, Motivo::Algoritmo | Motivo::Estrutura), "caso {i}: {m:?}");
    }
}

#[test]
fn profundidade_e_contada_antes_de_ler_sem_estourar_a_pilha() {
    // 100 mil níveis: antes da contagem prévia, isto derrubava o programa.
    let fundo = format!("<a>{}{}</a>", "<b>".repeat(100_000), "</b>".repeat(100_000));
    assert!(profundidade_excessiva(&fundo, PROFUNDIDADE_MAXIMA));
    assert_eq!(motivo(&fundo), Motivo::Malformado);
    // Comentários, CDATA, atributos com '>' e vazios não contam como nível.
    let raso = "<a x=\"1>2\"><!-- <b><b><b> --><![CDATA[<c><c>]]><d/><d/></a>";
    assert!(!profundidade_excessiva(raso, 1));
    assert!(profundidade_excessiva("<a><b><c/></b></a>", 1));
    assert!(!profundidade_excessiva(&fx("ok.xml"), PROFUNDIDADE_MAXIMA));
}

#[test]
fn a23_malformados_sao_recusados_sem_panico_e_rapido() {
    let ok = fx("ok.xml");
    let grande = format!("{}{}", ok, " ".repeat(6 * 1024 * 1024));
    let fundo = format!(
        "<nfeProc xmlns=\"http://www.portalfiscal.inf.br/nfe\">{}{}</nfeProc>",
        "<a>".repeat(200),
        "</a>".repeat(200)
    );
    let entidade = "<?xml version=\"1.0\"?><!DOCTYPE b [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;\">]><nfeProc>&b;</nfeProc>";
    let cert_ruim = ok.replace(&texto_de(&ok, "X509Certificate"), "nao-e-base64!!");
    let der_ruim = ok.replace(&texto_de(&ok, "X509Certificate"), "QUJDREVGRw==");
    let casos: Vec<(&str, String, &[Motivo])> = vec![
        ("vazio", String::new(), &[Motivo::Malformado]),
        ("truncado", ok[..ok.len() / 2].to_string(), &[Motivo::Malformado]),
        ("lixo", "\u{0}\u{1}PK\u{3}\u{4}lixo".to_string(), &[Motivo::Malformado]),
        ("grande", grande, &[Motivo::Malformado]),
        ("profundo", fundo, &[Motivo::Malformado]),
        ("entidade", entidade.to_string(), &[Motivo::Malformado]),
        ("base64 do certificado", cert_ruim, &[Motivo::Certificado]),
        ("DER do certificado", der_ruim, &[Motivo::Certificado]),
    ];
    for (nome, xml, esperado) in casos {
        eprintln!("A-23: {nome}");
        let t = Instant::now();
        let m = motivo(&xml);
        assert!(esperado.contains(&m), "{nome}: {m:?}");
        assert!(t.elapsed().as_secs_f64() < 2.0, "{nome} demorou {:?}", t.elapsed());
    }
}

// ---------------------------------------------------------------------------
// A-47, A-48: pacote de ACs e âncoras
// ---------------------------------------------------------------------------

#[test]
fn a48_pacote_embutido_confere_com_o_sha512_publicado_pelo_iti() {
    use sha2::Digest;
    let h = sha2::Sha512::digest(ancoras::PACOTE_ICP_BRASIL);
    let hex: String = h.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, ancoras::PACOTE_SHA512_PUBLICADO);
}

#[test]
fn a47_so_as_ancoras_escolhidas_valem_e_com_as_categorias_certas() {
    let a = Ancoras::icp_brasil();
    let mut lista: Vec<(String, Categoria)> = a.raizes().map(|r| (r.nome.clone(), r.categoria)).collect();
    lista.sort_by(|x, y| x.0.cmp(&y.0));
    let nomes: Vec<&str> = lista.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(
        nomes,
        [
            "Autoridade Certificadora Raiz Brasileira v12",
            "Autoridade Certificadora Raiz Brasileira v2",
            "Autoridade Certificadora Raiz Brasileira v5"
        ]
    );
    for (n, c) in lista {
        let esperado = if n.ends_with(" v2") { Categoria::Historica } else { Categoria::Vigente };
        assert_eq!(c, esperado, "{n}");
    }
    // Todas as outras raízes do pacote (v1, v3 revogada, v4, v6, v7, v10 SSL,
    // v11 código, v13 e a original) estão no pacote, mas NÃO são âncoras.
    assert!(a.quantidade_no_pacote() >= 330, "{}", a.quantidade_no_pacote());
    for proibida in ["v1", "v3", "v4", "v6", "v7", "v10", "v11", "v13"] {
        assert!(!a.raizes().any(|r| r.nome.ends_with(&format!(" {proibida}"))), "{proibida}");
    }
}

#[test]
fn a47_certificado_do_xml_nunca_vira_ancora() {
    // O autoassinado traz o próprio certificado; mesmo assim é recusado.
    assert_eq!(motivo(&fx("autoassinado.xml")), Motivo::Cadeia);
    // Nota de teste contra as âncoras reais da ICP-Brasil: recusada.
    assert_eq!(verificar(&fx("ok.xml"), Ancoras::icp_brasil()).unwrap_err().motivo, Motivo::Cadeia);
}

// ---------------------------------------------------------------------------
// A-03: C14N contra o libxml2 (casos sintéticos gerados em tests/pki/c14n)
// ---------------------------------------------------------------------------

#[test]
fn a03_c14n_bate_com_o_libxml2_nos_casos_sinteticos() {
    let pasta = format!("{}/tests/pki/c14n", env!("CARGO_MANIFEST_DIR"));
    let mut n = 0;
    for e in std::fs::read_dir(&pasta).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x != "xml").unwrap_or(true) {
            continue;
        }
        let xml = std::fs::read_to_string(&p).unwrap();
        let esperado = std::fs::read(p.with_extension("c14n")).unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let alvo = doc.descendants().find(|n| n.attribute("Id") == Some("alvo")).unwrap();
        let obtido = c14n::canonicalizar(alvo).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(
            String::from_utf8_lossy(&obtido),
            String::from_utf8_lossy(&esperado),
            "{}",
            p.display()
        );
        n += 1;
    }
    assert!(n >= 10, "só {n} casos");
}

// ---------------------------------------------------------------------------
// Acervo real (fora do repositório). `RUSTINHO_ACERVO_XML=/pasta cargo test
// -p rustinho-nucleo --release -- --ignored acervo`
// ---------------------------------------------------------------------------

fn acervo() -> Vec<(String, String)> {
    let pasta = std::env::var("RUSTINHO_ACERVO_XML").expect("defina RUSTINHO_ACERVO_XML");
    let mut v = vec![];
    for e in std::fs::read_dir(pasta).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x.eq_ignore_ascii_case("xml")).unwrap_or(false) {
            let bytes = std::fs::read(&p).unwrap();
            v.push((p.file_name().unwrap().to_string_lossy().to_string(), String::from_utf8_lossy(&bytes).to_string()));
        }
    }
    v.sort();
    v
}

#[test]
#[ignore]
fn acervo_a01_a55_notas_reais_com_as_ancoras_da_icp_brasil() {
    let a = Ancoras::icp_brasil();
    let (mut aceitas, mut teste_fsist, mut outras) = (0, 0, std::collections::BTreeMap::<String, usize>::new());
    let mut tipos = std::collections::BTreeMap::<String, usize>::new();
    let mut avisos = 0;
    let t = Instant::now();
    for (nome, xml) in acervo() {
        let nfe = xml.contains("<infNFe") && xml.contains("<nfeProc");
        match verificar(&xml, a) {
            Ok(v) => {
                aceitas += 1;
                *tipos.entry(v.prova.tipo.to_string()).or_default() += 1;
                avisos += v.prova.avisos.len();
            }
            Err(r) => {
                // As 4 de teste do fsist ("SEM VALIDADE JURÍDICA") têm chave
                // RSA-1024: são recusadas antes mesmo da cadeia.
                if nfe && r.motivo == Motivo::Certificado && r.texto.contains("1024") {
                    teste_fsist += 1;
                } else {
                    assert!(!nfe || r.motivo != Motivo::Resumo, "{nome}: {r}");
                    *outras.entry(format!("{:?}{}", r.motivo, if nfe { "" } else { " (não é NF-e)" })).or_default() += 1;
                }
            }
        }
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("aceitas: {aceitas}; de teste (fsist) recusadas: {teste_fsist}; outras recusas: {outras:?}");
    println!("tipos: {tipos:?}; avisos: {avisos}; tempo total {ms:.0} ms ({:.2} ms por arquivo)", ms / 541.0);
    assert_eq!(aceitas, 528);
    assert_eq!(teste_fsist, 4);
    assert_eq!(outras.get("Estrutura (não é NF-e)"), Some(&9), "{outras:?}");
    assert_eq!(outras.len(), 1, "{outras:?}");
}

#[test]
#[ignore]
fn acervo_a03_c14n_reproduz_o_resumo_de_todas_as_assinaturas() {
    let (mut iguais, mut total) = (0, 0);
    for (nome, xml) in acervo() {
        let xml = xml.trim_start_matches('\u{feff}').trim_start();
        let Ok(doc) = roxmltree::Document::parse(xml) else { continue };
        let Some(inf) = doc.descendants().find(|n| n.has_tag_name((NS_NFE, "infNFe"))) else { continue };
        let Some(dv) = doc.descendants().find(|n| n.has_tag_name((NS_DS, "DigestValue"))) else { continue };
        total += 1;
        let c = c14n::canonicalizar(inf).unwrap();
        let r = BASE64.encode(ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &c));
        if r == dv.text().unwrap_or("").trim() {
            iguais += 1;
        } else {
            println!("resumo diferente: {nome}");
        }
    }
    println!("C14N: {iguais} de {total} resumos reproduzidos");
    assert_eq!(iguais, total);
    assert!(total >= 532);
}

// ---------------------------------------------------------------------------

fn texto_de(xml: &str, tag: &str) -> String {
    let a = format!("<{tag}>");
    let i = xml.find(&a).unwrap() + a.len();
    xml[i..i + xml[i..].find('<').unwrap()].to_string()
}

fn id_de(xml: &str) -> String {
    let i = xml.find("<infNFe Id=\"").unwrap() + "<infNFe Id=\"".len();
    xml[i..i + 47].to_string()
}
