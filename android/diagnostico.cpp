#include "diagnostico.h"

#include <QtCore/QDateTime>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QMutex>
#include <QtCore/QMutexLocker>
#include <QtCore/QStandardPaths>
#include <QtCore/QSysInfo>
#include <QtCore/QTimer>
#include <QtGui/QImage>
#include <QtQuick/QQuickWindow>

#ifdef Q_OS_ANDROID
#include <QtCore/QJniObject>
#include <QtCore/qcoreapplication_platform.h>
#include <sys/system_properties.h>
#endif

namespace {
Diagnostico* g_instancia = nullptr;
QtMessageHandler g_anterior = nullptr;
QFile* g_arquivo = nullptr;
QMutex g_trava;
constexpr qint64 LIMITE_DO_ARQUIVO = 2 * 1024 * 1024;

#ifdef Q_OS_ANDROID
QString propriedade(const char* nome)
{
    char valor[PROP_VALUE_MAX] = {};
    return __system_property_get(nome, valor) > 0 ? QString::fromUtf8(valor) : QString();
}

// O nome do pacote é o nome do processo (/proc/self/cmdline).
QString nomeDoPacote()
{
    QFile f(QStringLiteral("/proc/self/cmdline"));
    if (!f.open(QIODevice::ReadOnly))
        return {};
    const QByteArray b = f.readAll();
    return QString::fromUtf8(b.left(b.indexOf('\0'))).trimmed();
}
#endif
} // namespace

Diagnostico::Diagnostico(QObject* pai) : QObject(pai) {}

QString Diagnostico::pasta()
{
    const QByteArray forcada = qgetenv("RUSTINHO_DIAGNOSTICO_PASTA");
    if (!forcada.isEmpty()) {
        QDir().mkpath(QString::fromUtf8(forcada));
        return QString::fromUtf8(forcada);
    }
#ifdef Q_OS_ANDROID
    // Android/data/<pacote>/files: o gerenciador de arquivos abre (Android 10).
    const QString pacote = nomeDoPacote();
    if (!pacote.isEmpty()) {
        const QString p = QStringLiteral("/sdcard/Android/data/%1/files").arg(pacote);
        if (QDir().mkpath(p)) {
            QFile prova(p + QStringLiteral("/.prova"));
            if (prova.open(QIODevice::WriteOnly)) {
                prova.close();
                prova.remove();
                return p;
            }
        }
    }
#endif
    const QString interna = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    QDir().mkpath(interna);
    return interna;
}

QString Diagnostico::desenhoPedido()
{
    QFile f(pasta() + QStringLiteral("/desenho.txt"));
    if (!f.open(QIODevice::ReadOnly))
        return {};
    const QString t = QString::fromUtf8(f.read(64)).trimmed().toLower();
    return (t == QLatin1String("gpu") || t == QLatin1String("software")) ? t : QString();
}

QString Diagnostico::textoDoModoDeDesenho()
{
    const QByteArray v = qgetenv("QT_QUICK_BACKEND");
    return v.isEmpty() ? QStringLiteral("automático do Qt (placa de vídeo)") : QString::fromUtf8(v);
}

void Diagnostico::escrever(const QString& linha)
{
    QMutexLocker l(&g_trava);
    if (!g_arquivo)
        return;
    const QByteArray b = QDateTime::currentDateTime().toString(QStringLiteral("HH:mm:ss.zzz ")).toUtf8() + linha.toUtf8() + '\n';
    if (g_arquivo->size() + b.size() > LIMITE_DO_ARQUIVO)
        return; // não deixa o registro crescer sem fim
    g_arquivo->write(b);
    g_arquivo->flush();
}

void Diagnostico::captador(QtMsgType tipo, const QMessageLogContext& contexto, const QString& texto)
{
    if (g_instancia) {
        const char* nome = tipo == QtDebugMsg ? "debug" : tipo == QtInfoMsg ? "info" : tipo == QtWarningMsg ? "AVISO" : "ERRO";
        QString onde;
        if (contexto.file && contexto.line > 0)
            onde = QStringLiteral(" (%1:%2)").arg(QString::fromUtf8(contexto.file), QString::number(contexto.line));
        g_instancia->escrever(QStringLiteral("[%1] %2%3").arg(QString::fromLatin1(nome), texto.left(600), onde));
        // Guarda os primeiros avisos de verdade (ruído conhecido fica de fora)
        // para o aviso na tela.
        const bool ruido = texto.startsWith(QLatin1String("QStandardPaths")) || texto.contains(QLatin1String("XDG_RUNTIME_DIR"));
        if ((tipo == QtWarningMsg || tipo == QtCriticalMsg || tipo == QtFatalMsg) && !ruido && g_instancia->m_erros.size() < 6)
            g_instancia->m_erros << texto.left(300);
    }
    if (g_anterior)
        g_anterior(tipo, contexto, texto);
}

