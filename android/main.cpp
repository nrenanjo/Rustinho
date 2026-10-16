// Rustinho no Android: cria o app Qt, aponta a pasta de dados do aplicativo
// e abre a mesma tela principal do computador.
#include <QtCore/QDir>
#include <QtCore/QStandardPaths>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>

#include <cstdio>
#include <fstream>
#include <string>
#include <vector>

int main(int argc, char* argv[])
{
    // Só para o teste automático (android/testar.sh): o adb deixa os
    // argumentos (ex.: "--abrir TelaVender") num arquivo da pasta do app.
    std::vector<std::string> extras;
    for (const char* arquivo : { "/storage/emulated/0/Android/data/io.github.nrenanjo.rustinho/files/argumentos.txt",
                                 "/data/local/tmp/rustinho-argumentos.txt" }) {
        std::ifstream f(arquivo);
        std::string palavra;
        while (f >> palavra)
            extras.push_back(palavra);
        if (!extras.empty()) {
            std::remove(arquivo);
            break;
        }
    }
    std::vector<char*> argumentos(argv, argv + argc);
    for (auto& e : extras)
        argumentos.push_back(e.data());
    int quantos = static_cast<int>(argumentos.size());
    argumentos.push_back(nullptr);

    QGuiApplication app(quantos, argumentos.data());
    app.setApplicationName(QStringLiteral("rustinho"));
    app.setApplicationDisplayName(QStringLiteral("Rustinho"));
    app.setOrganizationName(QStringLiteral("rustinho"));
    if (!extras.empty())
        qInfo("Rustinho: argumentos de teste: %s", qPrintable(QCoreApplication::arguments().join(u' ')));

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
