//! Testes da DRE gerencial. O cenário `dre_cenario_manual` é o do
//! docs/DRE-gerencial.md §6, conferido à mão.

// Valores escritos como reais_centavos (450_00 = R$ 450,00), para ler como dinheiro.
#![allow(clippy::inconsistent_digit_grouping)]

use super::*;
use crate::banco::Banco;
use crate::devolucoes::{self, ItemEntrada as ItemDev};
use crate::vendas::{self, ItemEntrada, PagamentoEntrada, VendaEntrada};
use crate::{caixa, financeiro, pagar, pessoas, produtos, usuarios};

struct Loja {
    b: Banco,
    u: String,
}

impl Loja {
    fn nova() -> Loja {
        let b = Banco::em_memoria().unwrap();
        let u = usuarios::criar(&b.conn, &b.dispositivo, "Maria", "1234", true).unwrap().id;
        caixa::abrir(&b.conn, &b.dispositivo, &u, "Caixa 1", "0").unwrap();
        Loja { b, u }
    }
    fn c(&self) -> &Connection {
        &self.b.conn
    }
    fn d(&self) -> &str {
        &self.b.dispositivo
    }
    fn peca(&self, nome: &str, custo: &str) -> String {
        let mut f = produtos::ficha_nova(self.c()).unwrap();
        f.nome = nome.into();
        f.custo = custo.into();
        f.estoque_inicial = "100".into();
        // Sem custo, sem preço de tabela: o preço é digitado na venda.
        f.preco_aberto = custo == "0";
        produtos::salvar(self.c(), self.d(), &self.u, &f).unwrap()
    }
    fn cliente(&self, nome: &str) -> String {
        let mut f = pessoas::ficha_nova(self.c(), "cliente").unwrap();
        f.nome = nome.into();
        pessoas::salvar(self.c(), self.d(), &self.u, &f).unwrap()
    }
    fn vender(&self, itens: &[(&str, &str, &str)], desconto: &str, forma: &str, cliente: &str) -> String {
        self.vender_por(&self.u.clone(), itens, desconto, forma, cliente)
    }
    fn vender_por(&self, u: &str, itens: &[(&str, &str, &str)], desconto: &str, forma: &str, cliente: &str) -> String {
        let e = VendaEntrada {
            itens: itens
                .iter()
                .map(|(p, q, preco)| ItemEntrada {
                    produto_id: (*p).into(),
                    quantidade: (*q).into(),
                    preco: (*preco).into(),
                    ..Default::default()
                })
                .collect(),
            tabela: 1,
            desconto: desconto.into(),
            cliente_id: cliente.into(),
            pagamentos: vec![PagamentoEntrada { forma: forma.into(), valor: String::new(), ..Default::default() }],
            ..Default::default()
        };
        let calc = vendas::calcular(self.c(), &e).unwrap();
        let e = VendaEntrada {
            pagamentos: vec![PagamentoEntrada { forma: forma.into(), valor: calc.total.clone(), ..Default::default() }],
            ..e
        };
        vendas::concluir(self.c(), self.d(), u, &e).unwrap()
    }
    fn conta(&self, categoria: &str, descricao: &str, valor: &str, competencia: &str) -> String {
        pagar::lancar(
            self.c(),
            self.d(),
            &self.u,
            &pagar::LancamentoEntrada {
                categoria_id: categoria.into(),
                descricao: descricao.into(),
                valor: valor.into(),
                parcelas: 1,
                competencia: competencia.into(),
                ..Default::default()
            },
        )
        .unwrap()
        .remove(0)
    }
    fn mes(&self) -> String {
        let h = fiscal::hoje(self.c()).unwrap();
        format!("{}/{}", &h[5..7], &h[..4])
    }
    fn dre(&self, periodo: &str) -> Dre {
        apurar_dre(self.c(), &Filtro { periodo: periodo.into(), ..Default::default() }).unwrap()
    }
    /// Põe a venda num dia passado (a data local vira `dias` atrás).
    fn mudar_data(&self, tabela: &str, coluna: &str, id: &str, dias: i64) {
        self.c()
            .execute(
                &format!("UPDATE {tabela} SET {coluna} = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?2) WHERE id = ?1"),
                params![id, format!("-{dias} days")],
            )
            .unwrap();
    }
}

