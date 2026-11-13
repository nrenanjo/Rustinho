//! Quem pode depender de quem (modularização, etapa 0).
//!
//! Lê os fontes do núcleo seguindo as declarações `mod` a partir do
//! `nucleo/src/lib.rs` (com `#[path]` e `include!`), monta o grafo entre os
//! módulos de primeiro nível pelas referências `crate::modulo` e
//! `super::modulo` (inclusive em grupos: `use crate::{a::X, b}`) e separa o
//! código de produção do código de teste: módulos declarados sob
//! `#[cfg(test)]`, itens e trechos sob `#[cfg(test)]` e o recurso
//! `ancoras-de-teste`, que só os testes ligam.
//!
//! Os ciclos de produção que já existiam ficam REGISTRADOS em
//! [`SETAS_EM_CICLO`]: cada entrada é uma seta `de → para` que está dentro de
//! um ciclo. O teste falha se aparecer uma seta nova dentro de um ciclo
//! (dependência circular nova) e também se uma seta registrada deixar de estar
//! em ciclo: aí ela tem de sair da lista, para o ciclo não poder voltar. Cada
//! etapa da modularização tira da lista os ciclos que desfez.
//!
//! Para ver o grafo inteiro (setas, contagens, componentes e pares):
//! `RUSTINHO_DEPENDENCIAS_MOSTRAR=1 cargo test -p rustinho-nucleo --test dependencias -- --nocapture`.
//!
//! Sem dependências novas: um leitor de fontes simples (comentários, textos,
//! textos crus, caracteres e tempos de vida) basta para achar os caminhos; os
//! testes `leitor_*` do fim do arquivo conferem o leitor.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

/// Setas de produção que estão dentro de um ciclo. Formato: módulo de origem e
/// os destinos que fecham ciclo. Uma etapa que desfaz um ciclo tira as setas
/// dele daqui; nunca se acrescenta uma seta (isso seria aceitar um ciclo novo).
///
/// - Etapa 0 (commit `f4ddbb5`): 137 setas, um componente de 34 módulos.
/// - Etapa 1: `VERSAO` desceu para a base e `registro → api` e `backup → api`
///   sumiram; com elas, 90 setas saíram de ciclo. Ficam 47, em 4 componentes
///   (2, 3, 7 e 10 módulos).
/// - Etapa 4 (ciclo D): o comprovante (`impressao`) recebe pronto o corpo da
///   DRE e da lista de compra; `impressao → dre` e `impressao → reposicao`
///   sumiram, e as 4 setas de `dre`/`impressao`/`reposicao` saíram. Ficam 43.
/// - Etapa 4 (ciclo C): as datas do `receber` foram para `tempo`, o cálculo
///   dos cheques de `vendas` para `cheques`, as duas consultas do resumo do
///   caixa para `caixa` e a OS que vira venda para `vendas`; 13 setas saíram.
///   Ficam 30; do componente C, só `cheques ↔ receber` (decisão do dono).
#[rustfmt::skip]
const SETAS_EM_CICLO: &[(&str, &[&str])] = &[
    ("arquivos", &["registro"]),
    ("backup", &["banco", "loja", "portatil", "sync"]),
    ("banco", &["backup", "faixas", "loja", "migracao", "portatil", "sync"]),
    ("caminhos", &["sync"]),
    ("cheques", &["receber"]),
    ("faixas", &["pessoas", "produtos", "sync"]),
    ("loja", &["banco", "faixas", "sync"]),
    ("migracao", &["banco"]),
    ("pessoas", &["faixas"]),
    ("portatil", &["sync"]),
    ("produtos", &["faixas"]),
    ("receber", &["cheques"]),
    ("registro", &["arquivos"]),
    ("sync", &["banco", "caminhos", "faixas", "loja", "portatil"]),
];

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

// ---------------------------------------------------------------- leitor

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tipo {
    Nome,
    Simbolo,
    Texto,
    Outro,
}

#[derive(Clone, Debug)]
struct Token {
    tipo: Tipo,
    texto: String,
    linha: usize,
}

