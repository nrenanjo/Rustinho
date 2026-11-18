#!/usr/bin/env python3
"""C8 (dev3.3): os casos da 2ª re-revisão independente (Fable, 15-16/11/2026; `revisao_independente/fable/
RELATORIO_REREVISAO2_C8.md`), levados para o projeto (achado S-5). O roteiro é o do revisor (`telas_fable_rerev2.py`);
aqui, S-2 e S-4 viram conferências estritas, para FALHAR na versão congelada (`congelado_rerevisao2_20261115/c8`) e
passar depois da correção.

  s1  duas (três) cadeias seguidas a partir da ficha do 10b: ficha + pergunta fecham e reabrem, na ordem, três vezes;
      a volta do 10b sobrevive; com guia, o véu continua cheio (I4).
  s2  a tela de destino sai antes de a espera da guia (N-2) acabar: volta, a janelinha reabre, nada sobra, sem erro.
  s3  compra: a 1ª oferta por um campo; a 2ª mudança só avisa (B-8); "Lançar no estoque" oferece de novo (N-5); a
      oferta sem janelinha traz o motivo (N-6).
  s4  aceitar OUTRO registro com a guia aberta: a guia continua e o destaque do registro dela volta (N-7).
  s5  Histórico: "Ver no histórico" sem evento do registro, com o filtro de texto valendo, diz o filtro (N-9).
  s6  I1 com três caixas: um aviso, o número uma vez, leva à encomenda de OUTRO caixa (pedida), com o véu leve (I4).
      S-4: na linha escolhida, nenhum texto fica por cima de outro (o programa abre com --medir-toque: as linhas RET:
      dão o retângulo de cada texto).
  s7  a sessão acaba no destino da guia, com a pergunta do aceite aberta; a pessoa seguinte faz o caso 7.
  s9  S-2: depois de apagar o filtro, o aviso do Histórico deixa de dizer "com o filtro".
  s8  (fora da lista padrão) só MEDE o S-3, o destaque da guia por cima das pílulas fixas com a página rolada. O S-3
      espera a decisão do proprietário (tratar junto com o H25); quando for corrigido, este caso vira estrito.

    RUSTINHO_CASOS_REREV2=s1,s3 python3 app/testes/telas_c8_rerevisao2.py [pasta das fotos]
    RUSTINHO_CASOS_BINARIO=/caminho/rustinho  (outro binário: a versão congelada, para o "antes")
"""
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_navegacao_casos as nav  # noqa: E402
import telas_c8_rerevisao as rr  # noqa: E402
from telas_navegacao_casos import (Placar, montar_loja, por_fora, ler, xdo, TITULO_7, PEDIDO, SENHA,  # noqa: E402
                                   AGORA, DISPOSITIVO, TELA)

BINARIO = os.environ.get("RUSTINHO_CASOS_BINARIO", "")
if BINARIO:
    nav.base.PROGRAMA = BINARIO
PROGRAMA = nav.base.PROGRAMA


def obs(*a):
    print("OBS", *a, flush=True)