fn soma_detalhe(l: &Loja, linha: &str, periodo: &str) -> i64 {
    let d = detalhe(l.c(), &Filtro { periodo: periodo.into(), ..Default::default() }, linha).unwrap();
    d.itens.iter().map(|i| numeros::ler_dinheiro(&i.valor).unwrap().unwrap_or(0)).sum()
}

/// O cenário do docs/DRE-gerencial.md §6, conta feita à mão.
fn cenario() -> Loja {
    let l = Loja::nova();
    let filtro = l.peca("Filtro", "60");
    let lampada = l.peca("Lâmpada", "0"); // sem custo no cadastro
    let cli = l.cliente("João");
    // Venda 1 à vista: 2 × 100 com 20 de desconto geral.
    l.vender(&[(&filtro, "2", "100")], "20", "dinheiro", "");
    // Venda 2 a prazo: 2 × 100 no crediário.
    let v2 = l.vender(&[(&filtro, "2", "100")], "", "crediario", &cli);
    // Venda 3: peça sem custo.
    l.vender(&[(&lampada, "1", "50")], "", "pix", "");
    // Devolução de 1 filtro da venda 2 (volta ao estoque; abate o crediário).
    let item = devolucoes::da_venda(l.c(), &v2).unwrap().itens[0].venda_item_id.clone();
    devolucoes::devolver(
        l.c(),
        l.d(),
        &l.u,
        &devolucoes::Entrada {
            venda_id: v2.clone(),
            itens: vec![ItemDev { venda_item_id: item, quantidade: "1".into(), defeito: false }],
            destino: "crediario".into(),
            resto: String::new(),
            motivo: "Não serviu".into(),
        },
    )
    .unwrap();
    // Aluguel (fixa) e DAS (tributos), competência deste mês.
    let mes = l.mes();
    l.conta("cat-aluguel", "Aluguel", "1000", &mes);
    l.conta("cat-impostos", "DAS", "30", &mes);
    // Recebe o resto do crediário (100) com R$ 2,00 de juros.
    receber::receber(
        l.c(),
        l.d(),
        &l.u,
        &receber::ReceberEntrada { cliente_id: cli, juros: "2".into(), forma: "dinheiro".into(), ..Default::default() },
    )
    .unwrap();
    l
}

#[test]
fn dre_cenario_manual() {
    let l = cenario();
    let d = l.dre("mes");
    let n = &d.numeros;
    assert_eq!(n.receita_bruta, 450_00, "200 + 200 + 50");
    assert_eq!(n.descontos, 20_00);
    assert_eq!(n.devolucoes, 100_00);
    assert_eq!(n.cancelamentos, 0);
    assert_eq!(n.tributos, 30_00);
    assert_eq!(n.receita_liquida, 300_00, "450 − 20 − 100 − 30");
    assert_eq!(n.cmv, 180_00, "2×60 + 2×60 − 60 da devolução");
    assert_eq!(n.lucro_bruto, 120_00);
    assert_eq!(n.margem_bruta, Some(40_00), "120 ÷ 300");
    assert_eq!(n.fixas, 1000_00);
    assert_eq!(n.resultado_operacional, -880_00);
    assert_eq!(n.receitas_financeiras, 2_00, "só os juros; a parcela não é receita nova");
    assert_eq!(n.resultado_financeiro, 2_00);
    assert_eq!(n.resultado, -878_00);
    assert_eq!(n.despesas, 1000_00, "o DAS é dedução, não despesa");
    assert_eq!(n.receita_sem_custo, 50_00);
    // Comercial: vendas concluídas e não canceladas, já com desconto.
    assert_eq!(d.comercial.vendas_liquidas, "430,00", "180 + 200 + 50");
    assert_eq!(d.comercial.quantidade, 3);
    assert_eq!(d.comercial.ticket, "143,33");
    assert_eq!(d.comercial.itens_por_venda, "1,7", "5 peças ÷ 3 vendas (arredondado)");
    assert_eq!(d.comercial.devolucoes_valor, "100,00");
    // Incompleto: peça sem custo e classificação não confirmada.
    assert_eq!(d.estado, "incompleto");
    let codigos: Vec<&str> = d.pendencias.iter().map(|p| p.codigo.as_str()).collect();
    assert!(codigos.contains(&"sem_custo"), "{codigos:?}");
    assert!(codigos.contains(&"classificacao"), "{codigos:?}");
    assert!(!codigos.contains(&"sem_tributos"), "o DAS foi lançado");
    // Linhas na ordem e com sinal.
    let l_rl = d.linhas.iter().find(|x| x.id == "receita_liquida").unwrap();
    assert_eq!((l_rl.valor.as_str(), l_rl.subtotal), ("300,00", true));
    let l_cmv = d.linhas.iter().find(|x| x.id == "cmv").unwrap();
    assert_eq!((l_cmv.v, l_cmv.percentual.as_str()), (-180_00, "-60%"));
    assert_eq!(d.linhas.last().unwrap().id, "resultado");
    assert_eq!(d.margem_bruta, "40%");
}

