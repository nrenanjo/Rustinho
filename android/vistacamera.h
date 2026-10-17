// Vista da câmera para o leitor de código de barras, sem depender da placa de
// vídeo do celular: cada quadro vira uma imagem em tons de cinza (a parte "Y"
// do quadro, que já é o cinza) e é pintada pela CPU. Funciona igual com o
// desenho por software (celulares de 32 bits e chips PowerVR).
#pragma once

#include <QtCore/QElapsedTimer>
#include <QtGui/QImage>
#include <QtMultimedia/QVideoFrame>
#include <QtMultimedia/QVideoSink>
#include <QtQuick/QQuickPaintedItem>

class VistaCamera : public QQuickPaintedItem
{
    Q_OBJECT
    Q_PROPERTY(bool temQuadro READ temQuadro NOTIFY temQuadroChanged)

public:
    explicit VistaCamera(QQuickItem* pai = nullptr);

    // A CaptureSession procura este método para mandar os quadros.
    Q_INVOKABLE QVideoSink* videoSink() const { return m_sink; }

    // Guarda o último quadro em PGM (tons de cinza), com no máximo essa largura.
    Q_INVOKABLE bool salvar(const QString& caminho, int larguraMaxima);

    bool temQuadro() const { return !m_imagem.isNull(); }
    void paint(QPainter* pintor) override;

signals:
    void temQuadroChanged();

private:
    void chegou(const QVideoFrame& quadro);

    QVideoSink* m_sink;
    QImage m_imagem;
    QElapsedTimer m_relogio;
};
