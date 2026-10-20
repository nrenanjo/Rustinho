// Diagnóstico do Rustinho no Android (PEND-18, tela cinza).
//
// Serve para descobrir, sem cabo nem "adb", por que o app não mostra nada:
//  * grava tudo o que o Qt e as telas avisam (inclusive erros de QML) num
//    arquivo de texto numa pasta que o gerenciador de arquivos do celular abre;
//  * grava marcos ("Main.qml carregado", "primeiro quadro", "tela pronta");
//  * tira uma foto da janela depois de alguns segundos e vê se ela é de uma
//    cor só (o que o usuário vê como "tela toda cinza");
//  * se algo deu errado, mostra um aviso do próprio Android (Toast), que
//    funciona mesmo quando o desenho do Qt não funciona;
//  * deixa trocar o modo de desenho (processador × placa de vídeo) criando o
//    arquivo "desenho.txt", sem precisar de um APK novo.
// Não guarda senha, dado de cliente nem nada do banco: só mensagens do Qt.
#pragma once

#include <QtCore/QObject>
#include <QtCore/qlogging.h>
#include <QtCore/QString>
#include <QtCore/QStringList>

class QQuickWindow;

class Diagnostico : public QObject
{
    Q_OBJECT

public:
    explicit Diagnostico(QObject* pai = nullptr);

    // Pasta onde ficam os arquivos (a do app no cartão, ou a interna se
    // aquela não puder ser usada). Em testes: RUSTINHO_DIAGNOSTICO_PASTA.
    static QString pasta();

    // Abre o registro e instala o captador de mensagens. Chamar antes de criar
    // o QGuiApplication, para pegar tudo.
    void iniciar();

    // "gpu", "software" ou "" (automático), lido de desenho.txt.
    static QString desenhoPedido();

    // Marcos e erros. Seguro de chamar de qualquer tela (QML: diagnostico.marcar).
    Q_INVOKABLE void marcar(const QString& texto);
    Q_INVOKABLE void telaPronta(const QString& nome);

    // Começa a vigiar a janela principal: primeiro quadro, foto, aviso.
    void vigiar(QQuickWindow* janela, int segundosAteVerificar = 8);

    // Aviso do próprio Android (só no Android; nos testes vira uma linha de registro).
    void avisarNaTela(const QString& texto);

    // Para testes: a foto é de uma cor só?
    static bool imagemUniforme(const class QImage& imagem);
    static QString textoDoModoDeDesenho();

    bool temTelaPronta() const { return m_telaPronta; }
    QStringList primeirosErros() const { return m_erros; }

private:
    void verificar(QQuickWindow* janela);

    bool m_telaPronta = false;
    bool m_primeiroQuadro = false;
    bool m_avisou = false;
    QStringList m_erros;

    static void captador(QtMsgType tipo, const QMessageLogContext& contexto, const QString& texto);
    void escrever(const QString& linha);
};