/// Cada total é a soma dos lançamentos que o detalhe mostra.
#[test]
fn dre_cada_total_e_rastreavel() {
    let l = cenario();
    let d = l.dre("mes");
    for linha in d.linhas.iter().filter(|x| !x.subtotal) {
        let soma = soma_detalhe(&l, &linha.id, "mes");
        assert_eq!(soma, d.numeros.valor(&linha.id), "{}", linha.id);
        let q = detalhe(l.c(), &Filtro { periodo: "mes".into(), ..Default::default() }, &linha.id).unwrap().quantidade;
        assert_eq!(linha.detalhavel, q > 0, "{}", linha.id);
    }
    let cmv = detalhe(l.c(), &Filtro { periodo: "mes".into(), ..Default::default() }, "cmv").unwrap();
    assert_eq!(cmv.quantidade, 3, "venda 1, venda 2 e a volta da devolução");
    assert!(cmv.itens.iter().any(|i| i.descricao.contains("voltou ao estoque") && i.valor == "-60,00"));
    let sem = detalhe(l.c(), &Filtro { periodo: "mes".into(), ..Default::default() }, "sem_custo").unwrap();
    assert_eq!((sem.quantidade, sem.total.as_str()), (1, "50,00"));
    assert!(detalhe(l.c(), &Filtro::default(), "inventada").is_err());
}

/// Venda cancelada em outro mês: o mês da venda não muda; o estorno entra no
/// mês do cancelamento, com o CMV de volta.
#[test]
fn dre_cancelamento_posterior_nao_apaga_o_mes_fechado() {
    let l = Loja::nova();
    let p = l.peca("Filtro", "60");
    let v = l.vender(&[(&p, "1", "100")], "", "dinheiro", "");
    l.mudar_data("venda", "criado_em", &v, 40);
    let antes = l.dre("mes_anterior").numeros;
    vendas::cancelar(l.c(), l.d(), &l.u, &v, "cliente desistiu").unwrap();
    // A venda era de ~40 dias atrás: está no mês anterior ou no outro.
    let h = data(&fiscal::hoje(l.c()).unwrap()).unwrap();
    let dia_venda = h - Duration::days(40);
    let tipo = if primeiro_do_mes(dia_venda) == mes_anterior(h) { "mes_anterior" } else { "personalizado" };
    let f = Filtro {
        periodo: tipo.into(),
        de: br(primeiro_do_mes(dia_venda)),
        ate: br(ultimo_do_mes(dia_venda)),
        ..Default::default()
    };
    let fechado = apurar_dre(l.c(), &f).unwrap();
    assert_eq!(fechado.numeros.receita_bruta, 100_00, "a venda continua no mês dela");
    assert_eq!(fechado.numeros.cmv, 60_00);
    assert_eq!(fechado.numeros.cancelamentos, 0);
    if tipo == "mes_anterior" {
        assert_eq!(antes.receita_bruta, fechado.numeros.receita_bruta, "nada mudou no mês fechado");
    }
    let agora = l.dre("hoje").numeros;
    assert_eq!(agora.cancelamentos, 100_00, "o estorno entra hoje");
    assert_eq!(agora.cmv, -60_00, "a peça voltou ao estoque hoje");
    assert_eq!(agora.receita_liquida, -100_00);
    // Comercial: a venda cancelada não conta na quantidade nem no ticket.
    let com = comercial(l.c(), &fechado.periodo.de, &fechado.periodo.ate, None).unwrap();
    assert_eq!((com.quantidade, com.canceladas, com.canceladas_valor.as_str()), (0, 1, "100,00"));
}

