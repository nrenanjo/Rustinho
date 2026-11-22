#!/usr/bin/env python3
"""Validador local paralelo no CachyOS para o Rustinho (T112).

Executa localmente TUDO o que não depende de Windows, preservando integralmente
a cobertura, a segurança e o rigor da CI:
  - L0: cargo fmt --all --check, cargo clippy (-D correctness -D suspicious),
        conferir_portao.py, versao_android.py --testar, testar_portao.py
  - L1/L2: cargo test (debug + release em paralelo na parte sem rede) + testes
           de rede serializados entre perfis (`--test-threads=1` sob trava
           mútua no host; `--isolar-rede-nucleo` opcional para `unshare -rn`)
  - L3: cargo test -p rustinho, todos os `app/testes/qml_*.py` e TODOS os 52
        roteiros de tela (69 unidades LPT, com os 4 gigantes fatiados) em pool
        paralelo de N workers (display Xvfb, HOME, XDG_*, TMPDIR e rede via
        `unshare -rn` exclusivos por worker, evitando disputa das portas
        47810/47811 e protegendo o Rustinho real da loja)

Todas as camadas (L0, L1/L2, L3) executam com `HOME`, `XDG_*` e `TMPDIR`
isolados em diretório temporário dedicado (preservando `CARGO_HOME` e
`RUSTUP_HOME` reais), sem tocar na configuração ou nos dados reais do usuário.

Uso:
    python3 scripts/validar_local.py
    python3 scripts/validar_local.py --camadas L0,L1,L2,L3 --workers 6
    python3 scripts/validar_local.py --saida /tmp/val_local --so-telas
"""
import argparse
import concurrent.futures
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from datetime import datetime, timezone

RAIZ_PADRAO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TESTES_DE_REDE = ["rede_local::", "sync::", "testes_supervisao::"]
PULAR_REDE = ["--skip", "rede_local::", "--skip", "sync::", "--skip", "testes_supervisao::"]


def detectar_linker_env(env):
    """Ativa `mold` (ou `lld`) se disponível e RUSTFLAGS não tiver linker explícito."""
    rf = env.get("RUSTFLAGS", "")
    if "-fuse-ld=" in rf or "linker=" in rf:
        return env, "custom"
    if shutil.which("mold"):
        env["RUSTFLAGS"] = (rf + " -C link-arg=-fuse-ld=mold").strip()
        return env, "mold"
    if shutil.which("lld"):
        env["RUSTFLAGS"] = (rf + " -C link-arg=-fuse-ld=lld").strip()
        return env, "lld"
    return env, "padrao"


def detectar_qmake(env):
    if env.get("QMAKE") and os.path.exists(env["QMAKE"]):
        return env["QMAKE"]
    for candidato in ("qmake6", "qmake"):
        p = shutil.which(candidato)
        if p:
            return p
    return ""


def detectar_qmltestrunner(env):
    if env.get("RUSTINHO_QMLTESTRUNNER") and os.path.exists(env["RUSTINHO_QMLTESTRUNNER"]):
        return env["RUSTINHO_QMLTESTRUNNER"]
    # Prioriza explicitamente Qt 6 (em Arch/CachyOS `/usr/bin/qmltestrunner` pode ser Qt 5).
    for candidato in (
        "/usr/lib/qt6/bin/qmltestrunner",
        "/usr/lib64/qt6/bin/qmltestrunner",
        shutil.which("qmltestrunner6"),
        shutil.which("qmltestrunner"),
        "/usr/bin/qmltestrunner",
    ):
        if candidato and os.path.exists(candidato):
            return candidato
    return ""


