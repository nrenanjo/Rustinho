//! Medidor da busca de peças (0.9.18). Não roda com os testes comuns
//! (`#[ignore]`): só quando pedido, e sempre em release.
//!
//! Gera (uma vez) um banco com peças sintéticas — nomes de autopeças, marcas,
//! códigos de barras, códigos originais, similares, carros e anos, códigos de
//! fornecedor, códigos antigos e movimentos de estoque — e mede o tempo de
//! `busca::buscar` num conjunto fixo de consultas típicas.
//!
//! ```text
//! RUSTINHO_MEDIR_PASTA=/tmp/medir RUSTINHO_MEDIR_PECAS=4000,20000 RUSTINHO_MEDIR_RODADAS=7 \
//!   cargo test --release -p rustinho-nucleo --test medir_busca -- --ignored --nocapture
//! ```
//!
//! O banco fica em `<pasta>/busca_<peças>.db` e é reaproveitado nas medições
//! seguintes (mesmos dados para medir antes e depois de uma mudança). Sem
//! `RUSTINHO_MEDIR_PASTA`, usa uma pasta no diretório temporário.

use std::path::PathBuf;
use std::time::Instant;

use rustinho_nucleo::banco::Banco;
use rustinho_nucleo::{busca, faixas, pessoas, produtos, tempo, texto, usuarios};

/// Sorteio determinístico (xorshift64*), sem dependência nova.
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n as u64) as usize
    }
    fn chance(&mut self, por_cento: usize) -> bool {
        self.ate(100) < por_cento
    }
    fn um<'a>(&mut self, lista: &[&'a str]) -> &'a str {
        lista[self.ate(lista.len())]
    }
    fn letras(&mut self, n: usize) -> String {
        (0..n).map(|_| (b'A' + self.ate(26) as u8) as char).collect()
    }
    fn digitos(&mut self, n: usize) -> String {
        (0..n).map(|_| (b'0' + self.ate(10) as u8) as char).collect()
    }
    /// Um código como os das notas e catálogos.
    fn codigo(&mut self) -> String {
        match self.ate(7) {
            0 => format!("{}{}{}", self.letras(2), self.digitos(4), self.letras(1)),
            1 => format!("{}{}", self.digitos(5), self.letras(3)),
            2 => format!("{}-{}", self.letras(2), self.digitos(4)),
            3 => self.digitos(5),
            4 => format!("{}{}{}", self.letras(3), self.digitos(6), self.letras(1)),
            5 => format!("{}-{}", self.letras(3), self.digitos(2)),
            _ => format!("{}{}", self.letras(2), self.digitos(5)),
        }
    }
}

const TIPOS: &[&str] = &[
    "PASTILHA DE FREIO DIANTEIRA",
    "PASTILHA DE FREIO TRASEIRA",
    "DISCO DE FREIO",
    "FILTRO DE OLEO",
    "FILTRO DE AR",
    "FILTRO DE COMBUSTIVEL",
    "FILTRO DE CABINE",
    "CORREIA DENTADA",
    "CORREIA POLI V",
    "TENSOR DA CORREIA",
    "ROLAMENTO RODA DIANTEIRA",
    "ROLAMENTO RODA TRASEIRA",
    "AMORTECEDOR DIANTEIRO",
    "AMORTECEDOR TRASEIRO",
    "KIT BATENTE",
    "BOMBA D'AGUA",
    "BOMBA DE COMBUSTIVEL",
    "JUNTA DO CABECOTE",
    "JOGO DE JUNTAS",
    "RETENTOR DO VIRABREQUIM",
    "RETENTOR CAIXA MUDANCA",
    "VELA DE IGNICAO",
    "CABO DE VELA",
    "BOBINA DE IGNICAO",
    "SENSOR DE ROTACAO",
    "SENSOR DE OXIGENIO",
    "RELE AUXILIAR",
    "RELE PISCA EMERGENCIA",
    "LAMPADA H4",
    "LAMPADA H7",
    "TERMINAL DE DIRECAO",
    "TERMINAL HIDRAULICO",
    "PIVO DE SUSPENSAO",
    "BANDEJA DE SUSPENSAO",
    "BUCHA DA BANDEJA",
    "COXIM DO MOTOR",
    "COXIM DO CAMBIO",
    "MANGUEIRA DO RADIADOR",
    "RADIADOR",
    "VALVULA TERMOSTATICA",
    "JOGO REPARO VALVULA DESCARGA",
    "CILINDRO MESTRE",
    "BURRINHO DE FREIO",
    "CABO DE EMBREAGEM",
    "KIT EMBREAGEM",
    "PLATO",
    "DISCO DE EMBREAGEM",
    "ESCAPAMENTO INTERMEDIARIO",
    "SILENCIOSO TRASEIRO",
    "OLEO 5W30 SINTETICO",
    "OLEO 15W40",
    "FLUIDO DE FREIO DOT 4",
    "ADITIVO RADIADOR",
    "PALHETA LIMPADOR",
    "ENGATE RAPIDO",
    "CONEXAO PNEUMATICA",
    "ABRACADEIRA",
    "PARAFUSO DE RODA",
    "PORCA DE RODA",
    "ANEL DE VEDACAO",
];

