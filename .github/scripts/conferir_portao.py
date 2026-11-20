#!/usr/bin/env python3
"""Confere a forma dos workflows (dev.3.3, DP-02a; AUD33-J01, J02, J03): o
portão não pode ser afrouxado sem este teste reprovar. Roda no job "formato"
da CI e à mão:

    python3 .github/scripts/conferir_portao.py

Regras:
1. ci.yml dispara por padrão: push em main, desenvolvimento-*, stabilization/*
   e tags v*; pull_request para main e desenvolvimento-*; workflow_dispatch.
2. ci.yml chama o pipeline único (validar.yml); o "portao" depende dele; o
   "rascunho" depende dos dois, só roda em tag v*, usa o ambiente "publicacao"
   e nenhum dos dois usa always(), failure() ou cancelled().
3. Só o "rascunho" grava no repositório (contents: write); o resto só lê.
4. No validar.yml, o "manifesto" depende de TODOS os outros jobs, e nenhum job
   ignora falha (continue-on-error só no passo do emulador de 32 bits).
5. Nenhum outro workflow publica versão.
6. A chave do Android (T050, 19/11/2026) é uma só e igual nos quatro lugares:
   a entrada `android` do validar.yml (booleana, padrão false), o `android` que
   o ci.yml passa, o RUSTINHO_ANDROID do portão e do rascunho ("sim"/"nao") e o
   do passo do manifesto (tirado da entrada). Só os jobs apk e emulador podem
   ser pulados por ela (if: inputs.android); nenhum outro job do validar.yml
   tem `if`, a não ser o manifesto (always()).

A pasta dos workflows pode vir como argumento (para os testes).
"""
import glob
import os
import re
import sys

import yaml

PASTA = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                                                             "workflows")
JOBS_ANDROID = ("apk", "emulador")
ESCAPES = re.compile(r"always\(\)|failure\(\)|cancelled\(\)")


def carregar(nome):
    with open(os.path.join(PASTA, nome), encoding="utf-8") as f:
        return yaml.safe_load(f)


def gatilhos(w):
    # O YAML 1.1 lê a chave "on" como True.
    return w.get("on", w.get(True)) or {}


def lista(x):
    if x is None:
        return []
    return [x] if isinstance(x, str) else list(x)


