#!/usr/bin/env python3
"""R19 / DP-18 (decisão do proprietário, 25/11/2026, regra 19): só o gerente desfaz a devolução de um cheque,
com registro.

Em Financeiro › Cheques › Todos, um cheque devolvido (depositado no "Banco R19", com a dívida lançada no Receber):
- a linha do cheque devolvido tem o botão "Desfazer";
- o clique pergunta "Desfazer a devolução do cheque?", diz que o cheque volta a ficar depositado no banco e que a
  saída da conta e a dívida são desfeitas;
- confirmado (a Ana é gerente), o cheque volta a "depositado", a saída da conta é estornada, a dívida é cancelada
  e o registro "cheque.desfazer_devolucao" fica no histórico.

O cheque, o banco, os dois movimentos e a dívida entram direto no banco da loja de teste, pelo núcleo (os gatilhos
da troca de dados usam as funções do Rustinho), antes de abrir. Usa o jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_r19_cheque.py [pasta para as fotos]
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

AGORA = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
PERGUNTA = "Desfazer a devolução do cheque?"


def ler(pasta, sql):
    c = sqlite3.connect(os.path.join(pasta, "rustinho.db"))
    try:
        return c.execute(sql).fetchone()
    finally:
        c.close()


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="fotos-r19-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-r19-")
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
            "INSERT INTO conta_loja (id, nome, tipo, saldo_inicial, ativo, ordem, criado_em, atualizado_em)"
            f" VALUES ('cl-r19', 'Banco R19', 'banco', 0, 1, 9, {AGORA}, {AGORA});"
            "INSERT INTO cheque (id, cliente_id, emitente, banco, numero, valor, bom_para, situacao, situacao_em,"
            " situacao_por, conta_loja_id, observacoes, criado_em, criado_por, dispositivo)"
            f" VALUES ('ch-r19', '{ze}', 'Emitente R19', 'Itaú', '000777', 25000, date('now'), 'devolvido', {AGORA},"
            f" '{ana}', 'cl-r19', 'Devolvido: Sem fundos', {AGORA}, '{ana}', 'disp-r19');"
            "INSERT INTO movimento_conta (id, conta_id, data, valor, tipo, descricao, origem_id, usuario_id, criado_em,"
            " dispositivo) VALUES"
            f" ('mv-r19-dep', 'cl-r19', date('now'), 25000, 'deposito_cheque', 'Depósito do cheque nº 000777',"
            f" 'ch-r19', '{ana}', {AGORA}, 'disp-r19'),"
            f" ('mv-r19-dev', 'cl-r19', date('now'), -25000, 'cheque_devolvido',"
            f" 'Cheque nº 000777 devolvido · Sem fundos', 'ch-r19', '{ana}', {AGORA}, 'disp-r19');"
            "INSERT INTO conta_receber (id, cliente_id, parcela, parcelas, descricao, valor, vencimento, criado_em,"
            f" criado_por, dispositivo) VALUES ('cr-r19', '{ze}', 1, 1, 'Cheque nº 000777 devolvido', 25000,"
            f" date('now'), {AGORA}, '{ana}', 'disp-r19');"
        )}, raiz=base.RAIZ)
        r = Rustinho(pasta, "TelaFinanceiro", {"secao": "cheques"}, fotos, "r19-cheque")
        try:
            r.entrar("Ana")
            time.sleep(2.5)
            if not r.clicar("Todos"):
                raise RuntimeError(f"não achei o filtro Todos (veja {r.saida})")
            time.sleep(1.5)
            if not r.clicar("Emitente R19", comeca=True):
                raise RuntimeError(f"não achei a linha do cheque devolvido (veja {r.saida})")
            time.sleep(1.0)
            tx = r.textos()
            tudo_certo &= conferir("cheque devolvido: a linha tem o botão Desfazer", r.foto("-1"), tx,
                                   igual=["Desfazer"])
            if not r.clicar("Desfazer"):
                raise RuntimeError(f"não achei o botão Desfazer na linha do cheque devolvido (veja {r.saida})")
            time.sleep(1.2)
            tudo_certo &= conferir(
                "a pergunta diz o que volta: depositado no banco, a saída da conta e a dívida desfeitas",
                r.foto("-2"), r.textos(),
                tem=[PERGUNTA, "volta a ficar depositado em Banco R19",
                     "A saída da conta e a dívida lançada no Receber são desfeitas. Só o gerente pode."],
                igual=["Desfazer a devolução"])
            if not r.clicar("Desfazer a devolução"):
                raise RuntimeError(f"não achei o botão Desfazer a devolução (veja {r.saida})")
            fim = time.time() + 8
            while time.time() < fim and ler(pasta, "SELECT situacao FROM cheque WHERE id = 'ch-r19'")[0] != "depositado":
                time.sleep(0.3)
            situacao = ler(pasta, "SELECT situacao FROM cheque WHERE id = 'ch-r19'")[0]
            estorno = ler(pasta, "SELECT estornado_em IS NOT NULL FROM movimento_conta WHERE id = 'mv-r19-dev'")[0]
            divida = ler(pasta, "SELECT cancelado_em IS NOT NULL FROM conta_receber WHERE id = 'cr-r19'")[0]
            registro = ler(pasta, "SELECT COUNT(*) FROM auditoria WHERE acao = 'cheque.desfazer_devolucao'"
                                  " AND entidade_id = 'ch-r19'")[0]
            ok = situacao == "depositado" and estorno == 1 and divida == 1 and registro == 1
            print(("✔" if ok else "✘") + " confirmado: o cheque volta a depositado, a saída é estornada, a dívida sai"
                  f" e fica o registro (situação={situacao} estorno={estorno} dívida cancelada={divida}"
                  f" registro={registro}; foto: {r.foto('-3')})", flush=True)
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
