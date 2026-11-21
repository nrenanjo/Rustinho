#!/usr/bin/env python3
"""Roteiro de tela da T067 (bug 22): a busca de Peças pelo leitor de código de barras abria a ficha da peça de antes.

O proprietário viu (21/11/2026, Windows, 0.9.18-dev.3.3.1): em Peças, depois de consultar uma peça (LAMPADA H11), a
leitura do código de outra (7896562108338, PARADA DE MOTOR) abriu a ficha da LAMPADA, e não a lista com a peça lida.

O leitor "digita" o código em poucos milissegundos e manda o Enter logo depois. Este roteiro faz o mesmo pelo xdotool
(--delay 3 por tecla e o Enter em seguida) e confere, pelos textos da tela, QUAL peça aparece: o título da ficha (se
alguma abriu) e os nomes na lista. Cenários do complemento ao briefing (texto N, §4):

  A  ficha da peça A aberta e fechada; leitura do código de B: a lista mostra B e nenhuma ficha abre (a de A, nunca);
  B  A, depois as leituras de B, de C e de novo de B: cada leitura mostra só a peça do último código;
  C  A, depois a leitura de um código que não existe: nenhuma ficha abre e a lista fica sem peças;
  D  leituras seguidas, sem pausa (B e logo C): vale a última (C), e nenhuma ficha abre;
  E  o mesmo que o A, com a peça A ainda marcada na lista (a seleção de antes não interfere);
  F  digitação manual, devagar (e o Enter só depois de a lista mostrar a peça): o Enter abre a ficha DESSA peça (o
     caminho de sempre do teclado continua);
  G  o clique numa linha da lista abre a ficha daquela linha (o caminho de sempre do mouse continua).
  (O leitor físico, o cenário G do texto N, é com o proprietário.)

    python3 app/testes/telas_pecas_leitor_t067.py [pasta para as fotos]
    RUSTINHO_TELA=:87 RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_pecas_leitor_t067.py
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_importar_cadastros as imp  # noqa: E402
from telas_d09 import PROGRAMA  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":87")
imp.TELA = base.TELA
TELA = base.TELA


def ean13(doze):
    s = sum(int(d) * (1 if i % 2 == 0 else 3) for i, d in enumerate(doze))
    return doze + str((10 - s % 10) % 10)


# As 3 peças e os códigos delas (o de B é o da embalagem da foto do proprietário).
PECAS = {
    "A": ("Filtro de óleo", ean13("789100010010")),
    "B": ("Óleo 5W30", ean13("789656210833")),
    "C": ("Pastilha de freio", ean13("789050000012")),
}
DESCONHECIDO = ean13("789999999999")
NOMES = {nome for nome, _ in PECAS.values()}


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def ler_com_o_leitor(codigo):
    """Como o leitor: o código em poucos milissegundos e o Enter na hora."""
    xdo("type", "--delay", "3", codigo)
    xdo("key", "Return")


def estado(r):
    """("ficha", título) se uma ficha de peça está aberta; senão ("lista", [nomes das 3 peças na lista])."""
    t = r.textos()
    textos = [tx for _, _, tx in t]
    if "Salvar" in textos and any(tx.lower() == "nome da peça" for tx in textos):
        titulos = [tx.split("|")[-1] for tx in textos if tx.startswith("RET:") and tx.split("|")[-2] == "Titulo"]
        dos_nomes = [tx for tx in titulos if tx in NOMES]
        return ("ficha", dos_nomes[0] if dos_nomes else (titulos[0] if titulos else "?"))
    nomes = sorted({tx.split("|")[-1] for tx in textos
                    if tx.startswith("RET:") and tx.split("|")[-2] == "LinhaPeca" and tx.split("|")[-1] in NOMES})
    return ("lista", nomes)


def esperar_estado(r, segundos=4.0):
    """O estado depois de a tela assentar: duas leituras iguais seguidas (a lista pode chegar depois do Enter)."""
    fim = time.time() + segundos
    antes = None
    while time.time() < fim:
        e = estado(r)
        if e == antes:
            return e
        antes = e
        time.sleep(0.6)
    return antes


def voltar(r):
    xdo("key", "Escape")
    time.sleep(1.5)


def limpar_e_digitar(r, texto, delay="120"):
    xdo("key", "ctrl+a")
    xdo("type", "--delay", delay, texto)


def abrir_ficha_pela_lista(r, nome):
    """Procura pelo nome (digitando devagar), espera a lista e clica na linha da peça (o caminho de sempre). Lê onde
    clicar quando o clique vale (textos_para_clicar: N6 e YVZ4 descartam o clique logo depois de a lista mudar) e
    confere que a ficha DESSA peça abriu; senão, para (erro de instrumentação, não resultado)."""
    limpar_e_digitar(r, nome)
    time.sleep(1.5)
    for _x, _y, tx in r.textos_para_clicar():
        # A linha da lista (o componente LinhaPeca), e não o próprio campo de busca, que também mostra o nome digitado.
        if tx.startswith("RET:") and tx.split("|")[-2] == "LinhaPeca" and tx.split("|")[-1] == nome:
            rx, ry, rw, rh = (float(v) for v in tx[4:].split("|")[0].split(",")[:4])
            xdo("mousemove", str(int(rx + rw / 2)), str(int(ry + rh / 2)), "click", "1")
            e = esperar_estado(r)
            if e != ("ficha", nome):
                raise RuntimeError(f"INSTRUMENTAÇÃO: o clique na linha de {nome} não abriu a ficha dela ({e}; "
                                   f"foto {r.foto('-instrumentacao')})")
            return True
    raise RuntimeError(f"INSTRUMENTAÇÃO: {nome} não apareceu na lista para o clique")


def voltar_da_ficha(r):
    """Fecha a ficha (Esc) e confere que voltou para a lista de Peças (o Esc na lista sairia de Peças)."""
    voltar(r)
    e = esperar_estado(r)
    if e[0] != "lista":
        raise RuntimeError(f"INSTRUMENTAÇÃO: o Esc não voltou da ficha para a lista ({e})")
    return e


QML_PECAS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "qml", "TelaPecas.qml")


def volta_do_importador_seleciona(caminho=QML_PECAS):
    """T083 (OPUS-E-12): ao fechar o "Trazer cadastros" em Peças, o texto da busca fica selecionado (focar), como ao
    voltar da ficha: a leitura seguinte troca o texto, em vez de se juntar a ele."""
    texto = open(caminho, encoding="utf-8").read()
    inicio = texto.index("ImportarCadastros {")
    bloco = texto[inicio:texto.index("onClosed:", inicio) + 400]
    fechar = bloco[bloco.index("onClosed:"):]
    fechar = fechar[:fechar.index("}") + 1]
    return "busca.focar()" in fechar and "focarCursor" not in fechar


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-leitor-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-leitor-")
    pasta = os.path.join(raiz, "loja")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certo = True

    def ok(nome, condicao, detalhe=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" ({detalhe})"))
        certo &= bool(condicao)

    ok("H: ao fechar o Trazer cadastros, a busca fica com o texto selecionado (no código, T083)",
       volta_do_importador_seleciona(), "o onClosed do importador usa focarCursor()")
    antes = os.environ.get("RUSTINHO_ARGS_TESTE")
    os.environ["RUSTINHO_ARGS_TESTE"] = ((antes or "") + " --medir-toque").strip()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)
        sql = []
        for nome, codigo in PECAS.values():
            sql.append(f"UPDATE produto SET codigo_barras = '{codigo}' WHERE nome = '{nome}';")
            sql.append(f"INSERT INTO produto_gtin (id, produto_id, gtin, tipo, origem, ordem, criado_em) "
                       f"SELECT lower(hex(randomblob(16))), id, '{codigo}', 'comercial', 'manual', 0, '' "
                       f"FROM produto WHERE nome = '{nome}';")
        imp.por_fora(pasta, "".join(sql))
        r = base.Rustinho(pasta, "TelaPecas", {}, fotos, "leitor")
        try:
            r.entrar("Ana")
            time.sleep(1.5)
            a, b, c = PECAS["A"], PECAS["B"], PECAS["C"]

            # ---- A e E: a ficha de A aberta e fechada (A fica marcada); a leitura de B ----
            abrir_ficha_pela_lista(r, a[0])
            ok(f"G: o clique na linha de A ({a[0]}) abre a ficha de A (o caminho de sempre do mouse)", True)
            e = voltar_da_ficha(r)
            ok("preparo: de volta à lista, com A nela (e marcada)", a[0] in e[1], f"{e}")
            ler_com_o_leitor(b[1])
            e = esperar_estado(r)
            f = r.foto("-A1-leitura-de-B")
            ok(f"A/E: a leitura do código de B ({b[1]}) não abre a ficha de A", e != ("ficha", a[0]), f"{e}; foto {f}")
            ok("A: a leitura não abre ficha nenhuma: mostra a lista para escolher", e[0] == "lista", f"{e}; foto {f}")
            ok(f"A: a lista mostra só B ({b[0]})", e == ("lista", [b[0]]), f"{e}; foto {f}")
            if e[0] == "ficha":
                voltar(r)

            # ---- B: B, C e de novo B, cada leitura com o código de antes ainda no campo ----
            for rotulo, peca in (("C", c), ("B de novo", b)):
                ler_com_o_leitor(peca[1])
                e = esperar_estado(r)
                f = r.foto(f"-B-{rotulo.replace(' ', '-')}")
                ok(f"B: a leitura de {rotulo} mostra só {peca[0]}, sem abrir ficha", e == ("lista", [peca[0]]),
                   f"{e}; foto {f}")
                if e[0] == "ficha":
                    voltar(r)

            # ---- C: código desconhecido, depois de consultar A ----
            abrir_ficha_pela_lista(r, a[0])
            voltar_da_ficha(r)
            ler_com_o_leitor(DESCONHECIDO)
            e = esperar_estado(r)
            f = r.foto("-C-desconhecido")
            ok(f"C: o código que não existe ({DESCONHECIDO}) não abre ficha nenhuma", e[0] == "lista", f"{e}; foto {f}")
            ok("C: a lista fica sem as peças A, B e C", e == ("lista", []), f"{e}; foto {f}")
            if e[0] == "ficha":
                voltar(r)

            # ---- D: leituras seguidas, sem pausa ----
            abrir_ficha_pela_lista(r, a[0])
            voltar_da_ficha(r)
            ler_com_o_leitor(b[1])
            ler_com_o_leitor(c[1])
            e = esperar_estado(r, 6.0)
            f = r.foto("-D-seguidas")
            ok("D: leituras seguidas (B e logo C): vale a última (C), sem ficha", e == ("lista", [c[0]]),
               f"{e}; foto {f}")
            if e[0] == "ficha":
                voltar(r)

            # ---- F: digitação manual, devagar; o Enter só depois de a lista mostrar a peça ----
            limpar_e_digitar(r, b[1], "120")
            time.sleep(1.5)
            e0 = esperar_estado(r)
            xdo("key", "Return")
            e = esperar_estado(r)
            f = r.foto("-F-manual")
            ok("F: digitando devagar, a lista mostra B antes do Enter", e0 == ("lista", [b[0]]), f"{e0}")
            ok(f"F: o Enter abre a ficha de B ({b[0]}), a peça da lista (o caminho de sempre do teclado)",
               e == ("ficha", b[0]), f"{e}; foto {f}")
            if e[0] == "ficha":
                voltar(r)

            with open(r.saida, encoding="utf-8", errors="replace") as reg:
                erros = [linha.strip() for linha in reg if "TypeError" in linha or "ReferenceError" in linha]
            ok("nenhum TypeError/ReferenceError no registro", not erros, str(erros[:3]))
        finally:
            r.fechar()
    finally:
        if antes is None:
            os.environ.pop("RUSTINHO_ARGS_TESTE", None)
        else:
            os.environ["RUSTINHO_ARGS_TESTE"] = antes
        shutil.rmtree(raiz, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()
    print(f"\nfotos em {fotos}")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
