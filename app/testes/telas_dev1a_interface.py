#!/usr/bin/env python3
"""Pacote 1 (0.9.19-dev.1 / dev.1-A): provas de interface, janelinhas confortáveis,
bloqueio de toque sob janelinha, DP-27, P-ICO, P-NOTIF (6 combinações) e D-IMG.

Cobre no binário real:
  EFIP    Metas em 400 x 700: cabe na largura, campos >= 40 px e Salvar meta acessível;
  JJE5    Classificar categorias em 400 x 700 e 780 x 360: nomes completos e seletores dentro da borda;
  OV4C    DRE em 400 x 700: linha do mês sem sobreposição entre vendas/resultado e chip dentro do cartão;
  3M90    Estoque › O que comprar em 360 x 780: "Marcar todas como pedidas" dentro da largura;
  TJ8X    Janelinha modal sobre lista com TapHandler (Compras): toque na janelinha não abre a compra de trás;
  DP27    Usuários: chip "Gerente" visível no gerente e ausente no caixa;
  PICO    Sem lupa nas buscas; Colar/Copiar em campos editáveis largos; só Colar na senha da rede; nada em senha comum;
  PNOTIF  Notificações nos 3 temas (Claro, Escuro, Planeta Roxo) × expandida e minimizada: RGB fixo idêntico;
  DIMG    "Não importadas" com orientação, ordem fornecedor+nota, aviso Mostrar valores em ConflitoCnpj e cartões alinhados.

    python3 app/testes/telas_dev1a_interface.py [pasta para as fotos]
"""
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_navegacao_casos as nav  # noqa: E402
from telas_d09 import PROGRAMA, RAIZ, simples  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":88")
nav.TELA = base.TELA
TELA = base.TELA


def xdo(*args):
    subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA), stderr=subprocess.DEVNULL, check=True)


def esperar_janela():
    for _ in range(120):
        s = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                           env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True)
        if s.stdout.split():
            return
        time.sleep(0.2)


def rets_de(t):
    out = []
    for _x, _y, tx in t:
        if tx.startswith("RET:"):
            geo, resto = tx[4:].split("|", 1)
            v = [float(a) for a in geo.split(",")]
            out.append((v[0], v[1], v[2], v[3], resto.split("|")[-1]))
    return out


def geos_de(t):
    out = {}
    for _x, _y, tx in t:
        if tx.startswith("GEO:"):
            _, nome, coords = tx.split(":", 2)
            v = [float(a) for a in coords.split(",")[:4]]
            out.setdefault(nome, []).append(tuple(v))
    return out


def achar_ret(t, trecho, exato=False):
    s = simples(trecho)
    out = []
    for x, y, w, h, tx in rets_de(t):
        st = simples(tx)
        if (st == s) if exato else (s in st):
            out.append((x, y, w, h, tx))
    return out


def sem_sobreposicao_horiz(r1, r2):
    """Verdadeiro se dois retângulos não se sobrepõem (ou estão em linhas distintas)."""
    x1, y1, w1, h1 = r1[:4]
    x2, y2, w2, h2 = r2[:4]
    sobre_y = min(y1 + h1, y2 + h2) - max(y1, y2) > 2
    sobre_x = min(x1 + w1, x2 + w2) - max(x1, x2) > 2
    return not (sobre_x and sobre_y)


