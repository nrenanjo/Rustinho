#!/usr/bin/env python3
"""T031 (26/11/2026): baixa os artefatos da execução escolhendo, de cada nome, o MAIS NOVO pela data de criação.

Ao repetir jobs ("Re-run failed jobs" ou "Re-run all jobs"), o GitHub guarda os artefatos das tentativas anteriores ao
lado dos novos, com o mesmo nome. O `actions/download-artifact` escolhe pelo maior ID, e o ID não segue a ordem de
criação (execução 45718798615: o Portão leu o Manifesto reprovado da 1ª tentativa); o `overwrite` do upload só vale
dentro da mesma tentativa (revisões R13 a R15 da 007). Aqui a escolha é pelo `created_at`: o artefato de um job
repetido é sempre o mais novo, e o de um job que não foi repetido é o único.

Uso (com GH_TOKEN e GITHUB_REPOSITORY; o job precisa de `actions: read`):

    python3 .github/scripts/baixar_artefatos.py DESTINO                 # todos, em DESTINO/<nome>/
    python3 .github/scripts/baixar_artefatos.py DESTINO --nome N [--execucao ID]   # só N, direto em DESTINO/

Sem `--execucao` (ou com ele vazio), a execução é a atual (GITHUB_RUN_ID). Confere o SHA-256 de cada zip com o
`digest` que a API informa, tenta de novo em erro passageiro e mostra o erro do `gh` quando falha.
"""
import argparse
import hashlib
import io
import os
import subprocess
import sys
import time
import zipfile

CONSULTA = ".artifacts[] | [.name, (.id | tostring), .created_at, (.expired | tostring), (.digest // \"\")] | @tsv"
TENTATIVAS = 3
ESPERA = float(os.environ.get("BAIXAR_ARTEFATOS_ESPERA", "5"))  # segundos (os testes usam 0)
TEMPO = float(os.environ.get("BAIXAR_ARTEFATOS_TEMPO", "600"))  # limite de cada chamada do gh, em segundos


def escolher(linhas):
    """De cada nome não expirado, o mais novo pelo created_at (empate: o maior id).
    Devolve ({nome: (id, digest)}, {nome: [ids deixados de lado]})."""
    por_nome = {}
    for linha in linhas:
        linha = linha.rstrip("\n")
        if not linha.strip():
            continue
        partes = linha.split("\t")
        if len(partes) == 4:
            partes.append("")
        if len(partes) != 5:
            raise ValueError(f"linha fora do formato nome/id/criado_em/expirado/digest: {linha!r}")
        nome, ident, criado, expirado, digest = partes
        if expirado == "true":
            continue
        if not nome or not ident.isdigit() or not criado:
            raise ValueError(f"linha fora do formato nome/id/criado_em/expirado/digest: {linha!r}")
        por_nome.setdefault(nome, []).append((criado, int(ident), digest))
    escolhidos, deixados = {}, {}
    for nome, v in por_nome.items():
        v.sort()
        escolhidos[nome] = (v[-1][1], v[-1][2])
        if len(v) > 1:
            deixados[nome] = [i for _, i, _ in v[:-1]]
    return escolhidos, deixados


def _nome_no_zip(info):
    # Sem o sinal UTF-8 (bit 11), o zipfile lê o nome como cp437; o download-artifact sempre lê UTF-8.
    if info.flag_bits & 0x800:
        return info.filename
    try:
        return info.filename.encode("cp437").decode("utf-8")
    except (UnicodeEncodeError, UnicodeDecodeError):
        return info.filename


def extrair(dados, destino):
    raiz = os.path.realpath(destino)
    os.makedirs(raiz, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(dados)) as z:
        itens = [(i, _nome_no_zip(i)) for i in z.infolist()]
        for _, nome in itens:
            alvo = os.path.realpath(os.path.join(raiz, nome))
            if alvo != raiz and not alvo.startswith(raiz + os.sep):
                raise ValueError(f"caminho fora da pasta no artefato: {nome!r}")
        for info, nome in itens:
            alvo = os.path.join(raiz, nome)
            if nome.endswith("/"):
                os.makedirs(alvo, exist_ok=True)
                continue
            os.makedirs(os.path.dirname(alvo), exist_ok=True)
            with z.open(info) as origem, open(alvo, "wb") as f:
                f.write(origem.read())


def gh(*args):
    for tentativa in range(1, TENTATIVAS + 1):
        try:
            r = subprocess.run(["gh", "api", *args], capture_output=True, timeout=TEMPO)
        except subprocess.TimeoutExpired:
            erro = f"passou de {TEMPO:.0f} s sem terminar"
        else:
            if r.returncode == 0:
                return r.stdout
            erro = r.stderr.decode(errors="replace").strip()
        print(f"::warning title=gh api::tentativa {tentativa} de {TENTATIVAS} falhou: {' '.join(args[:2])}: {erro}")
        if tentativa < TENTATIVAS:
            time.sleep(ESPERA * tentativa)
    print(f"::error title=gh api::não consegui {' '.join(args[:2])}: {erro}")
    sys.exit(1)


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("destino")
    ap.add_argument("--nome", default="")
    ap.add_argument("--execucao", default="")
    a = ap.parse_args(argv)
    repo = os.environ["GITHUB_REPOSITORY"]
    execucao = a.execucao or os.environ["GITHUB_RUN_ID"]
    lista = gh("--paginate", f"repos/{repo}/actions/runs/{execucao}/artifacts", "--jq", CONSULTA).decode()
    escolhidos, deixados = escolher(lista.splitlines())
    if a.nome:
        if a.nome not in escolhidos:
            print(f"::error title=Artefatos::a execução {execucao} não tem o artefato {a.nome!r}")
            sys.exit(1)
        escolhidos = {a.nome: escolhidos[a.nome]}
    if not escolhidos:
        print(f"::error title=Artefatos::a execução {execucao} não tem nenhum artefato para baixar")
        sys.exit(1)
    for nome, ids in sorted(deixados.items()):
        if nome in escolhidos:
            print(f"::notice title=Artefato repetido::{nome}: usado o mais novo ({escolhidos[nome][0]}); "
                  f"deixados de lado os das tentativas anteriores: {ids}")
    for nome, (ident, digest) in sorted(escolhidos.items()):
        dados = gh(f"repos/{repo}/actions/artifacts/{ident}/zip")
        if digest.startswith("sha256:") and hashlib.sha256(dados).hexdigest() != digest[len("sha256:"):]:
            print(f"::error title=Artefatos::{nome} ({ident}): o zip baixado não bate com o digest da API")
            sys.exit(1)
        extrair(dados, a.destino if a.nome else os.path.join(a.destino, nome))
        print(f"baixado: {nome} ({ident})")
    print(f"baixar_artefatos: {len(escolhidos)} artefato(s) em {a.destino}")


if __name__ == "__main__":
    main()
