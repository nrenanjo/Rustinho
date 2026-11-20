#!/usr/bin/env python3
"""Testes do manifesto e do Portão com o Android desligado e ligado (T050, 19/11/2026).

Decisão do proprietário (19/11/2026, 23h25): o Android saiu do escopo até ele
pedir de volta. A CI tem uma chave única (`android` no ci.yml); desligada, os
jobs `apk` e `emulador` são pulados, o manifesto não lista APK e o Portão exige
exatamente os 3 pacotes do computador, sem nenhum APK.

Cada caso monta uma pasta de artefatos falsa, roda o manifesto.py e o portao.py
de verdade (como na CI) num repositório git temporário, e confere a saída.
Roda no job "formato" da CI e à mão:

    python3 .github/scripts/testar_portao.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
from versao_android import codigo  # noqa: E402

VERSAO = "0.9.18-dev.3.3"
COMPUTADOR = [
    ("Rustinho-Linux", f"Rustinho-{VERSAO}-Linux-x86_64.AppImage"),
    ("Rustinho-Windows", f"Rustinho-{VERSAO}-Windows-instalador.exe"),
    ("Rustinho-Windows-portatil", f"Rustinho-{VERSAO}-Windows-portatil.zip"),
]
ANDROID = [
    ("Rustinho-Android", f"Rustinho-{VERSAO}-Android.apk"),
    ("Rustinho-Android-32bits", f"Rustinho-{VERSAO}-Android-32bits.apk"),
]
JOBS_COMPUTADOR = ["formato", "nucleo-linux", "nucleo-windows", "programa-e-telas", "pacote-linux",
                   "pacote-windows", "verificar"]
CHAVE = "AB" * 32


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, "-c", "user.name=teste", "-c", "user.email=teste@exemplo.invalid",
                           *args], check=True, capture_output=True, text=True).stdout.strip()


class Caso:
    """Um repositório temporário com Cargo.toml, uma etiqueta v<versão> e uma pasta de artefatos."""

    def __init__(self):
        self.raiz = tempfile.mkdtemp(prefix="testar_portao_")
        self.repo = os.path.join(self.raiz, "repo")
        os.makedirs(os.path.join(self.repo, "android", "rust"))
        for arq in ("Cargo.toml", os.path.join("android", "rust", "Cargo.toml")):
            with open(os.path.join(self.repo, arq), "w", encoding="utf-8") as f:
                f.write(f'[package]\nname = "teste"\nversion = "{VERSAO}"\n')
        git(self.repo, "init", "-q")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "teste")
        git(self.repo, "tag", f"v{VERSAO}")
        self.commit = git(self.repo, "rev-parse", "HEAD")
        self.artefatos = os.path.join(self.repo, "artefatos")
        os.makedirs(self.artefatos)

    def pacote(self, artefato, arquivo, conteudo=None):
        pasta = os.path.join(self.artefatos, artefato)
        os.makedirs(pasta, exist_ok=True)
        caminho = os.path.join(pasta, arquivo)
        with open(caminho, "wb") as f:
            f.write(conteudo or f"pacote {arquivo}".encode())
        h = subprocess.run(["sha256sum", caminho], capture_output=True, text=True, check=True).stdout.split()[0]
        with open(caminho + ".sha256", "w", encoding="utf-8") as f:
            f.write(f"{h}  {arquivo}\n")
        if arquivo.endswith(".apk"):
            for nome, texto in (("versao.txt", f"VERSAO={VERSAO}\nCODIGO={codigo(VERSAO)}\n"),
                                ("assinatura.txt", "loja\n"), ("certificado.txt", CHAVE + "\n")):
                with open(os.path.join(pasta, nome), "w", encoding="utf-8") as f:
                    f.write(texto)

    def env(self, android, ref, extra=None):
        e = {k: v for k, v in os.environ.items() if not k.startswith(("GITHUB_", "RUSTINHO_", "ANDROID_"))}
        e.update({"GITHUB_SHA": self.commit, "GITHUB_RUN_ID": "123", "GITHUB_REF": ref})
        if android is not None:
            e["RUSTINHO_ANDROID"] = android
        e.update(extra or {})
        return e

    def manifesto(self, jobs, android, ref="refs/heads/ensaio/teste"):
        e = self.env(android, ref)
        e["NECESSARIOS"] = json.dumps({j: {"result": r} for j, r in jobs.items()})
        r = subprocess.run([sys.executable, "-I", os.path.join(AQUI, "manifesto.py"), "artefatos",
                            os.path.join("artefatos", "Manifesto")], cwd=self.repo, env=e,
                           capture_output=True, text=True)
        with open(os.path.join(self.artefatos, "Manifesto", "MANIFESTO.json"), encoding="utf-8") as f:
            return r, json.load(f)

    def portao(self, android, ref="refs/heads/ensaio/teste", extra=None):
        return subprocess.run([sys.executable, "-I", os.path.join(AQUI, "portao.py"), "artefatos"], cwd=self.repo,
                              env=self.env(android, ref, extra), capture_output=True, text=True)

    def limpar(self):
        shutil.rmtree(self.raiz, ignore_errors=True)


def jobs_sem_android(**mudar):
    j = {n: "success" for n in JOBS_COMPUTADOR}
    j.update({"apk": "skipped", "emulador": "skipped"})
    j.update({k.replace("_", "-"): v for k, v in mudar.items()})
    return j


def jobs_com_android(**mudar):
    j = {n: "success" for n in JOBS_COMPUTADOR + ["apk", "emulador"]}
    j.update({k.replace("_", "-"): v for k, v in mudar.items()})
    return j


class SemAndroid(unittest.TestCase):
    """A chave desligada (o escopo atual)."""

    def setUp(self):
        self.c = Caso()
        for artefato, arquivo in COMPUTADOR:
            self.c.pacote(artefato, arquivo)

    def tearDown(self):
        self.c.limpar()

    def test_ramo_aprova_com_os_3_pacotes_do_computador(self):
        r, m = self.c.manifesto(jobs_sem_android(), "nao")
        self.assertTrue(m["aprovado"], r.stdout + r.stderr)
        self.assertIs(m.get("android"), False)
        self.assertEqual(sorted(p["artefato"] for p in m["pacotes"]), sorted(a for a, _ in COMPUTADOR))
        p = self.c.portao("nao")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)

    def test_etiqueta_aprova_sem_chave_do_android(self):
        ref = f"refs/tags/v{VERSAO}"
        r, m = self.c.manifesto(jobs_sem_android(), "nao", ref)
        self.assertTrue(m["aprovado"], r.stdout + r.stderr)
        p = self.c.portao("nao", ref)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("Android fora do escopo", p.stdout)

    def test_etiqueta_de_outra_versao_reprova(self):
        p0 = self.c.manifesto(jobs_sem_android(), "nao", "refs/tags/v9.9.9")[0]
        p = self.c.portao("nao", "refs/tags/v9.9.9")
        self.assertNotEqual(p.returncode, 0, p0.stdout + p.stdout)

    def test_falta_o_appimage_reprova(self):
        os.remove(os.path.join(self.c.artefatos, "Rustinho-Linux", COMPUTADOR[0][1]))
        self.c.manifesto(jobs_sem_android(), "nao")
        p = self.c.portao("nao")
        self.assertNotEqual(p.returncode, 0, p.stdout)
        self.assertIn("Rustinho-Linux", p.stdout)

    def test_falta_o_portatil_reprova(self):
        os.remove(os.path.join(self.c.artefatos, "Rustinho-Windows-portatil", COMPUTADOR[2][1]))
        self.c.manifesto(jobs_sem_android(), "nao")
        self.assertNotEqual(self.c.portao("nao").returncode, 0)

    def test_um_job_do_computador_reprovado_reprova(self):
        for job in JOBS_COMPUTADOR:
            with self.subTest(job=job):
                _, m = self.c.manifesto(jobs_sem_android(**{job: "failure"}), "nao")
                self.assertFalse(m["aprovado"])
                self.assertNotEqual(self.c.portao("nao").returncode, 0)

    def test_um_job_do_computador_pulado_reprova(self):
        _, m = self.c.manifesto(jobs_sem_android(verificar="skipped"), "nao")
        self.assertFalse(m["aprovado"])
        self.assertNotEqual(self.c.portao("nao").returncode, 0)

    def test_apk_que_rodou_com_a_chave_desligada_reprova(self):
        # Desligado quer dizer desligado: um job do Android que rodou (por engano no workflow) reprova.
        _, m = self.c.manifesto(jobs_sem_android(apk="success"), "nao")
        self.assertFalse(m["aprovado"])
        self.assertNotEqual(self.c.portao("nao").returncode, 0)

    def test_apk_nos_artefatos_nao_entra_no_manifesto(self):
        for artefato, arquivo in ANDROID:
            self.c.pacote(artefato, arquivo)
        _, m = self.c.manifesto(jobs_sem_android(), "nao")
        self.assertFalse([p for p in m["pacotes"] if p["arquivo"].endswith(".apk")])
        with open(os.path.join(self.c.artefatos, "Manifesto", "SHA256SUMS.txt"), encoding="utf-8") as f:
            self.assertNotIn(".apk", f.read())

    def test_apk_enfiado_no_manifesto_reprova_no_portao(self):
        _, m = self.c.manifesto(jobs_sem_android(), "nao")
        artefato, arquivo = ANDROID[0]
        self.c.pacote(artefato, arquivo)
        caminho = os.path.join(self.c.artefatos, artefato, arquivo)
        h = open(caminho + ".sha256", encoding="utf-8").read().split()[0]
        m["pacotes"].append({"artefato": artefato, "arquivo": arquivo, "bytes": 1, "sha256": h, "anotado_no_job": h})
        with open(os.path.join(self.c.artefatos, "Manifesto", "MANIFESTO.json"), "w", encoding="utf-8") as f:
            json.dump(m, f)
        with open(os.path.join(self.c.artefatos, "Manifesto", "SHA256SUMS.txt"), "a", encoding="utf-8") as f:
            f.write(f"{h}  {arquivo}\n")
        p = self.c.portao("nao")
        self.assertNotEqual(p.returncode, 0, p.stdout)
        self.assertIn("Rustinho-Android", p.stdout)

    def test_chave_diferente_entre_manifesto_e_portao_reprova(self):
        self.c.manifesto(jobs_sem_android(), "nao")
        self.assertNotEqual(self.c.portao("sim").returncode, 0)

    def test_chave_ausente_ou_estranha_reprova(self):
        r, m = self.c.manifesto(jobs_sem_android(), None)
        self.assertFalse(m["aprovado"], r.stdout)
        self.c.manifesto(jobs_sem_android(), "nao")
        for valor in (None, "", "talvez"):
            with self.subTest(valor=valor):
                self.assertNotEqual(self.c.portao(valor).returncode, 0)

    def test_notas_dizem_que_o_android_esta_fora(self):
        self.c.manifesto(jobs_sem_android(), "nao")
        with open(os.path.join(self.c.artefatos, "Manifesto", "NOTAS.md"), encoding="utf-8") as f:
            notas = f.read()
        self.assertIn("Android: fora do escopo", notas)
        self.assertNotIn("Moto e6i", notas)


class ComAndroid(unittest.TestCase):
    """A chave ligada: o comportamento de antes (para quando o Android voltar)."""

    def setUp(self):
        self.c = Caso()
        for artefato, arquivo in COMPUTADOR + ANDROID:
            self.c.pacote(artefato, arquivo)

    def tearDown(self):
        self.c.limpar()

    def test_ramo_aprova_com_os_5_pacotes(self):
        r, m = self.c.manifesto(jobs_com_android(), "sim")
        self.assertTrue(m["aprovado"], r.stdout + r.stderr)
        self.assertEqual(len(m["pacotes"]), 5)
        p = self.c.portao("sim")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)

    def test_etiqueta_sem_a_chave_da_loja_reprova(self):
        ref = f"refs/tags/v{VERSAO}"
        self.c.manifesto(jobs_com_android(), "sim", ref)
        p = self.c.portao("sim", ref)
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("ANDROID_CHAVE_IMPRESSAO vazia", p.stdout)

    def test_etiqueta_com_a_chave_da_loja_aprova(self):
        ref = f"refs/tags/v{VERSAO}"
        self.c.manifesto(jobs_com_android(), "sim", ref)
        p = self.c.portao("sim", ref, {"ANDROID_CHAVE_IMPRESSAO": CHAVE})
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)

    def test_falta_um_apk_reprova(self):
        os.remove(os.path.join(self.c.artefatos, "Rustinho-Android-32bits", ANDROID[1][1]))
        self.c.manifesto(jobs_com_android(), "sim")
        self.assertNotEqual(self.c.portao("sim").returncode, 0)

    def test_apk_pulado_com_a_chave_ligada_reprova(self):
        _, m = self.c.manifesto(jobs_com_android(apk="skipped", emulador="skipped"), "sim")
        self.assertFalse(m["aprovado"])
        self.assertNotEqual(self.c.portao("sim").returncode, 0)


class FormaDaChave(unittest.TestCase):
    """A regra 6 do conferir_portao.py: a chave do Android igual nos quatro lugares, e só ela pula jobs."""

    PASTA = os.path.join(AQUI, "..", "workflows")

    def setUp(self):
        self.raiz = tempfile.mkdtemp(prefix="testar_forma_")
        for arq in os.listdir(self.PASTA):
            shutil.copy(os.path.join(self.PASTA, arq), self.raiz)

    def tearDown(self):
        shutil.rmtree(self.raiz, ignore_errors=True)

    def mudar(self, arq, antes, depois):
        caminho = os.path.join(self.raiz, arq)
        with open(caminho, encoding="utf-8") as f:
            texto = f.read()
        self.assertEqual(texto.count(antes), 1, f"{arq}: o trecho a mudar não é único: {antes!r}")
        with open(caminho, "w", encoding="utf-8") as f:
            f.write(texto.replace(antes, depois))

    def conferir(self):
        return subprocess.run([sys.executable, "-I", os.path.join(AQUI, "conferir_portao.py"), self.raiz],
                              capture_output=True, text=True)

    def test_os_workflows_do_projeto_passam(self):
        r = self.conferir()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("Android fora do escopo", r.stdout)

    def test_ligar_so_no_validar_reprova(self):
        self.mudar("ci.yml", "      android: false\n", "      android: true\n")
        self.assertNotEqual(self.conferir().returncode, 0)

    def test_portao_diferente_do_validar_reprova(self):
        self.mudar("ci.yml", '      RUSTINHO_ANDROID: "nao"\n      ANDROID_CHAVE_IMPRESSAO: ${{ vars.ANDROID_CHAVE_IMPRESSAO }}\n'
                   "    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n"
                   "      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2\n"
                   "        with:\n          path: artefatos\n      - name: Conferir manifesto",
                   '      RUSTINHO_ANDROID: "sim"\n      ANDROID_CHAVE_IMPRESSAO: ${{ vars.ANDROID_CHAVE_IMPRESSAO }}\n'
                   "    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n"
                   "      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2\n"
                   "        with:\n          path: artefatos\n      - name: Conferir manifesto")
        self.assertNotEqual(self.conferir().returncode, 0)

    def test_job_do_computador_pulado_pela_chave_reprova(self):
        self.mudar("validar.yml", "  pacote-linux:\n", "  pacote-linux:\n    if: inputs.android\n")
        self.assertNotEqual(self.conferir().returncode, 0)

    def test_apk_sem_a_chave_reprova(self):
        self.mudar("validar.yml", "  apk:\n    if: inputs.android\n", "  apk:\n")
        self.assertNotEqual(self.conferir().returncode, 0)

    def test_manifesto_com_a_chave_fixa_reprova(self):
        self.mudar("validar.yml", "RUSTINHO_ANDROID: ${{ inputs.android && 'sim' || 'nao' }}", "RUSTINHO_ANDROID: nao")
        self.assertNotEqual(self.conferir().returncode, 0)

    def test_entrada_ligada_por_padrao_reprova(self):
        self.mudar("validar.yml", "        type: boolean\n        required: false\n        default: false\n",
                   "        type: boolean\n        required: false\n        default: true\n")
        self.assertNotEqual(self.conferir().returncode, 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
