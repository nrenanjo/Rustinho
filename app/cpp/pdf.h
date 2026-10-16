// Impressão: transforma o HTML montado pelo núcleo em PDF A4 e abre o
// arquivo no visualizador do sistema (de onde se imprime com Ctrl+P).
#pragma once

#include <QtCore/QString>

bool rustinho_gerar_pdf(const QString& html, const QString& caminho);
bool rustinho_abrir_arquivo(const QString& caminho);
// Etiquetas de preço: desenha as folhas A4 descritas no JSON (montado pelo núcleo).
bool rustinho_gerar_etiquetas(const QString& json, const QString& caminho);
// Impressão direta na impressora padrão, sem abrir o PDF (só no Windows;
// nos outros devolve false e o Linux usa o "lp").
bool rustinho_imprimir_html(const QString& html);
bool rustinho_imprimir_etiquetas(const QString& json);
