#!/usr/bin/env python3
"""Teste das telas de verdade da 0.9.14: D-14 (caixa já fechado) e o
Rustinho portátil.

1. D-14: numa loja de teste com um caixa já fechado (venda de R$ 100,00 em
   dinheiro) e um caixa novo aberto com R$ 300,00, a gerente Ana abre a venda
   e cancela pela tela. Confere: o fechamento antigo não mudou, a gaveta de
   hoje ganhou a sangria ligada à venda, e a tela do caixa mostra o
   movimento.
2. Portátil: uma cópia do programa com a marca `rustinho-portatil.txt` ao
   lado, aberta com HOME, XDG_DATA_HOME e RUSTINHO_DADOS apontando para uma
   "instalação de verdade" de mentira (com um banco que não pode mudar).
   Confere: abre a tela de configurar uma loja nova, os dados ficam em
   `dados/` da cópia, e nada do Rustinho aparece fora dela.

Precisa de: Xvfb, xdotool, ImageMagick (import) e o Rustinho compilado
(cargo build -p rustinho).

    python3 app/testes/telas_d14_portatil.py [pasta para as fotos]
"""

import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as t  # noqa: E402  (Rustinho, xdo, conferir, simples)


def montar_loja_d14(pasta):
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "estavel", "--",
         "--ignored", "--exact", "a1_auditoria::montar_loja_d14"],
        cwd=t.RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )
    with open(os.path.join(pasta, "d14.json")) as f:
        return json.load(f)


def sql(pasta, consulta, *args):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute(consulta, args).fetchall()
    finally:
        c.close()


