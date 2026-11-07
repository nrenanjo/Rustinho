#!/usr/bin/env python3
"""Teste do seletor de unidades de medida da peça (0.9.18, Seção 6 do padrão).

Abre o Rustinho de verdade numa tela virtual (Xvfb), no ambiente neutro dos
outros testes das telas (`telas_d09.ambiente_neutro`), pelo caminho de
verdade: Peças (a pílula: T da janela de 24 horas) → a peça → Unidade.
Confere:

  U1. A ordem na tela é a da regra, calculada aqui por um oráculo próprio
      (escrito a partir do texto do contrato, não do código do núcleo) no
      mesmo banco: 1ª linha (as 5 com mais peças ativas, sem serviços nem
      inativas), 2ª linha (6º ao 10º e, entrando pela direita, as até 5 mais
      vendidas nas últimas 24 horas, da maior para a menor) e as demais pela
      sigla; cada sigla uma vez só. O banco tem venda fora da janela, venda
      cancelada e venda devolvida inteira (critérios 1 a 18 vistos na tela).
  U2. Até 5 botões por linha no computador e 3 no celular (--celular); cada
      grupo começa numa linha nova (critério 19).
  U3. Descrição longa ("Metro quadrado quebrado") inteira, sem corte.
  U4. No roxo, a unidade atual da peça fica em magenta (#D123AB).
  U5. Tocar numa unidade escolhe e fecha (o campo fica com a sigla).
  U6. Sigla repetida ("kg") é recusada com a mensagem do contrato, e a KG
      continua como estava.
  U7. Nenhum TypeError/ReferenceError nos registros.
  U8. A janelinha cabe na tela: o "Criar e usar" aparece inteiro (no celular
      em pé, 360 × 780, a lista das unidades rola e o resto fica à vista).

    python3 app/testes/telas_unidades.py [pasta para as fotos]
    RUSTINHO_TELA=:72 RUSTINHO_PROGRAMA=/caminho/rustinho python3 app/testes/telas_unidades.py

Precisa de: Xvfb, xdotool, ImageMagick (import) e Pillow. Sai com 0 se tudo
conferiu.
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timedelta, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402  (importar já deixa o ambiente neutro)

base.TELA = os.environ.get("RUSTINHO_TELA", ":72")
import telas_tema_0918 as tema  # noqa: E402

tema.TELA = base.TELA
TELA = base.TELA
PROGRAMA = base.PROGRAMA
MAGENTA = "#D123AB"

# Peças e vendas de teste. As vendas ficam "agora − 1 h" (dentro da janela)
# ou "agora − 25 h" (fora), longe das bordas, qualquer que seja o T exato.
DADOS = """
INSERT INTO unidade (sigla, descricao, fracionada) VALUES ('MQQ', 'Metro quadrado quebrado', 1);
WITH n(i) AS (VALUES (1), (2), (3), (4), (5), (6))
INSERT INTO produto (id, codigo, nome, unidade, ativo, tipo, criado_em, atualizado_em, dispositivo)
  SELECT 'tu-' || u.s || '-' || n.i, 'TU' || u.s || n.i, 'Teste ' || u.s || ' ' || n.i, u.s, u.ativo, u.tipo,
         '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'teste'
    FROM n JOIN (SELECT 'PC' AS s, 6 AS q, 1 AS ativo, 'produto' AS tipo UNION ALL
                 SELECT 'CX', 5, 1, 'produto' UNION ALL
                 SELECT 'KG', 4, 1, 'produto' UNION ALL
                 SELECT 'LT', 3, 1, 'produto' UNION ALL
                 SELECT 'MT', 2, 1, 'produto' UNION ALL
                 SELECT 'PAR', 1, 1, 'produto' UNION ALL
                 SELECT 'MQQ', 1, 1, 'produto') u ON n.i <= u.q;
-- Não contam: KG inativas e KG de serviço (se contassem, KG passaria a PC).
INSERT INTO produto (id, codigo, nome, unidade, ativo, tipo, criado_em, atualizado_em, dispositivo)
  SELECT 'tu-x-' || i, 'TUX' || i, 'Teste fora ' || i, 'KG', i % 2, CASE i % 2 WHEN 1 THEN 'servico' ELSE 'produto' END,
         '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'teste'
    FROM (WITH RECURSIVE c(i) AS (VALUES (1) UNION ALL SELECT i + 1 FROM c WHERE i < 8) SELECT i FROM c);
