#!/usr/bin/env python3
"""Teste das telas do modo seguro (A3, complemento da 0.9.15).

Com o banco de dados com defeito de integridade, o Rustinho abre em modo
seguro: a faixa vermelha aparece em todas as telas, entrar funciona, e o
arquivo do banco não muda nada durante o
uso do programa (nem o backup automático, nem os avisos, nem o menu gravam).

Usa a mesma loja de teste e o mesmo jeito de ler a tela de `telas_d09.py`.

    python3 app/testes/telas_modo_seguro.py [pasta para as fotos]
"""

import hashlib
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from telas_d09 import PROGRAMA, RAIZ, TELA, Rustinho, conferir, montar_loja, xdo  # noqa: E402


def estragar_lista_livre(db):
    """Estraga a lista de páginas livres: o quick_check acusa, as tabelas
    continuam legíveis (o mesmo defeito do teste do núcleo)."""
    c = sqlite3.connect(db)
    c.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    c.execute("PRAGMA journal_mode = DELETE")
    c.execute("CREATE TABLE lixo_do_teste (x TEXT)")
    c.executemany("INSERT INTO lixo_do_teste VALUES (?)", (("x" * 3000,) for _ in range(200)))
    c.commit()
    c.execute("DROP TABLE lixo_do_teste")
    c.commit()
    c.close()
    dados = bytearray(open(db, "rb").read())
    pagina = int.from_bytes(dados[16:18], "big")
    pagina = 65536 if pagina == 1 else pagina
    tronco = int.from_bytes(dados[32:36], "big")
    inicio = (tronco - 1) * pagina
    dados[inicio + 4:inicio + 8] = (5000).to_bytes(4, "big")
    open(db, "wb").write(dados)
    c = sqlite3.connect(db)
    r = c.execute("PRAGMA quick_check").fetchone()[0]
    c.close()
    assert r != "ok", "o banco devia estar com defeito"


def sha(caminho):
    return hashlib.sha256(open(caminho, "rb").read()).hexdigest()


def montar_loja_v22_com_defeito(pasta):
    """RVI-03: loja de versão anterior (estrutura 22), com defeito, e um backup
    bom dela (montada pelo teste do núcleo `montar_loja_v22_com_defeito`)."""
    env = dict(os.environ, RUSTINHO_LOJA_TESTE=pasta)
    subprocess.run(
        ["cargo", "test", "-q", "-p", "rustinho-nucleo", "--lib", "--",
         "--ignored", "--exact", "testes_rvi03::montar_loja_v22_com_defeito"],
        cwd=RAIZ, env=env, stdout=subprocess.DEVNULL, check=True,
    )


def versao_do_banco(db):
    c = sqlite3.connect(db)
    v = c.execute("PRAGMA user_version").fetchone()[0]
    c.close()
    return v


def banco_antigo(fotos):
    """RVI-03: banco de versão anterior com defeito. O programa não atualiza a
    estrutura nem trata o banco como loja vazia: mostra a entrada, a faixa do
    modo seguro e o aviso no Início, e o arquivo não muda."""
    tudo_certo = True
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-seguro-v22-")
    try:
        montar_loja_v22_com_defeito(pasta)
        db = os.path.join(pasta, "rustinho.db")
        antes = sha(db)
        arquivos_antes = sorted(os.listdir(pasta))
        r = Rustinho(pasta, "TelaInicio", {}, fotos, "seguro-v22")
        try:
            r.entrar("Ana")
            time.sleep(3)
            textos = r.textos()
            tudo_certo &= conferir(
                "banco antigo com defeito: entra (não é loja vazia), faixa e aviso do modo seguro no Início",
                r.foto("-1"), textos,
                tem=["Modo seguro", "o banco de dados tem defeito"],
                nao_tem=["Vamos preparar este"],
            )
        finally:
            r.fechar()
        time.sleep(1)
        # O caminho de recuperação: Configurações › Backup lista o backup bom
        # com o botão de restaurar.
        r = Rustinho(pasta, "TelaConfiguracoes", {"secao": "backup"}, fotos, "seguro-v22-backup")
        try:
            r.entrar("Ana")
            time.sleep(3)
            textos = r.textos()
            botoes = sum(1 for _, _, t in textos if t.strip() == "Restaurar")
            r.foto("-1")
            ok = botoes >= 2
            print(("✔ " if ok else "✘ ") + "banco antigo com defeito: Configurações › Backup lista o backup bom para "
                  f"restaurar ({botoes} textos \"Restaurar\": o título e o botão de cada backup)")
            tudo_certo &= ok
        finally:
            r.fechar()
        time.sleep(2)
        ok = sha(db) == antes
        print(("✔ " if ok else "✘ ") + "banco antigo com defeito: o arquivo do banco não mudou")
        tudo_certo &= ok
        ok = versao_do_banco(db) == 22
        print(("✔ " if ok else "✘ ") + "banco antigo com defeito: a estrutura não foi atualizada (continua 22)")
        tudo_certo &= ok
        novos = [a for a in sorted(os.listdir(pasta)) if a not in arquivos_antes
                 and not a.endswith(("-wal", "-shm", ".trava", ".log"))]
        ok = not any(".antes-v" in a or "falha-migracao" in a or a.endswith(".migracao") for a in novos)
        print(("✔ " if ok else "✘ ") + "banco antigo com defeito: nenhuma cópia nem atualização começada"
              + ("" if ok else f": {novos}"))
        tudo_certo &= ok
    finally:
        shutil.rmtree(pasta, ignore_errors=True)
    return tudo_certo


def main():
    for p in ("Xvfb", "xdotool", "import"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-seguro-")
    os.makedirs(fotos, exist_ok=True)
    pasta = tempfile.mkdtemp(prefix="rustinho-loja-seguro-")
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    tudo_certo = True
    try:
        montar_loja(pasta)
        db = os.path.join(pasta, "rustinho.db")
        estragar_lista_livre(db)
        antes = sha(db)
        backups = os.path.join(pasta, "backups")
        backups_antes = sorted(os.listdir(backups)) if os.path.isdir(backups) else []
        r = Rustinho(pasta, "TelaVender", {}, fotos, "seguro")
        try:
            r.entrar("Ana")
            time.sleep(2)
            tudo_certo &= conferir(
                "modo seguro: a faixa aparece e a tela abre", r.foto("-1"), r.textos(),
                tem=["Modo seguro", "nada é gravado"],
            )
            # Outra tela: a faixa continua.
            xdo("key", "Escape")
            time.sleep(1)
            tudo_certo &= conferir("modo seguro: a faixa continua", r.foto("-3"), r.textos(), tem=["Modo seguro"])
        finally:
            r.fechar()
        time.sleep(2)
        depois = sha(db)
        ok = antes == depois
        print(("✔ " if ok else "✘ ") + "modo seguro: o arquivo do banco não mudou durante o uso")
        tudo_certo &= ok
        backups_depois = sorted(os.listdir(backups)) if os.path.isdir(backups) else []
        novos = [b for b in backups_depois if b not in backups_antes and not b.startswith(".")]
        ok = all("defeito" in b for b in novos)
        print(("✔ " if ok else "✘ ") + "modo seguro: nenhum backup comum novo" + ("" if ok else f": {novos}"))
        tudo_certo &= ok
        tudo_certo &= banco_antigo(fotos)
    finally:
        xvfb.terminate()
        shutil.rmtree(pasta, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if tudo_certo else 1)


if __name__ == "__main__":
    main()
