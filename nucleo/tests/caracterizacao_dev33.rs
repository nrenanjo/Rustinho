//! Dev.3.3, passo 1.3 — testes de caracterização (K01) e cenários do código do
//! fornecedor na importação de XML (N02).
//!
//! Dois tipos de teste moram aqui:
//!
//! - **Caracterização de defeito** (`#[ignore = "caracterização …"]`): descreve
//!   o comportamento CERTO e por isso FALHA em `4b253c6`, pelo motivo do achado.
//!   Fica fora da suíte normal até o passo que corrige o defeito tirar o
//!   `#[ignore]`. Para ver as falhas de hoje:
//!   `cargo test -p rustinho-nucleo --test caracterizacao_dev33 -- --ignored`
//! - **Trava** (sem `#[ignore]`): o que já está certo e não pode piorar
//!   (vários cenários do N02).
//!
//! DP-51 (decidida pelo proprietário em 10/11/2026): o "código original /
//! fabricante" é editável pelo operador; o código de cada fornecedor fica à
//! parte ("Código nos fornecedores"); a importação não depende do código
//! original. Os testes `dp51_*` travam o que já vale e caracterizam o resto.
//! Nada aqui decide regra comercial ainda pendente (DP-06, DP-07, DP-10).
//! Tudo com dados de mentira, em pastas temporárias.

#[path = "apoio/chave.rs"]
mod apoio_chave;
#[path = "estavel/comum.rs"]
mod comum;

use comum::*;
use serde_json::{json, Value};

/// CNPJ da loja de teste: o destinatário da nota de exemplo.
const LOJA_CNPJ: &str = "11444777000161";
/// Emitente da nota de exemplo (`tests/nfe_exemplo.xml`, assinada pela PKI de teste).
const FORNECEDOR_A: &str = "11222333000181";
const NOME_A: &str = "DISTRIBUIDORA DE AUTOPECAS EXEMPLO LTDA";
/// Protocolo de autorização da nota de exemplo.
const PROTOCOLO_EXEMPLO: &str = "135260000000001";
const EXEMPLO: &str = include_str!("nfe_exemplo.xml");

fn loja(nome: &str) -> Loja {
    let l = Loja::nova(nome);
    l.executar(&format!("INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '{LOJA_CNPJ}')"));
    l
}

/// Grava a nota de exemplo numa pasta própria e devolve o caminho da pasta.
fn pasta_com_exemplo(l: &Loja, nome: &str) -> String {
    let p = l.pasta.join(nome);
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("compra.xml"), EXEMPLO).unwrap();
    p.display().to_string()
}

/// "Trazer cadastros" só com o fornecedor: os itens sem vínculo viram pendências.
fn importar_so_fornecedor(l: &mut Loja) -> String {
    let pasta = pasta_com_exemplo(l, "xml");
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta.clone()], "clientes": false, "fornecedores": true,
                "pecasLoja": false, "pecasFornecedor": false }),
    );
    pasta
}

fn id_fornecedor(l: &Loja, documento: &str) -> String {
    l.conexao()
        .query_row("SELECT id FROM pessoa WHERE documento = ?1", [documento], |x| x.get(0))
        .unwrap_or_else(|e| panic!("fornecedor {documento}: {e}"))
}

