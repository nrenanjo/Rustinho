#!/usr/bin/env python3
"""N6 (decisão do proprietário, 18/11/2026: "1a"). Quando o aviso (ou a orientação da guia) entra ou sai, as telas descem
ou voltam pela altura dele. Um clique dado nesse instante caía em outra linha: o "Já pedi" marcava a encomenda vizinha
(M-2 da revisão do VIS2, sonda L do revisor). Agora, enquanto as telas se movem, e por um instante depois, o clique nas
telas é DESCARTADO (não vai para lugar nenhum); passado o instante, volta a valer.

Em Estoque › O que comprar, com seis encomendas abertas:
  J1  "Já pedi" numa linha: o aviso aparece e as telas descem. Com o aviso à vista, anota onde está o "Já pedi" da linha
      seguinte. Fecha o aviso com um toque (as telas voltam) e, 0,2 s depois, clica num "Já pedi": no ponto antigo, se
      depois da volta cair ali o botão de outra linha (o caso do revisor); senão, no ponto novo do mesmo botão. Esse
      clique tem de ser descartado: nenhuma outra encomenda muda.
  J2  Passado o instante, o clique vale: o "Já pedi" da mesma linha, lido de novo, marca ela, e só ela.
  J3  Clique duplo no "Já pedi" de outra linha (o 2º clique sai quando as telas descem com o aviso): marca uma só.
  J4  (Decisão 2c, bug YVZ4) Com o aviso JÁ à vista (as telas não se movem), clique duplo no "Já pedi" de uma linha: a
      linha sai da lista, a de baixo sobe para o lugar dela, e o 2º clique cairia nela. Marca uma só.
  J5  (Decisão 2c) "Marcar todas como pedidas" pede confirmação: nada muda antes de confirmar; confirmado, marca todas.
Roda no computador e, num 2º processo, no desenho do celular (360 × 780).

    RUSTINHO_TELA=:88 python3 app/testes/telas_clique_no_movimento.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o de antes, para a prova)
"""
import collections
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
AVISO = "marcada como pedida"
NOME = "Encomenda do roteiro "
# Dez encomendas abertas (a loja da sonda L do revisor, com mais linhas para o J4 e o J5).
SQL_ENCOMENDAS = """
CREATE TEMP TABLE s(n INTEGER PRIMARY KEY);
WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 10) INSERT INTO s SELECT n FROM c;
INSERT INTO encomenda (id, numero, cliente_nome, telefone, descricao, quantidade, situacao, criado_em, atualizado_em, dispositivo)
SELECT 'roteiro-e-' || n, 1000 + n, 'Cliente do roteiro ' || n, '', 'Encomenda do roteiro ' || n,
       1000, 'aberta', strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-' || n || ' minutes'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
       (SELECT dispositivo FROM produto LIMIT 1) FROM s;
"""
# O instante depois da volta em que o clique ainda é descartado (Main.qml: 140 ms da volta + 300 ms). O clique de
# prova sai 0,2 s depois do toque; o de J2, 1,5 s depois.
ATRASO_DO_CLIQUE = 0.2


def escrito(tx):
    return not tx.startswith(("GEO:", "TOQUE:", "RET:"))


# A altura da janela (o desenho tem linhas fora dela, abaixo da borda: no celular, a lista começa abaixo de 780).
ALTURA = [0]


def na_janela(y):
    return 60 <= y < ALTURA[0] - 30


def linhas(t):
    """[(y, x, nome)] das linhas de encomenda dentro da janela, de cima para baixo."""
    return sorted((y, x, tx) for x, y, tx in t if tx.startswith(NOME) and na_janela(y))


def botoes(t):
    return sorted((y, x) for x, y, tx in t if tx == "Já pedi" and na_janela(y))


def aviso_em(t):
    return [(x, y) for x, y, tx in t if AVISO in tx and escrito(tx)]


def pedidas(pasta):
    return [d for (d,) in ler(pasta, "SELECT descricao FROM encomenda WHERE situacao = 'pedida' ORDER BY numero")]


def botao_da_linha(t, ls, nome):
    """O "Já pedi" da linha `nome`: o primeiro botão abaixo do começo da linha e acima do começo da seguinte."""
    i = [n for _, _, n in ls].index(nome)
    y0 = ls[i][0] - 30
    y1 = ls[i + 1][0] - 30 if i + 1 < len(ls) else ls[i][0] + 120
    b = [(y, x) for y, x in botoes(t) if y0 <= y < y1]
    return b[0] if b else None


