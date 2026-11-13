#!/usr/bin/env python3
"""Roteiro da digitação rápida na busca de Peças (dev.3.3, passo 7.6 — H09).

Abre o Rustinho de verdade numa tela virtual 1920 × 1080 (Xvfb), no ambiente
neutro dos outros roteiros, com a loja de 20.000 peças da 7.5, montada PELO
NÚCLEO (`nucleo/tests/lojas_de_teste.rs::montar_lista_continua`, em release).
A loja montada fica guardada em `target/lojas_de_teste/` (a chave é o conteúdo
das migrações e da montagem): a próxima rodada copia a pasta em vez de montar
de novo. Cada rodada usa a sua cópia.

Digita na busca de Peças como um balconista rápido e confere (T5 do desenho
da 7.6):

  R1 o campo mostra todas as letras, na ordem (nenhuma se perde nem troca);
  R2 a lista final é a da busca do texto final: o mesmo resultado que a tela
     lê de novo, pelo caminho de sempre, ao voltar de uma peça;
  R3 nenhuma pausa da linha da janela passa de 100 ms enquanto se digita e até
     a lista final chegar. A pausa é medida dentro do programa por um relógio
     de 16 ms (só com --textos-da-tela; linhas TESTE_PAUSA do registro);
  R4 nenhum TypeError/ReferenceError no registro.

A digitação tem duas partes:
  A. 30 teclas com 20 ms entre elas (o texto, cinco apagadas e o fim), como
     no desenho da 7.6: só a última busca roda;
  B. o campo limpo e 19 teclas com 220 ms entre elas, mais que a espera de
     160 ms da busca: cada tecla dispara uma busca enquanto a pessoa continua
     digitando, a começar pela primeira letra sozinha ("p"), que acha quase a
     loja inteira (é o caso do H09: as letras esperam a busca anterior).

Os tempos de cada busca (TESTE_BUSCA_PRONTA) e as pausas vão para o resultado.

    python3 app/testes/telas_busca_rapida.py [pasta para as fotos]

RUSTINHO_LOJA_PECAS=<n> muda o número de peças (só para ensaio; o critério da
7.6 é com 20.000). Precisa de: Xvfb, xdotool, ImageMagick (import), cargo.
Sai com 0 se aprovado.
"""

import glob
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":74")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA
PECAS = int(os.environ.get("RUSTINHO_LOJA_PECAS", "20000"))
PAUSA_MAXIMA_MS = 100

# A: 30 teclas, 20 ms entre elas. B: do campo limpo, 19 teclas, 220 ms entre elas.
TEXTO_A = "pastilha lista 12341"          # 20 teclas
APAGAR_A = 5                              # → "pastilha lista "
FIM_A = "12346"                           # 5 teclas → "pastilha lista 12346"
TEXTO_B = "pastilha lista 1111"           # 19 teclas, uma busca por tecla
TEXTO_FINAL = TEXTO_B


def chave_da_loja():
    """O conteúdo das migrações e da montagem da loja: muda, monta de novo."""
    h = hashlib.sha256()
    arquivos = sorted(glob.glob(os.path.join(base.RAIZ, "nucleo", "src", "migracoes", "*")))
    arquivos.append(os.path.join(base.RAIZ, "nucleo", "tests", "lojas_de_teste.rs"))
    for a in arquivos:
        h.update(os.path.basename(a).encode())
        with open(a, "rb") as f:
            h.update(f.read())
    h.update(str(PECAS).encode())
    return h.hexdigest()[:16]


def preparar_loja(destino):
    """Copia a loja guardada (ou monta, guarda e copia). Devolve (segundos, montou)."""
    guardada = os.path.join(base.RAIZ, "target", "lojas_de_teste", f"lista_continua_{PECAS}_{chave_da_loja()}")
    inicio = time.time()
    montou = False
    if not os.path.isfile(os.path.join(guardada, "rustinho.db")):
        montando = guardada + ".montando"
        shutil.rmtree(montando, ignore_errors=True)
        env = dict(os.environ, RUSTINHO_LOJA_TESTE=montando, RUSTINHO_LOJA_PECAS=str(PECAS))
        subprocess.run(
            ["cargo", "test", "--release", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
             "--ignored", "--exact", "montar_lista_continua"],
            cwd=base.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
        )
        shutil.rmtree(guardada, ignore_errors=True)
        os.rename(montando, guardada)
        montou = True
    shutil.rmtree(destino, ignore_errors=True)
    shutil.copytree(guardada, destino)
    return time.time() - inicio, montou


