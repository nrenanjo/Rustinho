# App de Android do Rustinho (Qt for Android). A montagem automática
# (.github/workflows/android.yml) roda:
#   qt-cmake -S android -B build-android && cmake --build build-android --target apk
cmake_minimum_required(VERSION 3.24)
project(Rustinho VERSION 0.9.12 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Network Multimedia)

if(NOT DEFINED RUSTINHO_VERSAO_CODIGO)
    math(EXPR RUSTINHO_VERSAO_CODIGO "${PROJECT_VERSION_MAJOR} * 10000 + ${PROJECT_VERSION_MINOR} * 100 + ${PROJECT_VERSION_PATCH}")
endif()
# cxx-qt-cmake que acompanha o cxx-qt 0.10.
set(CXX_QT_CMAKE_TAG "64e63f560737b3b7f85ef8267c5918e4b8176d5a" CACHE STRING "Versão do cxx-qt-cmake")

include(FetchContent)
FetchContent_Declare(
    CxxQt
    GIT_REPOSITORY https://github.com/kdab/cxx-qt-cmake.git
    GIT_TAG ${CXX_QT_CMAKE_TAG}
)
FetchContent_MakeAvailable(CxxQt)

# O qmake do Qt for Android (não o do computador), para o cxx-qt achar os
# cabeçalhos e as bibliotecas certas.
if(NOT DEFINED RUSTINHO_QMAKE)
    set(RUSTINHO_QMAKE "${QT6_INSTALL_PREFIX}/${QT6_INSTALL_BINS}/qmake")
endif()
cxx_qt_import_crate(
    MANIFEST_PATH rust/Cargo.toml
    CRATES rustinho_android
    QT_MODULES Qt::Core Qt::Gui Qt::Qml Qt::Quick Qt::Multimedia
    QMAKE "${RUSTINHO_QMAKE}"
)
cxx_qt_import_qml_module(rustinho_qml
    URI "rustinho"
    SOURCE_CRATE rustinho_android
)

qt_add_executable(Rustinho main.cpp vistacamera.cpp vistacamera.h)
target_link_libraries(Rustinho PRIVATE
    rustinho_qml
    Qt::Core Qt::Gui Qt::Qml Qt::Quick Qt::QuickControls2 Qt::Network Qt::Multimedia
)

set_target_properties(Rustinho PROPERTIES
    QT_ANDROID_PACKAGE_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/pacote"
    QT_ANDROID_PACKAGE_NAME "io.github.nrenanjo.rustinho"
    QT_ANDROID_VERSION_NAME "${PROJECT_VERSION}"
    QT_ANDROID_VERSION_CODE "${RUSTINHO_VERSAO_CODIGO}"
    QT_ANDROID_MIN_SDK_VERSION 28
    QT_ANDROID_TARGET_SDK_VERSION 34
)

# As telas ficam dentro do programa (qrc); diz ao Qt onde elas estão para
# ele levar junto os módulos QML que usam (Controls, Layouts, Dialogs...).
set_property(TARGET Rustinho APPEND PROPERTY QT_QML_ROOT_PATH
    "${CMAKE_CURRENT_SOURCE_DIR}/../app/qml" "${CMAKE_CURRENT_SOURCE_DIR}/../app/base"
    "${CMAKE_CURRENT_SOURCE_DIR}/../app/qml_android")

qt_import_qml_plugins(Rustinho)