def main():
    erros = []

    def exigir(cond, msg):
        if not cond:
            erros.append(msg)

    ci = carregar("ci.yml")
    validar = carregar("validar.yml")
    g = gatilhos(ci)
    push = g.get("push") or {}
    exigir({"main", "desenvolvimento-*", "stabilization/*"} <= set(lista(push.get("branches"))),
           f"ci.yml: push sem os padrões de branch: {push.get('branches')}")
    exigir("v*" in lista(push.get("tags")), f"ci.yml: push sem tags v*: {push.get('tags')}")
    pr = g.get("pull_request") or {}
    exigir({"main", "desenvolvimento-*"} <= set(lista(pr.get("branches"))), f"ci.yml: pull_request: {pr}")
    exigir("workflow_dispatch" in g, "ci.yml: sem workflow_dispatch")

    jobs = ci.get("jobs", {})
    exigir(jobs.get("validar", {}).get("uses") == "./.github/workflows/validar.yml",
           "ci.yml: o job validar não chama ./.github/workflows/validar.yml")
    portao, rascunho = jobs.get("portao", {}), jobs.get("rascunho", {})
    exigir("validar" in lista(portao.get("needs")), "ci.yml: portao não depende de validar")
    exigir({"validar", "portao"} <= set(lista(rascunho.get("needs"))), "ci.yml: rascunho não depende de validar e portao")
    for nome, job in (("portao", portao), ("rascunho", rascunho)):
        exigir(not ESCAPES.search(str(job.get("if", ""))), f"ci.yml: {nome} usa always/failure/cancelled no if")
        exigir("continue-on-error" not in job, f"ci.yml: {nome} com continue-on-error")
    exigir("refs/tags/v" in str(rascunho.get("if", "")), "ci.yml: rascunho sem a condição de tag v*")
    amb = rascunho.get("environment")
    exigir((amb.get("name") if isinstance(amb, dict) else amb) == "publicacao", "ci.yml: rascunho fora do ambiente publicacao")

    # Permissões: só o rascunho grava.
    for arq in sorted(glob.glob(os.path.join(PASTA, "*.yml"))):
        w = carregar(os.path.basename(arq))
        topo = (w.get("permissions") or {})
        exigir(topo.get("contents") in (None, "read"), f"{os.path.basename(arq)}: permissions de topo grava")
        for nome, job in (w.get("jobs") or {}).items():
            p = job.get("permissions") or {}
            grava = p == "write-all" or (isinstance(p, dict) and p.get("contents") == "write")
            if grava:
                exigir(os.path.basename(arq) == "ci.yml" and nome == "rascunho",
                       f"{os.path.basename(arq)}: o job {nome} grava no repositório")
        texto = open(arq, encoding="utf-8").read()
        if os.path.basename(arq) != "ci.yml":
            exigir(not re.search(r"action-gh-release|gh release (create|upload|edit)", texto),
                   f"{os.path.basename(arq)}: publica versão (só o rascunho do ci.yml pode)")

    vjobs = validar.get("jobs", {})
    outros = set(vjobs) - {"manifesto"}
    exigir(set(lista(vjobs.get("manifesto", {}).get("needs"))) == outros,
           f"validar.yml: o manifesto não depende de todos os jobs: falta {outros - set(lista(vjobs.get('manifesto', {}).get('needs')))}")
    for nome, job in vjobs.items():
        exigir("continue-on-error" not in job, f"validar.yml: o job {nome} ignora falha (continue-on-error)")
        if nome != "manifesto":
            exigir(not ESCAPES.search(str(job.get("if", ""))), f"validar.yml: o job {nome} usa always/failure/cancelled")
        for passo in job.get("steps") or []:
            if "continue-on-error" in passo:
                exigir(passo.get("id") == "emulador" and nome == "emulador",
                       f"validar.yml: passo '{passo.get('name')}' do job {nome} ignora falha")
    gatilhos_validar = gatilhos(validar)
    exigir(set(gatilhos_validar) == {"workflow_call"}, f"validar.yml só pode ser chamado (workflow_call): {set(gatilhos_validar)}")

    # 6. A chave do Android.
    entrada = (((gatilhos_validar.get("workflow_call") or {}).get("inputs") or {}).get("android") or {})
    exigir(entrada.get("type") == "boolean" and entrada.get("default") is False,
           f"validar.yml: a entrada android tem de ser booleana com padrão false: {entrada}")
    for nome, job in vjobs.items():
        se = job.get("if")
        if nome in JOBS_ANDROID:
            exigir(se == "inputs.android", f"validar.yml: o job {nome} tem de ter if: inputs.android (tem {se!r})")
        elif nome != "manifesto":
            exigir(se is None, f"validar.yml: o job {nome} não pode ter if (tem {se!r}); só os do Android são pulados")
    passos = [p for p in (vjobs.get("manifesto", {}).get("steps") or []) if p.get("id") == "manifesto"]
    exigir(len(passos) == 1 and (passos[0].get("env") or {}).get("RUSTINHO_ANDROID")
           == "${{ inputs.android && 'sim' || 'nao' }}",
           "validar.yml: o passo do manifesto tem de ter RUSTINHO_ANDROID: ${{ inputs.android && 'sim' || 'nao' }}")
    ligado = (jobs.get("validar", {}).get("with") or {}).get("android")
    exigir(isinstance(ligado, bool), f"ci.yml: o validar tem de passar android: true ou false (passa {ligado!r})")
    for nome, job in (("portao", portao), ("rascunho", rascunho)):
        valor = (job.get("env") or {}).get("RUSTINHO_ANDROID")
        exigir(isinstance(ligado, bool) and valor == ("sim" if ligado else "nao"),
               f"ci.yml: o {nome} tem RUSTINHO_ANDROID {valor!r}, diferente do android: {ligado!r} do validar")

    if erros:
        for e in erros:
            print(f"::error title=Forma do portão::{e}")
        sys.exit(1)
    print(f"conferir_portao: ok ({len(vjobs)} jobs no Validar; rascunho só em tag v*, no ambiente publicacao; "
          f"Android {'ligado' if ligado else 'fora do escopo'})")


if __name__ == "__main__":
    main()