fn pendencia(l: &mut Loja, codigo: &str) -> String {
    let lista = l.ok("xml_pendencias_listar", json!({}));
    lista
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["codigoFornecedor"] == codigo)
        .unwrap_or_else(|| panic!("sem pendência de {codigo}: {lista}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Liga um código de fornecedor a uma peça como faria outro caminho (outra
/// nota, outro caixa), direto no banco.
fn ligar_no_banco(l: &Loja, fornecedor_id: &str, codigo: &str, peca: &str) {
    let compacto = rustinho_base::texto::compactar(codigo);
    l.conexao()
        .execute(
            "INSERT INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, '2026-11-10T00:00:00Z')",
            [fornecedor_id, &compacto, codigo, peca],
        )
        .unwrap();
}

fn dono_do_codigo(l: &Loja, fornecedor_id: &str, codigo: &str) -> Option<String> {
    let compacto = rustinho_base::texto::compactar(codigo);
    l.conexao()
        .query_row(
            "SELECT produto_id FROM produto_fornecedor WHERE fornecedor_id = ?1 AND codigo_compacto = ?2",
            [fornecedor_id, &compacto],
            |x| x.get(0),
        )
        .ok()
}

fn ficha(l: &mut Loja, peca: &str) -> Value {
    l.ok("produto_abrir", json!({ "id": peca }))
}

/// Peça com campos extras na ficha (código de barras, código original...).
fn peca_com(l: &mut Loja, nome: &str, ajustar: impl FnOnce(&mut Value)) -> String {
    let mut f = l.ok("produto_novo", json!({}));
    f["nome"] = json!(nome);
    f["custo"] = json!("");
    f["estoqueInicial"] = json!("");
    f["precos"][0]["preco"] = json!("10,00");
    f["precos"][0]["lucro"] = json!("");
    ajustar(&mut f);
    l.ok("produto_salvar", f)["id"].as_str().unwrap().to_string()
}

/// Importa a nota de exemplo como compra e devolve o id da compra.
fn importar_compra(l: &mut Loja) -> String {
    let pasta = pasta_com_exemplo(l, "compra");
    let caminho = format!("{pasta}/compra.xml");
    let r = l.ok("compra_importar", json!({ "caminho": caminho }));
    r["compras"][0]["id"].as_str().unwrap_or_else(|| panic!("{r}")).to_string()
}

fn item_da_compra(compra: &Value, codigo: &str) -> Value {
    compra["itens"]
        .as_array()
        .unwrap()
        .iter()
        .find(|it| it["codigoFornecedor"] == codigo)
        .unwrap_or_else(|| panic!("sem item {codigo}: {compra}"))
        .clone()
}

// ===========================================================================
// Caracterização de defeitos (falham em 4b253c6 pelo motivo do achado)
// ===========================================================================

/// G01 — "0.250" (zero à esquerda, ponto e três algarismos) é decimal, não milhar.
/// Era caracterização (falhava em 4b253c6: virava 250000); corrigido no passo 2.2.
#[test]
fn g01_zero_com_ponto_e_decimal() {
    use rustinho_base::numeros::ler_quantidade;
    assert_eq!(ler_quantidade("0.250"), Ok(Some(250)), "0.250 tem de ser 0,250 (250 milésimos)");
}

/// PEND-34 — corrigir o código de barras deixa o GTIN antigo ligado à peça.
#[test]
#[ignore = "caracterização PEND-34: o GTIN antigo continua na peça em 4b253c6; passa no passo 4.3 (DP-09)"]
fn pend34_corrigir_codigo_de_barras_retira_o_antigo() {
    let mut l = loja("pend34");
    let peca = peca_com(&mut l, "FILTRO DE AR", |f| f["codigoBarras"] = json!("7891000000014"));
    let mut f = ficha(&mut l, &peca);
    f["codigoBarras"] = json!("7891000000021");
    l.ok("produto_salvar", f);
    let antigos =
        l.numero(&format!("SELECT COUNT(*) FROM produto_gtin WHERE produto_id = '{peca}' AND gtin = '7891000000014'"));
    assert_eq!(antigos, 0, "o código de barras corrigido (7891000000014) continua ligado à peça");
}

/// PEND-36 — resolver a pendência à mão transfere, em silêncio, um código que
/// já estava ligado a outra peça.
#[test]
#[ignore = "caracterização PEND-36: o vínculo muda em silêncio em 4b253c6; passa no passo 4.2 (DP-06)"]
fn pend36_resolver_pendencia_nao_transfere_vinculo_em_silencio() {
    let mut l = loja("pend36");
    importar_so_fornecedor(&mut l);
    let p1 = l.peca("ROLAMENTO JA LIGADO", "", "10,00", "");
    let p2 = l.peca("ROLAMENTO ESCOLHIDO", "", "10,00", "");
    let forn = id_fornecedor(&l, FORNECEDOR_A);
    ligar_no_banco(&l, &forn, "FAG-32206", &p1);
    let pend = pendencia(&mut l, "FAG-32206");
    let r = l.chamar("xml_pendencia_resolver", json!({ "id": pend, "acao": "ligar", "produtoId": p2 }));
    assert_eq!(
        dono_do_codigo(&l, &forn, "FAG-32206").as_deref(),
        Some(p1.as_str()),
        "o código FAG-32206 já era da outra peça e mudou sem pergunta (resposta: {r})"
    );
}

/// PEND-37 — a pendência não tem encerramento por descarte (só "resolvida").
/// Quem descarta e a categoria do aviso são da DP-13; aqui é o gerente, com
/// motivo, o que vale em qualquer alternativa da DP-13.
#[test]
#[ignore = "caracterização PEND-37: não há descarte em 4b253c6; passa no passo 4.5 (DP-13, DP-48)"]
fn pend37_gerente_descarta_pendencia_com_motivo() {
    let mut l = loja("pend37");
    importar_so_fornecedor(&mut l);
    let antes = l.numero("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'");
    let pend = pendencia(&mut l, "OL5W30");
    let r = l.chamar("xml_pendencia_resolver", json!({ "id": pend, "acao": "descartar", "motivo": "Uso e consumo" }));
    assert_eq!(r["ok"], true, "o gerente não consegue descartar a pendência com motivo: {r}");
    let depois = l.numero("SELECT COUNT(*) FROM xml_pendencia_identificacao WHERE situacao = 'pendente'");
    assert_eq!(depois, antes - 1, "a pendência descartada continua aberta");
}

/// PEND-40 — a busca por código original pega as 5 primeiras peças do índice
/// ANTES de filtrar: a peça certa (a 6ª) some e o item fica "sem vínculo".
#[test]
#[ignore = "caracterização PEND-40: LIMIT 5 antes do filtro em 4b253c6; passa no passo 4.1"]
fn pend40_sexto_candidato_nao_some() {
    let mut l = loja("pend40");
    // Cinco peças que têm o código no índice de busca sem ser o código original
    // nem um equivalente (como um código interno ou de barras igual ao cProd).
    let mut chamarizes = Vec::new();
    for n in 1..=5 {
        chamarizes.push(l.peca(&format!("CHAMARIZ {n}"), "", "10,00", ""));
    }
    for c in &chamarizes {
        l.executar(&format!("UPDATE produto SET codigos = codigos || 'fag32206 ' WHERE id = '{c}'"));
    }
    let certa = peca_com(&mut l, "ROLAMENTO 32206 ORIGINAL", |f| f["codigoOriginal"] = json!("FAG-32206"));
    let compra = importar_compra(&mut l);
    let f = l.ok("compra_abrir", json!({ "id": compra }));
    let it = item_da_compra(&f, "FAG-32206");
    assert_eq!(
        (it["acao"].as_str(), it["peca"]["id"].as_str()),
        (Some("ligar"), Some(certa.as_str())),
        "a única peça com o código original FAG-32206 não foi achada: {it}"
    );
}

/// A08 — lançar a compra grava `DO UPDATE` no vínculo: escolher outra peça
/// transfere, em silêncio, o código que o fornecedor já usava para outra.
#[test]
#[ignore = "caracterização A08: lançar a compra transfere o vínculo em silêncio em 4b253c6; passa no passo 4.2 (DP-07)"]
fn a08_lancar_compra_nao_transfere_vinculo_em_silencio() {
    let mut l = loja("a08");
    let compra = importar_compra(&mut l);
    let forn = id_fornecedor(&l, FORNECEDOR_A);
    let p1 = l.peca("ROLAMENTO JA LIGADO", "", "10,00", "");
    let p2 = l.peca("ROLAMENTO ESCOLHIDO NA COMPRA", "", "10,00", "");
    ligar_no_banco(&l, &forn, "FAG-32206", &p1);
    let f = l.ok("compra_abrir", json!({ "id": compra }));
    let escolhas: Vec<Value> = f["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            if it["codigoFornecedor"] == "FAG-32206" {
                json!({ "id": it["id"], "acao": "ligar", "produtoId": p2, "fator": "1" })
            } else {
                json!({ "id": it["id"], "acao": "nova", "produtoId": "", "fator": "1" })
            }
        })
        .collect();
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": compra, "situacao": "autorizada", "protocolo": PROTOCOLO_EXEMPLO,
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    let r = l.chamar("compra_lancar", json!({ "id": compra, "itens": escolhas }));
    assert_eq!(
        dono_do_codigo(&l, &forn, "FAG-32206").as_deref(),
        Some(p1.as_str()),
        "lançar a compra mudou o vínculo do FAG-32206 sem pergunta (resposta: {r})"
    );
}

// ===========================================================================
// N02 — código do fornecedor na importação de XML (travas que já valem hoje)
// Cenários de `_reversa_sdd/auditoria_dev33/REQUISITO_CODIGO_FORNECEDOR_XML.md` §4.
// Os que pedem notas próprias assinadas (caso PATRAL completo, zeros à esquerda
// na importação, cProd igual ao GTIN de outra peça, GTIN tributável) estão no
// fim do arquivo, com a PKI de teste dos códigos (dev.3.3, passo 3.0).
// ===========================================================================

/// G01 (trava) — os casos que já valem continuam valendo.
#[test]
fn g01_trava_casos_atuais_de_quantidade() {
    use rustinho_base::numeros::ler_quantidade;
    assert_eq!(ler_quantidade("1,5"), Ok(Some(1500)));
    assert_eq!(ler_quantidade("2"), Ok(Some(2000)));
    assert_eq!(ler_quantidade("1.234"), Ok(Some(1_234_000)), "um ponto e três algarismos depois de 1 a 3 é milhar");
    assert_eq!(ler_quantidade("1.5"), Ok(Some(1500)), "ponto com um algarismo é decimal");
}

/// N02, cenários 1, 5 e 9 (leitura): o `cProd` sai como veio, com os zeros à
/// esquerda; o emitente vem do `emit`; GTIN comercial e tributável separados.
#[test]
fn n02_leitura_do_cprod_com_zeros_emitente_e_gtins() {
    let xml = EXEMPLO
        .replacen(&format!("<CNPJ>{FORNECEDOR_A}</CNPJ>"), "<CNPJ>12345678000195</CNPJ>", 1)
        .replacen("<cProd>FAG-32206</cProd>", "<cProd>00008373</cProd>", 1)
        .replacen("<cEAN>7891234567895</cEAN>", "<cEAN>7908168151578</cEAN>", 1)
        .replacen("<vProd>520.00</vProd>", "<vProd>520.00</vProd><cEANTrib>7891000000038</cEANTrib>", 1);
    let nota = rustinho_nucleo::nfe_xml::ler(&xml).expect("nota lida");
    assert_eq!(nota.emitente.documento, "12345678000195", "o fornecedor vem do CNPJ do emitente");
    let it = &nota.itens[0];
    assert_eq!(it.codigo, "00008373", "o código do fornecedor mantém os zeros à esquerda");
    assert_eq!((it.gtin_comercial.as_str(), it.origem_comercial.as_str()), ("7908168151578", "cEAN"));
    assert_eq!((it.gtin_tributavel.as_str(), it.origem_tributavel.as_str()), ("7891000000038", "cEANTrib"));
    assert_eq!(rustinho_base::texto::compactar(&it.codigo), "00008373", "a forma compacta também mantém os zeros");
    assert_ne!(
        rustinho_base::texto::compactar("00008373"),
        rustinho_base::texto::compactar("8373"),
        "com e sem zeros são códigos diferentes (nunca juntados em silêncio)"
    );
}

