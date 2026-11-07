#include "rustinho/cpp/janela.h"

#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QStandardPaths>
#include <QtCore/QSysInfo>
#include <QtCore/QUrl>
#ifdef Q_OS_ANDROID
#include <QtCore/QJniObject>
#include <QtCore/qcoreapplication_platform.h>
#endif

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

// Identificador que o sistema dá a este computador (Windows: MachineGuid,
// Linux: machine-id, Android: ANDROID_ID). O núcleo guarda só uma impressão
// digital dele no banco, para saber se o banco foi copiado para outro
// computador.
QString rustinho_identidade_do_computador()
{
#ifdef Q_OS_ANDROID
    // No Android o Qt não informa nada (QSysInfo::machineUniqueId procura o
    // machine-id do Linux, que o Android não tem), e sem identidade o núcleo
    // recusava cadastrar a loja (as faixas de códigos das peças exigem saber
    // em que aparelho o banco está): o app nunca saía da tela de boas-vindas.
    // O ANDROID_ID é o identificador do Android para este app neste aparelho:
    // muda em outro aparelho ou num aparelho zerado, que é o que importa aqui.
    const QJniObject contexto = QNativeInterface::QAndroidApplication::context();
    if (!contexto.isValid())
        return QString();
    const QJniObject resolvedor = contexto.callObjectMethod("getContentResolver", "()Landroid/content/ContentResolver;");
    if (!resolvedor.isValid())
        return QString();
    const QJniObject id = QJniObject::callStaticObjectMethod(
        "android/provider/Settings$Secure", "getString",
        "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;", resolvedor.object(),
        QJniObject::fromString(QStringLiteral("android_id")).object<jstring>());
    const QString texto = id.isValid() ? id.toString().trimmed() : QString();
    return texto.isEmpty() ? QString() : QStringLiteral("android:") + texto;
#else
    return QString::fromUtf8(QSysInfo::machineUniqueId());
#endif
}
