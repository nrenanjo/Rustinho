#include "rustinho/cpp/pdf.h"

#include <algorithm>
#include <cmath>

#include <QtCore/QDir>
#include <QtCore/QFileInfo>
#include <QtCore/QMarginsF>
#include <QtCore/QUrl>
#include <QtGui/QDesktopServices>
#include <QtGui/QFont>
#include <QtGui/QPageLayout>
#include <QtGui/QPageSize>
#include <QtGui/QPdfWriter>
#include <QtGui/QTextDocument>

bool rustinho_gerar_pdf(const QString& html, const QString& caminho)
{
    QDir().mkpath(QFileInfo(caminho).absolutePath());
    QPdfWriter pdf(caminho);
    pdf.setPageSize(QPageSize(QPageSize::A4));
    pdf.setPageMargins(QMarginsF(15, 15, 15, 15), QPageLayout::Millimeter);
    pdf.setResolution(300);
    pdf.setCreator(QStringLiteral("Rustinho"));
    pdf.setTitle(QFileInfo(caminho).completeBaseName());

    QTextDocument documento;
    documento.setDefaultFont(QFont(QStringLiteral("Source Sans 3"), 10));
    documento.setHtml(html);
    documento.print(&pdf);
    return QFileInfo::exists(caminho);
}

bool rustinho_abrir_arquivo(const QString& caminho)
{
    return QDesktopServices::openUrl(QUrl::fromLocalFile(caminho));
}

// ---------------------------------------------------------------------------
// Etiquetas
// ---------------------------------------------------------------------------

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QStringList>
#include <QtGui/QFontMetricsF>
#include <QtGui/QPainter>