/// N02, cenário 3: item sem GTIN liga só pelo código do fornecedor; nenhum GTIN é criado.
/// Também confere que o código aparece na ficha, com o nome do fornecedor.
#[test]
fn n02_sem_gtin_liga_pelo_codigo_do_fornecedor_e_mostra_na_ficha() {
    let mut l = loja("n02-sem-gtin");
    let pasta = pasta_com_exemplo(&l, "xml");
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta], "clientes": false, "fornecedores": true,
                "pecasLoja": false, "pecasFornecedor": true }),
    );
    let forn = id_fornecedor(&l, FORNECEDOR_A);
    for codigo in ["N1100", "OL5W30"] {
        let peca = dono_do_codigo(&l, &forn, codigo).unwrap_or_else(|| panic!("{codigo} sem peça"));
        let gtins = l.numero(&format!("SELECT COUNT(*) FROM produto_gtin WHERE produto_id = '{peca}'"));
        assert_eq!(gtins, 0, "{codigo} (SEM GTIN) ganhou GTIN");
        let f = ficha(&mut l, &peca);
        let mostrado =
            f["fornecedores"].as_array().unwrap().iter().any(|x| x["codigo"] == codigo && x["fornecedor"] == NOME_A);
        assert!(mostrado, "{codigo} não aparece na ficha com o fornecedor: {}", f["fornecedores"]);
    }
}

/// N02, cenário 2: a mesma peça com códigos de dois fornecedores; cada um com o seu.
#[test]
fn n02_varios_fornecedores_na_mesma_peca() {
    let mut l = loja("n02-varios");
    importar_so_fornecedor(&mut l);
    let forn_a = id_fornecedor(&l, FORNECEDOR_A);
    let forn_b = l.pessoa("fornecedor", "AUTOPECAS B TESTE LTDA");
    let peca = l.peca("ROLAMENTO 32206", "", "10,00", "");
    ligar_no_banco(&l, &forn_b, "X-77", &peca);
    let pend = pendencia(&mut l, "FAG-32206");
    l.ok("xml_pendencia_resolver", json!({ "id": pend, "acao": "ligar", "produtoId": peca }));
    assert_eq!(dono_do_codigo(&l, &forn_a, "FAG-32206").as_deref(), Some(peca.as_str()));
    assert_eq!(
        dono_do_codigo(&l, &forn_b, "X-77").as_deref(),
        Some(peca.as_str()),
        "o código do outro fornecedor sumiu"
    );
    let f = ficha(&mut l, &peca);
    let codigos: Vec<(String, String)> = f["fornecedores"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| (x["fornecedor"].as_str().unwrap_or("").to_string(), x["codigo"].as_str().unwrap_or("").to_string()))
        .collect();
    assert!(codigos.contains(&(NOME_A.to_string(), "FAG-32206".to_string())), "{codigos:?}");
    assert!(codigos.contains(&("AUTOPECAS B TESTE LTDA".to_string(), "X-77".to_string())), "{codigos:?}");
}

/// N02, cenário 4: o mesmo código usado por OUTRO fornecedor para outra peça
/// não faz a nota deste fornecedor cair naquela peça.
#[test]
fn n02_mesmo_codigo_de_outro_fornecedor_nao_cruza() {
    let mut l = loja("n02-cruzado");
    let forn_b = l.pessoa("fornecedor", "AUTOPECAS B TESTE LTDA");
    let peca_b = l.peca("PECA DO FORNECEDOR B", "", "10,00", "");
    ligar_no_banco(&l, &forn_b, "FAG-32206", &peca_b);
    let compra = importar_compra(&mut l);
    let f = l.ok("compra_abrir", json!({ "id": compra }));
    let it = item_da_compra(&f, "FAG-32206");
    assert_ne!(
        it["peca"]["id"].as_str(),
        Some(peca_b.as_str()),
        "a nota do fornecedor A caiu na peça do fornecedor B pelo código igual: {it}"
    );
}

/// N02, cenário 7: ligar o código do fornecedor a uma peça não sobrescreve o
/// código original que ela já tem.
#[test]
fn n02_nao_sobrescreve_codigo_original_existente() {
    let mut l = loja("n02-original");
    importar_so_fornecedor(&mut l);
    let peca = peca_com(&mut l, "ROLAMENTO COM ORIGINAL", |f| f["codigoOriginal"] = json!("FAB-ORIG-1"));
    let pend = pendencia(&mut l, "FAG-32206");
    l.ok("xml_pendencia_resolver", json!({ "id": pend, "acao": "ligar", "produtoId": peca }));
    let f = ficha(&mut l, &peca);
    assert_eq!(f["codigoOriginal"], "FAB-ORIG-1", "o código original foi sobrescrito");
}

/// N02, cenário 8: importar a mesma nota de novo não duplica vínculo, GTIN nem pendência.
#[test]
fn n02_reimportar_nao_duplica() {
    let mut l = loja("n02-reimportar");
    let pasta = pasta_com_exemplo(&l, "xml");
    let entrada = json!({ "arquivos": [pasta], "clientes": false, "fornecedores": true,
                          "pecasLoja": false, "pecasFornecedor": true });
    l.ok("importar_cadastros", entrada.clone());
    let contar = |l: &Loja| {
        (
            l.numero("SELECT COUNT(*) FROM produto_fornecedor"),
            l.numero("SELECT COUNT(*) FROM produto_gtin"),
            l.numero("SELECT COUNT(*) FROM xml_pendencia_identificacao"),
            l.numero("SELECT COUNT(*) FROM produto"),
        )
    };
    let antes = contar(&l);
    l.ok("importar_cadastros", entrada);
    assert_eq!(contar(&l), antes, "(vínculos, GTINs, pendências, peças) mudaram ao reimportar");
}

// ===========================================================================
// DP-51 (decisão do proprietário, 10/11/2026): o "CÓDIGO ORIGINAL /
// FABRICANTE" é editável pelo operador; o código de cada fornecedor fica à
// parte, em "Código nos fornecedores", e a importação do XML não depende do
// código original. Travas do que já vale; a implementação é a do passo 4.2.
// ===========================================================================

/// Importa a nota de exemplo com as peças do fornecedor (cada item vira peça
/// ligada ao código dele) e devolve o id do fornecedor A.
fn importar_com_pecas(l: &mut Loja) -> String {
    let pasta = pasta_com_exemplo(l, "xml-pecas");
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta], "clientes": false, "fornecedores": true,
                "pecasLoja": false, "pecasFornecedor": true }),
    );
    id_fornecedor(l, FORNECEDOR_A)
}

/// (fornecedor, código) de cada vínculo da peça.
fn vinculos(l: &Loja, peca: &str) -> Vec<(String, String)> {
    let c = l.conexao();
    let mut st =
        c.prepare("SELECT fornecedor_id, codigo FROM produto_fornecedor WHERE produto_id = ?1 ORDER BY 1, 2").unwrap();
    st.query_map([peca], |x| Ok((x.get(0)?, x.get(1)?))).unwrap().map(Result::unwrap).collect()
}

