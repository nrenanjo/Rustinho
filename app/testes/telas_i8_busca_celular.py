#!/usr/bin/env python3
"""I8 (dev3.3), A-1 da revisão independente do VIS2 (Fable, 16/11/2026; `revisao_independente/fable/
RELATORIO_REVISAO_VIS2_AVISOS.md`): no desenho do celular, o campo de busca das quatro listas do Estoque fica dentro da
lista (topoNaLista). A lista, quando recarrega (200 ms depois da última tecla), trocava o modelo e dava o foco à 1ª
linha: a busca perdia o foco e só 1 ou 2 letras entravam. Tirado da sonda do revisor (`i8_foco_busca.py`), com a loja
dele (linhas nas quatro listas) e conferências estritas.

Em cada lista (O que comprar, Encomendas, Mais vendidos, Parados), no desenho do celular (360 × 780):
  1. tocar no campo, digitar "02", esperar a lista recarregar, digitar "99": o campo tem "0299";
  2. limpar e digitar "0299" devagar (uma tecla a cada 350 ms, como no teclado do celular): o campo tem "0299".

    RUSTINHO_TELA=:88 python3 app/testes/telas_i8_busca_celular.py [pasta das fotos]
    RUSTINHO_PROGRAMA=/caminho/rustinho   (outro binário: o congelado, para o "antes")
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

os.environ["RUSTINHO_CASOS_CELULAR"] = "1"  # este roteiro é só do desenho do celular
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402

TELA = nav.TELA

# A loja com listas compridas no Estoque (a do revisor): 300 peças abaixo do mínimo, 150 com vendas, 60 encomendas.
SQL_CHEIA = """
CREATE TEMP TABLE s(n INTEGER PRIMARY KEY);
WITH RECURSIVE c(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM c WHERE n < 300) INSERT INTO s SELECT n FROM c;
CREATE TEMP TABLE tp AS SELECT p.* FROM produto p JOIN s WHERE p.nome = 'Pastilha de freio';
UPDATE tp SET id = 'sonda-' || rowid, codigo = 'SND-' || printf('%04d', rowid),
              nome = 'Peça da sonda ' || printf('%04d', rowid), estoque_minimo = 5000, codigo_barras = '',
              criado_em = datetime('now', '-400 days');
INSERT INTO produto SELECT * FROM tp;
CREATE TEMP TABLE tm AS SELECT m.* FROM estoque_movimento m JOIN s
 WHERE m.id = (SELECT id FROM estoque_movimento ORDER BY rowid LIMIT 1);
UPDATE tm SET id = 'sonda-m-' || rowid, produto_id = 'sonda-' || rowid, quantidade = 3000, tipo = 'inicial',
              criado_em = datetime('now', '-400 days');
INSERT INTO estoque_movimento SELECT * FROM tm;
CREATE TEMP TABLE tv AS SELECT vi.* FROM venda_item vi JOIN s
 WHERE s.n <= 150 AND vi.id = (SELECT vi2.id FROM venda_item vi2 JOIN venda v ON v.id = vi2.venda_id
                                WHERE v.tipo = 'venda' AND v.situacao = 'concluida' ORDER BY vi2.rowid LIMIT 1);
UPDATE tv SET id = 'sonda-vi-' || rowid, produto_id = 'sonda-' || rowid, ordem = 100 + rowid,
              descricao = 'Peça da sonda ' || printf('%04d', rowid), quantidade = 1000, total = 1000 + rowid * 10;
INSERT INTO venda_item SELECT * FROM tv;
INSERT INTO encomenda (id, numero, cliente_nome, telefone, descricao, quantidade, situacao, criado_em, atualizado_em, dispositivo)
SELECT 'sonda-e-' || n, 1000 + n, 'Cliente da sonda ' || printf('%03d', n), '', 'Encomenda da sonda ' || printf('%03d', n),
       1000, 'aberta', datetime('now'), datetime('now'), (SELECT dispositivo FROM produto LIMIT 1) FROM s WHERE n <= 60;
"""


def campo_da_busca(r):
    """(x, y) do centro do campo de busca da seção (a medida do toque do celular: o campo largo de cima)."""
    r.textos()
    campos = []
    for _, _, m in r._medidas:
        if m.startswith("TOQUE:campo|"):
            p = m[6:].split("|")
            x, y, w, h = (float(v) for v in p[4].split(","))
            campos.append((y, x, w, h))
    campos = [c for c in campos if c[0] > 200 and c[2] > 250]
    if not campos:
        return None
    y, x, w, h = sorted(campos)[0]
    return round(x + w / 2), round(y + h / 2)


def texto_no_campo(r, ponto):
    """O que está escrito no campo agora (a medida do toque do celular dá o texto do campo)."""
    r.textos()
    for _, _, m in r._medidas:
        if m.startswith("TOQUE:campo|"):
            p = m[6:].split("|")
            x, y, w, h = (float(v) for v in p[4].split(","))
            if x <= ponto[0] <= x + w and y <= ponto[1] <= y + h:
                return p[3]
    return "?"


def roteiro(placar, fotos):
    ok = placar.ok
    modelo = tempfile.mkdtemp(prefix="rustinho-loja-i8busca-")
    try:
        nav.montar_loja(modelo)
        nav.por_fora(modelo, SQL_CHEIA)
        for secao in ("comprar", "encomendas", "vendidos", "parados"):
            pasta = tempfile.mkdtemp(prefix="rustinho-loja-i8busca-")
            shutil.rmtree(pasta)
            shutil.copytree(modelo, pasta)
            r = placar.abrir(pasta, "TelaEstoque", {"secao": secao}, fotos, f"busca-{secao}")
            try:
                r.entrar("Ana")
                r.esperar(lambda: r.tem("Estoque / Fornecedores"), 15)
                time.sleep(2)
                ponto = campo_da_busca(r)
                if not ok(f"{secao}: achei o campo de busca", ponto is not None, r.foto("-0")):
                    continue
                # 1) "02", espera a lista recarregar (200 ms depois da tecla), "99".
                r.clicar_em(*ponto)
                time.sleep(0.6)
                nav.xdo("type", "--delay", "60", "02")
                time.sleep(1.0)
                nav.xdo("type", "--delay", "60", "99")
                time.sleep(1.0)
                b = texto_no_campo(r, ponto)
                ok(f"{secao}: \"02\", a lista recarrega, \"99\": o campo tem \"0299\"", b == "0299",
                   f"{r.foto('-1-pausa')} campo={b!r}")
                # 2) limpar e digitar devagar (350 ms por tecla: a lista recarrega entre as teclas).
                r.clicar_em(*ponto)
                time.sleep(0.4)
                nav.xdo("key", "ctrl+a")
                nav.xdo("key", "BackSpace")
                time.sleep(1.0)
                r.clicar_em(*ponto)
                time.sleep(0.4)
                nav.xdo("type", "--delay", "350", "0299")
                time.sleep(1.0)
                c = texto_no_campo(r, ponto)
                ok(f"{secao}: \"0299\" devagar (350 ms por tecla): o campo tem \"0299\"", c == "0299",
                   f"{r.foto('-2-devagar')} campo={c!r}")
            finally:
                r.fechar()
                shutil.rmtree(pasta, ignore_errors=True)
    finally:
        shutil.rmtree(modelo, ignore_errors=True)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(nav.PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({nav.PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-i8busca-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    print("BINÁRIO:", nav.PROGRAMA, flush=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = nav.Placar()
    try:
        try:
            roteiro(placar, fotos)
        except Exception as e:
            placar.ok(f"o roteiro chegou ao fim ({e!r})", False)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    print("fotos em", fotos)
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
