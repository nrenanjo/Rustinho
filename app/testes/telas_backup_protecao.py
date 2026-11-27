#!/usr/bin/env python3
"""T005 da 008 (E7-4; P-10 e a resposta do proprietário de 26/11/2026): a linha "Proteção contínua" em
Configurações › Backup.

Dois casos, com o Google Drive ligado (backup_nuvem = 1) e sem conexão de verdade com o Google:
- SEM PROVA: nada do que este caixa fez foi conferido no Drive. A linha diz "pendente", e nunca "em dia";
- COM PROVA: o cofre diz que tudo está conferido (backup_lote_conferido_ate acima do registro). A linha diz "em dia".
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
        por_fora(pasta, sql)
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
            ("sem_prova", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_nuvem', '1');",
             PENDENTE, [EM_DIA]),
            ("com_prova", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_nuvem', '1'); "
                          "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_lote_conferido_ate', '999999999');",
             EM_DIA, [PENDENTE]),
            ("sem_cofre", "INSERT OR REPLACE INTO config (chave, valor) VALUES ('backup_nuvem', '0');",
             None, [PENDENTE, EM_DIA]),
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