const QUALIFICADORES: &[&str] = &[
    "",
    "",
    "",
    "",
    "REFORCADO",
    "UNIVERSAL",
    "ORIGINAL",
    "C/ SENSOR",
    "S/ SENSOR",
    "LD",
    "LE",
    "24V",
    "12V",
    "3/8\"",
    "1/2\"",
    "3/4\" UNF",
    "M12 X 1.5",
    "8PK 1515",
    "6PK 1230",
    "5T",
    "GRANDE",
    "PEQUENO",
];

const MARCAS: &[&str] = &[
    "BOSCH",
    "COFAP",
    "SKF",
    "NAKATA",
    "MONROE",
    "FRAS-LE",
    "TECFIL",
    "MAHLE",
    "DAYCO",
    "GATES",
    "NGK",
    "VALEO",
    "SABO",
    "AMX",
    "HIDRAFLUX",
    "TRW",
    "VIEMAR",
    "URBA",
    "METAL LEVE",
    "WEGA",
    "MANN",
    "CONTITECH",
    "SACHS",
    "LUK",
    "INA",
    "FAG",
    "PHILIPS",
    "OSRAM",
    "DS",
    "MTE",
];

const CARROS: &[(&str, &[&str])] = &[
    ("VW", &["Gol", "Gol G5", "Fox", "Polo", "Saveiro", "Voyage", "Up"]),
    ("Fiat", &["Uno", "Palio", "Strada", "Siena", "Argo", "Mobi", "Toro"]),
    ("GM", &["Onix", "Celta", "Corsa", "Prisma", "S10", "Cruze"]),
    ("Ford", &["Ka", "Fiesta", "Focus", "Ranger", "Cargo 1114", "Cargo 1117"]),
    ("Renault", &["Sandero", "Logan", "Duster", "Kwid"]),
    ("Toyota", &["Corolla", "Hilux", "Etios"]),
    ("Honda", &["Civic", "Fit", "City", "HR-V"]),
    ("Hyundai", &["HB20", "Creta", "Tucson"]),
    ("Mercedes", &["Sprinter", "Accelo", "Atego 1719"]),
    ("Scania", &["R440", "P310"]),
    ("Volvo", &["FH 460", "VM 270"]),
];

const MOTORES: &[&str] = &["1.0", "1.4", "1.6", "1.8", "2.0", "2.8 Diesel", "1.0 Turbo", "OM 924", "", ""];

fn ean13(serie: u64) -> String {
    let base = format!("789{:09}", serie % 1_000_000_000);
    let soma: u32 =
        base.chars().enumerate().map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 0 { 1 } else { 3 }).sum();
    format!("{base}{}", (10 - soma % 10) % 10)
}

