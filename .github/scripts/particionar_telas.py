#!/usr/bin/env python3
"""Catálogo, particionamento balanceado (LPT) e execução paralela dos roteiros de tela (T112).

Fonte única de verdade usada tanto na CI do GitHub (`validar.yml`, sharded em N
runners × W workers Xvfb) quanto no validador local do CachyOS (`scripts/validar_local.py`).

Garantias de rigor:
1. Descobre TODOS os roteiros `app/testes/telas_*.py` + `app/testes/geometria_telas.py`
   (exceto `telas_seletor_nativo.py`, que exige portal KDE manual).
2. Subdivide os 4 roteiros mais longos (`telas_seta_esc`, `telas_toque_celular`,
   `telas_compras_trocas_t054`, `telas_voltar_android`) em fatias disjuntas e
   exaustivas — lendo por AST as listas de `telas_toque_celular.py` e o padrão
   de casos de `telas_compras_trocas_t054.py` para que qualquer tela ou caso
   novo adicionado no futuro entre automaticamente sem risco de omissão.
3. Particiona as unidades em `N` shards pelo algoritmo determinístico LPT
   (Longest Processing Time first), usando pesos reais medidos da CI.
4. `verificar_cobertura()` (usado por `manifesto.py` e `testar_portao.py`)
   comprova que 100% das unidades esperadas e todos os `N` shards foram
   executados sem omissão, duplicação indevida ou falha.
"""
import argparse
import ast
import concurrent.futures
import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

EXCLUIDOS = {"telas_seletor_nativo.py"}

# Telas virtuais aceitas por TODOS os roteiros (incl. a lista fechada dos 4 roteiros C8/QT1).
DISPLAYS_SEGUROS = [f":{n}" for n in range(60, 75)] + [":88", ":89", ":91", ":92", ":93"]

# Roteiros que fazem bind em porta TCP fixa (47810) quando executados sem network namespace.
# T014 (0.9.19-dev.2): o roteiro do protocolo 4 liga a troca de dados do Rustinho de verdade (47810/47811).
EXIGEM_REDE_EXCLUSIVA = {"telas_rvi02", "telas_dev2_protocolo4"}

# Pesos estimados (em segundos, calibrados pelos tempos reais medidos no CachyOS + CI 44984217573) para balanceamento LPT.
PESOS_SEGUNDOS = {
    "telas_seta_esc__estatico_a6_cinza": 75,
    "telas_seta_esc__a6_amoled": 90,
    "telas_seta_esc__a6_roxo": 75,
    "telas_seta_esc__a7_telas_1": 135,
    "telas_seta_esc__a7_telas_2": 135,
    "telas_seta_esc__a7_alt_dv3": 115,
    "telas_seta_esc__a8_a10_inicio": 90,
    "telas_toque_celular__p1": 195,
    "telas_toque_celular__p2": 195,
    "telas_toque_celular__p3": 195,
    "telas_toque_celular__p4": 195,
    "telas_toque_celular__p5": 195,
    "telas_toque_celular__p6": 195,
    "telas_compras_trocas_t054__C1": 90,
    "telas_compras_trocas_t054__C8A": 135,
    "telas_compras_trocas_t054__C8B": 135,
    "telas_compras_trocas_t054__TO": 105,
    "telas_compras_trocas_t054__F": 100,
    "telas_compras_trocas_t054__P": 55,
    "telas_voltar_android__computador": 105,
    "telas_voltar_android__celular": 185,
    "telas_importar_cadastros_rolagem": 258,
    "telas_busca_rapida": 248,
    "telas_c8_rerevisao2": 199,
    "telas_navegacao_casos": 178,
    "telas_lista_continua": 170,
    "telas_navegacao_etapa1": 160,
    "telas_i8_revisao": 145,
    "telas_c8_rerevisao": 140,
    "telas_tema_0918": 135,
    "telas_lista_que_muda": 125,
    "telas_layout_72": 107,
    "telas_c8_revisao": 99,
    "telas_janelinhas_altura": 90,
    "telas_navegacao_guiada": 89,
    "telas_receber_tela_altura": 84,
    "telas_avisos_topo": 79,
    "telas_campos_estados": 77,
    "telas_notificacoes": 71,
    "telas_clique_no_movimento": 69,
    "telas_codigo_fornecedor": 68,
    "telas_receber_altura": 68,
    "telas_d09": 65,
    "geometria_telas": 64,
    "telas_i8_busca_celular": 64,
    "telas_peca_tipo": 54,
    "telas_unidades": 50,
    "telas_qt1_rerevisao2": 49,
    "telas_texto_puro": 49,
    "telas_rvi02": 48,
    "telas_juntar_h24": 39,
    "telas_pecas_leitor_t067": 39,
    "telas_lista_pecas": 38,
    "telas_inicio_visual": 36,
    "telas_usuarios_t084": 36,
    "telas_chamadas_gerente": 33,
    "telas_importar_cadastros": 31,
    "telas_falha07": 28,
    "telas_estab_sessao": 28,
    "telas_numeros": 27,
    "telas_modo_seguro": 23,
    "telas_impressao_falha": 20,
    "telas_compras_pasta_t107": 17,
    "telas_compras_conferencia_t111": 25,
    "telas_dev1a_interface": 70,
    "telas_dev2_protocolo4": 150,
    "telas_dev1c_recuperacao": 45,
    "telas_dev1d_navegacao": 75,
    "telas_d14_portatil": 17,
    "telas_qt1b_fechamento": 15,
    "telas_notif03": 15,
    "telas_etiquetas_quantidade": 12,
    "telas_enter_espera_t068": 11,
    "telas_motivo_erro_t090": 7,
}


