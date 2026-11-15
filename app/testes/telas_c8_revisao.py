#!/usr/bin/env python3
"""C8 (dev3.3): regressão dos achados de tela da revisão independente (Fable, 15/11/2026;
`revisao_independente/fable/RELATORIO_C8.md`). Os cenários vêm do roteiro do revisor (`telas_fable_c8.py`); aqui as
conferências são estritas (✔/✘), para FALHAR na versão congelada e passar depois da correção.

  A-1  caso 7 a partir da ficha do A pagar: no destino, a ficha não fica por cima; na volta, a ficha e a pergunta
       reabrem com o motivo digitado; nada estornado.
  M-4  10b: o período escolhido em Comissões volta igual.
  A-2  caso 7 com o caixa dentro do Financeiro: na volta e depois de pagar, sem "Só o gerente pode"; paga só com o
       clique.
  M-2  9c pelo clique real: a linha do caixa destacado fica à vista (o botão dela aparece sem rolar).
  M-5  9b: o Esc no Histórico aberto por "Ver no histórico" volta às Duplicatas.
  M-7  caso 7 na conferência da compra: a senha do gerente não volta preenchida da guia (pede de novo); o
       protocolo digitado fica; nada é registrado sozinho.
  B-1  caso 4: Etiquetas › "Ver duplicatas" chega com a linha do código destacada. O roteiro não enxerga o
       destaque nos textos; ele mede a foto: com o destaque, o resto da tela escurece (o véu) e a linha do código
       não; seis segundos depois, o véu sai.

    RUSTINHO_TELA=:88 python3 app/testes/telas_c8_revisao.py [pasta das fotos]
    (RUSTINHO_CASOS_REVISAO=m7,b1 roda só esses casos)
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import (AGORA, DISPOSITIVO, PEDIDO, PROGRAMA, SENHA, TELA, TITULO_7, Placar,  # noqa: E402
                                   ler, montar_loja, por_fora, xdo)


def topo(r, texto):
    """Clica no texto igual mais em cima, dentro da janela."""
    pontos = [p for p in r.onde(texto) if 60 <= p[1] < r.altura() - 10]
    if not pontos:
        return False
    x, y = min(pontos, key=lambda p: p[1])
    r.clicar_em(x, y)
    return True


def a_vista(r, texto):
    """O texto aparece dentro da janela, sem rolar."""
    return any(60 <= p[1] < r.altura() - 30 for p in r.onde(texto))


def luz(foto, x, y, largura=60, altura=8):
    """Luminosidade (0 a 255, a mediana) de uma faixa da foto em volta de (x, y). A janela do Rustinho fica no canto
    da tela virtual, então as posições dos textos são as da foto."""
    im = Image.open(foto).convert("L")
    caixa = im.crop((max(0, x - largura), max(0, y - altura), x + largura, y + altura))
    px = sorted(caixa.getdata())
    return px[len(px) // 2] + 1


def pontinhos(r):
    """Algum campo de senha com texto (os textos da tela dão os pontinhos, nunca a senha)."""
    return any(len(t) >= 4 and set(t) <= set("●•∙*") for _, _, t in r.textos())


def senha_gerente(r):
    if r.esperar(lambda: r.tem("Liberar") or r.tem(PEDIDO), 6):
        time.sleep(0.6)
        r.digitar(SENHA["Ana"])
        xdo("key", "Return")
        time.sleep(1.5)


def a1_m4(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-rev-a1-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-nav10b', 'cat-comissoes', 'Comissão de Ana · teste', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO pagamento (id, conta_pagar_id, data, valor, forma, criado_em, dispositivo)
              VALUES ('pg-nav10b', 'cp-nav10b', date('now'), 1000, 'dinheiro', {AGORA}, {DISPOSITIVO});
            INSERT INTO comissao_fechamento (id, usuario_id, de, ate, vendas, total_vendido, valor, conta_pagar_id,
                                             criado_em, dispositivo)
              SELECT 'cf-nav10b', id, date('now', 'start of month'), date('now'), 1, 10000, 1000, 'cp-nav10b',
                     {AGORA}, {DISPOSITIVO} FROM usuario WHERE nome = 'Ana';
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('pagamento', '["pg-nav10b"]', 'conta_pagar', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        estornado = lambda: ler(pasta, "SELECT estornado_em FROM pagamento WHERE id = 'pg-nav10b'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "a1")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            base = r.pilha()
            r.clicar("Ver no A pagar")
            ok("A-1: abre a ficha da conta", r.esperar(lambda: r.tem("Comissão de Ana · teste")), r.foto("-1"))
            r.clicar("Estornar")
            r.esperar(lambda: r.tem("Estornar o pagamento?"))
            time.sleep(0.5)
            r.digitar("Pago em duplicidade")
            xdo("key", "Return")
            ok("A-1: o estorno de pagamento inconsistente oferece Ver o registro",
               r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-2"))
            r.clicar("Ver o registro")
            r.esperar(lambda: r.tem(TITULO_7))
            time.sleep(1.5)
            f = r.foto("-3")
            ok("A-1: no destino (Caixas), a ficha da conta NÃO fica por cima", not r.tem("Comissão de Ana · teste"), f)
            ok("A-1: no destino, a pergunta do motivo também não", not r.tem("Estornar o pagamento?"), f)
            xdo("key", "Escape")  # desiste da guia: volta ao Financeiro
            f = r.foto("-4")
            ok("A-1: desistir volta ao Financeiro", r.esperar(lambda: r.pilha() == base, 10), f)
            ok("A-1: a ficha e a pergunta reabrem", r.esperar(lambda: r.tem("Comissão de Ana · teste")
                                                             and r.tem("Estornar o pagamento?")), f)
            ok("A-1: com o motivo digitado", r.tem("Pago em duplicidade"), f)
            ok("A-1: nada estornado", estornado() is None)
        finally:
            r.fechar()
        por_fora(pasta, "DELETE FROM sync_referencia")
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "m4")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            xdo("key", "Home")
            r.clicar("Mês passado")
            time.sleep(1.5)
            datas = lambda: sorted(t for _, _, t in r.textos() if len(t) == 10 and t[2] == "/" and t[5] == "/")  # noqa: E731
            antes = datas()
            r.rolar_ate("Ver no A pagar")
            r.clicar("Ver no A pagar")
            r.esperar(lambda: r.tem("Comissão de Ana · teste"))
            xdo("key", "Escape")
            r.esperar(lambda: r.tem_igual("Ver no A pagar"))
            time.sleep(1.5)
            depois = datas()
            f = r.foto("-1")
            ok("M-4: o período escolhido em Comissões volta igual", bool(antes) and antes == depois,
               f"{f} antes={antes} depois={depois}")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def a2(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-rev-a2-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-f9', 'cat-aluguel', 'Conta teste revisão', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_pagar', '["cp-f9"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        pagos = lambda: ler(pasta, "SELECT COUNT(*) FROM pagamento WHERE conta_pagar_id = 'cp-f9'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "pagar"}, fotos, "a2-caixa")
        try:
            r.entrar("Bia")
            senha_gerente(r)
            if not ok("A-2: o caixa, com a senha, vê a conta", r.esperar(lambda: r.tem("Conta teste revisão")), r.foto("-0")):
                return
            base = r.pilha()
            r.clicar("Pagar")
            time.sleep(1.5)
            xdo("key", "Return")
            time.sleep(1)
            if not (r.tem("Liberar") or r.tem(PEDIDO) or r.tem_igual("Ver o registro")):
                r.clicar_ultimo("Pagar")
            senha_gerente(r)
            if not ok("A-2: pagar a conta inconsistente oferece Ver o registro",
                      r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1")):
                return
            r.clicar("Ver o registro")
            senha_gerente(r)
            r.esperar(lambda: r.tem(TITULO_7))
            r.rolar_ate("Aceitar como está")
            topo(r, "Aceitar como está")
            r.esperar(lambda: r.tem("Aceitar como está?"))
            time.sleep(0.5)
            r.digitar("Caixa de origem perdido")
            xdo("key", "Return")
            senha_gerente(r)
            f = r.foto("-2")
            ok("A-2: volta ao Financeiro", r.esperar(lambda: r.pilha() == base, 12), f)
            time.sleep(2)
            f = r.foto("-3")
            ok("A-2: na volta, o A pagar continua mostrando a conta", r.tem("Conta teste revisão"), f)
            ok("A-2: na volta, sem \"Só o gerente pode\"", not r.tem("Só o gerente"), f)
            ok("A-2: nada pago sozinho", pagos() == 0)
            xdo("key", "Return")
            time.sleep(1)
            if not (r.tem("Liberar") or r.tem(PEDIDO)) and pagos() == 0:
                r.clicar_ultimo("Pagar")
            senha_gerente(r)
            f = r.foto("-4")
            ok("A-2: com o clique, paga", r.esperar(lambda: pagos() == 1, 8), f)
            time.sleep(1.5)
            f = r.foto("-5")
            ok("A-2: depois de pagar, sem \"Só o gerente pode\"", not r.tem("Só o gerente"), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def m2_m5(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-rev-m2-")
    try:
        montar_loja(pasta)
        por_fora(pasta, "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando) "
                        "SELECT 'produto', json_array(id), 'v1', 'v2', 'caixa-2', datetime('now') FROM produto "
                        "ORDER BY criado_em LIMIT 1; "
                        "INSERT INTO sync_par (dispositivo, nome, endereco) VALUES ('caixa-2', 'Caixa 2', '10.0.0.2:47810');")
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "m2")
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            if not r.tem_igual("Ver caixas"):
                r.clicar("Atenção: Caixa 2", comeca=True)
            achou = r.esperar(lambda: r.tem_igual("Ver caixas"))
            time.sleep(1.2)
            if ok("M-2: a notificação do caixa oferece Ver caixas", achou, r.foto("-0")):
                r.clicar("Ver caixas")
                r.esperar(lambda: r.tem("Versão dos caixas") and r.pilha() == inicio + 1)
                time.sleep(2.5)  # o destaque espera a parte assentar
                f = r.foto("-1")
                ok("M-2: a linha do caixa destacado fica à vista (o botão dela, sem rolar)", a_vista(r, "Retirar da loja"), f)
                xdo("key", "Escape")
                r.esperar(lambda: r.pilha() == inicio)
        finally:
            r.fechar()
        r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "duplicatas"}, fotos, "m5")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.rolar_ate("Ver no histórico"))
            base = r.pilha()
            r.clicar("Ver no histórico")
            r.esperar(lambda: r.tem("Só o que aconteceu com este registro"))
            xdo("key", "Escape")
            f = r.foto("-1")
            ok("M-5: o Esc no Histórico (vindo de Ver no histórico) volta às Duplicatas, na mesma tela",
               r.esperar(lambda: r.tem_igual("Ver no histórico") and r.pilha() == base), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def m7(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-rev-m7-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO compra (id, fornecedor_id, numero, serie, emissao, total, situacao, autorizacao, xml, criado_em,
                                dispositivo)
              SELECT 'cp-m7', fornecedor_id, '777', '1', emissao, 1000, 'rascunho', 'aguardando',
                     CASE WHEN xml <> '' THEN xml ELSE '<NFe/>' END, {AGORA}, {DISPOSITIVO}
                FROM compra ORDER BY rowid LIMIT 1;
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('compra', '["cp-m7"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        situacao = lambda: tuple(ler(pasta, "SELECT autorizacao, autorizacao_dados FROM compra WHERE id = 'cp-m7'")[0])  # noqa: E731
        antes = situacao()
        r = placar.abrir(pasta, "TelaCompra", {"compraId": "cp-m7"}, fotos, "m7")
        try:
            r.entrar("Ana")
            if not ok("M-7: a compra em rascunho oferece Registrar conferência…",
                      r.esperar(lambda: r.tem_igual("Registrar conferência…")), r.foto("-0")):
                return
            base = r.pilha()
            r.clicar("Registrar conferência…")
            r.esperar(lambda: r.tem("Conferência no portal da NF-e"))
            time.sleep(0.6)
            r.digitar("135260000012345")  # o protocolo
            xdo("key", "Return")           # a data e hora já vêm preenchidas
            time.sleep(0.4)
            xdo("key", "Return")
            time.sleep(0.4)
            r.digitar(SENHA["Ana"])
            f = r.foto("-1")
            ok("M-7: a senha digitada aparece como pontinhos (o roteiro a enxerga)", pontinhos(r), f)
            xdo("key", "Return")
            ok("M-7: o registro inconsistente oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")),
               r.foto("-2"))
            r.clicar("Ver o registro")
            ok("M-7: a guia leva a Caixas da loja", r.esperar(lambda: r.tem(TITULO_7)), r.foto("-3"))
            xdo("key", "Escape")  # desiste da guia: volta à compra
            f = r.foto("-4")
            ok("M-7: desistir volta à compra, com a conferência reaberta",
               r.esperar(lambda: r.pilha() == base and r.tem("Conferência no portal da NF-e"), 10), f)
            time.sleep(0.8)
            f = r.foto("-5")
            ok("M-7: o protocolo digitado continua lá", r.tem("135260000012345"), f)
            ok("M-7: a senha do gerente NÃO volta preenchida (pede de novo)", not pontinhos(r), f)
            ok("M-7: nada registrado sozinho", situacao() == antes)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def b1(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-rev-b1-")
    try:
        montar_loja(pasta)
        compra = ler(pasta, "SELECT id FROM compra ORDER BY rowid LIMIT 1")[0][0]
        travada_id = ler(pasta, "SELECT i.produto_id FROM compra_item i WHERE i.compra_id = ? ORDER BY i.ordem LIMIT 1",
                         compra)[0][0]
        codigo = ler(pasta, "SELECT codigo FROM produto WHERE nome = 'Filtro de óleo'")[0][0]
        por_fora(pasta, f"UPDATE produto SET codigo = '{codigo}' WHERE id = '{travada_id}'")
        r = placar.abrir(pasta, "TelaEtiquetas", {"compraId": compra}, fotos, "b1")
        try:
            r.entrar("Ana")
            if not ok("B-1: as Etiquetas oferecem Ver duplicatas", r.esperar(lambda: r.tem_igual("Ver duplicatas")),
                      r.foto("-0")):
                return
            r.clicar("Ver duplicatas")
            r.esperar(lambda: r.tem("Códigos das peças") and r.tem_igual("Dar código novo"))
            # O destaque começa depois de a lista assentar (até uns 3 s com a máquina carregada) e dura 6 s: várias
            # fotos ao longo de 5 s, e a de referência (sem véu) depois que ele acabou.
            linha = []
            candidatas = []
            for k in range(10):
                time.sleep(0.5)
                if not linha:
                    linha = [(x, y) for x, y, t in r.textos() if nav.simples(t) == nav.simples(f"Código {codigo}")
                             and 60 <= y < r.altura() - 10]
                candidatas.append(r.foto(f"-1-{k}"))
            time.sleep(9)
            sem = r.foto("-2")
            if not ok("B-1: o código ambíguo aparece em Duplicatas", bool(linha), sem):
                return
            x, y = linha[0]
            fora = [(720, 30), (x, y - 200 if y - 200 > 70 else y + 300)]
            medidas = []
            for com in candidatas:
                dentro = luz(com, x, y) / luz(sem, x, y)
                escuro = [round(luz(com, *p) / luz(sem, *p), 2) for p in fora]
                medidas.append((com, round(dentro, 2), escuro))
            achou = [m for m in medidas if all(e < 0.85 for e in m[2]) and m[1] > 0.9]
            ok("B-1: com o destaque, o resto da tela escurece (o véu) e a linha do código não (é ela a destacada)",
               bool(achou), f"{achou[0][0] if achou else sem} medidas={[(m[1], m[2]) for m in medidas]}")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


CASOS = {"a1_m4": a1_m4, "a2": a2, "m2_m5": m2_m5, "m7": m7, "b1": b1}


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    aceitas = [":88", ":89", ":91", ":92", ":93"] + [f":{n}" for n in range(60, 75)]
    if TELA not in aceitas:
        sys.exit(f"use RUSTINHO_TELA=:88, :89, :91 a :93 ou :60 a :74 (pedida: {TELA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-c8rev-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        escolhidos = [c for c in os.environ.get("RUSTINHO_CASOS_REVISAO", "").split(",") if c]
        for c, f in CASOS.items():
            if escolhidos and c not in escolhidos:
                continue
            print(f"--- {c}", flush=True)
            try:
                f(placar, fotos)
            except Exception as e:
                placar.ok(f"{c} chegou ao fim ({e})", False)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln]
        placar.ok("nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("TUDO CERTO" if placar.errados == 0 else "HOUVE FALHAS")
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    nav.TELA = TELA
    main()
