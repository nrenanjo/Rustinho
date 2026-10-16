// Rustinho no Android: cria o app Qt, aponta a pasta de dados do aplicativo
// e abre a mesma tela principal do computador.
#include <QtCore/QDir>
#include <QtCore/QStandardPaths>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>

int main(int argc, char* argv[])
{
    QGuiApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("rustinho"));
    app.setApplicationDisplayName(QStringLiteral("Rustinho"));
    app.setOrganizationName(QStringLiteral("rustinho"));

    // Banco, backups e impressões ficam na pasta privada do app.
    const QString dados = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    QDir().mkpath(dados);
    qputenv("RUSTINHO_DADOS", dados.toUtf8());

    QQmlApplicationEngine engine;
    engine.addImportPath(QStringLiteral("qrc:/qt/qml"));
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/rustinho/qml/Main.qml")));
    if (engine.rootObjects().isEmpty())
        return 1;
    return app.exec();
}