def unshare_rede_ok():
    if not shutil.which("unshare"):
        return False
    try:
        r = subprocess.run(["unshare", "-rn", "sh", "-c", "ip link set lo up"], capture_output=True, timeout=5)
        return r.returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def rodar_cmd(nome, cmd, cwd, env, log_path, isolar_rede=False, timeout=None):
    t0 = time.monotonic()
    final_cmd = cmd
    if isolar_rede and unshare_rede_ok():
        final_cmd = ["unshare", "-rn", "sh", "-c", 'ip link set lo up && exec "$@"', "_", *cmd]
    os.makedirs(os.path.dirname(log_path), exist_ok=True)
    estourou = False
    codigo = -1
    try:
        with open(log_path, "w", encoding="utf-8", errors="replace") as lf:
            p = subprocess.run(final_cmd, cwd=cwd, env=env, stdout=lf, stderr=subprocess.STDOUT, timeout=timeout)
            codigo = p.returncode
    except subprocess.TimeoutExpired:
        estourou = True
        with open(log_path, "a", encoding="utf-8") as lf:
            lf.write(f"\n[ERRO] timeout de {timeout}s em {nome}\n")
    dt = round(time.monotonic() - t0, 2)
    ok = codigo == 0 and not estourou
    print(f"  [{'✔' if ok else '✘'}] {nome} ({dt:.1f}s)", flush=True)
    return {"nome": nome, "ok": ok, "codigo": codigo, "timeout": estourou, "segundos": dt, "log": log_path}


def contar_cargo_test(log_path):
    soma = {"binarios": 0, "passaram": 0, "falharam": 0, "ignorados": 0}
    try:
        with open(log_path, encoding="utf-8", errors="replace") as f:
            texto = f.read()
    except OSError:
        return soma
    for linha in texto.splitlines():
        if linha.startswith("test result:"):
            soma["binarios"] += 1
            for k, tok in (("passaram", "passed"), ("falharam", "failed"), ("ignorados", "ignored")):
                m = re.search(rf"(\d+) {tok}", linha)
                if m:
                    soma[k] += int(m.group(1))
    return soma


def info_git(raiz):
    def g(*a):
        r = subprocess.run(["git", "-C", raiz, *a], capture_output=True, text=True)
        return r.stdout.strip() if r.returncode == 0 else ""
    return {
        "commit": g("rev-parse", "HEAD"),
        "branch": g("rev-parse", "--abbrev-ref", "HEAD"),
        "alteracoes_nao_commitadas": len([l for l in g("status", "--short").splitlines() if l.strip()]),
    }


