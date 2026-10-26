# Integrações externas

Este documento descreve como o Rustinho se relaciona com programas de terceiros, em especial a integração fiscal prevista com o ACBrMonitorPLUS. Ele separa três categorias:

- **Confirmado oficialmente:** informações verificadas em fonte oficial, com a data da verificação.
- **Decisão arquitetural:** escolhas de projeto do Rustinho, sob nosso controle.
- **Pendente:** pontos que dependem de confirmação e não devem ser tratados como certos.

Status: documento de planejamento. Nenhuma integração fiscal está implementada.

---

## 1. Independência do projeto

O Rustinho é um projeto independente, de código aberto, sem fins lucrativos próprios. Ele é distribuído sozinho no GitHub, com seus próprios executáveis (Windows), AppImage (Linux), código-fonte e documentação.

**Decisão arquitetural — licença:** o Rustinho é licenciado sob a MIT. A intenção é dar liberdade ao desenvolvedor: qualquer pessoa pode usar, modificar, redistribuir e incorporar o código a projetos próprios, inclusive comerciais e de código fechado, desde que preserve os avisos de copyright e de licença exigidos pela MIT. Versões modificadas não são obrigadas a permanecer abertas.

**Decisão arquitetural — simplicidade:** simples é melhor do que complexo. Não adicionamos mecanismos de instalação automática, download de binários de terceiros ou dependências sem necessidade concreta.

## 2. Integração fiscal externa

O Rustinho **não incorpora nem distribui** o binário do ACBrMonitorPLUS em seus pacotes oficiais. Quando a funcionalidade fiscal for implementada, o usuário instalará o ACBrMonitorPLUS separadamente.

**Decisão arquitetural — papel do ACBrMonitorPLUS:** adaptador fiscal externo. Ele:
- não coordena os caixas;
- não armazena a fonte oficial das operações comerciais;
- não participa do protocolo de replicação entre caixas e Google Drive.

Se o ACBrMonitorPLUS estiver indisponível:
- o núcleo Rust, o banco local e a sincronização entre os caixas e o Google Drive continuam funcionando;
- a conclusão de operações comerciais e a emissão fiscal precisam respeitar as obrigações legais aplicáveis;
- não há garantia de que toda venda poderá ser concluída sem o ACBrMonitorPLUS. Se a legislação permitir emitir o documento depois, por contingência, o Rustinho seguirá as regras desse procedimento. Se não permitir, o Rustinho informará o problema e orientará o operador, sem realizar operação fiscal irregular.

**Decisão arquitetural — isolamento:** a integração fiscal fica atrás de uma interface própria no núcleo Rust. Assim, trocar de fornecedor (por exemplo, para a ACBrLibNFe) não exige reescrever vendas, estoque, financeiro ou interface.

## 3. Fatos confirmados oficialmente

Fonte: página oficial de downloads do ACBr (https://projetoacbr.com.br/pro/downloads/), consultada em 25/10/2026.

- O ACBrMonitorPLUS é listado com a indicação "Windows e Linux" (versões para os dois sistemas).
- Existe versão demonstrativa gratuita do ACBrMonitorPLUS.
- O ACBrMonitorPLUS está incluído no Pacote Pro. O acesso às versões profissionais para download depende de assinatura do ACBr Pro.
- A página não mostra nome de licença nem número de versão para o ACBrMonitorPLUS.

Outras fontes consultadas, citadas por terceiros e não verificadas integralmente por nós:
- Página de downloads geral: https://projetoacbr.com.br/downloads/
- Fórum oficial, categorias do ACBrMonitorPLUS: https://www.projetoacbr.com.br/forum/files/category/16-acbrmonitorplus/
- Discussão sobre ACBrMonitorPLUS no Linux: https://www.projetoacbr.com.br/forum/topic/33952-acbrmonitorplus-linux-no-docker/

## 4. Pendente — não tratar como certo

1. **Licença do ACBrMonitorPLUS.** Há informações conflitantes em fontes diferentes: uma indica GPL para o MonitorPLUS, outra indica LGPL para componentes do ACBr. A página oficial de downloads não nomeia a licença. Até haver confirmação escrita ou o arquivo de licença do pacote exato, nenhuma conclusão sobre a licença aplicável está estabelecida.
2. **Redistribuição de binários.** Não está confirmado se o ACBrMonitorPLUS pode ser redistribuído no GitHub por terceiros, nem em quais condições. Por isso o Rustinho não redistribui esse binário.
3. **Versão e compatibilidade no Linux.** A disponibilidade para Linux x86_64 foi indicada pela página, mas não foi testada no Rustinho. Dependências e compatibilidade com distribuições ainda não foram verificadas.
4. **Condições do ACBr Pro.** O preço, as condições de renovação e o que acontece com o acesso aos downloads após o fim da assinatura precisam de confirmação antes de qualquer afirmação no README.
5. **Questões legais gerais.** Quaisquer questões legais sobre a integração externa serão registradas aqui, sem conclusões jurídicas, até avaliação adequada.

## 5. Instalação e requisitos (planejados)

Ainda não implementados. Planejamento:

- **Windows:** o usuário baixa o ACBrMonitorPLUS no site oficial e o instala separadamente. Não há instalação automática.
- **Linux:** o usuário instala separadamente, seguindo as instruções oficiais. A integração no AppImage fica para depois de validada.
- O Rustinho detecta se o programa está instalado e em execução. Quando não estiver, informa o usuário sem encerrar as demais funções.

## 6. Comunicação e segurança

**Estado atual:** nenhuma integração fiscal e nenhuma proteção descrita abaixo está implementada. O conteúdo desta seção é requisito planejado, a ser verificado na implementação e nos testes.

**Requisitos planejados:**
- Comunicação com o ACBrMonitorPLUS por arquivos TXT ou TCP/IP.
- Quando a integração for local, comunicação TCP restrita a `localhost`, sem expor a interface de comandos à rede da loja.
- Certificados, senhas e outros segredos não serão guardados em texto aberto pelo Rustinho.

**Garantias ainda não comprovadas:** a ausência de armazenamento de certificados, senhas ou outros segredos em texto aberto é um requisito a verificar, não uma garantia já comprovada. Só poderá ser declarada após inspeção do código e testes específicos.

## 7. Limitações conhecidas

- O ACBrMonitorPLUS não gerencia sozinho toda a contingência fiscal. O Rustinho deve fornecer dados tributários corretos e tratar erros e retomadas.
- Atualizações do ACBr não significam que o Rustinho esteja atualizado automaticamente. Cada versão precisa de integração e testes de regressão antes de ser indicada.
- Enquanto a integração não for implementada e testada, o Rustinho não emite documentos fiscais por meio do ACBrMonitorPLUS.
- A palavra "gratuito" aplica-se ao Rustinho. O acesso à versão profissional atualizada do ACBrMonitorPLUS depende das condições comerciais do ACBr.

## 8. Responsabilidade e revisão

Este documento não substitui orientação jurídica. Antes de qualquer distribuição de binários de terceiros, as questões da seção 4 devem estar resolvidas e documentadas.
