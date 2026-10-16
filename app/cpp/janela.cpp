#include "rustinho/cpp/janela.h"

#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QStandardPaths>
#include <QtCore/QUrl>

void rustinho_preparar_janela()
{
    QGuiApplication::setWindowIcon(QIcon(QStringLiteral(":/qt/qml/rustinho/icone/rustinho.png")));
    QGuiApplication::setDesktopFileName(QStringLiteral("rustinho"));
}

QString rustinho_arquivo_local(const QString& endereco)
{
    if (!endereco.startsWith(QStringLiteral("content:")))
        return endereco;
    const QString pasta = QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + QStringLiteral("/importar");
    QDir().mkpath(pasta);
    QString nome = QUrl(endereco).fileName();
    if (nome.isEmpty())
        nome = QStringLiteral("arquivo");
    // O nome que o Android mostra às vezes vem com o caminho junto.
    nome = nome.section(QLatin1Char('/'), -1).section(QLatin1Char(':'), -1);
    const QString destino = pasta + QLatin1Char('/') + nome;
    QFile::remove(destino);
    QFile origem(endereco);
    if (!origem.copy(destino))
        return endereco;
    return destino;
}