def _ler_ast(caminho):
    with open(caminho, encoding="utf-8") as f:
        return ast.parse(f.read(), filename=caminho)


def _nomes_toque_celular(caminho):
    """Lê por AST todos os nomes de `TELAS` + `ACOES` em `telas_toque_celular.py`."""
    arvore = _ler_ast(caminho)
    telas, acoes = [], []
    for no in arvore.body:
        if isinstance(no, ast.Assign) and len(no.targets) == 1 and isinstance(no.targets[0], ast.Name):
            alvo = no.targets[0].id
            if alvo == "TELAS":
                val = ast.literal_eval(no.value)
                telas = [item[0] for item in val]
            elif alvo == "ACOES":
                val = ast.literal_eval(no.value)
                acoes = list(val.keys())
    nomes = telas + acoes
    if not nomes or len(nomes) != len(set(nomes)):
        raise ValueError(f"lista inválida de TELAS/ACOES em {caminho}: {len(nomes)} itens")
    return nomes


def _casos_padrao_t054(caminho):
    """Lê por AST o valor padrão de `RUSTINHO_CASOS` em `telas_compras_trocas_t054.py`."""
    arvore = _ler_ast(caminho)
    for no in ast.walk(arvore):
        if isinstance(no, ast.Call) and isinstance(no.func, ast.Attribute) and no.func.attr == "get":
            if len(no.args) == 2 and isinstance(no.args[0], ast.Constant) and no.args[0].value == "RUSTINHO_CASOS":
                if isinstance(no.args[1], ast.Constant) and isinstance(no.args[1].value, str):
                    return no.args[1].value
    raise ValueError(f"não achei o padrão de RUSTINHO_CASOS em {caminho}")


