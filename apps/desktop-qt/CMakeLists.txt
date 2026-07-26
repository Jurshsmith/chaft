cmake_minimum_required(VERSION 3.28)

project(ChaftDesktop VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(GNUInstallDirs)

find_package(Qt6 6.8.4 EXACT REQUIRED COMPONENTS Network Qml Quick Widgets)
qt_standard_project_setup(REQUIRES 6.8.4)

if(DEFINED QT_KNOWN_POLICY_QTP0004)
    qt_policy(SET QTP0004 NEW)
endif()

set_source_files_properties(
    qml/Chaft/tokens/AvatarCatalog.qml
    qml/Chaft/tokens/Themes.qml
    qml/Chaft/tokens/Tokens.qml
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE
)

qt_add_executable(ChaftDesktop
    src/main.cpp
)

target_compile_definitions(ChaftDesktop PRIVATE
    CHAFT_DESKTOP_VERSION="${PROJECT_VERSION}"
)

if(MSVC)
    target_compile_options(ChaftDesktop PRIVATE /bigobj)
endif()

if(WIN32)
    enable_language(RC)
    configure_file(
        resources/windows/chaft.ico
        "${CMAKE_CURRENT_BINARY_DIR}/chaft.ico"
        COPYONLY
    )
    configure_file(
        resources/windows/ChaftDesktop.rc.in
        "${CMAKE_CURRENT_BINARY_DIR}/ChaftDesktop.rc"
        @ONLY
    )
    target_sources(ChaftDesktop PRIVATE
        "${CMAKE_CURRENT_BINARY_DIR}/ChaftDesktop.rc"
    )
    set_source_files_properties(
        "${CMAKE_CURRENT_BINARY_DIR}/ChaftDesktop.rc"
        PROPERTIES
            OBJECT_DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/chaft.ico"
    )
endif()

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
        qml/Chaft/tokens/AvatarCatalog.qml
        qml/Chaft/tokens/Themes.qml
        qml/Chaft/tokens/Tokens.qml
        qml/Chaft/components/AvatarMark.qml
        qml/Chaft/components/AvatarPicker.qml
        qml/Chaft/components/AttachmentRow.qml
        qml/Chaft/components/BackupPeerStatusRow.qml
        qml/Chaft/components/BrandMark.qml
        qml/Chaft/components/KeyPackageRow.qml
        qml/Chaft/components/MemberRow.qml
        qml/Chaft/components/OnboardingActionCard.qml
        qml/Chaft/components/PeerEndpointHintRow.qml
        qml/Chaft/components/PeerMeshBackdrop.qml
        qml/Chaft/components/PeerRouteChip.qml
        qml/Chaft/components/ReactionPicker.qml
        qml/Chaft/components/SavedBackupPeerRow.qml
        qml/Chaft/components/SetupSection.qml
        qml/Chaft/components/SidebarItem.qml
        qml/Chaft/components/StatusChip.qml
        qml/Chaft/components/controls/LabeledField.qml
        qml/Chaft/components/SyncStatusPill.qml
        qml/Chaft/components/ThemeSwatch.qml
        qml/Chaft/components/ToastHost.qml
        qml/Chaft/components/ConfirmDialog.qml
        qml/Chaft/components/TimelineActionChip.qml
        qml/Chaft/components/TimelineAttachmentChip.qml
        qml/Chaft/components/TimelineAuthorMark.qml
        qml/Chaft/components/TimelineEncryptedBadge.qml
        qml/Chaft/components/TimelineHeaderRow.qml
        qml/Chaft/components/TimelineOverflowChip.qml
        qml/Chaft/components/TimelineQuickReactionChip.qml
        qml/Chaft/components/TimelineReactionChip.qml
        qml/Chaft/components/TimelineRepairChip.qml
        qml/Chaft/components/TimelineReplyPreview.qml
        qml/Chaft/components/TimelineThreadChip.qml
        qml/Chaft/components/WorkspaceRailItem.qml
        qml/Chaft/features/onboarding/EmptyWorkspaceView.qml
        qml/Chaft/features/onboarding/WorkspaceEntryDialog.qml
        qml/Chaft/features/palette/CommandPalette.qml
        qml/Chaft/features/palette/ShortcutOverlay.qml
        qml/Chaft/features/setup/SetupPanel.qml
        qml/Chaft/features/setup/InvitePeopleDialog.qml
        qml/Chaft/features/setup/ReviewAccessRequestDialog.qml
        qml/Chaft/features/setup/SettingsView.qml
        qml/Chaft/features/setup/DataPortabilityPanel.qml
        qml/Chaft/features/timeline/TimelineView.qml
        qml/Chaft/features/composer/ComposerBar.qml
        qml/Chaft/features/inspector/ChannelMediaPanel.qml
        qml/Chaft/features/inspector/InspectorThreadPanel.qml
)

qt_add_qml_module(ChaftStyleModule
    STATIC
    URI ChaftStyle
    VERSION 1.0
    OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/ChaftStyle"
    QML_FILES
        qml/ChaftStyle/Button.qml
        qml/ChaftStyle/CheckBox.qml
        qml/ChaftStyle/ComboBox.qml
        qml/ChaftStyle/Dialog.qml
        qml/ChaftStyle/Menu.qml
        qml/ChaftStyle/MenuItem.qml
        qml/ChaftStyle/ScrollBar.qml
        qml/ChaftStyle/TextField.qml
        qml/ChaftStyle/ToolTip.qml
)
target_link_libraries(ChaftDesktop PRIVATE ChaftStyleModuleplugin)

