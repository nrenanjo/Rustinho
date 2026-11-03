import QtQuick
Window {
    visible: true; width: 360; height: 640; color: "#E8E4DC"
    Rectangle { x: 20; y: 20; width: 200; height: 60; color: "#2D5A9E"; radius: 8
        Text { anchors.centerIn: parent; text: "Rustinho"; color: "white"; font.pixelSize: 24 } }
    Text { x: 20; y: 120; text: "Vender  Comprar  Estoque"; font.pixelSize: 20 }
    Component.onCompleted: diagnostico.telaPronta("TelaTeste")
}
