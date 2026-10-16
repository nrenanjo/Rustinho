// Rustinho no Android: cria o app Qt, aponta a pasta de dados do aplicativo
// e abre a mesma tela principal do computador.
#include <QtCore/QDir>
#include <QtCore/QStandardPaths>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>

#include <QtCore/QJniObject>
#include <QtCore/QStringList>
#include <QtCore/qcoreapplication_platform.h>
#include <QtQml/QQmlContext>

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

    // Só para o teste automático (android/testar.sh): o APK final não aceita
    // argumentos, então eles vêm num "extra" do Android:
    //   am start -n .../QtActivity --es rustinho "--abrir TelaVender"
    QStringList extras;
    QJniObject atividade = QNativeInterface::QAndroidApplication::context();
    if (atividade.isValid()) {
        QJniObject intencao = atividade.callObjectMethod("getIntent", "()Landroid/content/Intent;");
        if (intencao.isValid()) {
            QJniObject texto = intencao.callObjectMethod("getStringExtra", "(Ljava/lang/String;)Ljava/lang/String;",
                                                         QJniObject::fromString(QStringLiteral("rustinho")).object<jstring>());
            if (texto.isValid())
                extras = texto.toString().split(QLatin1Char(' '), Qt::SkipEmptyParts);
        }
    }
    if (!extras.isEmpty())
        qInfo("Rustinho: argumentos de teste: %s", qPrintable(extras.join(QLatin1Char(' '))));
    engine.rootContext()->setContextProperty(QStringLiteral("argumentosExtras"), extras);
    engine.addImportPath(QStringLiteral("qrc:/qt/qml"));
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/rustinho/qml/Main.qml")));
    if (engine.rootObjects().isEmpty())
        return 1;
    return app.exec();
}
