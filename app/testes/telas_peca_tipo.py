#!/usr/bin/env python3
"""Teste da troca produto ↔ serviço na ficha da peça (0.9.18, NEW-B8B2-02).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros testes das telas, numa peça nova, e confere que o fiscal escondido do
outro tipo não é conferido nem gravado:

  T1. Produto com NCM incompleto ("1234"), trocado para Serviço e salvo:
      salva (antes: "O NCM tem 8 números", num bloco sem pílula), e o
      serviço fica gravado sem NCM.
  T2. Serviço com código de serviço comprido demais (ISS), trocado para
      Produto e salvo: salva, e o produto fica gravado sem o bloco do ISS.
  T3. Nenhum TypeError/ReferenceError nos registros.

    python3 app/testes/telas_peca_tipo.py [pasta para as fotos]
    RUSTINHO_TELA=:73 RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_peca_tipo.py

Precisa de: Xvfb, xdotool e ImageMagick (import). Sai com 0 se tudo conferiu.
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":73")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402  (traz o ambiente neutro e o Rustinho com --medir-toque)

base, tema = u.base, u.tema
TELA = u.TELA


def achar(rr, texto):
    """(x, y) do texto na tela, rolando a ficha até ele aparecer inteiro e
    parado (a rolagem pela roda do mouse ainda desliza um pouco depois)."""
    anterior = None
    # A ficha rola por baixo do cabeçalho fixo ("Nova peça", Salvar...): o
    # que está acima de TOPO fica escondido atrás dele.
    topo = 200
    for _ in range(16):
        achados = [(x, y) for x, y, t in rr.textos() if base.simples(t) == base.simples(texto)]
        _, _, jw, jh = tema.janela_do_rustinho()
        dentro = [(x, y) for x, y in achados if topo < y < jh - 20]
        if dentro:
            if dentro[0] == anterior:
                return dentro[0]
            anterior = dentro[0]
            time.sleep(0.5)
            continue
        if achados:
            sentido = "5" if achados[0][1] >= jh - 20 else "4"
            px, py = rr.ponto(jw * 0.75, jh / 2)
            base.xdo("mousemove", str(px), str(py), "click", sentido, "click", sentido)
            time.sleep(0.7)
        else:
            time.sleep(0.5)
    return None


def tocar(rr, texto):
    p = achar(rr, texto)
    if p:
        base.xdo("mousemove", *map(str, rr.ponto(*p)), "click", "1")
        time.sleep(0.5)
    return p is not None


def digitar(rr, dica, valor):
    """Toca no campo pela dica (o texto de exemplo) e digita."""
    if not tocar(rr, dica):
        return False
    base.xdo("key", "ctrl+a")
    base.xdo("type", "--delay", "40", valor)
    time.sleep(0.3)
    return True


def gravado(pasta, nome):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute("SELECT tipo, ncm, fiscal FROM produto WHERE nome = ?", (nome,)).fetchone()
    finally:
        c.close()


def caso(modelo, fotos, res, nome, passos, conferir):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-tipo-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    rr = u.Rustinho(pasta, "TelaPeca", {}, fotos, nome)
    try:
        rr.entrar("Ana")
        time.sleep(2)
        certo = passos(rr)
        res.conferir(f"{nome}: os passos na tela", certo, rr.saida)
        base.xdo("key", "F2")   # Salvar
        time.sleep(2.5)
        textos = [t for _, _, t in rr.textos()]
        foto = rr.foto("-salvo")
        erros = [t for t in textos if "NCM tem 8" in t or "Código do serviço: use" in t]
        res.conferir(f"{nome}: salvou sem erro do fiscal escondido", not erros, f"{erros[:2]}; foto {foto}")
    finally:
        rr.fechar()
    conferir(gravado(pasta, nome.split(":")[0]), res, nome)
    shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(u.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({u.PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-tipo-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-tipo-modelo-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    res = tema.Resultado()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(modelo)

        def servico_com_ncm(rr):
            return (digitar(rr, "8 números", "1234") and tocar(rr, "Serviço (sem estoque)")
                    and tocar(rr, "Preço digitado na hora da venda") and nome_da_peca(rr, "Troca T1"))

        def conferir_t1(linha, res, nome):
            res.conferir(f"{nome}: gravado como serviço e sem NCM", linha is not None and linha[0] == "servico"
                         and linha[1] == "", f"{linha}")

        def produto_com_iss(rr):
            ok = tocar(rr, "Serviço (sem estoque)") and tocar(rr, "ISS")
            ok = ok and digitar(rr, "Ex.: 14.01", "CODIGO-LONGO-DEMAIS") if achar(rr, "Ex.: 14.01") else False
            return (ok and tocar(rr, "Produto") and tocar(rr, "Preço digitado na hora da venda")
                    and nome_da_peca(rr, "Troca T2"))

        def conferir_t2(linha, res, nome):
            fiscal = json.loads(linha[2]) if linha else {}
            res.conferir(f"{nome}: gravado como produto e sem o código de serviço", linha is not None
                         and linha[0] == "produto" and not fiscal.get("servicoCodigo") and not fiscal.get("servico_codigo"),
                         f"{linha}")

        caso(modelo, fotos, res, "Troca T1: produto com NCM ruim vira serviço", servico_com_ncm, conferir_t1)
        caso(modelo, fotos, res, "Troca T2: serviço com ISS ruim vira produto", produto_com_iss, conferir_t2)
        erros = [f for f in os.listdir(fotos) if f.endswith(".log")
                 and any(e in open(os.path.join(fotos, f), errors="replace").read() for e in ("TypeError", "ReferenceError"))]
        res.conferir("T3 nenhum TypeError/ReferenceError nos registros", not erros, ", ".join(erros))
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(modelo, ignore_errors=True)
    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha. Fotos em {fotos}")
    for f in res.falhas:
        print("  ✘ " + f)
    sys.exit(0 if not res.falhas else 1)


def nome_da_peca(rr, nome):
    """O nome por último (a ficha nova abre com o cursor nele): toca no
    rótulo "Nome da peça" e no campo logo abaixo."""
    p = achar(rr, "Nome da peça") or achar(rr, "Nome do serviço")
    if not p:
        return False
    x, y = p
    base.xdo("mousemove", *map(str, rr.ponto(x, y + 34)), "click", "1")
    time.sleep(0.3)
    base.xdo("key", "ctrl+a")
    base.xdo("type", "--delay", "40", nome)
    time.sleep(0.3)
    return True


if __name__ == "__main__":
    main()
