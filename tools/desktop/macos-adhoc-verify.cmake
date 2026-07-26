set(
    CHAFT_MACOS_APP_BUNDLE
    "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/ChaftDesktop.app"
)
if(NOT IS_DIRECTORY "${CHAFT_MACOS_APP_BUNDLE}")
    message(FATAL_ERROR
        "Final macOS app bundle was not installed: ${CHAFT_MACOS_APP_BUNDLE}"
    )
endif()

set(CHAFT_CODESIGN_EXECUTABLE "/usr/bin/codesign")
execute_process(
    COMMAND "/usr/bin/test" -x "${CHAFT_CODESIGN_EXECUTABLE}"
    RESULT_VARIABLE CHAFT_CODESIGN_EXECUTABLE_RESULT
)
if(NOT CHAFT_CODESIGN_EXECUTABLE_RESULT STREQUAL "0")
    message(FATAL_ERROR
        "Final macOS package verification requires executable "
        "${CHAFT_CODESIGN_EXECUTABLE}"
    )
endif()

execute_process(
    COMMAND
        "${CHAFT_CODESIGN_EXECUTABLE}"
        --verify
        --deep
        --strict
        --verbose=4
        "${CHAFT_MACOS_APP_BUNDLE}"
    RESULT_VARIABLE CHAFT_CODESIGN_VERIFY_RESULT
    ERROR_VARIABLE CHAFT_CODESIGN_VERIFY_ERROR
)
if(NOT CHAFT_CODESIGN_VERIFY_RESULT STREQUAL "0")
    message(FATAL_ERROR
        "Final macOS app ad-hoc signature is invalid: "
        "${CHAFT_CODESIGN_VERIFY_ERROR}"
    )
endif()

message(STATUS
    "Verified final macOS ad-hoc app signature: "
    "${CHAFT_MACOS_APP_BUNDLE}"
)
