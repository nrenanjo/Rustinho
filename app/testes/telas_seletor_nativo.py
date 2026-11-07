#!/usr/bin/env python3
"""Teste do seletor de pasta do sistema no Linux (T-16, 0.9.18).

O Rustinho do AppImage (variável APPIMAGE) usa o tema do portal
(xdg-desktop-portal): a janela de pasta é a do desktop (aqui, a do KDE). Sem
portal na sessão, volta à janela do próprio Qt, sem cair. Fora do AppImage
nada muda.

Tudo roda numa sessão fechada, para nenhuma janela abrir na área de trabalho
de verdade: ambiente zerado (sem Wayland), pasta de execução e pasta pessoal
próprias, um D-Bus só do teste (com apenas os serviços do portal) e a tela
virtual. Antes de clicar, o teste confere que os processos do portal estão
presos à tela virtual; se não estiverem, para.

  P1. AppImage, sessão sem portal: a janela de pasta do Qt abre e a pasta
      escolhida chega à importação.
  P2. AppImage, sessão com o portal do KDE: abre a janela do KDE (do processo
      do portal), nenhuma do Qt; a pasta escolhida nela chega à importação.
  P3. Fora do AppImage, com o portal na sessão: a janela continua a do Qt.

Precisa de: Xvfb, xdotool, ImageMagick (import), dbus-daemon e, para o P2,
xdg-desktop-portal e xdg-desktop-portal-kde. Sem eles, o P2 é pulado e o
teste diz isso. Use o Rustinho com o Qt dos pacotes (6.8.3), como o AppImage:

    RUSTINHO_PROGRAMA=... LD_LIBRARY_PATH=<Qt 6.8.3>/lib \\
        python3 app/testes/telas_seletor_nativo.py [pasta para as fotos]

Sai com 0 se tudo conferiu.
"""

import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import telas_d09 as base  # noqa: E402
import telas_navegacao_guiada as nav  # noqa: E402

base.TELA = os.environ.get("RUSTINHO_TELA", ":81")
nav.TELA = base.TELA
TELA = base.TELA
PROGRAMA = base.PROGRAMA
PORTAL = "/usr/lib/xdg-desktop-portal"
PORTAL_KDE = "/usr/lib/xdg-desktop-portal-kde"
TITULO = "Escolha a pasta com os XMLs"


def xd(*args):
    return subprocess.run(["xdotool", *args], env=dict(os.environ, DISPLAY=TELA),
                          capture_output=True, text=True).stdout


def janelas(classe):
    return xd("search", "--onlyvisible", "--class", classe).split()


def geometria(janela):
    d = dict(x.split("=", 1) for x in xd("getwindowgeometry", "--shell", janela).split() if "=" in x)
    return int(d["X"]), int(d["Y"]), int(d["WIDTH"]), int(d["HEIGHT"])


def ambiente_fechado(runtime, casa):
    """Só o necessário, nada herdado da sessão de quem roda o teste."""
    env = {
        "PATH": "/usr/local/bin:/usr/bin:/bin",
        "LANG": os.environ.get("LANG", "C.UTF-8"),
        "HOME": casa,
        "XDG_RUNTIME_DIR": runtime,
        "XDG_CONFIG_HOME": os.path.join(casa, ".config"),
        "XDG_DATA_HOME": os.path.join(casa, ".local", "share"),
        "XDG_CACHE_HOME": os.path.join(casa, ".cache"),
        "DISPLAY": TELA,
        "QT_QPA_PLATFORM": "xcb",
        "XDG_CURRENT_DESKTOP": "KDE",
        "XDG_SESSION_TYPE": "x11",
        "KDE_FULL_SESSION": "true",
        "KDE_SESSION_VERSION": "6",
        "QT_FORCE_STDERR_LOGGING": "1",
        # O D-Bus do teste (os serviços que ele iniciar também usam este).
        "DBUS_SESSION_BUS_ADDRESS": f"unix:path={runtime}/bus",
    }
    return env


def barramento(runtime, servicos, com_portal):
    """Um D-Bus só do teste. Com o portal: só o xdg-desktop-portal e o do KDE."""
    os.makedirs(servicos, exist_ok=True)
    if com_portal:
        for nome, programa in (("org.freedesktop.portal.Desktop", PORTAL),
                               ("org.freedesktop.impl.portal.desktop.kde", PORTAL_KDE)):
            with open(os.path.join(servicos, nome + ".service"), "w") as f:
                f.write(f"[D-BUS Service]\nName={nome}\nExec={programa}\n")
    config = os.path.join(runtime, "barramento.conf")
    with open(config, "w") as f:
        f.write(f"""<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={runtime}/bus</listen>
  <servicedir>{servicos}</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
""")
    return config