/// DP-51, requisitos 1, 3, 4 e 7: o operador preenche, corrige e muda o
/// código original na ficha; os zeros ficam; os vínculos dos fornecedores não
/// mudam; e a importação ligou o código do fornecedor sem precisar dele.
#[test]
fn dp51_codigo_original_editavel_sem_mexer_nos_fornecedores() {
    let mut l = loja("dp51-editar");
    let forn_a = importar_com_pecas(&mut l);
    let peca = dono_do_codigo(&l, &forn_a, "N1100").expect("a importação ligou N1100 a uma peça");
    assert_eq!(ficha(&mut l, &peca)["codigoOriginal"], "", "a importação preencheu o código original");
    let forn_b = l.pessoa("fornecedor", "AUTOPECAS B TESTE LTDA");
    ligar_no_banco(&l, &forn_b, "X-77", &peca);
    let antes = vinculos(&l, &peca);
    assert_eq!(antes.len(), 2, "{antes:?}");
    for valor in ["000123", "FAB-123", "FAB-123-B", ""] {
        let mut f = ficha(&mut l, &peca);
        f["codigoOriginal"] = json!(valor);
        l.ok("produto_salvar", f);
        assert_eq!(ficha(&mut l, &peca)["codigoOriginal"], valor, "o código original não ficou como digitado");
        assert_eq!(vinculos(&l, &peca), antes, "mudar o código original para {valor:?} mexeu nos fornecedores");
    }
}

/// DP-51, requisito 5: mudar a razão social e o nome fantasia do fornecedor
/// não rompe os vínculos; a nota seguinte (mesmo CNPJ) cai no mesmo fornecedor
/// e nas mesmas peças, sem duplicar nada.
#[test]
fn dp51_mudar_razao_social_e_fantasia_nao_rompe_vinculos() {
    let mut l = loja("dp51-razao");
    let forn_a = importar_com_pecas(&mut l);
    let peca = dono_do_codigo(&l, &forn_a, "N1100").unwrap();
    let vinculos_antes = l.numero("SELECT COUNT(*) FROM produto_fornecedor");
    let mut p = l.ok("pessoa_abrir", json!({ "id": forn_a }));
    p["nome"] = json!("NOVA RAZAO SOCIAL AUTOPECAS LTDA");
    p["fantasia"] = json!("NOVO NOME FANTASIA");
    l.ok("pessoa_salvar", p);
    assert_eq!(dono_do_codigo(&l, &forn_a, "N1100").as_deref(), Some(peca.as_str()), "o vínculo mudou");
    let f = ficha(&mut l, &peca);
    let mostrado = f["fornecedores"].as_array().unwrap().iter().any(|x| x["codigo"] == "N1100");
    assert!(mostrado, "o código sumiu da ficha depois de mudar o nome: {}", f["fornecedores"]);
    // A nota seguinte do mesmo CNPJ (com o nome antigo no XML).
    importar_com_pecas(&mut l);
    let com_o_cnpj = l.numero(&format!("SELECT COUNT(*) FROM pessoa WHERE documento = '{FORNECEDOR_A}'"));
    assert_eq!(com_o_cnpj, 1, "a nota seguinte criou outro fornecedor com o mesmo CNPJ");
    assert_eq!(id_fornecedor(&l, FORNECEDOR_A), forn_a);
    assert_eq!(dono_do_codigo(&l, &forn_a, "N1100").as_deref(), Some(peca.as_str()));
    assert_eq!(l.numero("SELECT COUNT(*) FROM produto_fornecedor"), vinculos_antes, "vínculos duplicados");
}

/// DP-51 com o cenário 4 do N02: o código original que o operador digitou
/// numa peça, igual ao cProd que outro fornecedor manda para OUTRA peça, não
/// mexe nos vínculos daquele fornecedor nem faz a nota dele mudar de peça.
#[test]
fn dp51_codigo_original_igual_ao_de_outro_fornecedor_nao_mexe_no_vinculo() {
    let mut l = loja("dp51-coincide");
    let forn_a = importar_com_pecas(&mut l);
    let peca_a = dono_do_codigo(&l, &forn_a, "N1100").unwrap();
    let peca_x = peca_com(&mut l, "PECA COM ORIGINAL DIGITADO", |f| f["codigoOriginal"] = json!("N1100"));
    assert_eq!(dono_do_codigo(&l, &forn_a, "N1100").as_deref(), Some(peca_a.as_str()), "o vínculo mudou");
    let compra = importar_compra(&mut l);
    let f = l.ok("compra_abrir", json!({ "id": compra }));
    let it = item_da_compra(&f, "N1100");
    assert_eq!(it["peca"]["id"].as_str(), Some(peca_a.as_str()), "o item foi para a peça do código original: {it}");
    assert!(vinculos(&l, &peca_x).is_empty(), "a peça do código original ganhou vínculo");
}

/// DP-51 com o cenário 4 do N02: sem vínculo do fornecedor, um cProd igual ao
/// código original que o operador digitou numa peça não liga a nota a ela em
/// silêncio (vira candidato/pendência, pela política única do passo 4.2).
#[test]
#[ignore = "caracterização N02/DP-51: em 4b253c6 o cProd liga sozinho pela coincidência com o código original; passa no passo 4.2"]
fn dp51_cprod_igual_ao_codigo_original_nao_liga_em_silencio() {
    let mut l = loja("dp51-silencio");
    let peca_x = peca_com(&mut l, "PECA COM ORIGINAL DIGITADO", |f| f["codigoOriginal"] = json!("FAG-32206"));
    let compra = importar_compra(&mut l);
    let f = l.ok("compra_abrir", json!({ "id": compra }));
    let it = item_da_compra(&f, "FAG-32206");
    assert_ne!(
        it["peca"]["id"].as_str(),
        Some(peca_x.as_str()),
        "a nota caiu sozinha na peça cujo código original (digitado) coincide com o cProd: {it}"
    );
    assert!(vinculos(&l, &peca_x).is_empty(), "a coincidência criou vínculo sem pergunta: {:?}", vinculos(&l, &peca_x));
}

// ===========================================================================
// N02 — cenários 1, 5, 6 e 9 com notas ASSINADAS sob medida (dev.3.3, passo
// 3.0), da PKI de teste dos códigos (`tests/pki/gerar_codigos_fornecedor.py`):
// uma "PATRAL" fictícia manda as notas para a loja de teste. Travas do que já
// vale. Ficam fora porque ainda não estão decididos ou porque são de tela: o
// destaque do código do fornecedor na ficha (passo 4.2, roteiro L5), o que fazer
// com o `8373` além de não juntá-lo (pendência ou pergunta: DP-06/DP-07, passo
// 4.2) e qual GTIN a venda usa (DP-10).
// ===========================================================================

/// CNPJ (inventado) da "PATRAL" das notas de `tests/pki/xml/codigos/`.
const PATRAL: &str = "44666777000165";
const NOME_PATRAL: &str = "PATRAL PECAS DE TESTE LTDA";
/// O GTIN do caso relatado (só o número).
const GTIN_PATRAL: &str = "7908168151578";
const GTIN_OUTRA: &str = "7891000000090";
const GTIN_COMERCIAL: &str = "7891000000113";
const GTIN_TRIBUTAVEL: &str = "7891000000120";
const NF601: &str = include_str!("pki/xml/codigos/nf601_patral_00008373.xml");
const NF602: &str = include_str!("pki/xml/codigos/nf602_patral_8373.xml");
const NF603: &str = include_str!("pki/xml/codigos/nf603_patral_cprod_igual_gtin.xml");
const NF604: &str = include_str!("pki/xml/codigos/nf604_patral_gtin_tributavel.xml");

