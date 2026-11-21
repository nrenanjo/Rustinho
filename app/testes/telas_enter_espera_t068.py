#!/usr/bin/env python3
"""Roteiro da T068 (AUD-L-2 da auditoria da 0.9.18-dev.3.3.3): o Enter dado dentro da espera da busca não age sobre a
lista de antes, em nenhuma tela.

O bug 22 (Peças, T067) tinha a mesma causa em outras telas: o campo de busca solta a busca 160 a 200 ms depois da
última tecla (um Timer de espera), e o Enter dado nesse intervalo (o leitor de código de barras manda o código e o
Enter em poucos ms) abria o item marcado da lista de ANTES. Duas partes:

  ESTÁTICA  em todo app/qml/*.qml, todo campo com "onEditado: <espera>.restart()" cujo Enter abre ou escolhe um item
            da lista (abrir, escolher, currentIndex, [0]) tem uma guarda: "<espera>.running" no próprio Enter (ou na
            função que ele chama), ou a comparação do texto da lista com o do campo (itensDe, textoDaLista,
            resultadosDe). Uma tela nova sem a guarda reprova.
  PESSOAS   (na tela): com dois clientes, a lista mostrando só "Maria" (marcada), digita "oficina" como o leitor
            (3 ms por tecla) e dá o Enter na hora: nenhuma ficha abre (a de Maria, nunca) e a lista mostra o Zé. Depois,
            com a lista certa, o Enter abre a ficha do Zé (o caminho de sempre).

    python3 app/testes/telas_enter_espera_t068.py [pasta para as fotos]
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_importar_cadastros as imp  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":88")
imp.TELA = base.TELA
TELA = base.TELA
QML = os.path.join(RAIZ, "app", "qml")
ACAO_DE_LISTA = re.compile(r"abrir|escolher|currentIndex|\[0\]")
GUARDAS_DE_TEXTO = ("itensDe", "textoDaLista", "resultadosDe")


def bloco(texto, inicio):
    """O corpo de um "onConfirmado:" a partir de `inicio`: o bloco entre chaves, ou a linha."""
    resto = texto[inicio:].lstrip()
    if not resto.startswith("{"):
        return resto.split("\n", 1)[0]
    prof, i = 0, 0
    for i, c in enumerate(resto):
        prof += {"{": 1, "}": -1}.get(c, 0)
        if prof == 0:
            break
    return resto[:i + 1]


def funcao(texto, nome):
    m = re.search(r"function\s+" + re.escape(nome) + r"\s*\(", texto)
    if not m:
        return ""
    return bloco(texto, texto.index("{", m.end()))


def estatica():
    """[(arquivo, linha, espera, corpo)] dos Enter que agem sobre a lista sem guarda; e quantos foram conferidos."""
    sem, conferidos = [], 0
    for nome in sorted(os.listdir(QML)):
        if not nome.endswith(".qml"):
            continue
        texto = open(os.path.join(QML, nome), encoding="utf-8").read()
        for m in re.finditer(r"onEditado:", texto):
            # a espera: "onEditado: x.restart()" na linha, ou um bloco "onEditado: { ...; x.restart() }"
            editado = bloco(texto, m.end())
            achou = re.search(r"(\w+)\.restart\(\)", editado)
            if not achou:
                continue
            espera = achou.group(1)
            # o onConfirmado do mesmo Campo: o próximo, antes do próximo onEditado ou "Campo {"
            fim_editado = texto.index(editado, m.end()) + len(editado)
            janela = texto[fim_editado:fim_editado + 1500]
            fim = min([x for x in (janela.find("onEditado:"), janela.find("Campo {")) if x >= 0] or [len(janela)])
            n = janela.find("onConfirmado:", 0, fim)
            if n < 0:
                continue
            corpo = bloco(janela, n + len("onConfirmado:"))
            alvo = corpo
            chamada = re.fullmatch(r"\s*(?:tela\.|secao\.|janela\.)?(\w+)\(\)\s*", corpo)
            if chamada:
                alvo = corpo + funcao(texto, chamada.group(1))
            if not ACAO_DE_LISTA.search(alvo):
                continue
            conferidos += 1
            if f"{espera}.running" in alvo or any(g in alvo for g in GUARDAS_DE_TEXTO):
                continue
            linha = texto[:m.start()].count("\n") + 1
            sem.append((nome, linha, espera, " ".join(corpo.split())[:120]))
    return sem, conferidos


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), check=True)


def estado(r, nomes):
    t = r.textos()
    textos = [tx for _, _, tx in t]
    titulos = [tx.split("|")[-1] for tx in textos if tx.startswith("RET:") and tx.split("|")[-2] == "Titulo"]
    for n in nomes:
        if n in titulos:
            return ("ficha", n)
    return ("lista", sorted({tx for tx in textos if tx in nomes}))


def esperar_estado(r, nomes, segundos=4.0):
    fim, antes = time.time() + segundos, None
    while time.time() < fim:
        e = estado(r, nomes)
        if e == antes:
            return e
        antes = e
        time.sleep(0.6)
    return antes


def main():
    certo = True

    def ok(nome, condicao, detalhe=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" ({detalhe})"))
        certo &= bool(condicao)

    sem, conferidos = estatica()
    ok(f"ESTÁTICA: todo Enter que abre ou escolhe da lista, num campo com espera, tem a guarda ({conferidos} conferidos)",
       conferidos >= 9 and not sem, f"sem guarda: {sem}")

    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-t068-")
    os.makedirs(fotos, exist_ok=True)
    raiz = tempfile.mkdtemp(prefix="rustinho-t068-")
    pasta = os.path.join(raiz, "loja")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    antes = os.environ.get("RUSTINHO_ARGS_TESTE")
    os.environ["RUSTINHO_ARGS_TESTE"] = ((antes or "") + " --medir-toque").strip()
    maria, ze = "Maria Teste Leitora", "Zé da Oficina"
    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        base.montar_loja(pasta)
        # o 2º cliente, copiado do Zé da loja de teste (todas as colunas obrigatórias preenchidas)
        imp.por_fora(pasta, f"CREATE TEMP TABLE t AS SELECT * FROM pessoa WHERE nome = '{ze}';"
                            f"UPDATE t SET id = lower(hex(randomblob(16))), codigo = 'T068', nome = '{maria}', "
                            f"fantasia = '', documento = '', telefone = '', celular = '', email = '', "
                            f"busca = '{maria} T068';"
                            "INSERT INTO pessoa SELECT * FROM t;")
        r = base.Rustinho(pasta, "TelaPessoas", {}, fotos, "pessoas")
        try:
            r.entrar("Ana")
            time.sleep(1.5)
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "120", "Maria")
            time.sleep(1.5)
            e0 = esperar_estado(r, (maria, ze))
            if e0 != ("lista", [maria]):
                raise RuntimeError(f"INSTRUMENTAÇÃO: a lista de Maria não apareceu ({e0}; foto {r.foto('-instr')})")
            xdo("key", "ctrl+a")
            xdo("type", "--delay", "3", "oficina")
            xdo("key", "Return")
            e = esperar_estado(r, (maria, ze))
            f = r.foto("-1-leitura")
            ok("PESSOAS: o Enter dentro da espera não abre a ficha de Maria (a da lista de antes)",
               e != ("ficha", maria), f"{e}; foto {f}")
            ok("PESSOAS: nada abre, e a lista mostra o Zé", e == ("lista", [ze]), f"{e}; foto {f}")
            if e[0] == "ficha":
                xdo("key", "Escape")
                time.sleep(1.5)
            else:
                xdo("key", "Return")
                e2 = esperar_estado(r, (maria, ze))
                ok("PESSOAS: com a lista certa, o Enter abre a ficha do Zé (o caminho de sempre)", e2 == ("ficha", ze),
                   f"{e2}; foto {r.foto('-2-enter')}")
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