def _casos_e_temas_seta_esc(caminho):
    """Confere por AST os padrões de `RUSTINHO_CASOS`, `RUSTINHO_TEMAS` e `TELAS` em `telas_seta_esc.py`."""
    arvore = _ler_ast(caminho)
    padrao = {}
    telas = []
    for no in arvore.body:
        if isinstance(no, ast.Assign) and len(no.targets) == 1 and isinstance(no.targets[0], ast.Name):
            if no.targets[0].id == "TELAS":
                telas = [item[0] for item in ast.literal_eval(no.value)]
    for no in ast.walk(arvore):
        if isinstance(no, ast.Call) and isinstance(no.func, ast.Attribute) and no.func.attr == "get":
            if len(no.args) == 2 and isinstance(no.args[0], ast.Constant) and no.args[0].value in ("RUSTINHO_CASOS", "RUSTINHO_TEMAS"):
                if isinstance(no.args[1], ast.Constant) and isinstance(no.args[1].value, str):
                    padrao[no.args[0].value] = no.args[1].value
    if "RUSTINHO_CASOS" not in padrao or "RUSTINHO_TEMAS" not in padrao or not telas:
        raise ValueError(f"não achei padrões de RUSTINHO_CASOS/RUSTINHO_TEMAS/TELAS em {caminho}")
    return padrao["RUSTINHO_CASOS"].split(","), padrao["RUSTINHO_TEMAS"].split(","), telas


def listar_roteiros(raiz):
    """Lista ordenada de caminhos relativos (`app/testes/...py`) de todos os roteiros de tela ativos."""
    padroes = [
        os.path.join(raiz, "app", "testes", "telas_*.py"),
        os.path.join(raiz, "app", "testes", "geometria_telas.py"),
    ]
    arquivos = []
    for p in padroes:
        arquivos.extend(sorted(glob.glob(p)))
    rel = []
    vistos = set()
    for a in arquivos:
        base = os.path.basename(a)
        if base in EXCLUIDOS or base in vistos:
            continue
        vistos.add(base)
        rel.append(os.path.relpath(a, raiz).replace(os.sep, "/"))
    return sorted(rel)


def listar_qml(raiz):
    """Lista ordenada dos nomes (sem .py) de todos os `app/testes/qml_*.py`."""
    return sorted(
        os.path.splitext(os.path.basename(a))[0]
        for a in glob.glob(os.path.join(raiz, "app", "testes", "qml_*.py"))
    )