/// Grava a nota numa pasta só dela e devolve (pasta, arquivo).
fn gravar_nota(l: &Loja, pasta: &str, xml: &str) -> (String, String) {
    let p = l.pasta.join(pasta);
    std::fs::create_dir_all(&p).unwrap();
    let arquivo = p.join("nota.xml");
    std::fs::write(&arquivo, xml).unwrap();
    (p.display().to_string(), arquivo.display().to_string())
}

/// "Trazer cadastros" de uma nota: o fornecedor e, se `pecas`, as peças dela.
fn trazer_cadastros(l: &mut Loja, pasta: &str, xml: &str, pecas: bool) {
    let (p, _) = gravar_nota(l, pasta, xml);
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [p], "clientes": false, "fornecedores": true,
                "pecasLoja": false, "pecasFornecedor": pecas }),
    );
}

/// Importa a nota como compra; devolve o id da compra e o item (as notas têm um só).
fn compra_da_nota(l: &mut Loja, pasta: &str, xml: &str) -> (String, Value) {
    let (_, arquivo) = gravar_nota(l, pasta, xml);
    let r = l.ok("compra_importar", json!({ "caminho": arquivo }));
    let id = r["compras"][0]["id"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    let f = l.ok("compra_abrir", json!({ "id": id }));
    let itens = f["itens"].as_array().unwrap();
    assert_eq!(itens.len(), 1, "{f}");
    (id, itens[0].clone())
}

/// (gtin, tipo, origem) de cada GTIN da peça.
fn gtins(l: &Loja, peca: &str) -> Vec<(String, String, String)> {
    let c = l.conexao();
    let mut st =
        c.prepare("SELECT gtin, tipo, origem FROM produto_gtin WHERE produto_id = ?1 ORDER BY 1, 2, 3").unwrap();
    st.query_map([peca], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?))).unwrap().map(Result::unwrap).collect()
}

fn trinca(a: &str, b: &str, c: &str) -> (String, String, String) {
    (a.to_string(), b.to_string(), c.to_string())
}

/// N02, cenário 1 (o caso relatado), de ponta a ponta. A nota da PATRAL com
/// `cProd` 00008373 e `cEAN` 7908168151578 cadastra o fornecedor pelo CNPJ e a
/// peça com o código do fornecedor (zeros mantidos), o GTIN comercial e um
/// código interno próprio, sem gravar o código original (DP-51). A compra da
/// mesma peça cai nela pelo vínculo e, lançada, não muda nenhum código.
#[test]
fn n02_c1_patral_de_ponta_a_ponta() {
    let mut l = loja("n02-c1");
    trazer_cadastros(&mut l, "cadastro", NF601, true);
    let patral = id_fornecedor(&l, PATRAL);
    let peca = dono_do_codigo(&l, &patral, "00008373").expect("00008373 ligado a uma peça");
    let so_o_da_patral = vec![(patral.clone(), "00008373".to_string())];
    assert_eq!(vinculos(&l, &peca), so_o_da_patral, "o código do fornecedor não ficou como veio na nota");
    assert!(
        gtins(&l, &peca).contains(&trinca(GTIN_PATRAL, "comercial", "cEAN")),
        "GTIN comercial: {:?}",
        gtins(&l, &peca)
    );
    let f = ficha(&mut l, &peca);
    let interno = f["codigo"].as_str().unwrap_or("");
    assert!(!interno.is_empty() && !interno.ends_with("8373"), "o código interno não é próprio: {interno:?}");
    assert_eq!(f["codigoOriginal"], "", "a importação gravou o código original (DP-51)");
    let mostrado = f["fornecedores"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["codigo"] == "00008373" && x["fornecedor"] == NOME_PATRAL);
    assert!(mostrado, "a ficha não mostra 00008373 com a PATRAL: {}", f["fornecedores"]);

    // A compra: o item cai na peça pelo vínculo do fornecedor; lançar não muda os códigos.
    let (compra, it) = compra_da_nota(&mut l, "compra", NF601);
    assert_eq!(it["codigoFornecedor"], "00008373", "{it}");
    assert_eq!(it["peca"]["id"].as_str(), Some(peca.as_str()), "{it}");
    assert_eq!(it["achadoPor"], "fornecedor", "{it}");
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": compra, "situacao": "autorizada", "protocolo": "135260000000601",
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    l.ok(
        "compra_lancar",
        json!({ "id": compra, "itens": [{ "id": it["id"], "acao": "ligar", "produtoId": peca, "fator": "1" }] }),
    );
    assert_eq!(l.estoque_pelos_movimentos(&peca), 2, "as 2 unidades da nota");
    assert_eq!(vinculos(&l, &peca), so_o_da_patral, "lançar a compra mexeu no código do fornecedor");
    assert_eq!(ficha(&mut l, &peca)["codigoOriginal"], "", "lançar a compra gravou o código original");
    assert_eq!(l.numero("SELECT COUNT(*) FROM produto_fornecedor"), 1, "vínculo a mais");
}

/// N02, cenário 5: a PATRAL manda `00008373` numa nota e `8373` (sem GTIN, mesma
/// descrição) na seguinte. Os zeros nunca saem do código guardado, e o `8373`
/// não é juntado em silêncio à peça do `00008373` por nenhum dos caminhos de
/// importação. O que fazer com ele (pendência ou pergunta) é da política da 4.2.
#[test]
fn n02_c5_zeros_a_esquerda_nao_juntam_em_silencio() {
    let preparar = |nome: &str| {
        let mut l = loja(nome);
        trazer_cadastros(&mut l, "cadastro", NF601, true);
        let patral = id_fornecedor(&l, PATRAL);
        let peca = dono_do_codigo(&l, &patral, "00008373").expect("00008373 ligado a uma peça");
        (l, patral, peca)
    };
    let intacto = |l: &Loja, patral: &str, peca: &str| {
        let so_o_com_zeros = vec![(patral.to_string(), "00008373".to_string())];
        assert_eq!(vinculos(l, peca), so_o_com_zeros, "o código com zeros mudou, ou a peça ganhou o 8373");
        assert_ne!(dono_do_codigo(l, patral, "8373").as_deref(), Some(peca), "o 8373 foi juntado à peça do 00008373");
    };

    // Compra: o item do 8373 não cai na peça do 00008373.
    let (mut l, patral, peca) = preparar("n02-c5-compra");
    let (_, it) = compra_da_nota(&mut l, "compra", NF602);
    assert_eq!(it["codigoFornecedor"], "8373", "{it}");
    assert_ne!(it["peca"]["id"].as_str(), Some(peca.as_str()), "o 8373 caiu na peça do 00008373: {it}");
    intacto(&l, &patral, &peca);

    // Trazer só o fornecedor: o 8373 fica pendente, sem ligar.
    let (mut l, patral, peca) = preparar("n02-c5-pendencia");
    trazer_cadastros(&mut l, "so-fornecedor", NF602, false);
    pendencia(&mut l, "8373");
    intacto(&l, &patral, &peca);

    // Trazer as peças do fornecedor: o 8373 não vai para a peça do 00008373.
    let (mut l, patral, peca) = preparar("n02-c5-pecas");
    trazer_cadastros(&mut l, "pecas", NF602, true);
    intacto(&l, &patral, &peca);
}

