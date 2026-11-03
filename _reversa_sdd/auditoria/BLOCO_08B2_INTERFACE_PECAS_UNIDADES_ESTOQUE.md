# BLOCO 8B2 — Interface de Busca, Peças, Unidades, Etiquetas e Estoque

**Fase do Plano:** Fase 2 — Auditoria de Código e Revalidação dos Achados (Bloco 8B2)  
**Data da Auditoria:** 2026-11-02  
**Status:** CONCLUÍDO ✅

---

## 1. Escopo Auditado (Leitura Integral)

Todos os 11 arquivos previstos para o Bloco 8B2 no `.reversa/plan.md` foram lidos na íntegra, totalizando **3.045 linhas** de código-fonte (Rust + QML):

| Arquivo | Linhas | Escopo Principal |
|---|---|---|
| [`nucleo/src/estoque.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/estoque.rs#L1-L97) | 97 | Cálculo de saldo (`saldo`), lançamento de movimentos (`lancar`) e ajuste de estoque com auditoria (`ajustar`) |
| [`app/qml/EscolherUnidade.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/EscolherUnidade.qml#L1-L127) | 127 | Modal de seleção e criação de unidades de medida (`unidades_listar`, `unidade_salvar`) |
| [`app/qml/TelaPecas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPecas.qml#L1-L221) | 221 | Tela de listagem e busca unificada de peças (`produtos_buscar`, atalhos `F2`/`F11`, `ImportarCadastros`) |
| [`app/qml/TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L1-L1371) | 1.371 | Ficha única de cadastro/edição de peça ou serviço, variações (grade), aplicações, equivalências, preços, estoque, fiscal (ICMS, PIS/Cofins, IPI, Combustível, ISS) e histórico |
| [`app/qml/EscolherPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/EscolherPeca.qml#L1-L145) | 145 | Popup modal de busca e seleção de peça (usado na conferência de compra e em etiquetas) |
| [`app/qml/LinhaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaPeca.qml#L1-L202) | 202 | Delegate de linha de peça (layout responsivo desktop vs. `Tema.estreito`, balão `achou`, alerta de estoque baixo e indicador de oferta) |
| [`app/qml/LinhaAplicacao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaAplicacao.qml#L1-L89) | 89 | Linha editável de aplicação veicular (montadora, modelo, motor, ano início, ano fim) |
| [`app/qml/LinhaEquivalente.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaEquivalente.qml#L1-L55) | 55 | Linha editável de código equivalente (marca e código) |
| [`app/qml/LinhaPreco.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaPreco.qml#L1-L81) | 81 | Linha de tabela de preço com recálculo bidirecional (`% sobre o custo` ↔ `Preço`) via `preco_calcular` |
| [`app/qml/LinhaVeiculo.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaVeiculo.qml#L1-L54) | 54 | Linha editável de veículo do cliente (placa e descrição) |
| [`app/qml/TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L1-L503) | 503 | Tela de preparação, pré-visualização A4/etiqueta e impressão de folhas de etiquetas adesivas |

---

## 2. Separação Obrigatória: Código Lido vs. Testes Encontrados vs. Testes Executados

### 2.1. Código Lido
- Leitura estática integral das 3.045 linhas listadas na Seção 1.

### 2.2. Testes Encontrados no Código
- `nucleo/src/estoque.rs` não possui bloco `#[cfg(test)]` embutido; seus testes residem em `nucleo/src/api.rs` (`ajuste_de_estoque_fica_no_historico` nas linhas `3358–3377`) e nos arquivos de teste dedicados do Bloco 9A1 (`nucleo/examples/estoque_teste.rs` e `nucleo/tests/estavel/t2_estoque.rs`).
- Os 10 arquivos QML deste bloco não possuem testes unitários QML dedicados.

### 2.3. Testes Executados
- **Nenhum teste foi executado neste bloco.** Todas as conclusões decorrem de verificação estática direta no código-fonte e cruzamento com os contratos do núcleo auditados no Bloco 8B1.

---

## 3. Revisão Independente dos Achados Históricos (Gemini 4 Argon)

### 3.1. `B8B1-05` (Reflexo na UI) — `EscolherUnidade.qml` exibe lista plana única em `Flow`, sem as 2 fileiras de 5 unidades do contrato documental
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado** (divergência entre contrato documental normativo de `docs/unidades-de-medida.md` e implementação QML + Rust)
- **Severidade:** Baixa / Média
- **Evidência no Código:**
  - Em [`EscolherUnidade.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/EscolherUnidade.qml#L27-L31) e [`EscolherUnidade.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/EscolherUnidade.qml#L59-L92):
    ```qml
    Flow {
        id: fluxo
        width: parent.width
        spacing: 6
        Repeater {
            model: escolher.lista
            delegate: Rectangle { ... }
        }
    }
    ```
  - Conforme verificado no Bloco 8B1 ([`unidades::listar`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/unidades.rs#L20-L39)), o backend devolve todas as unidades cadastradas em um único vetor plano ordenado por `COUNT(p.id) DESC, u.sigla`, e `EscolherUnidade.qml` renderiza todas elas num único `Flow`, sem limitar às 10 unidades em 2 fileiras de 5 (1ª fileira por peças ativas em estoque; 2ª fileira por vendas líquidas nas últimas 24h).

---

### 3.2. `B8B1-06` — Hipótese de corrida/ordem de carregamento de `sugestoes` em `TelaPeca.qml`
- **Status de Revisão:** **REFUTADO** (confirmando o fechamento da hipótese registrado pelo próprio Gemini 4 Argon no Bloco 8B2)
- **Classificação Técnica:** **Hipótese** (refutada na leitura de `TelaPeca.qml`)
- **Severidade:** N/A
- **Evidência no Código:**
  - Em [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L36-L51) (`carregar()`) e [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L233-L241) (`salvar()`):
    ```qml
    const s = Api.chamar("produto_sugestoes")
    if (s.ok)
        sugestoes = s.dados
    const r = produtoId ? Api.chamar("produto_abrir", { id: produtoId }) : Api.chamar("produto_novo")
    ...
    preencher(r.dados)
    ```
  - Como `Api.chamar` é síncrono via ponte C++/Rust e `sugestoes` é atribuído **antes** de `preencher(r.dados)` tanto na abertura quanto após salvar, `sugestoes.csosns`, `sugestoes.ufs`, `sugestoes.cstPis`, `sugestoes.cstIpi` e `sugestoes.exigibilidades` já estão populados quando `preencher()` e `preencherFiscal()` resolvem os índices dos componentes `Selecao`.

---

### 3.3. `B8B1-07` — `LinhaAplicacao.qml` não exibe nem permite editar o campo `observacao` da aplicação veicular
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado** (lacuna funcional na UI para campo existente no banco, na busca, na validação e no modelo QML)
- **Severidade:** Baixa / Média
- **Evidência no Código:**
  - Em [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L89-L91) e [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L105-L109), o `ListModel { id: aplicacoes }` armazena e reenvia `observacao`:
    ```qml
    aplicacoes.append({ montadora: a.montadora, modelo: a.modelo, motor: a.motor, anoInicio: a.anoInicio, anoFim: a.anoFim, observacao: a.observacao })
    ```
  - Contudo, em [`LinhaAplicacao.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/LinhaAplicacao.qml#L9-L26):
    ```qml
    required property int index
    required property string montadora
    required property string modelo
    required property string motor
    required property string anoInicio
    required property string anoFim
    ...
    function campo(nome) {
        const campos = { montadora: campoMontadora, modelo: campoModelo, motor: campoMotor, anoInicio: campoAnoInicio, anoFim: campoAnoFim, observacao: campoModelo }
        return campos[nome] || null
    }
    ```
  - Não existe `Campo` para `observacao` em `LinhaAplicacao.qml` (nem cabeçalho correspondente em [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L667-L677)), tornando impossível cadastrar ou editar a observação da aplicação pela interface gráfica.

---

### 3.4. `B8B1-09` — `TelaEtiquetas.qml` permite `quantidade = 0`, reconstrói delegates do `ListView` a cada tecla digitada na quantidade e `F2` ignora `tela.total > 0`
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média
- **Evidência no Código:**
  1. Em [`TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L78-L82):
     ```qml
     function mudarQuantidade(indice, texto) {
         const lista = itens.slice()
         lista[indice] = { etiqueta: lista[indice].etiqueta, quantidade: Math.max(0, Math.min(999, parseInt(texto) || 0)) }
         itens = lista
     }
     ```
     - Ao permitir `0`, o item permanece na lista com `quantidade: 0` (inclusive servindo de base para a prévia da primeira etiqueta em `previa.e` na linha `383` mesmo que `itens[0].quantidade === 0` e seja ignorado pelo núcleo em `etiquetas::montar`).
  2. Além disso, em [`TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L200) e [`TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L253-L261):
     ```qml
     model: tela.itens
     ...
     Campo {
         numerico: true
         maximo: 3
         sufixo: "etiq."
         texto: String(linha.modelData.quantidade)
         ...
         onEditado: tela.mudarQuantidade(linha.index, texto)
     }
     ```
     - Como `model: tela.itens` é um array JavaScript (`property var itens: []`) e `onEditado` substitui `itens = lista` a cada dígito editado, o `ListView` recria seus delegates durante a digitação, derrubando o foco/cursor do `Campo` de quantidade a cada tecla pressionada (e convertendo string vazia ao apagar com Backspace imediatamente em `"0"`).
  3. Enquanto o botão `"Imprimir"` ([`TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L156-L162)) verifica `enabled: tela.total > 0`, o atalho `F2` ([`TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L132-L136)) chama `tela.imprimir()` mesmo com `tela.total === 0`.

---

### 3.5. `B8B1-10` — `EscolherPeca.qml` omite `camera: true` e `atalho: "F11"` no campo de busca
- **Status de Revisão:** **CONFIRMADO**
- **Classificação Técnica:** **Melhoria** (inconsistência em relação ao contrato padrão do campo de busca de peças)
- **Severidade:** Baixa
- **Evidência no Código:**
  - Em [`EscolherPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/EscolherPeca.qml#L74-L81):
    ```qml
    Campo {
        id: busca
        icone: "busca"
        altura: 50
        tamanhoFonte: 17
        dica: "Qualquer código, nome, modelo, marca ou carro"
        Layout.fillWidth: true
    ```
  - Diferentemente de [`TelaPecas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPecas.qml#L119-L125) e [`TelaEtiquetas.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaEtiquetas.qml#L184-L189), `EscolherPeca.qml` não define `camera: true` (impedindo o uso da câmera para leitura de código de barras no celular ao vincular item de compra ou escolher peça para etiqueta) nem `atalho: "F11"`.

---

## 4. Novos Achados Identificados no Bloco 8B2

### 4.1. `NEW-B8B2-01` — `estoque::ajustar` permite lançar quantidade fracionada (com vírgula) em peças cuja unidade de medida não é fracionada (`fracionada = 0`)
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Média / Alta (quebra de invariante de domínio de unidades inteiras no estoque)
- **Evidência no Código:**
  - Em [`nucleo/src/produtos.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/produtos.rs#L265-L267), [`produtos.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/produtos.rs#L312-L319) e [`produtos.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/produtos.rs#L498-L511), o cadastro da peça consulta `SELECT fracionada FROM unidade WHERE sigla = ?1` e rejeita `estoqueInicial` ou `estoqueMinimo` com milésimos (`v % 1000 != 0`) quando `!fracionada` (`"Esta unidade não aceita números quebrados (com vírgula)."`).
  - Porém, em [`nucleo/src/estoque.rs`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/estoque.rs#L44-L77) (`ajustar`):
    ```rust
    let contado = numeros::ler_quantidade(contado)
        .map_err(|m| Erro::campo("quantidade", m))?
        .ok_or_else(|| Erro::campo("quantidade", "Digite a quantidade contada."))?;
    if contado < 0 {
        return Err(Erro::campo("quantidade", "A quantidade não pode ser negativa."));
    }
    ...
    let servico: bool = conn
        .query_row("SELECT tipo = 'servico' FROM produto WHERE id = ?1", [produto_id], |l| l.get(0))
        .optional()?
        .ok_or_else(|| Erro::NaoEncontrado("Peça".into()))?;
    if servico {
        return Err(Erro::validacao("Serviço não tem estoque."));
    }
    let atual = saldo(conn, produto_id)?;
    let diferenca = contado - atual;
    ```
  - `estoque::ajustar` **não verifica `u.fracionada`** da unidade do produto! Assim, uma peça cadastrada em `UN`, `PC`, `PAR` ou `JG` (`fracionada = 0`) aceita um ajuste de estoque para `"2,5"`, gravando `2500` milésimos em `estoque_movimento` e deixando o saldo da peça fracionado mesmo em unidade inteira.

---

### 4.2. `NEW-B8B2-02` — Alternar `tipo` (`"produto"` ↔ `"servico"`) em `TelaPeca.qml` mantém valores nos campos fiscais ocultos da outra modalidade, causando validação cruzada e exibição de aba fiscal inexistente nas `Pilulas`
- **Classificação Técnica:** **Defeito comprovado**
- **Severidade:** Baixa / Média
- **Evidência no Código:**
  - Em [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L487-L498), trocar a pílula entre `"produto"` e `"servico"` altera `tela.tipo` e `tela.blocoFiscal = id === "servico" ? "servico" : "icms"`, mas [`coletar()`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L115-L150) e [`coletarFiscal()`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L186-L200) enviam todos os campos fiscais (`ncm`, `cest`, `anpCodigo`, `ipi`, `combustivel`, `servico`) independentemente de `tela.tipo`.
  - No núcleo, [`produtos::validar`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/produtos.rs#L321-L343) e [`produtos::validar_fiscal`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/nucleo/src/produtos.rs#L514-L618) validam todos os blocos fiscais mesmo que `tipo == "servico"` (para ICMS/IPI/Combustível) ou `tipo == "produto"` (para ISS).
  - Se o usuário preencher um campo com erro em `"produto"` (ex.: `ncm` incompleto) e mudar para `"servico"` antes de salvar, [`mostrarErro`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L275-L285) força `blocoFiscal = "icms"`, abrindo o bloco ICMS dentro de um Serviço cujas pílulas ([`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L999-L1007)) só possuem `ISS` e `PIS/Cofins` (ou vice-versa para `blocoFiscal = "servico"` num Produto).

---

### 4.3. `NEW-B8B2-03` — Botão `"Etiqueta"` em `TelaPeca.qml` aparece mesmo para Serviços (`tipo === "servico"`) e para peças com código bloqueado
- **Classificação Técnica:** **Melhoria** / **Risco** de UX
- **Severidade:** Baixa
- **Evidência no Código:**
  - Em [`TelaPeca.qml`](file:///home/renan/%C3%81rea%20de%20trabalho/Rustinh0/app/qml/TelaPeca.qml#L439-L445):
    ```qml
    Botao {
        visible: !tela.nova && !tela.alterado
        text: "Etiqueta"
        icone: "impressora"
        tipo: "secundario"
        onClicked: tela.janela.abrir("TelaEtiquetas", { produtoId: tela.ficha.id })
    }
    ```
  - Diferentemente do botão de variações na linha `631` (`visible: !tela.nova && !tela.servico`) e das faixas de Aplicações/Equivalências/Estoque (`visible: !tela.servico`), o botão `"Etiqueta"` não filtra `!tela.servico`. Além disso, se a peça estiver sem código definido ou com código duplicado (`ficha.codigoAviso`), clicar em `"Etiqueta"` navega para `TelaEtiquetas` vazia apenas para exibir o toast de bloqueio lá.

---

## 5. Tabela Consolidada de Revisão do Bloco 8B2

| ID | Origem | Alvo | Status na Auditoria | Classificação Técnica |
|---|---|---|---|---|
| **B8B1-05 (UI)** | Gemini 4 Argon (8B1/8B2) | `EscolherUnidade.qml:59–92` | **CONFIRMADO** | Defeito comprovado |
| **B8B1-06** | Gemini 4 Argon (8B1/8B2) | `TelaPeca.qml:36–51, 233–241` | **REFUTADO** | Hipótese (refutada) |
| **B8B1-07** | Gemini 4 Argon (8B1/8B2) | `LinhaAplicacao.qml:9–26` / `TelaPeca.qml:89–109` | **CONFIRMADO** | Defeito comprovado |
| **B8B1-09** | Gemini 4 Argon (8B1/8B2) | `TelaEtiquetas.qml:78–82, 132–136, 253–261` | **CONFIRMADO** | Defeito comprovado |
| **B8B1-10** | Gemini 4 Argon (8B1/8B2) | `EscolherPeca.qml:74–81` | **CONFIRMADO** | Melhoria |
| **NEW-B8B2-01** | Auditoria Independente | `nucleo/src/estoque.rs:44–77` | **NOVO ACHADO** | Defeito comprovado |
| **NEW-B8B2-02** | Auditoria Independente | `TelaPeca.qml:115–200, 275–285, 487–498` | **NOVO ACHADO** | Defeito comprovado |
| **NEW-B8B2-03** | Auditoria Independente | `TelaPeca.qml:439–445` | **NOVO ACHADO** | Melhoria |