qt_add_resources(ChaftDesktop "chaft_desktop_fonts"
    PREFIX "/fonts"
    BASE fonts
    FILES
        fonts/SpaceGrotesk-Regular.ttf
        fonts/SpaceGrotesk-Medium.ttf
        fonts/SpaceGrotesk-Bold.ttf
        fonts/JetBrainsMono-Regular.ttf
        fonts/JetBrainsMono-Medium.ttf
        fonts/JetBrainsMono-SemiBold.ttf
        fonts/OFL-SpaceGrotesk.txt
        fonts/OFL-JetBrainsMono.txt
)

qt_add_resources(ChaftDesktop "chaft_desktop_branding"
    PREFIX "/branding"
    BASE resources/branding
    FILES
        resources/branding/chaft-avatar-dark.png
        resources/branding/chaft-avatar-light.png
        resources/branding/chaft-mark-frame-horizontal.png
        resources/branding/chaft-mark-frame.png
        resources/branding/chaft-mark.png
        resources/branding/chaft-wordmark.png
)

if(APPLE)
    # Qt 6.8 on GitHub macOS runners can scan the source qml/ path as a
    # generated QML module under Chaft. Provide the expected empty module path.
    file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/Chaft/QML")
    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/Chaft/QML/qmldir" "module QML\n")
endif()

target_link_libraries(ChaftDesktop PRIVATE Qt6::Network Qt6::Qml Qt6::Quick Qt6::Widgets)

if(BUILD_TESTING)
    find_package(Python3 COMPONENTS Interpreter QUIET)
    if(Python3_Interpreter_FOUND)
        add_test(
            NAME ChaftDesktopReactivityContract
            COMMAND "${Python3_EXECUTABLE}"
                "${CMAKE_CURRENT_SOURCE_DIR}/tests/check_reactivity_contract.py"
        )
    endif()

    find_program(QMLTESTRUNNER_EXECUTABLE
        NAMES qmltestrunner
    )
    if(QMLTESTRUNNER_EXECUTABLE)
        add_test(
            NAME ChaftTimelineMessageUx
            COMMAND "${QMLTESTRUNNER_EXECUTABLE}"
                -input "${CMAKE_CURRENT_SOURCE_DIR}/tests"
                -import "${CMAKE_CURRENT_SOURCE_DIR}/qml"
        )
        set_tests_properties(ChaftTimelineMessageUx PROPERTIES
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        )
    else()
        message(WARNING "qmltestrunner was not found; desktop QML tests are unavailable")
    endif()
endif()

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
    elseif(UNIX)
        install(FILES "${CHAFT_FFI_LIBRARY_PATH}"
            DESTINATION ${CMAKE_INSTALL_LIBDIR}
        )
    else()
        install(FILES "${CHAFT_FFI_LIBRARY_PATH}"
            DESTINATION ${CMAKE_INSTALL_BINDIR}
        )
    endif()
endif()

if(UNIX AND NOT APPLE)
    install(FILES
        "${CMAKE_SOURCE_DIR}/packaging/linux/io.github.jurshsmith.chaft.desktop"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/applications"
    )
    install(FILES
        "${CMAKE_SOURCE_DIR}/packaging/linux/io.github.jurshsmith.chaft.metainfo.xml"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/metainfo"
    )
    install(FILES
        "${CMAKE_SOURCE_DIR}/packaging/linux/io.github.jurshsmith.chaft.png"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/icons/hicolor/512x512/apps"
    )
endif()

set(CHAFT_PACKAGE_DOCUMENTATION
    "${CMAKE_SOURCE_DIR}/LICENSE"
    "${CMAKE_SOURCE_DIR}/packaging/qt/THIRD_PARTY_NOTICES.txt"
    "${CMAKE_SOURCE_DIR}/packaging/qt/LICENSE.LGPL3"
    "${CMAKE_SOURCE_DIR}/packaging/qt/LICENSE.GPL3"
    "${CMAKE_SOURCE_DIR}/packaging/qt/QT-CORRESPONDING-SOURCE.json"
)
if(APPLE)
    set(CHAFT_PACKAGE_DOCUMENTATION_DESTINATION
        "ChaftDesktop.app/Contents/Resources/doc/Chaft"
    )
else()
    set(CHAFT_PACKAGE_DOCUMENTATION_DESTINATION
        "${CMAKE_INSTALL_DATADIR}/doc/Chaft"
    )
endif()
install(FILES ${CHAFT_PACKAGE_DOCUMENTATION}
    DESTINATION "${CHAFT_PACKAGE_DOCUMENTATION_DESTINATION}"
)

if(APPLE OR WIN32)
    set(CHAFT_DEPLOY_OPTIONS)
    if(WIN32)
        list(APPEND CHAFT_DEPLOY_OPTIONS
            DEPLOY_TOOL_OPTIONS --include-plugins qoffscreen
        )
    endif()
    qt_generate_deploy_qml_app_script(
        TARGET ChaftDesktop
        OUTPUT_SCRIPT chaft_desktop_deploy_script
        ${CHAFT_DEPLOY_OPTIONS}
    )
    install(SCRIPT ${chaft_desktop_deploy_script})
endif()

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
    set(CPACK_PACKAGE_FILE_NAME "Chaft-${PROJECT_VERSION}-Windows-x86_64")
endif()

if(APPLE OR WIN32)
    include(CPack)
endif()