namespace {

// Quebra o texto em até `maxLinhas` linhas que cabem em `largura`; a última
// ganha "..." se sobrar texto.
QStringList quebrar(const QString& texto, const QFontMetricsF& fm, double largura, int maxLinhas)
{
    QStringList linhas;
    const QStringList palavras = texto.simplified().split(QLatin1Char(' '), Qt::SkipEmptyParts);
    QString atual;
    int i = 0;
    for (; i < palavras.size(); ++i) {
        const QString tentativa = atual.isEmpty() ? palavras[i] : atual + QLatin1Char(' ') + palavras[i];
        if (fm.horizontalAdvance(tentativa) <= largura || atual.isEmpty()) {
            atual = tentativa;
        } else {
            linhas << atual;
            atual = palavras[i];
            if (linhas.size() == maxLinhas)
                break;
        }
    }
    if (linhas.size() < maxLinhas && !atual.isEmpty()) {
        linhas << atual;
        atual.clear();
        i = palavras.size();
    }
    if (!linhas.isEmpty()) {
        QString ultima = linhas.last();
        if (i < palavras.size() || fm.horizontalAdvance(ultima) > largura) {
            for (int j = i; j < palavras.size(); ++j)
                ultima += QLatin1Char(' ') + palavras[j];
            linhas.last() = fm.elidedText(ultima, Qt::ElideRight, largura);
        }
    }
    return linhas;
}

QFont fonte(double pontos, int peso)
{
    QFont f(QStringLiteral("Source Sans 3"));
    f.setPointSizeF(pontos);
    f.setWeight(static_cast<QFont::Weight>(peso));
    return f;
}

void desenharEtiqueta(QPainter& p, QPaintDevice* dispositivo, const QRectF& caixa, double dpmm, const QJsonObject& e)
{
    const double h = caixa.height() / dpmm; // altura em mm
    const bool pequena = h < 23.0;
    const bool grande = h > 30.0;
    const QString barras = e.value(QStringLiteral("barras")).toString();
    const bool comBarras = !barras.isEmpty();

    const double pad = 1.6 * dpmm;
    QRectF r = caixa.adjusted(pad, pad * 0.9, -pad, -pad * 0.8);
    const QColor tinta(0x2A, 0x26, 0x22);
    const QColor suave(0x5C, 0x53, 0x4A);
    p.setPen(tinta);

    double y = r.top();

    // Loja (opcional)
    const QString loja = e.value(QStringLiteral("loja")).toString();
    if (!loja.isEmpty()) {
        QFont f = fonte(pequena ? 4.5 : 5.5, QFont::DemiBold);
        f.setCapitalization(QFont::AllUppercase);
        f.setLetterSpacing(QFont::PercentageSpacing, 108);
        QFontMetricsF fm(f, dispositivo);
        p.setFont(f);
        p.setPen(suave);
        p.drawText(QPointF(r.left(), y + fm.ascent()), fm.elidedText(loja, Qt::ElideRight, r.width()));
        y += fm.height();
        p.setPen(tinta);
    }

    // Nome da peça
    {
        QFont f = fonte(pequena ? 6.2 : grande ? 9.0 : 7.4, QFont::DemiBold);
        QFontMetricsF fm(f, dispositivo);
        p.setFont(f);
        const int maxLinhas = pequena ? (comBarras ? 1 : 2) : 2;
        for (const QString& linha : quebrar(e.value(QStringLiteral("nome")).toString(), fm, r.width(), maxLinhas)) {
            p.drawText(QPointF(r.left(), y + fm.ascent()), linha);
            y += fm.lineSpacing() * 0.95;
        }
    }

    // Barras embaixo (com o número)
    double fundo = r.bottom();
    if (comBarras) {
        QFont ft = fonte(pequena ? 4.4 : grande ? 6.0 : 5.0, QFont::Normal);
        QFontMetricsF fmt(ft, dispositivo);
        const double alturaBarras = (pequena ? 5.0 : grande ? 11.0 : 6.6) * dpmm;
        const double topoBarras = r.bottom() - fmt.height() - alturaBarras;
        // Módulo inteiro em pontos do PDF, para as barras saírem nítidas.
        const int n = barras.size();
        double modulo = std::floor(std::min(0.33 * dpmm, r.width() / (n + 12.0)));
        if (modulo < 1.0)
            modulo = 1.0;
        const double largura = modulo * n;
        double x = r.center().x() - largura / 2.0;
        int i = 0;
        while (i < n) {
            if (barras[i] == QLatin1Char('1')) {
                int j = i;
                while (j < n && barras[j] == QLatin1Char('1'))
                    ++j;
                p.fillRect(QRectF(x + i * modulo, topoBarras, (j - i) * modulo, alturaBarras), tinta);
                i = j;
            } else {
                ++i;
            }
        }
        p.setFont(ft);
        p.setPen(tinta);
        p.drawText(QRectF(r.left(), topoBarras + alturaBarras, r.width(), fmt.height()), Qt::AlignHCenter | Qt::AlignTop,
                   e.value(QStringLiteral("textoBarras")).toString());
        fundo = topoBarras - 0.6 * dpmm;
    }

    // Preço (grande) e o código interno
    {
        QFont fp = fonte(pequena ? (comBarras ? 10.5 : 13.0) : grande ? (comBarras ? 17.0 : 22.0) : (comBarras ? 12.5 : 16.0), QFont::Bold);
        QFontMetricsF fmp(fp, dispositivo);
        QFont fc = fonte(pequena ? 4.6 : 5.6, QFont::Normal);
        QFontMetricsF fmc(fc, dispositivo);
        double base = comBarras ? fundo - fmp.descent() : std::max(y + fmp.ascent(), (y + r.bottom()) / 2.0 + fmp.ascent() / 2.0);
        if (base > fundo)
            base = fundo;
        p.setFont(fp);
        p.setPen(tinta);
        p.drawText(QPointF(r.left(), base), e.value(QStringLiteral("preco")).toString());
        p.setFont(fc);
        p.setPen(suave);
        const QString codigo = e.value(QStringLiteral("codigo")).toString();
        p.drawText(QPointF(r.right() - fmc.horizontalAdvance(codigo), base), codigo);
    }
}

} // namespace

