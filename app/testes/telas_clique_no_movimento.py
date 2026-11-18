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
# Seis encomendas abertas (a mesma loja da sonda L do revisor).
SQL_ENCOMENDAS = """
CREATE TEMP TABLE s(n INTEGER PRIMARY KEY);
WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 6) INSERT INTO s SELECT n FROM c;
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
    for _, _, tx in t:
        if tx.startswith("GEO:setaDeRetorno:"):
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
            # No celular, a lista fica abaixo dos cartões: rolar até três linhas dentro da janela.
            for n in (3, 4, 5):
                if len(linhas(r.textos())) >= 3:
                    break
                r.rolar_ate(NOME + str(n))
                time.sleep(0.8)
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
            if not ok("J1: o aviso aparece e a 1ª encomenda fica pedida", com is not None and pedidas(pasta) == [primeira],
                      f"{r.foto('-2')} pedidas={pedidas(pasta)}"):
                return
            d = deslocamento(sem, com)
            ls2 = linhas(com)
            # A linha seguinte (a que a pessoa marcaria em seguida): a primeira abaixo de onde estava a 1ª.
            abaixo = [v for v in ls2 if v[2] != primeira and v[0] > ls[1][0] + 10]
            if not ok("J1: com o aviso, as telas desceram e há uma linha seguinte à vista", d > 20 and abaixo,
                      f"{r.foto('-2')} descida={d} linhas={[(y, n) for y, _, n in ls2]}"):
                return
            alvo = abaixo[0][2]
            com, b2 = ler_botao(r, ls2, alvo)
            avisos = aviso_em(com)
            if not ok(f"J1: com o aviso à vista, o \"Já pedi\" de \"{alvo}\" à vista", b2 is not None and avisos,
                      r.foto("-3")):
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
            xdo("mousemove", str(ax), str(ay), "click", "1", "mousemove", str(bx), str(py),
                "sleep", str(ATRASO_DO_CLIQUE), "click", "1")
            time.sleep(1.5)
            f = r.foto("-4-depois-do-clique-no-movimento")
            agora = pedidas(pasta)
            print(f"MEDIDA J1 ({JANELA}): pedidas depois do clique no movimento: {agora}", flush=True)
            ok("J1: o clique dado enquanto as telas voltam é descartado (nenhuma outra encomenda muda)",
               agora == [primeira], f"{f} pedidas={agora}")

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
