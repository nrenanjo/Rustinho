#!/usr/bin/env python3
"""R3 / D-AUD-4 (decisão do proprietário, 25/11/2026): a comissão fecha só até ontem.

Em Financeiro › Comissões, com o período "Este mês" (que termina hoje):
- o botão "Fechar" não aparece, e a tela diz "A comissão fecha só até ontem: as vendas de hoje
  ainda podem entrar.", com o botão "Ver até ontem";
- "Ver até ontem" mostra o período que termina ontem, e aí o "Fechar" aparece (a venda de ontem
  tem comissão).

Usa a loja de teste e o jeito de ler a tela de `telas_d09.py`. A Ana ganha 10% de comissão e a
primeira venda dela passa para ontem (direto no banco da loja de teste, antes de abrir).

    python3 app/testes/telas_r3_comissao.py [pasta para as fotos]
"""

import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
from telas_d09 import TELA, Rustinho, conferir, montar_loja  # noqa: E402

NOTA = "A comissão fecha só até ontem: as vendas de hoje ainda podem entrar."


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r3-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r3-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
        ana = c.execute("SELECT id FROM usuario WHERE nome = 'Ana'").fetchone()[0]
        venda = c.execute(
            "SELECT id FROM venda WHERE vendedor_id = ? AND tipo = 'venda' AND situacao = 'concluida' "
            "ORDER BY rowid LIMIT 1",
            (ana,),
        ).fetchone()
        c.close()
        if not venda:
            raise RuntimeError("a loja de teste não tem venda da Ana para passar para ontem")
        # Pelo núcleo: os gatilhos da troca de dados usam as funções do Rustinho.
        base._aux_nucleo.executar("estavel", "executar_por_fora", {"RUSTINHO_LOJA_TESTE": pasta, "RUSTINHO_SQL": (
            f"UPDATE usuario SET comissao = 1000 WHERE id = '{ana}';"
            f"UPDATE venda SET criado_em = strftime('%Y-%m-%dT%H:%M:%SZ', criado_em, '-1 day') WHERE id = '{venda[0]}';"
        )}, raiz=base.RAIZ)
        r = Rustinho(pasta, "TelaFinanceiro", {"secao": "comissoes"}, fotos, "r3-comissao")
        try:
            r.entrar("Ana")
            time.sleep(2.5)
            tudo_certo &= conferir("comissões, este mês: sem \"Fechar\"; a tela diz que fecha só até ontem",
                                   r.foto("-1"), r.textos(), tem=[NOTA], igual=["Ver até ontem"], sem_igual=["Fechar"])
            if not r.clicar("Ver até ontem"):
                raise RuntimeError(f"não achei o botão Ver até ontem (veja {r.saida})")
            time.sleep(1.5)
            tudo_certo &= conferir("comissões, até ontem: o \"Fechar\" aparece e a nota some", r.foto("-2"), r.textos(),
                                   igual=["Fechar"], nao_tem=[NOTA])
        finally:
            r.fechar()
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("RESULTADO:", "tudo certo" if tudo_certo else "HÁ FALHA")
    return 0 if tudo_certo else 1


if __name__ == "__main__":
    sys.exit(main())
