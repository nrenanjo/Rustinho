#!/usr/bin/env python3
"""MANIFESTO.json, SHA256SUMS.txt e NOTAS.md de uma execução do pipeline
"Validar" (dev.3.3, DP-02a; PLANO_VALIDACAO_E_PUBLICACAO_DEV33.md §3).

Lê os artefatos da execução (baixados em `pasta`, um subdiretório por
artefato) e o resultado de cada job (`NECESSARIOS`, o `toJSON(needs)` do job
do manifesto). Registra:
- de onde veio: commit, ref, tag, branch, run_id, tentativa, evento;
- conclusão de cada job e de cada camada (L0 a L4; L5 a L7 "não executado");
- contagem de testes por suíte;
- ferramentas e imagens das máquinas;
- SHA-256 de cada pacote (conferido com o que o job que montou anotou);
- versão da raiz, versão do app de Android e versionCode.

Nunca falha por conta própria: quem reprova é o passo seguinte do job, que lê
`aprovado` (para o manifesto sair mesmo quando algo falhou).

    NECESSARIOS='{"formato": {"result": "success"}, ...}' python3 manifesto.py artefatos saida
"""
import glob
import hashlib
import json
import os
import re
import sys

# Camadas do PLANO_VALIDACAO §2 e os jobs que as provam.
CAMADAS = {
    "L0": ("formatação, clippy, compilação", ["formato"]),
    "L1": ("testes do núcleo (debug e release; rede em série), Linux e Windows", ["nucleo-linux", "nucleo-windows"]),
    "L2": ("contratos, dependências, esquema, forma das chamadas, cópias do Android", ["nucleo-linux"]),
    "L3": ("programa e roteiros de tela no Xvfb, com geometria", ["programa-e-telas"]),
    "L4": ("pacotes abertos: AppImage, instalador e portátil no Windows da CI, APK no emulador",
           ["pacote-linux", "pacote-windows", "verificar", "apk", "emulador"]),
}
NAO_EXECUTADAS = {
    "L5": "revisão visual com capturas (responsável técnico, ambiente isolado)",
    "L6": "aparelhos reais: Moto e6i, Windows 10/11, dois computadores (proprietário)",
    "L7": "uso assistido na loja, com dados de mentira (proprietário)",
}
PACOTES = [
    ("Rustinho-Linux", "*.AppImage"),
    ("Rustinho-Windows", "*-Windows-instalador.exe"),
    ("Rustinho-Windows-portatil", "*.zip"),
    ("Rustinho-Android", "*.apk"),
    ("Rustinho-Android-32bits", "*.apk"),
]