INSERT INTO venda (id, tipo, numero, dispositivo, caixa_nome, situacao, subtotal, total, criado_em) VALUES
  ('tu-v1', 'venda', 9001, 'teste', 'Caixa', 'concluida', 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 hours')),
  ('tu-v2', 'venda', 9002, 'teste', 'Caixa', 'concluida', 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-25 hours')),
  ('tu-v3', 'venda', 9003, 'teste', 'Caixa', 'cancelada', 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-2 hours')),
  ('tu-v4', 'venda', 9004, 'teste', 'Caixa', 'concluida', 0, 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-3 hours'));
UPDATE venda SET cancelado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-90 minutes') WHERE id = 'tu-v3';
INSERT INTO venda_item (id, venda_id, ordem, descricao, unidade, quantidade, preco, total) VALUES
  ('tu-i1', 'tu-v1', 0, 'x', 'DZ', 3000, 0, 0),
  ('tu-i2', 'tu-v1', 1, 'x', 'RL', 1500, 0, 0),
  ('tu-i3', 'tu-v1', 2, 'x', 'PC', 2000, 0, 0),
  ('tu-i4', 'tu-v2', 0, 'x', 'GL', 100000, 0, 0),
  ('tu-i5', 'tu-v3', 0, 'x', 'FR', 50000, 0, 0),
  ('tu-i6', 'tu-v4', 0, 'x', 'BD', 5000, 0, 0);
INSERT INTO devolucao (id, numero, dispositivo, caixa_nome, venda_id, valor, criado_em) VALUES
  ('tu-d1', 9001, 'teste', 'Caixa', 'tu-v4', 0, strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-150 minutes'));
INSERT INTO devolucao_item (id, devolucao_id, venda_item_id, descricao, quantidade, valor_item, valor) VALUES
  ('tu-di1', 'tu-d1', 'tu-i6', 'x', 5000, 0, 0);
"""


def oraculo(pasta, t):
    """(1ª linha, 2ª linha, de baixo) pela regra do contrato, no banco da loja."""
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        siglas = [r[0] for r in c.execute("SELECT sigla FROM unidade")]
        conta = dict(c.execute(
            "SELECT unidade, COUNT(*) FROM produto WHERE ativo = 1 AND tipo <> 'servico' GROUP BY unidade"))
        fim = t.strftime("%Y-%m-%dT%H:%M:%SZ")
        inicio = (t - timedelta(hours=24)).strftime("%Y-%m-%dT%H:%M:%SZ")
        vendida = {}
        for item, unidade, q in c.execute(
                "SELECT vi.id, vi.unidade, vi.quantidade FROM venda v JOIN venda_item vi ON vi.venda_id = v.id "
                "WHERE v.tipo = 'venda' AND v.criado_em > ? AND v.criado_em <= ? AND v.situacao <> 'cancelada'",
                (inicio, fim)).fetchall():
            devolvido = c.execute(
                "SELECT COALESCE(SUM(di.quantidade), 0) FROM devolucao_item di JOIN devolucao d ON d.id = di.devolucao_id "
                "WHERE di.venda_item_id = ? AND d.cancelado_em IS NULL", (item,)).fetchone()[0]
            u = unidade.strip().upper()
            vendida[u] = vendida.get(u, 0) + max(0, q - devolvido)
    finally:
        c.close()
    ranking = sorted(siglas, key=lambda s: (-conta.get(s, 0), s))
    vendidas = sorted((s for s in siglas if vendida.get(s, 0) > 0), key=lambda s: (-vendida[s], s))[:5]
    resto = [s for s in ranking if s not in vendidas]
    linha1 = resto[:5]
    vagas = 5 - len(vendidas)
    linha2 = resto[5:5 + vagas] + vendidas
    de_baixo = sorted(resto[5 + vagas:])
    return linha1, linha2, de_baixo, vendida


class Rustinho(tema.Rustinho):
    """O Rustinho com --medir-toque (retângulos dos textos) e, no celular,
    --celular (o desenho do Android: Tema.celular)."""

    def __init__(self, pasta, tela, props, fotos, nome, celular=False):
        self.fotos = fotos
        self.nome = nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true")
        self.log = open(self.saida, "w")
        args = [PROGRAMA, "--textos-da-tela", "--medir-toque"] + (["--celular"] if celular else [])
        self.p = subprocess.Popen(args + ["--abrir", tela, "--props", json.dumps(props)],
                                  env=env, stdout=self.log, stderr=self.log)
        self.conferir_ambiente()
        if celular:
            # Em pé, do tamanho de um celular (como o roteiro do toque).
            for _ in range(150):
                s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                                   env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
                if s.stdout.split():
                    break
                time.sleep(0.2)
            time.sleep(0.5)
            self.redimensionar(360, 780)
            time.sleep(1)

    def ponto(self, x, y):
        """Da janela (pixels lógicos) para a tela virtual."""
        wx, wy, _, _ = tema.janela_do_rustinho()
        return round(wx + x), round(wy + y)

    def textos(self):
        # Sem gerenciador de janelas, o teclado vai para a janela embaixo do
        # ponteiro: antes do atalho, o ponteiro no canto de baixo da janela.
        wx, wy, w, h = tema.janela_do_rustinho()
        base.xdo("mousemove", str(wx + w - 4), str(wy + h - 4))
        time.sleep(0.15)
        return super().textos()

    def clicar(self, texto, comeca=False):
        for x, y, t in self.textos():
            if base.simples(t) == base.simples(texto) or (comeca and base.simples(t).startswith(base.simples(texto))):
                px, py = self.ponto(x, y)
                base.xdo("mousemove", str(px), str(py), "click", "1")
                return True
        return False

    def clicar_em(self, r):
        x, y, w, h = r
        px, py = self.ponto(x + w / 2, y + h / 2)
        base.xdo("mousemove", str(px), str(py), "click", "1")


def geos(textos):
    """{nome: (x, y, largura, altura)} das linhas GEO:."""
    d = {}
    for _, _, t in textos:
        if t.startswith("GEO:"):
            nome, numeros = t[4:].rsplit(":", 1)
            d[nome] = tuple(float(v) for v in numeros.split(","))
    return d


def rets(textos):
    """[(x, y, largura, altura, largura do texto, altura do texto, texto)] das linhas RET:."""
    lista = []
    for _, _, t in textos:
        if t.startswith("RET:"):
            numeros, _, _, texto = t[4:].split("|", 3)
            v = [float(x) for x in numeros.split(",")]
            lista.append((v[0], v[1], v[2], v[3], v[4], v[5], texto))
    return lista


def dentro(px, py, r):
    x, y, w, h = r
    return x <= px <= x + w and y <= py <= y + h


def grupos_na_tela(g):
    """{1: [[siglas da linha visual], ...], 2: ..., 3: ...} pelas posições."""
    botoes = {n[len("unidade_"):]: r for n, r in g.items() if n.startswith("unidade_")}
    saida = {}
    for k in (1, 2, 3):
        caixa = g.get(f"unidadesGrupo{k}")
        if not caixa:
            saida[k] = []
            continue
        meus = [(r[1], r[0], s) for s, r in botoes.items() if dentro(r[0] + r[2] / 2, r[1] + r[3] / 2, caixa)]
        meus.sort()
        linhas, ultima = [], None
        for y, _, s in meus:
            if ultima is None or abs(y - ultima) > 2:
                linhas.append([])
                ultima = y
            linhas[-1].append(s)
        saida[k] = linhas
    return saida


def abrir_seletor(rr, res, nome):
    """Na ficha da peça: toca no botão do campo Unidade e espera o seletor."""
    for _ in range(20):
        g = geos(rr.textos())
        if "campoUnidadeAcao" in g:
            _, y, _, h = g["campoUnidadeAcao"]
            _, _, jw, jh = tema.janela_do_rustinho()
            if y + h > jh - 8:
                # Fora da tela (no celular a ficha é comprida): rola a ficha.
                px, py = rr.ponto(jw / 2, jh / 2)
                base.xdo("mousemove", str(px), str(py), "click", "5", "click", "5", "click", "5")
                time.sleep(0.8)
                continue
            rr.clicar_em(g["campoUnidadeAcao"])
            break
        time.sleep(0.5)
    else:
        res.conferir(f"{nome}: a ficha da peça abriu com o campo Unidade", False, rr.saida)
        return None
    for _ in range(20):
        time.sleep(0.5)
        t = rr.textos()
        if any(n.startswith("unidade_") for n in geos(t)):
            return t
    res.conferir(f"{nome}: o seletor de unidades abriu", False, rr.saida)
    return None


def rodada(modelo, fotos, res, nome, tema_nome, celular):
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-unidades-")
    shutil.rmtree(pasta)
    shutil.copytree(modelo, pasta)
    chave, valor = tema.TEMA_NO_BANCO[tema_nome]
    tema.gravar_tema(pasta, chave, valor)
    t = datetime.now(timezone.utc).replace(microsecond=0)
    linha1, linha2, baixo, vendida = oraculo(pasta, t)
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    unidade_da_peca = c.execute("SELECT unidade FROM produto WHERE nome = 'Filtro de óleo'").fetchone()[0]
    c.close()
    print(f"# {nome}: oráculo 1ª {linha1} · 2ª {linha2} · vendidas {dict(sorted(vendida.items()))}")
    rr = Rustinho(pasta, "TelaPecas", {"buscaInicial": "Filtro de óleo"}, fotos, nome, celular)
    try:
        rr.entrar("Ana")
        time.sleep(1.5)
        if celular:
            # No celular a busca não pega o cursor sozinha (o teclado não
            # sobe): toca no cartão da peça, o "Filtro de óleo" mais embaixo.
            achados = [(y, x) for x, y, t in rr.textos() if base.simples(t) == base.simples("Filtro de óleo")]
            if achados:
                y, x = max(achados)
                base.xdo("mousemove", *map(str, rr.ponto(x, y)), "click", "1")
        else:
            base.xdo("key", "Return")   # abre a 1ª da busca (Filtro de óleo) pela pílula Peças
        time.sleep(2.5)
        textos = abrir_seletor(rr, res, nome)
        if textos is None:
            rr.foto("-falha")
            return
        time.sleep(0.5)
        foto = rr.foto("-seletor")
        g = geos(textos)
        tela = grupos_na_tela(g)
        planas = {k: [s for linha in v for s in linha] for k, v in tela.items()}
        res.conferir(f"{nome}: U1 1ª linha = {linha1}", planas[1] == linha1, f"na tela {planas[1]}; foto {foto}")
        res.conferir(f"{nome}: U1 2ª linha = {linha2}", planas[2] == linha2, f"na tela {planas[2]}")
        res.conferir(f"{nome}: U1 as demais pela sigla ({len(baixo)})", planas[3] == baixo,
                     f"na tela {planas[3][:12]}…")
        todas = planas[1] + planas[2] + planas[3]
        res.conferir(f"{nome}: U1 cada sigla uma vez só", len(todas) == len(set(todas)) == len(linha1 + linha2 + baixo),
                     f"{len(todas)} na tela")
        maximo = 3 if celular else 5
        larguras = [len(linha) for k in (1, 2, 3) for linha in tela[k]]
        res.conferir(f"{nome}: U2 até {maximo} por linha", larguras and max(larguras) == maximo
                     and all(n <= maximo for n in larguras), f"linhas {larguras}")
        esperado1 = [3, 2] if celular else [5]
        res.conferir(f"{nome}: U2 a 1ª linha lógica ocupa {esperado1}", [len(x) for x in tela[1]] == esperado1,
                     f"{[len(x) for x in tela[1]]}")
        # U8: a janelinha cabe na tela (o "Criar e usar" inteiro, mesmo no celular).
        _, _, jw, jh = tema.janela_do_rustinho()
        criar = g.get("criarUnidade")
        res.conferir(f"{nome}: U8 \"Criar e usar\" inteiro na tela", bool(criar) and criar[1] >= 0
                     and criar[1] + criar[3] <= jh and criar[0] + criar[2] <= jw, f"{criar} na janela {jw}×{jh}")
        # U3: a descrição longa inteira dentro do botão.
        longa = [r for r in rets(textos) if r[6] == "Metro quadrado quebrado"]
        botao = g.get("unidade_MQQ")
        certo = bool(longa) and bool(botao) and all(
            r[4] <= r[2] + 0.5 and r[5] <= r[3] + 0.5 and dentro(r[0], r[1], botao) and dentro(r[0] + r[2], r[1] + r[3], botao)
            for r in longa)
        res.conferir(f"{nome}: U3 \"Metro quadrado quebrado\" inteira, sem corte", certo, f"{longa} em {botao}")
        # U4: a unidade da peça em magenta (no roxo).
        if tema_nome == "roxo":
            r = g.get(f"unidade_{unidade_da_peca}")
            cor = tema.media(foto, *rr.ponto(r[0] + 5, r[1] + r[3] / 2)) if r else None
            res.conferir(f"{nome}: U4 a unidade da peça ({unidade_da_peca}) em magenta",
                         bool(cor) and tema.perto(cor, MAGENTA), tema.hx(cor) if cor else "botão não achado")
        # U5: tocar numa unidade escolhe e fecha.
        rr.clicar_em(g["unidade_MT"])
        time.sleep(1.2)
        depois = rr.textos()
        g2 = geos(depois)
        campo = g2.get("campoUnidade")
        no_campo = [tt for x, y, tt in depois if campo and dentro(x, y, campo) and not tt.startswith(("GEO:", "RET:", "TOQUE:"))]
        res.conferir(f"{nome}: U5 escolheu MT e fechou", not any(n.startswith("unidade_") for n in g2) and "MT" in no_campo,
                     f"no campo {no_campo}")
        # U6: sigla repetida recusada.
        if abrir_seletor(rr, res, nome) is not None:
            if rr.clicar("Ex.: QD"):
                time.sleep(0.4)
                base.xdo("type", "--delay", "60", "kg")
                base.xdo("key", "Return")
                time.sleep(0.4)
                base.xdo("type", "--delay", "40", "Outra coisa")
                base.xdo("key", "Return")
                time.sleep(1.2)
                msg = "Já existe uma unidade com a sigla 'KG'. Escolha outra sigla."
                tx = [tt for _, _, tt in rr.textos()]
                foto_erro = rr.foto("-sigla-repetida")
                res.conferir(f"{nome}: U6 \"kg\" recusada com a mensagem do contrato", msg in tx, f"foto {foto_erro}")
            else:
                res.conferir(f"{nome}: U6 campo Sigla achado", False, rr.saida)
    finally:
        rr.fechar()
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    kg = c.execute("SELECT descricao, fracionada FROM unidade WHERE sigla = 'KG'").fetchone()
    copias = c.execute("SELECT COUNT(*) FROM unidade WHERE UPPER(TRIM(sigla)) = 'KG'").fetchone()[0]
    c.close()
    res.conferir(f"{nome}: U6 a KG continua como estava", kg and kg[0] != "Outra coisa" and kg[1] == 1 and copias == 1,
                 f"{kg}, {copias} KG")
    shutil.rmtree(pasta, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    if TELA in (":0", ":1"):
        sys.exit(f"tela {TELA} recusada: use uma tela virtual (a :0 é a área de trabalho de verdade)")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-unidades-")
    os.makedirs(fotos, exist_ok=True)
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-unidades-modelo-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    res = tema.Resultado()
    try:
        if xvfb.poll() is not None:
            sys.exit(f"o Xvfb não abriu na tela {TELA} (ocupada?)")
        base.montar_loja(modelo)
        tema.por_fora(modelo, DADOS)
        for nome, tema_nome, celular in (("roxo-computador", "roxo", False), ("cinza-computador", "cinza", False),
                                         ("roxo-celular", "roxo", True)):
            rodada(modelo, fotos, res, nome, tema_nome, celular)
        erros = [f for f in os.listdir(fotos) if f.endswith(".log")
                 and any(e in open(os.path.join(fotos, f), errors="replace").read() for e in ("TypeError", "ReferenceError"))]
        res.conferir("U7 nenhum TypeError/ReferenceError nos registros", not erros, ", ".join(erros))
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