/// Devolução desfeita: a dedução volta na data em que foi desfeita;
/// peça com defeito mantém o custo no CMV.
#[test]
fn dre_devolucao_desfeita_e_defeito() {
    let l = Loja::nova();
    let p = l.peca("Filtro", "60");
    let v = l.vender(&[(&p, "2", "100")], "", "dinheiro", "");
    let item = devolucoes::da_venda(l.c(), &v).unwrap().itens[0].venda_item_id.clone();
    let dev = |defeito: bool| {
        devolucoes::devolver(
            l.c(),
            l.d(),
            &l.u,
            &devolucoes::Entrada {
                venda_id: v.clone(),
                itens: vec![ItemDev { venda_item_id: item.clone(), quantidade: "1".into(), defeito }],
                destino: "dinheiro".into(),
                resto: String::new(),
                motivo: "teste".into(),
            },
        )
        .unwrap()
    };
    let d1 = dev(false);
    let n = l.dre("hoje").numeros;
    assert_eq!((n.devolucoes, n.cmv), (100_00, 60_00), "120 da venda − 60 da volta");
    devolucoes::desfazer(l.c(), l.d(), &l.u, &d1, "engano").unwrap();
    let n = l.dre("hoje").numeros;
    assert_eq!(n.devolucoes, 0, "devolução (+100) e desfeita (−100) no mesmo dia");
    assert_eq!(n.cmv, 120_00);
    let det = detalhe(l.c(), &Filtro { periodo: "hoje".into(), ..Default::default() }, "devolucoes").unwrap();
    assert_eq!(det.quantidade, 2, "as duas aparecem, nada some");
    // Com defeito: a peça não volta e o custo fica no CMV.
    dev(true);
    let d = l.dre("hoje");
    assert_eq!((d.numeros.devolucoes, d.numeros.cmv, d.numeros.perda_defeito), (100_00, 120_00, 60_00));
    assert!(d.avisos.iter().any(|a| a.contains("defeito")));
}