def catalogo(raiz):
    """Devolve a lista determinística de unidades de execução cobrindo 100% dos roteiros.

    Cada unidade é um dict:
      {"id": str, "roteiro": str, "arquivo": str, "env": dict[str, str], "peso": int}
    """
    unidades = []
    for rel in listar_roteiros(raiz):
        abs_arq = os.path.join(raiz, rel)
        stem = os.path.splitext(os.path.basename(rel))[0]

        if stem == "telas_seta_esc":
            casos, temas, telas_a7 = _casos_e_temas_seta_esc(abs_arq)
            # Garante que qualquer caso, tema ou tela nova não fique de fora.
            casos_cobertos = {"ESTATICO", "A6", "A7", "A8", "A10", "INICIO"}
            extras_casos = [c for c in casos if c not in casos_cobertos]
            for i, tema in enumerate(temas):
                casos_fatia = ["ESTATICO", "A6"] if i == 0 else ["A6"]
                uid = f"telas_seta_esc__{'estatico_' if i == 0 else ''}a6_{tema}"
                unidades.append({
                    "id": uid, "roteiro": stem, "arquivo": rel,
                    "env": {"RUSTINHO_CASOS": ",".join(casos_fatia), "RUSTINHO_TEMAS": tema},
                    "peso": PESOS_SEGUNDOS.get(uid, 90),
                })
            meio_a7 = len(telas_a7) // 2
            for idx, fatia_t in enumerate((telas_a7[:meio_a7], telas_a7[meio_a7:]), 1):
                uid = f"telas_seta_esc__a7_telas_{idx}"
                unidades.append({
                    "id": uid, "roteiro": stem, "arquivo": rel,
                    "env": {"RUSTINHO_CASOS": "A7TELAS", "RUSTINHO_SO_TELAS": ",".join(fatia_t)},
                    "peso": PESOS_SEGUNDOS.get(uid, 135),
                })
            unidades.append({
                "id": "telas_seta_esc__a7_alt_dv3", "roteiro": stem, "arquivo": rel,
                "env": {"RUSTINHO_CASOS": "A7ALT,A7DV3"},
                "peso": PESOS_SEGUNDOS.get("telas_seta_esc__a7_alt_dv3", 115),
            })
            resto = ["A8", "A10", "INICIO"] + extras_casos
            unidades.append({
                "id": "telas_seta_esc__a8_a10_inicio", "roteiro": stem, "arquivo": rel,
                "env": {"RUSTINHO_CASOS": ",".join(resto), "RUSTINHO_TEMAS": ",".join(temas)},
                "peso": PESOS_SEGUNDOS.get("telas_seta_esc__a8_a10_inicio", 90),
            })

        elif stem == "telas_toque_celular":
            nomes = _nomes_toque_celular(abs_arq)
            partes = 6
            fatias = [nomes[i::partes] for i in range(partes)]
            for idx, f in enumerate(fatias, 1):
                uid = f"telas_toque_celular__p{idx}"
                unidades.append({
                    "id": uid, "roteiro": stem, "arquivo": rel,
                    "env": {"RUSTINHO_TOQUE_TELAS": ",".join(f)},
                    "peso": PESOS_SEGUNDOS.get(uid, 195),
                })

        elif stem == "telas_compras_trocas_t054":
            padrao = _casos_padrao_t054(abs_arq)
            grupos = [("C1", "1"), ("C8A", "2"), ("C8B", "3"), ("TO", "TO"), ("F", "F"), ("P", "P")]
            cobertos = set("CTOFP")
            sobra = "".join(c for c in padrao if c not in cobertos)
            if sobra:
                grupos[-1] = ("P", "P" + sobra)
            for rotulo, letras in grupos:
                uid = f"telas_compras_trocas_t054__{rotulo}"
                unidades.append({
                    "id": uid, "roteiro": stem, "arquivo": rel,
                    "env": {"RUSTINHO_CASOS": letras},
                    "peso": PESOS_SEGUNDOS.get(uid, 120),
                })

        elif stem == "telas_voltar_android":
            unidades.append({
                "id": "telas_voltar_android__computador", "roteiro": stem, "arquivo": rel,
                "env": {"RUSTINHO_VOLTAR_SO_ESTE": "1"},
                "peso": PESOS_SEGUNDOS.get("telas_voltar_android__computador", 270),
            })
            unidades.append({
                "id": "telas_voltar_android__celular", "roteiro": stem, "arquivo": rel,
                "env": {"RUSTINHO_VOLTAR_SO_ESTE": "1", "RUSTINHO_CASOS_CELULAR": "1"},
                "peso": PESOS_SEGUNDOS.get("telas_voltar_android__celular", 285),
            })

        else:
            unidades.append({
                "id": stem, "roteiro": stem, "arquivo": rel,
                "env": {},
                "peso": PESOS_SEGUNDOS.get(stem, 90),
            })

    return unidades


def particionar(unidades, total_shards):
    """Particionamento determinístico LPT em `total_shards` grupos (índices 1..total_shards)."""
    if total_shards < 1:
        raise ValueError("total_shards tem de ser >= 1")
    ordenadas = sorted(unidades, key=lambda u: (-u["peso"], u["id"]))
    cargas = [0] * total_shards
    grupos = [[] for _ in range(total_shards)]
    for u in ordenadas:
        k = min(range(total_shards), key=lambda i: (cargas[i], i))
        grupos[k].append(u)
        cargas[k] += u["peso"]
    return {i + 1: grupos[i] for i in range(total_shards)}


