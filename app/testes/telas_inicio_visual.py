#!/usr/bin/env python3
"""Ajuste visual da tela inicial (15/11/2026): o funcionamento continua igual e as fotos para comparar.

Confere, com o Rustinho de verdade numa tela virtual:
  - as 14 pílulas, com os mesmos nomes, na janela larga e na estreita (uma coluna);
  - cada pílula abre a sua tela (uma por cima do Início) e o Esc volta; "Notas fiscais" (em breve) só avisa e fica
    no Início;
  - o F11 leva à busca (o texto digitado depois dele cai na busca);
  - nenhum TypeError/ReferenceError e nenhum ícone que não carregou.
E tira as fotos: larga, larga com o foco do teclado numa pílula (Tab) e estreita (uma coluna).

    RUSTINHO_TELA=:88 python3 app/testes/telas_inicio_visual.py [pasta das fotos]
    (RUSTINHO_PROGRAMA=<outro binário> tira as mesmas fotos de outra versão, para comparar)
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
from telas_navegacao_casos import PROGRAMA, TELA, Placar, montar_loja, xdo  # noqa: E402

PILULAS = ["Vender / Receber", "Peças", "Orçamentos", "Caixa", "Trocas e devoluções", "Notas fiscais", "Clientes",
           "Compras", "Estoque / Fornecedores", "Financeiro", "Ordens de serviço", "Painel", "Usuários", "Configurações"]


def janela(largura, altura):
    s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                       env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
    for j in s.stdout.split():
        xdo("windowsize", j, str(largura), str(altura))
    time.sleep(1.5)


def voltar_ao_inicio(r, inicio):
    for _ in range(3):
        if r.pilha() == inicio:
            return True
        xdo("key", "Escape")
        time.sleep(1.2)
    return r.pilha() == inicio


def roteiro(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-inicio-")
    try:
        montar_loja(pasta)
        r = placar.abrir(pasta, "TelaInicio", {}, fotos, "inicio")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.tem("Vender / Receber"), 15)
            time.sleep(1)
            inicio = r.pilha()
            f = r.foto("-larga")
            ok("as 14 pílulas, com os mesmos nomes (janela larga)", all(r.tem_igual(p) for p in PILULAS), f)
            # A composição: onde ficam a busca e a 1ª pílula (para comparar a altura do cabeçalho entre versões).
            textos = r.textos()
            busca = [y for _, y, t in textos if t.startswith("Buscar peça")]
            vender = [y for _, y, t in textos if t == "Vender / Receber"]
            print(f"MEDIDA: busca y={busca[0] if busca else None}; 'Vender / Receber' y={vender[0] if vender else None}",
                  flush=True)
            # "Estoque / Fornecedores" numa linha só a 1280 px: a altura da faixa de texto escuro em volta dele, na foto
            # (uma linha tem menos de 28 px; duas passam de 40).
            pontos = r.onde("Estoque / Fornecedores")
            altura = 0
            if pontos:
                from PIL import Image
                x, y = pontos[0]
                im = Image.open(f).convert("L").crop((x - 95, y - 34, x + 95, y + 34))
                linhas = [i for i in range(im.height) if min(im.getpixel((j, i)) for j in range(im.width)) < 90]
                altura = max(linhas) - min(linhas) + 1 if linhas else 0
            ok("'Estoque / Fornecedores' numa linha só (janela de 1280 px)", 0 < altura <= 28,
               f"{f} altura do texto = {altura} px")
            for p in PILULAS:
                r.clicar(p)
                if p == "Notas fiscais":
                    ok("Notas fiscais (em breve): só avisa e fica no Início",
                       r.esperar(lambda: r.tem("chega na etapa")) and r.pilha() == inicio)
                    continue
                abriu = r.esperar(lambda: r.pilha() == inicio + 1, 10)
                ok(f"{p}: abre a sua tela por cima do Início", abriu, "" if abriu else r.foto("-" + p[:6]))
                ok(f"{p}: o Esc volta ao Início", voltar_ao_inicio(r, inicio))
            xdo("key", "Tab")
            xdo("key", "Tab")
            xdo("key", "Tab")
            time.sleep(0.6)
            r.foto("-foco")
            xdo("key", "F11")
            time.sleep(0.5)
            xdo("type", "--delay", "60", "filtro")
            f = r.foto("-f11")
            ok("F11 leva à busca (o texto digitado cai nela)", r.esperar(lambda: r.tem_igual("filtro")), f)
            janela(700, 1000)
            r._altura = 1000
            r.esperar(lambda: r.tem("Configurações"))
            f = r.foto("-estreita")
            ok("as 14 pílulas na janela estreita (uma coluna)", all(r.tem_igual(p) for p in PILULAS), f)
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
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-inicio-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        try:
            roteiro(placar, fotos)
        except Exception as e:
            placar.ok(f"o roteiro chegou ao fim ({e})", False)
        erros = []
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                erros += [ln.strip() for ln in fh if "TypeError" in ln or "ReferenceError" in ln
                          or ("inicio-" in ln and ".png" in ln)]
        placar.ok("nenhum TypeError/ReferenceError nem ícone que não carregou", not erros, "; ".join(erros[:3]))
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("TUDO CERTO" if placar.errados == 0 else "HOUVE FALHAS")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    nav.TELA = TELA
    main()