def main():
    cpus = os.cpu_count() or 4
    workers_padrao = max(2, min(6, cpus // 2))

    ap = argparse.ArgumentParser(description="Validação local paralela do Rustinho (CachyOS / Linux).")
    ap.add_argument("--raiz", default=RAIZ_PADRAO, help="Raiz do repositório/worktree a validar.")
    ap.add_argument("--saida", default=None, help="Diretório de logs, fotos e resumo.")
    ap.add_argument("--camadas", default="L0,L1,L2,L3", help="Camadas a executar (padrão: L0,L1,L2,L3).")
    ap.add_argument("--workers", type=int, default=workers_padrao, help=f"Workers paralelos nas telas (padrão: {workers_padrao}).")
    ap.add_argument("--isolamento", choices=("auto", "unshare", "display"), default="auto")
    ap.add_argument("--target-dir", default=None, help="CARGO_TARGET_DIR opcional para reaproveitar cache de compilação.")
    ap.add_argument("--sem-release", action="store_true", help="Pula testes do núcleo em release (só para iteração rápida; NÃO vale como validação completa).")
    ap.add_argument("--isolar-rede-nucleo", action="store_true", help="Envolve testes de rede do núcleo em `unshare -rn` (atenção: oculta interfaces físicas não-loopback exigidas por `rede_local::t05`).")
    ap.add_argument("--so-telas", action="store_true", help="Atalho para `--camadas L3` (programa + QML + 52 roteiros).")
    args = ap.parse_args()

    raiz = os.path.abspath(args.raiz)
    camadas = {"L3"} if args.so_telas else {c.strip().upper() for c in args.camadas.split(",") if c.strip()}
    saida = os.path.abspath(args.saida) if args.saida else tempfile.mkdtemp(prefix="rustinho-validar-local-")
    os.makedirs(saida, exist_ok=True)
    logs = os.path.join(saida, "logs")
    os.makedirs(logs, exist_ok=True)

    sys.path.insert(0, os.path.join(raiz, ".github", "scripts"))
    sys.path.insert(0, os.path.join(raiz, "app", "testes"))
    import particionar_telas  # noqa: E402
    import _aux_nucleo  # noqa: E402

    env = dict(os.environ)
    home_orig = env.get("HOME") or os.path.expanduser("~")
    env.setdefault("CARGO_HOME", os.path.join(home_orig, ".cargo"))
    env.setdefault("RUSTUP_HOME", os.path.join(home_orig, ".rustup"))
    env["CARGO_TERM_COLOR"] = "never"
    if args.target_dir:
        env["CARGO_TARGET_DIR"] = os.path.abspath(args.target_dir)
    env, linker = detectar_linker_env(env)
    qmake = detectar_qmake(env)
    if qmake:
        env["QMAKE"] = qmake
    qmlrunner = detectar_qmltestrunner(env)
    if qmlrunner:
        env["RUSTINHO_QMLTESTRUNNER"] = qmlrunner

    # Isola HOME / XDG_* / TMPDIR em diretório temporário dedicado para L0, L1/L2,
    # L3:testes-programa e L3:qml_* (defesa em profundidade contra interferência no
    # ambiente real do usuário/loja; as 69 unidades de tela ainda ganham um sandbox
    # individual por worker dentro de particionar_telas.py).
    sandbox_home = tempfile.mkdtemp(prefix="rustinho-validar-home-")
    sandbox_run = os.path.join(sandbox_home, "run")
    sandbox_tmp = os.path.join(sandbox_home, "tmp")
    os.makedirs(sandbox_run, mode=0o700, exist_ok=True)
    os.makedirs(sandbox_tmp, exist_ok=True)
    env["HOME"] = sandbox_home
    env["XDG_CONFIG_HOME"] = os.path.join(sandbox_home, ".config")
    env["XDG_DATA_HOME"] = os.path.join(sandbox_home, ".local", "share")
    env["XDG_CACHE_HOME"] = os.path.join(sandbox_home, ".cache")
    env["XDG_RUNTIME_DIR"] = sandbox_run
    env["TMPDIR"] = sandbox_tmp

    git = info_git(raiz)
    print(
        f"=== validar_local (T112) ===\n"
        f"raiz:    {raiz} ({git['branch']} @ {git['commit'][:12]}, dirty={git['alteracoes_nao_commitadas']})\n"
        f"camadas: {','.join(sorted(camadas))} · cpus={cpus} · workers_telas={args.workers} · linker={linker}\n"
        f"saida:   {saida}",
        flush=True,
    )

    t_global = time.monotonic()
    etapas = []
    suites = {}
    resumo_telas = None
    cobertura_problemas = []

    try:
        # ---- L0: formato, clippy, portão, versionCode, testar_portao ----
        if "L0" in camadas:
            print("\n[L0] Formatação, Clippy e integridade do Portão", flush=True)
            # fmt + scripts Python rodam em paralelo com clippy (sem disputar target/ com clippy).
            l0_tarefas = [
                ("L0:cargo-fmt", ["cargo", "fmt", "--all", "--check"], os.path.join(logs, "l0_fmt.txt")),
                ("L0:conferir-portao", [sys.executable, "-I", ".github/scripts/conferir_portao.py"], os.path.join(logs, "l0_conferir_portao.txt")),
                ("L0:versao-android", [sys.executable, "-I", ".github/scripts/versao_android.py", "--testar"], os.path.join(logs, "l0_versao_android.txt")),
                ("L0:testar-portao", [sys.executable, "-I", ".github/scripts/testar_portao.py"], os.path.join(logs, "l0_testar_portao.txt")),
                ("L0:cargo-clippy", [
                    "cargo", "clippy", "-p", "rustinho-base", "-p", "rustinho-nucleo",
                    "--all-targets", "--message-format=short", "--",
                    "-D", "clippy::correctness", "-D", "clippy::suspicious",
                ], os.path.join(logs, "l0_clippy.txt")),
            ]
            with concurrent.futures.ThreadPoolExecutor(max_workers=len(l0_tarefas)) as pool:
                futs = [pool.submit(rodar_cmd, n, c, raiz, env, lp) for n, c, lp in l0_tarefas]
                for f in futs:
                    etapas.append(f.result())

        # ---- L1/L2: núcleo debug + release (sem rede em paralelo; rede serializada sob trava) ----
        if "L1" in camadas or "L2" in camadas:
            print("\n[L1/L2] Testes do núcleo (debug + release; rede em série sem sobreposição)", flush=True)
            isolar_rede = args.isolar_rede_nucleo
            perfis = ["debug"] if args.sem_release else ["debug", "release"]
            # Evita sobre-subscrição de SQLite em `reserva_concorrente` quando debug e release rodam juntos.
            threads_por_perfil = str(max(2, cpus // (2 * len(perfis))))
            trava_rede = threading.Lock()

            def rodar_nucleo_perfil(perfil):
                rel_flag = ["--release"] if perfil == "release" else []
                env_p = dict(env)
                if not args.sem_release and perfil == "release":
                    base_target = env.get("CARGO_TARGET_DIR") or os.path.join(raiz, "target")
                    env_p["CARGO_TARGET_DIR"] = os.path.join(base_target, "val_release")
                lp_main = os.path.join(logs, f"l1_nucleo_{perfil}.txt")
                lp_rede = os.path.join(logs, f"l1_nucleo_{perfil}_rede.txt")
                r1 = rodar_cmd(
                    f"L1:nucleo-{perfil}",
                    ["cargo", "test", *rel_flag, "-p", "rustinho-base", "-p", "rustinho-nucleo", "--", *PULAR_REDE, f"--test-threads={threads_por_perfil}"],
                    raiz, env_p, lp_main, isolar_rede=isolar_rede,
                )
                cmd_rede = ["cargo", "test", *rel_flag, "-p", "rustinho-base", "-p", "rustinho-nucleo", "--", *TESTES_DE_REDE, "--test-threads=1"]
                if isolar_rede:
                    r2 = rodar_cmd(f"L1:nucleo-{perfil}-rede", cmd_rede, raiz, env_p, lp_rede, isolar_rede=True)
                else:
                    # Serializa debug-rede e release-rede no host para não sobrepor sockets/broadcasts UDP.
                    with trava_rede:
                        r2 = rodar_cmd(f"L1:nucleo-{perfil}-rede", cmd_rede, raiz, env_p, lp_rede, isolar_rede=False)
                return [r1, r2], {
                    f"nucleo-{perfil}": contar_cargo_test(lp_main),
                    f"nucleo-{perfil}-rede": contar_cargo_test(lp_rede),
                }

            with concurrent.futures.ThreadPoolExecutor(max_workers=len(perfis)) as pool:
                futs = [pool.submit(rodar_nucleo_perfil, p) for p in perfis]
                for f in futs:
                    rs, st = f.result()
                    etapas.extend(rs)
                    suites.update(st)

        # ---- L3: cargo test -p rustinho, QML e todos os 52 roteiros de tela em paralelo ----
        if "L3" in camadas:
            print("\n[L3] Programa Qt, QML e 52 roteiros de tela (paralelos)", flush=True)
            if not qmake:
                etapas.append({"nome": "L3:qmake", "ok": False, "codigo": 1, "timeout": False, "segundos": 0.0, "log": ""})
                print("  [✘] qmake6/qmake não encontrado no PATH", flush=True)
            else:
                # 1) Compila rustinho + roda cargo test -p rustinho.
                lp_prog = os.path.join(logs, "l3_testes_programa.txt")
                lp_build = os.path.join(logs, "l3_build_rustinho.txt")
                r_prog = rodar_cmd("L3:testes-programa", ["cargo", "test", "-p", "rustinho"], raiz, env, lp_prog)
                etapas.append(r_prog)
                suites["programa"] = contar_cargo_test(lp_prog)

                r_build = rodar_cmd("L3:build-rustinho", ["cargo", "build", "-p", "rustinho"], raiz, env, lp_build)
                etapas.append(r_build)

                # 2) Testes QML em paralelo (offscreen, levam ~2s, independem do binário rustinho).
                qml_scripts = sorted(
                    os.path.join(raiz, "app", "testes", f"{n}.py")
                    for n in particionar_telas.listar_qml(raiz)
                )
                env_qml = dict(env, QT_QPA_PLATFORM="offscreen")
                with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, len(qml_scripts))) as pool:
                    futs_qml = [
                        pool.submit(
                            rodar_cmd,
                            f"L3:{os.path.splitext(os.path.basename(q))[0]}",
                            [sys.executable, "-I", q, raiz],
                            raiz, env_qml, os.path.join(logs, f"l3_{os.path.splitext(os.path.basename(q))[0]}.txt"),
                            timeout=600,
                        )
                        for q in qml_scripts
                    ]
                    for f in futs_qml:
                        etapas.append(f.result())

                # 3) Descobre binários auxiliares e executa os 52 roteiros somente se o build passou
                #    (evita rodar roteiros contra um binário `target/debug/rustinho` obsoleto e garante
                #    sempre a emissão do RESUMO_LOCAL.json/md mesmo em caso de falha de compilação).
                if not r_build["ok"]:
                    msg_pulo = "L3:build-rustinho falhou; binários auxiliares e 52 roteiros não executados para evitar validar binário obsoleto"
                    print(f"  [✘] {msg_pulo}", flush=True)
                    cobertura_problemas.append(msg_pulo)
                    etapas.append({"nome": "L3:binarios-auxiliares", "ok": False, "codigo": 1, "timeout": False, "segundos": 0.0, "log": lp_build})
                else:
                    t_aux = time.monotonic()
                    lp_aux = os.path.join(logs, "l3_binarios_auxiliares.txt")
                    bins = None
                    try:
                        bins = _aux_nucleo.descobrir_binarios(raiz, env=env, incluir_release_lojas=True)
                        dt_aux = round(time.monotonic() - t_aux, 2)
                        print(f"  [✔] L3:binarios-auxiliares ({dt_aux:.1f}s: {', '.join(sorted(bins))})", flush=True)
                        etapas.append({"nome": "L3:binarios-auxiliares", "ok": True, "codigo": 0, "timeout": False, "segundos": dt_aux, "log": ""})
                    except (subprocess.CalledProcessError, RuntimeError, OSError) as exc:
                        dt_aux = round(time.monotonic() - t_aux, 2)
                        with open(lp_aux, "w", encoding="utf-8", errors="replace") as lf:
                            lf.write(f"Falha ao compilar/descobrir binários auxiliares: {exc}\n")
                            if isinstance(exc, subprocess.CalledProcessError):
                                if exc.stdout:
                                    lf.write(f"\n--- stdout ---\n{exc.stdout}\n")
                                if exc.stderr:
                                    lf.write(f"\n--- stderr ---\n{exc.stderr}\n")
                        print(f"  [✘] L3:binarios-auxiliares ({dt_aux:.1f}s; veja {lp_aux})", flush=True)
                        cobertura_problemas.append(f"falha em L3:binarios-auxiliares ({exc})")
                        etapas.append({"nome": "L3:binarios-auxiliares", "ok": False, "codigo": 1, "timeout": False, "segundos": dt_aux, "log": lp_aux})

                    if bins is not None:
                        env_telas = dict(env)
                        env_telas.update(bins)
                        target_dir = env.get("CARGO_TARGET_DIR") or os.path.join(raiz, "target")
                        env_telas["RUSTINHO_PROGRAMA"] = os.path.join(target_dir, "debug", "rustinho")

                        unidades = particionar_telas.catalogo(raiz)
                        telas_dir = os.path.join(saida, "telas")
                        resumo_telas, falharam_telas = particionar_telas.executar_unidades(
                            raiz=raiz,
                            unidades=unidades,
                            saida_dir=telas_dir,
                            workers=args.workers,
                            isolamento=args.isolamento,
                            python_bin=sys.executable,
                            env_base=env_telas,
                            shard=1,
                            total_shards=1,
                        )
                        pares = [(u["id"], u["status"]) for u in resumo_telas["unidades"]]
                        cobertura_problemas = particionar_telas.verificar_cobertura(raiz, pares, shards_vistos=[1], total_shards=1)
                        etapas.append({
                            "nome": f"L3:telas-52-roteiros ({len(unidades)} unidades)",
                            "ok": not falharam_telas and not cobertura_problemas,
                            "codigo": 0 if (not falharam_telas and not cobertura_problemas) else 1,
                            "timeout": False,
                            "segundos": resumo_telas["segundos_total"],
                            "log": os.path.join(telas_dir, "telas.txt"),
                        })
    finally:
        shutil.rmtree(sandbox_home, ignore_errors=True)

    duracao_total = round(time.monotonic() - t_global, 2)
    aprovado = all(e["ok"] for e in etapas) and not cobertura_problemas
    completa = camadas >= {"L0", "L1", "L2", "L3"} and not args.sem_release

    relatorio = {
        "formato": 1,
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "raiz": raiz,
        "git": git,
        "camadas": sorted(camadas),
        "validacao_linux_completa": completa,
        "aprovado": aprovado,
        "segundos_total": duracao_total,
        "hardware": {"cpus": cpus, "workers_telas": args.workers, "linker": linker},
        "etapas": etapas,
        "suites_rust": suites,
        "telas": resumo_telas,
        "cobertura_problemas": cobertura_problemas,
    }
    json_path = os.path.join(saida, "RESUMO_LOCAL.json")
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(relatorio, f, ensure_ascii=False, indent=2)
        f.write("\n")

    linhas_md = [
        f"# Validação Local Linux — {'APROVADA' if aprovado else 'REPROVADA'}",
        "",
        f"- Commit: `{git['commit']}` (`{git['branch']}`, dirty={git['alteracoes_nao_commitadas']})",
        f"- Tempo total: **{duracao_total:.1f}s ({duracao_total / 60:.2f} min)**",
        f"- Camadas: `{','.join(sorted(camadas))}` · completa={completa} · linker=`{linker}` · workers_telas={args.workers}",
        "",
        "## Etapas",
        "",
        "| Etapa | Resultado | Tempo |",
        "|---|---|---:|",
    ]
    for e in etapas:
        linhas_md.append(f"| `{e['nome']}` | {'✔ ok' if e['ok'] else '✘ FALHOU'} | {e['segundos']:.1f}s |")
    if suites:
        linhas_md += ["", "## Suítes Rust", ""]
        for k, s in suites.items():
            linhas_md.append(f"- `{k}`: {s['passaram']} passaram, {s['falharam']} falharam, {s['ignorados']} ignorados ({s['binarios']} binários)")
    if resumo_telas:
        ok_u = sum(1 for u in resumo_telas["unidades"] if u["status"] == "ok")
        linhas_md += [
            "",
            "## Roteiros de Tela",
            "",
            f"- Unidades: **{ok_u}/{len(resumo_telas['unidades'])} ok** em **{resumo_telas['segundos_total']:.1f}s** ({resumo_telas['isolamento']}, {resumo_telas['workers']} workers)",
            f"- Falhas esperadas registradas: {len(resumo_telas['falhas_esperadas'])}",
        ]
        top10 = sorted(resumo_telas["unidades"], key=lambda u: -u["segundos"])[:10]
        linhas_md += ["", "Top 10 unidades mais lentas:", ""]
        for u in top10:
            linhas_md.append(f"- `{u['id']}`: {u['segundos']:.1f}s ({u['status']})")
    if cobertura_problemas:
        linhas_md += ["", "## Problemas de cobertura", ""] + [f"- {p}" for p in cobertura_problemas]

    md_path = os.path.join(saida, "RESUMO_LOCAL.md")
    with open(md_path, "w", encoding="utf-8") as f:
        f.write("\n".join(linhas_md) + "\n")

    print(
        f"\n=== RESULTADO: {'APROVADA' if aprovado else 'REPROVADA'} em {duracao_total:.1f}s ({duracao_total/60:.2f} min) ===\n"
        f"Resumo: {md_path}\nJSON:   {json_path}",
        flush=True,
    )
    sys.exit(0 if aprovado else 1)


if __name__ == "__main__":
    main()