def unshare_rede_funciona():
    """Confere se `unshare -rn` (user + network namespace isolado) funciona."""
    if not shutil.which("unshare"):
        return False
    try:
        r = subprocess.run(
            ["unshare", "-rn", "sh", "-c", "ip link set lo up"],
            capture_output=True, timeout=5,
        )
        return r.returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def executar_unidades(
    raiz,
    unidades,
    saida_dir,
    workers=2,
    isolamento="auto",
    timeout_unidade=1800,
    python_bin=None,
    env_base=None,
    shard=1,
    total_shards=1,
):
    """Executa uma lista de unidades em paralelo, gravando telas.txt, telas-resultado.txt e telas-unidades.json."""
    os.makedirs(saida_dir, exist_ok=True)
    fotos_root = os.path.join(saida_dir, "fotos")
    logs_dir = os.path.join(saida_dir, "logs_unidades")
    os.makedirs(fotos_root, exist_ok=True)
    os.makedirs(logs_dir, exist_ok=True)

    python_bin = python_bin or ("/usr/bin/python3" if os.path.exists("/usr/bin/python3") else sys.executable)
    if isolamento == "auto":
        usar_unshare_rede = unshare_rede_funciona()
    elif isolamento == "unshare":
        if not unshare_rede_funciona():
            raise RuntimeError("isolamento=unshare pedido, mas `unshare -rn` não está disponível nesta máquina")
        usar_unshare_rede = True
    elif isolamento == "display":
        usar_unshare_rede = False
    else:
        raise ValueError(f"isolamento inválido: {isolamento}")

    modo_rotulo = "unshare-rn" if usar_unshare_rede else "display-exclusivo"
    workers = max(1, min(workers, len(DISPLAYS_SEGUROS), max(1, len(unidades))))
    print(
        f"particionar_telas: shard {shard}/{total_shards} · {len(unidades)} unidade(s) · "
        f"{workers} worker(s) · isolamento={modo_rotulo}",
        flush=True,
    )

    # Com `unshare -rn` (network namespace isolado por worker, sem alterar /tmp),
    # cada instância do Rustinho ganha suas próprias portas 47810/47811 (evitando
    # `os error 98` entre workers e protegendo o Rustinho real da loja no host).
    # Sem `unshare -rn`, roteiros que fazem bind fixo em 47810 rodam à parte.
    if usar_unshare_rede:
        paralelas = sorted(unidades, key=lambda u: (-u["peso"], u["id"]))
        exclusivas = []
    else:
        paralelas = sorted([u for u in unidades if u["roteiro"] not in EXIGEM_REDE_EXCLUSIVA],
                           key=lambda u: (-u["peso"], u["id"]))
        exclusivas = [u for u in unidades if u["roteiro"] in EXIGEM_REDE_EXCLUSIVA]

    # Pool de slots de display (1 display exclusivo por worker concorrente).
    slots_livres = list(range(workers))
    trava_slots = threading.Lock()
    sem_slots = threading.Semaphore(workers)
    resultados = {}
    trava_res = threading.Lock()

    def rodar_uma(u, slot_fixo=None):
        if slot_fixo is None:
            sem_slots.acquire()
            with trava_slots:
                slot = slots_livres.pop(0)
        else:
            slot = slot_fixo
        display = DISPLAYS_SEGUROS[slot]
        home_w = tempfile.mkdtemp(prefix=f"rustinho-w{slot}-{u['id'][:16]}-")
        xdg_run = os.path.join(home_w, "run")
        tmp_w = os.path.join(home_w, "tmp")
        os.makedirs(xdg_run, mode=0o700, exist_ok=True)
        os.makedirs(tmp_w, exist_ok=True)
        fotos_u = os.path.join(fotos_root, u["id"])
        os.makedirs(fotos_u, exist_ok=True)
        log_u = os.path.join(logs_dir, u["id"] + ".log")

        env = dict(os.environ if env_base is None else env_base)
        env.update(u["env"])
        env["RUSTINHO_TELA"] = display
        env["HOME"] = home_w
        env["XDG_CONFIG_HOME"] = os.path.join(home_w, ".config")
        # N-10 (0.9.19-dev.2): sem sessão D-Bus, o `secret-tool` do Rustinho não alcança
        # o chaveiro de quem roda os roteiros (a identidade do protocolo 4 vai para a
        # reserva local, dentro do HOME temporário deste worker).
        env.pop("DBUS_SESSION_BUS_ADDRESS", None)
        env["XDG_DATA_HOME"] = os.path.join(home_w, ".local", "share")
        env["XDG_CACHE_HOME"] = os.path.join(home_w, ".cache")
        env["XDG_RUNTIME_DIR"] = xdg_run
        env["TMPDIR"] = tmp_w
        env["PYTHONUNBUFFERED"] = "1"

        script_abs = os.path.join(raiz, u["arquivo"])
        if usar_unshare_rede:
            cmd = [
                "unshare", "-rn", "sh", "-c",
                'ip link set lo up && exec "$@"',
                "_", python_bin, script_abs, fotos_u,
            ]
        else:
            cmd = [python_bin, script_abs, fotos_u]

        t0 = time.monotonic()
        codigo = -1
        estourou = False
        try:
            with open(log_u, "w", encoding="utf-8", errors="replace") as lf:
                p = subprocess.run(cmd, cwd=raiz, env=env, stdout=lf, stderr=subprocess.STDOUT, timeout=timeout_unidade)
                codigo = p.returncode
        except subprocess.TimeoutExpired:
            estourou = True
            with open(log_u, "a", encoding="utf-8") as lf:
                lf.write(f"\n[ERRO] tempo limite de {timeout_unidade} s excedido em {u['id']}\n")
        finally:
            dt = round(time.monotonic() - t0, 2)
            shutil.rmtree(home_w, ignore_errors=True)
            if slot_fixo is None:
                with trava_slots:
                    slots_livres.append(slot)
                sem_slots.release()

        status = "ok" if codigo == 0 else "falhou"
        print(f"  [{'✔' if codigo == 0 else '✘'}] {u['id']} ({dt:.1f}s, {display})", flush=True)
        with trava_res:
            resultados[u["id"]] = {
                "id": u["id"],
                "roteiro": u["roteiro"],
                "arquivo": u["arquivo"],
                "status": status,
                "codigo": codigo,
                "timeout": estourou,
                "segundos": dt,
                "display": display,
                "log": log_u,
            }

    t_inicio = time.monotonic()
    if paralelas:
        with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
            futs = [pool.submit(rodar_uma, u) for u in paralelas]
            for f in concurrent.futures.as_completed(futs):
                f.result()
    for u in exclusivas:
        rodar_uma(u, slot_fixo=0)
    duracao_total = round(time.monotonic() - t_inicio, 2)

    # Consolida telas.txt, telas-resultado.txt e telas-unidades.json em ordem determinística.
    telas_txt = os.path.join(saida_dir, "telas.txt")
    res_txt = os.path.join(saida_dir, "telas-resultado.txt")
    meta_json = os.path.join(saida_dir, "telas-unidades.json")
    commit = os.environ.get("GITHUB_SHA", "")
    falharam = []
    falhas_esperadas = []
    ordem = [u["id"] for u in sorted(unidades, key=lambda x: x["id"])]

    with open(telas_txt, "w", encoding="utf-8") as ft, open(res_txt, "w", encoding="utf-8") as fr:
        if commit:
            ft.write(f"commit: {commit}\n")
        for uid in ordem:
            r = resultados[uid]
            ft.write(f"=== {uid}\n")
            with open(r["log"], encoding="utf-8", errors="replace") as f_log:
                conteudo = f_log.read()
            ft.write(conteudo)
            if conteudo and not conteudo.endswith("\n"):
                ft.write("\n")
            for linha in conteudo.splitlines():
                if "falha ESPERADA" in linha:
                    falhas_esperadas.append(linha.strip())
            fr.write(f"{uid}\t{r['status']}\t{r['segundos']:.2f}\n")
            if r["status"] != "ok":
                falharam.append(uid)

    metadados = {
        "formato": 1,
        "shard": shard,
        "total_shards": total_shards,
        "workers": workers,
        "isolamento": modo_rotulo,
        "segundos_total": duracao_total,
        "unidades": [
            {k: v for k, v in resultados[uid].items() if k != "log"}
            for uid in ordem
        ],
        "falhas_esperadas": falhas_esperadas,
    }
    with open(meta_json, "w", encoding="utf-8") as fm:
        json.dump(metadados, fm, ensure_ascii=False, indent=2)
        fm.write("\n")

    print(
        f"particionar_telas: shard {shard}/{total_shards} concluído em {duracao_total:.1f}s · "
        f"{len(ordem) - len(falharam)}/{len(ordem)} ok"
        + (f" · FALHARAM: {', '.join(falharam)}" if falharam else ""),
        flush=True,
    )
    return metadados, falharam


