// Coisas da janela que o QML não faz sozinho.
#pragma once

// Ícone do Rustinho na barra de tarefas e no alt+tab.
void rustinho_preparar_janela();

#include <QtCore/QString>

// Arquivo escolhido na janela de arquivos → caminho comum. No Android, os
// arquivos vêm como "content://..." e são copiados para a pasta do app.
QString rustinho_arquivo_local(const QString& endereco);

// Identificador do computador (veja janela.cpp). Vazio se o sistema não informa.
QString rustinho_identidade_do_computador();