fn gerar(caminho: &PathBuf, quantas: usize) {
    let inicio = Instant::now();
    let b = Banco::abrir(caminho).expect("abrir banco novo");
    let c = &b.conn;
    let d = &b.dispositivo;
    faixas::assumir(c).unwrap();
    let u = usuarios::criar(c, d, "Medidor", "1234", true).unwrap().id;
    let mut s = Sorteio(0x5EED_0918_B05C_A001);

    c.execute_batch("BEGIN").unwrap();
    let mut fornecedores = Vec::new();
    for i in 0..60 {
        let mut f = pessoas::ficha_nova(c, "fornecedor").unwrap();
        f.nome =
            if i == 0 { "HIDRAFLUX INDUSTRIAL LTDA".into() } else { format!("{} DISTRIBUIDORA {i}", s.um(MARCAS)) };
        fornecedores.push(pessoas::salvar(c, d, &u, &f).unwrap());
    }
    let ligar_fornecedor = |forn: &str, produto: &str, codigo: &str| {
        c.execute(
            "INSERT OR IGNORE INTO produto_fornecedor (fornecedor_id, codigo_compacto, codigo, produto_id, atualizado_em)
             VALUES (?1, ?2, ?3, ?4, '2026-10-15')",
            [forn, &texto::compactar(codigo), codigo, produto],
        )
        .unwrap();
    };
    let peca = |f: &produtos::ProdutoFicha| produtos::salvar(c, d, &u, f).unwrap();

    // As peças das capturas do dono.
    let capturas: &[(&str, &str, &str)] = &[
        ("TERMINAL FG 3/4\" UNF 20 FIOS X 3/8\"", "", "TCF-01"),
        ("VALVULA REGULADORA DE AR", "", "RG-0815"),
        ("02530BRG RETENTOR CAIXA MUDANCA - SAB", "2108", ""),
        ("FR99285 JOGO REPARO VALVULA DESCARGA RAPIDA - AMX", "40812", ""),
        ("FR99375 JOGO REPARO VALVULA DESCARGA - AMX", "50813", ""),
        ("RELE PISCA EMERGENCIA OU ALTERNADO 24V 5T", "70815", ""),
        ("TECFIL AS836 AR FORD CARGO 1114 / 1117 - TEG129620A", "90817", ""),
    ];
    for (nome, original, do_fornecedor) in capturas {
        let mut f = produtos::ficha_nova(c).unwrap();
        f.nome = nome.to_string();
        f.custo = "10".into();
        f.codigo_original = original.to_string();
        let id = peca(&f);
        if !do_fornecedor.is_empty() {
            ligar_fornecedor(&fornecedores[0], &id, do_fornecedor);
        }
    }

    for i in 0..quantas.saturating_sub(capturas.len()) {
        let mut f = produtos::ficha_nova(c).unwrap();
        let tipo = s.um(TIPOS);
        let qualificador = s.um(QUALIFICADORES);
        let marca = s.um(MARCAS);
        let prefixo = if s.chance(25) { format!("{} ", s.codigo()) } else { String::new() };
        f.nome = format!("{prefixo}{tipo} {qualificador}").trim().to_string();
        if s.chance(30) {
            f.nome.push_str(&format!(" - {}", s.letras(3)));
        }
        f.marca = marca.to_string();
        f.custo = format!("{},{:02}", 5 + s.ate(400), s.ate(100));
        f.categoria = s.um(&["Freios", "Suspensão", "Motor", "Elétrica", "Filtros", "Arrefecimento", ""]).to_string();
        f.local = format!("{}-{}-{}", s.letras(1), s.ate(40), s.ate(9));
        if s.chance(55) {
            f.codigo_barras = ean13(100_000 + i as u64);
        }
        if s.chance(60) {
            f.codigo_original = s.codigo();
        }
        if s.chance(70) {
            f.estoque_inicial = format!("{}", 1 + s.ate(20));
        }
        for _ in 0..s.ate(4) {
            let (montadora, modelos) = CARROS[s.ate(CARROS.len())];
            let ano = 1995 + s.ate(28);
            f.aplicacoes.push(produtos::AplicacaoFicha {
                montadora: montadora.to_string(),
                modelo: s.um(modelos).to_string(),
                motor: s.um(MOTORES).to_string(),
                ano_inicio: if s.chance(85) { ano.to_string() } else { String::new() },
                ano_fim: if s.chance(70) { (ano + s.ate(10)).min(2026).to_string() } else { String::new() },
                observacao: String::new(),
            });
        }
        if s.chance(40) {
            for _ in 0..1 + s.ate(2) {
                f.equivalentes.push(produtos::EquivalenteFicha { marca: s.um(MARCAS).to_string(), codigo: s.codigo() });
            }
        }
        let id = match produtos::salvar(c, d, &u, &f) {
            Ok(id) => id,
            Err(_) => {
                // Código de barras ou original repetido por sorteio: sem eles.
                f.codigo_barras.clear();
                peca(&f)
            }
        };
        if s.chance(80) {
            for _ in 0..1 + s.ate(2) {
                let codigo = s.codigo();
                ligar_fornecedor(&fornecedores[s.ate(fornecedores.len())], &id, &codigo);
            }
        }
        if s.chance(10) {
            let antigo = if s.chance(50) { s.digitos(4) } else { format!("{}-{}", s.letras(1), s.digitos(3)) };
            c.execute(
                "INSERT OR IGNORE INTO produto_codigo_antigo (codigo, produto_id, codigo_compacto, trocado_em)
                 VALUES (?1, ?2, ?3, '2026-10-15')",
                [&antigo, &id, &texto::compactar(&antigo)],
            )
            .unwrap();
        }
        // Histórico de vendas e compras (a soma do estoque entra na busca).
        for _ in 0..s.ate(16) {
            c.execute(
                "INSERT INTO estoque_movimento (id, produto_id, quantidade, tipo, dispositivo, criado_em)
                 VALUES (?1, ?2, ?3, 'venda', ?4, '2026-10-01T10:00:00Z')",
                rusqlite_params(&tempo::novo_id(), &id, -(1 + s.ate(3) as i64) * 1000, d),
            )
            .unwrap();
        }
        if i % 500 == 499 {
            c.execute_batch("COMMIT; BEGIN").unwrap();
            eprint!("\r{} peças", i + 1 + capturas.len());
        }
    }
    c.execute_batch("COMMIT").unwrap();
    c.execute_batch("ANALYZE").unwrap();
    eprintln!("\rgerado: {quantas} peças em {:.1} s", inicio.elapsed().as_secs_f64());
}

