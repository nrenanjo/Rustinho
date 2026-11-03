# BLOCO 11 — Auditoria do Módulo Fiscal, Leitura de NF-e XML, Assinatura Digital ICP-Brasil (PEND-09) e Impressão A4 (Núcleo + UI)

**Data da Auditoria:** 2026-11-02  
**Versão Auditada:** Rustinho 0.9.17  
**Escopo do Bloco:** Auditoria integral de código-fonte (leitura linha a linha) dos módulos fiscais e criptográficos do núcleo Rust (`fiscal.rs`, `nfe_xml.rs`, `nfe_assinatura/mod.rs`, `nfe_assinatura/ancoras.rs`, `nfe_assinatura/c14n.rs`, `nfe_assinatura/testes.rs`, `impressao.rs`), artefatos de cadeias ICP-Brasil (`nucleo/dados/icp-brasil/*`), arquivos XML de exemplo (`exemplos/nfe_exemplo.xml` e `nucleo/tests/nfe_exemplo.xml`) e componentes QML fiscais (`app/qml/EmissaoFiscal.qml`, `app/qml/LinhaRegraSt.qml`, além de `app/qml/TelaFiscal.qml` já lido no Bloco 8B2).  
**Restrição de Imutabilidade:** Cumprida integralmente (`allowLegacyEdits: false`). Nenhum arquivo do projeto legado foi modificado.

---

## 1. Inventário de Arquivos Lidos Integralmente neste Bloco

| # | Arquivo | Linhas Lidas | Total de Linhas | `#[test]` Encontrados | Escopo Auditado |
|---|---|---:|---:|---:|---|
| 1 | [nucleo/src/fiscal.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/fiscal.rs) | 1–728 | 728 | 2 | Regras de ST por UF/CEST/vigência (`regra_st`), decisão automática de CSOSN (`102`/`500`) e CFOP (`5102`/`5405`), conferência fiscal de peças (`conflitos`, `sem_ncm`, `sem_cest`, `monofasicas`), migração para CSOSN automático e estrutura JSON `FiscalPeca` (IPI, PIS/Cofins, ANP, ISS). |
| 2 | [nucleo/src/nfe_xml.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_xml.rs) | 1–607 | 607 | 5 | Leitura de XML de NF-e (modelo 55) e NFC-e (modelo 65 para cadastro), integração com `nfe_assinatura::verificar`, extração do nó `infNFe` verificado, chave de acesso numérica e alfanumérica (PEND-19 / NT Conjunta 2025.001, módulo 11 com `ASCII - 48`), coerência com `protNFe`. |
| 3 | [nucleo/src/nfe_assinatura/mod.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/mod.rs) | 1–759 | 759 | 0 *(em `testes.rs`)* | Verificador criptográfico completo de NF-e (`PEND-09`): limites de tamanho (5 MB) e profundidade (64 níveis), perfil estrutural estrito contra ataques de XML Signature Wrapping (XSW), algoritmos RSA-SHA1/RSA-SHA256 (`ring`), chave RSA ≥ 2048 bits, perfil X.509 ICP-Brasil A1/A3 (`KeyUsage`, `ExtendedKeyUsage`, `BasicConstraints`, `CertificatePolicies`), validação temporal dupla (`dhEmi` e `dhRecbto`), cadeia até âncora ICP-Brasil (`rustls-webpki` + validação de `keyCertSign` e validade da raiz), titularidade por raiz de CNPJ (8 posições, inclusive alfanumérico) ou CPF (`otherName` OID `2.16.76.1.3.3` / `2.16.76.1.3.1`) e bloqueio de homologação (`tpAmb != 1`). |
| 4 | [nucleo/src/nfe_assinatura/ancoras.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/ancoras.rs) | 1–241 | 241 | 0 *(em `testes.rs`)* | Carregamento e verificação SHA-512 do pacote embutido `ACcompactadox.zip` do ITI, seleção exclusiva por SHA-256 das raízes ICP-Brasil `v5` e `v12` (`Vigente`) e `v2` (`Historica`), separação de ACs intermediárias e bloqueio de certificados autoassinados não escolhidos. |
| 5 | [nucleo/src/nfe_assinatura/c14n.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/c14n.rs) | 1–154 | 154 | 0 *(em `testes.rs`)* | Canonicalização XML 1.0 inclusiva sem comentários (`REC-xml-c14n-20010315`) em perfil fechado (sem prefixos de elementos/atributos, sem `xml:*`, sem PIs). |
| 6 | [nucleo/src/nfe_assinatura/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/testes.rs) | 1–593 | 593 | 31 *(29 ativos + 2 `#[ignore]`)* | Suíte de testes de aceitação `A-01` a `A-55` e `PEND-19` para `nfe_assinatura` e `c14n`. |
| 7 | [nucleo/src/impressao.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/impressao.rs) | 1–624 | 624 | 1 | Geração de HTML para impressão A4/PDF: comprovante de venda, recibo de crediário, extrato de cliente, orçamento, fechamento de caixa, lista de compra, DRE gerencial, comprovante de devolução, vale-troca e ordem de serviço. |
| 8 | [nucleo/dados/icp-brasil/hashsha512x.txt](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/dados/icp-brasil/hashsha512x.txt) | 1–1 | 1 | 0 | Hash SHA-512 oficial publicado pelo ITI para `ACcompactadox.zip` (embutido via `include_bytes!`). |
| 9 | [exemplos/nfe_exemplo.xml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/exemplos/nfe_exemplo.xml) | 1–66 | 66 | 0 | XML de exemplo na pasta pública `exemplos/` na raiz do repositório. |
| 10 | [nucleo/tests/nfe_exemplo.xml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/tests/nfe_exemplo.xml) | 1–67 | 67 | 0 | XML de exemplo assinado pela PKI de teste usado pelos testes internos do núcleo. |
| 11 | [app/qml/EmissaoFiscal.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/EmissaoFiscal.qml) | 1–170 | 170 | 0 | Subseção honesta de Emissão Fiscal em Configurações (RVI-02 / D-11: informa que a emissão de NFC-e/NF-e ainda não está implementada, sem fingir estado). |
| 12 | [app/qml/LinhaRegraSt.qml](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaRegraSt.qml) | 1–111 | 111 | 0 | Delegate de edição de regra de Substituição Tributária (`uf`, `cestInicio`, `comSt`, `vigenteDe`, `vigenteAte`, `descricao`) em `TelaFiscal.qml`. |