def ler_botao(r, ls, nome):
    """Lê a tela com o "Já pedi" da linha à vista (no computador, ele só aparece com o ponteiro em cima da linha)."""
    if not nav.CELULAR:
        y, x, _ = next(v for v in ls if v[2] == nome)
        xdo("mousemove", str(x), str(y))
        time.sleep(0.5)
    t = r.textos()
    return t, botao_da_linha(t, linhas(t), nome)


def seta_y(t):
    """O topo da tela: a seta de retorno; no celular, sem a seta desde a T047 (pedido de 18/11/2026), o cabeçalho
    da tela, onde a seta ficava (move-se com a tela do mesmo jeito)."""
    # (Um programa de antes da T047 ainda tem a seta no celular e não dá o nome do cabeçalho: vale a seta.)
    for nome in (("cabecalhoDaTela", "setaDeRetorno") if nav.CELULAR else ("setaDeRetorno",)):
        for _, _, tx in t:
            if tx.startswith(f"GEO:{nome}:"):
                return int(tx.rsplit(":", 1)[1].split(",")[1])
    return None


def deslocamento(sem, com):
    """Quanto as telas desceram com o aviso. Pela seta de retorno, que fica no cabeçalho da tela (fixo; no celular, o
    miolo com a lista rola à parte e os textos dele mudam de lugar com a lista). Sem a seta: a diferença mais comum de
    altura entre os textos fixos (fora as linhas e os botões) lidos sem e com o aviso."""
    a0, a1 = seta_y(sem), seta_y(com)
    if a0 is not None and a1 is not None and a1 != a0:
        return a1 - a0
    a = collections.Counter((x, tx) for x, _, tx in sem)
    b = collections.Counter((x, tx) for x, _, tx in com)
    ys = {(x, tx): y for x, y, tx in sem}
    difs = collections.Counter()
    for x, y, tx in com:
        k = (x, tx)
        if escrito(tx) and a[k] == 1 and b[k] == 1 and not tx.startswith(NOME) and tx != "Já pedi" and AVISO not in tx:
            if y - ys[k] != 0:
                difs[y - ys[k]] += 1
    return difs.most_common(1)[0][0] if difs else 0


def esperar_textos(r, condicao, segundos):
    fim = time.time() + segundos
    while time.time() < fim:
        t = r.textos()
        if condicao(t):
            return t
    return None


