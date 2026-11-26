#!/usr/bin/env python3
"""T035 da 005 (M-1 da R13): o clique do mouse numa linha tocável DENTRO de uma janelinha não atravessa para a linha da
tela de trás (no toque do celular, nem antes vazava).

Caso real: Estoque › Encomendas com três encomendas; "Nova encomenda"; no campo Cliente, a busca lista os clientes
(linhas com TapHandler) por cima da lista de encomendas (linhas com TapHandler que escolhem a encomenda e mostram os
botões dela, como "Cancelar encomenda"). Antes da T035, o clique no cliente também escolhia a encomenda que estava atrás.

Conferências:
- CONTROLE do sensor (B-4 da R14): com a "Pastilha dianteira" escolhida antes, o sensor ("Cancelar encomenda", lido com o
  mouse fora da janela) vê exatamente a linha dela; um sensor cego reprova;
- CONTROLE da sobreposição: a linha do cliente fica na altura (e no x) de uma linha de encomenda atrás, que não é a
  Pastilha; sem isso, o roteiro reprova como "não medido" (ele não pode passar sem provar nada; B-1 da R13);
- o clique no cliente escolhe o cliente (a linha da lista some; o nome vai para o campo);
- a escolha da lista de trás não muda (os "Cancelar encomenda" continuam na Pastilha).

    python3 app/testes/telas_janelinha_nao_vaza.py [pasta das fotos]
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import Placar, montar_loja, por_fora, xdo, TELA  # noqa: E402

TITULOS = ("Pastilha dianteira", "Correia dentada", "Bomba de óleo")


def unicos(pontos, folga=4):
    fica = []
    for p in sorted(pontos, key=lambda q: (q[1], q[0])):
        if not any(abs(p[0] - q[0]) <= folga and abs(p[1] - q[1]) <= folga for q in fica):
            fica.append(p)
    return fica


def escolhidos(r):
    """Onde estão os "Cancelar encomenda" com o mouse FORA da janela: eles aparecem com o mouse em cima da linha ou na
    linha escolhida (SecaoEncomendas.qml); sem o mouse em cima, só os da linha escolhida."""
    xdo("mousemove", "1400", "950")
    time.sleep(0.6)
    return unicos(r.onde("Cancelar encomenda"))


def linha_do_cliente(r, nome):
    """A linha do cliente na lista da janelinha ("Nome" ou "Nome  ·  telefone"), e não a linha de uma encomenda atrás
    ("Nº 1 (Caixa 1) · Nome · …")."""
    return [(x, y) for x, y, t in r.textos() if t == nome or t.startswith(nome + "  ·")]


def caso(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-janelinha-vaza-")
    try:
        montar_loja(pasta, avisos=True)
        # Mais duas encomendas (a mesma receita da s6 do telas_c8_rerevisao2), para a lista de trás ter linhas na
        # altura da lista de clientes da janelinha.
        por_fora(pasta,
                 "CREATE TEMP TABLE o2 AS SELECT * FROM encomenda WHERE situacao = 'aberta' LIMIT 1; "
                 "UPDATE o2 SET id = 'enc-v2', cliente_nome = 'Cliente Dois', descricao = 'Correia dentada'; "
                 "INSERT INTO encomenda SELECT * FROM o2; "
                 "UPDATE o2 SET id = 'enc-v3', cliente_nome = 'Cliente Tres', descricao = 'Bomba de óleo', "
                 "criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+1 minute'); "
                 "INSERT INTO encomenda SELECT * FROM o2;")
        r = placar.abrir(pasta, "TelaEstoque", {"secao": "encomendas"}, fotos, "vaza")
        try:
            r.entrar("Ana")
            # A janela mais alta (1280 × 1000): a janelinha fica no centro e desce, e a lista de encomendas, presa ao
            # alto, fica onde está; assim a lista de clientes da janelinha cai em cima de uma linha de encomenda.
            s_ = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(r.p.pid)],
                                env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
            for janela in s_.stdout.split():
                subprocess.run(["xdotool", "windowsize", janela, "1280", "1000"], env=dict(os.environ, DISPLAY=TELA))
            time.sleep(1.5)
            if not ok("as encomendas aparecem", r.esperar(lambda: all(r.tem(t) for t in TITULOS), 15), r.foto("-1")):
                return
            # B-4 da R14: o controle positivo do sensor. Antes de abrir a janelinha, escolhe com o mouse uma encomenda que
            # NÃO fica atrás do clique (a "Pastilha dianteira") e confere que o sensor a vê; depois, exige o mesmo
            # resultado (se vazar, os botões mudam para a linha de trás).
            pastilha = [(x, y) for x, y, t in r.textos() if t == "Pastilha dianteira"]
            if pastilha:
                r.clicar_em(*pastilha[0])
                time.sleep(0.8)
            antes = escolhidos(r)
            print(f"MEDIDA: \"Cancelar encomenda\" com a Pastilha escolhida: {antes}", flush=True)
            if not ok("CONTROLE: o sensor vê a encomenda escolhida (os botões da Pastilha à vista)",
                      bool(pastilha) and len(antes) == 1 and abs(antes[0][1] - (pastilha[0][1] + 8)) < 25, r.foto("-1b")):
                return
            r.clicar("Nova encomenda")
            if not ok("a janelinha da nova encomenda abre", r.esperar(lambda: r.tem("Para o cliente que paga"), 10),
                      r.foto("-2")):
                return
            time.sleep(0.8)
            # O cliente da loja de teste (comum::povoar): "Zé da Oficina".
            r.digitar("Oficina")
            achou = r.esperar(lambda: bool(linha_do_cliente(r, "Zé da Oficina")), 10)
            f = r.foto("-3")
            if not ok("a busca lista o cliente na janelinha", achou, f):
                return
            cliente = linha_do_cliente(r, "Zé da Oficina")[-1]
            # O controle: alguma linha de encomenda atrás está na mesma altura da linha do cliente (a linha da
            # encomenda tem 66 px; o título fica ~11 px acima do meio).
            # A linha de encomenda ocupa a largura da lista (x de 32 a 1248): o controle confere a altura e que o x do
            # clique está dentro dela (B-4 da R14); a Pastilha (escolhida antes) não pode ser a de trás.
            atras = [(t, y) for x, y, t in r.textos() if t in TITULOS and t != "Pastilha dianteira"
                     and abs((y + 11) - cliente[1]) < 30 and 32 < cliente[0] < 1248]
            print(f"MEDIDA: cliente em {cliente}; linhas de trás na mesma altura: {atras}", flush=True)
            if not ok("CONTROLE: há uma linha de encomenda atrás da linha do cliente (senão, não mede nada)",
                      bool(atras), f):
                return
            r.clicar_em(*cliente)
            time.sleep(1.2)
            f = r.foto("-4")
            # escolherCliente põe o nome no campo e esvazia a lista (a linha "Zé da Oficina  ·  …" some).
            ok("o clique escolhe o cliente (a linha da lista some; o nome vai para o campo)",
               not any(abs(y - cliente[1]) < 6 for _, y in linha_do_cliente(r, "Zé da Oficina")), f)
            depois = escolhidos(r)
            print(f"MEDIDA: \"Cancelar encomenda\" antes {antes}, depois {depois}", flush=True)
            ok("o clique na janelinha não escolhe a encomenda de trás (os botões continuam na Pastilha)",
               len(depois) == len(antes) and all(abs(a[1] - d[1]) <= 4 for a, d in zip(antes, depois)), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-vaza-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        try:
            caso(placar, fotos)
        except Exception as e:
            placar.ok(f"chegou ao fim ({e!r})", False)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