/// Pagar a conta não cria outra despesa; compra de mercadoria fica fora;
/// juros do pagamento vão para o financeiro.
#[test]
fn dre_pagamento_posterior_e_compra_de_estoque() {
    let l = Loja::nova();
    let mes = l.mes();
    let aluguel = l.conta("cat-aluguel", "Aluguel", "1000", &mes);
    l.conta("cat-fornecedores", "Peças da distribuidora (sem nota)", "500", &mes);
    let antes = l.dre("mes").numeros;
    assert_eq!(antes.fixas, 1000_00);
    assert_eq!(antes.fora_resultado, 500_00, "mercadoria entra pelo CMV, não aqui");
    pagar::pagar(
        l.c(),
        l.d(),
        &l.u,
        &pagar::PagamentoEntrada {
            conta_id: aluguel,
            juros: "15".into(),
            forma: "dinheiro".into(),
            origem: "caixa-geral".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let depois = l.dre("mes").numeros;
    assert_eq!(depois.fixas, 1000_00, "pagar não é despesa nova");
    assert_eq!(depois.despesas_financeiras, 15_00, "só os juros");
    assert_eq!(depois.resultado, antes.resultado - 15_00);
}

/// Contas mensais não entram em "Hoje" (vira pendência); conta sem
/// competência e categoria sem classificação viram pendências.
#[test]
fn dre_contas_mensais_competencia_e_classificacao() {
    let l = Loja::nova();
    let mes = l.mes();
    l.conta("cat-aluguel", "Aluguel", "1000", &mes);
    let hoje = l.dre("hoje");
    assert_eq!(hoje.numeros.fixas, 0);
    assert_eq!(hoje.numeros.mensais_fora, 1000_00);
    assert!(hoje.pendencias.iter().any(|p| p.codigo == "mensais_fora"));
    // Conta sem competência (pelo vencimento, que é hoje).
    l.conta("cat-energia", "Energia", "200", "");
    // Categoria nova, ainda sem classificação.
    let cat = financeiro::salvar_categoria(
        l.c(),
        &financeiro::CategoriaEntrada {
            nome: "Brindes".into(),
            tipo: "despesa".into(),
            ativo: true,
            ..Default::default()
        },
    )
    .unwrap();
    l.conta(&cat, "Canetas", "50", &mes);
    // Saída avulsa sem categoria.
    financeiro::lancar_movimento(
        l.c(),
        l.d(),
        &l.u,
        &financeiro::MovimentoEntrada {
            conta_id: "caixa-geral".into(),
            tipo: "saida".into(),
            valor: "30".into(),
            data: String::new(),
            categoria_id: String::new(),
            descricao: "Café".into(),
        },
    )
    .unwrap();
    let d = l.dre("mes");
    assert_eq!(d.numeros.fixas, 1200_00);
    assert_eq!(d.numeros.sem_classificacao, 80_00, "brindes + café: contam como despesa");
    assert_eq!(d.numeros.resultado, -1280_00);
    let p: HashMap<&str, &Pendencia> = d.pendencias.iter().map(|x| (x.codigo.as_str(), x)).collect();
    assert_eq!(p["sem_classificacao"].quantidade, 2);
    assert_eq!(p["sem_competencia"].quantidade, 1);
    assert_eq!(p["sem_competencia"].valor, "200,00");
    // Classificando: a pendência some e o valor muda de linha (com auditoria).
    salvar_classificacao(
        l.c(),
        l.d(),
        &l.u,
        &ClassificacaoEntrada {
            categorias: vec![CategoriaGrupo { id: cat.clone(), grupo: "variavel".into() }],
            confirmar: true,
        },
    )
    .unwrap();
    let d = l.dre("mes");
    assert_eq!((d.numeros.variaveis, d.numeros.sem_classificacao), (50_00, 30_00));
    assert!(!d.pendencias.iter().any(|x| x.codigo == "classificacao"));
    let trilha: String =
        l.c().query_row("SELECT detalhes FROM auditoria WHERE acao = 'dre.classificacao'", [], |r| r.get(0)).unwrap();
    assert!(trilha.contains("Brindes: Sem classificação → Despesas variáveis de venda"), "{trilha}");
    assert!(trilha.contains("confirmada"));
    assert!(salvar_classificacao(
        l.c(),
        l.d(),
        &l.u,
        &ClassificacaoEntrada {
            categorias: vec![CategoriaGrupo { id: cat, grupo: "inventado".into() }],
            confirmar: false
        }
    )
    .is_err());
}

/// Mês sem dados não é "resultado zero comprovado"; a série marca os meses.
#[test]
fn dre_mes_sem_dados_e_serie() {
    let l = Loja::nova();
    let d = l.dre("mes_anterior");
    assert_eq!(d.estado, "sem_dados");
    assert!(d.estado_texto.contains("não é um resultado zero"));
    let p = l.peca("Filtro", "60");
    l.vender(&[(&p, "1", "100")], "", "dinheiro", "");
    let s = serie(l.c()).unwrap();
    assert_eq!(s.len(), 12);
    let ultimo = s.last().unwrap();
    assert_eq!((ultimo.vendas.as_str(), ultimo.lucro_bruto.as_str()), ("100,00", "40,00"));
    assert!(s[..11].iter().all(|m| m.estado == "antes" && m.vendas.is_empty()), "antes do primeiro registro");
}

/// O custo da peça muda depois da venda: o CMV continua o da hora da venda.
#[test]
fn dre_alteracao_de_custo_nao_muda_o_passado() {
    let l = Loja::nova();
    let p = l.peca("Filtro", "60");
    l.vender(&[(&p, "1", "100")], "", "dinheiro", "");
    l.c().execute("UPDATE produto SET custo = 9000 WHERE id = ?1", [&p]).unwrap();
    assert_eq!(l.dre("mes").numeros.cmv, 60_00);
}

/// Períodos e comparações.
#[test]
fn dre_periodos() {
    let (p, c) = periodos("2026-10-20", "mes", "", "").unwrap();
    assert_eq!((p.de.as_str(), p.ate.as_str(), p.parcial), ("2026-10-01", "2026-10-20", true));
    assert_eq!((c.de.as_str(), c.ate.as_str()), ("2026-09-01", "2026-09-20"));
    let (p, c) = periodos("2026-03-31", "mes", "", "").unwrap();
    assert!(!p.parcial, "último dia do mês");
    assert_eq!(c.ate, "2026-02-28", "fevereiro não tem 31");
    let (p, c) = periodos("2026-10-20", "7dias", "", "").unwrap();
    assert_eq!((p.de.as_str(), c.de.as_str(), c.ate.as_str()), ("2026-10-14", "2026-10-07", "2026-10-13"));
    assert!(!p.mensais);
    let (p, c) = periodos("2026-01-15", "mes_anterior", "", "").unwrap();
    assert_eq!((p.de.as_str(), p.ate.as_str(), c.de.as_str()), ("2025-12-01", "2025-12-31", "2025-11-01"));
    let (p, c) = periodos("2026-10-20", "ano", "", "").unwrap();
    assert_eq!((p.de.as_str(), c.de.as_str(), c.ate.as_str()), ("2026-01-01", "2025-01-01", "2025-10-20"));
    let (p, c) = periodos("2026-10-20", "personalizado", "01/09/2026", "10/09/2026").unwrap();
    assert_eq!((p.de.as_str(), c.de.as_str(), c.ate.as_str()), ("2026-09-01", "2026-08-22", "2026-08-31"));
    assert!(periodos("2026-10-20", "personalizado", "10/09/2026", "01/09/2026").is_err());
    let (h, o) = periodos("2026-10-20", "hoje", "", "").unwrap();
    assert_eq!((h.de.as_str(), o.de.as_str()), ("2026-10-20", "2026-10-19"));
    // Contas mensais: mês inteiro ou mês corrente até hoje.
    let mes = |de: &str, ate: &str| Periodo {
        tipo: "x".into(),
        de: de.into(),
        ate: ate.into(),
        rotulo: String::new(),
        parcial: false,
        mensais: true,
    };
    assert!(mes_entra(&mes("2026-09-01", "2026-09-30"), "2026-09", "2026-10-20"));
    assert!(mes_entra(&mes("2026-10-01", "2026-10-20"), "2026-10", "2026-10-20"));
    assert!(!mes_entra(&mes("2026-09-01", "2026-09-20"), "2026-09", "2026-10-20"), "pedaço de mês passado");
    assert!(!mes_entra(&mes("2026-09-15", "2026-10-20"), "2026-09", "2026-10-20"));
    assert!(mes_entra(&mes("2026-01-01", "2026-10-20"), "2026-03", "2026-10-20"));
}

/// Rankings: desconto geral rateado; peça sem custo fora dos rankings de lucro.
#[test]
fn dre_rankings_e_vendedores() {
    let l = Loja::nova();
    let filtro = l.peca("Filtro", "60");
    let vela = l.peca("Vela", "5");
    let lamp = l.peca("Lâmpada", "0");
    // 1 filtro (100) + 10 velas (10 cada = 100), desconto geral 20 → 10 para cada.
    l.vender(&[(&filtro, "1", "100"), (&vela, "10", "10")], "20", "dinheiro", "");
    l.vender(&[(&lamp, "3", "10")], "", "pix", "");
    let ana = usuarios::criar(l.c(), l.d(), "Ana", "1234", true).unwrap().id;
    l.vender_por(&ana, &[(&filtro, "1", "100")], "", "dinheiro", "");
    let i = indicadores(l.c(), &Filtro { periodo: "mes".into(), ..Default::default() }, "", 10).unwrap();
    assert_eq!(i.por_quantidade[0].nome, "Vela");
    assert_eq!(i.por_receita[0].nome, "Filtro");
    assert_eq!(i.por_receita[0].receita, "190,00", "90 + 100");
    assert_eq!(i.por_lucro[0].nome, "Filtro");
    assert_eq!(i.por_lucro[0].lucro, "70,00", "190 − 120");
    assert_eq!(i.por_margem[0].nome, "Vela", "90 − 50 = 40 de 90 (44%) > filtro 37%");
    assert!(!i.por_lucro.iter().any(|p| p.nome == "Lâmpada"), "sem custo não entra no lucro");
    assert!(i.por_receita.iter().any(|p| p.nome == "Lâmpada" && p.sem_custo));
    assert_eq!(i.vendedores.len(), 2);
    let maria = i.vendedores.iter().find(|v| v.nome == "Maria").unwrap();
    assert_eq!((maria.vendas, maria.receita.as_str(), maria.descontos.as_str()), (2, "210,00", "20,00"));
    assert!(maria.sem_custo && maria.margem.is_empty(), "lucro incompleto: sem margem");
    let ana_v = i.vendedores.iter().find(|v| v.nome == "Ana").unwrap();
    assert_eq!((ana_v.lucro.as_str(), ana_v.margem.as_str()), ("40,00", "40%"));
    // Filtro por vendedor: só a parte das vendas.
    let d =
        apurar_dre(l.c(), &Filtro { periodo: "mes".into(), vendedor_id: ana.clone(), ..Default::default() }).unwrap();
    assert_eq!((d.numeros.receita_bruta, d.numeros.cmv, d.vendedor.as_str()), (100_00, 60_00, "Ana"));
    assert!(d.linhas.iter().all(|x| x.id != "fixas"));
    assert!(d.pendencias.iter().any(|p| p.codigo == "vendedor"));
    // Busca por peça.
    let so = indicadores(l.c(), &Filtro { periodo: "mes".into(), ..Default::default() }, "vela", 10).unwrap();
    assert_eq!(so.por_receita.len(), 1);
}

/// Rateio do desconto geral fecha no centavo.
#[test]
fn dre_rateio() {
    assert_eq!(ratear(100, &[1, 1, 1]), vec![33, 33, 34]);
    assert_eq!(ratear(0, &[5, 5]), vec![0, 0]);
    assert_eq!(ratear(10, &[0, 0]), vec![0, 0]);
    assert_eq!(ratear(2000, &[10000, 10000]).iter().sum::<i64>(), 2000);
}

/// Metas: grava, calcula o atingido, tira e registra na auditoria.
#[test]
fn dre_metas() {
    let l = Loja::nova();
    let p = l.peca("Filtro", "60");
    l.vender(&[(&p, "1", "100")], "", "dinheiro", "");
    let id = salvar_meta(
        l.c(),
        l.d(),
        &l.u,
        &MetaEntrada { periodo: "mes".into(), indicador: "vendas".into(), valor: "400".into(), ..Default::default() },
    )
    .unwrap();
    assert_eq!(id, "meta:mes:vendas:loja");
    salvar_meta(
        l.c(),
        l.d(),
        &l.u,
        &MetaEntrada { periodo: "mes".into(), indicador: "margem".into(), valor: "30".into(), ..Default::default() },
    )
    .unwrap();
    salvar_meta(
        l.c(),
        l.d(),
        &l.u,
        &MetaEntrada {
            periodo: "semana".into(),
            indicador: "quantidade".into(),
            vendedor_id: l.u.clone(),
            valor: "2".into(),
        },
    )
    .unwrap();
    let m = metas(l.c()).unwrap();
    let vendas = m.iter().find(|x| x.indicador == "vendas").unwrap();
    assert_eq!(
        (vendas.meta.as_str(), vendas.realizado.as_str(), vendas.diferenca.as_str(), vendas.percentual.as_str()),
        ("R$ 400,00", "R$ 100,00", "R$ -300,00", "25%")
    );
    assert!(!vendas.atingida);
    let margem = m.iter().find(|x| x.indicador == "margem").unwrap();
    assert_eq!((margem.realizado.as_str(), margem.atingida), ("40%", true));
    let semana = m.iter().find(|x| x.periodo == "semana").unwrap();
    assert_eq!(
        (semana.vendedor.as_str(), semana.realizado.as_str(), semana.percentual.as_str()),
        ("Maria", "1", "50%")
    );
    // Valores inválidos.
    for (ind, v) in [("margem", "101"), ("vendas", "-5"), ("inventado", "1"), ("quantidade", "1,5")] {
        assert!(
            salvar_meta(
                l.c(),
                l.d(),
                &l.u,
                &MetaEntrada { indicador: ind.into(), valor: v.into(), ..Default::default() }
            )
            .is_err(),
            "{ind} {v}"
        );
    }
    // Tirar.
    salvar_meta(
        l.c(),
        l.d(),
        &l.u,
        &MetaEntrada { indicador: "vendas".into(), valor: "".into(), ..Default::default() },
    )
    .unwrap();
    assert!(metas(l.c()).unwrap().iter().all(|x| x.indicador != "vendas"));
    let n: i64 = l.c().query_row("SELECT COUNT(*) FROM auditoria WHERE acao = 'dre.meta'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 4);
}

/// Resumo do Painel e o impresso.
#[test]
fn dre_resumo_do_painel_e_impresso() {
    let l = cenario();
    salvar_meta(
        l.c(),
        l.d(),
        &l.u,
        &MetaEntrada { indicador: "vendas".into(), valor: "1000".into(), ..Default::default() },
    )
    .unwrap();
    let r = resumo_painel(l.c()).unwrap();
    assert_eq!(r.vendas_liquidas, "430,00");
    assert_eq!((r.lucro_bruto.as_str(), r.margem_bruta.as_str()), ("120,00", "40%"));
    assert_eq!(r.despesas, "1.000,00");
    assert_eq!((r.resultado.as_str(), r.resultado_negativo, r.estado.as_str()), ("-878,00", true, "incompleto"));
    assert_eq!(r.ticket, "143,33");
    assert!(r.avisos.iter().any(|a| a.contains("sem custo")));
    assert!(r.avisos.iter().any(|a| a.contains("Meta de vendas líquidas")));
    let h = html(l.c(), &Filtro::default()).unwrap();
    assert!(h.contains("Resultado gerencial estimado") && h.contains("-878,00") && h.contains("contador"));
    assert!(!h.contains("Lucro líquido"));
}

/// A classificação de fábrica (migração 0021).
#[test]
fn dre_classificacao_de_fabrica() {
    let l = Loja::nova();
    let c = classificacao(l.c()).unwrap();
    let g = |id: &str| c.categorias.iter().find(|x| x.id == id).unwrap().grupo.clone();
    assert_eq!(g("cat-fornecedores"), "estoque");
    assert_eq!(g("cat-aluguel"), "fixa");
    assert_eq!(g("cat-impostos"), "tributos_venda");
    assert_eq!(g("cat-comissoes"), "variavel");
    assert_eq!(g("cat-tarifas"), "despesa_financeira");
    assert_eq!(g("cat-outras-entradas"), "outra_receita_operacional");
    assert!(!c.confirmada);
    assert_eq!(c.grupos.len(), GRUPOS.len());
}