def marcador(textos, nome):
    for _, _, t in textos:
        if t.startswith(nome + ":"):
            return t[len(nome) + 1:]
    return None


def lista(textos):
    m = marcador(textos, "TESTE_LISTA")
    if m is None:
        return None
    n, dif, mais, atual = m.split(":")[:4]
    return int(n), int(dif), mais == "1", int(atual)


def nomes_na_tela(textos):
    geos = {(x, y) for x, y, t in textos if t.startswith("GEO:nomePeca:")}
    return [t for x, y, t in sorted(textos, key=lambda v: v[1])
            if (x, y) in geos and not t.startswith(("GEO:", "RET:"))]


def texto_do_campo(textos, g):
    """O texto do campo da busca (o que está dentro do retângulo dele)."""
    x, y, w, h = g["campoDeBusca"]
    achados = [t for px, py, t in textos
               if x <= px <= x + w and y <= py <= y + h and not t.startswith(("GEO:", "RET:", "TOQUE:"))]
    return achados


def linhas_do_registro(arquivo, prefixo):
    """[(quando em ms, resto)] das linhas do registro com o prefixo."""
    saida = []
    with open(arquivo, errors="replace") as f:
        for linha in f:
            i = linha.find(prefixo + ":")
            if i < 0:
                continue
            resto = linha[i + len(prefixo) + 1:].strip()
            quando, _, mais = resto.partition(":")
            if quando.isdigit():
                saida.append((int(quando), mais))
    return saida