---

## 2. Achados Descobertos na Auditoria do Bloco 11

### 2.1. Defeitos Comprovados

#### `[NEW-B11-01]` `nfe_xml::verificada` chama `ler_nota(xml)` sem limite de tamanho e com `allow_dtd: true` quando `nfe_assinatura::verificar` recusa um arquivo com `Motivo::Malformado`
- **Classificação:** **Defeito comprovado** (Contorno de Proteção DoS / XML Entity Expansion no Tratamento de Erro)
- **Severidade:** Média / Alta
- **Arquivos e Linhas:**
  - [`nucleo/src/nfe_xml.rs:194-224`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_xml.rs#L194-L224)
  - [`nucleo/src/nfe_assinatura/mod.rs:469-476`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/mod.rs#L469-L476)
- **Evidência:**
  Em `nfe_assinatura::verificar_com` (`mod.rs:469–476`), o verificador protege o processo contra arquivos gigantes (`> 5 MB`) e contra bombas de expansão de entidades XML (`<!DOCTYPE ... <!ENTITY ...>>`, Billion Laughs) usando `ParsingOptions { allow_dtd: false, nodes_limit: 1_000_000 }`, retornando `Err(Recusa { motivo: Motivo::Malformado, ... })`:
  ```rust
  // nucleo/src/nfe_assinatura/mod.rs:469-476
  if x.len() > TAMANHO_MAXIMO {
      return Err(recusa(Motivo::Malformado, "maior que 5 MB"));
  }
  ...
  let opcoes = ParsingOptions { allow_dtd: false, nodes_limit: 1_000_000, ..ParsingOptions::default() };
  let doc = Document::parse_with_options(x, opcoes).map_err(|e| recusa(Motivo::Malformado, e.to_string()))?;
  ```
  Contudo, em [`nfe_xml::verificada`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_xml.rs#L200-L210), quando `nfe_assinatura::verificar` retorna `Err(r)` com `Motivo::Malformado` (ou `Motivo::Estrutura`), o código chama `ler_nota(xml, aceita_nfce)?` na linha `206`:
  ```rust
  // nucleo/src/nfe_xml.rs:200-210
  let v = match resultado {
      Ok(v) => v,
      Err(r) => {
          // Arquivo que nem é NF-e (CT-e, evento, resumo...): a mensagem
          // de antes explica melhor o que fazer.
          if matches!(r.motivo, nfe_assinatura::Motivo::Estrutura | nfe_assinatura::Motivo::Malformado) {
              ler_nota(xml, aceita_nfce)?;
          }
          return Err(format!("{r} Peça ao fornecedor o XML original ou baixe-o no portal da SEFAZ."));
      }
  };
  ```
  E `ler_nota` (`nfe_xml.rs:216–224`) **não verifica `TAMANHO_MAXIMO`** e invoca `Document::parse(xml)` (cujo padrão no `roxmltree` é **`allow_dtd: true`** e sem `nodes_limit`)!
- **Impacto:** Um XML malicioso com `<!DOCTYPE ... <!ENTITY ...>>` (expansão exponencial de entidades) ou com dezenas de megabytes, que foi corretamente barrado em `nfe_assinatura::verificar_com` com `Motivo::Malformado`, é imediatamente submetido a `Document::parse(xml)` com `allow_dtd: true` e sem limite de tamanho/nós dentro do bloco `Err(r)` de `nfe_xml::verificada`, anulando na prática a barreira de DoS de `nfe_assinatura`!

---

#### `[NEW-B11-02]` O arquivo público `exemplos/nfe_exemplo.xml` está desatualizado (pré-PEND-09), sem `<Signature>`, sem `<tpAmb>`, com dígito verificador incorreto (`7` em vez de `4`) e é rejeitado na importação pelo próprio Rustinho 0.9.17
- **Classificação:** **Defeito comprovado** (Artefato de Exemplo Quebrado / Divergência com `nucleo/tests/nfe_exemplo.xml`)
- **Severidade:** Baixa / Média
- **Arquivos e Linhas:**
  - [`exemplos/nfe_exemplo.xml:1-66`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/exemplos/nfe_exemplo.xml#L1-L66)
  - [`nucleo/tests/nfe_exemplo.xml:1-67`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/tests/nfe_exemplo.xml#L1-L67)
- **Evidência:**
  Enquanto `nucleo/tests/nfe_exemplo.xml` foi atualizado no `PEND-09` com a chave correta (`NFe35261011222333000181550010000123451000000014`, dígito `4`), tags `<cNF>`, `<tpEmis>`, `<tpAmb>1</tpAmb>`, `<cDV>4</cDV>`, elemento `<Signature>` e `<infProt>` completo (`dhRecbto`, `nProt`, `digVal`), o arquivo `exemplos/nfe_exemplo.xml` distribuído para demonstração ao usuário permaneceu na versão antiga:
  - Linha 4: `Id="NFe35261011222333000181550010000123451000000017"` (dígito `7` inválido pelo módulo 11);
  - Linha 64: `</NFe>` sem `<Signature>`;
  - Linha 65: `<infProt><chNFe>...0017</chNFe><cStat>100</cStat></infProt>` sem `<dhRecbto>`, `<nProt>` e `<digVal>`.
- **Impacto:** Se o usuário tentar testar a tela de Compras (`SecaoComprar.qml`) ou a Importação de XML (`SecaoImportar.qml`) carregando `exemplos/nfe_exemplo.xml`, o Rustinho recusa o próprio arquivo de exemplo com erro de estrutura (`"O XML não está no formato da NF-e assinada (NFe deve ter infNFe e uma Signature)"`).

---

#### `[NEW-B11-03]` `fiscal::tributacao` e `fiscal::conferir` não emitem alerta de conflito quando uma peça tem CSOSN manual `201`, `202` ou `203` (com ST) e a regra estadual indica que a peça não tem ST (`com_st == Some(false)`)
- **Classificação:** **Defeito comprovado** (Omissão na Conferência Fiscal de Conflitos de ST)
- **Severidade:** Baixa / Média
- **Arquivo e Linhas:** [`nucleo/src/fiscal.rs:99-109, 178-180`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/fiscal.rs#L99-L109)
- **Evidência:**
  Na linha `99`, `fiscal::tributacao` reconhece que `"500" | "201" | "202" | "203"` são CSOSNs com Substituição Tributária (atribuindo-lhes `CFOP 5405`):
  ```rust
  let cfop = if matches!(csosn.as_str(), "500" | "201" | "202" | "203") { "5405" } else { "5102" }.to_string();
  let alerta = match (csosn.as_str(), com_st) {
      ("500", Some(false)) => format!(
          "Esta peça está com CSOSN 500, mas pela regra de {uf} ela não tem mais ST nesta data. O certo deve ser 102 (CFOP 5102). Confirme com o contador."
      ),
      ("102", Some(true)) if !automatico => format!(
          "Esta peça está com CSOSN 102, mas pela regra de {uf} ela tem ST nesta data. O certo deve ser 500 (CFOP 5405). Confirme com o contador."
      ),
      _ => String::new(),
  };
  ```
  Porém, o `match` de `alerta` nas linhas `100–108` só testa `"500"` contra `Some(false)`, ignorando `"201"`, `"202"` e `"203"`.
- **Impacto:** Peças cadastradas manualmente com CSOSN `201`, `202` ou `203` continuam recebendo `CFOP 5405` sem aparecer na lista de `conflitos` de `TelaFiscal.qml` quando o estado retira o CEST da Substituição Tributária.

---

#### `[NEW-B11-04]` `fiscal::usar_automatico` altera a tabela `produto` sem verificar `sync::exigir_editavel(conn, "produto")`
- **Classificação:** **Defeito comprovado** (Quebra de Invariante de Sincronização Multi-Caixa)
- **Severidade:** Baixa / Média
- **Arquivo e Linhas:** [`nucleo/src/fiscal.rs:209-234`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/fiscal.rs#L209-L234)
- **Evidência:**
  Embora `api.rs` verifique `sync::exigir_editavel(conn, "produto")` nas rotas de cadastro de produtos, a rota `fiscal_usar_automatico` chama `fiscal::usar_automatico`, que executa `UPDATE produto SET csosn = '' ...` diretamente sem verificar se a tabela `produto` é editável neste computador segundo a topologia de sincronização.

---

### 2.2. Riscos e Limitações Documentadas Verificadas

#### `[NEW-B11-05]` Verificação de `protNFe` limita-se à coerência sintática (`cStat`, `chNFe`, `nProt`, `digVal == DigestValue`) porque o protocolo SEFAZ distribuído não possui assinatura XML própria (`PEND-15` / Decisão `D7`)
- **Classificação:** **Risco** (Limitação Inerente ao Padrão `nfeProc` Offline, já documentada em `docs/PEND-09-criterios.md` e mitigada por `D7`)
- **Severidade:** Informativa / Arquitetural
- **Arquivos e Linhas:**
  - [`nucleo/src/nfe_xml.rs:36-46, 479-492`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_xml.rs#L36-L46)
  - [`nucleo/src/nfe_assinatura/mod.rs:3-7`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/mod.rs#L3-L7)
- **Análise:** A implementação de `nfe_assinatura` prova criptograficamente que o nó `infNFe` foi assinado por um certificado ICP-Brasil A1/A3 válido na data da emissão (`dhEmi`) e cujo titular (raiz de 8 posições do CNPJ ou CPF) coincide com o `<emit>` da nota. Como o bloco `<protNFe>` nos arquivos `nfeProc` não é assinado pela SEFAZ, a confirmação de que a nota não foi cancelada ou denegada depende da consulta online à SEFAZ (`PEND-15`, futuro) ou da confirmação presencial pelo gerente (`D7`).

---

## 3. Registro Separado de Testes Encontrados no Código vs. Testes Executados

### 3.1. Testes Unitários Lidos no Código-Fonte (39 funções `#[test]`)

| Arquivo | Quantidade de `#[test]` | Destaques dos Testes Lidos |
|---|---:|---|
| [nucleo/src/fiscal.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/fiscal.rs#L665-L728) | 2 | `autopecas_sp_saem_da_st_em_outubro_de_2026`, `regras_editaveis_e_conferencia`. |
| [nucleo/src/nfe_xml.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_xml.rs#L499-L607) | 5 | `decimais`, `le_nota_de_exemplo`, `pend19_chave_com_cnpj_alfanumerico`, `recusa_o_que_nao_e_nfe`, `data_malformada_nao_derruba`. |
| [nucleo/src/nfe_assinatura/testes.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/nfe_assinatura/testes.rs#L46-L580) | 31 *(29 ativos + 2 `#[ignore]`)* | Cobertura exemplar dos critérios `A-03` a `A-54` e `PEND-19`: adulteração de campos assinados (`A-05..A-08`), recálculo de `DigestValue` sem chave privada (`A-09`), troca de assinatura/certificado (`A-10..A-13`), quebra de cadeia (`A-14`), validade temporal dupla em `dhEmi` e `dhRecbto` (`A-15..A-16c`), perfil X.509 `KeyUsage`/`EKU`/`CertificatePolicies`/`BasicConstraints`/`keyCertSign` (`A-49..A-54`), RSA-1024, titularidade CNPJ numérico/alfanumérico e CPF (`A-18`, `PEND-19`), homologação (`A-19`), 14 vetores de XML Signature Wrapping (`A-21`), algoritmos proibidos (`A-22`), entradas malformadas/profundas/DTD (`A-23`), integridade SHA-512 do pacote ITI (`A-48`), raízes ICP-Brasil (`A-47`) e paridade C14N com `libxml2` (`A-03`). |
| [nucleo/src/impressao.rs](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/impressao.rs#L616-L624) | 1 | `escapa_html`. |

### 3.2. Testes Executados neste Bloco
- **Nenhum teste foi executado neste bloco.** A execução da suíte de testes está isolada e programada para o **Bloco 18**.

---

## 4. Resumo Consolidado do Bloco 11

| Categoria | Quantidade | Identificadores |
|---|---:|---|
| Novos Defeitos Comprovados | 4 | `NEW-B11-01`, `NEW-B11-02`, `NEW-B11-03`, `NEW-B11-04` |
| Riscos / Limitações Documentadas | 1 | `NEW-B11-05` |
| Arquivos Auditados Integralmente | 12 | 7 Rust (`3.706` linhas) + 3 dados/XML (`134` linhas + ZIP ITI) + 2 QML (`281` linhas) = `4.121` linhas |

