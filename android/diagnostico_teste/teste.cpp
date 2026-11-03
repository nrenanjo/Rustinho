// Teste no computador do diagnóstico do Android (PEND-18). Roda com
//   QT_QPA_PLATFORM=offscreen; usa a classe real (../diagnostico.cpp).
// Cenários: "cinza" (janela de uma cor só) e "conteudo" (janela com texto).
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QTimer>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickWindow>
#include <cstdio>

#include "../diagnostico.h"

int main(int argc, char* argv[])
{
    const QString cenario = argc > 1 ? QString::fromLocal8Bit(argv[1]) : QStringLiteral("cinza");
    Diagnostico d;
    d.iniciar();
    QGuiApplication app(argc, argv);
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("diagnostico"), &d);
    engine.load(QUrl::fromLocalFile(QDir::current().filePath(cenario + QStringLiteral(".qml"))));
    if (engine.rootObjects().isEmpty()) {
        d.marcar(QStringLiteral("Main.qml NÃO carregou"));
        return 3;
    }
    d.vigiar(qobject_cast<QQuickWindow*>(engine.rootObjects().first()), 1);
    QTimer::singleShot(2500, &app, &QCoreApplication::quit);
    return app.exec();
}
