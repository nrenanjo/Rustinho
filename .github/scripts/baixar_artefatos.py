#!/usr/bin/env python3
"""T031 (26/11/2026): baixa os artefatos da execução escolhendo, de cada nome, o MAIS NOVO pela data de criação.

Ao repetir jobs ("Re-run failed jobs" ou "Re-run all jobs"), o GitHub guarda os artefatos das tentativas anteriores ao
lado dos novos, com o mesmo nome. O `actions/download-artifact` escolhe pelo maior ID, e o ID não segue a ordem de
criação (execução 45718798615: o Portão leu o Manifesto reprovado da 1ª tentativa); o `overwrite` do upload só vale
dentro da mesma tentativa (revisões R13 e R14 da 007). Aqui a escolha é pelo `created_at`: o artefato de um job
repetido é sempre o mais novo, e o de um job que não foi repetido é o único.

Uso (no job, com GH_TOKEN, GITHUB_REPOSITORY e GITHUB_RUN_ID; o job precisa de `actions: read`):

    python3 .github/scripts/baixar_artefatos.py DESTINO

Cada artefato vai para DESTINO/<nome>/, como o download-artifact sem nome fazia.
"""
import io
import os
import subprocess
import sys
import zipfile

CONSULTA = ".artifacts[] | [.name, (.id | tostring), .created_at, (.expired | tostring)] | @tsv"


def escolher(linhas):
    """De cada nome não expirado, o mais novo pelo created_at (empate: o maior id).
    Devolve ({nome: id}, {nome: [ids deixados de lado]})."""
    por_nome = {}
    for linha in linhas:
        linha = linha.rstrip("\n")
        if not linha.strip():
            continue
        partes = linha.split("\t")
        if len(partes) != 4 or not partes[0] or not partes[1].isdigit() or not partes[2]:
            raise ValueError(f"linha fora do formato nome/id/criado_em/expirado: {linha!r}")
        nome, ident, criado, expirado = partes
        if expirado == "true":
            continue
        por_nome.setdefault(nome, []).append((criado, int(ident)))
    escolhidos, deixados = {}, {}
    for nome, v in por_nome.items():
        v.sort()
        escolhidos[nome] = v[-1][1]
        if len(v) > 1:
            deixados[nome] = [i for _, i in v[:-1]]
    return escolhidos, deixados


def extrair(dados, destino):
    with zipfile.ZipFile(io.BytesIO(dados)) as z:
        raiz = os.path.realpath(destino)
        for item in z.namelist():
            alvo = os.path.realpath(os.path.join(raiz, item))
            if alvo != raiz and not alvo.startswith(raiz + os.sep):
                raise ValueError(f"caminho fora da pasta no artefato: {item!r}")
        z.extractall(raiz)


def gh(*args):
    return subprocess.run(["gh", "api", *args], check=True, capture_output=True).stdout


def main():
    destino = sys.argv[1]
    repo, execucao = os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_RUN_ID"]
    lista = gh("--paginate", f"repos/{repo}/actions/runs/{execucao}/artifacts", "--jq", CONSULTA).decode()
    escolhidos, deixados = escolher(lista.splitlines())
    if not escolhidos:
        print("::error title=Artefatos::a execução não tem nenhum artefato para baixar")
        sys.exit(1)
    for nome, ids in sorted(deixados.items()):
        print(f"::notice title=Artefato repetido::{nome}: usado o mais novo ({escolhidos[nome]}); "
              f"deixados de lado os das tentativas anteriores: {ids}")
    for nome, ident in sorted(escolhidos.items()):
        pasta = os.path.join(destino, nome)
        os.makedirs(pasta, exist_ok=True)
        extrair(gh(f"repos/{repo}/actions/artifacts/{ident}/zip"), pasta)
        print(f"baixado: {nome} ({ident})")
    print(f"baixar_artefatos: {len(escolhidos)} artefatos em {destino}")


if __name__ == "__main__":
    main()
