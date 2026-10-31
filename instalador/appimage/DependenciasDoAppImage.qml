// Não faz parte do programa: serve só para montar o AppImage.
// No Linux, sem tema nativo, as janelas de escolher arquivo e pasta
// (QtQuick.Dialogs) são feitas em QML pelo próprio Qt e usam estes módulos.
// O linuxdeploy-plugin-qt só junta os módulos que acha nos imports dos .qml
// que recebe e não enxerga os de dentro do Qt; sem o folderlistmodel,
// "Escolher arquivos" fechava o programa e "Escolher uma pasta" não abria.
import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Dialogs
import QtQuick.Dialogs.quickimpl
import QtQuick.Layouts
import QtQuick.Templates
import Qt.labs.folderlistmodel

Item {}