fn rusqlite_params<'a>(
    id: &'a str,
    produto: &'a str,
    quantidade: i64,
    dispositivo: &'a str,
) -> (&'a str, &'a str, i64, &'a str) {
    (id, produto, quantidade, dispositivo)
}

fn um_valor(b: &Banco, sql: &str) -> String {
    b.conn.query_row(sql, [], |l| l.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

fn percentil(ordenados: &[f64], p: f64) -> f64 {
    let n = ordenados.len();
    let posicao = ((p * n as f64).ceil() as usize).clamp(1, n);
    ordenados[posicao - 1]
}

#[test]
#[ignore = "medidor de desempenho: rodar em release, com --ignored --nocapture"]
fn medir_busca() {
    let pasta = std::env::var_os("RUSTINHO_MEDIR_PASTA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("rustinho-medir-busca"));
    let pecas = std::env::var("RUSTINHO_MEDIR_PECAS").unwrap_or_else(|_| "4000,20000".into());
    let rodadas: usize = std::env::var("RUSTINHO_MEDIR_RODADAS").ok().map(|r| r.parse().expect("rodadas")).unwrap_or(7);
    if cfg!(debug_assertions) {
        println!("# AVISO: compilado sem --release; os tempos não valem para comparar.");
    }
    for quantas in pecas.split(',') {
        medir(&pasta, quantas.trim().parse().expect("número de peças"), rodadas);
    }
}

fn medir(pasta: &std::path::Path, quantas: usize, rodadas: usize) {
    std::fs::create_dir_all(pasta).unwrap();
    let caminho = pasta.join(format!("busca_{quantas}.db"));
    if !caminho.exists() {
        gerar(&caminho, quantas);
    }

    let b = Banco::abrir(&caminho).expect("abrir banco");
    let pecas = um_valor(&b, "SELECT COUNT(*) || '' FROM produto");
    let contagens = um_valor(
        &b,
        "SELECT (SELECT COUNT(*) FROM produto_fornecedor) || ' códigos de fornecedor, '
             || (SELECT COUNT(*) FROM produto_codigo_antigo) || ' códigos antigos, '
             || (SELECT COUNT(*) FROM produto_aplicacao) || ' aplicações, '
             || (SELECT COUNT(*) FROM produto_equivalente) || ' similares, '
             || (SELECT COUNT(*) FROM estoque_movimento) || ' movimentos de estoque'",
    );
    let versao = um_valor(&b, "SELECT sqlite_version()");
    println!("# banco {} — {pecas} peças; {contagens}; SQLite {versao}", caminho.display());

    // Códigos reais do banco (sempre os mesmos: o banco é determinístico).
    let barras =
        um_valor(&b, "SELECT codigo_barras FROM produto WHERE codigo_barras <> '' ORDER BY rowid LIMIT 1 OFFSET 777");
    let interno = um_valor(&b, "SELECT codigo FROM produto WHERE codigo <> '' ORDER BY rowid LIMIT 1 OFFSET 1234");
    let original = um_valor(
        &b,
        "SELECT codigo_original FROM produto WHERE codigo_original <> '' ORDER BY rowid LIMIT 1 OFFSET 555",
    );
    let do_fornecedor = um_valor(&b, "SELECT codigo FROM produto_fornecedor ORDER BY rowid LIMIT 1 OFFSET 999");
    let prefixo: String = texto::compactar(&do_fornecedor).chars().take(4).collect();
    let antigo = um_valor(&b, "SELECT codigo FROM produto_codigo_antigo ORDER BY rowid LIMIT 1 OFFSET 99");
    let similar = um_valor(&b, "SELECT codigo FROM produto_equivalente ORDER BY rowid LIMIT 1 OFFSET 321");

    let consultas: Vec<String> = [
        "",
        "tcf",
        "tcf 01",
        "01 tcf",
        "rg08",
        "rg-08",
        "rg 08",
        "pastilha",
        "pastilha gol",
        "pastilha diant gol 2012",
        "filtro oleo",
        "filtro de ar fiat uno",
        "correia 8pk",
        "8pk1515",
        "rolamento roda",
        "amortecedor dianteiro cofap",
        "gol 1.6 2010",
        "onix",
        "bosch",
        "terminal tcf",
        "terminal 3/4",
        "rele 24v",
        &barras,
        &interno,
        &original,
        &do_fornecedor,
        &prefixo,
        &antigo,
        &similar,
        "pa",
        "p",
        "xyzw",
        "cargo 1114",
        "kit embreagem vw",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut todas: Vec<f64> = Vec::new();
    for limite in [200_i64, 30] {
        let mut amostras: Vec<f64> = Vec::new();
        println!("\n## limite {limite} ({rodadas} rodadas por consulta, depois de 1 de aquecimento)");
        println!("{:<34} {:>9} {:>9} {:>9}  achou  primeiras", "consulta", "mediana", "mínimo", "máximo");
        for consulta in &consultas {
            let _ = busca::buscar(&b.conn, consulta, false, limite).unwrap();
            let mut tempos = Vec::new();
            let mut achou = Vec::new();
            for _ in 0..rodadas {
                let t = Instant::now();
                achou = busca::buscar(&b.conn, consulta, false, limite).unwrap();
                tempos.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            amostras.extend(&tempos);
            tempos.sort_by(f64::total_cmp);
            let primeiras: Vec<String> = achou.iter().take(3).map(|p| p.nome.chars().take(38).collect()).collect();
            println!(
                "{:<34} {:>7.2}ms {:>7.2}ms {:>7.2}ms  {:>5}  {}",
                format!("{consulta:?}"),
                percentil(&tempos, 0.5),
                tempos[0],
                tempos[tempos.len() - 1],
                achou.len(),
                primeiras.join(" | ")
            );
        }
        amostras.sort_by(f64::total_cmp);
        println!(
            "RESUMO limite {limite}: {} amostras — p50 {:.2} ms, p95 {:.2} ms, máximo {:.2} ms",
            amostras.len(),
            percentil(&amostras, 0.5),
            percentil(&amostras, 0.95),
            amostras[amostras.len() - 1]
        );
        todas.extend(amostras);
    }

    // Leitor de código de barras / Enter com código.
    let mut amostras: Vec<f64> = Vec::new();
    println!("\n## leitor (pelo_codigo_exato)");
    for consulta in [&barras, &interno, &original, &do_fornecedor, &antigo, &similar, &"tcf 01".to_string()] {
        let _ = busca::pelo_codigo_exato(&b.conn, consulta).unwrap();
        let mut tempos = Vec::new();
        let mut n = 0;
        for _ in 0..rodadas {
            let t = Instant::now();
            n = busca::pelo_codigo_exato(&b.conn, consulta).unwrap().len();
            tempos.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        amostras.extend(&tempos);
        tempos.sort_by(f64::total_cmp);
        println!("{:<34} {:>7.2}ms  {n} peça(s)", format!("{consulta:?}"), percentil(&tempos, 0.5));
    }
    amostras.sort_by(f64::total_cmp);
    println!(
        "RESUMO leitor: p50 {:.2} ms, p95 {:.2} ms, máximo {:.2} ms",
        percentil(&amostras, 0.5),
        percentil(&amostras, 0.95),
        amostras[amostras.len() - 1]
    );
    todas.sort_by(f64::total_cmp);
    println!(
        "RESUMO GERAL buscar (limites 200 e 30): {} amostras — p50 {:.2} ms, p95 {:.2} ms, máximo {:.2} ms",
        todas.len(),
        percentil(&todas, 0.5),
        percentil(&todas, 0.95),
        todas[todas.len() - 1]
    );
}
