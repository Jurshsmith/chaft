cmake_minimum_required(VERSION 3.28)

project(ChaftDesktop VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 REQUIRED COMPONENTS Network Quick)
qt_standard_project_setup(REQUIRES 6.7)

qt_add_executable(ChaftDesktop
    src/main.cpp
)

qt_add_qml_module(ChaftDesktop
    URI Chaft
    VERSION 1.0
    QML_FILES
        qml/Chaft/App.qml
        qml/Chaft/tokens/Tokens.qml
        qml/Chaft/components/SidebarItem.qml
        qml/Chaft/features/timeline/TimelineView.qml
        qml/Chaft/features/composer/ComposerBar.qml
)

target_link_libraries(ChaftDesktop PRIVATE Qt6::Network Qt6::Quick)