def processos_da_sessao(runtime):
    """PIDs dos processos que carregam a pasta de execução do teste."""
    marca = ("XDG_RUNTIME_DIR=" + runtime).encode()
    achados = []
    for pid in filter(str.isdigit, os.listdir("/proc")):
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                if marca in f.read().split(b"\0"):
                    achados.append(int(pid))
        except OSError:
            pass
    return achados


def ambiente_de(pid):
    with open(f"/proc/{pid}/environ", "rb") as f:
        return dict(x.split(b"=", 1) for x in f.read().split(b"\0") if b"=" in x)


def encerrar_sessao(runtime):
    for pid in processos_da_sessao(runtime):
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
    time.sleep(1)
    for pid in processos_da_sessao(runtime):
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass


def presos_a_tela_virtual(runtime):
    """Os processos da sessão do teste não podem alcançar a área de trabalho."""
    for pid in processos_da_sessao(runtime):
        try:
            e = ambiente_de(pid)
        except OSError:
            continue
        if b"WAYLAND_DISPLAY" in e or e.get(b"DISPLAY") != TELA.encode():
            return False, pid
    return True, None


def main():
    for p in ("Xvfb", "xdotool", "import", "dbus-daemon"):
        if not shutil.which(p):
            sys.exit(f"falta o programa {p}")
    if not os.path.exists(PROGRAMA):
        sys.exit(f"compile antes: cargo build -p rustinho ({PROGRAMA})")
    tem_portal = os.path.exists(PORTAL) and os.path.exists(PORTAL_KDE)
    fotos = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix="rustinho-fotos-seletor-")
    os.makedirs(fotos, exist_ok=True)
    xvfb = subprocess.Popen(["Xvfb", TELA, "-screen", "0", "1440x1000x24"], stderr=subprocess.DEVNULL)
    time.sleep(1)
    if xvfb.poll() is not None:
        sys.exit(f"a tela virtual {TELA} já está em uso; escolha outra com RUSTINHO_TELA=:NN")
    raiz = tempfile.mkdtemp(prefix="rustinho-seletor-")
    modelo = os.path.join(raiz, "loja-modelo")
    os.makedirs(modelo)
    certo = True
    original = dict(os.environ)

    def ok(nome, condicao, foto=""):
        nonlocal certo
        print(("✔ " if condicao else "✘ ") + nome + ("" if condicao else f" (foto: {foto})"), flush=True)
        certo &= bool(condicao)

    def caso(nome, appimage, com_portal):
        """Abre Peças › Importar XML › Escolher uma pasta numa sessão fechada.
        Devolve (rustinho, runtime, dbus) já com a janela pedida."""
        runtime = tempfile.mkdtemp(prefix="run-", dir=raiz)
        os.chmod(runtime, 0o700)
        pasta = os.path.join(raiz, nome, "loja")
        shutil.copytree(modelo, pasta)
        casa = nav.casa_com_xmls(raiz, ["a-nota.xml"])
        env = ambiente_fechado(runtime, casa)
        config = barramento(runtime, os.path.join(runtime, "servicos"), com_portal)
        # Como numa sessão de verdade, os serviços começam na pasta pessoal.
        dbus = subprocess.Popen(["dbus-daemon", "--config-file", config, "--nofork", "--nopidfile"],
                                env=env, cwd=casa, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)
        # O que o Rustinho pede ao portal (a pasta inicial vai em current_folder).
        pedido = open(os.path.join(fotos, nome + "-dbus.txt"), "w")
        monitor = subprocess.Popen(["dbus-monitor", "--address", f"unix:path={runtime}/bus",
                                    "interface='org.freedesktop.portal.FileChooser'"],
                                   env=env, stdout=pedido, stderr=subprocess.DEVNULL)
        # O Qt dos pacotes (LD_LIBRARY_PATH) só para o Rustinho: o portal do
        # KDE é do sistema e usa o Qt do sistema.
        if original.get("LD_LIBRARY_PATH"):
            env["LD_LIBRARY_PATH"] = original["LD_LIBRARY_PATH"]
        if appimage:
            env["APPIMAGE"] = os.path.join(raiz, "Rustinho-teste.AppImage")
        os.environ.clear()
        os.environ.update(env)
        try:
            r = nav.Rustinho(pasta, "TelaPecas", {}, fotos, nome, casa)
        finally:
            os.environ.clear()
            os.environ.update(original)
        r.casa, r.runtime, r.monitor, r.pedido = casa, runtime, monitor, pedido
        return r, runtime, dbus

    def fechar(r, runtime, dbus):
        r.fechar()
        r.monitor.terminate()
        r.pedido.close()
        dbus.terminate()
        try:
            dbus.wait(5)
        except subprocess.TimeoutExpired:
            dbus.kill()
        encerrar_sessao(runtime)

    def pedir_pasta(r, nome):
        r.entrar("Ana")
        ok(f"{nome}: Peças aberta", r.esperar("Importar XML"))
        presos, pid = presos_a_tela_virtual(r.runtime)
        if not presos:
            encerrar_sessao(r.runtime)
            sys.exit(f"o processo {pid} da sessão do teste não está preso à tela virtual: parei antes de abrir janelas")
        r.clicar("Importar XML")
        time.sleep(1.5)
        ok(f"{nome}: Escolher uma pasta", r.clicar("Escolher uma pasta"))
        time.sleep(4)

    def chegou_a_importacao(r):
        return r.esperar("Falta o CNPJ da loja", segundos=20)

    try:
        base.montar_loja(modelo)

        # ---------------- P1. AppImage sem portal: janela do Qt ----------------
        r, runtime, dbus = caso("P1-sem-portal", appimage=True, com_portal=False)
        try:
            pedir_pasta(r, "P1")
            f = r.foto("-janela")
            qt = [j for j in janelas("rustinho") if TITULO in xd("getwindowname", j)]
            ok("P1: a janela de pasta é a do Qt (sem portal na sessão)", bool(qt), f)
            ok("P1: o Rustinho continua aberto", r.p.poll() is None, f)
            if qt:
                # Subir marca Downloads (a pasta de onde se veio); depois, Abrir.
                x, y, largura, altura = geometria(qt[0])
                xd("mousemove", str(x + 41), str(y + 41), "click", "1")
                time.sleep(1.5)
                xd("mousemove", str(x + largura - 180), str(y + altura - 30), "click", "1")
                time.sleep(3)
                nav.devolver_teclado()
            ok("P1: a pasta chegou à importação", chegou_a_importacao(r), r.foto("-depois"))
        finally:
            fechar(r, runtime, dbus)

        # ---------------- P2. AppImage com o portal do KDE ----------------
        if not tem_portal:
            print("– P2 pulado: xdg-desktop-portal ou xdg-desktop-portal-kde não instalado")
        else:
            r, runtime, dbus = caso("P2-portal-kde", appimage=True, com_portal=True)
            try:
                pedir_pasta(r, "P2")
                kde = []
                for _ in range(20):
                    kde = janelas("xdg-desktop-portal-kde")
                    if kde:
                        break
                    time.sleep(0.5)
                f = r.foto("-janela")
                ok("P2: abriu a janela de pasta do KDE (processo do portal)", bool(kde), f)
                qt = [j for j in janelas("rustinho") if TITULO in xd("getwindowname", j)]
                ok("P2: nenhuma janela de pasta do Qt", not qt, f)
                presos, _ = presos_a_tela_virtual(runtime)
                ok("P2: o portal está preso à tela virtual", presos, f)
                if kde:
                    # Na janela do KDE: o caminho no campo do local, Enter (vai
                    # para a pasta) e o botão Abrir (canto de baixo, à direita).
                    downloads = os.path.join(r.casa, "Downloads")
                    xd("windowfocus", "--sync", kde[0])
                    time.sleep(0.5)
                    xd("key", "ctrl+l")
                    time.sleep(0.3)
                    xd("key", "ctrl+a")
                    xd("type", "--delay", "30", downloads)
                    time.sleep(0.5)
                    # O primeiro Enter pode só aceitar a sugestão de completar.
                    for _ in range(2):
                        xd("key", "Return")
                        time.sleep(1.5)
                    r.foto("-na-pasta")
                    if janelas("xdg-desktop-portal-kde"):
                        x, y, largura, altura = geometria(kde[0])
                        xd("mousemove", str(x + largura - 147), str(y + altura - 23), "click", "1")
                        time.sleep(3)
                    nav.devolver_teclado()
                with open(os.path.join(fotos, "P2-portal-kde-dbus.txt"), errors="replace") as f:
                    pedido = f.read()
                ok("P2: o Rustinho pediu a pasta inicial ao portal (current_folder)", "current_folder" in pedido,
                   os.path.join(fotos, "P2-portal-kde-dbus.txt"))
                ok("P2: a pasta escolhida na janela do KDE chegou à importação", chegou_a_importacao(r),
                   r.foto("-depois"))
            finally:
                fechar(r, runtime, dbus)

        # ---------------- P3. Fora do AppImage: nada muda ----------------
        if tem_portal:
            r, runtime, dbus = caso("P3-sem-appimage", appimage=False, com_portal=True)
            try:
                pedir_pasta(r, "P3")
                f = r.foto("-janela")
                qt = [j for j in janelas("rustinho") if TITULO in xd("getwindowname", j)]
                ok("P3: fora do AppImage, a janela continua a do Qt", bool(qt) and not janelas("xdg-desktop-portal-kde"),
                   f)
            finally:
                fechar(r, runtime, dbus)
    finally:
        xvfb.terminate()
        shutil.rmtree(raiz, ignore_errors=True)
    print("fotos em", fotos)
    sys.exit(0 if certo else 1)


if __name__ == "__main__":
    main()