/// N02, cenário 6: o `cProd` da PATRAL é igual ao GTIN de outra peça. O código do
/// fornecedor só vale para a PATRAL: a nota não cai na peça do GTIN, ela não
/// ganha vínculo da PATRAL, e o GTIN continua só nela.
#[test]
fn n02_c6_cprod_igual_ao_gtin_de_outra_peca_nao_colide() {
    let preparar = |nome: &str| {
        let mut l = loja(nome);
        let outra = peca_com(&mut l, "PECA DONA DO GTIN", |f| f["codigoBarras"] = json!(GTIN_OUTRA));
        (l, outra)
    };
    let gtin_so_na_outra = |l: &mut Loja, outra: &str| {
        assert_eq!(ficha(l, outra)["codigoBarras"], GTIN_OUTRA, "o código de barras da outra peça mudou");
        let com_barras = l.numero(&format!("SELECT COUNT(*) FROM produto WHERE codigo_barras = '{GTIN_OUTRA}'"));
        let em_outras = l.numero(&format!(
            "SELECT COUNT(*) FROM produto_gtin WHERE gtin = '{GTIN_OUTRA}' AND produto_id <> '{outra}'"
        ));
        assert_eq!((com_barras, em_outras), (1, 0), "o GTIN foi parar em outra peça");
        assert!(vinculos(l, outra).is_empty(), "a peça do GTIN ganhou vínculo: {:?}", vinculos(l, outra));
    };

    // Compra: o cProd não leva o item para a peça do GTIN.
    let (mut l, outra) = preparar("n02-c6-compra");
    let (_, it) = compra_da_nota(&mut l, "compra", NF603);
    assert_eq!(it["codigoFornecedor"], GTIN_OUTRA, "{it}");
    assert_ne!(it["peca"]["id"].as_str(), Some(outra.as_str()), "a nota caiu na peça do GTIN pelo cProd: {it}");
    gtin_so_na_outra(&mut l, &outra);

    // Trazer só o fornecedor: o código fica pendente, sem ligar.
    let (mut l, outra) = preparar("n02-c6-pendencia");
    trazer_cadastros(&mut l, "so-fornecedor", NF603, false);
    pendencia(&mut l, GTIN_OUTRA);
    gtin_so_na_outra(&mut l, &outra);

    // Trazer as peças do fornecedor: a peça da nota é outra, ligada à PATRAL.
    let (mut l, outra) = preparar("n02-c6-pecas");
    trazer_cadastros(&mut l, "pecas", NF603, true);
    let patral = id_fornecedor(&l, PATRAL);
    let da_nota = dono_do_codigo(&l, &patral, GTIN_OUTRA).expect("o código da PATRAL ligado a uma peça");
    assert_ne!(da_nota, outra, "o código da PATRAL foi ligado à peça do GTIN");
    gtin_so_na_outra(&mut l, &outra);
}

/// N02, cenário 9: GTIN tributável diferente do comercial. Os dois ficam na
/// peça, cada um com o seu tipo e a sua origem; a compra seguinte acha a mesma
/// peça e trazer a nota de novo não duplica. Qual GTIN a venda usa é da DP-10.
#[test]
fn n02_c9_gtin_tributavel_diferente_do_comercial() {
    let mut l = loja("n02-c9");
    trazer_cadastros(&mut l, "pecas", NF604, true);
    let patral = id_fornecedor(&l, PATRAL);
    let peca = dono_do_codigo(&l, &patral, "PT-9").expect("PT-9 ligado a uma peça");
    let os_dois = vec![trinca(GTIN_COMERCIAL, "comercial", "cEAN"), trinca(GTIN_TRIBUTAVEL, "tributavel", "cEANTrib")];
    assert_eq!(gtins(&l, &peca), os_dois, "GTINs com o tipo ou a origem trocados");
    let (_, it) = compra_da_nota(&mut l, "compra", NF604);
    assert_eq!(it["peca"]["id"].as_str(), Some(peca.as_str()), "{it}");
    trazer_cadastros(&mut l, "de-novo", NF604, true);
    assert_eq!(gtins(&l, &peca), os_dois, "trazer a nota de novo mudou os GTINs");
    assert_eq!(l.numero("SELECT COUNT(*) FROM produto"), 1, "trazer a nota de novo criou outra peça");
}

// ---------------------------------------------------------------------------
// Fase 4, lote 1: travas de idempotência e de interrupção (passam hoje) e as
// caracterizações dos itens A01, A03a, A04, A06, A07, D04 e A13 (falham hoje;
// cada item tira o `#[ignore]` da sua). A do A02 fica no próprio `importar.rs`:
// nenhuma nota assinada tem "AB-12" e "AB12" do mesmo emitente, e as chaves da
// PKI de teste foram descartadas. O gatilho que simula a queda existe só no
// banco de teste, criado pelo próprio teste.
// ---------------------------------------------------------------------------

/// Nota de venda da própria loja (emitente = a loja de teste), item `4455`.
const NOTA_LOJA: &str = include_str!("pki/xml/venda_da_loja_sem_protocolo.xml");
/// O GTIN do FAG-32206 na nota de exemplo.
const GTIN_FAG: &str = "7891234567895";
const IDENTIFICACAO: [&str; 5] =
    ["produto", "produto_fornecedor", "produto_gtin", "xml_pendencia_identificacao", "pessoa"];
const TODAS: [&str; 9] = [
    "produto",
    "produto_fornecedor",
    "produto_gtin",
    "xml_pendencia_identificacao",
    "pessoa",
    "compra",
    "compra_item",
    "alteracao",
    "auditoria",
];

fn contagens(l: &Loja, tabelas: &[&str]) -> Vec<(String, i64)> {
    tabelas.iter().map(|t| (t.to_string(), l.numero(&format!("SELECT COUNT(*) FROM {t}")))).collect()
}

/// Uma pasta com várias notas, uma por arquivo.
fn pasta_com_notas(l: &Loja, nome: &str, notas: &[(&str, &str)]) -> String {
    let p = l.pasta.join(nome);
    std::fs::create_dir_all(&p).unwrap();
    for (arquivo, xml) in notas {
        std::fs::write(p.join(arquivo), xml).unwrap();
    }
    p.display().to_string()
}

fn numeros_das_compras(l: &Loja) -> Vec<String> {
    let c = l.conexao();
    let mut st = c.prepare("SELECT numero FROM compra ORDER BY numero").unwrap();
    st.query_map([], |x| x.get(0)).unwrap().map(Result::unwrap).collect()
}

/// Uma pendência a mais, copiada de uma que a importação abriu (o mesmo
/// fornecedor, a mesma nota e o mesmo motivo), com outro código, descrição e
/// GTINs: como a de outro item da mesma nota.
fn pendencia_copiada(l: &Loja, id: &str, codigo: &str, descricao: &str, gtins_json: &str) {
    let compacto = rustinho_base::texto::compactar(codigo);
    let desc_comp = rustinho_base::texto::compactar(descricao);
    l.executar(&format!(
        "CREATE TEMP TABLE copia AS SELECT * FROM xml_pendencia_identificacao WHERE situacao = 'pendente' LIMIT 1;
         UPDATE copia SET id = '{id}', codigo_fornecedor = '{codigo}', codigo_compacto = '{compacto}',
                descricao = '{descricao}', descricao_compacta = '{desc_comp}', gtin_comercial = '',
                gtin_tributavel = '', gtins_json = '{gtins_json}';
         INSERT INTO xml_pendencia_identificacao SELECT * FROM copia;
         DROP TABLE copia;"
    ));
}

fn situacao_da_pendencia(l: &Loja, id: &str) -> String {
    l.conexao()
        .query_row("SELECT situacao FROM xml_pendencia_identificacao WHERE id = ?1", [id], |x| x.get(0))
        .unwrap_or_else(|e| panic!("pendência {id}: {e}"))
}

