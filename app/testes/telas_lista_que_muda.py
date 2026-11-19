#!/usr/bin/env python3
"""Decisão 2c do proprietário (18/11/2026, bug YVZ4), no escopo que ele aprovou depois da revisão Sonnet (S1-R4):
ignorar novos cliques QUANDO A LISTA MUDA debaixo do ponteiro, e só isso. Gravar sem a lista mudar não bloqueia nada, e
a rolagem por gesto continua livre.

Em Estoque › Encomendas ("Em andamento": as que chegaram, as abertas e as pedidas, cada grupo da mais nova para a mais
antiga). "Já pedi" ali não mostra aviso: as telas não se movem (a N6 fica fora do caminho).
  K3  A lista NÃO muda: a única encomenda aberta é a mais nova de todas; "Já pedi" nela e, 0,2 s depois, outro clique
      no mesmo ponto. Ela vira pedida e continua no mesmo lugar (a 1ª das pedidas), agora com "Chegou": o 2º clique
      VALE e abre "A peça chegou?". (Antes do ajuste, qualquer gravação bloqueava a tela inteira por 0,4 s.)
  K1  A lista MUDA: duas abertas no alto; "Já pedi" na 1ª e, 0,2 s depois, outro clique no mesmo ponto, onde a 2ª
      subiu: o 2º clique é ignorado (a 2ª continua aberta).
  K2  A lista MUDA e, logo em seguida, um arrasto na lista (fora dos botões): a lista rola. (Antes do ajuste, o
      bloqueio pegava a tela inteira e engolia o arrasto.)
  K4  Como o K1, com a conferência atrasada (1 s), como numa máquina lenta: o 2º clique chega ANTES da conferência
      e, mesmo assim, é ignorado (o botão confere na hora do toque). Sem a conferência no toque, B fica pedida (o
      mutante de prod13/k4_evid/).
  K5  Como o K3, com a mesma conferência atrasada: a lista não muda, e o 2º clique VALE.
  K7  A lista some (Esc) enquanto a conferência atrasada ainda espera: os botões do resto do programa continuam
      respondendo, sem erro de script (S2-R1 da conferência Sonnet).
Roda no computador e, num 2º processo, no desenho do celular (360 × 780).

    RUSTINHO_TELA=:88 python3 app/testes/telas_lista_que_muda.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o de antes, para a prova)
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, ler, montar_loja, por_fora, xdo  # noqa: E402

JANELA = "celular" if nav.CELULAR else "computador"
NOME = "Encomenda K "


def sql_encomendas(abertas, pedidas):
    """As abertas mais novas (1 minuto, 2 minutos…) e as pedidas mais antigas (10 minutos em diante)."""
    linhas = []
    for i, nome in enumerate(abertas):
        linhas.append((nome, "aberta", 1 + i))
    for i in range(pedidas):
        linhas.append((f"P{i + 1:02d}", "pedida", 10 + i))
    valores = ",\n".join(
        f"('k-{n}', {2000 + k}, 'Cliente K {n}', '', '{NOME}{n}', 1000, '{s}', "
        f"strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-{m} minutes'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), "
        f"(SELECT dispositivo FROM produto LIMIT 1))"
        for k, (n, s, m) in enumerate(linhas))
    return ("INSERT INTO encomenda (id, numero, cliente_nome, telefone, descricao, quantidade, situacao, criado_em, "
            "atualizado_em, dispositivo) VALUES\n" + valores + ";")


ALTURA = [0]
# O 2º clique: dentro dos 0,4 s da regra (se a lista mudou, ele tem de ser ignorado; se não mudou, tem de valer).
# O `xdotool click` espera 100 ms DEPOIS de cada clique (o --delay padrão, mesmo sem --repeat; medido em 19/11/2026:
# "click 1" leva 105 ms). Até o ab68b2c, o "click, 0,35 s, click" dava 0,45 s entre os dois, fora dos 0,4 s, e o K1
# reprovava ou passava conforme a corrida com o fim do bloqueio (CIs 44450377052, 44451182365 e 44473141430; o
# diagnóstico em prod13/k5_evid/diag/: o 2º toque chegou ao programa 450 ms depois do 1º, e o bloqueio acabou em
# +444 a +452 ms). Agora o apertar e o soltar são mandados à parte (clique_duplo), com 0,2 s entre os dois cliques,
# e o programa diz em quantos ms depois da mudança o 2º toque chegou (a premissa do tempo, conferida no K1 e no K4).
INTERVALO = 0.2
# K4 e K5: a 1ª conferência da lista vem 1 s depois da gravação (a opção de teste --atraso-conferencia-lista), como
# numa máquina lenta em que a janela fica ocupada e o 2º clique é atendido antes da conferência. O 2º clique, 0,2 s
# depois, chega antes da conferência: o botão tem de conferir na hora.
ATRASO_LENTO = 1000


def clique_duplo(bx, by):
    """O clique e, INTERVALO s depois, outro no mesmo ponto, num comando só do xdotool (apertar e soltar à parte: o
    "click" esperaria mais 100 ms)."""
    xdo("mousemove", str(bx), str(by), "mousedown", "1", "mouseup", "1", "sleep", str(INTERVALO),
        "mousedown", "1", "mouseup", "1")


def toque_depois(t):
    """Em quantos ms depois de a mudança da lista ser vista o último toque num botão chegou ao programa (None: o
    programa não diz; -1: nenhum toque depois de uma mudança)."""
    for _, _, tx in t:
        if tx.startswith("TESTE_TOQUE_DEPOIS_DA_MUDANCA:"):
            return int(tx.split(":")[1])
    return None


def na_janela(y):
    return 60 <= y < ALTURA[0] - 30


def linhas(t):
    return sorted((y, x, tx) for x, y, tx in t if tx.startswith(NOME) and na_janela(y))


def botao_da_linha(t, nome, texto="Já pedi"):
    ls = linhas(t)
    nomes = [n for _, _, n in ls]
    if nome not in nomes:
        return None
    i = nomes.index(nome)
    y0 = ls[i][0] - 30
    y1 = ls[i + 1][0] - 30 if i + 1 < len(ls) else ls[i][0] + 120
    b = sorted((y, x) for x, y, tx in t if tx == texto and y0 <= y < y1 and na_janela(y))
    return b[0] if b else None


def situacao(pasta, nome):
    r = ler(pasta, "SELECT situacao FROM encomenda WHERE descricao = ?", NOME + nome)
    return r[0][0] if r else None


def contador(t):
    for _, _, tx in t:
        if tx.startswith("TESTE_LISTA_MUDOU:"):
            return int(tx.split(":")[1])
    return None


def achar_botao(r, nome, texto="Já pedi"):
    """(textos, (y, x)) do botão da linha, rolando um pouco se ele estiver abaixo da borda (no celular, o botão fica
    embaixo do nome da linha)."""
    for _ in range(4):
        t = r.textos()
        b = botao_da_linha(t, nome, texto)
        if b is not None and not nav.CELULAR:
            # No computador, os botões extras da linha (Receber, Cancelar encomenda…) aparecem com o ponteiro em
            # cima dela, e a linha se rearruma. Na CI 44429969283, o clique logo depois de levar o ponteiro caiu em
            # "Cancelar encomenda" no meio da rearrumação. Como uma pessoa: o ponteiro na linha, os botões
            # assentam, e só então a posição é lida de novo.
            xdo("mousemove", str(b[1]), str(b[0]))
            time.sleep(0.8)
            t = r.textos()
            b = botao_da_linha(t, nome, texto)
        if b is not None:
            return t, b
        if nav.CELULAR:
            # O arrasto começa na própria linha, perto do pé (dentro da lista; nos cartões do alto ele não pega) e
            # sobe 300 px, devagar, sem impulso.
            x0, y0 = 180, ALTURA[0] - 50
            xdo("mousemove", str(x0), str(y0), "mousedown", "1")
            for k in range(1, 11):
                xdo("mousemove", str(x0), str(y0 - 30 * k))
                time.sleep(0.03)
            time.sleep(0.15)
            xdo("mouseup", "1")
            time.sleep(0.8)
        else:
            xdo("mousemove", "600", str(ALTURA[0] // 2))
            for _ in range(2):
                xdo("click", "5")
        esperar_parada(r)
    return r.textos(), None


def esperar_parada(r):
    """Espera a lista parar (as linhas no mesmo lugar em duas leituras seguidas)."""
    antes = None
    for _ in range(10):
        agora = [(y, n) for y, _, n in linhas(r.textos())]
        if agora == antes:
            return
        antes = agora
        time.sleep(0.4)


def abrir(placar, fotos, pasta, nome, mostrar, atraso=0):
    # atraso > 0: a conferência da lista vem atrasada (opção de teste do programa), como numa máquina lenta.
    antes = os.environ.pop("RUSTINHO_ARGS_TESTE", None)
    if atraso:
        os.environ["RUSTINHO_ARGS_TESTE"] = f"--atraso-conferencia-lista={atraso}"
    try:
        r = placar.abrir(pasta, "TelaEstoque", {"secao": "encomendas"}, fotos, f"{nome}-{JANELA}")
    finally:
        os.environ.pop("RUSTINHO_ARGS_TESTE", None)
        if antes is not None:
            os.environ["RUSTINHO_ARGS_TESTE"] = antes
    r.entrar("Ana")
    ALTURA[0] = r.altura()
    if not placar.ok(f"{nome}: a lista de encomendas aparece", r.esperar(lambda: r.tem(NOME + mostrar), 15), r.foto("-0")):
        return r, False
    time.sleep(1.0)
    if not any(n == NOME + mostrar for _, _, n in linhas(r.textos())):
        r.rolar_ate(NOME + mostrar)
    esperar_parada(r)
    return r, True


def cenario_k3(placar, fotos, rot="K3", atraso=0):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix=f"rustinho-loja-{rot.lower()}-")
    try:
        montar_loja(pasta)
        por_fora(pasta, sql_encomendas(["X"], 8))
        r, aberta = abrir(placar, fotos, pasta, rot, "X", atraso)
        try:
            if not aberta:
                return
            t, b = achar_botao(r, NOME + "X")
            if not ok(f"{rot}: o \"Já pedi\" da única aberta (a mais nova) à vista", b is not None, r.foto("-1")):
                return
            n0 = contador(t)
            by, bx = b
            # O clique e, 0,2 s depois, outro no mesmo ponto.
            clique_duplo(bx, by)
            time.sleep(1.5 + atraso / 1000)
            t = r.textos()
            f = r.foto("-2")
            perguntou = r.tem("A peça chegou?")
            n1 = contador(t)
            print(f"MEDIDA {rot} ({JANELA}): X {situacao(pasta, 'X')}; o 2º clique abriu \"A peça chegou?\": {perguntou}; "
                  f"listas que mudaram {n0} → {n1}", flush=True)
            ok(f"{rot}: \"Já pedi\" vale (a encomenda fica pedida, no mesmo lugar)", situacao(pasta, "X") == "pedida", f)
            ok(f"{rot}: a lista não mudou: o 2º clique, 0,2 s depois, VALE (abre \"A peça chegou?\")", perguntou, f)
            ok(f"{rot}: o programa não contou mudança de lista (nada bloqueado)", n0 is not None and n1 == n0,
               f"{f} INSTRUMENTAÇÃO se None: {n0} → {n1}")
            xdo("key", "Escape")
            time.sleep(0.8)
            ok(f"{rot}: desistir da pergunta não muda nada (continua pedida)", situacao(pasta, "X") == "pedida",
               r.foto("-3"))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def cenario_k12(placar, fotos, rot="K1", atraso=0, com_k2=True):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix=f"rustinho-loja-{rot.lower()}-")
    try:
        montar_loja(pasta)
        por_fora(pasta, sql_encomendas(["A", "B"], 14))
        r, aberta = abrir(placar, fotos, pasta, rot, "A", atraso)
        try:
            if not aberta:
                return
            # K1 (K4 com a conferência atrasada): a lista muda debaixo do ponteiro.
            t, b = achar_botao(r, NOME + "A")
            if not ok(f"{rot}: o \"Já pedi\" da 1ª aberta à vista", b is not None, r.foto("-1")):
                return
            n0 = contador(t)
            by, bx = b
            clique_duplo(bx, by)
            time.sleep(1.5 + atraso / 1000)
            t = r.textos()
            f = r.foto("-2")
            n1 = contador(t)
            d = toque_depois(t)
            print(f"MEDIDA {rot} ({JANELA}): A {situacao(pasta, 'A')}, B {situacao(pasta, 'B')}; listas que mudaram "
                  f"{n0} → {n1}; o 2º toque chegou {d} ms depois de a mudança ser vista", flush=True)
            ok(f"{rot} (premissa do tempo): o 2º clique chegou ao programa dentro dos 0,4 s depois de a mudança ser vista "
               "(fora disso, o resultado abaixo é INCONCLUSIVO POR TEMPO)", d is not None and 0 <= d < 400,
               f"{f} {d} ms (INSTRUMENTAÇÃO se None: o programa não diz)")
            ok(f"{rot}: \"Já pedi\" vale para a clicada (A fica pedida)", situacao(pasta, "A") == "pedida", f)
            ok(f"{rot}: a lista mudou (B subiu para o lugar de A): o 2º clique, 0,2 s depois, é ignorado (B continua "
               "aberta)", situacao(pasta, "B") == "aberta", f"{f} B={situacao(pasta, 'B')}")
            ok(f"{rot}: o programa contou a mudança da lista", n0 is not None and n1 == n0 + 1,
               f"{f} INSTRUMENTAÇÃO se None: {n0} → {n1}")
            if not com_k2:
                return
            # K2: a lista muda de novo e, logo em seguida, um arrasto na lista (fora dos botões): ela rola.
            esperar_parada(r)
            time.sleep(0.6)
            t, b = achar_botao(r, NOME + "B")
            ls = linhas(t)
            ref = [v for v in ls if v[2] == NOME + "P05"] or [v for v in ls if v[2].startswith(NOME + "P")]
            if not ok("K2: o \"Já pedi\" de B e uma linha de referência à vista", b is not None and ref, r.foto("-3")):
                return
            yref, xref, nref = ref[0]
            by, bx = b
            # O arrasto começa no nome de uma linha do meio (fora dos botões) e sobe 250 px.
            yi = max(yref, ALTURA[0] // 2)
            xi = 40
            passos = []
            for k in range(1, 9):
                passos += ["mousemove", str(xi), str(yi - 250 * k // 8), "sleep", "0.02"]
            xdo("mousemove", str(bx), str(by), "click", "1", "sleep", "0.08",
                "mousemove", str(xi), str(yi), "mousedown", "1", *passos, "sleep", "0.1", "mouseup", "1")
            time.sleep(1.2)
            t = r.textos()
            f = r.foto("-4")
            depois = [(y, n) for y, _, n in linhas(t) if n == nref]
            ydepois = depois[0][0] if depois else None
            rolou = ydepois is None or ydepois <= yref - 100
            print(f"MEDIDA K2 ({JANELA}): \"{nref}\" em y={yref} antes do arrasto; depois: "
                  f"{ydepois if ydepois is not None else 'fora da janela (rolou para cima)'}; B {situacao(pasta, 'B')}",
                  flush=True)
            ok("K2: \"Já pedi\" vale para B", situacao(pasta, "B") == "pedida", f)
            ok("K2: logo depois de a lista mudar, o arrasto na lista (fora dos botões) rola a lista", rolou,
               f"{f} {nref}: {yref} → {ydepois}")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def cenario_k7(placar, fotos):
    """K7 (S2-R1 da conferência Sonnet): a lista é destruída enquanto a conferência ainda espera. Com a conferência
    atrasada (1,5 s), "Já pedi" e, 0,3 s depois, o Esc (a tela do Estoque sai da pilha, e a lista com ela). Quando a
    conferência roda, a lista já não existe: os botões do resto do programa têm de continuar respondendo (aqui, o
    "Trocar usuário" do Início), sem erro de script."""
    ok = placar.ok
    rot = "K7"
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-k7-")
    try:
        montar_loja(pasta)
        por_fora(pasta, sql_encomendas(["A", "B"], 14))
        r, aberta = abrir(placar, fotos, pasta, rot, "A", 1500)
        try:
            if not aberta:
                return
            t, b = achar_botao(r, NOME + "A")
            if not ok(f"{rot}: o \"Já pedi\" da 1ª aberta à vista", b is not None, r.foto("-1")):
                return
            by, bx = b
            xdo("mousemove", str(bx), str(by), "mousedown", "1", "mouseup", "1", "sleep", "0.3", "key", "Escape")
            time.sleep(2.5)
            f = r.foto("-2")
            no_inicio = r.esperar(lambda: r.tem("Trocar usuário"), 5)
            ok(f"{rot}: \"Já pedi\" vale (A pedida) e o Esc volta ao Início", situacao(pasta, "A") == "pedida" and no_inicio,
               f"{f} A={situacao(pasta, 'A')}")
            if not no_inicio:
                return
            r.clicar("Trocar usuário")
            saiu = r.esperar(lambda: not r.tem("Trocar usuário"), 5)
            ok(f"{rot}: depois de a lista sumir com a conferência esperando, os botões continuam respondendo (o \"Trocar "
               "usuário\" do Início leva à entrada)", saiu, r.foto("-3"))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-lista-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    print(f"janela: {'celular 360 × 780' if nav.CELULAR else 'computador'}", flush=True)
    try:
        cenarios = (("K3", cenario_k3), ("K1/K2", cenario_k12),
                    ("K5", lambda p, f: cenario_k3(p, f, "K5", ATRASO_LENTO)),
                    ("K4", lambda p, f: cenario_k12(p, f, "K4", ATRASO_LENTO, com_k2=False)),
                    ("K7", cenario_k7))
        for nome, cenario in cenarios:
            try:
                cenario(placar, fotos)
            except Exception as e:
                placar.ok(f"{nome} ({JANELA}) chegou ao fim ({e})", False)
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
    print(f"RESULTADO ({JANELA}): {placar.certos} certos, {placar.errados} errados")
    saida = 0 if placar.errados == 0 else 1
    if not nav.CELULAR and not os.environ.get("RUSTINHO_LISTA_SO_ESTE"):
        env = dict(os.environ, RUSTINHO_CASOS_CELULAR="1", RUSTINHO_LISTA_SO_ESTE="1")
        s = subprocess.run([sys.executable, "-B", os.path.abspath(__file__), os.path.join(fotos, "celular")], env=env)
        saida = saida or s.returncode
    if not os.environ.get("RUSTINHO_LISTA_SO_ESTE"):
        print("TUDO CERTO" if saida == 0 else "HOUVE FALHAS")
    sys.exit(saida)


if __name__ == "__main__":
    main()