namespace {

// Desenha as folhas de etiquetas em qualquer "papel" (PDF ou impressora).
bool desenharFolhas(QPagedPaintDevice& dispositivo, double resolucao, const QJsonObject& raiz)
{
    const QJsonObject m = raiz.value(QStringLiteral("modelo")).toObject();
    const QJsonArray lista = raiz.value(QStringLiteral("etiquetas")).toArray();
    if (lista.isEmpty())
        return false;
    const int colunas = m.value(QStringLiteral("colunas")).toInt(3);
    const int linhas = m.value(QStringLiteral("linhas")).toInt(11);
    const double largura = m.value(QStringLiteral("largura")).toDouble();
    const double altura = m.value(QStringLiteral("altura")).toDouble();
    const double topo = m.value(QStringLiteral("topo")).toDouble() + raiz.value(QStringLiteral("ajusteY")).toDouble();
    const double esquerda = m.value(QStringLiteral("esquerda")).toDouble() + raiz.value(QStringLiteral("ajusteX")).toDouble();
    const double espacoH = m.value(QStringLiteral("espacoH")).toDouble();
    const double espacoV = m.value(QStringLiteral("espacoV")).toDouble();
    const int porFolha = colunas * linhas;

    QPainter p(&dispositivo);
    if (!p.isActive())
        return false;
    p.setRenderHint(QPainter::Antialiasing, false);
    p.setRenderHint(QPainter::TextAntialiasing, true);
    const double dpmm = resolucao / 25.4;
    int posicao = std::max(0, raiz.value(QStringLiteral("inicio")).toInt(1) - 1);
    for (const QJsonValue& v : lista) {
        if (posicao >= porFolha) {
            dispositivo.newPage();
            posicao = 0;
        }
        const int col = posicao % colunas;
        const int lin = posicao / colunas;
        const QRectF caixa((esquerda + col * (largura + espacoH)) * dpmm, (topo + lin * (altura + espacoV)) * dpmm,
                           largura * dpmm, altura * dpmm);
        p.save();
        p.setClipRect(caixa);
        desenharEtiqueta(p, &dispositivo, caixa, dpmm, v.toObject());
        p.restore();
        ++posicao;
    }
    p.end();
    return true;
}

} // namespace

bool rustinho_gerar_etiquetas(const QString& json, const QString& caminho)
{
    const QJsonObject raiz = QJsonDocument::fromJson(json.toUtf8()).object();
    QDir().mkpath(QFileInfo(caminho).absolutePath());
    QPdfWriter pdf(caminho);
    pdf.setPageSize(QPageSize(QPageSize::A4));
    pdf.setPageMargins(QMarginsF(0, 0, 0, 0), QPageLayout::Millimeter);
    pdf.setResolution(600);
    pdf.setCreator(QStringLiteral("Rustinho"));
    pdf.setTitle(QStringLiteral("Etiquetas"));
    if (!desenharFolhas(pdf, pdf.resolution(), raiz))
        return false;
    return QFileInfo::exists(caminho);
}

// ---------------------------------------------------------------------------
// Impressão direta na impressora padrão (Windows; no Linux usa o "lp").
// ---------------------------------------------------------------------------

#ifdef Q_OS_WIN
#include <QtPrintSupport/QPrinter>
#include <QtPrintSupport/QPrinterInfo>

bool rustinho_imprimir_html(const QString& html)
{
    if (QPrinterInfo::defaultPrinter().isNull())
        return false;
    QPrinter impressora(QPrinter::HighResolution);
    impressora.setPageSize(QPageSize(QPageSize::A4));
    impressora.setPageMargins(QMarginsF(15, 15, 15, 15), QPageLayout::Millimeter);
    impressora.setDocName(QStringLiteral("Rustinho"));
    QTextDocument documento;
    documento.setDefaultFont(QFont(QStringLiteral("Source Sans 3"), 10));
    documento.setHtml(html);
    documento.print(&impressora);
    return impressora.printerState() != QPrinter::Error;
}

bool rustinho_imprimir_etiquetas(const QString& json)
{
    if (QPrinterInfo::defaultPrinter().isNull())
        return false;
    QPrinter impressora(QPrinter::HighResolution);
    impressora.setPageSize(QPageSize(QPageSize::A4));
    impressora.setFullPage(true);
    impressora.setPageMargins(QMarginsF(0, 0, 0, 0), QPageLayout::Millimeter);
    impressora.setDocName(QStringLiteral("Rustinho - etiquetas"));
    const QJsonObject raiz = QJsonDocument::fromJson(json.toUtf8()).object();
    return desenharFolhas(impressora, impressora.resolution(), raiz) && impressora.printerState() != QPrinter::Error;
}
#else
bool rustinho_imprimir_html(const QString&) { return false; }
bool rustinho_imprimir_etiquetas(const QString&) { return false; }
#endif
