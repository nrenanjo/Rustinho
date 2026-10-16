#include "rustinho/cpp/janela.h"

#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>

void rustinho_preparar_janela()
{
    QGuiApplication::setWindowIcon(QIcon(QStringLiteral(":/qt/qml/rustinho/icone/rustinho.png")));
    QGuiApplication::setDesktopFileName(QStringLiteral("rustinho"));
}