fn letra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Divide o fonte em nomes, símbolos (`::` junto) e textos; comentários somem.
fn tokens(fonte: &str, onde: &str) -> Vec<Token> {
    let c: Vec<char> = fonte.chars().collect();
    let n = c.len();
    let em = |k: usize| if k < n { c[k] } else { '\0' };
    let mut v = Vec::new();
    let mut i = 0;
    let mut linha = 1;
    while i < n {
        let ch = c[i];
        let linha_do_token = linha;
        let mut por = |tipo: Tipo, texto: String| v.push(Token { tipo, texto, linha: linha_do_token });
        if ch == '\n' {
            linha += 1;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '/' && em(i + 1) == '/' {
            while i < n && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && em(i + 1) == '*' {
            let mut nivel = 0;
            while i < n {
                if c[i] == '/' && em(i + 1) == '*' {
                    nivel += 1;
                    i += 2;
                } else if c[i] == '*' && em(i + 1) == '/' {
                    nivel -= 1;
                    i += 2;
                    if nivel == 0 {
                        break;
                    }
                } else {
                    if c[i] == '\n' {
                        linha += 1;
                    }
                    i += 1;
                }
            }
            continue;
        }
        // Texto cru: r"…", r#"…"#, br"…", cr#"…"#.
        let cru = match ch {
            'r' => Some(i + 1),
            'b' | 'c' if em(i + 1) == 'r' => Some(i + 2),
            _ => None,
        };
        if let Some(mut j) = cru {
            let mut cercas = 0;
            while em(j) == '#' {
                cercas += 1;
                j += 1;
            }
            if em(j) == '"' {
                j += 1;
                let comeco = j;
                loop {
                    assert!(j < n, "{onde}:{linha_do_token}: texto cru sem fim");
                    if c[j] == '"' && (0..cercas).all(|k| em(j + 1 + k) == '#') {
                        break;
                    }
                    if c[j] == '\n' {
                        linha += 1;
                    }
                    j += 1;
                }
                por(Tipo::Texto, c[comeco..j].iter().collect());
                i = j + 1 + cercas;
                continue;
            }
        }
        // Texto: "…", b"…", c"…".
        let aspas = match ch {
            '"' => Some(i),
            'b' | 'c' if em(i + 1) == '"' => Some(i + 1),
            _ => None,
        };
        if let Some(a) = aspas {
            let mut j = a + 1;
            while j < n && c[j] != '"' {
                if c[j] == '\\' {
                    j += 1;
                }
                if j < n && c[j] == '\n' {
                    linha += 1;
                }
                j += 1;
            }
            assert!(j < n, "{onde}:{linha_do_token}: texto sem fim");
            por(Tipo::Texto, c[a + 1..j].iter().collect());
            i = j + 1;
            continue;
        }
        // Caractere ('a', '\n', '\'', b'x') ou tempo de vida ('a, 'static).
        let apostrofo = match ch {
            '\'' => Some(i),
            'b' if em(i + 1) == '\'' => Some(i + 1),
            _ => None,
        };
        if let Some(a) = apostrofo {
            if em(a + 1) == '\\' {
                let mut j = a + 3;
                while j < n && c[j] != '\'' {
                    j += 1;
                }
                i = j + 1;
            } else if em(a + 2) == '\'' {
                i = a + 3;
            } else {
                let mut j = a + 1;
                while j < n && letra(c[j]) {
                    j += 1;
                }
                i = j;
            }
            por(Tipo::Outro, String::new());
            continue;
        }
        if letra(ch) {
            let mut j = i;
            while j < n && letra(c[j]) {
                j += 1;
            }
            let tipo = if ch.is_ascii_digit() { Tipo::Outro } else { Tipo::Nome };
            por(tipo, c[i..j].iter().collect());
            i = j;
            continue;
        }
        if ch == ':' && em(i + 1) == ':' {
            por(Tipo::Simbolo, "::".into());
            i += 2;
            continue;
        }
        por(Tipo::Simbolo, ch.to_string());
        i += 1;
    }
    v
}

/// Para cada `(`, `[` e `{`, onde ele fecha (e vice-versa).
fn pares(t: &[Token], onde: &str) -> Vec<usize> {
    let mut par = vec![usize::MAX; t.len()];
    let mut pilha: Vec<usize> = Vec::new();
    for (i, k) in t.iter().enumerate() {
        if k.tipo != Tipo::Simbolo {
            continue;
        }
        let fecha = match k.texto.as_str() {
            "(" | "[" | "{" => {
                pilha.push(i);
                continue;
            }
            ")" => "(",
            "]" => "[",
            "}" => "{",
            _ => continue,
        };
        let a = pilha.pop().unwrap_or_else(|| panic!("{onde}:{}: '{}' sem abrir", k.linha, k.texto));
        assert_eq!(t[a].texto, fecha, "{onde}: '{}' da linha {} fechado na linha {}", t[a].texto, t[a].linha, k.linha);
        par[a] = i;
        par[i] = a;
    }
    if let Some(&a) = pilha.last() {
        panic!("{onde}:{}: '{}' sem fechar", t[a].linha, t[a].texto);
    }
    par
}

fn simbolo(t: &[Token], k: usize, s: &str) -> bool {
    t.get(k).is_some_and(|x| x.tipo == Tipo::Simbolo && x.texto == s)
}

fn nome(t: &[Token], k: usize, s: &str) -> bool {
    t.get(k).is_some_and(|x| x.tipo == Tipo::Nome && x.texto == s)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tri {
    Sim,
    Nao,
    Talvez,
}

/// Avalia um predicado de `cfg` sem saber a plataforma: `test` e o recurso
/// `ancoras-de-teste` valem "não"; o resto (windows, unix, target_os…) "talvez".
fn predicado(t: &[Token], p: &mut usize, onde: &str) -> Tri {
    let rotulo = t[*p].texto.clone();
    *p += 1;
    if simbolo(t, *p, "(") {
        *p += 1;
        let mut itens = Vec::new();
        while !simbolo(t, *p, ")") {
            itens.push(predicado(t, p, onde));
            if simbolo(t, *p, ",") {
                *p += 1;
            }
        }
        *p += 1;
        match rotulo.as_str() {
            "any" if itens.contains(&Tri::Sim) => Tri::Sim,
            "any" if itens.iter().all(|x| *x == Tri::Nao) => Tri::Nao,
            "all" if itens.contains(&Tri::Nao) => Tri::Nao,
            "all" if itens.iter().all(|x| *x == Tri::Sim) => Tri::Sim,
            "any" | "all" => Tri::Talvez,
            "not" => match itens.first() {
                Some(Tri::Sim) => Tri::Nao,
                Some(Tri::Nao) => Tri::Sim,
                _ => Tri::Talvez,
            },
            outro => panic!("{onde}: cfg({outro}…) desconhecido; ensine o leitor"),
        }
    } else if simbolo(t, *p, "=") {
        let valor = t[*p + 1].texto.clone();
        *p += 2;
        if rotulo == "feature" && valor == "ancoras-de-teste" {
            Tri::Nao
        } else {
            Tri::Talvez
        }
    } else if rotulo == "test" {
        Tri::Nao
    } else {
        Tri::Talvez
    }
}

/// Onde termina o item (ou a instrução) que começa em `inicio`: no `;`, no fim
/// do bloco `{…}` (a não ser que siga `else`, `.` ou `?`), na vírgula de um
/// braço de `match`, ou antes do fechamento do grupo de fora.
fn extensao(t: &[Token], par: &[usize], inicio: usize) -> usize {
    let mut j = inicio;
    let mut braco = false;
    while j < t.len() {
        if t[j].tipo == Tipo::Simbolo {
            match t[j].texto.as_str() {
                "(" | "[" => {
                    j = par[j] + 1;
                    continue;
                }
                "{" => {
                    j = par[j] + 1;
                    if nome(t, j, "else") || simbolo(t, j, ".") || simbolo(t, j, "?") {
                        continue;
                    }
                    return j;
                }
                ";" => return j + 1,
                "," if braco => return j + 1,
                ")" | "]" | "}" => return j,
                "=" if simbolo(t, j + 1, ">") => {
                    braco = true;
                    j += 2;
                    continue;
                }
                _ => {}
            }
        }
        j += 1;
    }
    t.len()
}

/// Os primeiros nomes depois de `crate::` (ou de `super::` até a raiz):
/// `crate::a::X` dá `a`; `crate::{a::X, b}` dá `a` e `b`.
fn alvos(t: &[Token], par: &[usize], j: usize) -> Vec<String> {
    if t.get(j).is_some_and(|x| x.tipo == Tipo::Nome) {
        return alvo_do_nome(t, par, j);
    }
    if !simbolo(t, j, "{") {
        return vec!["?".into()];
    }
    let mut v = Vec::new();
    let mut k = j + 1;
    let mut comeco = true;
    while k < par[j] {
        if comeco {
            match t[k].tipo {
                Tipo::Nome if t[k].texto != "self" => v.extend(alvo_do_nome(t, par, k)),
                Tipo::Nome => {}
                _ => v.push("?".into()),
            }
            comeco = false;
        }
        if simbolo(t, k, "(") || simbolo(t, k, "[") || simbolo(t, k, "{") {
            k = par[k];
        } else if simbolo(t, k, ",") {
            comeco = true;
        }
        k += 1;
    }
    v
}

/// O módulo de um nome depois de `crate::`: o próprio nome ou, se for uma
/// pasta de domínio, o módulo dentro dela (`crate::dados::esquema` é `esquema`).
/// Em `crate::a::b::…` fica `a::b`: se `b` for um módulo com arquivo próprio
/// dentro de `a` (`crate::compras::pendencias`, passo 3.4), o alvo é `b`; senão,
/// `a` (veja `ler_crate`).
fn alvo_do_nome(t: &[Token], par: &[usize], k: usize) -> Vec<String> {
    if PASTAS_DE_DOMINIO.contains(&t[k].texto.as_str()) && simbolo(t, k + 1, "::") {
        return alvos(t, par, k + 2);
    }
    if simbolo(t, k + 1, "::") && t.get(k + 2).is_some_and(|x| x.tipo == Tipo::Nome) && simbolo(t, k + 3, "::") {
        return vec![format!("{}::{}", t[k].texto, t[k + 2].texto)];
    }
    vec![t[k].texto.clone()]
}

#[derive(Debug)]
struct Referencia {
    /// Caminho do módulo de onde sai a referência (vazio = raiz da crate).
    de: Vec<String>,
    alvo: String,
    teste: bool,
    linha: usize,
}

#[derive(Debug)]
struct Declaracao {
    caminho: Vec<String>,
    arquivo: Option<String>,
    teste: bool,
}

#[derive(Debug)]
struct Inclusao {
    arquivo: String,
    caminho: Vec<String>,
    teste: bool,
}

#[derive(Debug, Default)]
struct Analise {
    refs: Vec<Referencia>,
    mods: Vec<Declaracao>,
    inclusoes: Vec<Inclusao>,
    /// Tabelas em que um texto de SQL grava (veja `escritas_sql`).
    escritas: Vec<Escrita>,
    /// Lugares que abrem uma transação (contrato C5, passo 3.5).
    transacoes: Vec<Transacao>,
    /// Tabelas que um texto de SQL lê (FROM/JOIN; veja `leituras_sql`). Só
    /// para o inventário da fase 8 (`RUSTINHO_DEPENDENCIAS_LUGARES`).
    leituras: Vec<Escrita>,
}

/// Uma abertura de transação: `.transaction()`, `.unchecked_transaction()`,
/// `.transaction_with_behavior(…)` ou um texto de SQL que começa com `BEGIN`.
#[derive(Debug)]
struct Transacao {
    de: Vec<String>,
    teste: bool,
    linha: usize,
}

#[derive(Debug)]
struct Escrita {
    de: Vec<String>,
    tabela: String,
    teste: bool,
    linha: usize,
    minuscula: bool,
}

/// Lê um arquivo que está no módulo `base`; `teste_inicial` quando o arquivo
/// inteiro é de teste.
fn analisar(fonte: &str, onde: &str, base: &[String], teste_inicial: bool) -> Analise {
    let t = tokens(fonte, onde);
    let par = pares(&t, onde);
    let n = t.len();
    let mut a = Analise::default();
    let mut fim_teste = if teste_inicial { usize::MAX } else { 0 };
    let mut blocos: Vec<(String, usize)> = Vec::new();
    let mut arquivo_pendente: Option<String> = None;
    let mut i = 0;
    while i < n {
        while blocos.last().is_some_and(|b| i > b.1) {
            blocos.pop();
        }
        let caminho: Vec<String> = base.iter().cloned().chain(blocos.iter().map(|b| b.0.clone())).collect();
        let teste = i < fim_teste;
        if simbolo(&t, i, "#") && (simbolo(&t, i + 1, "[") || (simbolo(&t, i + 1, "!") && simbolo(&t, i + 2, "["))) {
            let interno = simbolo(&t, i + 1, "!");
            let abre = if interno { i + 2 } else { i + 1 };
            let fecha = par[abre];
            let corpo = &t[abre + 1..fecha];
            if nome(corpo, 0, "cfg") && simbolo(corpo, 1, "(") {
                let mut p = 2;
                if predicado(corpo, &mut p, onde) == Tri::Nao {
                    let fim = if interno {
                        blocos.last().map_or(n, |b| b.1)
                    } else {
                        let mut s = fecha + 1;
                        while simbolo(&t, s, "#") && simbolo(&t, s + 1, "[") {
                            s = par[s + 1] + 1;
                        }
                        extensao(&t, &par, s)
                    };
                    fim_teste = fim_teste.max(fim);
                }
            } else if nome(corpo, 0, "path")
                && simbolo(corpo, 1, "=")
                && corpo.get(2).is_some_and(|x| x.tipo == Tipo::Texto)
            {
                arquivo_pendente = Some(corpo[2].texto.clone());
            }
            i = fecha + 1;
            continue;
        }
        if nome(&t, i, "mod") && t.get(i + 1).is_some_and(|x| x.tipo == Tipo::Nome) {
            let m = t[i + 1].texto.clone();
            if simbolo(&t, i + 2, ";") {
                let mut c = caminho.clone();
                c.push(m);
                a.mods.push(Declaracao { caminho: c, arquivo: arquivo_pendente.take(), teste });
                i += 3;
                continue;
            }
            if simbolo(&t, i + 2, "{") {
                blocos.push((m, par[i + 2]));
                arquivo_pendente = None;
                i += 3;
                continue;
            }
        }
        if nome(&t, i, "include")
            && simbolo(&t, i + 1, "!")
            && simbolo(&t, i + 2, "(")
            && t.get(i + 3).is_some_and(|x| x.tipo == Tipo::Texto)
            && simbolo(&t, i + 4, ")")
        {
            a.inclusoes.push(Inclusao { arquivo: t[i + 3].texto.clone(), caminho, teste });
            i += 5;
            continue;
        }
        if nome(&t, i, "crate") && simbolo(&t, i + 1, "::") && !(i > 0 && simbolo(&t, i - 1, "$")) {
            for alvo in alvos(&t, &par, i + 2) {
                a.refs.push(Referencia { de: caminho.clone(), alvo, teste, linha: t[i].linha });
            }
            i += 2;
            continue;
        }
        if nome(&t, i, "super") && simbolo(&t, i + 1, "::") && !(i > 0 && simbolo(&t, i - 1, "::")) {
            let mut subidas = 0;
            let mut j = i;
            while nome(&t, j, "super") && simbolo(&t, j + 1, "::") {
                subidas += 1;
                j += 2;
            }
            assert!(subidas <= caminho.len(), "{onde}:{}: super:: acima da raiz", t[i].linha);
            if subidas == caminho.len() {
                for alvo in alvos(&t, &par, j) {
                    a.refs.push(Referencia { de: caminho.clone(), alvo, teste, linha: t[i].linha });
                }
            }
            i = j;
            continue;
        }
        let abre_por_metodo = t[i].tipo == Tipo::Nome
            && ["transaction", "unchecked_transaction", "transaction_with_behavior"].contains(&t[i].texto.as_str())
            && i > 0
            && simbolo(&t, i - 1, ".")
            && simbolo(&t, i + 1, "(");
        let abre_por_sql = t[i].tipo == Tipo::Texto && t[i].texto.trim_start().to_uppercase().starts_with("BEGIN");
        if abre_por_metodo || abre_por_sql {
            a.transacoes.push(Transacao { de: caminho.clone(), teste, linha: t[i].linha });
        }
        if t[i].tipo == Tipo::Texto {
            for (tabela, minuscula) in escritas_sql_com_caixa(&t[i].texto) {
                a.escritas.push(Escrita { de: caminho.clone(), tabela, teste, linha: t[i].linha, minuscula });
            }
            for tabela in leituras_sql(&t[i].texto) {
                a.leituras.push(Escrita { de: caminho.clone(), tabela, teste, linha: t[i].linha, minuscula: false });
            }
        }
        i += 1;
    }
    a
}

/// Os nomes que o `lib.rs` reexporta e de onde vêm (`pub use erro::{Erro,
/// Resultado};` dá Erro → erro e Resultado → erro).
fn reexportes(fonte: &str, onde: &str) -> BTreeMap<String, String> {
    fn nomes(t: &[Token], par: &[usize], de: usize, ate: usize, v: &mut Vec<String>) {
        if ate > de && simbolo(t, ate - 1, "}") {
            let abre = par[ate - 1];
            let mut comeco = abre + 1;
            let mut k = abre + 1;
            while k < ate {
                if simbolo(t, k, "{") {
                    k = par[k];
                } else if simbolo(t, k, ",") || k == ate - 1 {
                    nomes(t, par, comeco, k, v);
                    comeco = k + 1;
                }
                k += 1;
            }
        } else if let Some(x) = t[de..ate].iter().rev().find(|x| x.tipo == Tipo::Nome && x.texto != "self") {
            v.push(x.texto.clone());
        }
    }
    let t = tokens(fonte, onde);
    let par = pares(&t, onde);
    let mut mapa = BTreeMap::new();
    let mut i = 0;
    while i < t.len() {
        if nome(&t, i, "pub") && nome(&t, i + 1, "use") {
            let fim = (i..t.len()).find(|&k| simbolo(&t, k, ";")).unwrap();
            let mut k = i + 2;
            if nome(&t, k, "crate") || nome(&t, k, "self") {
                k += 2;
            }
            let origem = t[k].texto.clone();
            let mut v = Vec::new();
            nomes(&t, &par, k + 1, fim, &mut v);
            for x in v {
                mapa.insert(x, origem.clone());
            }
            i = fim;
        }
        i += 1;
    }
    mapa
}

// ----------------------------------------------------- um dono por tabela

/// As tabelas em que cada texto de SQL grava (INSERT, REPLACE, UPDATE,
/// DELETE). Nome montado na hora (`{tabela}`, `{}`) vira "*". Eventos de
/// gatilho (`AFTER UPDATE ON t`) e `ON CONFLICT … DO UPDATE SET` não gravam
/// por si (o corpo do gatilho, sim).
///
/// Palavras-chave em maiúsculas OU em minúsculas (a dev.3 gravou
/// `produto_gtin` e `xml_pendencia_identificacao` em minúsculas e a trava não
/// as via: AUD33-K02); mistas ("Update") não contam. Para texto comum não
/// contar ("update depois"), o UPDATE só vale com SET logo depois do nome.
fn escritas_sql(texto: &str) -> Vec<String> {
    escritas_sql_com_caixa(texto).into_iter().map(|(t, _)| t).collect()
}

/// Como `escritas_sql`, dizendo também se a gravação estava em minúsculas.
fn escritas_sql_com_caixa(texto: &str) -> Vec<(String, bool)> {
    let p: Vec<String> = texto
        .split(|c: char| c.is_whitespace() || c == '(' || c == ',' || c == ';')
        .filter(|x| !x.is_empty())
        .map(String::from)
        .collect();
    // A palavra em maiúsculas, se estiver toda em maiúsculas ou toda em minúsculas.
    let maiusc = |k: usize| {
        let x = p.get(k).cloned().unwrap_or_default();
        if x == x.to_ascii_lowercase() {
            x.to_ascii_uppercase()
        } else {
            x
        }
    };
    let tabela = |k: usize| -> Option<String> {
        let t = p.get(k)?;
        if t.starts_with('{') {
            return Some("*".into());
        }
        let t = t.strip_prefix("main.").or_else(|| t.strip_prefix("temp.")).unwrap_or(t);
        let t = t.trim_matches('"');
        (t.chars().next()?.is_ascii_alphabetic() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .then(|| t.to_string())
    };
    let mut v = Vec::new();
    for k in 0..p.len() {
        let mut j = k + 1;
        let alvo = match maiusc(k).as_str() {
            // "INSERT OR REPLACE INTO": o REPLACE não é outra gravação.
            "REPLACE" if k > 0 && maiusc(k - 1) == "OR" => continue,
            "INSERT" | "REPLACE" => {
                if maiusc(j) == "OR" {
                    j += 2;
                }
                if maiusc(j) != "INTO" {
                    continue;
                }
                tabela(j + 1)
            }
            "UPDATE" => {
                if maiusc(j) == "OR" {
                    j += 2;
                }
                if ["SET", "ON", "OF"].contains(&maiusc(j).as_str()) || maiusc(j + 1) != "SET" {
                    continue;
                }
                tabela(j)
            }
            "DELETE" if maiusc(j) == "FROM" => tabela(j + 1),
            _ => continue,
        };
        if let Some(t) = alvo {
            v.push((t, p.get(k).is_some_and(|x| x.chars().any(|c| c.is_ascii_lowercase()))));
        }
    }
    v
}

/// As tabelas que um texto de SQL lê: o nome depois de FROM ou JOIN (em
/// maiúsculas ou minúsculas), menos o `DELETE FROM` (gravação). Subconsulta
/// (`FROM (SELECT`) e nome montado na hora não contam. Só para o inventário
/// da fase 8: o filtro pelas tabelas do esquema fica com quem imprime.
fn leituras_sql(texto: &str) -> Vec<String> {
    let p: Vec<&str> = texto
        .split(|c: char| c.is_whitespace() || c == ')' || c == ',' || c == ';')
        .filter(|x| !x.is_empty())
        .collect();
    let mut v = Vec::new();
    for k in 0..p.len() {
        let w = p[k];
        let chave = w == "FROM" || w == "from" || w == "JOIN" || w == "join";
        if !chave || (k > 0 && (p[k - 1] == "DELETE" || p[k - 1] == "delete")) {
            continue;
        }
        let Some(t) = p.get(k + 1) else { continue };
        let t = t.strip_prefix("main.").unwrap_or(t).trim_matches('"');
        if t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            v.push(t.to_string());
        }
    }
    v
}

// ----------------------------------------------------------------- grafo

#[derive(Debug, Default)]
struct Grafo {
    /// Módulos de primeiro nível e se são só de teste.
    modulos: BTreeMap<String, bool>,
    /// Setas entre módulos de primeiro nível, com quantas referências.
    producao: BTreeMap<(String, String), usize>,
    teste: BTreeMap<(String, String), usize>,
    /// Setas para nomes que o `lib.rs` reexporta de outra crate.
    externas: BTreeMap<(String, String), usize>,
    /// Primeiro lugar de cada seta de produção (para as mensagens).
    exemplo: BTreeMap<(String, String), String>,
    /// (tabela, módulo) que grava nela no código de produção, com o primeiro lugar.
    escritores: BTreeMap<(String, String), String>,
    /// Toda gravação de produção: (lugar, tabela, módulo, em minúsculas).
    gravacoes: Vec<(String, String, String, bool)>,
    /// Toda referência de produção entre módulos daqui: (de, para, lugar, alvo).
    lugares: Vec<(String, String, String, String)>,
    /// Toda leitura de produção: (lugar, tabela, módulo).
    leituras: Vec<(String, String, String)>,
    /// Módulo que abre a própria transação no código de produção, com o primeiro lugar.
    transacoes: BTreeMap<String, String>,
    /// Problemas de leitura (o teste falha se houver).
    problemas: Vec<String>,
}

/// Pastas de domínio (dev.3.3, passo 3.1) que só agrupam módulos: o lib.rs
/// os reexporta nos caminhos de antes. O `compras` é ao mesmo tempo a pasta e
/// o módulo de antes (`compras.rs` virou `compras/mod.rs`), por isso não está aqui.
const PASTAS_DE_DOMINIO: &[&str] = &["cadastros", "catalogo", "dados", "integracoes", "operacao", "servico", "troca"];

/// Módulos que também são pasta (o `compras.rs` virou `compras/mod.rs`): os
/// submódulos com arquivo próprio são módulos do grafo (`compras::pendencias`).
const MODULOS_PASTA: &[&str] = &["compras"];

/// O módulo de um caminho: o primeiro nome; dentro de uma pasta de domínio, o
/// módulo de dentro dela (`dados::banco` é `banco`; o `dados::esquema`, novo
/// da 3.2, também); no `compras`, os submódulos de arquivo próprio
/// (`importar`, reexportado pelo lib.rs, e o `pendencias`, novo da 3.4). Os
/// módulos escritos dentro do próprio arquivo (`compras::comandos`) são parte
/// dele. Assim o grafo fica igual ao de antes das pastas.
fn modulo_de(caminho: &[String], reexp: &BTreeMap<String, String>, modulos: &BTreeMap<String, bool>) -> Option<String> {
    match caminho {
        [d, m, ..]
            if PASTAS_DE_DOMINIO.contains(&d.as_str())
                || reexp.get(m) == Some(d)
                || (MODULOS_PASTA.contains(&d.as_str()) && modulos.contains_key(m)) =>
        {
            Some(m.clone())
        }
        [m, ..] => Some(m.clone()),
        [] => None,
    }
}

fn ler_crate(src: &Path) -> Grafo {
    let ler = |p: &Path| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let lib = src.join("lib.rs");
    let reexp = reexportes(&ler(&lib), &lib.display().to_string());
    let mut g = Grafo::default();
    let mut lidos = BTreeSet::new();
    let mut refs: Vec<(Referencia, String)> = Vec::new();
    let mut escritas: Vec<(Escrita, String)> = Vec::new();
    let mut transacoes: Vec<(Transacao, String)> = Vec::new();
    let mut leituras: Vec<(Escrita, String)> = Vec::new();
    let mut fila: VecDeque<(PathBuf, Vec<String>, bool)> = VecDeque::from([(lib.clone(), Vec::new(), false)]);
    while let Some((arq, caminho, teste)) = fila.pop_front() {
        let real = arq.canonicalize().unwrap_or_else(|e| panic!("{}: {e}", arq.display()));
        if !lidos.insert(real) {
            continue;
        }
        let onde = arq.strip_prefix(src).unwrap_or(&arq).display().to_string();
        let a = analisar(&ler(&arq), &onde, &caminho, teste);
        let pasta = arq.parent().unwrap().to_path_buf();
        let haste = arq.file_stem().unwrap().to_string_lossy().to_string();
        let filhos = if ["lib", "mod", "main"].contains(&haste.as_str()) { pasta.clone() } else { pasta.join(&haste) };
        for d in a.mods {
            let m = d.caminho.last().unwrap().clone();
            let pasta_de_dominio = d.caminho.len() == 1 && PASTAS_DE_DOMINIO.contains(&m.as_str());
            let de_dominio = d.caminho.len() == 2
                && (PASTAS_DE_DOMINIO.contains(&d.caminho[0].as_str())
                    || reexp.get(&m) == Some(&d.caminho[0])
                    || MODULOS_PASTA.contains(&d.caminho[0].as_str()));
            if (d.caminho.len() == 1 && !pasta_de_dominio) || de_dominio {
                g.modulos.insert(m.clone(), d.teste);
            }
            let arquivo = match d.arquivo {
                Some(p) => pasta.join(p),
                None => {
                    let mut base = filhos.clone();
                    for b in &d.caminho[caminho.len()..d.caminho.len() - 1] {
                        base.push(b);
                    }
                    let plano = base.join(format!("{m}.rs"));
                    if plano.exists() {
                        plano
                    } else {
                        base.join(&m).join("mod.rs")
                    }
                }
            };
            fila.push_back((arquivo, d.caminho, d.teste));
        }
        for inc in a.inclusoes {
            fila.push_back((pasta.join(&inc.arquivo), inc.caminho, inc.teste));
        }
        refs.extend(a.refs.into_iter().map(|r| (r, onde.clone())));
        escritas.extend(a.escritas.into_iter().map(|e| (e, onde.clone())));
        transacoes.extend(a.transacoes.into_iter().map(|x| (x, onde.clone())));
        leituras.extend(a.leituras.into_iter().map(|e| (e, onde.clone())));
    }
    // Todo arquivo .rs da pasta tem de ter sido lido (senão o leitor perdeu algum).
    let mut pendentes = vec![src.to_path_buf()];
    while let Some(p) = pendentes.pop() {
        for item in std::fs::read_dir(&p).unwrap() {
            let c = item.unwrap().path();
            if c.is_dir() {
                pendentes.push(c);
            } else if c.extension().is_some_and(|e| e == "rs") && !lidos.contains(&c.canonicalize().unwrap()) {
                g.problemas.push(format!("arquivo não alcançado pelas declarações mod: {}", c.display()));
            }
        }
    }
    for (r, onde) in refs {
        let Some(de) = modulo_de(&r.de, &reexp, &g.modulos) else { continue };
        let so_teste = g.modulos.get(&de).copied().unwrap_or(false);
        let teste = r.teste || so_teste;
        // Segue as reexportações do lib.rs até um módulo daqui ou outra crate:
        // `crate::Erro` → `erro` → `rustinho_base`.
        // `a::b` (veja `alvo_do_nome`): `b` se for módulo daqui; senão, `a`.
        let mut para = match r.alvo.split_once("::") {
            Some((_, filho)) if g.modulos.contains_key(filho) => filho.to_string(),
            Some((pai, _)) => pai.to_string(),
            None => r.alvo.clone(),
        };
        let mut passos = 0;
        while !g.modulos.contains_key(&para) && passos < 8 {
            match reexp.get(&para) {
                Some(origem) if *origem != para => para = origem.clone(),
                _ => break,
            }
            passos += 1;
        }
        let externa = !g.modulos.contains_key(&para);
        if externa && passos == 0 {
            g.problemas.push(format!("{onde}:{}: crate::{} não é módulo nem reexportação do lib.rs", r.linha, r.alvo));
            continue;
        }
        if para == de {
            continue;
        }
        let chave = (de, para);
        if externa {
            if !teste {
                *g.externas.entry(chave).or_default() += 1;
            }
        } else if teste {
            *g.teste.entry(chave).or_default() += 1;
        } else {
            if g.modulos[&chave.1] {
                g.problemas.push(format!("{onde}:{}: código de produção usa o módulo de teste {}", r.linha, chave.1));
            }
            g.exemplo.entry(chave.clone()).or_insert_with(|| format!("{onde}:{}", r.linha));
            g.lugares.push((chave.0.clone(), chave.1.clone(), format!("{onde}:{}", r.linha), r.alvo.clone()));
            *g.producao.entry(chave).or_default() += 1;
        }
    }
    for (e, onde) in escritas {
        let Some(de) = modulo_de(&e.de, &reexp, &g.modulos) else { continue };
        if e.teste || g.modulos.get(&de).copied().unwrap_or(false) {
            continue;
        }
        g.gravacoes.push((format!("{onde}:{}", e.linha), e.tabela.clone(), de.clone(), e.minuscula));
        g.escritores.entry((e.tabela, de)).or_insert_with(|| format!("{onde}:{}", e.linha));
    }
    for (e, onde) in leituras {
        let Some(de) = modulo_de(&e.de, &reexp, &g.modulos) else { continue };
        if e.teste || g.modulos.get(&de).copied().unwrap_or(false) {
            continue;
        }
        g.leituras.push((format!("{onde}:{}", e.linha), e.tabela, de));
    }
    for (x, onde) in transacoes {
        let Some(de) = modulo_de(&x.de, &reexp, &g.modulos) else { continue };
        // Aberta pelo atendente do comando (o submódulo `comandos` de cada
        // domínio): é a regra C5, não conta.
        if x.teste || g.modulos.get(&de).copied().unwrap_or(false) || x.de.iter().any(|n| n == "comandos") {
            continue;
        }
        g.transacoes.entry(de).or_insert_with(|| format!("{onde}:{}", x.linha));
    }
    g
}

/// Componentes fortemente conexos (Tarjan): módulo → número do componente.
fn componentes(setas: &BTreeSet<(String, String)>) -> BTreeMap<String, usize> {
    struct Estado<'a> {
        vizinhos: BTreeMap<&'a str, Vec<&'a str>>,
        indice: BTreeMap<&'a str, usize>,
        baixo: BTreeMap<&'a str, usize>,
        pilha: Vec<&'a str>,
        na_pilha: BTreeSet<&'a str>,
        proximo: usize,
        comp: BTreeMap<String, usize>,
        ncomp: usize,
    }
    fn visitar<'a>(e: &mut Estado<'a>, v: &'a str) {
        e.indice.insert(v, e.proximo);
        e.baixo.insert(v, e.proximo);
        e.proximo += 1;
        e.pilha.push(v);
        e.na_pilha.insert(v);
        for w in e.vizinhos.get(v).cloned().unwrap_or_default() {
            if !e.indice.contains_key(w) {
                visitar(e, w);
                let b = e.baixo[v].min(e.baixo[w]);
                e.baixo.insert(v, b);
            } else if e.na_pilha.contains(w) {
                let b = e.baixo[v].min(e.indice[w]);
                e.baixo.insert(v, b);
            }
        }
        if e.baixo[v] == e.indice[v] {
            loop {
                let w = e.pilha.pop().unwrap();
                e.na_pilha.remove(w);
                e.comp.insert(w.to_string(), e.ncomp);
                if w == v {
                    break;
                }
            }
            e.ncomp += 1;
        }
    }
    let mut e = Estado {
        vizinhos: BTreeMap::new(),
        indice: BTreeMap::new(),
        baixo: BTreeMap::new(),
        pilha: Vec::new(),
        na_pilha: BTreeSet::new(),
        proximo: 0,
        comp: BTreeMap::new(),
        ncomp: 0,
    };
    let mut nos = BTreeSet::new();
    for (a, b) in setas {
        e.vizinhos.entry(a.as_str()).or_default().push(b.as_str());
        nos.insert(a.as_str());
        nos.insert(b.as_str());
    }
    for v in nos {
        if !e.indice.contains_key(v) {
            visitar(&mut e, v);
        }
    }
    e.comp
}

/// As setas que estão dentro de algum ciclo.
fn setas_em_ciclo(setas: &BTreeSet<(String, String)>) -> BTreeSet<(String, String)> {
    let comp = componentes(setas);
    setas.iter().filter(|(a, b)| comp[a] == comp[b]).cloned().collect()
}

/// Um caminho de `de` até `ate` (o mais curto), para mostrar o ciclo.
fn caminho(setas: &BTreeSet<(String, String)>, de: &str, ate: &str) -> Vec<String> {
    let mut veio: BTreeMap<String, String> = BTreeMap::new();
    let mut fila = VecDeque::from([de.to_string()]);
    while let Some(v) = fila.pop_front() {
        if v == ate {
            let mut c = vec![v.clone()];
            let mut x = v;
            while let Some(p) = veio.get(&x) {
                c.push(p.clone());
                x = p.clone();
            }
            c.reverse();
            return c;
        }
        for (_, b) in setas.range((v.clone(), String::new())..).take_while(|(a, _)| *a == v) {
            if b != de && !veio.contains_key(b) {
                veio.insert(b.clone(), v.clone());
                fila.push_back(b.clone());
            }
        }
    }
    Vec::new()
}

fn mostrar(g: &Grafo) {
    let setas: BTreeSet<_> = g.producao.keys().cloned().collect();
    let comp = componentes(&setas);
    let prod: Vec<_> = g.modulos.iter().filter(|(_, t)| !**t).map(|(m, _)| m.as_str()).collect();
    let test: Vec<_> = g.modulos.iter().filter(|(_, t)| **t).map(|(m, _)| m.as_str()).collect();
    println!("== módulos de produção ({}): {}", prod.len(), prod.join(" "));
    println!("== módulos só de teste ({}): {}", test.len(), test.join(" "));
    println!("== setas de produção: {} ({} referências)", g.producao.len(), g.producao.values().sum::<usize>());
    for ((a, b), n) in &g.producao {
        println!("P {a} -> {b} {n}");
    }
    println!("== setas só de teste: {} ({} referências)", g.teste.len(), g.teste.values().sum::<usize>());
    for ((a, b), n) in &g.teste {
        println!("T {a} -> {b} {n}");
    }
    println!("== setas para outra crate (reexportadas no lib.rs): {}", g.externas.len());
    for ((a, b), n) in &g.externas {
        println!("X {a} -> {b} {n}");
    }
    let mut grupos: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for (m, c) in &comp {
        grupos.entry(*c).or_default().push(m);
    }
    let ciclicos: Vec<_> = grupos.values().filter(|v| v.len() > 1).collect();
    println!("== componentes com ciclo (produção): {}", ciclicos.len());
    for v in &ciclicos {
        println!("C {} módulos: {}", v.len(), v.join(" "));
    }
    let em_ciclo = setas_em_ciclo(&setas);
    println!("== setas em ciclo (produção): {}", em_ciclo.len());
    println!("== pares mútuos (produção; ida/volta; entre parênteses contando os testes):");
    for ((a, b), n) in &g.producao {
        if a < b {
            if let Some(m) = g.producao.get(&(b.clone(), a.clone())) {
                let ta = n + g.teste.get(&(a.clone(), b.clone())).unwrap_or(&0);
                let tb = m + g.teste.get(&(b.clone(), a.clone())).unwrap_or(&0);
                println!("M {a} <-> {b} {n}/{m} ({ta}/{tb})");
            }
        }
    }
    println!("== pares mútuos só ao contar os testes (produção ida/volta; teste ida/volta):");
    let conta = |m: &BTreeMap<(String, String), usize>, a: &str, b: &str| {
        m.get(&(a.to_string(), b.to_string())).copied().unwrap_or(0)
    };
    let todas: BTreeSet<_> = g.producao.keys().chain(g.teste.keys()).cloned().collect();
    for (a, b) in &todas {
        if a < b && todas.contains(&(b.clone(), a.clone())) {
            let (pi, pv) = (conta(&g.producao, a, b), conta(&g.producao, b, a));
            if pi == 0 || pv == 0 {
                println!("MT {a} <-> {b} {pi}/{pv} {}/{}", conta(&g.teste, a, b), conta(&g.teste, b, a));
            }
        }
    }
    println!("== por módulo (produção): saídas / entradas");
    for m in &prod {
        let s = g.producao.keys().filter(|(a, _)| a == m).count();
        let e = g.producao.keys().filter(|(_, b)| b == m).count();
        println!("F {m} {s} {e}");
    }
    println!("== SETAS_EM_CICLO como está hoje:");
    let mut por: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (a, b) in &em_ciclo {
        por.entry(a).or_default().push(format!("\"{b}\""));
    }
    for (a, bs) in por {
        println!("    (\"{a}\", &[{}]),", bs.join(", "));
    }
}

fn setas_registradas() -> BTreeSet<(String, String)> {
    let mut v = BTreeSet::new();
    for (a, bs) in SETAS_EM_CICLO {
        for b in *bs {
            assert!(v.insert((a.to_string(), b.to_string())), "seta repetida na lista: {a} → {b}");
        }
    }
    v
}

#[test]
fn nenhum_ciclo_novo_entre_os_modulos_do_nucleo() {
    let src = raiz().join("nucleo/src");
    let g = ler_crate(&src);
    assert!(g.problemas.is_empty(), "o leitor de fontes não entendeu tudo: {:#?}", g.problemas);
    assert!(g.modulos.len() > 40 && !g.producao.is_empty(), "o leitor achou pouco: {:?}", g.modulos.keys());
    if std::env::var_os("RUSTINHO_DEPENDENCIAS_MOSTRAR").is_some() {
        mostrar(&g);
    }
    let setas: BTreeSet<_> = g.producao.keys().cloned().collect();
    let em_ciclo = setas_em_ciclo(&setas);
    let registradas = setas_registradas();
    println!(
        "{} módulos, {} setas de produção, {} em ciclo ({} registradas)",
        g.modulos.len(),
        setas.len(),
        em_ciclo.len(),
        registradas.len()
    );
    let mut erros = Vec::new();
    for (a, b) in em_ciclo.difference(&registradas) {
        let volta = caminho(&setas, b, a);
        erros.push(format!(
            "dependência circular NOVA: {a} → {b} ({} referência(s), a primeira em {}); fecha o ciclo {a} → {}",
            g.producao[&(a.clone(), b.clone())],
            g.exemplo[&(a.clone(), b.clone())],
            volta.join(" → ")
        ));
    }
    for (a, b) in registradas.difference(&em_ciclo) {
        erros.push(format!(
            "{a} → {b} não está mais em ciclo: tire-a de SETAS_EM_CICLO (assim o ciclo não pode voltar)"
        ));
    }
    assert!(erros.is_empty(), "{}", erros.join("\n"));
}

/// A base (`rustinho-base`, etapa 1) fica embaixo de tudo: sem ciclo entre os
/// seus módulos, sem depender de nenhuma crate do Rustinho e sem SQL (do
/// SQLite só aparece o tipo `rusqlite::Error`, em `erro.rs`, que o núcleo
/// examina em `Erro::Banco`). E a versão que ela dá é a do núcleo.
#[test]
fn base_fica_embaixo_sem_sql_e_sem_ciclos() {
    let pasta = raiz().join("base");
    let g = ler_crate(&pasta.join("src"));
    assert!(g.problemas.is_empty(), "o leitor de fontes não entendeu a base: {:#?}", g.problemas);
    let setas: BTreeSet<_> = g.producao.keys().cloned().collect();
    let em_ciclo = setas_em_ciclo(&setas);
    assert!(em_ciclo.is_empty(), "ciclo dentro da base: {em_ciclo:?}");
    println!("base: {} módulos, {} setas de produção: {setas:?}", g.modulos.len(), setas.len());

    let manifesto = std::fs::read_to_string(pasta.join("Cargo.toml")).unwrap();
    let mut secao = String::new();
    for linha in manifesto.lines().map(str::trim) {
        if linha.starts_with('[') {
            secao = linha.to_string();
        } else if secao.contains("dependencies") {
            assert!(!linha.starts_with("rustinho"), "a base não pode depender do Rustinho ({secao}: {linha})");
        }
    }

    let mut pendentes = vec![pasta.join("src")];
    while let Some(p) = pendentes.pop() {
        for item in std::fs::read_dir(&p).unwrap() {
            let c = item.unwrap().path();
            if c.is_dir() {
                pendentes.push(c);
                continue;
            }
            if c.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let t = tokens(&std::fs::read_to_string(&c).unwrap(), &c.display().to_string());
            for (i, k) in t.iter().enumerate() {
                if k.tipo == Tipo::Nome && k.texto == "rusqlite" {
                    let so_o_tipo = simbolo(&t, i + 1, "::") && nome(&t, i + 2, "Error") && !simbolo(&t, i + 3, "::");
                    assert!(
                        c.ends_with("erro.rs") && so_o_tipo,
                        "{}:{}: a base só pode citar o tipo rusqlite::Error, em erro.rs",
                        c.display(),
                        k.linha
                    );
                }
            }
        }
    }

    assert_eq!(rustinho_nucleo::VERSAO, env!("CARGO_PKG_VERSION"), "a versão da base é a do núcleo");
}

/// Quem grava em cada tabela no código de produção (etapa 4 da
/// modularização: "um dono por tabela"). Desde a dev.3.3, passo 3.2 (ARQ-10),
/// a lista sai do `dados::esquema` do núcleo (os `escritores` de cada tabela),
/// a mesma fonte da `sync::TABELAS`: tabela e os módulos que gravam nela
/// (INSERT, REPLACE, UPDATE ou DELETE num texto de SQL; ler não conta). Um
/// módulo que passe a gravar numa tabela em que não gravava faz o teste falhar
/// — a gravação nova vai pela função do dono, como `pagar::criar_da_compra` —,
/// e um escritor que deixou de gravar tem de sair do esquema (para não voltar).
/// `*` = nome de tabela montado na hora (`esquema::GRAVAM_QUALQUER_TABELA`).
/// As tabelas com mais de um escritor são as que ainda precisam de um dono só
/// (ARQ-01).
///
/// Para ver o que o código grava hoje: `RUSTINHO_DEPENDENCIAS_MOSTRAR=1 cargo
/// test -p rustinho-nucleo --test dependencias -- --nocapture nenhum_escritor_novo`.
///
/// Dev.3.3, passo 1.4 (AUD33-K02): as duas tabelas da migração 0024
/// (`produto_gtin`, `xml_pendencia_identificacao`) são gravadas em minúsculas.
/// Se o leitor voltar a não ver minúsculas, elas "deixam de ter escritor" e o
/// teste falha.
fn escritores() -> Vec<(&'static str, &'static [&'static str])> {
    use rustinho_nucleo::dados::esquema;
    let mut v = vec![("*", esquema::GRAVAM_QUALQUER_TABELA)];
    v.extend(esquema::TABELAS.iter().chain(esquema::TEMPORARIAS).map(|t| (t.nome, t.escritores)));
    v
}

/// Dev.3.3, passo 3.5 (contrato C5; a regra está no cabeçalho de
/// `servico/despacho.rs`): o comando abre a transação e o domínio recebe
/// `&Transaction`. Abrem pela regra o serviço e o banco (as migrações); os
/// atendentes dos comandos (o submódulo `comandos` de cada domínio) não contam.
const ABREM_PELA_REGRA: &[&str] = &["api", "banco"];

/// Os módulos que ainda abrem a própria transação, como estavam antes da
/// regra. Cada um passa a receber a do comando quando for mexido na fase dele
/// (6 a 8). Módulo novo abrindo a própria transação faz o teste falhar; um que
/// deixou de abrir tem de sair daqui (para não voltar).
const TRANSACAO_PROPRIA: &[&str] = &["backup", "faixas", "loja", "sync"];

#[test]
fn so_os_modulos_de_hoje_abrem_a_propria_transacao() {
    let g = ler_crate(&raiz().join("nucleo/src"));
    assert!(g.problemas.is_empty(), "o leitor de fontes não entendeu tudo: {:#?}", g.problemas);
    if std::env::var_os("RUSTINHO_DEPENDENCIAS_MOSTRAR").is_some() {
        println!("== abrem a própria transação ({}):", g.transacoes.len());
        for (m, lugar) in &g.transacoes {
            println!("TX {m} {lugar}");
        }
    }
    let mut erros = Vec::new();
    for (m, lugar) in &g.transacoes {
        if !ABREM_PELA_REGRA.contains(&m.as_str()) && !TRANSACAO_PROPRIA.contains(&m.as_str()) {
            erros.push(format!(
                "NOVO: {m} abre a própria transação (primeiro em {lugar}); pela regra C5 o comando abre e o domínio recebe &Transaction"
            ));
        }
    }
    for m in TRANSACAO_PROPRIA.iter().chain(ABREM_PELA_REGRA) {
        if !g.transacoes.contains_key(*m) {
            erros.push(format!("{m} não abre mais a própria transação: tire-o da lista (assim não volta)"));
        }
    }
    assert!(erros.is_empty(), "{}", erros.join("\n"));
}

#[test]
fn nenhum_escritor_novo_por_tabela() {
    let g = ler_crate(&raiz().join("nucleo/src"));
    assert!(g.problemas.is_empty(), "o leitor de fontes não entendeu tudo: {:#?}", g.problemas);
    let atuais: BTreeSet<(String, String)> = g.escritores.keys().cloned().collect();
    let mut por_tabela: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (t, m) in &atuais {
        por_tabela.entry(t).or_default().push(m);
    }
    if std::env::var_os("RUSTINHO_DEPENDENCIAS_MOSTRAR").is_some() {
        println!("== ESCRITORES como está hoje ({} tabelas, {} pares):", por_tabela.len(), atuais.len());
        for (t, ms) in &por_tabela {
            let v: Vec<String> = ms.iter().map(|m| format!("\"{m}\"")).collect();
            println!("    (\"{t}\", &[{}]),", v.join(", "));
        }
        println!("== gravações em minúsculas (AUD33-K02):");
        for (lugar, t, m, _) in g.gravacoes.iter().filter(|x| x.3) {
            println!("K02 {lugar}: {m} grava {t}");
        }
        println!("== tabelas com mais de um escritor:");
        for (t, ms) in por_tabela.iter().filter(|(_, ms)| ms.len() > 1) {
            println!("E {t}: {}", ms.join(", "));
        }
    }
    let lista = escritores();
    let mut registrados = BTreeSet::new();
    for (t, ms) in &lista {
        for m in *ms {
            assert!(registrados.insert((t.to_string(), m.to_string())), "repetido na lista: {t} ← {m}");
        }
    }
    let mut erros = Vec::new();
    for (t, m) in atuais.difference(&registrados) {
        let outros: Vec<&str> = lista.iter().filter(|(x, _)| x == t).flat_map(|(_, ms)| ms.iter().copied()).collect();
        erros.push(format!(
            "escritor NOVO: {m} grava a tabela {t} (primeiro em {}); quem já gravava: {outros:?}",
            g.escritores[&(t.clone(), m.clone())]
        ));
    }
    for (t, m) in registrados.difference(&atuais) {
        erros.push(format!("{m} não grava mais a tabela {t}: tire-o do dados::esquema (assim não volta)"));
    }
    assert!(erros.is_empty(), "{}", erros.join("\n"));
    assert!(atuais.len() > 40, "o leitor achou poucas gravações: {}", atuais.len());
}

/// Inventário da fase 8 (preparação, prep 8/9): com
/// `RUSTINHO_DEPENDENCIAS_LUGARES=1 … -- --nocapture lugares_da_fase_8`
/// imprime cada referência das setas em ciclo (LC), cada gravação das tabelas
/// com mais de um escritor (LW) e cada leitura de tabela do esquema feita fora
/// dos escritores dela (LR). Sem a variável não faz nada.
#[test]
fn lugares_da_fase_8() {
    if std::env::var_os("RUSTINHO_DEPENDENCIAS_LUGARES").is_none() {
        return;
    }
    let g = ler_crate(&raiz().join("nucleo/src"));
    assert!(g.problemas.is_empty(), "o leitor de fontes não entendeu tudo: {:#?}", g.problemas);
    let setas: BTreeSet<_> = g.producao.keys().cloned().collect();
    let em_ciclo = setas_em_ciclo(&setas);
    println!("== LC: referências das setas em ciclo (de, para, lugar, alvo)");
    for (de, para, lugar, alvo) in &g.lugares {
        if em_ciclo.contains(&(de.clone(), para.clone())) {
            println!("LC\t{de}\t{para}\t{lugar}\t{alvo}");
        }
    }
    let mut por_tabela: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (t, m) in g.escritores.keys() {
        por_tabela.entry(t).or_default().insert(m);
    }
    println!("== LW: gravações das tabelas com mais de um escritor (tabela, módulo, lugar)");
    for (lugar, t, m, _) in &g.gravacoes {
        if por_tabela.get(t.as_str()).is_some_and(|ms| ms.len() > 1) {
            println!("LW\t{t}\t{m}\t{lugar}");
        }
    }
    let tabelas: BTreeMap<&str, &[&str]> = escritores().into_iter().collect();
    println!("== LR: leituras fora dos escritores da tabela (tabela, módulo, escritores, lugar)");
    for (lugar, t, m) in &g.leituras {
        if let Some(ms) = tabelas.get(t.as_str()) {
            if *t != "*" && !ms.contains(&m.as_str()) {
                println!("LR\t{t}\t{m}\t{}\t{lugar}", ms.join(","));
            }
        }
    }
}

// ------------------------------------------------- conferência do leitor

fn refs(fonte: &str) -> Vec<(String, bool)> {
    analisar(fonte, "trecho", &["m".to_string()], false).refs.into_iter().map(|r| (r.alvo, r.teste)).collect()
}

#[test]
fn leitor_ignora_comentarios_textos_e_caracteres() {
    let fonte = r##"
        // crate::a
        /* crate::b /* crate::c */ crate::d */
        /// [`crate::e`]
        const T: &str = "crate::f \" crate::g";
        const U: &str = r#"crate::h "  crate::i"#;
        const C: char = '"';
        const D: char = '\'';
        fn f<'a>(x: &'a str) -> &'a str { x }
        use crate::certo::X;
    "##;
    assert_eq!(refs(fonte), vec![("certo".to_string(), false)]);
}

#[test]
fn leitor_separa_producao_de_teste() {
    let fonte = r#"
        #[cfg(test)]
        mod testes { use crate::a; fn f() { crate::b::g(); } }
        #[cfg(not(test))]
        use crate::c;
        #[cfg(any(windows, test))]
        use crate::d;
        #[cfg(any(test, feature = "ancoras-de-teste"))]
        pub fn de_teste() { crate::e::x() }
        fn h() {
            #[cfg(test)]
            if crate::f::y() { crate::g::z() } else { crate::h::w() }
            crate::i::v();
            match k {
                #[cfg(test)]
                "teste" => crate::j::u(),
                _ => crate::k::t(),
            }
        }
    "#;
    let r = refs(fonte);
    let teste: Vec<_> = r.iter().filter(|x| x.1).map(|x| x.0.as_str()).collect();
    let producao: Vec<_> = r.iter().filter(|x| !x.1).map(|x| x.0.as_str()).collect();
    assert_eq!(teste, ["a", "b", "e", "f", "g", "h", "j"]);
    assert_eq!(producao, ["c", "d", "i", "k"]);
}

#[test]
fn leitor_entende_grupos_super_e_modulos_em_bloco() {
    let fonte = r#"
        use crate::{a::X, b::{Y, Z}, self as raiz};
        use super::c::W;
        pub(crate) fn f() {}
        pub(super) fn g() {}
        mod dentro { use super::*; use super::super::d; }
        macro_rules! m { () => { $crate::e::f() } }
    "#;
    let r: Vec<_> = refs(fonte).into_iter().map(|x| x.0).collect();
    assert_eq!(r, ["a", "b", "c", "d"]);
}

#[test]
fn leitor_entende_modulo_dentro_de_pasta_de_dominio() {
    let fonte = r#"
        use crate::dados::esquema::TABELAS;
        use crate::{troca::sync, catalogo::{busca, produtos::X}, banco};
        fn f() -> usize { crate::dados::esquema::QUE_VIAJAM.len() }
    "#;
    let r: Vec<_> = refs(fonte).into_iter().map(|x| x.0).collect();
    assert_eq!(r, ["esquema", "sync", "busca", "produtos", "banco", "esquema"]);
}

#[test]
fn leitor_guarda_o_submodulo_de_um_modulo_pasta() {
    let fonte = r#"
        use crate::compras::pendencias::{registrar, resolver};
        fn f() { crate::compras::lancar(); crate::compras::Cabecalho::novo(); }
    "#;
    let r: Vec<_> = refs(fonte).into_iter().map(|x| x.0).collect();
    assert_eq!(r, ["compras::pendencias", "compras", "compras::Cabecalho"]);
}

#[test]
fn leitor_acha_aberturas_de_transacao() {
    let fonte = r#"
        fn a(c: &mut Connection) { let tx = c.transaction()?; }
        fn b(c: &Connection) { c.unchecked_transaction()?; c.execute_batch("BEGIN IMMEDIATE")?; }
        fn transaction() {} // não é abertura
        #[cfg(test)]
        fn t(c: &mut Connection) { c.transaction_with_behavior(x)?; }
    "#;
    let a = analisar(fonte, "trecho", &["m".to_string()], false);
    let v: Vec<(usize, bool)> = a.transacoes.iter().map(|x| (x.linha, x.teste)).collect();
    assert_eq!(v, [(2, false), (3, false), (3, false), (6, true)]);
}

#[test]
fn leitor_acha_ciclos() {
    let s = |v: &[(&str, &str)]| -> BTreeSet<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    };
    let setas = s(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "d"), ("d", "e"), ("e", "d"), ("f", "a")]);
    assert_eq!(setas_em_ciclo(&setas), s(&[("a", "b"), ("b", "c"), ("c", "a"), ("d", "e"), ("e", "d")]));
    assert_eq!(caminho(&setas, "b", "a"), ["b", "c", "a"]);
}