def resumo(caminho):
    with open(caminho, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def arquivos(pasta):
    lista = []
    for raiz, _, nomes in os.walk(pasta):
        for n in nomes:
            lista.append(os.path.join(raiz, n))
    return sorted(lista)


def d14(fotos):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-d14-")
    ids = montar_loja_d14(pasta)
    venda, fechado, aberto = ids["venda"], ids["fechado"], ids["aberto"]
    esperado_antes = sql(pasta, "SELECT esperado, contado FROM caixa_sessao WHERE id = ?", fechado)[0]
    certo = True
    r = t.Rustinho(pasta, "TelaVenda", {"vendaId": venda}, fotos, "d14-cancelar")
    try:
        r.entrar("Ana")
        certo &= t.conferir("d14: a venda do caixa fechado aberta", r.foto("-1"), r.textos(),
                            tem=["Cancelar venda", "100,00"])
        if not r.clicar("Cancelar venda"):
            raise RuntimeError(f"não achei o botão Cancelar venda (veja {r.saida})")
        time.sleep(1.5)
        certo &= t.conferir("d14: a caixa de cancelar explica o caixa fechado", r.foto("-2"), r.textos(),
                            tem=["Cancelar a venda?", "caixa já fechado", "o fechamento dele não muda"])
        t.xdo("type", "--delay", "60", "cliente voltou")
        t.xdo("key", "Return")
        time.sleep(3)
        certo &= t.conferir("d14: venda cancelada pela tela", r.foto("-3"), r.textos(), tem=["Venda cancelada"])
    finally:
        r.fechar()
    # Banco: a venda cancelada, marcada; a sangria na gaveta de hoje; o
    # fechamento antigo com o mesmo esperado e contado.
    situacao = sql(pasta, "SELECT situacao, cancelado_apos_fechar FROM venda WHERE id = ?", venda)[0]
    movs = sql(pasta, "SELECT sessao_id, tipo, valor, origem_tipo FROM caixa_movimento WHERE origem_id = ?", venda)
    ok = situacao == ("cancelada", 1) and movs == [(aberto, "sangria", 10000, "venda_cancelada")]
    print(("✔ " if ok else "✘ ") + f"d14: no banco, venda {situacao} e movimento {movs}")
    certo &= ok
    ok = sql(pasta, "SELECT esperado, contado FROM caixa_sessao WHERE id = ?", fechado)[0] == esperado_antes
    print(("✔ " if ok else "✘ ") + "d14: o fechamento gravado do caixa antigo não mudou")
    certo &= ok
    # A tela do caixa (o de hoje) mostra a sangria.
    r = t.Rustinho(pasta, "TelaCaixa", {}, fotos, "d14-caixa-hoje")
    try:
        r.entrar("Ana")
        certo &= t.conferir("d14: a gaveta de hoje mostra a sangria do cancelamento", r.foto(), r.textos(),
                            tem=["Cancelamento da venda", "caixa já fechado"])
    finally:
        r.fechar()
    shutil.rmtree(pasta, ignore_errors=True)
    return certo


def portatil(fotos):
    base = tempfile.mkdtemp(prefix="rustinho-portatil-tela-")
    casa = os.path.join(base, "casa")
    sempre = os.path.join(casa, ".local", "share", "rustinho")
    outra = os.path.join(casa, "dados-env")
    for p in (sempre, outra):
        os.makedirs(p)
        with open(os.path.join(p, "rustinho.db"), "wb") as f:
            f.write(b"banco da loja de verdade (nao pode mudar)")
    antes = [(a, resumo(a)) for a in arquivos(casa)]
    raiz = os.path.join(base, "Rustinho-Portatil")
    os.makedirs(raiz)
    programa = os.path.join(raiz, "rustinho")
    shutil.copy2(t.PROGRAMA, programa)
    shutil.copy2(os.path.join(t.RAIZ, "instalador", "portatil", "rustinho-portatil.txt"), raiz)

    saida = os.path.join(fotos, "portatil.log")
    log = open(saida, "w")
    env = dict(os.environ, DISPLAY=t.TELA, QT_QUICK_BACKEND="software", HOME=casa,
               XDG_DATA_HOME=os.path.join(casa, ".local", "share"), XDG_CACHE_HOME=os.path.join(casa, ".cache"),
               XDG_CONFIG_HOME=os.path.join(casa, ".config"), RUSTINHO_DADOS=outra)
    p = subprocess.Popen([programa, "--textos-da-tela"], env=env, stdout=log, stderr=log, cwd=base)
    certo = True
    try:
        banco = os.path.join(raiz, "dados", "rustinho.db")
        for _ in range(40):
            if os.path.exists(banco):
                break
            time.sleep(0.5)
        time.sleep(4)
        r = t.Rustinho.__new__(t.Rustinho)
        r.saida, r.fotos, r.nome = saida, fotos, "portatil"
        certo &= t.conferir("portátil: abre a loja nova (não a instalada)", r.foto(), r.textos(),
                            tem=["Nome da loja"])
        ok = os.path.exists(banco)
        print(("✔ " if ok else "✘ ") + "portátil: o banco ficou em dados/ da pasta portátil")
        certo &= ok
    finally:
        p.terminate()
        try:
            p.wait(10)
        except subprocess.TimeoutExpired:
            p.kill()
        log.close()
    depois = [(a, resumo(a)) for a in arquivos(casa)]
    mudou = [a for a in depois if a not in antes] + [a for a in antes if a not in depois]
    do_rustinho = [a for a, _ in mudou if "rustinho" in a.lower()]
    ok = not do_rustinho and resumo(os.path.join(sempre, "rustinho.db")) == antes[[a for a, _ in antes].index(
        os.path.join(sempre, "rustinho.db"))][1]
    print(("✔ " if ok else "✘ ") + "portátil: nada do Rustinho mudou fora da pasta dele"
          + ("" if ok else f" ({do_rustinho})"))
    if mudou and not do_rustinho:
        print("   (fora do Rustinho, só caches do sistema: " + ", ".join(os.path.relpath(a, casa) for a, _ in mudou[:5]) + ")")
    certo &= ok
    shutil.rmtree(base, ignore_errors=True)
    return certo


def main():
    for prog in ("Xvfb", "xdotool", "import"):
        if not shutil.which(prog):
            sys.exit(f"falta o programa {prog}")
    if not os.path.exists(t.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({t.PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-d14-")
    os.makedirs(fotos, exist_ok=True)
    xvfb = subprocess.Popen(["Xvfb", t.TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    try:
        certo = d14(fotos)
        certo &= portatil(fotos)
    finally:
        xvfb.terminate()
    print(f"fotos e registros em {fotos}")
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