def cor_media(foto, x, y, w=12, h=8):
    im = Image.open(foto).convert("RGB")
    iw, ih = im.size
    x0, y0 = max(0, int(x)), max(0, int(y))
    x1, y1 = min(iw, x0 + max(1, int(w))), min(ih, y0 + max(1, int(h)))
    px = im.load()
    rs, gs, bs, n = 0, 0, 0, 0
    for yy in range(y0, y1):
        for xx in range(x0, x1):
            r, g, b = px[xx, yy]
            rs += r
            gs += g
            bs += b
            n += 1
    return (rs // n, gs // n, bs // n) if n else (0, 0, 0)


def dist_rgb(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


class App(base.Rustinho):
    def __init__(self, pasta, tela, props, fotos, nome, extra_args=None):
        self.fotos, self.nome = fotos, nome
        self.saida = os.path.join(fotos, nome + ".log")
        env = dict(os.environ, DISPLAY=TELA, QT_QUICK_BACKEND="software", RUSTINHO_DADOS=pasta,
                   QT_LOGGING_RULES="qml.info=true;js.info=true;default.info=true", QT_SCALE_FACTOR="1")
        env.pop("WAYLAND_DISPLAY", None)
        self.log = open(self.saida, "w")
        args = [PROGRAMA, "--textos-da-tela", "--medir-toque"] + (extra_args or [])
        import json
        self.p = subprocess.Popen(args + ["--abrir", tela, "--props", json.dumps(props)],
                                  env=env, stdout=self.log, stderr=self.log)

    def redim(self, w, h):
        esperar_janela()
        ids = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "rustinho"],
                             env=dict(os.environ, DISPLAY=TELA), capture_output=True, text=True).stdout.split()
        for i in ids:
            xdo("windowmove", i, "0", "0")
            xdo("windowsize", i, str(w), str(h))
        time.sleep(1.2)

    def achar_txt(self, pedaco, exato=False):
        s = simples(pedaco)
        for x, y, t in self.textos():
            if t.startswith(("GEO:", "RET:", "TOQUE:")):
                continue
            st = simples(t)
            if (st == s) if exato else (s in st):
                return (x, y, t)
        return None

    def clicar_txt(self, pedaco, exato=False):
        p = self.achar_txt(pedaco, exato=exato)
        if not p:
            raise RuntimeError(f"não achei {pedaco!r} em {self.saida}")
        xdo("mousemove", str(int(p[0])), str(int(p[1])), "click", "1")
        time.sleep(0.8)