/// Fase 4 (trava): a mesma NF-e importada de novo como compra é recusada, e
/// nada é gravado de novo (a recusa é pelo banco: vale depois de reiniciar).
#[test]
fn f4_compra_repetida_e_recusada() {
    let mut l = loja("f4-compra-repetida");
    let (_, arquivo) = gravar_nota(&l, "compra", NF601);
    let r = l.ok("compra_importar", json!({ "caminho": arquivo }));
    assert_eq!(r["compras"].as_array().map(Vec::len), Some(1), "{r}");
    let uma = contagens(&l, &TODAS);
    let r = l.ok("compra_importar", json!({ "caminho": arquivo }));
    assert_eq!(r["compras"].as_array().map(Vec::len), Some(0), "{r}");
    assert!(r["erros"].to_string().contains("já foi importada"), "{r}");
    assert_eq!(contagens(&l, &TODAS), uma, "importar a mesma nota de novo gravou algo");
}

/// Fase 4 (trava): "Trazer cadastros" duas vezes com a mesma nota identifica
/// igual a uma vez só (nenhuma peça, vínculo, GTIN, pendência ou pessoa a mais).
#[test]
fn f4_trazer_cadastros_duas_vezes_igual_a_uma() {
    let mut l = loja("f4-trazer-duas-vezes");
    let pasta = pasta_com_exemplo(&l, "xml");
    let dados = json!({ "arquivos": [pasta], "clientes": true, "fornecedores": true,
                        "pecasLoja": true, "pecasFornecedor": true });
    l.ok("importar_cadastros", dados.clone());
    let uma = contagens(&l, &IDENTIFICACAO);
    assert!(uma.iter().any(|(t, n)| t == "produto" && *n > 0), "a primeira vez não cadastrou peça: {uma:?}");
    l.ok("importar_cadastros", dados);
    assert_eq!(contagens(&l, &IDENTIFICACAO), uma, "trazer a mesma nota de novo mudou a identificação");
}

/// Fase 4 (trava): uma queda no meio do "Trazer cadastros" não deixa nada pela
/// metade (é uma transação só). O gatilho aborta a gravação de um GTIN só
/// depois de a mesma operação já ter gravado uma peça nova; nenhuma das 9
/// tabelas muda, nem as alterações da troca. Sem o gatilho, a operação entra
/// uma vez, e repetir não duplica.
#[test]
fn f4_interrupcao_trazer_cadastros_nao_deixa_metade() {
    let mut l = loja("f4-interrupcao-trazer");
    let pasta = pasta_com_exemplo(&l, "xml");
    let dados = json!({ "arquivos": [pasta], "clientes": true, "fornecedores": true,
                        "pecasLoja": true, "pecasFornecedor": true });
    let antes = contagens(&l, &TODAS);
    let pecas_antes = l.numero("SELECT COUNT(*) FROM produto");
    l.executar(&format!(
        "CREATE TRIGGER falha_de_teste BEFORE INSERT ON produto_gtin
           WHEN (SELECT COUNT(*) FROM produto) > {pecas_antes}
           BEGIN SELECT RAISE(ABORT, 'falha de teste no meio'); END"
    ));
    l.erro("importar_cadastros", dados.clone());
    assert_eq!(contagens(&l, &TODAS), antes, "a interrupção deixou algo pela metade");
    l.executar("DROP TRIGGER falha_de_teste");
    l.ok("importar_cadastros", dados.clone());
    let uma = contagens(&l, &IDENTIFICACAO);
    assert!(uma.iter().any(|(t, n)| t == "produto_gtin" && *n > 0), "depois da queda, a operação não entrou: {uma:?}");
    l.ok("importar_cadastros", dados);
    assert_eq!(contagens(&l, &IDENTIFICACAO), uma, "repetir depois da queda duplicou a identificação");
}

/// Fase 4 (trava): numa compra de 3 notas, uma queda na 2ª desfaz só ela (uma
/// transação por nota). O gatilho aborta o primeiro item da nota 603 depois de
/// ela já ter gravado o cabeçalho; a 601 e a 604 entram, a 603 vai para os
/// erros sem deixar nada. Repetir o pacote: 601 e 604 recusadas, 603 entra.
#[test]
fn f4_interrupcao_compra_desfaz_so_a_nota_da_falha() {
    let mut l = loja("f4-interrupcao-compra");
    let pasta = pasta_com_notas(&l, "pacote", &[("1.xml", NF601), ("2.xml", NF603), ("3.xml", NF604)]);
    l.executar(
        "CREATE TRIGGER falha_de_teste BEFORE INSERT ON compra_item
           WHEN (SELECT numero FROM compra WHERE id = NEW.compra_id) LIKE '%603'
           BEGIN SELECT RAISE(ABORT, 'falha de teste no meio'); END",
    );
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!(r["compras"].as_array().map(Vec::len), Some(2), "{r}");
    assert_eq!(r["erros"].as_array().map(Vec::len), Some(1), "{r}");
    let duas = numeros_das_compras(&l);
    assert_eq!(duas.len(), 2, "{duas:?}");
    assert!(duas.iter().all(|n| !n.ends_with("603")), "a nota da queda ficou pela metade: {duas:?}");
    l.executar("DROP TRIGGER falha_de_teste");
    let r = l.ok("compra_importar", json!({ "caminho": pasta }));
    assert_eq!(r["compras"].as_array().map(Vec::len), Some(1), "só a 603 entra de novo: {r}");
    assert_eq!(r["erros"].as_array().map(Vec::len), Some(2), "a 601 e a 604 são recusadas: {r}");
    assert_eq!(numeros_das_compras(&l).len(), 3, "{:?}", numeros_das_compras(&l));
}

/// A01 — um código que compacta para vazio ("---", "***") não fecha a
/// pendência de outro item: resolver uma não mexe na outra.
#[test]
#[ignore = "caracterização A01: o compacto vazio fecha pendências de outros itens em 310cee0; passa no item A01 da fase 4"]
fn a01_codigo_so_de_simbolos_nao_fecha_pendencia_de_outro() {
    let mut l = loja("a01");
    importar_so_fornecedor(&mut l);
    pendencia_copiada(&l, "a01-um", "---", "PECA SO DE SIMBOLOS UM", "[]");
    pendencia_copiada(&l, "a01-dois", "***", "PECA SO DE SIMBOLOS DOIS", "[]");
    l.ok("xml_pendencia_resolver", json!({ "id": "a01-um", "acao": "nova", "produtoId": "" }));
    assert_eq!(situacao_da_pendencia(&l, "a01-um"), "resolvida");
    assert_eq!(
        situacao_da_pendencia(&l, "a01-dois"),
        "pendente",
        "resolver a pendência de um código só de símbolos fechou a de outro item"
    );
}

/// A06 — resolver uma pendência fecha só as do mesmo fornecedor, código e
/// descrição (D-13); a de outra descrição continua aberta, com os seus GTINs.
#[test]
#[ignore = "caracterização A06: resolver fecha a pendência de outra descrição com o mesmo código em 310cee0; passa no item A06 da fase 4"]
fn a06_resolver_fecha_so_a_mesma_descricao() {
    let mut l = loja("a06");
    importar_so_fornecedor(&mut l);
    let fag = pendencia(&mut l, "FAG-32206");
    let gtins_da_outra = r#"[{"gtin":"7891000000090"}]"#;
    pendencia_copiada(&l, "a06-outra", "FAG-32206", "ROLAMENTO DIANTEIRO OUTRO MODELO", gtins_da_outra);
    l.ok("xml_pendencia_resolver", json!({ "id": fag, "acao": "nova", "produtoId": "" }));
    assert_eq!(situacao_da_pendencia(&l, &fag), "resolvida");
    assert_eq!(situacao_da_pendencia(&l, "a06-outra"), "pendente", "fechou a pendência de outra descrição");
    let guardados: String = l
        .conexao()
        .query_row("SELECT gtins_json FROM xml_pendencia_identificacao WHERE id = 'a06-outra'", [], |x| x.get(0))
        .unwrap();
    assert_eq!(guardados, gtins_da_outra, "os GTINs da outra pendência se perderam");
}

