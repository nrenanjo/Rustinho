#!/usr/bin/env python3
"""Medições de layout e contraste do passo 7.2 (dev.3.3).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros roteiros, e MEDE pelas posições que o próprio programa informa
(--textos-da-tela --medir-toque) e pelos pixels da foto:

  M1  H22       Etiquetas no AMOLED: o número sob as barras, na prévia (fundo
                branco), tem contraste de texto (>= 4,5:1).
  M2  VIS33-13  AMOLED: a caixa de marcar desmarcada ("Nome da loja") se
                distingue da faixa embaixo (>= 3:1, contraste de componente).
  M3  VIS33-08  Celular (360 × 780), Painel › Resumo: nenhum rótulo de cartão
                alarga o cartão nem passa da margem da tela.
  M4  VIS33-09  Celular, Financeiro › A pagar sem contas: o estado vazio fica
                abaixo dos filtros (não por baixo do "Categorias") e dentro da tela.
  M5  VIS33-05  1366 × 768, Vender com uma peça no carrinho: "Salvar como
                orçamento" inteiro dentro da janela.
  M6  H06       1366 × 768, Receber com crediário: "Concluir venda" inteiro
                dentro da janela.
  M7  H06       Celular, Receber com crediário: "Concluir venda" inteiro dentro
                da tela.
  M8  H04       Roxo, pendências XML: o motivo (vermelho) de cada cartão tem
                contraste de texto (>= 4,5:1), inclusive na faixa escura.
  M10 H10       Peça com variação de nome "<i>P</i>" na grade: o texto formatado
                (StyledText) recebe o nome escapado (&lt;i&gt;), e não a marcação.

O contraste é o da WCAG 2: fundo = a cor mais comum em volta e dentro do
retângulo do texto (ou da caixa); frente = o pixel de maior contraste com o
fundo dentro dele.

Antes das correções da 7.2 (b3a171c), todas são falhas ESPERADAS (ESPERADAS
abaixo); cada commit de correção tira o seu item da lista. Se uma esperada
passar, o roteiro avisa e reprova (é hora de tirá-la da lista).

    python3 app/testes/telas_layout_72.py [pasta para as fotos]

Precisa de: Xvfb, xdotool, ImageMagick (import), Pillow, cargo.
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":66")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA
CELULAR = (360, 780)

# Item → (passo). Cada correção da 7.2 tira o seu.
ESPERADAS = {
    "M2": "VIS33-13 (caixa de marcar sem contraste no AMOLED), passo 7.2",
    "M5": "VIS33-05 (\"Salvar como orçamento\" cortado em 1366 × 768), passo 7.2",
    "M6": "H06 (janelinha do Receber sem rolagem em 1366 × 768), passo 7.2",
    "M7": "H06 (janelinha do Receber sem rolagem no celular), passo 7.2",
    "M8": "H04 (vermelho sobre a faixa escura nas pendências), passo 7.2",
    "M10": "H10 (texto formatado com dados sem escapar), passo 7.2",
}


# ---- contraste (WCAG 2) ----

def _lin(c):
    c = c / 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminancia(rgb):
    r, g, b = rgb
    return 0.2126 * _lin(r) + 0.7152 * _lin(g) + 0.0722 * _lin(b)


def contraste(a, b):
    la, lb = sorted((luminancia(a), luminancia(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def medir_contraste(foto, x, y, w, h, folga=3):
    """(razão, fundo, frente) dentro do retângulo (pixels da foto)."""
    from PIL import Image
    img = Image.open(foto).convert("RGB")
    x0, y0 = max(0, int(x) - folga), max(0, int(y) - folga)
    x1, y1 = min(img.width, int(x + w) + folga), min(img.height, int(y + h) + folga)
    contagem = {}
    for i in range(x0, x1):
        for j in range(y0, y1):
            p = img.getpixel((i, j))
            contagem[p] = contagem.get(p, 0) + 1
    if not contagem:
        return 0.0, None, None
    fundo = max(contagem, key=contagem.get)
    frente = max(contagem, key=lambda p: contraste(p, fundo))
    return contraste(frente, fundo), fundo, frente


def hx(c):
    return "#%02X%02X%02X" % tuple(c) if c else "?"


# ---- o programa ----

def nova(modelo, tema_nome=None):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-72-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    if tema_nome:
        chave, valor = tema.TEMA_NO_BANCO[tema_nome]
        tema.gravar_tema(pasta, chave, valor)
    return pasta


def montar_geometria(pasta):
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--test", "lojas_de_teste", "--",
         "--ignored", "--exact", "montar_geometria"],
        cwd=base.RAIZ, env=dict(os.environ, RUSTINHO_LOJA_TESTE=pasta), stdout=subprocess.DEVNULL, check=True,
    )


def toques(textos):
    lista = []
    for _, _, t in textos:
        if t.startswith("TOQUE:"):
            p = t[len("TOQUE:"):].split("|")
            if len(p) >= 6:
                x, y, w, h = (float(v) for v in p[4].split(","))
                lista.append((p[0], p[1], p[2], p[3], x, y, w, h))
    return lista


def rets_completos(textos):
    """[(x, y, w, h, cw, ch, componente, texto)] das linhas RET:."""
    lista = []
    for _, _, t in textos:
        if t.startswith("RET:"):
            numeros, _, comp, texto = t[4:].split("|", 3)
            v = [float(n) for n in numeros.split(",")]
            lista.append((v[0], v[1], v[2], v[3], v[4], v[5], comp, texto))
    return lista


def abrir(pasta, tela, props, fotos, nome, celular=False, tamanho=None, usuario="Ana"):
    rr = u.Rustinho(pasta, tela, props, fotos, nome, celular=celular)
    time.sleep(1.5)
    if tamanho:
        rr.redimensionar(*tamanho)
        time.sleep(1)
    rr.entrar(usuario)
    time.sleep(2.5)
    return rr


def foto_e_ponto(rr, sufixo=""):
    """A foto da tela inteira e a função que leva um ponto da janela para a foto."""
    foto = rr.foto(sufixo)
    wx, wy, _, _ = tema.janela_do_rustinho()
    return foto, (lambda x, y: (wx + x, wy + y))


def por_carrinho_uma_peca(rr):
    """Vender: digita o código 1 (Filtro de óleo) na busca e Enter."""
    base.xdo("type", "--delay", "80", "1")
    time.sleep(0.6)
    base.xdo("key", "Return")
    time.sleep(2)


def receber_no_crediario(rr):
    """Abre o Receber (F2), esvazia o dinheiro e põe tudo no crediário."""
    base.xdo("key", "F2")
    time.sleep(2)
    base.xdo("key", "ctrl+a")
    base.xdo("key", "BackSpace")
    time.sleep(0.6)
    return rr.clicar("Crediário")


def concluir_dentro(textos, altura):
    c = [r for r in toques(textos) if r[0] == "botao" and r[3].startswith("Concluir venda")]
    if not c:
        return None, "Concluir venda não achado"
    x, y, w, h = c[-1][4:]
    return (y >= 0 and y + h <= altura + 0.5), f"Concluir venda em y={y:.0f}..{y + h:.0f} (janela {altura})"


def main():
    for p in ("Xvfb", "xdotool", "import", "cargo"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(u.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({u.PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-72-")
    os.makedirs(fotos, exist_ok=True)
    so = [c for c in os.environ.get("RUSTINHO_72_SO", "").split(",") if c]
    est = tempfile.mkdtemp(prefix="rustinho-loja-72-est-")
    geo = tempfile.mkdtemp(prefix="rustinho-loja-72-geo-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1920x1080x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    resultados = {}
    logs = []

    def conferir(codigo, nome, certo, detalhe=""):
        resultados[codigo] = (bool(certo), nome, detalhe)

    def quer(*codigos):
        return not so or any(c in so for c in codigos)

    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(est)
        montar_geometria(geo)
        c = sqlite3.connect(os.path.join(est, "rustinho.db"))
        filtro = c.execute("SELECT id FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0]
        c.close()

        # ---- M1 e M2: Etiquetas no AMOLED (1920 × 1080: a prévia à vista) ----
        if quer("M1", "M2"):
            rr = abrir(nova(est, "amoled"), "TelaEtiquetas", {"produtoId": filtro}, fotos, "m1_m2_etiquetas_amoled",
                       tamanho=(1920, 1080))
            logs.append(rr.saida)
            try:
                t = rr.textos()
                foto, pt = foto_e_ponto(rr)
                rs = rets_completos(t)
                preco = [r for r in rs if r[6] == "Faixa" and r[7].startswith("R$ ")]
                abaixo = [r for r in rs if preco and r[6] == "Faixa" and r[1] > preco[-1][1] + 20 and r[3] < 30]
                if abaixo:
                    r = abaixo[-1]
                    x, y = pt(r[0] + (r[2] - min(r[2], r[4])) / 2, r[1])
                    razao, fundo, frente = medir_contraste(foto, x, y, min(r[2], r[4]), min(r[3], r[5]), folga=1)
                    conferir("M1", "Etiquetas (AMOLED): o número da prévia tem contraste >= 4,5:1", razao >= 4.5,
                             f"\"{r[7]}\": {razao:.2f}:1 (frente {hx(frente)} sobre {hx(fundo)})")
                else:
                    conferir("M1", "Etiquetas (AMOLED): NÃO MEDIDO (a prévia não apareceu)", False, "")
                    ESPERADAS.pop("M1", None)
                caixa = [r for r in toques(t) if r[0] == "botao" and r[1] == "Marcar" and r[3] == "Nome da loja"]
                ret = [r for r in rs if r[6] == "Marcar" and r[7] == "Nome da loja"]
                if caixa and ret:
                    _, _, _, _, bx, by, bw, bh = caixa[0]
                    ix, iy = pt(ret[0][0], by + (bh - 22) / 2)
                    razao, fundo, frente = medir_contraste(foto, ix, iy, 22, 22, folga=4)
                    conferir("M2", "Etiquetas (AMOLED): a caixa desmarcada \"Nome da loja\" tem contraste >= 3:1",
                             razao >= 3.0, f"{razao:.2f}:1 (borda/caixa {hx(frente)} sobre {hx(fundo)})")
                else:
                    conferir("M2", "Etiquetas (AMOLED): NÃO MEDIDO (a caixa \"Nome da loja\" não apareceu)", False, "")
                    ESPERADAS.pop("M2", None)
            finally:
                rr.fechar()

        # ---- M3: rótulos dos cartões no celular ----
        if quer("M3"):
            rr = abrir(nova(geo, "amoled"), "TelaPainel", {"secao": "resumo"}, fotos, "m3_painel_celular", celular=True)
            logs.append(rr.saida)
            try:
                rr.foto("")
                rs = rets_completos(rr.textos())
                ruins, medidos = [], 0
                for r in rs:
                    if r[6] != "Rotulo":
                        continue
                    valores = [v for v in rs if v[6] == "CartaoNumero" and abs(v[0] - r[0]) <= 1 and r[1] < v[1] <= r[1] + 50]
                    if not valores:
                        continue
                    medidos += 1
                    v = valores[0]
                    direita = v[0] + v[2]
                    if r[0] + r[2] > direita + 1 or direita > CELULAR[0] - 14 + 1:
                        ruins.append((r[7], round(r[0] + r[2]), round(direita)))
                conferir("M3", "Painel no celular: nenhum rótulo alarga o cartão nem passa da margem",
                         medidos >= 6 and not ruins, f"{medidos} cartões medidos; com problema (rótulo, fim do rótulo, fim do cartão): {ruins}")
                if medidos < 6:
                    ESPERADAS.pop("M3", None)
            finally:
                rr.fechar()

        # ---- M4: estado vazio do A pagar no celular ----
        if quer("M4"):
            rr = abrir(nova(geo, "roxo"), "TelaFinanceiro", {"secao": "pagar"}, fotos, "m4_pagar_celular", celular=True)
            logs.append(rr.saida)
            try:
                rr.foto("")
                t = rr.textos()
                titulo = [r for r in rets_completos(t) if r[6] == "Titulo" and r[7] == "Nenhuma conta em aberto"]
                cat = [r for r in toques(t) if r[3] == "Categorias"]
                if titulo and cat:
                    fim_cat = max(r[5] + r[7] for r in cat)
                    ti = titulo[0]
                    ok = ti[1] >= fim_cat - 0.5 and ti[1] + min(ti[3], ti[5]) <= CELULAR[1] + 0.5
                    conferir("M4", "A pagar no celular: o estado vazio fica abaixo dos filtros e dentro da tela", ok,
                             f"título em y={ti[1]:.0f}..{ti[1] + min(ti[3], ti[5]):.0f}; \"Categorias\" termina em y={fim_cat:.0f}")
                else:
                    conferir("M4", "A pagar no celular: NÃO MEDIDO", False, f"título {bool(titulo)}, Categorias {bool(cat)}")
                    ESPERADAS.pop("M4", None)
            finally:
                rr.fechar()

        # ---- M5 e M6: Vender em 1366 × 768 ----
        if quer("M5", "M6"):
            rr = abrir(nova(est, "roxo"), "TelaVender", {}, fotos, "m5_m6_vender_1366", tamanho=(1366, 768))
            logs.append(rr.saida)
            try:
                por_carrinho_uma_peca(rr)
                rr.foto("-carrinho")
                t = rr.textos()
                salvar = [r for r in toques(t) if r[0] == "botao" and r[3].startswith("Salvar como or")]
                no_carrinho = any("Filtro de óleo" in s for _, _, s in t if not s.startswith(("RET:", "TOQUE:", "GEO:")))
                if salvar and no_carrinho:
                    x, y, w, h = salvar[0][4:]
                    conferir("M5", "Vender 1366 × 768: \"Salvar como orçamento\" inteiro dentro da janela",
                             y + h <= 768 + 0.5, f"botão em y={y:.0f}..{y + h:.0f} (janela 768)")
                else:
                    conferir("M5", "Vender 1366 × 768: NÃO MEDIDO", False, f"botão {bool(salvar)}, peça no carrinho {no_carrinho}")
                    ESPERADAS.pop("M5", None)
                if receber_no_crediario(rr):
                    time.sleep(1.5)
                    rr.foto("-receber-crediario")
                    t = rr.textos()
                    cred = any(s == "Crediário" or s.upper() == "CREDIÁRIO" for _, _, s in t) and \
                        any("1º vencimento" in s or "1º VENCIMENTO" in s.upper() for _, _, s in t)
                    ok, detalhe = concluir_dentro(t, 768)
                    if ok is None or not cred:
                        conferir("M6", "Receber 1366 × 768: NÃO MEDIDO", False, f"{detalhe}; crediário aberto {cred}")
                        ESPERADAS.pop("M6", None)
                    else:
                        conferir("M6", "Receber com crediário em 1366 × 768: \"Concluir venda\" dentro da janela", ok, detalhe)
                else:
                    conferir("M6", "Receber 1366 × 768: NÃO MEDIDO (o botão Crediário não apareceu)", False, "")
                    ESPERADAS.pop("M6", None)
            finally:
                rr.fechar()

        # ---- M7: Receber com crediário no celular ----
        if quer("M7"):
            rr = abrir(nova(est, "roxo"), "TelaVender", {}, fotos, "m7_receber_celular", celular=True)
            logs.append(rr.saida)
            try:
                por_carrinho_uma_peca(rr)
                rr.foto("-carrinho")
                if receber_no_crediario(rr):
                    time.sleep(1.5)
                    rr.foto("-receber-crediario")
                    t = rr.textos()
                    ok, detalhe = concluir_dentro(t, CELULAR[1])
                    if ok is None:
                        conferir("M7", "Receber no celular: NÃO MEDIDO", False, detalhe)
                        ESPERADAS.pop("M7", None)
                    else:
                        conferir("M7", "Receber com crediário no celular: \"Concluir venda\" dentro da tela", ok, detalhe)
                else:
                    conferir("M7", "Receber no celular: NÃO MEDIDO (o botão Crediário não apareceu)", False, "")
                    ESPERADAS.pop("M7", None)
            finally:
                rr.fechar()

        # ---- M8: pendências XML no roxo ----
        if quer("M8"):
            pasta = nova(est, "roxo")
            c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
            forn = c.execute("SELECT id FROM pessoa WHERE documento = '11222333000181'").fetchone()
            for i, desc in ((1, "PENDENCIA DE TESTE A"), (2, "PENDENCIA DE TESTE B")):
                c.execute(
                    "INSERT INTO xml_pendencia_identificacao (id, fornecedor_id, fornecedor_documento, fornecedor_nome,"
                    " numero_nfe, emissao_nfe, codigo_fornecedor, codigo_compacto, descricao, descricao_compacta, unidade,"
                    " motivo, situacao, criado_em) VALUES (?, ?, '11222333000181', 'FORNECEDOR DE TESTE', '1',"
                    " '2026-11-01', ?, ?, ?, ?, 'PC', 'sem_vinculo', 'pendente', ?)",
                    (f"m8-{i}", forn[0] if forn else None, f"COD-{i}", f"cod{i}", desc, desc.lower().replace(" ", ""),
                     f"2026-11-1{i}T00:00:00Z"))
            c.commit()
            c.close()
            rr = abrir(pasta, "TelaPecas", {"abrirPendenciasXml": True}, fotos, "m8_pendencias_roxo")
            logs.append(rr.saida)
            try:
                time.sleep(1)
                t = rr.textos()
                foto, pt = foto_e_ponto(rr)
                rs = rets_completos(t)
                medidas = []
                for d in ("PENDENCIA DE TESTE A", "PENDENCIA DE TESTE B"):
                    desc = [r for r in rs if r[7] == d]
                    if not desc:
                        continue
                    linhas = [r for r in rs if r[6] == desc[0][6] and abs(r[0] - desc[0][0]) <= 1
                              and desc[0][1] + 10 < r[1] < desc[0][1] + 90 and r[7] and not r[7].startswith("Cód. forn.")]
                    if not linhas:
                        continue
                    m = linhas[-1]
                    x, y = pt(m[0], m[1])
                    razao, fundo, frente = medir_contraste(foto, x, y, min(m[2], m[4]), min(m[3], m[5]), folga=1)
                    medidas.append((d[-1], m[7][:30], round(razao, 2), hx(frente), hx(fundo)))
                ok = len(medidas) == 2 and all(m[2] >= 4.5 for m in medidas)
                conferir("M8", "Pendências no roxo: o motivo de cada cartão tem contraste >= 4,5:1", ok,
                         f"(cartão, motivo, razão, frente, fundo): {medidas}")
                if len(medidas) != 2:
                    ESPERADAS.pop("M8", None)
            finally:
                rr.fechar()
        # ---- M10: nome com "<" num texto formatado (a grade da peça) ----
        if quer("M10"):
            pasta = nova(est)
            c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
            c.execute("UPDATE produto SET grade = 'GRADE-M10', variacao = '<i>P</i>' WHERE id = ?", (filtro,))
            outra = c.execute("SELECT id FROM produto WHERE id <> ? AND tipo = 'produto' AND ativo = 1 LIMIT 1",
                              (filtro,)).fetchone()[0]
            c.execute("UPDATE produto SET grade = 'GRADE-M10', variacao = 'M' WHERE id = ?", (outra,))
            c.commit()
            c.close()
            rr = abrir(pasta, "TelaPeca", {"produtoId": filtro}, fotos, "m10_grade_escapada")
            logs.append(rr.saida)
            try:
                rr.foto("")
                rs = rets_completos(rr.textos())
                grade = [r for r in rs if "P</i>" in r[7] or "P&lt;/i&gt;" in r[7]]
                escapado = [r for r in grade if "&lt;i&gt;P&lt;/i&gt;" in r[7]]
                if grade:
                    conferir("M10", "Grade da peça: a variação \"<i>P</i>\" vai escapada para o texto formatado",
                             bool(escapado) and len(escapado) == len(grade), f"textos: {[r[7] for r in grade]}")
                else:
                    conferir("M10", "Grade da peça: NÃO MEDIDO (a grade não apareceu)", False, "")
                    ESPERADAS.pop("M10", None)
            finally:
                rr.fechar()
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(est, ignore_errors=True)
        shutil.rmtree(geo, ignore_errors=True)

    erros = []
    for caminho in logs:
        erros += [caminho + ": " + e for e in tema.erros_de_script(caminho)]
    conferir("M9", "nenhum TypeError/ReferenceError nos registros", not erros, "; ".join(erros[:3]))

    reprovado = False
    for codigo in sorted(resultados):
        certo, nome, detalhe = resultados[codigo]
        if certo and codigo in ESPERADAS:
            print(f"  ⚠ {codigo} {nome}: PASSOU, mas estava como falha esperada ({ESPERADAS[codigo]}) — tirar da lista")
            reprovado = True
        elif certo:
            print(f"  ✔ {codigo} {nome} · {detalhe}")
        elif codigo in ESPERADAS:
            print(f"  ✘ {codigo} {nome} — falha ESPERADA: {ESPERADAS[codigo]} · {detalhe}")
        else:
            print(f"  ✘ {codigo} {nome} — {detalhe}")
            reprovado = True
    print(f"\n{sum(1 for v in resultados.values() if v[0])} de {len(resultados)} certas; fotos em {fotos}")
    sys.exit(1 if reprovado else 0)


if __name__ == "__main__":
    main()
