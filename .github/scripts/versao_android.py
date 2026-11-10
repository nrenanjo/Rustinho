#!/usr/bin/env python3
"""Versão do APK a partir da versão da raiz (`Cargo.toml`): nome e versionCode.

DP-36 (dev.3.3, AUD33-J08): cada versão de desenvolvimento tem um versionCode
próprio, e a versão final continua atualizando por cima de todas as de
desenvolvimento dela.

    versionCode = A·10.000.000 + B·100.000 + C·1.000 + S
    S = 999 na versão final (A.B.C)
    S = X·100 + Y·10 + Z em A.B.C-dev.X.Y.Z (cada parte de 0 a 9; "-dev" sozinho = 0;
        no máximo 998, abaixo da final)

Exemplos: 0.9.18-dev.3.2 → 918320; 0.9.18-dev.3.3 → 918330; 0.9.18 → 918999.
O esquema antigo dava A·10.000 + B·100 + C (0.9.17 → 917, toda a 0.9.18 → 918):
todo código novo é maior, então os APKs já instalados atualizam.
Outra forma de versão (ou parte maior que 9) para a montagem com erro, em vez
de gerar um código fora de ordem.

    python3 .github/scripts/versao_android.py            # lê o Cargo.toml da raiz
    python3 .github/scripts/versao_android.py 0.9.18-dev.3.3
    python3 .github/scripts/versao_android.py --testar
"""
import os
import re
import sys


def ler_versao_da_raiz(raiz="."):
    with open(os.path.join(raiz, "Cargo.toml"), encoding="utf-8") as f:
        for linha in f:
            m = re.match(r'^version\s*=\s*"([^"]+)"', linha)
            if m:
                return m.group(1)
    raise SystemExit("Cargo.toml da raiz sem version")


def codigo(versao):
    m = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)(?:-dev((?:\.\d+){0,3}))?", versao)
    if not m:
        raise ValueError(f"versão fora do formato A.B.C ou A.B.C-dev[.X[.Y[.Z]]]: {versao}")
    a, b, c = (int(x) for x in m.group(1, 2, 3))
    if b > 99 or c > 99:
        raise ValueError(f"B e C vão até 99: {versao}")
    if m.group(4) is None and "-" not in versao:
        s = 999
    else:
        partes = [int(x) for x in (m.group(4) or "").split(".") if x]
        if any(p > 9 for p in partes):
            raise ValueError(f"cada parte depois de -dev vai de 0 a 9: {versao}")
        partes += [0] * (3 - len(partes))
        s = partes[0] * 100 + partes[1] * 10 + partes[2]
        if s >= 999:
            raise ValueError(f"-dev.9.9.9 teria o código da versão final: {versao}")
    return a * 10_000_000 + b * 100_000 + c * 1_000 + s


def testar():
    casos = {
        "0.9.18-dev.3.2": 918320,
        "0.9.18-dev.3.3": 918330,
        "0.9.18-dev": 918000,
        "0.9.18-dev.2": 918200,
        "0.9.18": 918999,
        "0.9.19-dev.1": 919100,
        "0.9.18-dev.3.3.1": 918331,
        "1.0.0": 10_000_999,
    }
    for v, esperado in casos.items():
        assert codigo(v) == esperado, (v, codigo(v), esperado)
    ordem = ["0.9.17", "0.9.18-dev", "0.9.18-dev.2", "0.9.18-dev.3.1", "0.9.18-dev.3.2", "0.9.18-dev.3.3",
             "0.9.18-dev.9.9.8", "0.9.18", "0.9.19-dev.1", "0.9.19", "1.0.0"]
    codigos = [codigo(v) for v in ordem]
    assert codigos == sorted(codigos) and len(set(codigos)) == len(codigos), list(zip(ordem, codigos))
    assert codigo("0.9.17") > 918, "maior que todo código do esquema antigo"
    for ruim in ["0.9.18-rc.1", "0.9.18-dev.10", "0.9.18-dev.9.9.9", "0.9", "0.9.18-dev.3.3.1.1", "0.100.0"]:
        try:
            codigo(ruim)
        except ValueError:
            continue
        raise AssertionError(f"devia recusar {ruim}")
    print("versao_android: ok")


def main():
    if sys.argv[1:] == ["--testar"]:
        testar()
        return
    versao = sys.argv[1] if len(sys.argv) > 1 else ler_versao_da_raiz()
    base = versao.split("-", 1)[0]
    print(f"VERSAO={versao}")
    print(f"BASE={base}")
    print(f"CODIGO={codigo(versao)}")


if __name__ == "__main__":
    main()
