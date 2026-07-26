cmake_minimum_required(VERSION 3.28)

project(ChaftQtSdkProbe LANGUAGES CXX)

find_package(Qt6 6.8.4 EXACT REQUIRED COMPONENTS Network Qml Quick Widgets)

add_executable(chaft_qt_sdk_probe main.cpp)
target_compile_features(chaft_qt_sdk_probe PRIVATE cxx_std_20)
target_link_libraries(
    chaft_qt_sdk_probe
    PRIVATE
        Qt6::Network
        Qt6::Qml
        Qt6::Quick
        Qt6::Widgets
)