def luz_fina(foto, x, y, largura=8, altura=8):
    """Luminosidade mediana (1 a 256) de um quadradinho da foto em volta de (x, y): a margem direita da janela."""
    from PIL import Image
    im = Image.open(foto).convert("L")
    px = sorted(im.crop((x - largura, y - altura, x + largura, y + altura)).getdata())
    return px[len(px) // 2] + 1


MARGEM = [(1262, 300), (1262, 450), (1262, 600)]


def razoes(com, sem):
    return [round(luz_fina(com, *p) / luz_fina(sem, *p), 2) for p in MARGEM]


def erros(r):
    return rr.erros_do_registro(r)


def textos_com(r, pedaco):
    return [(x, y, t) for x, y, t in r.textos() if nav.simples(pedaco) in nav.simples(t)]


def unicos(pontos, folga=4):
    """As posições sem repetição: o texto de um botão sai duas vezes nos textos da tela (a 0–1 px uma da outra)."""
    fica = []
    for p in sorted(pontos, key=lambda q: (q[1], q[0])):
        if not any(abs(p[0] - q[0]) <= folga and abs(p[1] - q[1]) <= folga for q in fica):
            fica.append(p)
    return fica


def clicar_perto(r, texto, referencia):
    """Clica no `texto` (igual) mais perto, na vertical, do texto que contém `referencia`."""
    ref = textos_com(r, referencia)
    alvos = r.onde(texto)
    if not ref or not alvos:
        return False
    y0 = ref[0][1]
    x, y = min(alvos, key=lambda p: abs(p[1] - y0))
    r.clicar_em(x, y)
    return True


class RustinhoComRetangulos(nav.Rustinho):
    """Como o dos roteiros, mais --medir-toque: as linhas RET: dão o retângulo de cada texto (S-4)."""
    def __init__(self, *a):
        popen = subprocess.Popen

        def com_medida(argv, *k, **kw):
            if argv and argv[0] == PROGRAMA and "--medir-toque" not in argv:
                argv = [argv[0], "--medir-toque", *argv[1:]]
            return popen(argv, *k, **kw)
        subprocess.Popen = com_medida
        try:
            super().__init__(*a)
        finally:
            subprocess.Popen = popen


def textos_pintados(r):
    """[(x0, y0, x1, y1, texto)] do que cada texto pinta de fato (largura do desenho e alinhamento), das linhas RET:."""
    fica = []
    for _, _, t in r.textos():
        if not t.startswith("RET:"):
            continue
        partes = t[4:].split("|")
        x, y, w, h, cw, ch = (float(v) for v in partes[0].split(",")[:6])
        alinh, texto = int(partes[1]), "|".join(partes[3:])
        pw, ph = min(cw, w), min(ch, h)
        x0 = x + (w - pw) if alinh & 2 else x + (w - pw) / 2 if alinh & 4 else x
        y0 = y + (h - ph) / 2
        fica.append((x0, y0, x0 + pw, y0 + ph, texto))
    return fica


def sobrepostos(textos, folga=1.0):
    """Os pares de textos diferentes que se cobrem (mais de `folga` px nas duas direções)."""
    pares = []
    for i, a in enumerate(textos):
        for b in textos[i + 1:]:
            if a[4] == b[4]:
                continue
            if min(a[2], b[2]) - max(a[0], b[0]) > folga and min(a[3], b[3]) - max(a[1], b[1]) > folga:
                pares.append((a[4], b[4], [round(v) for v in a[:4]], [round(v) for v in b[:4]]))
    return pares


REF = "INSERT INTO sync_referencia (tabela, chave, pai, falta, estado, desde, motivo) VALUES "


# ---------------------------------------------------------------------------------------------------------------
def s1_cadeias_seguidas(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s1-")
    try:
        montar_loja(pasta)
        por_fora(pasta, rr.SQL_COMISSAO_PAGA + REF + """
              ('pagamento', '["pg-nav10b"]', 'conta_pagar', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        estornado = lambda: ler(pasta, "SELECT estornado_em FROM pagamento WHERE id = 'pg-nav10b'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "s1")
        try:
            r.entrar("Ana")
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            base = r.pilha()
            time.sleep(1)
            f0 = r.foto("-f0")
            r.clicar("Ver no A pagar")
            ok("s1: o 10b abre a ficha da conta", r.esperar(lambda: r.tem("Comissão de Ana · teste")), r.foto("-0"))
            r.clicar("Estornar")
            r.esperar(lambda: r.tem("Estornar o pagamento?"))
            time.sleep(0.5)
            r.digitar("Pago em duplicidade")
            xdo("key", "Return")
            if not ok("s1: oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1")):
                return
            for ida in (1, 2, 3):
                r.clicar("Ver o registro")
                chegou = r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10)
                time.sleep(1.5)
                f = r.foto(f"-ida{ida}")
                ok(f"s1 (ida {ida}): Ver o registro leva a Caixas com a guia", chegou, f)
                setas = r.setas()
                ok(f"s1 (ida {ida}): no destino, nem a ficha nem a pergunta por cima (uma seta só)",
                   not r.tem("Comissão de Ana · teste") and not r.tem("Estornar o pagamento?") and len(setas) == 1,
                   f"{f} setas={setas}")
                pos = r.onde("Aceitar como está")
                ok(f"s1 (ida {ida}): o botão Aceitar como está do registro destacado à vista, sem rolar",
                   any(60 <= y < r.altura() - 30 for _, y in pos), f"{f} posições={pos}")
                if ida == 1:
                    q = razoes(f, f0)
                    obs("s1 I4, com guia: margem direita com o destaque / sem (na origem):", q)
                    ok("s1 (I4): com guia, o véu do destaque continua cheio (a margem a ~0,75 da luz sem véu)",
                       all(0.66 <= v <= 0.82 for v in q), f"{f} razões={q}")
                xdo("key", "Escape")
                voltou = r.esperar(lambda: r.pilha() == base and r.tem("Estornar o pagamento?")
                                   and r.tem("Comissão de Ana · teste"), 10)
                time.sleep(0.8)
                f = r.foto(f"-volta{ida}")
                ok(f"s1 (volta {ida}): desistir volta ao Financeiro com a ficha e a pergunta", voltou, f)
                ok(f"s1 (volta {ida}): com o motivo digitado", r.tem("Pago em duplicidade"), f)
                setas = r.setas()
                ok(f"s1 (volta {ida}): três setas (a tela, a ficha e a pergunta): nenhuma janelinha a mais nem a menos",
                   len(setas) == 3, f"{f} setas={setas}")
                ok(f"s1 (volta {ida}): nada estornado sozinho", estornado() is None)
                if ida < 3:
                    time.sleep(1.0)
                    xdo("key", "Return")
                    if not r.esperar(lambda: r.tem_igual("Ver o registro"), 4):
                        obs(f"s1 volta {ida}: o Enter não confirmou na pergunta reaberta; clicando no último Estornar")
                        r.clicar_ultimo("Estornar")
                    if not ok(f"s1 (volta {ida}): confirmar de novo oferece Ver o registro de novo",
                              r.esperar(lambda: r.tem_igual("Ver o registro"), 6), r.foto(f"-oferta{ida}")):
                        return
            time.sleep(1.0)
            xdo("key", "Escape")
            time.sleep(1.2)
            f = r.foto("-ordem1")
            ok("s1: o 1º Esc fecha a pergunta (a de cima) e a ficha fica",
               not r.tem("Estornar o pagamento?") and r.tem("Comissão de Ana · teste"), f)
            xdo("key", "Escape")
            f = r.foto("-ordem2")
            ok("s1: o 2º Esc fecha a ficha e volta a Comissões (a volta do 10b sobreviveu a três guias)",
               r.esperar(lambda: r.tem_igual("Mês passado") and not r.tem("Comissão de Ana · teste"), 6), f)
            ok("s1: nada estornado", estornado() is None)
            # O 10b de novo, depois de tudo.
            time.sleep(1.2)
            r.esperar(lambda: r.rolar_ate("Ver no A pagar"))
            r.clicar("Ver no A pagar")
            r.esperar(lambda: r.tem("Comissão de Ana · teste"))
            time.sleep(0.8)
            xdo("key", "Escape")
            ok("s1: o 10b de novo: fechar a ficha volta a Comissões", r.esperar(lambda: r.tem_igual("Mês passado"), 6),
               r.foto("-10b"))
            ok("s1: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s2_sair_antes_da_espera(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s2-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-f9', 'cat-aluguel', 'Conta teste Fable', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            """ + REF + """('conta_pagar', '["cp-f9"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        pagos = lambda: ler(pasta, "SELECT COUNT(*) FROM pagamento WHERE conta_pagar_id = 'cp-f9'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "pagar"}, fotos, "s2")
        try:
            r.entrar("Ana")
            if not ok("s2: o A pagar mostra a conta inconsistente", r.esperar(lambda: r.tem("Conta teste Fable")), r.foto("-0")):
                return
            base = r.pilha()
            time.sleep(1)
            f0 = r.foto("-f0")
            for i, atraso in enumerate((0.0, 0.1, 0.25, 0.6)):
                clicar_perto(r, "Pagar", "Conta teste Fable")
                time.sleep(1.5)
                xdo("key", "Return")
                time.sleep(1)
                if not r.tem_igual("Ver o registro"):
                    r.clicar_ultimo("Pagar")
                if not ok(f"s2 ({atraso} s): pagar a conta inconsistente oferece Ver o registro",
                          r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto(f"-{i}-oferta")):
                    obs("s2 textos:", [t for _, _, t in r.textos()][:60])
                    return
                x, y = r.onde("Ver o registro")[-1]
                xdo("mousemove", str(x), str(y), "click", "1")
                if atraso:
                    time.sleep(atraso)
                xdo("key", "Escape")
                voltou = r.esperar(lambda: r.pilha() == base, 6)
                if not voltou:
                    obs(f"s2 ({atraso} s): o 1º Esc não voltou; pilha", r.pilha(), "guia:", r.tem(TITULO_7), "setas:", r.setas())
                    xdo("key", "Escape")
                    voltou = r.esperar(lambda: r.pilha() == base, 6)
                f = r.foto(f"-{i}-volta")
                ok(f"s2 ({atraso} s): sair logo depois de pedir Ver o registro volta ao Financeiro", voltou, f)
                reabriu = r.esperar(lambda: len(r.setas()) == 2, 5)
                ok(f"s2 ({atraso} s): a janelinha do pagamento reabre (duas setas)", reabriu, f"{f} setas={r.setas()}")
                time.sleep(3.5)  # passou o prazo da espera da guia (25 × 120 ms)
                ok(f"s2 ({atraso} s): nenhum TypeError/ReferenceError", not erros(r), "; ".join(erros(r)[:2]))
                ok(f"s2 ({atraso} s): nada pago sozinho", pagos() == 0)
                xdo("key", "Escape")
                fechou = r.esperar(lambda: len(r.setas()) == 1 and r.pilha() == base, 5)
                ok(f"s2 ({atraso} s): o Esc fecha a janelinha reaberta (o teclado continua valendo)", fechou,
                   f"{r.foto(f'-{i}-fechou')} setas={r.setas()} pilha={r.pilha()}")
                if not fechou:
                    return
                r.esperar(lambda: not r.tem("ainda não foi concluída"), 14)
                time.sleep(1.0)
                f = r.foto(f"-{i}-limpo")
                q = razoes(f, f0)
                ok(f"s2 ({atraso} s): nenhum véu de destaque nem faixa da guia sobrou",
                   all(v >= 0.97 for v in q) and not r.tem(TITULO_7), f"{f} razões={q}")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s3_compra(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s3-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO compra (id, fornecedor_id, numero, serie, emissao, total, situacao, autorizacao, xml, criado_em,
                                dispositivo)
              SELECT 'cp-n5', fornecedor_id, '555', '1', emissao, 1000, 'rascunho', 'anterior',
                     CASE WHEN xml <> '' THEN xml ELSE '<NFe/>' END, {AGORA}, {DISPOSITIVO}
                FROM compra ORDER BY rowid LIMIT 1;
            """ + REF + """('compra', '["cp-n5"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        situacao = lambda: ler(pasta, "SELECT situacao FROM compra WHERE id = 'cp-n5'")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaCompra", {"compraId": "cp-n5"}, fotos, "s3")
        try:
            r.entrar("Ana")
            if not ok("s3: a compra em rascunho oferece Lançar no estoque",
                      r.esperar(lambda: r.tem_igual("Lançar no estoque")), r.foto("-0")):
                obs("s3 textos:", [t for _, _, t in r.textos()][:60])
                return
            base = r.pilha()
            marcar = "Atualizar o preço de revenda pelo % de lucro de cada peça"
            if not r.onde(marcar):
                obs("s3: não achei a caixa de marcar; textos:", [t for _, _, t in r.textos()][:80])
            r.rolar_ate(marcar)
            r.clicar(marcar)
            f = r.foto("-1")
            if not ok("s3: mudar um campo da compra inconsistente oferece Ver o registro (1ª vez)",
                      r.esperar(lambda: r.tem_igual("Ver o registro"), 6), f):
                obs("s3 textos:", [t for _, _, t in r.textos()][:80])
                return
            f = r.foto("-1b")
            oferta = [t for _, _, t in r.textos() if "não foi feita" in t]
            obs("s3 texto da oferta (campo):", oferta)
            ok("s3 (N-6): sem janelinha, a oferta traz a primeira frase do motivo (qual registro e o que falta)",
               any("ficou inconsistente" in t and "falta" in t for t in oferta), f)
            xdo("key", "Escape")
            r.esperar(lambda: not r.tem_igual("Ver o registro"), 4)
            time.sleep(0.8)
            r.clicar(marcar)
            time.sleep(1.5)
            f = r.foto("-2")
            ok("s3 (B-8): a 2ª mudança de campo só avisa, sem outra oferta", not r.tem_igual("Ver o registro"), f)
            obs("s3 aviso da 2ª mudança à vista:", r.tem("ficou inconsistente"))
            time.sleep(4.5)
            rr.topo(r, "Lançar no estoque")
            perguntou = r.esperar(lambda: r.tem("Lançar a NF 555?"), 5)
            f = r.foto("-3")
            if not ok("s3: Lançar no estoque pergunta antes", perguntou, f):
                obs("s3 textos:", [t for _, _, t in r.textos()][:80])
                return
            time.sleep(0.6)
            r.clicar_ultimo("Lançar no estoque")
            ofereceu = r.esperar(lambda: r.tem_igual("Ver o registro"), 6)
            f = r.foto("-4")
            ok("s3 (N-5): depois da 1ª oferta, Lançar no estoque oferece Ver o registro de novo", ofereceu, f)
            oferta = [t for _, _, t in r.textos() if "não foi feita" in t]
            obs("s3 texto da oferta (lançar):", oferta, "| aviso só:", r.tem("ficou inconsistente"))
            ok("s3: nada lançado", situacao() == "rascunho")
            if ofereceu:
                ok("s3 (N-6): a oferta do lançar também traz o motivo", any("ficou inconsistente" in t for t in oferta), f)
                r.clicar("Ver o registro")
                ok("s3: a guia leva a Caixas", r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10), r.foto("-5"))
                time.sleep(1.2)
                xdo("key", "Escape")
                ok("s3: desistir volta à compra, sem janelinha por cima",
                   r.esperar(lambda: r.pilha() == base and len(r.setas()) == 1, 10), f"{r.foto('-6')} setas={r.setas()}")
                ok("s3: nada lançado sozinho na volta", situacao() == "rascunho")
            ok("s3: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s4_aceitar_outro(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s4-")
    try:
        montar_loja(pasta)
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-a', 'cat-aluguel', 'Conta A Fable', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-b', 'cat-aluguel', 'Conta B Fable', 2000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            """ + REF + """('conta_pagar', '["cp-a"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log'),
                           ('conta_pagar', '["cp-b"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-02T00:00:00Z', 'log');""")
        estados = lambda: dict(ler(pasta, "SELECT chave, estado FROM sync_referencia WHERE chave IN ('[\"cp-a\"]', '[\"cp-b\"]')"))  # noqa: E731
        pagos = lambda: ler(pasta, "SELECT COUNT(*) FROM pagamento WHERE conta_pagar_id IN ('cp-a', 'cp-b')")[0][0]  # noqa: E731
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "pagar"}, fotos, "s4")
        try:
            r.entrar("Ana")
            if not ok("s4: o A pagar mostra as duas contas inconsistentes",
                      r.esperar(lambda: r.tem("Conta A Fable") and r.tem("Conta B Fable")), r.foto("-0")):
                return
            base = r.pilha()
            time.sleep(1)
            f0 = r.foto("-f0")
            clicar_perto(r, "Pagar", "Conta A Fable")
            time.sleep(1.5)
            xdo("key", "Return")
            time.sleep(1)
            if not r.tem_igual("Ver o registro"):
                r.clicar_ultimo("Pagar")
            if not ok("s4: pagar a conta A oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1")):
                return
            r.clicar("Ver o registro")
            ok("s4: a guia aparece em Caixas", r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10), r.foto("-2"))
            chegou = time.time()
            # A tela espera a parte assentar antes de destacar (TelaConfiguracoes, esperaDaGuia: até 25 × 120 ms). Na CI
            # da N6 (44361340632), a foto de 2 s depois da chegada ainda saiu sem o destaque (razões 1.0; em todas as
            # outras, 0,74), sem causa explicada. CI-1/CI-2: só o roteiro espera mais. Fotos a cada ~0,5 s, por até 6 s,
            # com o tempo anotado (uma demora de verdade continua à vista na MEDIDA).
            time.sleep(1.0)
            while True:
                f1 = r.foto("-3")
                q1 = razoes(f1, f0)
                if all(0.66 <= v <= 0.82 for v in q1) or time.time() - chegou > 6.0:
                    break
                time.sleep(0.5)
            print(f"MEDIDA s4: o destaque visto {time.time() - chegou:.1f} s depois da chegada (razões {q1})", flush=True)
            obs("s4 na chegada: margem com o destaque / sem (na origem):", q1)
            ok("s4: na chegada, o registro da guia está destacado (o véu cheio na margem)", all(0.66 <= v <= 0.82 for v in q1),
               f"{f1} razões={q1}")
            pos = unicos(r.onde("Aceitar como está"))
            obs("s4 botões Aceitar como está (o 1º é o da guia):", pos)
            if not ok("s4: os dois registros aparecem com Aceitar como está", len(pos) == 2, f1):
                return
            # O de baixo é o do OUTRO registro (o da guia vem primeiro na lista).
            for _ in range(8):
                pos = unicos(r.onde("Aceitar como está"))
                if len(pos) == 2 and 60 <= pos[1][1] < r.altura() - 30:
                    break
                xdo("mousemove", "600", "560")
                xdo("click", "5")
                time.sleep(0.5)
            pos = unicos(r.onde("Aceitar como está"))
            r.clicar_em(*pos[-1])
            r.esperar(lambda: r.tem("Aceitar como está?"))
            time.sleep(0.5)
            r.digitar("Outro registro, aceito pelo revisor")
            time.sleep(0.3)
            xdo("key", "Return")
            aceitou = r.esperar(lambda: "aceito" in estados().values(), 8)
            # A falha do s4 na CI da integ7 (16/11): neste computador o destaque volta de 2,5 a 2,7 s depois de aceitar
            # (sonda, validacao/integ8_provas/sonda_s4_*), e a CI é cerca de duas vezes mais lenta; a espera fixa de 2 s
            # não bastava lá. O roteiro espera o destaque voltar, até 10 s, e continua EXIGINDO que ele volte (a
            # conferência N-7 abaixo é a mesma). O programa não muda.
            t_aceito = time.time()
            time.sleep(2.0)
            f2 = r.foto("-4")
            while not all(0.66 <= v <= 0.82 for v in razoes(f2, f0)) and time.time() - t_aceito < 10:
                time.sleep(0.25)
                f2 = r.foto("-4")
            e = estados()
            obs("s4 estados depois de aceitar o de baixo:", e)
            ok("s4: o aceito foi o OUTRO registro (cp-b); o da guia (cp-a) continua inconsistente",
               aceitou and e.get('["cp-b"]') == "aceito" and e.get('["cp-a"]') == "inconsistente", f"{f2} {e}")
            ok("s4 (B-2): a guia continua aberta, na mesma tela, sem erro na faixa",
               r.tem(TITULO_7) and r.pilha() == base + 1 and not r.tem("Não foi salvo"), f2)
            q2 = razoes(f2, f0)
            obs("s4 depois de aceitar o outro: margem / sem véu:", q2)
            ok("s4 (N-7): depois de aceitar outro registro, o destaque do registro da guia volta (o véu na margem)",
               all(0.66 <= v <= 0.82 for v in q2), f"{f2} razões={q2}")
            pos = unicos(r.onde("Aceitar como está"))
            obs("s4 botões Aceitar como está depois:", pos)
            ok("s4 (N-7): o botão do registro da guia fica à vista depois de o destaque voltar",
               len(pos) >= 1 and 60 <= pos[0][1] < r.altura() - 30, f"{f2} posições={pos}")
            # Agora o da guia: aceito, a guia conclui e volta.
            if pos:
                r.clicar_em(*pos[0])
                r.esperar(lambda: r.tem("Aceitar como está?"))
                time.sleep(0.5)
                r.digitar("Registro da guia")
                time.sleep(0.3)
                xdo("key", "Return")
                f = r.foto("-5")
                ok("s4: aceito o registro da guia, volta ao Financeiro com a janelinha do pagamento",
                   r.esperar(lambda: r.pilha() == base and len(r.setas()) == 2, 12), f"{f} setas={r.setas()}")
                ok("s4: os dois aceitos", set(estados().values()) == {"aceito"}, str(estados()))
            ok("s4: nada pago sozinho", pagos() == 0)
            ok("s4: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s5_historico_com_filtro(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s5-")
    try:
        montar_loja(pasta)
        por_fora(pasta, "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando) "
                        "VALUES ('categoria', json_array('cat-aluguel'), 'v1', 'v2', 'caixa-2', datetime('now'))")
        r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "historico"}, fotos, "s5")
        try:
            r.entrar("Ana")
            if not ok("s5: o Histórico abre", r.esperar(lambda: r.tem("Quem fez o quê")), r.foto("-0")):
                obs("s5 textos:", [t for _, _, t in r.textos()][:60])
                return
            base = r.pilha()
            dica = "Filtrar por usuário, caixa ou o que foi feito"
            r.rolar_ate(dica)
            if not r.clicar(dica):
                obs("s5: a dica do filtro não está nos textos; clico abaixo do título")
                p = r.onde("Quem fez o quê")
                if p:
                    r.clicar_em(p[0][0] + 200, p[0][1] + 60)
            time.sleep(0.6)
            r.digitar("zzfable")
            time.sleep(1.0)
            f = r.foto("-1")
            if not ok("s5: o filtro de texto fica digitado no Histórico", r.tem("zzfable"), f):
                obs("s5 textos:", [t for _, _, t in r.textos()][:60])
                return
            xdo("key", "Home")
            rr.topo(r, "Duplicatas")
            if not ok("s5: Duplicatas mostra o conflito com Ver no histórico",
                      r.esperar(lambda: r.rolar_ate("Ver no histórico")), r.foto("-2")):
                obs("s5 textos:", [t for _, _, t in r.textos()][:60])
                return
            r.clicar("Ver no histórico")
            r.esperar(lambda: r.tem("Quem fez o quê"))
            time.sleep(1.0)
            f = r.foto("-3")
            aviso = [t for _, _, t in r.textos() if "Não há evento só deste registro" in t]
            obs("s5 aviso do Histórico:", aviso)
            if not ok("s5: sem evento do registro, o Histórico avisa (o ramo do N-9)", bool(aviso), f):
                obs("s5 textos:", [t for _, _, t in r.textos()][:60])
                return
            ok("s5 (N-9): com o filtro de texto valendo, o aviso diz o filtro (e não \"todo o histórico\")",
               "com o filtro" in aviso[0] and "zzfable" in aviso[0] and "todo o histórico" not in aviso[0], f"{f} {aviso}")
            ok("s5: o filtro continua no campo (M-5)", any(t.strip() == "zzfable" for _, _, t in r.textos()), f)
            ok("s5: na mesma tela (sem empilhar)", r.pilha() == base, f)
            xdo("key", "Escape")
            ok("s5 (M-5): o Esc volta às Duplicatas", r.esperar(lambda: r.tem_igual("Ver no histórico") and r.pilha() == base, 6),
               r.foto("-4"))
            ok("s5: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s6_i1_tres_caixas(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s6-")
    try:
        montar_loja(pasta, avisos=True)
        por_fora(pasta,
                 "CREATE TEMP TABLE o2 AS SELECT * FROM encomenda WHERE situacao = 'aberta' LIMIT 1; "
                 "UPDATE o2 SET id = 'enc-caixa-2', dispositivo = 'caixa-2', cliente_nome = 'Cliente do Caixa 2', "
                 "descricao = 'Correia dentada', situacao = 'pedida', pedida_em = criado_em; "
                 "INSERT INTO encomenda SELECT * FROM o2; "
                 "UPDATE o2 SET id = 'enc-caixa-3', dispositivo = 'caixa-3', cliente_nome = 'Cliente do Caixa 3', "
                 "descricao = 'Bomba de óleo', situacao = 'aberta', pedida_em = NULL, "
                 "criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+1 minute'); "
                 "INSERT INTO encomenda SELECT * FROM o2;")
        situacao = lambda: ler(pasta, "SELECT id, situacao FROM encomenda ORDER BY id")  # noqa: E731
        antes = situacao()
        obs("s6 encomendas:", ler(pasta, "SELECT numero, dispositivo, situacao, descricao FROM encomenda ORDER BY numero, criado_em"))
        original = nav.Rustinho
        nav.Rustinho = RustinhoComRetangulos
        try:
            r = placar.abrir(pasta, "TelaInicio", {}, fotos, "s6")
        finally:
            nav.Rustinho = original
        try:
            r.entrar("Ana")
            inicio = r.pilha()
            aviso = "Mesmo número em caixas diferentes"
            if not r.tem(aviso):
                r.clicar("Atenção: Número de encomenda repetido")
            achou = r.esperar(lambda: r.tem(aviso))
            f = r.foto("-1")
            if not ok("s6 (I1): com três caixas, o Início avisa o número repetido", achou, f):
                obs("s6 textos:", [t for _, _, t in r.textos()][:80])
                return
            texto = [t for _, _, t in r.textos() if aviso in t]
            obs("s6 texto do aviso:", texto)
            ok("s6 (I1): o número aparece uma vez só no aviso", all(t.count("nº ") == 1 for t in texto), f"{f} {texto}")
            time.sleep(1.2)
            r.clicar("Ver encomendas")
            chegou = r.esperar(lambda: r.tem("Correia dentada") and r.pilha() == inicio + 1)
            xdo("mousemove", "1400", "950")  # o ponteiro fora da janela: nenhuma linha com o mouse em cima
            time.sleep(0.8)
            com = r.foto("-2")
            ok("s6 (I1): Ver encomendas leva a Estoque › Encomendas, com a do outro caixa à vista", chegou, com)
            linha = textos_com(r, "Cliente do Caixa 2")
            obs("s6 linha da encomenda do caixa 2:", linha)
            ok("s6 (I1): a linha diz o número e o caixa (\"outro caixa\")",
               any("Nº" in t and "caixa" in t.lower() for _, _, t in linha), f"{com} {linha}")
            acoes = unicos(r.onde("Cancelar encomenda"))
            correia = textos_com(r, "Correia dentada")
            bomba = textos_com(r, "Bomba de óleo")
            obs("s6 Cancelar encomenda em:", acoes, "| Correia em:", [(x, y) for x, y, _ in correia],
                "| Bomba em:", [(x, y) for x, y, _ in bomba])
            ok("s6 (I1): as ações à vista são as da encomenda do caixa 2 (a mais antiga dos outros caixas), e só dela",
               len(acoes) == 1 and bool(correia) and bool(bomba)
               and abs(acoes[0][1] - correia[0][1]) < abs(acoes[0][1] - bomba[0][1]), f"{com} {acoes}")
            # S-4 (2ª re-revisão): na linha escolhida, nenhum texto por cima de outro (antes, a etiqueta "Paga na
            # entrega" ficava por baixo de "Cancelar encomenda").
            # A altura da linha: a do "Cancelar encomenda" dela (o meio da linha de 66 px); a faixa de ±34 px pega os
            # textos da linha e nenhum da vizinha.
            y_linha = acoes[0][1] if acoes else -999
            da_linha = [p for p in textos_pintados(r) if abs((p[1] + p[3]) / 2 - y_linha) < 34]
            cobertos = sobrepostos(da_linha)
            obs("s6 S-4: textos da linha escolhida:", [(p[4], [round(v) for v in p[:4]]) for p in da_linha])
            ok("s6 (S-4): na linha escolhida, nenhum texto fica por cima de outro",
               len(da_linha) >= 3 and not cobertos, f"{com} {cobertos[:3]}")
            time.sleep(9)
            sem = r.foto("-3")
            q = razoes(com, sem)
            obs("s6 I4, sem guia: margem com o destaque / sem:", q)
            ok("s6 (I4): sem guia, o véu do destaque é o leve (a margem a ~0,87 da luz sem véu) e acaba sozinho",
               all(0.83 <= v <= 0.93 for v in q), f"{com} razões={q}")
            xdo("key", "Escape")
            ok("s6: o Esc volta ao Início, sem empilhar", r.esperar(lambda: r.pilha() == inicio), r.foto("-4"))
            ok("s6: nenhuma encomenda mudou", situacao() == antes)
            ok("s6: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s7_sessao_acaba_no_destino(placar, fotos):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s7-")
    try:
        montar_loja(pasta)
        por_fora(pasta, """
            INSERT INTO caixa_sessao (id, dispositivo, caixa_nome, aberto_em, aberto_por, valor_inicial)
              SELECT 'sessao-s7', valor, 'Caixa 1', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT id FROM usuario WHERE nome = 'Ana'), 0
                FROM config WHERE chave = 'dispositivo_id';
            INSERT INTO conta_receber (id, cliente_id, descricao, valor, vencimento, criado_em, dispositivo)
              SELECT 'cr-s7', id, 'Caderno', 5000, date('now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                     (SELECT valor FROM config WHERE chave = 'dispositivo_id')
                FROM pessoa WHERE nome = 'Zé da Oficina';
            """ + REF + """('conta_receber', '["cr-s7"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log');""")
        ze = ler(pasta, "SELECT id FROM pessoa WHERE nome = 'Zé da Oficina'")[0][0]
        ja = ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0]
        recebidos = lambda: ler(pasta, "SELECT COUNT(*) FROM recebimento")[0][0] - ja  # noqa: E731
        estado = lambda: ler(pasta, "SELECT estado FROM sync_referencia WHERE chave = '[\"cr-s7\"]'")  # noqa: E731
        r = placar.abrir(pasta, "TelaReceber", {"clienteInicial": ze}, fotos, "s7")
        try:
            r.entrar("Bia")
            r.esperar(lambda: r.tem("Receber tudo"))
            r.clicar("Receber tudo")
            r.esperar(lambda: r.tem("Receber de"))
            time.sleep(0.6)
            xdo("key", "Return")
            if not ok("s7: a recusa oferece Ver o registro ao caixa", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-0")):
                return
            r.clicar("Ver o registro")
            rr.senha_gerente(r, "destino da guia")
            if not ok("s7: com a senha, a guia aparece em Caixas", r.esperar(lambda: r.tem(TITULO_7), 10), r.foto("-1")):
                return
            time.sleep(1.5)
            r.rolar_ate("Aceitar como está")
            rr.topo(r, "Aceitar como está")
            if not ok("s7: a pergunta do aceite abre", r.esperar(lambda: r.tem("Aceitar como está?")), r.foto("-2")):
                return
            rr.desativar(pasta, "Bia")  # desativada em outro caixa, com a guia ativa e a pergunta aberta
            time.sleep(0.5)
            r.digitar("teste do revisor")
            time.sleep(0.3)
            xdo("key", "Return")
            f = r.foto("-3")
            if not ok("s7: a sessão encerrada no destino da guia volta à tela de entrada",
                      r.esperar(lambda: r.tem("Quem está usando?"), 15), f):
                obs("s7 textos:", [t for _, _, t in r.textos()][:60])
                return
            time.sleep(1.0)
            f = r.foto("-3b")
            ok("s7: nada sobra por cima da entrada (a faixa da guia, a pergunta, a janelinha do recebimento)",
               not r.tem(TITULO_7) and not r.tem("Aceitar como está?") and not r.tem("Receber de"), f)
            ok("s7: o registro continua inconsistente e nada foi recebido", estado() == [("inconsistente",)] and recebidos() == 0,
               str(estado()))
            r.entrar("Ana")
            if not ok("s7: a pessoa seguinte (Ana) entra e chega ao Receber", r.esperar(lambda: r.tem("Receber tudo"), 12), r.foto("-4")):
                obs("s7 textos:", [t for _, _, t in r.textos()][:60])
                return
            base = r.pilha()
            ok("s7: sem resto da guia nem véu para a pessoa seguinte", not r.tem(TITULO_7) and not r.tem("conclua a operação"), r.foto("-4b"))
            r.clicar("Receber tudo")
            r.esperar(lambda: r.tem("Receber de"))
            time.sleep(0.6)
            xdo("key", "Return")
            if not ok("s7: a recusa oferece Ver o registro à Ana", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-5")):
                return
            r.clicar("Ver o registro")
            chegou = r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10)
            f = r.foto("-6")
            ok("s7: depois de a sessão acabar no meio de uma guia, Ver o registro leva a Caixas com a guia", chegou, f)
            ok("s7: no destino, a janelinha do recebimento não fica por cima", not r.tem("Receber de") and len(r.setas()) == 1,
               f"{f} setas={r.setas()}")
            time.sleep(1.2)
            xdo("key", "Escape")
            ok("s7: desistir volta ao Receber com a janelinha reaberta (duas setas)",
               r.esperar(lambda: r.pilha() == base and r.tem("Receber de") and len(r.setas()) == 2, 10),
               f"{r.foto('-7')} setas={r.setas()}")
            ok("s7: nada foi recebido sozinho", recebidos() == 0)
            ok("s7: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


# ---------------------------------------------------------------------------------------------------------------
def s8_destaque_e_pilulas(placar, fotos):
    """A observação do autor: com a página de Configurações rolada, o destaque da guia é desenhado por cima das
    pílulas fixas. Uma lista comprida de registros (14), a guia no primeiro; rola até ele passar por baixo das
    pílulas e tira as fotos."""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s8-")
    try:
        montar_loja(pasta)
        outros = ", ".join(f"""('conta_pagar', '["nao-existe-{i:02d}"]', 'pessoa', '["sumiu"]', 'inconsistente',
                               '2026-02-{i:02d}T00:00:00Z', 'log')""" for i in range(1, 14))
        por_fora(pasta, f"""
            INSERT INTO conta_pagar (id, categoria_id, descricao, valor, vencimento, criado_em, dispositivo, competencia)
              VALUES ('cp-f9', 'cat-aluguel', 'Conta teste Fable', 1000, date('now'), {AGORA}, {DISPOSITIVO},
                      strftime('%Y-%m', 'now'));
            """ + REF + """('conta_pagar', '["cp-f9"]', 'pessoa', '["sumiu"]', 'inconsistente', '2026-01-01T00:00:00Z', 'log'), """
                 + outros + ";")
        r = placar.abrir(pasta, "TelaFinanceiro", {"secao": "pagar"}, fotos, "s8")
        try:
            r.entrar("Ana")
            if not ok("s8: o A pagar mostra a conta inconsistente", r.esperar(lambda: r.tem("Conta teste Fable")), r.foto("-0")):
                return
            base = r.pilha()
            clicar_perto(r, "Pagar", "Conta teste Fable")
            time.sleep(1.5)
            xdo("key", "Return")
            time.sleep(1)
            if not r.tem_igual("Ver o registro"):
                r.clicar_ultimo("Pagar")
            if not ok("s8: oferece Ver o registro", r.esperar(lambda: r.tem_igual("Ver o registro")), r.foto("-1")):
                return
            r.clicar("Ver o registro")
            ok("s8: a guia aparece em Caixas", r.esperar(lambda: r.tem(TITULO_7) and r.pilha() == base + 1, 10), r.foto("-2"))
            time.sleep(2.0)
            f = r.foto("-3")
            pos = unicos(r.onde("Aceitar como está"))
            obs("s8 na chegada, botões Aceitar como está:", pos, "| pílulas:", r.onde("Caixas"), r.onde("Simultaneidade"))
            ok("s8: com 14 registros, o da guia (o 1º) chega à vista", len(pos) >= 2 and 60 <= pos[0][1] < r.altura() - 30,
               f"{f} posições={pos}")
            pilula = max([y for _, y in r.onde("Caixas")] or [350])
            for k in range(12):
                pos = unicos(r.onde("Aceitar como está"))
                if pos and pos[0][1] < pilula + 10:
                    break
                xdo("mousemove", "600", "600")
                xdo("click", "5")
                time.sleep(0.5)
            time.sleep(0.8)
            f = r.foto("-4")
            pos = unicos(r.onde("Aceitar como está"))
            obs("s8 depois de rolar: o botão do registro da guia em", pos[:1], "| a pílula Caixas em y =", pilula, "| foto:", f)
            # O destaque por cima das pílulas: a luz da pílula "Caixas"/"Duplicatas" com a página rolada / na chegada.
            from PIL import Image
            a, b = Image.open(r.fotos + "/" + r.nome + "-3.png").convert("L"), Image.open(f).convert("L")
            faixa = [(x, pilula) for x in (90, 200, 380, 470)]
            def med(im, x, y):
                px = sorted(im.crop((x - 6, y - 14, x + 6, y - 9)).getdata())
                return px[len(px) // 2] + 1
            q = [round(med(b, x, y) / med(a, x, y), 2) for x, y in faixa]
            obs("s8 luz em volta das pílulas, rolada / na chegada:", q)
            ok("s8: medido (a observação do autor): ver a foto e as razões acima", True, f)
            xdo("key", "Escape")
            r.esperar(lambda: r.pilha() == base, 10)
            ok("s8: nenhum TypeError/ReferenceError no registro", not erros(r), "; ".join(erros(r)[:2]))
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def s9_aviso_do_filtro_depois(placar, fotos):
    """N-9, o que sobra: o aviso do Histórico é escrito uma vez; se a pessoa apaga o filtro, ele continua dizendo
    "com o filtro …"?"""
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix="rustinho-fable2-s9-")
    try:
        montar_loja(pasta)
        por_fora(pasta, "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando) "
                        "VALUES ('categoria', json_array('cat-aluguel'), 'v1', 'v2', 'caixa-2', datetime('now'))")
        r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "historico"}, fotos, "s9")
        try:
            r.entrar("Ana")
            if not ok("s9: o Histórico abre", r.esperar(lambda: r.tem("Quem fez o quê")), r.foto("-0")):
                return
            linhas_todas = len(r.textos())
            dica = "Filtrar por usuário, caixa ou o que foi feito"
            r.rolar_ate(dica)
            r.clicar(dica)
            time.sleep(0.6)
            r.digitar("zzfable")
            time.sleep(1.0)
            rr.topo(r, "Duplicatas")
            r.esperar(lambda: r.rolar_ate("Ver no histórico"))
            r.clicar("Ver no histórico")
            r.esperar(lambda: r.tem("Quem fez o quê"))
            time.sleep(1.0)
            aviso = [t for _, _, t in r.textos() if "Não há evento só deste registro" in t]
            if not ok("s9: o aviso diz o filtro", bool(aviso) and "zzfable" in aviso[0], r.foto("-1")):
                return
            com_filtro = len(r.textos())
            campo = [(x, y) for x, y, t in r.textos() if t.strip() == "zzfable"]
            if campo:
                r.clicar_em(*campo[0])
            time.sleep(0.4)
            xdo("key", "ctrl+a")
            xdo("key", "BackSpace")
            time.sleep(1.2)
            f = r.foto("-2")
            aviso2 = [t for _, _, t in r.textos() if "Não há evento só deste registro" in t]
            sem_filtro = len(r.textos())
            obs("s9 depois de apagar o filtro: aviso:", aviso2, "| textos na tela: todos", linhas_todas, "com o filtro",
                com_filtro, "depois de apagar", sem_filtro, "| campo ainda com zzfable:",
                any(t.strip() == "zzfable" for _, _, t in r.textos()))
            ok("s9 (S-2): o filtro foi apagado do campo", not any(t.strip() == "zzfable" for _, _, t in r.textos()), f)
            ok("s9 (S-2): o aviso continua (o registro não tem evento) e não fala mais do filtro apagado",
               bool(aviso2) and "zzfable" not in aviso2[0] and "todo o histórico" in aviso2[0], f"{f} {aviso2}")
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


