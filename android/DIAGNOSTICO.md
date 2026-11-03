# Diagnóstico do Rustinho no Android (PEND-18)

Para quando o app abre e a tela fica toda cinza (ou fecha sozinho).

**Onde ficam os arquivos** (no celular, pelo app Arquivos): `Android/data/<nome do pacote>/files/`
- Pacote do APK de teste: `io.github.nrenanjo.rustinho.teste` (ou o do APK que você instalou).
- `diagnostico.txt`: o que o app e o Qt avisaram, os marcos da abertura e a conclusão ("uniforme (uma cor só): SIM" = a tela cinza).
- `diagnostico-anterior.txt`: o da abertura anterior.
- `captura-HHMMSS.png`: foto da janela aos 8 e aos 20 segundos (mostra o que está na tela de verdade).
- Se a pasta `Android/data` não abrir, o app guarda na pasta interna e o aviso na tela diz isso.

**Aviso na tela:** se a tela principal não carregar, ou se aos 8 segundos a foto for de uma cor só, ou se a tela inicial não ficou pronta, aparece um aviso do próprio Android ("Rustinho: a tela não abriu direito…").

**Trocar o modo de desenho sem novo APK:** crie na mesma pasta um arquivo `desenho.txt` com só uma palavra:
- `software`: desenha pelo processador (padrão em celulares de 32 bits);
- `gpu`: desenha pela placa de vídeo.
Feche o app por completo e abra de novo. O `diagnostico.txt` diz qual modo foi usado e por quê.

**Privacidade:** o arquivo só tem mensagens do Qt e marcos; não tem senha, cliente, venda nem o conteúdo do banco. As fotos mostram a tela como estava (no primeiro uso, a tela de boas-vindas).

**Teste no computador:** `bash android/diagnostico_teste/rodar.sh` (precisa do Qt 6 de desenvolvimento). Cobre tela cinza, tela com conteúdo e QML quebrado. O que só existe no Android (pasta Android/data, aviso do Android) **não** é testado aí; só no celular.