def cenario(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-clique-")
    try:
        montar_loja(pasta)
        por_fora(pasta, SQL_ENCOMENDAS)
        r = placar.abrir(pasta, "TelaEstoque", {"secao": "comprar"}, fotos, f"J-{JANELA}")
        try:
            r.entrar("Ana")
            if not ok("J: a lista de encomendas aparece", r.esperar(lambda: r.tem("Encomendas de clientes"), 15),
                      r.foto("-0")):
                return
            time.sleep(1.2)
            ALTURA[0] = r.altura()
            # No celular, a lista fica abaixo dos cartões: rolar até três linhas dentro da janela (as mais novas vêm
            # primeiro: 10, 9, 8…), só o necessário, e esperar a lista parar (um toque na lista ainda deslizando só a
            # para, e não aciona o botão).
            for n in (8, 7, 6):
                if len(linhas(r.textos())) >= 3:
                    break
                r.rolar_ate(NOME + str(n))
                time.sleep(0.8)
            antes_parar = None
            for _ in range(10):
                agora_l = [(y, n) for y, _, n in linhas(r.textos())]
                if agora_l == antes_parar:
                    break
                antes_parar = agora_l
                time.sleep(0.4)
            sem = r.textos()
            ls = linhas(sem)
            if not ok("J: há pelo menos 3 encomendas à vista", len(ls) >= 3, f"{r.foto('-0')} {len(ls)} linhas"):
                return

            # J1. "Já pedi" na 2ª linha: o aviso aparece e as telas descem.
            primeira = ls[1][2]
            t, b = ler_botao(r, ls, primeira)
            if not ok(f"J1: o \"Já pedi\" de \"{primeira}\" à vista", b is not None, r.foto("-1")):
                return
            r.clicar_em(b[1], b[0])
            com = esperar_textos(r, aviso_em, 6)
            visto = time.time()
            if not ok("J1: o aviso aparece e a 1ª encomenda fica pedida", com is not None and pedidas(pasta) == [primeira],
                      f"{r.foto('-2')} pedidas={pedidas(pasta)}"):
                return
            ls2 = linhas(com)
            # A linha seguinte (a que a pessoa marcaria em seguida): a primeira abaixo de onde estava a 1ª.
            abaixo = [v for v in ls2 if v[2] != primeira and v[0] > ls[1][0] + 10]
            if not ok("J1: com o aviso, há uma linha seguinte à vista", abaixo,
                      f"{r.foto('-2')} linhas={[(y, n) for y, _, n in ls2]}"):
                return
            alvo = abaixo[0][2]
            com, b2 = ler_botao(r, ls2, alvo)
            # N6-R6 da revisão: a 1ª leitura com o aviso pode cair no meio da descida (140 ms; na validação, 57 px em vez
            # de 66). A descida vale só com a tela parada: a seta de retorno no mesmo lugar em duas leituras seguidas,
            # com o aviso à vista.
            depois_de = r.textos()
            d = deslocamento(sem, depois_de)
            if os.environ.get("RUSTINHO_CLIQUE_PROVA_D"):
                # Só para a prova do próprio critério (n6_evid): uma descida errada de propósito; o J1 não pode passar.
                d = int(os.environ["RUSTINHO_CLIQUE_PROVA_D"])
            parada = seta_y(com) is not None and seta_y(com) == seta_y(depois_de) and aviso_em(depois_de)
            avisos = aviso_em(com)
            if not ok(f"J1: com o aviso à vista e a tela parada, o \"Já pedi\" de \"{alvo}\" à vista",
                      b2 is not None and avisos and parada and d > 20,
                      f"{r.foto('-3')} INSTRUMENTAÇÃO: seta {seta_y(com)} e {seta_y(depois_de)}; descida {d}"):
                return
            by, bx = b2
            # Depois da volta, cada linha sobe `d`: se outra linha cair no lugar da seguinte, o clique vai no ponto
            # antigo, onde fica o botão dela (o caso do revisor: a encomenda vizinha); senão, no ponto novo do mesmo
            # botão. (Pelas linhas, e não pelos botões: no computador, o botão só aparece com o ponteiro em cima.)
            ya = next(y for y, _, n in linhas(com) if n == alvo)
            vizinha = [n for y, _, n in linhas(com) if n != alvo and abs((y - d) - ya) <= 12]
            if vizinha:
                modo, py = f"o ponto antigo (lá cai o botão de \"{vizinha[0]}\")", by
            else:
                modo, py = "o ponto novo do mesmo botão", by - d
            ax, ay = avisos[0]
            print(f"MEDIDA J1 ({JANELA}): descida {d} px; aviso em ({ax}, {ay}); \"Já pedi\" de \"{alvo}\" em ({bx}, "
                  f"{by}); clique em ({bx}, {py}), {modo}, {ATRASO_DO_CLIQUE} s depois do toque", flush=True)
            # O toque no aviso (as telas voltam); o ponteiro vai logo ao ponto, como o de quem já ia clicar ali; o
            # clique sai 0,2 s depois do toque. Tudo num comando só do xdotool, sem esperar pela leitura da tela. (No
            # computador, o "Já pedi" só aparece com o ponteiro em cima da linha: com o ponteiro parado no ponto, a
            # linha que chega embaixo dele mostra o botão dela.)
            # O aviso fica 3 s: numa máquina lenta, ele pode sumir sozinho antes do toque (como no cenário D da CI do
            # bb32ea2). Com mais de 2,5 s desde que foi visto, o J1 não decide (inconclusivo por tempo, sem aprovar).
            espera = time.time() - visto
            print(f"MEDIDA J1 ({JANELA}): o toque {espera:.1f} s depois de ver o aviso", flush=True)
            if not ok("J1: o toque vem antes de o aviso sumir sozinho (menos de 2,5 s depois de visto)", espera < 2.5,
                      f"INSTRUMENTAÇÃO (tempo): {espera:.1f} s"):
                return
            xdo("mousemove", str(ax), str(ay), "click", "1", "mousemove", str(bx), str(py),
                "sleep", str(ATRASO_DO_CLIQUE), "click", "1")
            time.sleep(1.5)
            f = r.foto("-4-depois-do-clique-no-movimento")
            agora = pedidas(pasta)
            # N6-R6: o clique de prova só prova algo se o ponto dele cai num "Já pedi" com a tela parada depois da
            # volta (senão, sem a correção, ele cairia num vão e nada mudaria: um verde falso). No computador, o
            # ponteiro continua no ponto, e a linha embaixo dele mostra o botão.
            sobre = [(y, x) for y, x in botoes(r.textos()) if abs(x - bx) < 40 and abs(y - py) <= 14]
            print(f"MEDIDA J1 ({JANELA}): pedidas depois do clique no movimento: {agora}; um \"Já pedi\" no ponto do "
                  f"clique depois da volta: {sobre or 'nenhum'}", flush=True)
            if agora == [primeira]:
                ok("J1: o clique dado enquanto as telas voltam é descartado (nenhuma outra encomenda muda; o ponto do "
                   "clique cai num \"Já pedi\")", bool(sobre),
                   f"{f} INSTRUMENTAÇÃO: o ponto ({bx}, {py}) não cai em nenhum \"Já pedi\" depois da volta")
            else:
                ok("J1: o clique dado enquanto as telas voltam é descartado (nenhuma outra encomenda muda)", False,
                   f"{f} pedidas={agora}")

            # J2. Passado o instante, o clique vale: o "Já pedi" de alvo, lido de novo, marca ela.
            if not esperar_textos(r, lambda t: not aviso_em(t), 8):
                ok("J2: o aviso some", False, r.foto("-5"))
                return
            time.sleep(0.6)
            t = r.textos()
            # A mesma linha, se continua aberta (sem a correção, o clique de J1 já a marcou); senão, outra aberta.
            abertas = [n for _, _, n in linhas(t) if n not in pedidas(pasta)]
            alvo = alvo if alvo in abertas else (abertas[0] if abertas else alvo)
            t, b3 = ler_botao(r, linhas(t), alvo) if alvo in abertas else (t, None)
            if not ok(f"J2: o \"Já pedi\" de \"{alvo}\" à vista", b3 is not None, r.foto("-5")):
                return
            antes = pedidas(pasta)
            r.clicar_em(b3[1], b3[0])
            esperar_textos(r, aviso_em, 6)
            time.sleep(0.6)
            agora = pedidas(pasta)
            ok("J2: passado o instante, o clique vale: marca a linha clicada, e só ela",
               sorted(agora) == sorted(antes + [alvo]), f"{r.foto('-6')} antes={antes} depois={agora}")

            # J3. Clique duplo no "Já pedi" de outra linha: o 2º clique sai quando as telas descem com o aviso.
            esperar_textos(r, lambda t: not aviso_em(t), 8)
            time.sleep(0.6)
            t = r.textos()
            ls3 = [v for v in linhas(t) if v[2] not in pedidas(pasta)]
            if not ok("J3: há outra encomenda aberta à vista", len(ls3) >= 2, r.foto("-7")):
                return
            outra = ls3[1][2] if len(ls3) > 2 else ls3[0][2]
            t, b4 = ler_botao(r, linhas(t), outra)
            if not ok(f"J3: o \"Já pedi\" de \"{outra}\" à vista", b4 is not None, r.foto("-7")):
                return
            antes = pedidas(pasta)
            xdo("mousemove", str(b4[1]), str(b4[0]), "click", "--repeat", "2", "--delay", "120", "1")
            time.sleep(1.5)
            agora = pedidas(pasta)
            print(f"MEDIDA J3 ({JANELA}): pedidas antes {antes}; depois do clique duplo {agora}", flush=True)
            ok("J3: o clique duplo no \"Já pedi\" marca uma encomenda só (a clicada)",
               sorted(agora) == sorted(antes + [outra]), f"{r.foto('-8')} antes={antes} depois={agora}")

            # J4. Com o aviso JÁ à vista: "Já pedi" numa linha (o aviso aparece); passado o instante da N6, clique duplo
            # no "Já pedi" de outra linha, com outra aberta logo abaixo. A altura do aviso não muda (as telas não se
            # movem): sem a proteção da 2c, o 2º clique caía na linha que sobe.
            esperar_textos(r, lambda t: not aviso_em(t), 8)
            time.sleep(0.6)
            t = r.textos()
            abertas = [v for v in linhas(t) if v[2] not in pedidas(pasta)]
            if not ok("J4: há pelo menos três encomendas abertas à vista", len(abertas) >= 3, r.foto("-9")):
                return
            t, b5 = ler_botao(r, linhas(t), abertas[0][2])
            if not ok(f"J4: o \"Já pedi\" de \"{abertas[0][2]}\" à vista", b5 is not None, r.foto("-9")):
                return
            r.clicar_em(b5[1], b5[0])
            if not ok("J4: o aviso aparece", esperar_textos(r, aviso_em, 6) is not None, r.foto("-9")):
                return
            visto4 = time.time()
            time.sleep(0.8)
            t = r.textos()
            abertas = [v for v in linhas(t) if v[2] not in pedidas(pasta)]
            alvo4 = abertas[0][2] if len(abertas) >= 2 else None
            t, b6 = ler_botao(r, linhas(t), alvo4) if alvo4 else (t, None)
            if not ok(f"J4: com o aviso à vista, o \"Já pedi\" de \"{alvo4}\" e outra encomenda abaixo dela",
                      b6 is not None and aviso_em(t), r.foto("-10")):
                return
            antes = pedidas(pasta)
            espera = time.time() - visto4
            if not ok("J4: o clique duplo vem com o aviso ainda à vista (menos de 2,5 s depois de visto)", espera < 2.5,
                      f"INSTRUMENTAÇÃO (tempo): {espera:.1f} s"):
                return
            xdo("mousemove", str(b6[1]), str(b6[0]), "click", "--repeat", "2", "--delay", "200", "1")
            time.sleep(1.5)
            agora = pedidas(pasta)
            print(f"MEDIDA J4 ({JANELA}): pedidas antes {antes}; depois do clique duplo com o aviso à vista {agora}",
                  flush=True)
            ok("J4: com o aviso já à vista, o clique duplo no \"Já pedi\" marca uma encomenda só (a clicada)",
               sorted(agora) == sorted(antes + [alvo4]), f"{r.foto('-11')} antes={antes} depois={agora}")

            # J5. "Marcar todas como pedidas" pede confirmação antes de gravar.
            esperar_textos(r, lambda t: not aviso_em(t), 8)
            time.sleep(0.6)
            todas = [(x, y) for x, y, tx in r.textos() if tx == "Marcar todas como pedidas" and na_janela(y)]
            if not ok("J5: o botão \"Marcar todas como pedidas\" à vista", todas, r.foto("-12")):
                return
            x, y = todas[0]
            if nav.CELULAR:
                x = min(x, nav.CELULAR_DP[0] - 20)  # no celular, o botão sai cortado pela direita (bug 3M90)
            antes = pedidas(pasta)
            r.clicar_em(x, y)
            time.sleep(1.2)
            agora = pedidas(pasta)
            perguntou = r.tem("encomendas como pedidas?")
            print(f"MEDIDA J5 ({JANELA}): pedidas antes {len(antes)}; depois do clique em Marcar todas {len(agora)}; "
                  f"perguntou={perguntou}", flush=True)
            if not ok("J5: \"Marcar todas como pedidas\" pergunta antes, e nada muda sem a confirmação",
                      perguntou and agora == antes, f"{r.foto('-13')} antes={len(antes)} depois={len(agora)}"):
                return
            r.clicar("Marcar todas")
            time.sleep(1.5)
            sobrou = ler(pasta, "SELECT COUNT(*) FROM encomenda WHERE situacao = 'aberta'")[0][0]
            ok("J5: confirmado, marca todas", sobrou == 0, f"{r.foto('-14')} abertas={sobrou}")
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
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-clique-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    print(f"janela: {'celular 360 × 780' if nav.CELULAR else 'computador'}", flush=True)
    try:
        try:
            cenario(placar, fotos)
        except Exception as e:
            placar.ok(f"J ({JANELA}) chegou ao fim ({e})", False)
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
    # O celular num 2º processo (o desenho do celular é escolhido na importação do roteiro de navegação).
    if not nav.CELULAR and not os.environ.get("RUSTINHO_CLIQUE_SO_ESTE"):
        env = dict(os.environ, RUSTINHO_CASOS_CELULAR="1", RUSTINHO_CLIQUE_SO_ESTE="1")
        s = subprocess.run([sys.executable, "-B", os.path.abspath(__file__), os.path.join(fotos, "celular")], env=env)
        saida = saida or s.returncode
    if not os.environ.get("RUSTINHO_CLIQUE_SO_ESTE"):
        print("TUDO CERTO" if saida == 0 else "HOUVE FALHAS")
    sys.exit(saida)


if __name__ == "__main__":
    main()