CASOS = {"s1": s1_cadeias_seguidas, "s2": s2_sair_antes_da_espera, "s3": s3_compra, "s4": s4_aceitar_outro,
         "s5": s5_historico_com_filtro, "s6": s6_i1_tres_caixas, "s7": s7_sessao_acaba_no_destino,
         "s9": s9_aviso_do_filtro_depois, "s8": s8_destaque_e_pilulas}
PADRAO = ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s9"]  # o s8 só mede (S-3, à espera da decisão)


def main():
    if not os.path.exists(PROGRAMA):
        sys.exit("compile antes")
    with open(PROGRAMA, "rb") as fh:
        print("BINÁRIO USADO:", PROGRAMA, hashlib.sha256(fh.read()).hexdigest(), flush=True)
    args = sys.argv[1:]
    escolhidos = PADRAO
    if os.environ.get("RUSTINHO_CASOS_REREV2"):
        escolhidos = os.environ["RUSTINHO_CASOS_REREV2"].split(",")
    fotos = args[0] if args else tempfile.mkdtemp(prefix="rustinho-fotos-rerev2-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        for c in escolhidos:
            print(f"--- {c}", flush=True)
            try:
                CASOS[c](placar, fotos)
            except Exception as e:
                placar.ok(f"{c} chegou ao fim ({e!r})", False)
        for log in placar.logs:
            with open(log, errors="replace") as fh:
                for ln in fh:
                    if "TypeError" in ln or "ReferenceError" in ln or "Error:" in ln:
                        print("LOG", os.path.basename(log), ln.strip()[:300], flush=True)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    # R-1 da re-revisão 3 do Fable (16/11/2026): a saída diz o resultado (a CI e a roteiros_tela.sh decidem pelo
    # código de saída; antes, este roteiro saía com 0 mesmo com ✘).
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
