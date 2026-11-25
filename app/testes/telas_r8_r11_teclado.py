#!/usr/bin/env python3
"""R8 / D-AUD-8 e R11 / D-AUD-9 (decisões do proprietário, 25/11/2026): o teclado no balcão.

A. Fechar o caixa:
   - o Enter no "Dinheiro contado" não fecha o caixa;
   - "Fechar e imprimir" pede confirmação, com o contado, o esperado e a diferença;
   - o Enter não confirma a pergunta.
   O caixa da loja de teste continua aberto: o roteiro desiste com o Esc. Fechar de verdade imprime, e a conta do
   fechamento é conferida pelos testes do núcleo.
B. Pergunta de perigo ("Descartar alterações?" na ficha da peça): o Enter não confirma; a pergunta continua aberta.
A2. Receber da venda: o Enter no valor não conclui; o foco vai ao "Concluir" (as outras formas estão vazias), e o
    Enter seguinte conclui: um Enter a mais por venda (M-N1 e M-N2 da revisão R2).
C. Botões das linhas pelo teclado (Trocas › feitas): sem o mouse em cima, o Tab chega à linha e os botões dela
   ("Desfazer") aparecem.
A3. Por último: fechar o caixa de verdade pelo teclado (Tab até o botão da pergunta e Enter).

Usa a loja de teste e o jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_r8_r11_teclado.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import TELA, Rustinho, conferir, montar_loja, simples, xdo  # noqa: E402


def caixas_abertos(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    n = c.execute("SELECT COUNT(*) FROM caixa_sessao WHERE fechado_em IS NULL").fetchone()[0]
    c.close()
    return n


def vendas_concluidas(pasta):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    n = c.execute("SELECT COUNT(*) FROM venda WHERE tipo = 'venda' AND situacao = 'concluida'").fetchone()[0]
    c.close()
    return n


def tem(textos, pedaco):
    return any(simples(pedaco) in simples(t) for _, _, t in textos)


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r8-r11-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r8-r11-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        filtro, _compra, _oleo = montar_loja(pasta)

        # ---- A. Fechar o caixa ----
        r = Rustinho(pasta, "TelaCaixa", {}, fotos, "r8-fechar")
        try:
            r.entrar("Ana")
            time.sleep(2)
            if not r.clicar("Fechar caixa"):
                raise RuntimeError(f"não achei o botão Fechar caixa (veja {r.saida})")
            time.sleep(1.5)
            xdo("type", "--delay", "60", "150")
            xdo("key", "Return")
            time.sleep(1.5)
            textos = r.textos()
            ok = caixas_abertos(pasta) == 1 and not tem(textos, "Fechar o caixa?")
            print(("✔ " if ok else "✘ ") + "caixa: o Enter no dinheiro contado não fecha o caixa"
                  + ("" if ok else f" (foto: {r.foto('-1')})"))
            tudo_certo &= ok
            if not r.clicar("Fechar e imprimir"):
                raise RuntimeError(f"não achei o botão Fechar e imprimir (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("caixa: a pergunta mostra o contado, o esperado e a diferença", r.foto("-2"), r.textos(),
                                   tem=["Fechar o caixa?", "Contado: R$ 150,00", "Esperado na gaveta: R$"])
            xdo("key", "Return")
            time.sleep(1.5)
            textos = r.textos()
            ok = caixas_abertos(pasta) == 1 and tem(textos, "Fechar o caixa?")
            print(("✔ " if ok else "✘ ") + "caixa: o Enter não confirma a pergunta (o caixa continua aberto)"
                  + ("" if ok else f" (foto: {r.foto('-3')})"))
            tudo_certo &= ok
            xdo("key", "Escape")
            time.sleep(1)
            ok = caixas_abertos(pasta) == 1
            print(("✔ " if ok else "✘ ") + "caixa: o Esc desiste e o caixa continua aberto")
            tudo_certo &= ok
        finally:
            r.fechar()

        # ---- A2. O Receber da venda (M-N1 e M-N2 da revisão R2): o Enter no valor não conclui; vai ao "Concluir"
        # (as outras formas estão vazias); o Enter seguinte conclui: um Enter a mais por venda. ----
        r = Rustinho(pasta, "TelaVender", {}, fotos, "r8-receber")
        try:
            r.entrar("Ana")
            time.sleep(2)
            xdo("type", "--delay", "30", "Filtro")
            time.sleep(0.8)
            xdo("key", "Return")
            time.sleep(1.2)
            if not tem(r.textos(), "Subtotal"):
                raise RuntimeError(f"a peça não entrou no carrinho (veja {r.saida})")
            xdo("key", "F2")
            time.sleep(1.5)
            if not tem(r.textos(), "Concluir venda"):
                raise RuntimeError(f"o Receber da venda não abriu (veja {r.saida})")
            antes = vendas_concluidas(pasta)
            xdo("type", "--delay", "60", "500")
            xdo("key", "Return")
            time.sleep(1.5)
            ok = vendas_concluidas(pasta) == antes and tem(r.textos(), "Concluir venda")
            print(("✔ " if ok else "✘ ") + "receber: o Enter no valor não conclui a venda"
                  + ("" if ok else f" (foto: {r.foto('-1')})"))
            tudo_certo &= ok
            xdo("key", "Return")
            time.sleep(2.5)
            ok = vendas_concluidas(pasta) == antes + 1
            print(("✔ " if ok else "✘ ") + "receber: o Enter seguinte (no Concluir) conclui: um Enter a mais"
                  + ("" if ok else f" (foto: {r.foto('-2')})"))
            tudo_certo &= ok
        finally:
            r.fechar()

        # ---- B. Pergunta de perigo ----
        r = Rustinho(pasta, "TelaPeca", {"produtoId": filtro}, fotos, "r8-perigo")
        try:
            r.entrar("Ana")
            time.sleep(2)
            # O nome aparece no título e no campo: o campo é o último (como em telas_seta_esc.py).
            pos = [(x, y) for x, y, t in r.textos_para_clicar() if t == "Filtro de óleo"]
            if not pos:
                raise RuntimeError(f"não achei o campo do nome da peça (veja {r.saida})")
            xdo("mousemove", str(pos[-1][0]), str(pos[-1][1]), "click", "1")
            time.sleep(0.5)
            xdo("key", "End")
            xdo("type", "--delay", "60", " X")
            time.sleep(0.5)
            xdo("key", "Escape")
            time.sleep(1.5)
            tudo_certo &= conferir("perigo: o Esc na ficha alterada pergunta", r.foto("-1"), r.textos(),
                                   tem=["Descartar alterações?"])
            xdo("key", "Return")
            time.sleep(1.5)
            textos = r.textos()
            ok = tem(textos, "Descartar alterações?") and tem(textos, "Filtro de óleo X")
            print(("✔ " if ok else "✘ ") + "perigo: o Enter não confirma o descarte (a pergunta e a mudança continuam)"
                  + ("" if ok else f" (foto: {r.foto('-2')})"))
            tudo_certo &= ok
        finally:
            r.fechar()

        # ---- C. Botões das linhas pelo teclado ----
        r = Rustinho(pasta, "TelaTrocas", {"secao": "feitas"}, fotos, "r11-trocas")
        try:
            r.entrar("Ana")
            time.sleep(2)
            xdo("mousemove", "1", "1")
            time.sleep(0.5)
            antes = r.textos()
            escondido = not any(simples(t) == simples("Desfazer") for _, _, t in antes)
            print(("✔ " if escondido else "✘ ") + "linhas: sem o mouse em cima, o Desfazer da linha está escondido"
                  + ("" if escondido else f" (foto: {r.foto('-1')})"))
            tudo_certo &= escondido
            apareceu = False
            for _ in range(40):
                xdo("key", "Tab")
                time.sleep(0.25)
                if any(simples(t) == simples("Desfazer") for _, _, t in r.textos()):
                    apareceu = True
                    break
            foto = r.foto("-2")
            print(("✔ " if apareceu else "✘ ") + "linhas: o Tab chega à linha e o Desfazer aparece, sem o mouse"
                  + ("" if apareceu else f" (foto: {foto})"))
            tudo_certo &= apareceu
        finally:
            r.fechar()

        # ---- C2. Decisão 5 do proprietário (25/11/2026): as linhas de Cheques pelo teclado. Um cheque em carteira,
        # pelo núcleo; sem o mouse, o "Devolvido" da linha está escondido; o Tab chega à linha e ele aparece. ----
        import telas_d09 as base
        ana = sqlite3.connect(os.path.join(pasta, "rustinho.db")).execute("SELECT id FROM usuario WHERE nome = 'Ana'").fetchone()[0]
        base._aux_nucleo.executar("estavel", "executar_por_fora", {"RUSTINHO_LOJA_TESTE": pasta, "RUSTINHO_SQL": (
            "INSERT INTO cheque (id, emitente, banco, numero, valor, bom_para, situacao, criado_em, criado_por, dispositivo)"
            f" VALUES ('ch-r11', 'Emitente R11', 'Itaú', '000111', 15000, date('now'), 'carteira',"
            f" strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), '{ana}', 'disp-r11');"
        )}, raiz=base.RAIZ)
        r = Rustinho(pasta, "TelaFinanceiro", {"secao": "cheques"}, fotos, "r11-cheques")
        try:
            r.entrar("Ana")
            time.sleep(2)
            xdo("mousemove", "1", "1")
            time.sleep(0.5)
            escondido = not any(simples(t) == simples("Devolvido") for _, _, t in r.textos())
            print(("✔ " if escondido else "✘ ") + "cheques: sem o mouse em cima, o Devolvido da linha está escondido"
                  + ("" if escondido else f" (foto: {r.foto('-1')})"))
            tudo_certo &= escondido
            apareceu = False
            for _ in range(40):
                xdo("key", "Tab")
                time.sleep(0.25)
                if any(simples(t) == simples("Devolvido") for _, _, t in r.textos()):
                    apareceu = True
                    break
            print(("✔ " if apareceu else "✘ ") + "cheques: o Tab chega à linha e o Devolvido aparece, sem o mouse"
                  + ("" if apareceu else f" (foto: {r.foto('-2')})"))
            tudo_certo &= apareceu
        finally:
            r.fechar()

        # ---- C3. Decisão 5: a lista de Compras pelo teclado. O Tab chega à compra e o Enter abre (como o clique). ----
        # B4 da revisão R6: uma segunda compra (cópia da primeira, com outro número), para o Tab chegar à segunda linha.
        base._aux_nucleo.executar("estavel", "executar_por_fora", {"RUSTINHO_LOJA_TESTE": pasta, "RUSTINHO_SQL": (
            "CREATE TEMP TABLE c2 AS SELECT * FROM compra LIMIT 1;"
            " UPDATE c2 SET id = 'compra-r11b', numero = numero || '9',"
            " chave = CASE WHEN chave IS NULL THEN NULL ELSE substr(chave, 1, 40) || '9999' END;"
            " INSERT INTO compra SELECT * FROM c2;"
        )}, raiz=base.RAIZ)
        r = Rustinho(pasta, "TelaCompras", {}, fotos, "r11-compras")
        try:
            r.entrar("Ana")
            time.sleep(2)
            xdo("mousemove", "1", "1")
            # O Tab passa pela primeira linha e chega à segunda: ela vira a destacada (o Enter e as setas seguem o foco).
            linhas_vistas = []
            for _ in range(60):
                xdo("key", "Tab")
                time.sleep(0.25)
                foco = [t.split(":", 1)[1] for _, _, t in r.textos() if t.startswith("TESTE_FOCO_LINHA:")]
                foco = foco[0] if foco else ""
                if foco and (not linhas_vistas or linhas_vistas[-1] != foco):
                    linhas_vistas.append(foco)
                if len({f.split(":")[0] for f in linhas_vistas}) >= 2:
                    break
            segunda = [f for f in linhas_vistas if not f.startswith("0:")]
            acompanha = bool(segunda) and segunda[0].endswith(":atual")
            print(("✔ " if acompanha else "✘ ") + "compras: o Tab chega à segunda linha e ela vira a destacada"
                  + ("" if acompanha else f" (vistas: {linhas_vistas}; foto: {r.foto('-b4')})"))
            tudo_certo &= acompanha
            # Volta para a primeira linha (Shift+Tab até ela) para o Enter abrir a nota, como antes.
            na_linha = False
            for _ in range(20):
                if any(t == "TESTE_FOCO:linhaCompra" for _, _, t in r.textos()) \
                        and any(t == "TESTE_FOCO_LINHA:0:atual" for _, _, t in r.textos()):
                    na_linha = True
                    break
                xdo("key", "shift+Tab")
                time.sleep(0.25)
            print(("✔ " if na_linha else "✘ ") + "compras: o Tab chega à linha da compra"
                  + ("" if na_linha else f" (foto: {r.foto('-0')})"))
            tudo_certo &= na_linha
            xdo("key", "Return")
            time.sleep(1.5)
            tx = r.textos()
            abriu = na_linha and any(t.startswith("TESTE_PILHA:") and t != "TESTE_PILHA:1" for _, _, t in tx) \
                and any(t.startswith("NF ") and "·" in t and y < 200 for _, y, t in tx)
            print(("✔ " if abriu else "✘ ") + "compras: o Tab chega à compra e o Enter abre a nota, sem o mouse"
                  + ("" if abriu else f" (foto: {r.foto('-1')})"))
            tudo_certo &= abriu
        finally:
            r.fechar()

        # ---- A3. Fechar o caixa de verdade pelo teclado (B-N3 da revisão R2): na pergunta, o Tab chega ao botão
        # (o primeiro Tab para na seta de desistir) e o Enter ali confirma; o caixa fecha. Por último, porque fecha. ----
        r = Rustinho(pasta, "TelaCaixa", {}, fotos, "r8-fechar-de-verdade")
        try:
            r.entrar("Ana")
            time.sleep(2)
            if not r.clicar("Fechar caixa"):
                raise RuntimeError(f"não achei o botão Fechar caixa (veja {r.saida})")
            time.sleep(1.5)
            xdo("type", "--delay", "60", "150")
            if not r.clicar("Fechar e imprimir"):
                raise RuntimeError(f"não achei o botão Fechar e imprimir (veja {r.saida})")
            time.sleep(1.5)
            xdo("key", "Tab")
            time.sleep(0.3)
            xdo("key", "Tab")
            time.sleep(0.3)
            xdo("key", "Return")
            time.sleep(3)
            ok = caixas_abertos(pasta) == 0
            print(("✔ " if ok else "✘ ") + "caixa: pelo teclado (Tab até o botão e Enter), a confirmação fecha o caixa"
                  + ("" if ok else f" (foto: {r.foto('-1')})"))
            tudo_certo &= ok
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
