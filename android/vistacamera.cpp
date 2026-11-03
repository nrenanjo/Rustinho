#include "vistacamera.h"

#include <QtCore/QtGlobal>
#include <QtGui/QPainter>
#include <QtGui/QTransform>

namespace {

// Quadros maiores que isso são reduzidos ao copiar (menos memória e CPU).
constexpr int LARGURA_MAXIMA = 960;

bool temPlanoCinza(QVideoFrameFormat::PixelFormat f)
{
    using F = QVideoFrameFormat::PixelFormat;
    switch (f) {
    case F::Format_NV12:
    case F::Format_NV21:
    case F::Format_YUV420P:
    case F::Format_YV12:
    case F::Format_YUV422P:
    case F::Format_Y8:
        return true;
    default:
        return false;
    }
}

QImage paraCinza(QVideoFrame quadro)
{
    if (!quadro.isValid())
        return {};
    QImage imagem;
    if (temPlanoCinza(quadro.pixelFormat()) && quadro.map(QVideoFrame::ReadOnly)) {
        const int largura = quadro.width();
        const int altura = quadro.height();
        const int passo = qMax(1, (largura + LARGURA_MAXIMA - 1) / LARGURA_MAXIMA);
        const int bpl = quadro.bytesPerLine(0);
        const uchar* origem = quadro.bits(0);
        if (origem && largura > 0 && altura > 0) {
            imagem = QImage(largura / passo, altura / passo, QImage::Format_Grayscale8);
            for (int y = 0; y < imagem.height(); ++y) {
                const uchar* linha = origem + qsizetype(y) * passo * bpl;
                uchar* destino = imagem.scanLine(y);
                for (int x = 0; x < imagem.width(); ++x)
                    destino[x] = linha[x * passo];
            }
        }
        quadro.unmap();
    }
    if (imagem.isNull()) {
        imagem = quadro.toImage();
        if (imagem.isNull())
            return {};
        if (imagem.width() > LARGURA_MAXIMA)
            imagem = imagem.scaledToWidth(LARGURA_MAXIMA, Qt::FastTransformation);
        imagem = imagem.convertToFormat(QImage::Format_Grayscale8);
    }
#if QT_VERSION >= QT_VERSION_CHECK(6, 7, 0)
    int graus = 0;
    switch (quadro.rotation()) {
    case QtVideo::Rotation::Clockwise90: graus = 90; break;
    case QtVideo::Rotation::Clockwise180: graus = 180; break;
    case QtVideo::Rotation::Clockwise270: graus = 270; break;
    default: break;
    }
    if (graus)
        imagem = imagem.transformed(QTransform().rotate(graus));
    if (quadro.mirrored())
        imagem = imagem.mirrored(true, false);
#endif
    return imagem;
}

} // namespace

VistaCamera::VistaCamera(QQuickItem* pai)
    : QQuickPaintedItem(pai)
    , m_sink(new QVideoSink(this))
{
    setFillColor(Qt::black);
    connect(m_sink, &QVideoSink::videoFrameChanged, this, &VistaCamera::chegou);
    m_relogio.start();
}

void VistaCamera::chegou(const QVideoFrame& quadro)
{
    // Uns 12 quadros por segundo bastam para mirar e não pesam no celular.
    if (!m_imagem.isNull() && m_relogio.elapsed() < 80)
        return;
    m_relogio.restart();
    QImage nova = paraCinza(quadro);
    if (nova.isNull())
        return;
    const bool primeiro = m_imagem.isNull();
    m_imagem = std::move(nova);
    if (primeiro)
        emit temQuadroChanged();
    update();
}

void VistaCamera::paint(QPainter* pintor)
{
    if (m_imagem.isNull() || width() <= 0 || height() <= 0)
        return;
    // Preenche a área toda, cortando as sobras (como "PreserveAspectCrop").
    const qreal escala = qMax(width() / m_imagem.width(), height() / m_imagem.height());
    const qreal largura = m_imagem.width() * escala;
    const qreal altura = m_imagem.height() * escala;
    const QRectF alvo((width() - largura) / 2, (height() - altura) / 2, largura, altura);
    pintor->drawImage(alvo, m_imagem);
}

bool VistaCamera::salvar(const QString& caminho, int larguraMaxima)
{
    if (m_imagem.isNull())
        return false;
    QImage imagem = m_imagem;
    if (larguraMaxima > 0 && imagem.width() > larguraMaxima)
        imagem = imagem.scaledToWidth(larguraMaxima, Qt::SmoothTransformation);
    return imagem.save(caminho, "PGM");
}
