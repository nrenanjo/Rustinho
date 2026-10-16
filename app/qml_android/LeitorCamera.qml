import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtCore
import QtMultimedia
import rustinho

// Câmera do celular como leitor de código de barras. Só entra no app de
// Android (no computador, o leitor é o de mão, que "digita" o código).
// Uso: leitor.ler(codigo => { ... })
Item {
    id: leitor
    property var depois: null
    property bool ocupado: false
    readonly property string foto: Api.caminho(StandardPaths.writableLocation(StandardPaths.CacheLocation)) + "/quadro.pgm"

    function ler(funcao) {
        depois = funcao
        janelaCamera.open()
    }

    CameraPermission { id: permissao }

    Popup {
        id: janelaCamera
        parent: Overlay.overlay
        width: parent ? parent.width : 0
        height: parent ? parent.height : 0
        modal: true
        padding: 0
        closePolicy: Popup.CloseOnEscape
        background: Rectangle { color: "#1E1A16" }

        onOpened: {
            leitor.ocupado = false
            if (permissao.status !== Qt.PermissionStatus.Granted)
                permissao.request()
        }
        onClosed: leitor.depois = null

        CaptureSession {
            camera: Camera {
                id: camera
                active: janelaCamera.opened && permissao.status === Qt.PermissionStatus.Granted
                focusMode: Camera.FocusModeAuto
            }
            videoOutput: saida
        }

        VideoOutput {
            id: saida
            anchors.fill: parent
            fillMode: VideoOutput.PreserveAspectCrop
        }

        // A cada meio segundo, uma "foto" do que a câmera vê vai para o núcleo.
        Timer {
            interval: 450
            repeat: true
            running: camera.active
            onTriggered: {
                if (leitor.ocupado)
                    return
                leitor.ocupado = true
                const larg = Math.min(saida.width, 900)
                const alt = Math.round(saida.height * larg / Math.max(1, saida.width))
                const ok = saida.grabToImage(resultado => {
                    resultado.saveToFile(leitor.foto)
                    const r = Api.chamar("codigo_ler_foto", { caminho: leitor.foto })
                    leitor.ocupado = false
                    if (r.ok && r.dados.codigo && janelaCamera.opened) {
                        const f = leitor.depois
                        janelaCamera.close()
                        if (f)
                            f(r.dados.codigo)
                    }
                }, Qt.size(larg, alt))
                if (!ok)
                    leitor.ocupado = false
            }
        }

        // Moldura: a pílula onde o código deve ficar.
        Rectangle {
            anchors.centerIn: parent
            width: parent.width * 0.82
            height: Math.min(parent.height * 0.22, 180)
            radius: height / 2
            color: "transparent"
            border.width: 3
            border.color: Tema.verde
        }

        ColumnLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 22
            spacing: 14

            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                Layout.maximumWidth: parent.width
                implicitWidth: aviso.implicitWidth + 36
                implicitHeight: aviso.implicitHeight + 18
                radius: height / 2
                color: Tema.pergaminho
                Text {
                    id: aviso
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, leitor.width - 80)
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    text: permissao.status === Qt.PermissionStatus.Denied
                          ? "O Rustinho não pode usar a câmera. Libere nas configurações do Android."
                          : "Aponte para o código de barras"
                    color: Tema.tinta
                    font.family: Tema.texto
                    font.weight: Font.DemiBold
                    font.pixelSize: 16
                }
            }
            Botao {
                text: "Fechar"
                icone: "fechar"
                tipo: "secundario"
                Layout.fillWidth: true
                onClicked: janelaCamera.close()
            }
        }
    }
}
