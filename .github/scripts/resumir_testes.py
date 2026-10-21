#!/usr/bin/env python3
"""Resume a saída do `cargo test` em anotações do GitHub Actions (avisos que
aparecem na página da montagem e na API), para ler sem baixar os registros.

    python3 .github/scripts/resumir_testes.py saida.txt "Testes no Windows"
"""
import os
import re
import sys

LIMITE = 3500  # tamanho de cada aviso


def aviso(nivel, titulo, linhas):
    texto = "\n".join(linhas)
    partes = [texto[i:i + LIMITE] for i in range(0, len(texto), LIMITE)] or [""]
    for n, p in enumerate(partes[:8], 1):
        corpo = p.replace("%", "%25").replace("\r", "").replace("\n", "%0A")
        sufixo = f" ({n}/{min(len(partes), 8)})" if len(partes) > 1 else ""
        t = f"{titulo}{sufixo}".replace("%", "%25").replace(":", "%3A").replace(",", "%2C")
        print(f"::{nivel} title={t}::{corpo}")


def main():
    arquivo, titulo = sys.argv[1], sys.argv[2]
    t = open(arquivo, encoding="utf-8", errors="replace").read()
    linhas = t.splitlines()
    resultados = [l for l in linhas if l.startswith("test result") or re.match(r"^\s+Running ", l)]
    aviso("notice", f"{titulo}: resultados", resultados)
    falhas = [l for l in linhas if re.match(r"^test .* \.\.\. FAILED$", l)]
    if falhas:
        aviso("error", f"{titulo}: {len(falhas)} falharam", falhas)
        blocos = re.findall(r"^---- (\S+) stdout ----\n(.*?)(?=^---- |^failures:$|\Z)", t, re.S | re.M)
        detalhes = []
        for nome, corpo in blocos:
            util = [l for l in corpo.splitlines() if l.strip() and not re.match(r"^\s+\d+: |^\s+at |note: run with", l)]
            detalhes.append(f"[{nome}]")
            detalhes.extend(x[:300] for x in util[:12])
        aviso("warning", f"{titulo}: detalhes das falhas", detalhes)
    erros = [l for l in linhas if l.startswith("error") and "test failed" not in l][:20]
    if erros:
        aviso("error", f"{titulo}: erros de compilação/execução", erros)
    # Para a página da versão: uma linha com as contas.
    soma = {"passed": 0, "failed": 0, "ignored": 0}
    for l in linhas:
        if l.startswith("test result"):
            for chave in soma:
                m = re.search(rf"(\d+) {chave}", l)
                if m:
                    soma[chave] += int(m.group(1))
    nomes = [re.sub(r"^test (.*) \.\.\. FAILED$", r"\1", l) for l in falhas]
    resumo = f"{soma['passed']} aprovados, {soma['failed']} falharam, {soma['ignored']} só à mão (ignorados)"
    if not resultados:
        resumo = "não terminaram (sem resultado)"
    saida = os.environ.get("GITHUB_OUTPUT")
    if saida:
        with open(saida, "a", encoding="utf-8") as f:
            f.write(f"resumo={resumo}\n")
            f.write(f"falhas={', '.join(nomes) if nomes else 'nenhuma'}\n")
    print(resumo)


if __name__ == "__main__":
    main()
