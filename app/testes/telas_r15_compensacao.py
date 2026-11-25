#!/usr/bin/env python3
"""R15 / R-03 (decisão do proprietário, 25/11/2026, regra 15): a compensação pronta, com um clique do gerente.

Em Configurações › Simultaneidade › Duplicatas › Diferenças entre caixas, uma parcela de R$ 50 do Zé recebida duas
vezes (aqui e no "Caixa R15", sem rede):
- a diferença mostra "Compensação pronta: Estornar o recebimento Nº 902 do Caixa R15 (R$ 50,00)…" e o botão
  "Fazer a compensação";
- o clique pergunta "Fazer a compensação?" com a mesma proposta;
- confirmado (a Ana é gerente), o recebimento que sobrou (o do Caixa R15) é estornado, o outro fica, e o registro
  "conflito.compensar" fica no histórico.

A parcela, os dois recebimentos, o outro caixa e a diferença entram direto no banco da loja de teste, pelo núcleo
(os gatilhos da troca de dados usam as funções do Rustinho), antes de abrir. Usa o jeito de ler a tela de
`telas_d09.py`.

    python3 app/testes/telas_r15_compensacao.py [pasta para as fotos]
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
from telas_d09 import TELA, Rustinho, conferir, montar_loja, simples, xdo  # noqa: E402

AGORA = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
DEPOIS = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+1 second')"


def clicar_ultimo(r, texto):
    """Clica no último texto igual da tela (o de cima: a janelinha vem depois na lista dos textos)."""
    achados = [(x, y) for x, y, t in r.textos_para_clicar() if simples(t) == simples(texto)]
    if not achados:
        return False
    xdo("mousemove", str(achados[-1][0]), str(achados[-1][1]), "click", "1")
    return True


def ler(pasta, sql):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute(sql).fetchone()
    finally:
        c.close()


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r15-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r15-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        ana = ler(pasta, "SELECT id FROM usuario WHERE nome = 'Ana'")[0]
        cliente = ler(pasta, "SELECT id FROM pessoa WHERE nome LIKE 'Zé%' ORDER BY rowid LIMIT 1")
        if not cliente:
            raise RuntimeError("a loja de teste não tem o cliente Zé")
        ze = cliente[0]
        base._aux_nucleo.executar("estavel", "executar_por_fora", {"RUSTINHO_LOJA_TESTE": pasta, "RUSTINHO_SQL": (
            "INSERT INTO sync_par (dispositivo, nome, endereco, visto_em) VALUES ('disp-r15', 'Caixa R15', '', '2026-11-25');"
            "INSERT INTO conta_receber (id, cliente_id, parcela, parcelas, descricao, valor, vencimento, criado_em,"
            f" criado_por, dispositivo) VALUES ('cr-r15', '{ze}', 1, 1, 'Peça R15', 5000, date('now'), {AGORA},"
            f" '{ana}', 'disp-r15-aqui');"
            "INSERT INTO recebimento (id, cliente_id, numero, dispositivo, caixa_nome, forma, valor, usuario_id, criado_em)"
            f" VALUES ('rec-r15-aqui', '{ze}', 901, 'disp-r15-aqui', 'Caixa 1', 'dinheiro', 5000, '{ana}', {AGORA}),"
            f" ('rec-r15-la', '{ze}', 902, 'disp-r15', 'Caixa R15', 'dinheiro', 5000, '{ana}', {DEPOIS});"
            "INSERT INTO recebimento_parcela (id, recebimento_id, conta_id, valor) VALUES"
            " ('rp-r15-aqui', 'rec-r15-aqui', 'cr-r15', 5000), ('rp-r15-la', 'rec-r15-la', 'cr-r15', 5000);"
            "INSERT INTO sync_conflito (tabela, chave, ficou, perdeu, dispositivo, quando, tipo)"
            f" VALUES ('conta_receber', 'cr-r15', '', 'rec-r15-aqui,rec-r15-la', '', {AGORA}, 'pago_a_mais');"
        )}, raiz=base.RAIZ)
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "duplicatas"}, fotos, "r15-compensacao")
        try:
            r.entrar("Ana")
            time.sleep(3.0)
            tx = r.textos()
            tudo_certo &= conferir("a diferença mostra a compensação pronta e o botão", r.foto("-1"), tx,
                                   tem=["Compensação pronta: Estornar o recebimento Nº 902 do Caixa R15 (R$ 50,00)"],
                                   igual=["Fazer a compensação"])
            if not r.clicar("Fazer a compensação"):
                raise RuntimeError(f"não achei o botão Fazer a compensação (veja {r.saida})")
            time.sleep(1.2)
            tudo_certo &= conferir("a pergunta repete a proposta", r.foto("-2"), r.textos(),
                                   tem=["Fazer a compensação?", "Estornar o recebimento Nº 902 do Caixa R15",
                                        "O estorno fica no histórico."])
            # O botão da pergunta tem o mesmo texto do da lista: o último na tela é o da pergunta (por cima).
            if not clicar_ultimo(r, "Fazer a compensação"):
                raise RuntimeError(f"não achei o botão da pergunta (veja {r.saida})")
            fim = time.time() + 8
            while time.time() < fim and ler(pasta, "SELECT estornado_em FROM recebimento WHERE id = 'rec-r15-la'")[0] is None:
                time.sleep(0.3)
            la = ler(pasta, "SELECT estornado_em IS NOT NULL FROM recebimento WHERE id = 'rec-r15-la'")[0]
            aqui = ler(pasta, "SELECT estornado_em IS NULL FROM recebimento WHERE id = 'rec-r15-aqui'")[0]
            registro = ler(pasta, "SELECT COUNT(*) FROM auditoria WHERE acao = 'conflito.compensar'")[0]
            ok = la == 1 and aqui == 1 and registro == 1
            print(("✔" if ok else "✘") + " confirmado: o recebimento que sobrou (Caixa R15) é estornado, o daqui fica e"
                  f" o registro está no histórico (lá estornado={la} aqui ativo={aqui} registro={registro};"
                  f" foto: {r.foto('-3')})", flush=True)
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
