//! Ajudantes dos testes de estabilidade: uma loja de teste num arquivo (como
//! no computador da loja), comandos pela mesma porta que as telas usam e uma
//! conexão à parte com o banco para conferir (ou "voltar no tempo") por SQL.
//!
//! Tudo com dados de mentira, em pastas temporárias apagadas no fim.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rustinho_nucleo::Servico;
use serde_json::{json, Value};

/// Senhas das pessoas de teste (só o tamanho conta: mínimo 4).
pub const SENHA_ANA: &str = "1234";
pub const SENHA_BIA: &str = "1111";

pub fn pasta_nova(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rustinho-estavel-{nome}-{}-{}",
        std::process::id(),
        rustinho_nucleo::tempo::novo_id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Uma loja de teste.
pub struct Loja {
    pub s: Option<Servico>,
    pub pasta: PathBuf,
    pub db: PathBuf,
    /// Apagar a pasta no fim (as lojas abertas de novo na mesma pasta não apagam).
    pub dona: bool,
}

impl Drop for Loja {
    fn drop(&mut self) {
        self.s.take();
        if self.dona {
            let _ = std::fs::remove_dir_all(&self.pasta);
        }
    }
}

impl Loja {
    /// Abre (ou cria) o banco em `pasta/rustinho.db`, sem configurar.
    pub fn abrir_em(pasta: &Path, dona: bool) -> Loja {
        let db = pasta.join("rustinho.db");
        let s = Servico::abrir(&db).unwrap();
        Loja { s: Some(s), pasta: pasta.to_path_buf(), db, dona }
    }

    /// Loja nova, configurada: Ana (gerente, senha 1234) conectada.
    pub fn nova(nome: &str) -> Loja {
        let pasta = pasta_nova(nome);
        let mut l = Loja::abrir_em(&pasta, true);
        l.ok(
            "configurar_inicio",
            json!({ "loja": "Loja Teste Auto Peças", "caixa": "Caixa 1", "nome": "Ana",
                    "senha": SENHA_ANA, "senha2": SENHA_ANA }),
        );
        l
    }

    pub fn servico(&mut self) -> &mut Servico {
        self.s.as_mut().expect("serviço aberto")
    }

    /// Fecha o Rustinho (solta a trava).
    pub fn fechar(&mut self) {
        self.s.take();
    }

    /// Abre de novo na mesma pasta.
    pub fn reabrir(&mut self) {
        self.s.take();
        self.s = Some(Servico::abrir(&self.db).unwrap());
    }

    pub fn chamar(&mut self, comando: &str, dados: Value) -> Value {
        let texto = dados.to_string();
        let servico = self.servico();
        let r = crate::apoio_chave::chamar(rustinho_nucleo::api::NAO_REPETIR, comando, &texto, |d| {
            servico.chamar(comando, d)
        });
        serde_json::from_str(&r).expect("resposta em JSON")
    }

    pub fn ok(&mut self, comando: &str, dados: Value) -> Value {
        let r = self.chamar(comando, dados.clone());
        assert_eq!(r["ok"], true, "{comando} {dados}: {r}");
        r["dados"].clone()
    }

    /// Exige erro; devolve a mensagem.
    pub fn erro(&mut self, comando: &str, dados: Value) -> String {
        let r = self.chamar(comando, dados.clone());
        assert_eq!(r["ok"], false, "{comando} {dados} deveria falhar: {r}");
        r["erro"].as_str().unwrap_or_default().to_string()
    }

    pub fn entrar(&mut self, nome: &str, senha: &str) {
        self.ok("sair", json!({}));
        let lista = self.ok("usuarios_entrada", json!({}));
        let id = lista
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["nome"] == nome)
            .unwrap_or_else(|| panic!("sem usuário {nome}: {lista}"))["id"]
            .as_str()
            .unwrap()
            .to_string();
        self.ok("entrar", json!({ "id": id, "senha": senha }));
    }

    /// Conexão à parte com o banco (para conferir e, nos testes, mudar datas).
    pub fn conexao(&self) -> Connection {
        let c = Connection::open(&self.db).unwrap();
        c.busy_timeout(std::time::Duration::from_secs(10)).unwrap();
        // Os gatilhos da troca de dados usam as funções do Rustinho.
        rustinho_nucleo::banco::registrar_funcoes(&c).unwrap();
        c
    }

    pub fn numero(&self, sql: &str) -> i64 {
        self.conexao().query_row(sql, [], |l| l.get(0)).unwrap()
    }

    pub fn executar(&self, sql: &str) {
        self.conexao().execute_batch(sql).unwrap();
    }

    // ---- Cadastros ----

    /// Peça com custo (vazio = sem custo), preço da Revenda e estoque inicial.
    pub fn peca(&mut self, nome: &str, custo: &str, preco: &str, estoque: &str) -> String {
        let mut f = self.ok("produto_novo", json!({}));
        f["nome"] = json!(nome);
        f["custo"] = json!(custo);
        f["estoqueInicial"] = json!(estoque);
        f["precos"][0]["preco"] = json!(preco);
        f["precos"][0]["lucro"] = json!("");
        self.ok("produto_salvar", f)["id"].as_str().unwrap().to_string()
    }

    pub fn pessoa(&mut self, papel: &str, nome: &str) -> String {
        let mut f = self.ok("pessoa_nova", json!({ "papel": papel }));
        f["nome"] = json!(nome);
        self.ok("pessoa_salvar", f)["id"].as_str().unwrap().to_string()
    }

    /// Usuário comum (caixa), sem ser gerente.
    pub fn usuario_caixa(&mut self, nome: &str, senha: &str) -> String {
        self.ok(
            "usuario_salvar",
            json!({ "nome": nome, "senha": senha, "senha2": senha, "ativo": true, "gerente": false }),
        )["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// Estoque da peça, em texto ("9", "2,5"...).
    pub fn estoque(&mut self, peca: &str) -> String {
        self.ok("produto_abrir", json!({ "id": peca }))["estoque"].as_str().unwrap().to_string()
    }

    /// Estoque pela soma dos movimentos, em unidades inteiras (milésimos / 1000).
    pub fn estoque_pelos_movimentos(&self, peca: &str) -> i64 {
        let m: i64 = self
            .conexao()
            .query_row(
                "SELECT COALESCE(SUM(quantidade), 0) FROM estoque_movimento WHERE produto_id = ?1",
                [peca],
                |l| l.get(0),
            )
            .unwrap();
        assert_eq!(m % 1000, 0, "estoque fracionado inesperado: {m}");
        m / 1000
    }

    /// Movimentos de estoque de um tipo, para uma peça.
    pub fn movimentos(&self, peca: &str) -> Vec<(String, i64)> {
        self.conexao()
            .prepare("SELECT tipo, quantidade FROM estoque_movimento WHERE produto_id = ?1 ORDER BY criado_em, rowid")
            .unwrap()
            .query_map([peca], |l| Ok((l.get(0)?, l.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    // ---- Vendas ----

    pub fn abrir_caixa(&mut self) {
        let e = self.ok("caixa_estado", json!({}));
        if e["aberto"] != true {
            self.ok("caixa_abrir", json!({ "valor": "50" }));
        }
    }

    /// Venda simples: itens (peça, quantidade) e pagamentos (forma, valor).
    pub fn vender(&mut self, itens: &[(&str, &str)], pagamentos: &[(&str, &str)]) -> Value {
        self.vender_com(itens, pagamentos, json!({}))
    }

    pub fn vender_com(&mut self, itens: &[(&str, &str)], pagamentos: &[(&str, &str)], extra: Value) -> Value {
        let mut v = json!({
            "itens": itens.iter().map(|(p, q)| json!({ "produtoId": p, "quantidade": q })).collect::<Vec<_>>(),
            "tabela": 1,
            "pagamentos": pagamentos.iter().map(|(f, v)| json!({ "forma": f, "valor": v })).collect::<Vec<_>>(),
        });
        if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, x) in e {
                o.insert(k.clone(), x.clone());
            }
        }
        self.ok("venda_concluir", v)
    }

    /// Devolve `quantidade` do item `ordem` (0 = primeiro) de uma venda.
    pub fn devolver(&mut self, venda: &str, ordem: usize, quantidade: &str, destino: &str, defeito: bool) -> Value {
        let d = self.ok("devolucao_venda", json!({ "id": venda }));
        let item = d["itens"][ordem]["vendaItemId"].as_str().unwrap().to_string();
        self.ok(
            "devolucao_gravar",
            json!({ "vendaId": venda, "destino": destino, "motivo": "teste",
                    "itens": [{ "vendaItemId": item, "quantidade": quantidade, "defeito": defeito }] }),
        )
    }
}

/// Tabelas com os dados da loja (as da troca de dados, do registro de
/// alterações, da auditoria e das coisas só deste computador ficam de fora).
pub const TABELAS_DO_NEGOCIO: &[&str] = &[
    "caixa_movimento",
    "caixa_sessao",
    "categoria",
    "cheque",
    "comissao_fechamento",
    "compra",
    "compra_item",
    "conta_loja",
    "conta_pagar",
    "conta_receber",
    "devolucao",
    "devolucao_item",
    "encomenda",
    "estoque_movimento",
    "meta",
    "movimento_conta",
    "ordem_servico",
    "ordem_servico_item",
    "pagamento",
    "pessoa",
    "pessoa_veiculo",
    "produto",
    "produto_aplicacao",
    "produto_codigo_antigo",
    "produto_equivalente",
    "produto_fornecedor",
    "produto_preco",
    "recebimento",
    "recebimento_parcela",
    "regra_st",
    "tabela_preco",
    "unidade",
    "usuario",
    "vale",
    "venda",
    "venda_item",
    "venda_pagamento",
];

/// Foto de todos os dados do negócio: tabela → linhas (em texto, em ordem).
pub fn foto_do_negocio(c: &Connection) -> BTreeMap<String, Vec<String>> {
    let mut f = BTreeMap::new();
    for t in TABELAS_DO_NEGOCIO {
        let colunas: Vec<String> = c
            .prepare(&format!("PRAGMA table_info({t})"))
            .unwrap()
            .query_map([], |l| l.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        // Muda só por entrar de novo.
        let colunas: Vec<String> = colunas.into_iter().filter(|c| c != "ultimo_acesso").collect();
        let sel = colunas.iter().map(|c| format!("quote({c})")).collect::<Vec<_>>().join(" || '|' || ");
        let mut linhas: Vec<String> = c
            .prepare(&format!("SELECT {sel} FROM {t}"))
            .unwrap()
            .query_map([], |l| l.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        linhas.sort();
        f.insert(t.to_string(), linhas);
    }
    f
}

/// Diferenças entre duas fotos (para a mensagem do teste).
pub fn diferencas(a: &BTreeMap<String, Vec<String>>, b: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut d = Vec::new();
    for (t, la) in a {
        let lb = &b[t];
        if la != lb {
            let so_a: Vec<&String> = la.iter().filter(|x| !lb.contains(x)).take(3).collect();
            let so_b: Vec<&String> = lb.iter().filter(|x| !la.contains(x)).take(3).collect();
            d.push(format!("{t}: {} x {} linhas; só antes: {so_a:?}; só depois: {so_b:?}", la.len(), lb.len()));
        }
    }
    d
}

/// Resumo de um arquivo (tamanho e soma) para provar que não mudou.
pub fn marca_do_arquivo(p: &Path) -> (u64, String) {
    use sha2::{Digest, Sha256};
    let b = std::fs::read(p).unwrap();
    let h = Sha256::digest(&b);
    (b.len() as u64, h.iter().map(|x| format!("{x:02x}")).collect())
}

/// Dinheiro em texto ("1.234,56") para centavos.
pub fn centavos(t: &str) -> i64 {
    let t = t.trim().replace("R$", "").replace(['.', ' '], "");
    if t.is_empty() {
        return 0;
    }
    let neg = t.starts_with('-');
    let t = t.trim_start_matches('-');
    let (i, d) = t.split_once(',').unwrap_or((t, "0"));
    let d = format!("{d:0<2}");
    let v = i.parse::<i64>().unwrap() * 100 + d[..2].parse::<i64>().unwrap();
    if neg {
        -v
    } else {
        v
    }
}

/// O que `povoar` criou.
pub struct Povo {
    pub ze: String,
    pub fornecedor: String,
    pub filtro: String,
    pub pastilha: String,
    pub oleo: String,
    pub v1: String,
    pub v2: String,
    pub v3: String,
    pub v4: String,
    pub compra: String,
}

pub const XML_COMPRA: &str = include_str!("../nfe_exemplo.xml");

/// Desde a 0.9.17, Compras só importa com o CNPJ da loja salvo e válido:
/// grava o do destinatário da nota de exemplo, se a loja ainda não tem um.
/// Devolve se gravou (para tirar depois e a loja ficar como antes).
pub fn cnpj_da_nota_de_exemplo(l: &Loja) -> bool {
    if l.numero("SELECT COUNT(*) FROM config WHERE chave = 'loja_cnpj' AND valor <> ''") > 0 {
        return false;
    }
    l.executar(
        "INSERT INTO config (chave, valor) VALUES ('loja_cnpj', '11444777000161')
         ON CONFLICT (chave) DO UPDATE SET valor = excluded.valor",
    );
    true
}

/// Tira o CNPJ gravado por `cnpj_da_nota_de_exemplo`: a loja de teste
/// continua sem CNPJ, como era antes da 0.9.17 (as telas da navegação guiada
/// partem dela).
pub fn tirar_cnpj_da_nota_de_exemplo(l: &Loja, gravou: bool) {
    if gravou {
        l.executar("UPDATE config SET valor = '' WHERE chave = 'loja_cnpj'");
    }
}

/// Importa e lança a nota de exemplo (3 itens, peças novas), com a
/// conferência do gerente. Devolve o id da compra.
pub fn lancar_compra_de_exemplo(l: &mut Loja) -> String {
    let caminho = l.pasta.join("nota-de-teste.xml");
    std::fs::write(&caminho, XML_COMPRA).unwrap();
    let gravou = cnpj_da_nota_de_exemplo(l);
    let r = l.ok("compra_importar", json!({ "caminho": caminho.display().to_string() }));
    tirar_cnpj_da_nota_de_exemplo(l, gravou);
    let id = r["compras"][0]["id"].as_str().unwrap_or_else(|| panic!("sem compra: {r}")).to_string();
    let c = l.ok("compra_abrir", json!({ "id": id }));
    let escolhas: Vec<Value> = c["itens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| json!({ "id": it["id"], "acao": "nova", "produtoId": "", "fator": "1", "baixarPreco": false }))
        .collect();
    l.ok(
        "compra_conferir_autorizacao",
        json!({ "id": id, "situacao": "autorizada", "protocolo": "135260000000001",
                "conferidoEm": "08/10/2026 11:00", "senha": SENHA_ANA }),
    );
    l.ok("compra_lancar", json!({ "id": id, "itens": escolhas }));
    id
}

/// Id da venda devolvida por `venda_concluir`.
pub fn id(v: &Value) -> String {
    v["id"].as_str().unwrap_or_else(|| panic!("sem id: {v}")).to_string()
}

/// Uma loja com um pouco de tudo: cadastros, compra por XML, vendas em
/// dinheiro, pix, crediário, cancelada, devolução, recebimento parcial,
/// conta paga, lançamento no crediário, orçamento e meta.
pub fn povoar(l: &mut Loja) -> Povo {
    l.usuario_caixa("Bia", SENHA_BIA);
    let ze = l.pessoa("cliente", "Zé da Oficina");
    let fornecedor = l.pessoa("fornecedor", "Distribuidora Vale");
    let filtro = l.peca("Filtro de óleo", "60", "100", "10");
    let pastilha = l.peca("Pastilha de freio", "40", "90", "5");
    let oleo = l.peca("Óleo 5W30", "", "30", "20");
    let compra = lancar_compra_de_exemplo(l);
    l.abrir_caixa();
    let v1 = id(&l.vender(&[(&filtro, "2")], &[("dinheiro", "200")]));
    let v2 = id(&l.vender_com(&[(&pastilha, "1")], &[("pix", "80")], json!({ "desconto": "10" })));
    let v3 = id(&l.vender_com(&[(&filtro, "1")], &[("crediario", "100")], json!({ "clienteId": ze, "parcelas": 2 })));
    let v4 = id(&l.vender(&[(&oleo, "1")], &[("dinheiro", "30")]));
    l.ok("venda_cancelar", json!({ "id": v4, "motivo": "cliente desistiu" }));
    l.devolver(&v1, 0, "1", "dinheiro", false);
    l.ok("receber_baixar", json!({ "clienteId": ze, "contas": [], "valor": "30", "forma": "dinheiro" }));
    l.ok(
        "receber_lancar",
        json!({ "clienteId": ze, "descricao": "caderno", "valor": "50", "parcelas": 1, "primeiroVencimento": "" }),
    );
    let ids = l.ok(
        "pagar_lancar",
        json!({ "categoriaId": "cat-aluguel", "descricao": "Aluguel", "valor": "300", "parcelas": 1, "competencia": "",
                "fornecedorId": fornecedor }),
    );
    l.ok("pagar_pagar", json!({ "contaId": ids[0], "valor": "", "forma": "pix", "origem": "caixa-geral" }));
    l.ok("caixa_movimentar", json!({ "tipo": "sangria", "valor": "20", "motivo": "depósito no banco", "conta": "" }));
    l.ok(
        "movimento_lancar",
        json!({ "contaId": "caixa-geral", "tipo": "saida", "valor": "45", "data": "", "categoriaId": "cat-energia",
                "descricao": "Conta de luz" }),
    );
    l.ok("dre_meta_salvar", json!({ "periodo": "mes", "indicador": "vendas", "vendedorId": "", "valor": "1000" }));
    l.ok(
        "orcamento_salvar",
        json!({ "itens": [{ "produtoId": pastilha, "quantidade": "2" }], "tabela": 1, "validadeDias": 7 }),
    );
    Povo { ze, fornecedor, filtro, pastilha, oleo, v1, v2, v3, v4, compra }
}

/// O Rustinho faz um backup ou restauração por vez no programa todo (de
/// propósito). Os testes que usam backup esperam a vez uns dos outros.
pub static UM_BACKUP: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn vez_do_backup() -> std::sync::MutexGuard<'static, ()> {
    UM_BACKUP.lock().unwrap_or_else(|e| e.into_inner())
}

/// Valor (centavos) de uma linha da DRE pelo id ("receita_bruta", "cmv"...).
pub fn linha_dre(d: &Value, linha: &str) -> i64 {
    d["linhas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == linha)
        .unwrap_or_else(|| panic!("a DRE não tem a linha {linha}: {}", d["linhas"]))["v"]
        .as_i64()
        .unwrap()
}

pub fn dre_do_mes(l: &mut Loja) -> Value {
    l.ok("dre_apurar", json!({ "periodo": "mes", "de": "", "ate": "", "vendedorId": "" }))
}