#[test]
fn leitor_acha_leituras_sql() {
    assert_eq!(leituras_sql("SELECT a FROM venda v JOIN venda_item i ON 1"), ["venda", "venda_item"]);
    assert_eq!(leituras_sql("select x from main.pessoa where id in (select y from cheque)"), ["pessoa", "cheque"]);
    assert!(leituras_sql("DELETE FROM venda WHERE id = 1").is_empty());
    assert!(leituras_sql("SELECT 1 FROM (SELECT 2)").is_empty());
}

#[test]
fn leitor_acha_escritas_sql() {
    assert_eq!(escritas_sql("INSERT INTO venda (id) VALUES (?1)"), ["venda"]);
    assert_eq!(escritas_sql("INSERT OR REPLACE INTO config(chave, valor) VALUES (?1, ?2)"), ["config"]);
    // K02: minúsculas contam (as seis gravações da dev.3 são assim); mistas, não.
    assert_eq!(escritas_sql("insert into config values (1)"), ["config"]);
    assert_eq!(escritas_sql("insert or ignore into produto_gtin (id) values (?1)"), ["produto_gtin"]);
    assert_eq!(escritas_sql_com_caixa("delete from a; DELETE FROM b"), [("a".into(), true), ("b".into(), false)]);
    assert_eq!(
        escritas_sql("update xml_pendencia_identificacao\n                set situacao = 'resolvida'"),
        ["xml_pendencia_identificacao"]
    );
    assert!(escritas_sql("Insert Into config values (1)").is_empty());
    assert!(escritas_sql("depois do update, delete o arquivo; insert depois").is_empty());
    assert_eq!(escritas_sql("UPDATE OR IGNORE produto SET x = 1"), ["produto"]);
    assert_eq!(escritas_sql("DELETE FROM main.sync_par WHERE id = ?1"), ["sync_par"]);
    assert_eq!(escritas_sql("REPLACE INTO a (x) VALUES (1); UPDATE b SET y = 2;"), ["a", "b"]);
    assert_eq!(escritas_sql("INSERT INTO {tabela} ({colunas}) VALUES ({marcas})"), ["*"]);
    assert_eq!(escritas_sql("INSERT INTO x (id) VALUES (1) ON CONFLICT(id) DO UPDATE SET y = 2"), ["x"]);
    assert!(escritas_sql("CREATE TRIGGER t AFTER UPDATE OF x ON venda BEGIN SELECT 1; END").is_empty());
    assert!(escritas_sql("CREATE TRIGGER t BEFORE DELETE ON venda BEGIN SELECT 1; END").is_empty());
    assert_eq!(
        escritas_sql("CREATE TRIGGER t AFTER INSERT ON venda BEGIN INSERT INTO alteracao (t) VALUES (1); END"),
        ["alteracao"]
    );
    assert!(escritas_sql("SELECT * FROM venda WHERE situacao = 'atualizar'").is_empty());
    assert!(escritas_sql("Não deu para apagar (delete) o arquivo; update depois").is_empty());
}