void Diagnostico::iniciar()
{
    g_instancia = this;
    const QString dir = pasta();
    const QString atual = dir + QStringLiteral("/diagnostico.txt");
    const QString anterior = dir + QStringLiteral("/diagnostico-anterior.txt");
    if (QFile::exists(atual)) {
        QFile::remove(anterior);
        QFile::rename(atual, anterior);
    }
    g_arquivo = new QFile(atual);
    if (!g_arquivo->open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        delete g_arquivo;
        g_arquivo = nullptr;
    }
    g_anterior = qInstallMessageHandler(&Diagnostico::captador);

    escrever(QStringLiteral("== Rustinho: diagnóstico de abertura =="));
    escrever(QStringLiteral("pasta: %1").arg(dir));
    escrever(QStringLiteral("qt: %1 · compilado para: %2 · sistema: %3 %4")
                 .arg(QString::fromLatin1(qVersion()), QSysInfo::buildCpuArchitecture(), QSysInfo::productType(), QSysInfo::productVersion()));
#ifdef Q_OS_ANDROID
    escrever(QStringLiteral("aparelho: %1 %2 · android %3 · cpu: %4 · placa: %5/%6")
                 .arg(propriedade("ro.product.manufacturer"), propriedade("ro.product.model"), propriedade("ro.build.version.release"),
                      propriedade("ro.product.cpu.abi"), propriedade("ro.hardware.egl"), propriedade("ro.board.platform")));
    escrever(QStringLiteral("pacote: %1").arg(nomeDoPacote()));
#endif
}

void Diagnostico::marcar(const QString& texto)
{
    escrever(QStringLiteral("[marco] %1").arg(texto));
}

void Diagnostico::telaPronta(const QString& nome)
{
    m_telaPronta = true;
    marcar(QStringLiteral("tela pronta: %1").arg(nome));
}

bool Diagnostico::imagemUniforme(const QImage& imagem)
{
    if (imagem.isNull() || imagem.width() < 8 || imagem.height() < 8)
        return true;
    const QImage i = imagem.convertToFormat(QImage::Format_RGB32);
    QSet<QRgb> cores;
    const int passoX = qMax(1, i.width() / 60), passoY = qMax(1, i.height() / 60);
    for (int y = 0; y < i.height(); y += passoY)
        for (int x = 0; x < i.width(); x += passoX)
            cores.insert(i.pixel(x, y));
    // Texto e botões têm bem mais de 3 cores (bordas suavizadas incluídas).
    return cores.size() <= 3;
}

void Diagnostico::avisarNaTela(const QString& texto)
{
    escrever(QStringLiteral("[aviso na tela] %1").arg(texto));
#ifdef Q_OS_ANDROID
    QNativeInterface::QAndroidApplication::runOnAndroidMainThread([texto]() -> QVariant {
        QJniObject contexto = QNativeInterface::QAndroidApplication::context();
        QJniObject toast = QJniObject::callStaticObjectMethod(
            "android/widget/Toast", "makeText", "(Landroid/content/Context;Ljava/lang/CharSequence;I)Landroid/widget/Toast;",
            contexto.object(), QJniObject::fromString(texto).object<jstring>(), 1 /* LENGTH_LONG */);
        if (toast.isValid())
            toast.callMethod<void>("show");
        return QVariant();
    });
#endif
}

void Diagnostico::vigiar(QQuickWindow* janela, int segundos)
{
    if (!janela)
        return;
    connect(janela, &QQuickWindow::frameSwapped, this, [this] {
        if (!m_primeiroQuadro) {
            m_primeiroQuadro = true;
            marcar(QStringLiteral("primeiro quadro desenhado"));
        }
    });
    connect(janela, &QQuickWindow::sceneGraphError, this, [this](QQuickWindow::SceneGraphError, const QString& msg) {
        escrever(QStringLiteral("[ERRO do desenho] %1").arg(msg));
        avisarNaTela(QStringLiteral("Rustinho: erro ao desenhar a tela. Veja diagnostico.txt na pasta do app."));
    });
    QTimer::singleShot(segundos * 1000, this, [this, janela] { verificar(janela); });
    // Segunda conferência, mais tarde, para pegar telas lentas.
    QTimer::singleShot((segundos + 12) * 1000, this, [this, janela] { verificar(janela); });
}

void Diagnostico::verificar(QQuickWindow* janela)
{
    if (!janela)
        return;
    const QImage foto = janela->grabWindow();
    const bool uniforme = imagemUniforme(foto);
    const QString nome = QStringLiteral("captura-%1.png").arg(QDateTime::currentDateTime().toString(QStringLiteral("HHmmss")));
    foto.save(pasta() + QLatin1Char('/') + nome);
    escrever(QStringLiteral("[verificação] janela %1x%2 · uniforme (uma cor só): %3 · tela pronta: %4 · primeiro quadro: %5 · foto: %6")
                 .arg(janela->width())
                 .arg(janela->height())
                 .arg(uniforme ? QStringLiteral("SIM") : QStringLiteral("não"), m_telaPronta ? QStringLiteral("sim") : QStringLiteral("NÃO"),
                      m_primeiroQuadro ? QStringLiteral("sim") : QStringLiteral("NÃO"), nome));
    if ((uniforme || !m_telaPronta) && !m_avisou) {
        m_avisou = true;
        QString msg = QStringLiteral("Rustinho: a tela não abriu direito. Mande o arquivo diagnostico.txt (pasta Android/data do app).");
        if (!m_erros.isEmpty())
            msg += QStringLiteral(" Primeiro erro: ") + m_erros.first().left(160);
        avisarNaTela(msg);
    }
}