def teclar(texto_ou_tecla, atraso_ms, tecla=False, vezes=1):
    """Uma tecla por vez, com o atraso entre elas (o xdotool espera dentro dele)."""
    if tecla:
        base.xdo("key", "--delay", str(atraso_ms), *([texto_ou_tecla] * vezes))
    else:
        base.xdo("type", "--delay", str(atraso_ms), texto_ou_tecla)


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"Falta o programa {p}")

    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-busca-rapida-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-dados-busca-rapida-")

    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}
    info = []

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"Xvfb não abriu na tela {TELA}")
        segundos, montou = preparar_loja(os.path.join(pasta, "loja"))
        info.append(f"loja de {PECAS} peças {'montada pelo núcleo e guardada' if montou else 'copiada da guardada'}"
                    f" em {segundos:.0f} s")
        sha = subprocess.run(["sha256sum", os.path.join(pasta, "loja", "rustinho.db")],
                             capture_output=True, text=True).stdout.split()[0]
        info.append(f"banco da rodada (antes de abrir): sha256 {sha}")
        rr = u.Rustinho(os.path.join(pasta, "loja"), "TelaPecas", {}, fotos, "busca_rapida")
        try:
            for _ in range(150):
                s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                                   env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
                if s.stdout.split():
                    break
                time.sleep(0.2)
            time.sleep(0.5)
            rr.redimensionar(1920, 1080)
            time.sleep(1)
            rr.entrar("Ana")
            time.sleep(3.0)
            t = rr.textos()
            info.append(f"lista ao abrir: {lista(t)}")
            g = u.geos(t)
            rr.clicar_em(g["campoDeBusca"])
            time.sleep(0.6)
            # O ponteiro sai de cima da lista (a roda do mouse não mexe nela) e
            # fica na janela (sem gerenciador de janelas, o teclado vai para a
            # janela embaixo do ponteiro).
            wx, wy, w, h = tema.janela_do_rustinho()
            base.xdo("mousemove", str(wx + w - 4), str(wy + h - 4))
            time.sleep(0.4)

            # ---- Digitação (nada de ler a tela no meio: ler a tela também pausa a janela) ----
            inicio_ms = int(time.time() * 1000)
            teclar(TEXTO_A, 20)
            teclar("BackSpace", 20, tecla=True, vezes=APAGAR_A)
            teclar(FIM_A, 20)
            fim_a_ms = int(time.time() * 1000)
            time.sleep(0.4)
            base.xdo("key", "ctrl+a")
            base.xdo("key", "BackSpace")
            time.sleep(0.4)
            teclar(TEXTO_B, 220)
            ultima_tecla_ms = int(time.time() * 1000)
            time.sleep(5.0)
            fim_ms = int(time.time() * 1000)

            rapida = rr.textos()
            rr.foto("-1-depois-de-digitar")
            g = u.geos(rapida)
            campo = texto_do_campo(rapida, g)
            l_rapida = lista(rapida)
            n_rapida = nomes_na_tela(rapida)
            busca_da_lista = marcador(rapida, "TESTE_BUSCA")

            # ---- O caminho de sempre: abrir a peça e voltar lê a lista de novo ----
            base.xdo("key", "Return")
            time.sleep(2.5)
            base.xdo("key", "Escape")
            time.sleep(3.0)
            de_novo = rr.textos()
            rr.foto("-2-lida-de-novo")
            l_de_novo = lista(de_novo)
            n_de_novo = nomes_na_tela(de_novo)

            pausas = [(q, int(r.split(":")[0])) for q, r in linhas_do_registro(rr.saida, "TESTE_PAUSA")
                      if r.split(":")[0].isdigit()]
            na_digitacao = [p for q, p in pausas if inicio_ms <= q <= fim_ms]
            buscas = [(q, r) for q, r in linhas_do_registro(rr.saida, "TESTE_BUSCA_PRONTA") if inicio_ms <= q <= fim_ms]
            tempos = []
            for q, r in buscas:
                ms, _, txt = r.partition(":")
                tempos.append((int(ms) if ms.isdigit() else -1, txt, q))
            ultima = [q for _, txt, q in tempos if txt == TEXTO_FINAL]

            info.append(f"parte A ({len(TEXTO_A) + APAGAR_A + len(FIM_A)} teclas, 20 ms): "
                        f"{fim_a_ms - inicio_ms} ms no xdotool")
            info.append(f"buscas prontas na digitação: {len(tempos)}; "
                        + "; ".join(f"“{txt}” {ms} ms" for ms, txt, _ in tempos))
            info.append(f"pausas da janela > 50 ms na digitação: {len(na_digitacao)}; maior "
                        f"{max(na_digitacao) if na_digitacao else 0} ms; todas {sorted(na_digitacao, reverse=True)[:12]}")
            if ultima:
                info.append(f"a lista do texto final ficou pronta {max(ultima) - ultima_tecla_ms} ms depois de o xdotool "
                            "terminar (ele espera 220 ms depois da última tecla; a busca começa 160 ms depois dela)")
            info.append(f"lista depois de digitar: {l_rapida}, busca “{busca_da_lista}”, primeiras {n_rapida[:3]}")
            info.append(f"lista lida de novo: {l_de_novo}, primeiras {n_de_novo[:3]}")

            conferir("R1", "o campo mostra todas as letras, na ordem",
                     TEXTO_FINAL in campo, f"campo {campo}")
            conferir("R2", "a lista final é a da busca do texto final (igual à lida de novo pelo caminho de sempre)",
                     busca_da_lista == TEXTO_FINAL and l_rapida is not None and l_de_novo is not None
                     and l_rapida[:3] == l_de_novo[:3] and n_rapida and n_rapida == n_de_novo,
                     f"busca “{busca_da_lista}”; {l_rapida} × {l_de_novo}; {n_rapida[:3]} × {n_de_novo[:3]}")
            conferir("R3", f"nenhuma pausa da janela passa de {PAUSA_MAXIMA_MS} ms enquanto se digita",
                     all(p <= PAUSA_MAXIMA_MS for p in na_digitacao) and tempos,
                     f"maior pausa {max(na_digitacao) if na_digitacao else 0} ms; "
                     f"{sum(1 for p in na_digitacao if p > PAUSA_MAXIMA_MS)} acima de {PAUSA_MAXIMA_MS} ms")
        finally:
            rr.fechar()
        with open(rr.saida, errors="replace") as f:
            erros = [ln.strip() for ln in f if re.search(r"TypeError|ReferenceError", ln)]
        conferir("R4", "nenhum TypeError/ReferenceError no registro", not erros, f"{erros[:3]}")
    finally:
        shutil.rmtree(pasta, ignore_errors=True)
        xvfb.terminate()
        xvfb.wait()

    for linha in info:
        print(f"  · {linha}")
    reprovado = False
    for codigo in sorted(resultados, key=lambda c: int(c[1:])):
        certo, nome, detalhe = resultados[codigo]
        if certo:
            print(f"  ✔ {codigo} {nome}")
        else:
            print(f"  ✘ {codigo} {nome} — {detalhe}")
            reprovado = True
    certas = sum(1 for c in resultados.values() if c[0])
    print(f"\n{certas} de {len(resultados)} conferências certas; fotos em {fotos}")
    sys.exit(1 if reprovado or len(resultados) != 4 else 0)


if __name__ == "__main__":
    main()