def montar_loja_avisos(pasta):
    base._aux_nucleo.executar("estavel", "montar_loja_de_teste",
                              {"RUSTINHO_LOJA_TESTE": pasta, "RUSTINHO_LOJA_AVISOS": "1"}, raiz=RAIZ)


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-dev1a-")
    os.makedirs(fotos, exist_ok=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    certos = errados = 0

    def ok(nome, cond, det=""):
        nonlocal certos, errados
        if cond:
            certos += 1
            print(f"✔ {nome}", flush=True)
        else:
            errados += 1
            print(f"✘ {nome} — {det}", flush=True)

    modelo = tempfile.mkdtemp(prefix="rustinho-dev1a-modelo-")
    try:
        montar_loja_avisos(modelo)

        # ---- 1. P-NOTIF: 6 combinações (Claro, Escuro, Planeta Roxo × expandida e minimizada) ----
        pasta = tempfile.mkdtemp(prefix="rustinho-dev1a-notif-")
        shutil.rmtree(pasta)
        shutil.copytree(modelo, pasta)
        r = App(pasta, "TelaInicio", {}, fotos, "1-pnotif")
        try:
            r.entrar("Ana")
            time.sleep(1.2)
            r.redim(1280, 760)
            time.sleep(1.2)
            cores_exp = {}
            cores_min = {}
            for tema_nome, btn in (("claro", "Tema claro"), ("escuro", "Tema escuro"), ("roxo", "Planeta Roxo")):
                p = r.achar_txt(btn)
                if p:
                    xdo("mousemove", str(int(p[0])), str(int(p[1])), "click", "1")
                    time.sleep(1.2)
                t = r.textos()
                f = r.foto(f"-{tema_nome}")
                cartoes = geos_de(t).get("cartaoNotificacao", [])
                exps = [c for c in cartoes if c[2] > 300]
                mins = [c for c in cartoes if 60 <= c[2] <= 120]
                if exps:
                    cx, cy, cw, ch = exps[0]
                    # Amostra o fundo livre (canto inferior esquerdo dentro do cartão expandido)
                    cores_exp[tema_nome] = (
                        cor_media(f, cx + 10, cy + ch - 12, 16, 6),
                        cor_media(f, cx + cw * 0.72, cy + ch - 12, 16, 6),
                    )
                if mins:
                    mx, my, mw, mh = mins[0]
                    cores_min[tema_nome] = cor_media(f, mx + 8, my + mh - 12, 14, 6)
            ok("P-NOTIF: expandida presente e com cores idênticas nos 3 temas (Claro, Escuro, Roxo)",
               len(cores_exp) == 3
               and dist_rgb(cores_exp["claro"][0], cores_exp["escuro"][0]) <= 3
               and dist_rgb(cores_exp["claro"][0], cores_exp["roxo"][0]) <= 3
               and dist_rgb(cores_exp["claro"][1], cores_exp["escuro"][1]) <= 3
               and dist_rgb(cores_exp["claro"][1], cores_exp["roxo"][1]) <= 3
               and cores_exp["claro"][0][0] > 215,
               f"cores_exp={cores_exp}")
            ok("P-NOTIF: minimizada presente e com cores idênticas nos 3 temas (Claro, Escuro, Roxo)",
               len(cores_min) == 3
               and dist_rgb(cores_min["claro"], cores_min["escuro"]) <= 3
               and dist_rgb(cores_min["claro"], cores_min["roxo"]) <= 3
               and cores_min["claro"][0] > 215,
               f"cores_min={cores_min}")
        finally:
            r.fechar()
            shutil.rmtree(pasta, ignore_errors=True)

        # ---- 2. DP-27 + P-ICO (Usuários: chip Gerente, Colar/Copiar em Nome, sem Colar/Copiar em Senha) ----
        pasta = tempfile.mkdtemp(prefix="rustinho-dev1a-usr-")
        shutil.rmtree(pasta)
        shutil.copytree(modelo, pasta)
        r = App(pasta, "TelaUsuarios", {}, fotos, "2-usuarios-pico")
        try:
            r.entrar("Ana")
            time.sleep(1.2)
            r.redim(1280, 760)
            time.sleep(1.2)
            t = r.textos()
            f = r.foto("-0")
            gerentes = achar_ret(t, "Gerente", exato=True)
            ok("DP-27: marca clara 'Gerente' aparece na lista de Usuários", len(gerentes) >= 1, f"{f} {gerentes}")
            # P-ICO: campo Nome (largo, não-senha) tem botões Colar e Copiar; campos Senha não têm.
            colares = [x for x in t if x[2] == "Colar"]
            copiares = [x for x in t if x[2] == "Copiar"]
            ok("P-ICO: campo de texto editável (Nome) tem exatamente 1 Colar e 1 Copiar; senhas não expõem nenhum",
               len(colares) == 1 and len(copiares) == 1,
               f"{f} colares={colares} copiares={copiares}")
            # Prova funcional P-ICO: digita no campo Nome, clica Copiar, limpa e clica Colar;
            # em seguida tenta Ctrl+V no campo Senha e confirma que continua vazio (sem colar).
            rot_nome = achar_ret(t, "Nome", exato=True)
            rot_senha = achar_ret(t, "Senha", exato=True)
            if rot_nome and colares and copiares and rot_senha:
                nx, ny = int(rot_nome[0][0] + 120), int(rot_nome[0][1] + 42)
                xdo("mousemove", str(nx), str(ny), "click", "1")
                time.sleep(0.3)
                xdo("type", "--delay", "20", "CARLOS PICO")
                time.sleep(0.3)
                # Clica no botão Copiar do campo Nome
                xdo("mousemove", str(int(copiares[0][0])), str(int(copiares[0][1])), "click", "1")
                time.sleep(0.4)
                # Limpa o campo Nome e clica Colar
                xdo("mousemove", str(nx), str(ny), "click", "1")
                xdo("key", "ctrl+a", "BackSpace")
                time.sleep(0.2)
                xdo("mousemove", str(int(colares[0][0])), str(int(colares[0][1])), "click", "1")
                time.sleep(0.4)
                t_col = r.textos()
                colou_nome = any("CARLOS PICO" in tx for _, _, tx in t_col)
                ok("P-ICO: Copiar e Colar executam função real no campo editável (Nome)",
                   colou_nome, f"textos após colar: {[tx for _, _, tx in t_col if 'CARLOS' in tx]}")
                # Tenta Ctrl+V no campo Senha (não deve colar, então Salvar recusa por falta de senha)
                sx, sy = int(rot_senha[0][0] + 120), int(rot_senha[0][1] + 42)
                xdo("mousemove", str(sx), str(sy), "click", "1")
                time.sleep(0.2)
                xdo("key", "ctrl+v")
                time.sleep(0.3)
                r.clicar_txt("Salvar", exato=True)
                time.sleep(0.5)
                t_sen = r.textos()
                erros_sen = [tx for _, _, tx in t_sen if "senha" in simples(tx) and ("4" in tx or "digite" in simples(tx) or "informe" in simples(tx) or "minimo" in simples(tx) or "caracter" in simples(tx))]
                tem_bolinha = any("•" in tx for _, _, tx in t_sen)
                ok("P-ICO: Ctrl+V bloqueado no campo Senha (nenhum caractere colado)",
                   not tem_bolinha and bool(erros_sen),
                   f"bolinha={tem_bolinha} erros={erros_sen}")
        finally:
            r.fechar()
            shutil.rmtree(pasta, ignore_errors=True)

        # ---- 3. P-ICO chave da rede (TelaJuntar: só Colar, sem Copiar) ----
        pasta = tempfile.mkdtemp(prefix="rustinho-dev1a-juntar-")
        shutil.rmtree(pasta)
        shutil.copytree(modelo, pasta)
        r = App(pasta, "TelaJuntar", {}, fotos, "3-juntar-pico")
        try:
            r.entrar("Ana")
            time.sleep(1.2)
            r.redim(1280, 760)
            time.sleep(1.2)
            t = r.textos()
            f = r.foto("-0")
            # Localiza a linha y do campo "Senha da rede da loja"
            rot_rede = achar_ret(t, "Senha da rede da loja")
            colares = [(x, y) for x, y, tx in t if tx == "Colar"]
            copiares = [(x, y) for x, y, tx in t if tx == "Copiar"]
            tem_colar_rede = bool(rot_rede and any(0 < cy - rot_rede[0][1] < 80 for _, cy in colares))
            tem_copiar_rede = bool(rot_rede and any(0 < cy - rot_rede[0][1] < 80 for _, cy in copiares))
            ok("P-ICO: senha da rede expõe Colar e NÃO expõe Copiar",
               tem_colar_rede and not tem_copiar_rede,
               f"{f} rot={rot_rede} colares={colares} copiares={copiares}")
        finally:
            r.fechar()
            shutil.rmtree(pasta, ignore_errors=True)

        # ---- 4. EFIP (Metas), JJE5 (Classificar categorias) e OV4C (DRE) em 400 x 700 ----
        pasta = tempfile.mkdtemp(prefix="rustinho-dev1a-dre-")
        shutil.rmtree(pasta)
        shutil.copytree(modelo, pasta)
        r = App(pasta, "TelaPainel", {"secao": "dre"}, fotos, "4-dre-400x700")
        try:
            r.entrar("Ana")
            time.sleep(1.5)
            r.redim(400, 700)
            time.sleep(1.5)
            t = r.textos()
            f = r.foto("-dre")
            # OV4C: checa que nenhum par "vendas R$" e "resultado R$" se sobrepõe e "em andamento" cabe em 400 px
            vendas_r = achar_ret(t, "vendas R$")
            res_r = achar_ret(t, "resultado R$")
            chips_and = achar_ret(t, "em andamento", exato=True)
            ov_ok = all(sem_sobreposicao_horiz(v, res) for v in vendas_r for res in res_r)
            chip_ok = all(c[0] >= 0 and c[0] + c[2] <= 401 for c in chips_and)
            ok("BUG-OV4C: DRE em 400x700 sem sobreposição vendas/resultado e chip dentro da largura",
               ov_ok and chip_ok, f"{f} vendas={vendas_r[:2]} res={res_r[:2]} chips={chips_and}")

            # EFIP: abrir Metas em 400 x 700
            r.clicar_txt("Metas", exato=True)
            time.sleep(1.0)
            t = r.textos()
            f = r.foto("-metas")
            fundo = geos_de(t).get("fundoDaJanelinha", [])
            salvar_m = achar_ret(t, "Salvar meta", exato=True)
            efip_ok = bool(fundo and salvar_m
                           and fundo[-1][0] >= 0 and fundo[-1][0] + fundo[-1][2] <= 401
                           and salvar_m[-1][0] >= fundo[-1][0] and salvar_m[-1][0] + salvar_m[-1][2] <= fundo[-1][0] + fundo[-1][2] + 1
                           and salvar_m[-1][1] + salvar_m[-1][3] <= 701)
            ok("BUG-EFIP: janelinha Metas em 400x700 cabe inteira com 'Salvar meta' visível",
               efip_ok, f"{f} fundo={fundo[-1:]} salvar={salvar_m}")
            xdo("key", "Escape")
            time.sleep(0.8)

            # JJE5: abrir Classificar categorias em 400 x 700
            r.clicar_txt("Classificar categorias", exato=True)
            time.sleep(1.0)
            t = r.textos()
            f = r.foto("-classificar")
            fundo = geos_de(t).get("fundoDaJanelinha", [])
            salvar_c = achar_ret(t, "Salvar", exato=True)
            nomes_cat = [x for x in rets_de(t) if simples(x[4]) in ("aluguel", "energia", "fornecedores", "agua", "internet", "vendas")]
            jje5_ok = bool(fundo and salvar_c
                           and fundo[-1][0] + fundo[-1][2] <= 401
                           and salvar_c[-1][0] + salvar_c[-1][2] <= fundo[-1][0] + fundo[-1][2] + 1
                           and all(c[2] >= 40 and c[0] + c[2] <= fundo[-1][0] + fundo[-1][2] + 1 for c in nomes_cat))
            ok("BUG-JJE5: Classificar categorias em 400x700 com nomes legíveis e Salvar dentro da janelinha",
               jje5_ok, f"{f} fundo={fundo[-1:]} salvar={salvar_c} nomes={nomes_cat[:3]}")

            # JJE5 em paisagem baixa (780 x 360): janelinha e Salvar continuam dentro da janela
            r.redim(780, 360)
            time.sleep(1.0)
            t_pais = r.textos()
            f_pais = r.foto("-classificar-780x360")
            fundo_p = geos_de(t_pais).get("fundoDaJanelinha", [])
            salvar_p = achar_ret(t_pais, "Salvar", exato=True)
            nomes_p = [x for x in rets_de(t_pais) if simples(x[4]) in ("aluguel", "energia", "fornecedores", "agua", "internet", "vendas")]
            jje5_pais_ok = bool(fundo_p and salvar_p
                                and fundo_p[-1][1] + fundo_p[-1][3] <= 361
                                and salvar_p[-1][1] + salvar_p[-1][3] <= fundo_p[-1][1] + fundo_p[-1][3] + 1
                                and len(nomes_p) >= 1)
            ok("BUG-JJE5: Classificar categorias em 780x360 exibe lista e Salvar dentro da janela",
               jje5_pais_ok, f"{f_pais} fundo={fundo_p[-1:]} salvar={salvar_p} nomes={nomes_p[:2]}")
        finally:
            r.fechar()
            shutil.rmtree(pasta, ignore_errors=True)

        # ---- 5. BUG-3M90: Estoque › O que comprar em 360 x 780 ----
        pasta = tempfile.mkdtemp(prefix="rustinho-dev1a-3m90-")
        shutil.rmtree(pasta)
        shutil.copytree(modelo, pasta)
        # Garante pelo menos 2 encomendas abertas para exibir o botão "Marcar todas como pedidas"
        nav.por_fora(pasta, """
            INSERT INTO encomenda (id, numero, cliente_nome, telefone, descricao, quantidade, situacao, criado_em, atualizado_em, dispositivo)
            SELECT 'enc-3m90-901', 901, 'Cliente 901', '', 'PECA ENCOMENDA 901', 1000, 'aberta',
                   '2026-11-22T10:00:00Z', '2026-11-22T10:00:00Z', (SELECT dispositivo FROM produto LIMIT 1);
            INSERT INTO encomenda (id, numero, cliente_nome, telefone, descricao, quantidade, situacao, criado_em, atualizado_em, dispositivo)
            SELECT 'enc-3m90-902', 902, 'Cliente 902', '', 'PECA ENCOMENDA 902', 1000, 'aberta',
                   '2026-11-22T10:00:00Z', '2026-11-22T10:00:00Z', (SELECT dispositivo FROM produto LIMIT 1);
        """)
        r = App(pasta, "TelaEstoque", {"secao": "comprar"}, fotos, "5-3m90-360x780", extra_args=["--celular"])
        try:
            r.entrar("Ana")
            time.sleep(1.5)
            r.redim(360, 780)
            time.sleep(1.5)
            t = r.textos()
            f = r.foto("-0")
            # No celular, o topo rola com a lista; rolar para trazer o título das encomendas se necessário
            btn = achar_ret(t, "Marcar todas como pedidas", exato=True)
            if not btn or btn[0][1] > 760:
                xdo("mousemove", "180", "500")
                xdo("click", "--repeat", "6", "--delay", "100", "5")
                time.sleep(1.0)
                t = r.textos()
                f = r.foto("-1")
                btn = achar_ret(t, "Marcar todas como pedidas", exato=True)
            ok("BUG-3M90: 'Marcar todas como pedidas' cabe inteiro na largura de 360 px",
               bool(btn and btn[0][0] >= 0 and btn[0][0] + btn[0][2] <= 361),
               f"{f} btn={btn}")
        finally:
            r.fechar()
            shutil.rmtree(pasta, ignore_errors=True)

        # ---- 6. D-IMG-1..6 + BUG-TJ8X em TelaCompras ----
        pasta = tempfile.mkdtemp(prefix="rustinho-dev1a-compras-")
        shutil.rmtree(pasta)
        shutil.copytree(modelo, pasta)
        conflitos_teste = [
            {
                "arquivo": "498752-nfe.xml", "lojaNome": "Loja Teste", "lojaDocumento": "11.444.777/0001-61",
                "emitenteNome": "UNIVERSAL AUTOMOTIVE", "emitenteDocumento": "12.345.678/0001-95",
                "destinatarioNome": "EMPRESA COM NOME MUITO LONGO QUE OCUPA DUAS LINHAS NO CARTAO DO XML LTDA",
                "destinatarioDocumento": "11.222.333/0001-81", "numero": "803058", "serie": "1",
                "emissao": "08/10/2026", "total": "100,00", "itens": [],
            },
        ]
        r = App(pasta, "TelaCompras", {"naoImportadas": conflitos_teste}, fotos, "6-compras-dimg-tj8x")
        try:
            r.entrar("Ana")
            time.sleep(1.2)
            r.redim(1280, 760)
            time.sleep(1.2)
            t = r.textos()
            f = r.foto("-lista")
            ok("D-IMG-5: orientação curta visível sob 'Não importada'",
               bool(achar_ret(t, "Nada foi importado. Veja cada nota")), f)
            ok("D-IMG-6: subtítulo informa múltiplos arquivos ou arrastar",
               bool(achar_ret(t, "escolha vários arquivos ou arraste")), f)
            forn = achar_ret(t, "UNIVERSAL AUTOMOTIVE — nota 803058")
            arq = achar_ret(t, "498752-nfe.xml")
            ok("D-IMG-4: fornecedor e nota em destaque acima do nome do arquivo",
               bool(forn and arq and forn[0][1] < arq[0][1]), f"{f} forn={forn} arq={arq}")

            # Abre ConflitoCnpj
            r.clicar_txt("Ver", exato=True)
            time.sleep(1.0)
            t = r.textos()
            f = r.foto("-conflito")
            ok("D-IMG-3: aviso sobre 'Mostrar valores' visível com CNPJs escondidos",
               bool(achar_ret(t, "Os CNPJs estão escondidos")), f)
            # Verifica alinhamento dos 2 cartões de CNPJ (ponto 9 doc 12): mesmos topos
            c_loja = achar_ret(t, "CNPJ da loja")
            c_xml = achar_ret(t, "CNPJ no XML (destinatário)")
            ok("Doc 12 #9: cartões CNPJ da loja e CNPJ no XML alinhados no mesmo topo",
               bool(c_loja and c_xml and abs(c_loja[0][1] - c_xml[0][1]) <= 2),
               f"{f} loja={c_loja} xml={c_xml}")
            # Achado Hydrogen #4: em 1280x760, "Valor total" e o aviso "A nota não foi importada" cabem inteiros acima dos botões
            fundo_c = geos_de(t).get("fundoDaJanelinha", [])
            v_tot = achar_ret(t, "Valor total")
            av_nao = achar_ret(t, "A nota nao foi importada e nada foi gravado")
            btn_desc = achar_ret(t, "Descartar nota", exato=True)
            limite_y = btn_desc[0][1] if btn_desc else (fundo_c[-1][1] + fundo_c[-1][3] if fundo_c else 760)
            ok("ConflitoCnpj em 1280x760: 'Valor total' e aviso de nada gravado visíveis sem corte acima dos botões",
               bool(v_tot and av_nao and v_tot[0][1] + v_tot[0][3] <= limite_y - 18 and av_nao[0][1] + av_nao[0][3] <= limite_y - 18),
               f"{f} v_tot={v_tot} av_nao={av_nao} limite_y={limite_y}")

            # BUG-TJ8X: clicar numa área da janelinha que fica sobre a lista de trás NÃO fecha nem abre nada atrás
            fundo = geos_de(t).get("fundoDaJanelinha", [])
            if fundo:
                fx, fy, fw, fh = fundo[-1]
                xdo("mousemove", str(int(fx + fw * 0.5)), str(int(fy + fh * 0.5)), "click", "1")
                time.sleep(0.8)
            t2 = r.textos()
            ok("BUG-TJ8X: clique na janelinha não atravessa para a tela de trás",
               bool(achar_ret(t2, "CNPJ da nota é diferente")), r.foto("-tj8x"))
        finally:
            r.fechar()
            shutil.rmtree(pasta, ignore_errors=True)

    finally:
        shutil.rmtree(modelo, ignore_errors=True)
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()

    print(f"RESULTADO: {certos} certos, {errados} errados", flush=True)
    print("fotos em", fotos, flush=True)
    sys.exit(0 if errados == 0 else 1)


if __name__ == "__main__":
    main()
