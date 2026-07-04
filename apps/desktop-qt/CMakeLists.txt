cmake_minimum_required(VERSION 3.28)

project(ChaftDesktop VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(GNUInstallDirs)

find_package(Qt6 REQUIRED COMPONENTS Network Qml Quick)
qt_standard_project_setup(REQUIRES 6.8)

if(DEFINED QT_KNOWN_POLICY_QTP0004)
    qt_policy(SET QTP0004 NEW)
endif()

set_source_files_properties(qml/Chaft/tokens/Tokens.qml
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE
)

qt_add_executable(ChaftDesktop
    src/main.cpp
)

set(CHAFT_QML_MODULE_OPTIONS)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    list(APPEND CHAFT_QML_MODULE_OPTIONS NO_CACHEGEN)
endif()

set_target_properties(ChaftDesktop PROPERTIES
    MACOSX_BUNDLE TRUE
    MACOSX_BUNDLE_BUNDLE_NAME "Chaft"
    MACOSX_BUNDLE_BUNDLE_VERSION "${PROJECT_VERSION}"
    MACOSX_BUNDLE_GUI_IDENTIFIER "app.chaft.desktop"
    MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION}"
    WIN32_EXECUTABLE TRUE
)

qt_add_qml_module(ChaftDesktop
    URI Chaft
    VERSION 1.0
    ${CHAFT_QML_MODULE_OPTIONS}
    IMPORTS
        QtQuick
        QtQuick.Controls
        QtQuick.Dialogs
        QtQuick.Layouts
    QML_FILES
        qml/Chaft/App.qml
        qml/Chaft/tokens/Tokens.qml
        qml/Chaft/components/AttachmentRow.qml
        qml/Chaft/components/SidebarItem.qml
        qml/Chaft/components/WorkspaceRailItem.qml
        qml/Chaft/features/timeline/TimelineView.qml
        qml/Chaft/features/composer/ComposerBar.qml
        qml/Chaft/features/inspector/ChannelMediaPanel.qml
        qml/Chaft/features/inspector/InspectorThreadPanel.qml
)

if(APPLE)
    # Qt 6.8 on GitHub macOS runners can scan the source qml/ path as a
    # generated QML module under Chaft. Provide the expected empty module path.
    file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/Chaft/QML")
    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/Chaft/QML/qmldir" "module QML\n")
endif()

target_link_libraries(ChaftDesktop PRIVATE Qt6::Network Qt6::Qml Qt6::Quick)

install(TARGETS ChaftDesktop
    BUNDLE DESTINATION .
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

set(CHAFT_FFI_LIBRARY_PATH "" CACHE FILEPATH "Path to the built chaft-ffi dynamic library")
if(CHAFT_FFI_LIBRARY_PATH)
    if(APPLE)
        install(FILES "${CHAFT_FFI_LIBRARY_PATH}"
            DESTINATION "ChaftDesktop.app/Contents/MacOS"
        )
    else()
        install(FILES "${CHAFT_FFI_LIBRARY_PATH}"
            DESTINATION ${CMAKE_INSTALL_BINDIR}
        )
    endif()
endif()

qt_generate_deploy_qml_app_script(
    TARGET ChaftDesktop
    OUTPUT_SCRIPT chaft_desktop_deploy_script
    NO_UNSUPPORTED_PLATFORM_ERROR
)
install(SCRIPT ${chaft_desktop_deploy_script})

set(CPACK_PACKAGE_NAME "Chaft")
set(CPACK_PACKAGE_VENDOR "Chaft")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Native local-first peer-to-peer chat workspace")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/Jurshsmith/chaft")
set(CPACK_PACKAGE_DIRECTORY "${CMAKE_BINARY_DIR}/package")

if(APPLE)
    set(CPACK_GENERATOR "DragNDrop")
elseif(WIN32)
    set(CPACK_GENERATOR "ZIP")
else()
    set(CPACK_GENERATOR "TGZ")
endif()

include(CPack)
