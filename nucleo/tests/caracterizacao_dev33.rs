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
// na importação, cProd igual ao GTIN de outra peça) entram junto da PKI de
// teste dos códigos (ver o relatório da fase 1).
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
