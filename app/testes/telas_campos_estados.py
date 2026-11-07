#!/usr/bin/env python3
"""Teste dos seis estados dos campos (0.9.18, ordem visual §6), nos três temas.

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros testes das telas, e mede as cores na tela (pixels da foto), com os
valores esperados lidos do próprio Tema.qml:

  E1. Normal: contorno fino e discreto (quase a cor do painel).
  E2. Preenchido: contorno mais forte que o normal.
  E3. Focado: contorno magenta (Tema.foco) e brilho magenta em volta.
  E4. Validado: código de barras com o dígito certo fica com contorno verde
      e a marca de conferido; com o dígito errado, não.
  E5. Inválido: o núcleo recusa o código de barras errado ao salvar; o campo
      fica com contorno vermelho, brilho vermelho e a mensagem embaixo.
  E6. Desabilitado: o lucro das tabelas, para quem não vê custos (Bia, caixa,
      em Configurações › Vendas), fica apagado perto do nome da tabela, que
      está habilitado.
  E7. Nenhum TypeError/ReferenceError nos registros.

    python3 app/testes/telas_campos_estados.py [pasta para as fotos]
    RUSTINHO_TELA=:76 RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_campos_estados.py

Precisa de: Xvfb, xdotool, ImageMagick (import) e Pillow. Sai com 0 se tudo
conferiu.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

os.environ.setdefault("RUSTINHO_TELA", ":76")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_unidades as u  # noqa: E402

base, tema = u.base, u.tema
TELA = u.TELA
TEMAS = [t for t in os.environ.get("RUSTINHO_TEMAS", "roxo,cinza,amoled").split(",") if t]
GTIN_CERTO, GTIN_ERRADO = "7891234567895", "7891234567890"


def pixel(foto, rr, x, y, r=0):
    return tema.media(foto, *rr.ponto(x, y), r=r)


def perto(c, alvo, tol=34):
    return all(abs(a - b) <= tol for a, b in zip(c, tema.rgb(alvo) if isinstance(alvo, str) else alvo))


def mistura(cor, fundo, a):
    c, f = tema.rgb(cor), fundo
    return tuple(round(c[i] * a + f[i] * (1 - a)) for i in range(3))


def dist(a, b):
    return sum(abs(x - y) for x, y in zip(a, b))


def medir(rr, nome_campo, sufixo):
    """(foto, borda, fora, painel, caixa): o meio do contorno de cima, 2 px
    fora dele (o brilho), o painel mais acima e o retângulo do campo."""
    g = u.geos(rr.textos())
    caixa = g.get(nome_campo + "Caixa")
    if not caixa:
        return None
    time.sleep(0.3)
    foto = rr.foto(sufixo)
    x, y, w, h = caixa
    # A linha de cima do contorno é a primeira do retângulo (o contorno de
    # 1 px é só ela; o de 2 px, ela e a seguinte).
    borda = pixel(foto, rr, x + w * 0.75, y + 0.2)
    fora = pixel(foto, rr, x + w * 0.75, y - 2)
    painel = pixel(foto, rr, x + w * 0.75, y - 9)
    return foto, borda, fora, painel, caixa, g


def ficha(modelo, fotos, res, nome_tema):
    cores = tema.cores_do_tema(nome_tema)
    pasta = tema.nova_loja(modelo, nome_tema)
    ids = {"FILTRO": None}
    import sqlite3
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    ids["FILTRO"] = c.execute("SELECT id FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0]
    c.close()
    rr = u.Rustinho(pasta, "TelaPeca", {"produtoId": ids["FILTRO"]}, fotos, f"{nome_tema}-ficha")
    campo = "campoCodigoBarras"
    try:
        rr.entrar("Ana")
        time.sleep(2)
        m = medir(rr, campo, "-1-normal")
        if not m:
            res.conferir(f"{nome_tema}: achei o código de barras", False, rr.saida)
            return
        foto, borda_n, fora_n, painel, caixa, g = m
        res.conferir(f"E1 {nome_tema} normal: contorno discreto", 3 <= dist(borda_n, painel) <= 160 and dist(fora_n, painel) <= 24,
                     f"borda {tema.hx(borda_n)} painel {tema.hx(painel)} fora {tema.hx(fora_n)}")
        # Focado: clica no campo.
        rr.clicar_em(caixa)
        time.sleep(0.6)
        _, borda_f, fora_f, painel_f, _, _ = medir(rr, campo, "-2-focado")
        brilho = mistura(cores["foco"], painel_f, 0.28)
        res.conferir(f"E3 {nome_tema} focado: contorno magenta {cores['foco']}", perto(borda_f, cores["foco"]),
                     tema.hx(borda_f))
        res.conferir(f"E3 {nome_tema} focado: brilho magenta em volta", perto(fora_f, brilho, 30) and dist(fora_f, painel_f) > 20,
                     f"fora {tema.hx(fora_f)}, esperado ≈ {tema.hx(brilho)} (painel {tema.hx(painel_f)})")
        # Preenchido: digita algo que não é código de barras completo e sai.
        base.xdo("type", "--delay", "40", "123")
        base.xdo("key", "Tab")
        time.sleep(0.6)
        _, borda_p, fora_p, painel_p, _, g = medir(rr, campo, "-3-preenchido")
        res.conferir(f"E2 {nome_tema} preenchido: contorno mais forte que o normal",
                     dist(borda_p, painel_p) > dist(borda_n, painel) + 10 and dist(fora_p, painel_p) <= 24
                     and campo + "Valido" not in g,
                     f"preenchido {tema.hx(borda_p)} × normal {tema.hx(borda_n)} (painel {tema.hx(painel_p)})")
        # Validado: o código de barras certo.
        rr.clicar_em(caixa)
        base.xdo("key", "ctrl+a")
        base.xdo("type", "--delay", "40", GTIN_CERTO)
        base.xdo("key", "Tab")
        time.sleep(0.6)
        _, borda_v, _, painel_v, _, g = medir(rr, campo, "-4-validado")
        verde = tema.cores_do_tema(nome_tema)["verdeEscuro"]
        res.conferir(f"E4 {nome_tema} validado: contorno verde {verde} e a marca de conferido",
                     perto(borda_v, verde) and campo + "Valido" in g, f"borda {tema.hx(borda_v)}")
        # O dígito errado não fica verde (até salvar, fica só preenchido).
        rr.clicar_em(caixa)
        base.xdo("key", "ctrl+a")
        base.xdo("type", "--delay", "40", GTIN_ERRADO)
        base.xdo("key", "Tab")
        time.sleep(0.6)
        _, borda_e, _, _, _, g = medir(rr, campo, "-5-digito-errado")
        res.conferir(f"E4 {nome_tema}: dígito errado não fica verde", not perto(borda_e, verde, 20) and campo + "Valido" not in g,
                     tema.hx(borda_e))
        # Inválido: o núcleo recusa ao salvar.
        base.xdo("key", "F2")
        time.sleep(2)
        _, borda_i, fora_i, painel_i, _, _ = medir(rr, campo, "-6-invalido")
        vermelho = cores["vermelho"]
        textos = [t for _, _, t in rr.textos()]
        res.conferir(f"E5 {nome_tema} inválido: contorno vermelho {vermelho}, brilho e mensagem",
                     perto(borda_i, vermelho) and dist(fora_i, painel_i) > 12
                     and any("Código de barras inválido" in t for t in textos),
                     f"borda {tema.hx(borda_i)} fora {tema.hx(fora_i)} painel {tema.hx(painel_i)}")
    finally:
        rr.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def desabilitado(modelo, fotos, res, nome_tema):
    pasta = tema.nova_loja(modelo, nome_tema)
    rr = u.Rustinho(pasta, "TelaConfiguracoes", {"secao": "vendas"}, fotos, f"{nome_tema}-desabilitado")
    try:
        rr.entrar("Bia")
        time.sleep(2)
        # Caixa: as configurações da loja pedem a senha do gerente para
        # consultar (D-10); depois, Vendas, com o lucro das tabelas desligado
        # para quem não vê custos.
        if any(t == "Liberar" for _, _, t in rr.textos()):
            base.xdo("type", "--delay", "60", base.SENHA["Ana"])
            base.xdo("key", "Return")
            time.sleep(1.5)
        rr.clicar("Vendas")
        time.sleep(1.5)
        g = u.geos(rr.textos())
        lucro, nome = g.get("campoLucroTabelaCaixa"), g.get("campoNomeTabelaCaixa")
        if not (lucro and nome):
            res.conferir(f"E6 {nome_tema}: achei os campos das tabelas", False, rr.saida)
            return
        foto = rr.foto("-tabelas")
        # Perto da borda esquerda, por dentro: longe do texto (o lucro é
        # número, alinhado à direita, com o "%" no fim).
        dentro_l = pixel(foto, rr, lucro[0] + 6, lucro[1] + lucro[3] / 2, r=1)
        dentro_n = pixel(foto, rr, nome[0] + nome[2] - 8, nome[1] + nome[3] / 2, r=1)
        painel = pixel(foto, rr, lucro[0] + lucro[2] / 2, lucro[1] - 9, r=1)
        res.conferir(f"E6 {nome_tema} desabilitado: o lucro (só o gerente) fica apagado, mais perto do painel",
                     dist(dentro_l, painel) + 10 < dist(dentro_n, painel),
                     f"lucro {tema.hx(dentro_l)} nome {tema.hx(dentro_n)} painel {tema.hx(painel)}; foto {foto}")
    finally:
        rr.fechar()
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(u.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({u.PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-campos-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-campos-modelo-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    res = tema.Resultado()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(modelo)
        for nome_tema in TEMAS:
            ficha(modelo, fotos, res, nome_tema)
            desabilitado(modelo, fotos, res, nome_tema)
        erros = [f for f in os.listdir(fotos) if f.endswith(".log")
                 and any(e in open(os.path.join(fotos, f), errors="replace").read() for e in ("TypeError", "ReferenceError"))]
        res.conferir("E7 nenhum TypeError/ReferenceError nos registros", not erros, ", ".join(erros))
    finally:
        xvfb.terminate()
        xvfb.wait(10)
        shutil.rmtree(modelo, ignore_errors=True)
    print(f"\n{res.ok} conferências certas, {len(res.falhas)} com falha. Fotos em {fotos}")
    for f in res.falhas:
        print("  ✘ " + f)
    sys.exit(0 if not res.falhas else 1)


if __name__ == "__main__":
    main()