/// A07 — "ligar" a uma peça inativa é recusado, com o nome da peça na mensagem;
/// a pendência continua aberta e nenhum vínculo é gravado.
#[test]
#[ignore = "caracterização A07: ligar a peça inativa é aceito em 310cee0; passa no item A07 da fase 4"]
fn a07_ligar_a_peca_inativa_e_recusado() {
    let mut l = loja("a07");
    importar_so_fornecedor(&mut l);
    let inativa = l.peca("ROLAMENTO DESATIVADO", "", "10,00", "");
    l.executar(&format!("UPDATE produto SET ativo = 0 WHERE id = '{inativa}'"));
    let fag = pendencia(&mut l, "FAG-32206");
    let e = l.erro("xml_pendencia_resolver", json!({ "id": fag, "acao": "ligar", "produtoId": inativa }));
    assert!(e.contains("ROLAMENTO DESATIVADO"), "a mensagem não diz qual peça: {e}");
    assert_eq!(situacao_da_pendencia(&l, &fag), "pendente");
    let a = id_fornecedor(&l, FORNECEDOR_A);
    assert_eq!(dono_do_codigo(&l, &a, "FAG-32206"), None, "o código foi ligado à peça inativa");
}

/// D04 — um GTIN repetido via `produto_gtin` (não só no código de barras)
/// aparece nas duplicatas e bloqueia a etiqueta.
#[test]
#[ignore = "caracterização D04: duplicatas e bloqueio da etiqueta olham só o codigo_barras em 310cee0; passa no item D04 da fase 4"]
fn d04_gtin_repetido_em_produto_gtin_bloqueia_a_etiqueta() {
    let mut l = loja("d04");
    peca_com(&mut l, "PECA COM O CODIGO DE BARRAS", |f| f["codigoBarras"] = json!(GTIN_FAG));
    let com_gtin = peca_com(&mut l, "PECA COM O GTIN DA NOTA", |_| {});
    // Como o GTIN que outra nota ou outro caixa gravou nesta peça.
    l.executar(&format!(
        "INSERT INTO produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em)
         VALUES ('d04-gtin', '{com_gtin}', '{GTIN_FAG}', 'comercial', 'cEAN', 0, '2026-11-11T00:00:00Z')"
    ));
    let s = l.ok("codigos_situacao", json!({}));
    let repetido = s["repetidos"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["codigo"] == GTIN_FAG && r["pecas"].as_array().map_or(0, Vec::len) >= 2);
    assert!(repetido, "o GTIN repetido via produto_gtin não aparece nas duplicatas: {}", s["repetidos"]);
    let e = l.ok("etiqueta_previa", json!({ "id": com_gtin }));
    assert!(!e["bloqueio"].as_str().unwrap_or("").is_empty(), "a etiqueta não foi bloqueada: {e}");
}

/// A13 — uma falha ao contar as pendências não vira "zero pendências": o
/// aviso continua na Início, dizendo que a contagem falhou.
#[test]
#[ignore = "caracterização A13: a falha ao contar as pendências vira zero e o aviso some em 310cee0; passa no item A13 da fase 4"]
fn a13_falha_na_contagem_aparece_no_aviso() {
    let mut l = loja("a13");
    importar_so_fornecedor(&mut l);
    let avisos = l.ok("avisos_do_dia", json!({}));
    assert!(avisos.to_string().contains("\"xml_pendencias\""), "sem o aviso antes da falha: {avisos}");
    // A falha de leitura, só no banco de teste: a tabela sai do lugar.
    l.executar("ALTER TABLE xml_pendencia_identificacao RENAME TO xml_pendencia_fora_do_lugar");
    let avisos = l.ok("avisos_do_dia", json!({}));
    assert!(avisos.to_string().contains("\"xml_pendencias\""), "a falha ao contar sumiu com o aviso: {avisos}");
}

/// A03a — "Trazer cadastros" só com "Clientes", ou só com "Fornecedores", não
/// grava GTIN nem vínculo numa peça que já existe (DP-11). Pela leitura técnica
/// da DP-11 (sem veto), o item continua como pendência, com a peça candidata.
#[test]
#[ignore = "caracterização A03: trazer só clientes ou só fornecedores muda peças existentes em 310cee0; passa no item A03a da fase 4"]
fn a03_so_clientes_nao_muda_peca_existente() {
    let mut l = loja("a03");
    let peca = peca_com(&mut l, "ROLAMENTO JA CADASTRADO", |f| f["codigoBarras"] = json!(GTIN_FAG));
    let retrato = |l: &Loja| {
        (gtins(l, &peca), vinculos(l, &peca), contagens(l, &["produto", "produto_fornecedor", "produto_gtin"]))
    };
    let antes = retrato(&l);
    let pasta = pasta_com_exemplo(&l, "xml");
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta.clone()], "clientes": true, "fornecedores": false,
                "pecasLoja": false, "pecasFornecedor": false }),
    );
    assert_eq!(retrato(&l), antes, "trazer só os clientes mudou a peça existente");
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [pasta], "clientes": false, "fornecedores": true,
                "pecasLoja": false, "pecasFornecedor": false }),
    );
    assert_eq!(retrato(&l), antes, "trazer só os fornecedores mudou a peça existente");
    let lista = l.ok("xml_pendencias_listar", json!({}));
    let fag = lista
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["codigoFornecedor"] == "FAG-32206")
        .unwrap_or_else(|| panic!("o FAG-32206 não ficou como pendência: {lista}"));
    assert!(fag.to_string().contains(&peca), "a peça existente não aparece como candidata: {fag}");
}

/// A04 — o código interno de uma nota da própria loja que coincide com uma
/// peça de outra descrição não cai nela (D-13): vira pendência, sem GTIN.
#[test]
#[ignore = "caracterização A04: o código da nota da loja cai na peça de outra descrição em 310cee0; passa no item A04 da fase 4"]
fn a04_codigo_interno_com_descricao_diferente_vira_pendencia() {
    let mut l = loja("a04");
    let outra = peca_com(&mut l, "AMORTECEDOR DIANTEIRO", |_| {});
    l.executar(&format!("UPDATE produto SET codigo = '4455' WHERE id = '{outra}'"));
    let antes = (gtins(&l, &outra), contagens(&l, &["produto", "produto_gtin"]));
    let (p, _) = gravar_nota(&l, "loja", NOTA_LOJA);
    l.ok(
        "importar_cadastros",
        json!({ "arquivos": [p], "clientes": false, "fornecedores": false,
                "pecasLoja": true, "pecasFornecedor": false }),
    );
    assert_eq!((gtins(&l, &outra), contagens(&l, &["produto", "produto_gtin"])), antes, "a nota da loja mexeu na peça");
    let lista = l.ok("xml_pendencias_listar", json!({}));
    let virou = lista.as_array().unwrap().iter().any(|x| x["codigoFornecedor"] == "4455");
    assert!(virou, "o 4455 da nota da loja (outra descrição) não virou pendência: {lista}");
}
