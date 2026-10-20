// Rustinho no Android: cria o app Qt, aponta a pasta de dados do aplicativo
// e abre a mesma tela principal do computador.
#include <QtCore/QDir>
#include <QtCore/QStandardPaths>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>

#include <QtCore/QJniObject>
#include <QtCore/QRegularExpression>
#include <QtCore/QStringList>
#include <QtCore/qcoreapplication_platform.h>
#include <QtQml/QQmlContext>
#include <QtQml/qqml.h>

#include <sys/system_properties.h>

#include <QtCore/QTimer>
#include <QtQuick/QQuickWindow>

#include "diagnostico.h"
#include "vistacamera.h"

// Celulares de 32 bits e chips de vídeo PowerVR (como o do Moto e6i) desenham
// as telas do Qt com cores trocadas e sem letras. Neles, o Rustinho desenha
// pela CPU ("software"), que é mais lento mas sempre fica certo.
static bool desenharPorSoftware()
{
#if defined(__arm__) && !defined(__aarch64__)
    return true;
#else
    char valor[PROP_VALUE_MAX] = {};
    for (const char* nome : {"ro.hardware.egl", "ro.hardware.vulkan", "ro.board.platform"}) {
        if (__system_property_get(nome, valor) > 0) {
            const QByteArray texto = QByteArray(valor).toLower();
            if (texto.contains("powervr") || texto.contains("pvr"))
                return true;
        }
    }
    return false;
#endif
}

int main(int argc, char* argv[])
{
    // PEND-18: registro de diagnóstico desde o primeiro instante.
    Diagnostico diagnostico;
    diagnostico.iniciar();

    // Modo de desenho. Quem manda, na ordem: RUSTINHO_DESENHO (só testes),
    // o arquivo "desenho.txt" na pasta do app (gpu ou software; serve para
    // testar no celular sem APK novo) e, por fim, a escolha automática.
    QByteArray forcado = qgetenv("RUSTINHO_DESENHO");
    QString motivo = QStringLiteral("variável RUSTINHO_DESENHO");
    if (forcado.isEmpty()) {
        const QString pedido = Diagnostico::desenhoPedido();
        if (!pedido.isEmpty()) {
            forcado = pedido.toUtf8();
            motivo = QStringLiteral("arquivo desenho.txt");
        } else {
            motivo = QStringLiteral("escolha automática");
        }
    }
    const bool software = forcado == "software" || (forcado != "gpu" && desenharPorSoftware());
    if (software)
        qputenv("QT_QUICK_BACKEND", "software");
    diagnostico.marcar(QStringLiteral("modo de desenho: %1 (%2)").arg(software ? QStringLiteral("software (processador)") : QStringLiteral("placa de vídeo"), motivo));

    QGuiApplication app(argc, argv);
    diagnostico.marcar(QStringLiteral("QGuiApplication criado"));
    app.setApplicationName(QStringLiteral("rustinho"));
    app.setApplicationDisplayName(QStringLiteral("Rustinho"));
    app.setOrganizationName(QStringLiteral("rustinho"));

    // Banco, backups e impressões ficam na pasta privada do app.
    const QString dados = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    QDir().mkpath(dados);
    qputenv("RUSTINHO_DADOS", dados.toUtf8());

    qmlRegisterType<VistaCamera>("rustinho.camera", 1, 0, "VistaCamera");

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
    // Outro app pode mandar esse "extra": só passam "--abrir NomeDaTela" e
    // "--camera" (nada de dados). O resto é ignorado (C-13).
    static const QRegularExpression nomeTela(QStringLiteral("^Tela[A-Za-z]{1,40}$"));
    QStringList aceitos;
    for (int i = 0; i < extras.size(); ++i) {
        if (extras[i] == QLatin1String("--camera"))
            aceitos << extras[i];
        else if (extras[i] == QLatin1String("--abrir") && i + 1 < extras.size() && nomeTela.match(extras[i + 1]).hasMatch())
            aceitos << extras[i] << extras[++i];
    }
    extras = aceitos;
    if (!extras.isEmpty())
        qInfo("Rustinho: argumentos de teste: %s", qPrintable(extras.join(QLatin1Char(' '))));
    engine.rootContext()->setContextProperty(QStringLiteral("argumentosExtras"), extras);
    engine.rootContext()->setContextProperty(QStringLiteral("diagnostico"), &diagnostico);
    engine.addImportPath(QStringLiteral("qrc:/qt/qml"));
    diagnostico.marcar(QStringLiteral("carregando Main.qml"));
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/rustinho/qml/Main.qml")));
    if (engine.rootObjects().isEmpty()) {
        // A tela principal nem carregou: em vez de fechar em silêncio, avisa.
        diagnostico.marcar(QStringLiteral("Main.qml NÃO carregou"));
        QString msg = QStringLiteral("Rustinho: a tela principal não carregou. Mande o arquivo diagnostico.txt (pasta Android/data do app).");
        if (!diagnostico.primeirosErros().isEmpty())
            msg += QStringLiteral(" Erro: ") + diagnostico.primeirosErros().first().left(160);
        diagnostico.avisarNaTela(msg);
        QTimer::singleShot(10000, &app, &QCoreApplication::quit);
        app.exec();
        return 1;
    }
    diagnostico.marcar(QStringLiteral("Main.qml carregado"));
    diagnostico.vigiar(qobject_cast<QQuickWindow*>(engine.rootObjects().first()));
    return app.exec();
}
