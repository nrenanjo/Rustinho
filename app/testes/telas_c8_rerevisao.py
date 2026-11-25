#!/usr/bin/env python3
"""Roteiro da re-revisão independente (Fable) da C8 na dev3.3 (15/11/2026), trazido para o projeto como regressão dos
achados novos (N-1, N-2 e N-8 do `fable/RELATORIO_REREVISAO_C8.md`). Origem: `rerevisao_fable_c8/saida/
telas_fable_rerev.py`, sem mudança nos casos; só o nome do arquivo e este cabeçalho. Cada passo anota o que viu (OBS)
e marca ✔/✘.

  r1  depois de uma volta do 10b (a ficha fecha e a seção troca), o caso 7 ainda leva à guia? E o 10b de novo?
  r2  A-1 completo: aceitar o registro, voltar com a ficha e a pergunta, concluir com o clique, fechar a ficha.
  r3  sessão encerrada com uma janelinha aberta (o fim do 7i do autor) e, depois, o caso 7 com a pessoa seguinte.
  r4  M-1 na tela: Comissões › Desfazer com a conta inconsistente oferece o caso 7; nada é desfeito sozinho.
  r5  A-2, segurança: depois da volta da guia e de sair do Financeiro, Configurações pede a senha de novo ao caixa.
  r6  M-2, a espera do destaque: a tela sai antes do fim; o alvo não existe; a lista está vazia.

    python3 app/testes/telas_c8_rerevisao.py [pasta das fotos] [--casos r1,r3]
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import (Placar, montar_loja, por_fora, ler, xdo, TITULO_7, PEDIDO, SENHA,  # noqa: E402
                                   AGORA, DISPOSITIVO, TELA, PROGRAMA)


def obs(*a):
    print("OBS", *a, flush=True)


def topo(r, texto):
    pontos = [p for p in r.onde(texto) if 60 <= p[1] < r.altura() - 10]
    if not pontos:
        return False
    x, y = min(pontos, key=lambda p: p[1])
    r.clicar_em(x, y)
    return True


def senha_gerente(r, passo=""):
    if r.esperar(lambda: r.tem("Liberar") or r.tem(PEDIDO), 6):
        time.sleep(0.6)
        r.digitar(SENHA["Ana"])
        xdo("key", "Return")
        time.sleep(1.5)
        obs("senha do gerente digitada em:", passo)
        return True
    obs("sem pedido de senha em:", passo)
    return False


def aceitar(r, senha=False):
    r.rolar_ate("Aceitar como está")
    # Espera o fim da rolagem automática do destaque da guia antes de clicar no botão.
    anterior = None
    for _ in range(15):
        pts = [p for p in r.onde("Aceitar como está") if 60 <= p[1] < r.altura() - 10]
        atual = min(pts, key=lambda p: p[1]) if pts else None
        if atual is not None and atual == anterior:
            break
        anterior = atual
        time.sleep(0.2)
    topo(r, "Aceitar como está")
    r.esperar(lambda: r.tem("Aceitar como está?"))
    time.sleep(0.5)
    r.digitar("Caixa de origem perdido")
    time.sleep(0.3)
    xdo("key", "Return")
    if senha:
        senha_gerente(r, "aceitar (gravação)")


def desativar(pasta, nome):
    """Como a troca de dados faria: o usuário fica inativo (o mesmo de telas_estab_sessao.py). O módulo troca a tela
    virtual ao ser importado; devolve a deste roteiro."""
    tela = nav.base.TELA
    import telas_estab_sessao
    nav.base.TELA = tela
    telas_estab_sessao.desativar(pasta, nome)


def erros_do_registro(r):
    with open(r.saida, errors="replace") as fh:
        return [ln.strip()[:260] for ln in fh if "TypeError" in ln or "ReferenceError" in ln]


SQL_COMISSAO_PAGA = f"""
    INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
      VALUES ('cp-nav10b', 'cat-comissoes', 'Comissão de Ana · teste', 1000, date('now'), {AGORA}, {DISPOSITIVO},
              strftime('%Y-%m', 'now'));
    INSERT INTO pagamento (id, conta_pagar_id, data, valor, forma, criado_em, dispositivo)
      VALUES ('pg-nav10b', 'cp-nav10b', date('now'), 1000, 'dinheiro', {AGORA}, {DISPOSITIVO});
    INSERT INTO comissao_fechamento (id, usuario_id, de, ate, vendas, total_vendido, valor, conta_pagar_id,
                                     criado_em, dispositivo)
      SELECT 'cf-nav10b', id, date('now', 'start of month'), date('now'), 1, 10000, 1000, 'cp-nav10b',
             {AGORA}, {DISPOSITIVO} FROM usuario WHERE nome = 'Ana';"""


def r1_10b_depois_caso7(placar, fotos):
    """Uma volta do 10b e, em seguida, o caso 7 no A pagar (gerente). O registro das janelinhas guarda resto?"""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-r1-")
    try:
        montar_loja(pasta)
        por_fora(pasta, SQL_COMISSAO_PAGA + f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-f9', 'cat-aluguel', 'Conta teste Fable', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_pagar', '["cp-f9"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        pagos = lambda: ler(pasta, "SELECT COUNT(*) FROM pagamento WHERE conta_pagar_id = 'cp-f9'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "r1")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            base = r.pilha()
            r.clicar("Ver no A pagar")
            ok("r1: o 10b abre a ficha da conta", r.esperar(lambda: r.tem("Comissão de Ana · teste")), r.foto("-0"))
            xdo("key", "Escape")
            ok("r1: fechar a ficha volta a Comissões (1ª vez)", r.esperar(lambda: r.tem_igual("Mês passado")), r.foto("-1"))
            time.sleep(1.5)
            r.clicar("A pagar")
            if not ok("r1: o A pagar mostra a conta inconsistente", r.esperar(lambda: r.tem("Conta teste Fable")), r.foto("-2")):
                obs("r1 textos:", [t for _, _, t in r.textos()][:60])
                return
            r.clicar("Pagar")
            time.sleep(1.5)
            xdo("key", "Return")
            time.sleep(1)
            if not r.tem_igual("Ver o registro"):
                r.clicar_ultimo("Pagar")
            f = r.foto("-3")
            if not ok("r1: pagar a conta inconsistente oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), f):
                obs("r1 textos:", [t for _, _, t in r.textos()][:60])
                return
            r.clicar("Ver o registro")
            chegou = r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10)
            f = r.foto("-4")
            ok("r1: depois de uma volta do 10b, Ver o registro leva a Caixas com a guia", chegou, f)
            obs("r1 depois de Ver o registro: pilha", r.pilha(), "base", base, "guia:", r.tem(TITULO_7),
                "oferta à vista:", r.tem_igual("Ver o registro"), "conta à vista:", r.tem("Conta teste Fable"),
                "erros no registro:", erros_do_registro(r)[:3])
            if chegou:
                xdo("key", "Escape")
                ok("r1: desistir volta ao Financeiro", r.esperar(lambda: r.pilha() == base, 10), r.foto("-5"))
                time.sleep(1.5)
            ok("r1: nada pago sozinho", pagos() == 0)
            # A janelinha do pagamento (se voltou) sai; o 10b de novo.
            for _ in range(3):
                if len(r.setas()) <= 1:
                    break
                xdo("key", "Escape")
                time.sleep(1)
                if r.pilha() != base:
                    break
            obs("r1 antes do 2º 10b: pilha", r.pilha(), "setas:", len(r.setas()))
            if r.pilha() != base:
                ok("r1: a tela do Financeiro continua aberta para o 2º 10b", False, r.foto("-6"))
                return
            r.clicar("Comissões")
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            r.clicar("Ver no A pagar")
            r.esperar(lambda: r.tem("Comissão de Ana · teste"))
            time.sleep(0.8)
            xdo("key", "Escape")
            f = r.foto("-7")
            ok("r1: depois do caso 7, fechar a ficha ainda volta a Comissões (2ª vez)",
               r.esperar(lambda: r.tem_igual("Mês passado"), 6), f)
            ok("r1: nenhum TypeError/ReferenceError no registro", not erros_do_registro(r), "; ".join(erros_do_registro(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def guia_assentada(textos):
    """O programa diz que a guia destacou o registro e que nada está se movendo (rolagem ou destaque deslizando)."""
    tx = {t for _, _, t in textos}
    return "TESTE_DESTAQUE_ATIVO:1" in tx and "TESTE_MOVIMENTO_ATIVO:0" in tx


def r2_a1_completo(placar, fotos):
    """A-1 até o fim: aceitar o registro; na volta, a ficha e a pergunta; o clique conclui; a ficha fecha para Comissões."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-r2-")
    try:
        montar_loja(pasta)
        por_fora(pasta, SQL_COMISSAO_PAGA + """
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('pagamento', '["pg-nav10b"]', 'conta_pagar', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        estornado = lambda: ler(pasta, "SELECT estornado_em FROM pagamento WHERE id = 'pg-nav10b'")[0][0]  # noqa: E731
        estado = lambda: ler(pasta, "SELECT estado FROM sync_referencia WHERE chave = '[\"pg-nav10b\"]'")  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "r2")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            xdo("key", "Home")
            r.clicar("Mês passado")
            time.sleep(1.5)
            datas = lambda: sorted(t for _, _, t in r.textos() if len(t) == 10 and t[2] == "/" and t[5] == "/")  # noqa: E731
            periodo = datas()
            r.rolar_ate("Ver no A pagar")
            base = r.pilha()
            r.clicar("Ver no A pagar")
            ok("r2: abre a ficha da conta", r.esperar(lambda: r.tem("Comissão de Ana · teste")), r.foto("-0"))
            r.clicar("Estornar")
            r.esperar(lambda: r.tem("Estornar o pagamento?"))
            time.sleep(0.5)
            r.digitar("Pago em duplicidade")
            xdo("key", "Return")
            if not ok("r2: oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1")):
                return
            r.clicar("Ver o registro")
            ok("r2: a guia aparece em Caixas", r.esperar(lambda: r.tem(TITULO_7)), r.foto("-2"))
            time.sleep(1.2)
            f = r.foto("-3")
            ok("r2: no destino, nem a ficha nem a pergunta por cima",
               not r.tem("Comissão de Ana · teste") and not r.tem("Estornar o pagamento?"), f)
            # Estabilização de 25/11/2026 (JNAP, CI 45574350469, duas tentativas): a guia destaca e rola até o registro
            # quando a parte assenta. Na CI (desenho por software) isso passou dos 2,7 s fixos de antes: o botão
            # apareceu em 703, a parte terminou de montar e o empurrou para 813, e só depois a guia rolou e destacou.
            # Agora a medida espera, com limite, DUAS leituras seguidas com a guia parada (destaque ativo e nada se
            # movendo) e o botão à vista. Quem separa o certo do errado é a visibilidade: o sinal do destaque sozinho
            # não prova que a guia chegou ao registro (com a guia sem rolar, `evid_006/ESTAB_R2/mutado`, ele fica em 1
            # por outro destaque, com o botão em 813; na CI, a leitura em 481 veio com o destaque ainda em 0). B5 da
            # revisão E1. Se a guia nunca levar o botão para a área visível, reprova como antes.
            # B1 da revisão E1: confirmado o assentamento, mais uma leitura ~1 s depois (o botão continua à vista com
            # a guia parada): pega a parte que termina de montar e empurra o botão para fora depois das duas leituras.
            # B4: limite de 30 s (a rodada verde volta no assentamento; só a que reprova demora).
            chegou = time.time()
            leituras = []

            def a_vista_e_parada():
                tx = r.textos()
                pos = [(x, y) for x, y, t in tx if t == "Aceitar como está"]
                leituras.append(pos)
                bom = guia_assentada(tx) and any(60 <= y < r.altura() - 30 for _, y in pos)
                leituras[-1] = (pos, bom)
                return bom and len(leituras) >= 2 and leituras[-2][1]

            assentou = r.esperar(a_vista_e_parada, 30)
            print(f"MEDIDA r2: botão à vista com a guia parada {time.time() - chegou:.1f} s depois da chegada "
                  f"(assentou={assentou}, {len(leituras)} leituras)", flush=True)
            if assentou:
                time.sleep(1.0)
                assentou = a_vista_e_parada()
                print(f"MEDIDA r2: ~1 s depois, ainda à vista com a guia parada: {assentou}", flush=True)
            f = r.foto("-3b")
            ok("r2: na chegada (gerente), o botão Aceitar como está do registro destacado fica à vista, sem rolar",
               assentou, f"{f} últimas leituras (posições, à vista com a guia parada)={leituras[-3:]} "
                         f"altura da janela={r.altura()}")
            aceitar(r)
            f = r.foto("-4")
            ok("r2: aceito, volta ao Financeiro com a ficha e a pergunta",
               r.esperar(lambda: r.pilha() == base and r.tem("Estornar o pagamento?") and r.tem("Comissão de Ana · teste"), 12), f)
            ok("r2: aceito pelo gerente", estado() == [("aceito",)])
            ok("r2: com o motivo digitado", r.tem("Pago em duplicidade"), f)
            ok("r2: nada estornado sozinho", estornado() is None)
            time.sleep(1.2)
            xdo("key", "Return")
            if not r.esperar(lambda: estornado() is not None, 5):
                obs("r2: Enter não estornou; clicando no último Estornar")
                r.clicar_ultimo("Estornar")
            f = r.foto("-5")
            ok("r2: com o clique na pergunta reaberta, o estorno acontece (a pessoa conclui)",
               r.esperar(lambda: estornado() is not None, 6), f)
            time.sleep(1.5)
            f = r.foto("-6")
            ok("r2: a pergunta fechou e a ficha continua aberta", not r.tem("Estornar o pagamento?") and r.tem("Comissão de Ana · teste"), f)
            obs("r2 ficha depois do estorno:", [t for _, _, t in r.textos() if "ESTORNADO" in t or "aberto" in t.lower()][:6])
            xdo("key", "Escape")
            f = r.foto("-7")
            ok("r2: fechar a ficha volta a Comissões (a volta do 10b sobreviveu à guia)",
               r.esperar(lambda: r.tem_igual("Mês passado") and not r.tem("Comissão de Ana · teste"), 6), f)
            time.sleep(1.5)
            depois = datas()
            ok("r2: o período escolhido em Comissões volta igual (M-4, pelo caminho da guia)",
               bool(periodo) and periodo == depois, f"antes={periodo} depois={depois}")
            ok("r2: nenhum TypeError/ReferenceError no registro", not erros_do_registro(r), "; ".join(erros_do_registro(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def r3_sessao_encerrada_com_janelinha(placar, fotos):
    """O fim do 7i do autor (a sessão acaba com a janelinha do recebimento aberta) e, depois, o caso 7 com a pessoa
    seguinte: a guia ainda começa?"""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-r3-")
    try:
        montar_loja(pasta)
        por_fora(pasta, """
            INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, aberto_por, valor_inicial)
              SELECT 'sessao-r3', valor, 'Caixa 1', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT id FROM usuario WHERE nome = 'Ana'), 0
                FROM config WHERE chave = 'dispositivo_id';
            INSERT INTO conta_receber (id, cliente_id, descricao, valor, vencimento, criado_em, dispositivo)
              SELECT 'cr-r3', id, 'Caderno', 5000, date('now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT valor FROM config WHERE chave = 'dispositivo_id')
                FROM pessoa WHERE nome = 'Zé da Oficina';
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_receber', '["cr-r3"]', 'pessoa', '["sumiu"]', 'inconsistente',
                      '2026-01-01T00:00:00Z', 'log');""")
        ze = ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
        ja = ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0]
        r = placar.abrir(pasta, "TelaReceber", {"clienteInicial": ze}, fotos, "r3")
        try:
            r.entrar("Bia")
            r.esperar(lambda: r.tem("Receber tudo"))
            r.clicar("Receber tudo")
            ok("r3: a janelinha do recebimento abre", r.esperar(lambda: r.tem("Receber de")), r.foto("-0"))
            desativar(pasta, "Bia")
            time.sleep(0.6)
            xdo("key", "Return")  # tenta concluir: a sessão já acabou, com a janelinha aberta
            f = r.foto("-1")
            if not ok("r3: a sessão encerrada volta à tela de entrada", r.esperar(lambda: r.tem("Quem está usando?"), 15), f):
                obs("r3 textos:", [t for _, _, t in r.textos()][:40])
                return
            ok("r3: nada foi recebido", ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0] == ja)
            r.entrar("Ana")
            if not ok("r3: a pessoa seguinte (Ana) entra e chega ao Receber", r.esperar(lambda: r.tem("Receber tudo"), 12), r.foto("-2")):
                obs("r3 textos:", [t for _, _, t in r.textos()][:40])
                return
            base = r.pilha()
            r.clicar("Receber tudo")
            r.esperar(lambda: r.tem("Receber de"))
            time.sleep(0.6)
            xdo("key", "Return")
            if not ok("r3: a recusa oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-3")):
                obs("r3 textos:", [t for _, _, t in r.textos()][:40])
                return
            r.clicar("Ver o registro")
            chegou = r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10)
            f = r.foto("-4")
            ok("r3: depois de uma sessão encerrada com janelinha aberta, Ver o registro leva a Caixas com a guia", chegou, f)
            obs("r3 depois de Ver o registro: pilha", r.pilha(), "base", base, "guia:", r.tem(TITULO_7),
                "janelinha do recebimento à vista:", r.tem("Receber de"), "erros no registro:", erros_do_registro(r)[:3])
            ok("r3: nada foi recebido sozinho", ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0] == ja)
            ok("r3: nenhum TypeError/ReferenceError no registro", not erros_do_registro(r), "; ".join(erros_do_registro(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def r4_m1_na_tela(placar, fotos):
    """M-1 na tela: Comissões › Desfazer com a conta da comissão inconsistente (não paga) → o caso 7."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-r4-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-m1', 'cat-comissoes', 'Comissão de Ana · m1', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO comissao_fechamento (id, usuario_id, de, ate, vendas, total_vendido, valor, conta_pagar_id,
                                             criado_em, dispositivo)
              SELECT 'cf-m1', id, date('now', 'start of month'), date('now'), 1, 10000, 1000, 'cp-m1',
                     {AGORA}, {DISPOSITIVO} FROM usuario WHERE nome = 'Ana';
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_pagar', '["cp-m1"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        desfeito = lambda: ler(pasta, "SELECT cancelado_em FROM comissao_fechamento WHERE id = 'cf-m1'")[0][0]  # noqa: E731
        estado = lambda: ler(pasta, "SELECT estado FROM sync_referencia WHERE chave = '[\"cp-m1\"]'")  # noqa: E731

        def desfazer(r):
            r.rolar_ate("Desfazer")
            r.clicar("Desfazer")
            r.esperar(lambda: r.tem("Desfazer o fechamento?"))
            time.sleep(0.6)
            xdo("key", "Return")
            time.sleep(1.2)
            if r.tem("Desfazer o fechamento?"):
                obs("r4: Enter não confirmou; clicando no último Desfazer")
                r.clicar_ultimo("Desfazer")
                time.sleep(1.2)

        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "r4")
        try:
            r.entrar("Ana")
            if not ok("r4: o fechamento não pago oferece Desfazer", r.esperar(lambda: r.rolar_ate("Desfazer")), r.foto("-0")):
                obs("r4 textos:", [t for _, _, t in r.textos()][:60])
                return
            base = r.pilha()
            desfazer(r)
            f = r.foto("-1")
            obs("r4 depois de Desfazer:", [t for _, _, t in r.textos() if "paga" in t.lower() or "nconsist" in t or "Ver " in t][:10])
            ok("r4: NÃO diz que a comissão já foi paga", not r.tem("já foi paga"), f)
            if not ok("r4: oferece Ver o registro (caso 7)", r.esperar(lambda: r.tem_igual("Ver o registro"), 5), f):
                return
            ok("r4: nada desfeito", desfeito() is None)
            r.clicar("Ver o registro")
            ok("r4: a guia aparece em Caixas", r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1), r.foto("-2"))
            aceitar(r)
            f = r.foto("-3")
            ok("r4: aceito, volta ao Financeiro", r.esperar(lambda: r.pilha() == base, 12), f)
            ok("r4: aceito pelo gerente", estado() == [("aceito",)])
            time.sleep(2)
            ok("r4: nada desfeito sozinho na volta", desfeito() is None, r.foto("-4"))
            desfazer(r)
            f = r.foto("-5")
            ok("r4: com o clique, o fechamento é desfeito", r.esperar(lambda: desfeito() is not None, 6), f)
            ok("r4: nenhum TypeError/ReferenceError no registro", not erros_do_registro(r), "; ".join(erros_do_registro(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def r5_a2_seguranca(placar, fotos):
    """A-2, o lado da segurança: o caixa (Bia) passa pela guia a partir do Financeiro; depois de voltar e sair do
    Financeiro, a consulta de Configurações/Simultaneidade tem de ter acabado (pede a senha de novo)."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-r5-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-f9', 'cat-aluguel', 'Conta teste Fable', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo)
              VALUES ('conta_pagar', '["cp-f9"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        pagos = lambda: ler(pasta, "SELECT COUNT(*) FROM pagamento WHERE conta_pagar_id = 'cp-f9'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "pagar"}, fotos, "r5")
        try:
            r.entrar("Bia")
            senha_gerente(r, "abrir o Financeiro")
            if not ok("r5: o caixa, com a senha, vê a conta", r.esperar(lambda: r.tem("Conta teste Fable")), r.foto("-0")):
                return
            base = r.pilha()
            r.clicar("Pagar")
            time.sleep(1.5)
            xdo("key", "Return")
            time.sleep(1)
            if not (r.tem("Liberar") or r.tem(PEDIDO) or r.tem_igual("Ver o registro")):
                r.clicar_ultimo("Pagar")
            senha_gerente(r, "pagar (gravação)")
            if not ok("r5: oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1")):
                return
            r.clicar("Ver o registro")
            senha_gerente(r, "Simultaneidade (destino da guia)")
            ok("r5: a guia aparece em Caixas", r.esperar(lambda: r.tem(TITULO_7)), r.foto("-2"))
            r.esperar(lambda: any(60 <= y < r.altura() - 30 for _, y in r.onde("Aceitar como está")), 10)
            time.sleep(0.3)
            f = r.foto("-2b")
            pos = r.onde("Aceitar como está")
            ok("r5: na chegada (caixa, depois da senha), o botão Aceitar como está do registro destacado fica à vista, sem rolar",
               any(60 <= y < r.altura() - 30 for _, y in pos), f"{f} posições={pos} altura da janela={r.altura()}")
            xdo("key", "Escape")  # desiste da guia
            ok("r5: desistir volta ao Financeiro", r.esperar(lambda: r.pilha() == base, 10), r.foto("-3"))
            time.sleep(1.5)
            ok("r5: na volta, sem \"Só o gerente pode\" (a consulta do Financeiro continua)", not r.tem("Só o gerente"), r.foto("-4"))
            # 2ª ida, sem sair do Financeiro: a consulta da Simultaneidade (aberta na 1ª ida) tem de ter acabado
            # quando Configurações saiu; o destino pede a senha do gerente de novo.
            xdo("key", "Return")
            time.sleep(1)
            if not (r.tem("Liberar") or r.tem(PEDIDO) or r.tem_igual("Ver o registro")):
                r.clicar_ultimo("Pagar")
            senha_gerente(r, "pagar de novo (gravação)")
            if ok("r5: a 2ª tentativa oferece Ver o registro de novo", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-4b")):
                r.clicar("Ver o registro")
                f = r.foto("-4c")
                ok("r5: na 2ª ida, o destino pede a senha do gerente DE NOVO (a consulta da 1ª ida acabou)",
                   r.esperar(lambda: r.tem(PEDIDO) or r.tem("Liberar"), 6), f)
                ok("r5: antes da senha, nada da Simultaneidade à vista (2ª ida)", not r.tem_igual("Aceitar como está"), f)
                xdo("key", "Escape")  # desiste da senha: volta ao Financeiro
                ok("r5: desistir da senha volta ao Financeiro", r.esperar(lambda: r.pilha() == base, 10), r.foto("-4d"))
                time.sleep(1.5)
            # Fecha a janelinha do pagamento que voltou e sai do Financeiro.
            for _ in range(4):
                if r.pilha() != base:
                    break
                xdo("key", "Escape")
                time.sleep(1.2)
            f = r.foto("-5")
            ok("r5: saiu do Financeiro para o Início", r.esperar(lambda: r.pilha() == base - 1, 6), f)
            ok("r5: nada pago", pagos() == 0)
            # Configurações agora: a parte Simultaneidade tem de pedir a senha de novo.
            achou_cfg = r.clicar("Configurações")
            time.sleep(2)
            f = r.foto("-6")
            obs("r5 em Configurações: achou o cartão:", achou_cfg, "pilha", r.pilha(), "pede a senha logo:",
                r.tem(PEDIDO) or r.tem("Liberar"), "Simultaneidade à vista:", r.tem_igual("Simultaneidade"))
            if not achou_cfg or not r.tem_igual("Simultaneidade") and not (r.tem(PEDIDO) or r.tem("Liberar")):
                obs("r5: Configurações não abriu pelo Início; o passo final não foi medido")
            elif r.tem(PEDIDO) or r.tem("Liberar"):
                ok("r5: Configurações pede a senha do gerente de novo ao caixa", True, f)
            else:
                r.clicar("Simultaneidade")
                f = r.foto("-7")
                ok("r5: a Simultaneidade pede a senha do gerente de novo ao caixa (a consulta da guia acabou)",
                   r.esperar(lambda: r.tem(PEDIDO) or r.tem("Liberar"), 6), f)
                ok("r5: antes da senha, nada da Simultaneidade à vista", not r.tem_igual("Aceitar como está"), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def luz(foto, x, y, largura=60, altura=8):
    """Luminosidade mediana (1 a 256) de uma faixa da foto em volta de (x, y)."""
    from PIL import Image
    im = Image.open(foto).convert("L")
    px = sorted(im.crop((max(0, x - largura), max(0, y - altura), x + largura, y + altura)).getdata())
    return px[len(px) // 2] + 1


def r6_espera_do_destaque(placar, fotos):
    """M-2, a espera do destaque (até 25 tentativas): a tela sai antes do fim; o alvo não existe; a lista está vazia."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable-r6-")
    try:
        montar_loja(pasta)
        por_fora(pasta, "INSERT INTO sync_par (dispositivo, nome, endereco) VALUES ('caixa-2', 'Caixa 2', '10.0.0.2:47810');")
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "r6a")
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            if not r.tem_igual("Ver caixas"):
                r.clicar("Atenção: Caixa 2", comeca=True)
            achou = r.esperar(lambda: r.tem_igual("Ver caixas"))
            time.sleep(1.5)
            ref = r.foto("-ref")
            if ok("r6: a notificação do caixa oferece Ver caixas", achou, ref):
                r.clicar("Ver caixas")
                time.sleep(0.25)
                xdo("key", "Escape")  # sai antes de a espera do destaque acabar
                ok("r6: sair logo depois de abrir volta ao Início", r.esperar(lambda: r.pilha() == inicio, 8), r.foto("-a1"))
                time.sleep(4.5)  # passou o prazo da espera (25 × 120 ms)
                f = r.foto("-a2")
                pontos = [(640, 420), (200, 700), (1100, 300)]
                razoes = [round(luz(f, *p) / luz(ref, *p), 2) for p in pontos]
                ok("r6: no Início, nenhum véu de destaque sobrou (a tela saiu antes)", not all(q < 0.9 for q in razoes),
                   f"{f} razões={razoes}")
                ok("r6: nenhum TypeError/ReferenceError (saiu antes)", not erros_do_registro(r), "; ".join(erros_do_registro(r)[:2]))
        finally:
            r.fechar()
        for nome, props, texto in (("r6b", {"secao": "caixas", "alvo": "caixa:nao-existe"}, "Versão dos caixas"),
                                   ("r6c", {"secao": "duplicatas", "alvo": "conflito:999999"}, "Diferenças entre caixas")):
            r = placar.abrir(pasta, "TelaConfiguracoes", props, fotos, nome)
            try:
                r.entrar("Ana")
                abriu = r.esperar(lambda: r.tem(texto))
                time.sleep(0.5)
                antes = r.foto("-0")
                time.sleep(4.5)
                depois = r.foto("-1")
                razoes = [round(luz(depois, *p) / luz(antes, *p), 2) for p in [(640, 420), (200, 700), (1100, 300)]]
                ok(f"{nome}: alvo que não existe ({props['alvo']}): a parte abre, sem destaque e sem erro",
                   abriu and not all(q < 0.9 for q in razoes) and not erros_do_registro(r),
                   f"{depois} razões={razoes} erros={erros_do_registro(r)[:2]}")
                xdo("key", "Escape")
                time.sleep(1)
            finally:
                r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


CASOS = {"r1": r1_10b_depois_caso7, "r2": r2_a1_completo, "r3": r3_sessao_encerrada_com_janelinha,
         "r4": r4_m1_na_tela, "r5": r5_a2_seguranca, "r6": r6_espera_do_destaque}


def main():
    if not os.path.exists(PROGRAMA):
        sys.exit("compile antes")
    args = sys.argv[1:]
    escolhidos = list(CASOS)
    if "--casos" in args:
        i = args.index("--casos")
        escolhidos = args[i + 1].split(",")
        del args[i:i + 2]
    if os.environ.get("RUSTINHO_CASOS_FABLE"):
        escolhidos = os.environ["RUSTINHO_CASOS_FABLE"].split(",")
    fotos = args[0] if args else tempfile.mkdtemp(prefix="rustinho-fotos-fable-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        for c in escolhidos:
            print(f"--- {c}", flush=True)
            try:
                CASOS[c](placar, fotos)
            except Exception as e:
                placar.ok(f"{c} chegou ao fim ({e!r})", False)
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                for ln in fh:
                    if "TypeError" in ln or "ReferenceError" in ln or "Error:" in ln:
                        print("LOG", os.path.basename(log), ln.strip()[:300], flush=True)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    # R-1 da re-revisão 3 do Fable (16/11/2026): a saída diz o resultado (a CI e a roteiros_tela.sh decidem pelo
    # código de saída; antes, este roteiro saía com 0 mesmo com ✘).
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