def verificar_cobertura(raiz, unidades_executadas, shards_vistos=None, total_shards=None):
    """Confere se as unidades executadas cobrem EXATAMENTE 100% do catálogo esperado.

    Devolve lista de problemas (vazia = cobertura 100% íntegra).
    """
    problemas = []
    esperadas = {u["id"]: u for u in catalogo(raiz)}
    if total_shards is not None and shards_vistos is not None:
        esperados_shards = set(range(1, total_shards + 1))
        faltam_shards = sorted(esperados_shards - set(shards_vistos))
        extras_shards = sorted(set(shards_vistos) - esperados_shards)
        if faltam_shards:
            problemas.append(f"shards de telas ausentes: {faltam_shards} (esperados 1..{total_shards})")
        if extras_shards:
            problemas.append(f"shards de telas inesperados: {extras_shards}")

    vistos = {}
    duplicadas = []
    for uid, status in unidades_executadas:
        if uid in vistos:
            duplicadas.append(uid)
        vistos[uid] = status
    if duplicadas:
        problemas.append(f"unidades de tela duplicadas entre shards: {sorted(set(duplicadas))}")

    faltam = sorted(set(esperadas) - set(vistos))
    sobram = sorted(set(vistos) - set(esperadas))
    falharam = sorted(uid for uid, st in vistos.items() if st != "ok")
    if faltam:
        problemas.append(f"roteiros/unidades de tela omitidos ({len(faltam)}): {faltam[:10]}")
    if sobram:
        problemas.append(f"unidades de tela desconhecidas ({len(sobram)}): {sobram[:10]}")
    if falharam:
        problemas.append(f"unidades de tela reprovadas ({len(falharam)}): {falharam[:10]}")
    return problemas


