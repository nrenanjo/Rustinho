#!/usr/bin/env python3
"""Portão (dev.3.3, DP-02a; AUD33-J01, J02, I06, J08). Roda em TODA execução
do CI, depois do pipeline "Validar", e só roda se todos os jobs dele
passaram (`needs`, sem `always()`). O rascunho de versão depende dele.

Confere, com os artefatos desta mesma execução:
1. o manifesto é desta execução e deste commit, e diz "aprovado";
2. cada pacote baixado tem o SHA-256 do manifesto e do SHA256SUMS.txt;
3. a versão do app de Android é a da raiz (I06) e o versionCode segue a DP-36 (J08);
4. só numa tag `v*`: a tag é `v` + a versão da raiz; a tag aponta para o commit
   desta execução; os APKs têm a assinatura da loja com a impressão digital
   esperada (variável ANDROID_CHAVE_IMPRESSAO).

    python3 .github/scripts/portao.py artefatos
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from versao_android import codigo  # noqa: E402


def sha256(caminho):
    h = hashlib.sha256()
    with open(caminho, "rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()


def main():
    pasta = sys.argv[1]
    env = os.environ
    erros, ok = [], []

    def conferir(condicao, certo, errado):
        (ok if condicao else erros).append(certo if condicao else errado)

    caminho = os.path.join(pasta, "Manifesto", "MANIFESTO.json")
    if not os.path.exists(caminho):
        sys.exit(f"::error title=Portão::sem {caminho}")
    with open(caminho, encoding="utf-8") as f:
        m = json.load(f)
    o = m["origem"]
    conferir(o["commit"] == env["GITHUB_SHA"], f"manifesto do commit {o['commit'][:12]}",
             f"manifesto de outro commit: {o['commit']} (esta execução: {env['GITHUB_SHA']})")
    conferir(o["run_id"] == env["GITHUB_RUN_ID"], f"manifesto da execução {o['run_id']}",
             f"manifesto de outra execução: {o['run_id']} (esta: {env['GITHUB_RUN_ID']})")
    fora = {j: r for j, r in m["jobs"].items() if r != "success"}
    conferir(m["aprovado"] and not fora and m["jobs"], f"{len(m['jobs'])} jobs aprovados",
             f"manifesto não aprovado: {fora or m.get('problemas')}")

    # Pacotes: os arquivos baixados são os do manifesto.
    somas = {}
    with open(os.path.join(pasta, "Manifesto", "SHA256SUMS.txt"), encoding="utf-8") as f:
        for linha in f:
            if linha.strip():
                h, nome = linha.split(None, 1)
                somas[nome.strip()] = h
    conferir(len(m["pacotes"]) >= 5, f"{len(m['pacotes'])} pacotes no manifesto",
             f"faltam pacotes no manifesto: {[p['arquivo'] for p in m['pacotes']]}")
    for p in m["pacotes"]:
        arq = os.path.join(pasta, p["artefato"], p["arquivo"])
        if not os.path.exists(arq):
            erros.append(f"{p['arquivo']} não está nos artefatos desta execução")
            continue
        h = sha256(arq)
        conferir(h == p["sha256"] == somas.get(p["arquivo"]), f"{p['arquivo']}: SHA-256 conferido",
                 f"{p['arquivo']}: SHA-256 {h}, manifesto {p['sha256']}, SHA256SUMS {somas.get(p['arquivo'])}")

    # Versões (I06, J08).
    v = m["versao"]
    conferir(v["raiz"] and v["raiz"] == v["android_rust"], f"versão {v['raiz']} na raiz e no app de Android",
             f"versão da raiz {v['raiz']} ≠ android/rust/Cargo.toml {v['android_rust']} (AUD33-I06)")
    for nome in ("Rustinho-Android", "Rustinho-Android-32bits"):
        apk = v.get(nome)
        if not apk:
            erros.append(f"{nome}: sem versao.txt")
            continue
        esperado = str(codigo(v["raiz"]))
        conferir(apk["nome"] == v["raiz"] and apk["versionCode"] == esperado,
                 f"{nome}: versionName {apk['nome']}, versionCode {apk['versionCode']}",
                 f"{nome}: versionName {apk['nome']} / versionCode {apk['versionCode']} (esperado {v['raiz']} / {esperado})")

    # Só em tag v*: o que a publicação exige.
    ref = env.get("GITHUB_REF", "")
    if ref.startswith("refs/tags/"):
        tag = ref[len("refs/tags/"):]
        conferir(tag == f"v{v['raiz']}", f"tag {tag} = versão da raiz",
                 f"tag {tag} ≠ v{v['raiz']} (versão do Cargo.toml)")
        r = subprocess.run(["git", "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"],
                           capture_output=True, text=True)
        alvo = r.stdout.strip() if r.returncode == 0 else ""
        conferir(alvo == env["GITHUB_SHA"], f"tag aponta para {alvo[:12]}",
                 f"a tag aponta para {alvo or '(tag não encontrada no checkout)'}, não para o commit desta "
                 f"execução {env['GITHUB_SHA']}")
        esperada = "".join(c for c in env.get("ANDROID_CHAVE_IMPRESSAO", "").upper() if c in "0123456789ABCDEF")
        for nome in ("Rustinho-Android", "Rustinho-Android-32bits"):
            apk = v.get(nome) or {}
            conferir(apk.get("assinatura") == "loja" and esperada and apk.get("certificado") == esperada,
                     f"{nome}: assinatura da loja conferida",
                     f"{nome}: assinatura '{apk.get('assinatura')}', certificado {apk.get('certificado')}, "
                     f"esperado {esperada or '(variável ANDROID_CHAVE_IMPRESSAO vazia)'}")
    else:
        ok.append("sem tag: nenhuma versão é preparada nesta execução")

    resumo = ["## Portão", ""] + [f"- ✔ {x}" for x in ok] + [f"- ✘ {x}" for x in erros]
    print("\n".join(resumo))
    if env.get("GITHUB_STEP_SUMMARY"):
        with open(env["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as f:
            f.write("\n".join(resumo) + "\n")
    for e in erros:
        print(f"::error title=Portão::{e}")
    sys.exit(1 if erros else 0)


if __name__ == "__main__":
    main()
