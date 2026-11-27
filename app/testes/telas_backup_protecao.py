#!/usr/bin/env python3
"""T005 da 008 (E7-4; P-10 e a resposta do proprietário de 26/11/2026): a linha "Proteção contínua" em
Configurações › Backup.

DRV-BETA (27/11/2026): a 0.9.19 Beta sai sem o Google Drive (ver os casos). Antes, com o Drive:
- SEM PROVA: nada do que este caixa fez foi conferido no Drive. A linha diz "pendente", e nunca "em dia";
- COM PROVA: o cofre diz que tudo está conferido (os lotes conferidos de 0 até acima do registro). A linha diz "em dia".
  É também o controle positivo de que a linha muda com a prova, e não é um texto fixo.
Com o Drive desligado, a linha não aparece (não se fala em proteção sem cofre).

    python3 app/testes/telas_backup_protecao.py [pasta das fotos]
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_c8_rerevisao as rr  # noqa: E402
from telas_navegacao_casos import Placar, montar_loja, por_fora, TELA  # noqa: E402

PENDENTE = "Proteção contínua: pendente"
EM_DIA = "Proteção contínua: em dia"
TENTAR = "Tentar de novo"
VOLTAR = "Voltar aos dados de antes"
SEM_DRIVE = "O Google Drive volta na versão 0.9.20. As cópias continuam neste computador e no pen drive."
INTERROMPIDA = "A restauração não terminou. Nada foi perdido: os dados de antes estão guardados."


def abrir_backup(placar, pasta, fotos, nome):
    r = placar.abrir(pasta, "TelaConfiguracoes", {"secao": "backup"}, fotos, nome)
    r.entrar("Ana")
    rr.senha_gerente(r, "Backup")
    return r


def caso(placar, fotos, nome, sql, deve, nao_deve):
    ok = placar.ok
    pasta = tempfile.mkdtemp(prefix=f"rustinho-backup-protecao-{nome}-")
    try:
        montar_loja(pasta)
        if "{ANTES}" in sql:
            # A cópia de antes da restauração (só o nome e o arquivo existindo contam para o botão).
            antes = os.path.join(pasta, "rustinho-20261126-100000-caixa1.rbk")
            open(antes, "wb").close()
            sql = sql.replace("{ANTES}", antes)
        por_fora(pasta, sql.replace("{PASTA}", pasta))
        r = abrir_backup(placar, pasta, fotos, nome)
        try:
            # A tela de Backup aberta (a linha "No computador:" sempre aparece): só então o "não diz" vale.
            abriu = r.esperar(lambda: r.tem("No computador:"), 20)
            if not ok(f"{nome}: a tela de Backup abriu", abriu, r.foto("-0")):
                return
            achou = r.esperar(lambda: (deve is None or r.tem(deve)), 10)
            f = r.foto("-1")
            if deve:
                ok(f"{nome}: a linha diz \"{deve}\"", achou, f)
            if nome == "interrompida":
                ok(f"{nome}: só \"Tentar de novo\" (sem troca)", r.tem(TENTAR) and not r.tem(VOLTAR), f)
            if nome == "interrompida_trocada":
                ok(f"{nome}: os dois botões aparecem", r.tem(TENTAR) and r.tem(VOLTAR), f)
            for t in nao_deve:
                ok(f"{nome}: a linha não diz \"{t}\"", not r.tem(t), f)
        finally:
            r.fechar()
    finally:
        shutil.rmtree(pasta, ignore_errors=True)


def main():
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-backup-protecao-")
    os.makedirs(fotos, exist_ok=True)
    if subprocess.run(["xdpyinfo", "-display", TELA], capture_output=True).returncode == 0:
        sys.exit(f"a tela virtual {TELA} já está em uso")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    placar = Placar()
    try:
        casos = [
            # DRV-BETA (decisão do proprietário de 27/11/2026): a Beta sai sem o Google Drive. A loja que ligou o Drive
            # numa versão anterior (com prova ou sem) não vê a proteção contínua, e vê que o Drive volta na 0.9.20.
            # As linhas "pendente" e "em dia" continuam no código, testadas no núcleo; a prova delas na tela volta
            # com o Drive, na 0.9.20.
            ("drive_de_antes", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_nuvem', '1'); "
                               "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_lote_conferido_ate', '999999999'); "
                               "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_lote_inicio', '0');",
             SEM_DRIVE, [PENDENTE, EM_DIA, "Conectar ao Google Drive"]),
            ("sem_cofre", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_nuvem', '0');",
             SEM_DRIVE, [PENDENTE, EM_DIA, TENTAR, VOLTAR]),
            # M6 da R1 e M1 da R2 da 008 (ES-3 (a)): a restauração interrompida ANTES da troca oferece só "Tentar de
            # novo" (voltar desfaria o trabalho de depois da queda); DEPOIS da troca (terminou ao abrir), os dois
            # botões, com a hora da cópia de antes. O sem_cofre é o controle (nenhum botão).
            ("interrompida", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_restauracao_interrompida', "
                             "json_object('terminouAgora', json('false'), 'quando', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), "
                             "'arquivo', '{PASTA}/rustinho.db', 'nuvem', json('false'), "
                             "'copia', strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 day'), 'antes', '{ANTES}'));",
             INTERROMPIDA, [VOLTAR]),
            ("interrompida_trocada", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_restauracao_interrompida', "
                                     "json_object('terminouAgora', json('true'), 'quando', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), "
                                     "'arquivo', '{PASTA}/rustinho.db', 'nuvem', json('false'), "
                                     "'copia', strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 day'), 'antes', '{ANTES}'));",
             "desfaz o que foi feito depois de 26/11/2026 10:00", []),
        ]
        for nome, sql, deve, nao_deve in casos:
            try:
                caso(placar, fotos, nome, sql, deve, nao_deve)
            except Exception as e:
                placar.ok(f"{nome} chegou ao fim ({e!r})", False)
    finally:
        xvfb.terminate()
        try:
            xvfb.wait(10)
        except subprocess.TimeoutExpired:
            xvfb.kill()
    print(f"RESULTADO: {placar.certos} certos, {placar.errados} errados")
    sys.exit(0 if placar.errados == 0 else 1)


if __name__ == "__main__":
    main()