def main():
    ap = argparse.ArgumentParser(description="Particiona e executa roteiros de tela do Rustinho.")
    ap.add_argument("--raiz", default=os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    ap.add_argument("--shard", type=int, default=1)
    ap.add_argument("--total", type=int, default=1)
    ap.add_argument("--listar", action="store_true", help="Imprime JSON do catálogo ou do shard.")
    ap.add_argument("--executar", action="store_true", help="Executa o shard selecionado.")
    ap.add_argument("--saida", default=".", help="Pasta de saída de telas.txt, telas-resultado.txt e fotos/.")
    ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--isolamento", choices=("auto", "unshare", "display"), default="auto")
    ap.add_argument("--timeout", type=int, default=1800)
    ap.add_argument("--python", default=None)
    args = ap.parse_args()

    raiz = os.path.abspath(args.raiz)
    todas = catalogo(raiz)
    grupos = particionar(todas, args.total)
    if args.shard < 1 or args.shard > args.total:
        sys.exit(f"shard inválido: {args.shard} (total {args.total})")
    selecionadas = grupos[args.shard]

    if args.listar:
        resumo = {
            "total_unidades": len(todas),
            "total_roteiros": len({u["roteiro"] for u in todas}),
            "total_shards": args.total,
            "cargas_por_shard": {str(k): sum(u["peso"] for u in v) for k, v in grupos.items()},
            "shard": args.shard,
            "unidades": selecionadas,
        }
        print(json.dumps(resumo, ensure_ascii=False, indent=2))
        return

    if args.executar:
        _, falharam = executar_unidades(
            raiz=raiz,
            unidades=selecionadas,
            saida_dir=os.path.abspath(args.saida),
            workers=args.workers,
            isolamento=args.isolamento,
            timeout_unidade=args.timeout,
            python_bin=args.python,
            shard=args.shard,
            total_shards=args.total,
        )
        if falharam:
            print(f"::error title=Roteiros das telas (shard {args.shard}/{args.total})::falharam: {' '.join(falharam)}")
            sys.exit(1)
        return

    ap.print_help()


if __name__ == "__main__":
    main()
