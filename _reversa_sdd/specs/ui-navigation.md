# Especificação de Interface e Navegação Contextual Guiada — Rustinho 0.9.17

> Gerado pelo Reversa em 2026-11-02
> Extraído de `app/qml/Main.qml`, `Guia.qml`, `Notificacao.qml`, `TelaInicio.qml`, `EscolherUnidade.qml` e das 100 telas/componentes QML.

---

## 1. Arquitetura de Navegação (`app/qml/Main.qml`)

- **Pilha Customizada (`pilha: []` + `Loader`):** `Main.qml` mantém um array JS `pilha` com entradas `{ tela, props }` e carrega a tela ativa em um `Loader` central (`carregador`).
- **Primitivas de Navegação:**
  - `abrir(tela, props)`: empilha a tela atual e abre a nova;
  - `substituir(tela, props)`: substitui o topo sem empilhar;
  - `voltar()`: desempilha o topo e restaura `{ tela, props }` anterior;
  - `irInicio()`: limpa a pilha e abre `TelaInicio`;
  - `irParaRequisito(requisito, origem)`: aciona a navegação contextual guiada (`guiaAtiva`), empilhando a tela de origem com seu estado preservado (`guiaRetorno`) e abrindo a tela de destino (`TelaConfiguracoes`) com destaque visual e foco no primeiro campo pendente;
  - `concluirGuia()`: retorna à tela de origem com `guiaConcluida: true`, exibindo a faixa verde `"Requisito satisfeito — conclua a operação."`;
  - `abandonarGuia()`: retorna à tela de origem com `guiaAbandonada: true`, mantendo os arquivos/dados em espera sem marcar o requisito como concluído.

---

## 2. Matriz de Requisitos Guiados (`nucleo/src/requisitos.rs`)

| Operação de Origem | Tela de Origem | Condição de Disparo | Destino Guiado | Campos Exigidos | Retorno à Origem |
|---|---|---|---|---|---|
| Peças › Importar XML (`importar_ler_cadastros`) | `TelaPecas` (`ImportarCadastros`) | `loja_cnpj` vazio ou inválido | `TelaConfiguracoes` (seção `loja`) | `campoCnpjLoja` | Reabre `ImportarCadastros`, confere `arquivos_conferir`, relê XMLs se presentes, sem cadastrar sozinho. |
| Compras › Importar XML (`importar_xml`) | `TelaCompras` | `loja_cnpj` vazio ou inválido | `TelaConfiguracoes` (seção `loja`) | `campoCnpjLoja` | Mantém XML em `"Esperando para importar"` com botões `"Importar agora"` e `"Desistir"`, sem importar sozinho. |
| Configurações › Simultaneidade › Fiscal (`fiscal_verificar_requisitos`) | `TelaConfiguracoes` / `TelaFiscal` | `loja_uf` vazia ou `loja_cnpj` vazio/inválido | `TelaConfiguracoes` (seção `loja`) | `campoUfLoja` e/ou `campoCnpjLoja` | Só retorna ao Fiscal quando **ambos** (`UF` e `CNPJ`) estão gravados e válidos. |

---

## 3. Especificação de Notificações do Início (`Notificacao.qml` + `TelaInicio.qml`)

| Gravidade | Estado Inicial | Interação por Clique | Gesto Arrastar Direita (`>= +72px`) | Gesto Arrastar Esquerda (`<= -72px`) | Persistência (`config`) |
|---|---|---|---|---|---|
| **Informativa** (`receber`, `pagar`, `encomendas`, `orcamentos`) | Quadrado (`92x92` desktop) | Alterna Quadrado ↔ Faixa (fecha qualquer outra faixa não-crítica aberta) | Na faixa: volta para Quadrado | Esconde até meia-noite local (`aviso_escondido.<id> = YYYY-MM-DD`) | `aviso_minimizado.<id>` e `aviso_escondido.<id>` (locais ao computador, fora do `sync`) |
| **Atenção** (`conflitos`, `estoque_negativo`, `caixa_antigo`) | Quadrado (`92x92` desktop) com triângulo de alerta | Alterna Quadrado ↔ Faixa (fecha qualquer outra faixa não-crítica aberta) | Na faixa: volta para Quadrado | Ignorado (não esconde) | `aviso_minimizado.<id>` |
| **Crítica** (`outra_loja`, `codigos_outro_computador`, `sync_parou`, `backup_atrasado`, `modo_seguro`) | Faixa expandida (uma por ocorrência) | Ignorado (apenas o botão interno de ação responde) | Ignorado | Ignorado | Sempre aberta enquanto a condição existir no banco |