def sha256(caminho):
    h = hashlib.sha256()
    with open(caminho, "rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()


def ler(caminho, padrao=""):
    try:
        with open(caminho, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return padrao


def contar_testes(texto):
    """Soma das linhas `test result:` de uma saída do cargo test."""
    soma = {"binarios": 0, "passaram": 0, "falharam": 0, "ignorados": 0}
    for linha in texto.splitlines():
        if linha.startswith("test result:"):
            soma["binarios"] += 1
            for chave, nome in (("passaram", "passed"), ("falharam", "failed"), ("ignorados", "ignored")):
                m = re.search(rf"(\d+) {nome}", linha)
                if m:
                    soma[chave] += int(m.group(1))
    soma["terminou"] = soma["binarios"] > 0
    return soma


def chave_valor(texto):
    return dict(l.split("=", 1) for l in texto.splitlines() if "=" in l)


def montar(pasta, necessarios):
    env = os.environ
    ref = env.get("GITHUB_REF", "")
    m = {
        "formato": 1,
        "origem": {
            "repositorio": env.get("GITHUB_REPOSITORY", ""),
            "commit": env.get("GITHUB_SHA", ""),
            "ref": ref,
            "tag": ref[len("refs/tags/"):] if ref.startswith("refs/tags/") else None,
            "branch": ref[len("refs/heads/"):] if ref.startswith("refs/heads/") else env.get("GITHUB_HEAD_REF") or None,
            "evento": env.get("GITHUB_EVENT_NAME", ""),
            "run_id": env.get("GITHUB_RUN_ID", ""),
            "run_attempt": env.get("GITHUB_RUN_ATTEMPT", ""),
            "workflow": env.get("GITHUB_WORKFLOW_REF", ""),
            "endereco": f"{env.get('GITHUB_SERVER_URL', '')}/{env.get('GITHUB_REPOSITORY', '')}/actions/runs/"
                        f"{env.get('GITHUB_RUN_ID', '')}",
        },
        "jobs": {j: v.get("result", "?") for j, v in sorted(necessarios.items())},
    }
    m["aprovado"] = bool(necessarios) and all(r == "success" for r in m["jobs"].values())

    # Versões: raiz, app de Android e versionCode.
    raiz = re.search(r'^version\s*=\s*"([^"]+)"', ler("Cargo.toml"), re.M)
    android = re.search(r'^version\s*=\s*"([^"]+)"', ler("android/rust/Cargo.toml"), re.M)
    m["versao"] = {"raiz": raiz.group(1) if raiz else None, "android_rust": android.group(1) if android else None}
    for nome in ("Rustinho-Android", "Rustinho-Android-32bits"):
        kv = chave_valor(ler(os.path.join(pasta, nome, "versao.txt")))
        if kv:
            m["versao"][nome] = {"nome": kv.get("VERSAO"), "versionCode": kv.get("CODIGO"),
                                 "assinatura": ler(os.path.join(pasta, nome, "assinatura.txt")).strip(),
                                 "certificado": ler(os.path.join(pasta, nome, "certificado.txt")).strip()}

    # Testes por suíte.
    suites = {}
    for arq in sorted(glob.glob(os.path.join(pasta, "Testes-*", "*.txt"))):
        suites[f"{os.path.basename(os.path.dirname(arq))}/{os.path.basename(arq)}"] = contar_testes(ler(arq))
    programa = os.path.join(pasta, "Telas-Linux", "testes-programa.txt")
    if os.path.exists(programa):
        suites["Telas-Linux/testes-programa.txt"] = contar_testes(ler(programa))
    m["testes"] = suites
    roteiros = {}
    for linha in ler(os.path.join(pasta, "Telas-Linux", "telas-resultado.txt")).splitlines():
        if "\t" in linha:
            nome, resultado = linha.split("\t", 1)
            roteiros[nome] = resultado
    esperadas = [l.strip() for l in ler(os.path.join(pasta, "Telas-Linux", "telas.txt")).splitlines()
                 if "falha ESPERADA" in l]
    m["roteiros_de_tela"] = {"resultado": roteiros, "falhas_esperadas": esperadas}

    # Emulador: o resultado de verdade (o de 32 bits não bloqueia: PEND-18).
    m["emulador"] = {}
    for nome in ("64bits", "32bits"):
        kv = chave_valor(ler(os.path.join(pasta, f"Emulador-{nome}", "resultado.txt")))
        if kv:
            m["emulador"][nome] = kv

    # Pacotes.
    pacotes, problemas = [], []
    for artefato, padrao in PACOTES:
        for arq in sorted(glob.glob(os.path.join(pasta, artefato, padrao))):
            h = sha256(arq)
            nome = os.path.basename(arq)
            anotado = None
            lado = arq + ".sha256"
            if os.path.exists(lado):
                anotado = ler(lado).split()[0].lower()
            elif os.path.exists(os.path.join(pasta, artefato, "arquivo.txt")):
                anotado = ler(os.path.join(pasta, artefato, "arquivo.txt")).split()[0].lower()
            if anotado != h:
                problemas.append(f"{nome}: SHA-256 {h} diferente do anotado pelo job que montou ({anotado})")
            pacotes.append({"artefato": artefato, "arquivo": nome, "bytes": os.path.getsize(arq), "sha256": h,
                            "anotado_no_job": anotado})
    m["pacotes"] = pacotes
    m["problemas"] = problemas
    if problemas:
        m["aprovado"] = False

    # Conferências dos pacotes (o que os jobs registraram).
    m["conferencias"] = {
        "appimage": ler(os.path.join(pasta, "Rustinho-Linux", "conferencia.txt")).strip(),
        "instalador": ler(os.path.join(pasta, "Rustinho-Windows", "instalador.txt")).strip(),
        "dlls": ler(os.path.join(pasta, "Rustinho-Windows", "dlls.txt")).strip(),
        "portatil": ler(os.path.join(pasta, "Rustinho-Windows-portatil", "isolamento.txt")).strip(),
        "verificacao_windows": ler(os.path.join(pasta, "verificacao-windows", "relatorio.txt")).strip(),
        "verificacao_linux": ler(os.path.join(pasta, "verificacao-linux", "relatorio.txt")).strip(),
    }

    # Ferramentas e máquinas.
    m["ferramentas"] = {k[len("FERRAMENTA_"):].lower(): v for k, v in sorted(env.items())
                        if k.startswith("FERRAMENTA_")}
    maquinas = {}
    for arq in sorted(glob.glob(os.path.join(pasta, "Ambiente-*", "ambiente.json"))):
        try:
            a = json.loads(ler(arq))
        except ValueError:
            continue
        maquinas[a.pop("job", os.path.basename(os.path.dirname(arq)))] = a
    m["maquinas"] = maquinas

    # Camadas.
    camadas = {}
    for c, (o_que, jobs) in CAMADAS.items():
        r = [m["jobs"].get(j, "não rodou") for j in jobs]
        camadas[c] = {"o_que": o_que, "jobs": dict(zip(jobs, r)),
                      "situacao": "aprovada" if all(x == "success" for x in r) else "reprovada"}
    ressalvas = [f"APK {n} no emulador {kv.get('resultado')} ({kv.get('observacao')})"
                 for n, kv in m["emulador"].items() if kv.get("resultado") != "aprovado"]
    if ressalvas and camadas["L4"]["situacao"] == "aprovada":
        camadas["L4"]["situacao"] = "aprovada, com ressalva"
        camadas["L4"]["ressalvas"] = ressalvas
    for c, o_que in NAO_EXECUTADAS.items():
        camadas[c] = {"o_que": o_que, "situacao": "não executado"}
    m["camadas"] = camadas
    return m


def notas(m):
    o = m["origem"]
    linhas = [
        f"# Rustinho {m['versao']['raiz']} — notas geradas do manifesto",
        "",
        f"- Commit: `{o['commit']}`" + (f" · tag `{o['tag']}`" if o["tag"] else "") +
        (f" · branch `{o['branch']}`" if o["branch"] else ""),
        f"- Execução: {o['endereco']} (tentativa {o['run_attempt']}, evento `{o['evento']}`)",
        f"- Resultado da execução: **{'APROVADA' if m['aprovado'] else 'REPROVADA'}**",
        "",
        "## Validado automaticamente",
        "",
    ]
    for c in ("L0", "L1", "L2", "L3", "L4"):
        info = m["camadas"][c]
        linhas.append(f"- **{c}** ({info['o_que']}): {info['situacao']}")
        for r in info.get("ressalvas", []):
            linhas.append(f"  - ressalva: {r}")
    linhas += ["", "Contagens:", ""]
    for suite, s in m["testes"].items():
        linhas.append(f"- `{suite}`: {s['passaram']} passaram, {s['falharam']} falharam, "
                      f"{s['ignorados']} ignorados ({s['binarios']} binários)")
    if m["roteiros_de_tela"]["resultado"]:
        ok = sum(1 for r in m["roteiros_de_tela"]["resultado"].values() if r == "ok")
        linhas.append(f"- roteiros de tela: {ok} de {len(m['roteiros_de_tela']['resultado'])} sem falha "
                      f"inesperada; {len(m['roteiros_de_tela']['falhas_esperadas'])} falhas esperadas listadas "
                      "(defeitos já registrados, com passo de correção)")
    for nome, kv in m["emulador"].items():
        # N03: a causa de uma reprovação e as instabilidades do emulador
        # contornadas pelo roteiro (cada uma registrada nas fotos do job).
        extra = ""
        if kv.get("resultado") != "aprovado" and kv.get("causa"):
            extra += f"; causa: {kv['causa']}"
        if kv.get("instabilidades", "0") not in ("", "0"):
            extra += f"; {kv['instabilidades']} instabilidades do emulador contornadas e registradas"
        linhas.append(f"- emulador {nome}: {kv.get('resultado', '?')}{extra}" +
                      (f" — {kv['observacao']}" if kv.get("observacao") else ""))
    linhas += [
        "",
        "## Validado à mão (com evidência)",
        "",
        "- Nada nesta execução. O que for validado à mão entra aqui com a evidência.",
        "",
        "## Não validado",
        "",
    ]
    for c in ("L5", "L6", "L7"):
        linhas.append(f"- **{c}** ({m['camadas'][c]['o_que']}): não executado")
    for nome, kv in m["emulador"].items():
        if kv.get("resultado") != "aprovado":
            linhas.append(f"- APK {nome} no emulador: {kv.get('resultado', '?')} — {kv.get('observacao', '')}")
    linhas += [
        "- O emulador da CI não substitui o Moto e6i; o Windows da CI não tem firewall, SmartScreen e impressora "
        "reais; a troca de dados da CI roda em 127.0.0.1.",
        "",
        "## Arquivos (SHA-256)",
        "",
        "| Arquivo | Bytes | SHA-256 |",
        "|---|---|---|",
    ]
    for p in m["pacotes"]:
        linhas.append(f"| `{p['arquivo']}` | {p['bytes']} | `{p['sha256']}` |")
    if m["problemas"]:
        linhas += ["", "## Problemas", ""] + [f"- {p}" for p in m["problemas"]]
    return "\n".join(linhas) + "\n"


def main():
    pasta, saida = sys.argv[1], sys.argv[2]
    necessarios = json.loads(os.environ.get("NECESSARIOS") or "{}")
    os.makedirs(saida, exist_ok=True)
    m = montar(pasta, necessarios)
    with open(os.path.join(saida, "MANIFESTO.json"), "w", encoding="utf-8") as f:
        json.dump(m, f, ensure_ascii=False, indent=2)
        f.write("\n")
    with open(os.path.join(saida, "SHA256SUMS.txt"), "w", encoding="utf-8") as f:
        for p in m["pacotes"]:
            f.write(f"{p['sha256']}  {p['arquivo']}\n")
    with open(os.path.join(saida, "NOTAS.md"), "w", encoding="utf-8") as f:
        f.write(notas(m))
    reprovados = [j for j, r in m["jobs"].items() if r != "success"]
    print(f"manifesto: {'APROVADO' if m['aprovado'] else 'REPROVADO'}; jobs fora de 'success': {reprovados}; "
          f"{len(m['pacotes'])} pacotes; problemas: {m['problemas']}")
    saida_gh = os.environ.get("GITHUB_OUTPUT")
    if saida_gh:
        with open(saida_gh, "a", encoding="utf-8") as f:
            f.write(f"aprovado={'sim' if m['aprovado'] else 'nao'}\n")
    resumo = os.environ.get("GITHUB_STEP_SUMMARY")
    if resumo:
        with open(resumo, "a", encoding="utf-8") as f:
            f.write(notas(m))


if __name__ == "__main__":
    main()
