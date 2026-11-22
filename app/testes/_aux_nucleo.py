#!/usr/bin/env python3
"""Auxiliar de execução dos binários de teste do núcleo usados pelos roteiros de tela (T112).

Quando os roteiros rodam em paralelo (CachyOS ou CI), invocar `cargo test` a
cada montagem de loja ou `por_fora` disputa a trava global de `target/` e
pode até disparar uma recompilação surpresa no meio dos testes.

Se o orquestrador já compilou os binários (`cargo test --no-run`) e exportou:
  - RUSTINHO_BIN_ESTAVEL        (alvo `--test estavel`, debug)
  - RUSTINHO_BIN_LOJAS          (alvo `--test lojas_de_teste`, debug)
  - RUSTINHO_BIN_LOJAS_RELEASE  (alvo `--test lojas_de_teste`, release)
  - RUSTINHO_BIN_NUCLEO_LIB     (alvo `--lib` de `rustinho-nucleo`, debug)
este módulo executa o binário diretamente, sem trava nem overhead do Cargo.
Fora do orquestrador (execução manual de um roteiro isolado), cai de volta
para o comando `cargo test` tradicional, mantendo 100% de compatibilidade.
"""
import json
import os
import subprocess

RAIZ_PADRAO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

VAR_BINARIO = {
    ("estavel", False): "RUSTINHO_BIN_ESTAVEL",
    ("lojas_de_teste", False): "RUSTINHO_BIN_LOJAS",
    ("lojas_de_teste", True): "RUSTINHO_BIN_LOJAS_RELEASE",
    ("lib", False): "RUSTINHO_BIN_NUCLEO_LIB",
}


def executar(alvo, filtro, env_extra=None, release=False, raiz=None, offline=False):
    """Executa um teste `--ignored --exact` de `rustinho-nucleo`."""
    raiz = raiz or RAIZ_PADRAO
    env = dict(os.environ)
    if offline:
        env["CARGO_NET_OFFLINE"] = "true"
    if env_extra:
        env.update(env_extra)
    filtros = [filtro] if isinstance(filtro, str) else list(filtro)
    var = VAR_BINARIO.get((alvo, bool(release)))
    binario = env.get(var, "") if var else ""
    if binario and os.path.isfile(binario) and os.access(binario, os.X_OK):
        cmd = [binario, "--ignored", "--exact", *filtros]
    else:
        cmd = ["cargo", "test"]
        if release:
            cmd.append("--release")
        cmd += ["-q", "-p", "rustinho-nucleo"]
        if alvo == "lib":
            cmd.append("--lib")
        else:
            cmd += ["--test", alvo]
        cmd += ["--", "--ignored", "--exact", *filtros]
    subprocess.run(cmd, cwd=raiz, env=env, stdout=subprocess.DEVNULL, check=True)


def descobrir_binarios(raiz=None, env=None, incluir_release_lojas=True):
    """Compila (`--no-run`) e devolve `{VAR_ENV: caminho}` dos binários auxiliares do núcleo."""
    raiz = raiz or RAIZ_PADRAO
    ambiente = dict(os.environ if env is None else env)
    achados = {}

    def coletar(args, mapa_nomes):
        p = subprocess.run(
            ["cargo", "test", "--no-run", "--message-format=json", *args],
            cwd=raiz, env=ambiente, capture_output=True, text=True, check=True,
        )
        for linha in p.stdout.splitlines():
            if not linha.startswith("{"):
                continue
            try:
                msg = json.loads(linha)
            except ValueError:
                continue
            if msg.get("reason") != "compiler-artifact":
                continue
            exe = msg.get("executable")
            if not exe:
                continue
            alvo = msg.get("target") or {}
            nome = alvo.get("name")
            kinds = alvo.get("kind") or []
            if "lib" in kinds and "lib" in mapa_nomes:
                achados[mapa_nomes["lib"]] = exe
            elif nome in mapa_nomes:
                achados[mapa_nomes[nome]] = exe

    coletar(
        ["-p", "rustinho-nucleo", "--lib", "--test", "estavel", "--test", "lojas_de_teste"],
        {"lib": "RUSTINHO_BIN_NUCLEO_LIB", "estavel": "RUSTINHO_BIN_ESTAVEL", "lojas_de_teste": "RUSTINHO_BIN_LOJAS"},
    )
    if incluir_release_lojas:
        coletar(
            ["--release", "-p", "rustinho-nucleo", "--test", "lojas_de_teste"],
            {"lojas_de_teste": "RUSTINHO_BIN_LOJAS_RELEASE"},
        )
    return achados
