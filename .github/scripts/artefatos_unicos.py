#!/usr/bin/env python3
"""T031 (26/11/2026): a execução não pode ter dois artefatos válidos com o mesmo nome.

Ao repetir só os jobs que falharam ("Re-run failed jobs"), o GitHub guarda os artefatos da tentativa anterior ao
lado dos novos, com o mesmo nome; o `actions/download-artifact` escolhe um deles pelo maior ID, e o ID não segue a
ordem de criação (execução 45718798615: o Portão leu o Manifesto reprovado da 1ª tentativa). O `overwrite` do
upload-artifact só vale dentro da mesma tentativa (revisão R13 da 007). Então o Portão e o rascunho conferem a
lista ANTES de baixar e reprovam, dizendo o que fazer: rodar a execução inteira de novo.

Entrada: o arquivo com uma linha por artefato, "nome<TAB>id<TAB>criado_em<TAB>expirado" (o `gh api --paginate
.../actions/runs/<id>/artifacts --jq '.artifacts[] | [.name, .id, .created_at, .expired] | @tsv'`).

    python3 .github/scripts/artefatos_unicos.py LISTA.tsv
"""
import collections
import sys


def repetidos(linhas):
    """Os nomes com mais de um artefato não expirado: {nome: [(id, criado_em), ...]}."""
    por_nome = collections.defaultdict(list)
    for linha in linhas:
        linha = linha.rstrip("\n")
        if not linha.strip():
            continue
        partes = linha.split("\t")
        if len(partes) != 4:
            raise ValueError(f"linha fora do formato nome/id/criado_em/expirado: {linha!r}")
        nome, ident, criado, expirado = partes
        if expirado == "true":
            continue
        por_nome[nome].append((ident, criado))
    return {n: sorted(v, key=lambda x: x[1]) for n, v in por_nome.items() if len(v) > 1}


def main():
    with open(sys.argv[1], encoding="utf-8") as f:
        linhas = f.readlines()
    if not any(l.strip() for l in linhas):
        print("::error title=Artefatos::a lista de artefatos da execução veio vazia")
        sys.exit(1)
    rep = repetidos(linhas)
    if rep:
        for nome, v in sorted(rep.items()):
            print(f"::error title=Artefato repetido::{nome}: {len(v)} artefatos com o mesmo nome "
                  f"({', '.join(f'{i} de {c}' for i, c in v)})")
        print("::error title=Artefatos::Esta execução teve jobs repetidos (Re-run failed jobs): os artefatos da "
              "tentativa anterior continuam aqui com o mesmo nome e o Portão não sabe qual ler. Rode a execução "
              "INTEIRA de novo (Run workflow, ou um commit novo); não repita só os jobs que falharam.")
        sys.exit(1)
    print(f"artefatos_unicos: ok ({sum(1 for l in linhas if l.strip())} artefatos, nenhum nome repetido)")


if __name__ == "__main__":
    main()
