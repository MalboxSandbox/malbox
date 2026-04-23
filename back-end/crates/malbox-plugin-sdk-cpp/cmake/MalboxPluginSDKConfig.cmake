include(CMakeFindDependencyMacro)
include("${CMAKE_CURRENT_LIST_DIR}/MalboxPluginSDKTargets.cmake")

# Locate the malbox-codegen binary shipped with this SDK.
#
# Precedence: 1) user override via `-DMALBOX_CODEGEN_EXECUTABLE=...`,
#             2) `bin/malbox-codegen[.exe]` relative to this cmake file.
if(NOT DEFINED MALBOX_CODEGEN_EXECUTABLE)
    find_program(MALBOX_CODEGEN_EXECUTABLE
        NAMES malbox-codegen malbox-codegen.exe
        HINTS "${CMAKE_CURRENT_LIST_DIR}/../bin"
              "${CMAKE_CURRENT_LIST_DIR}/../../bin"
        NO_DEFAULT_PATH
    )
endif()
if(NOT MALBOX_CODEGEN_EXECUTABLE)
    message(FATAL_ERROR
        "MalboxPluginSDK: could not locate malbox-codegen. "
        "Install the SDK with its bin/ directory, or pass "
        "-DMALBOX_CODEGEN_EXECUTABLE=/path/to/malbox-codegen.")
endif()

# Generate <malbox_runtime_config.hpp> from plugin.toml at configure/build time.
#
# Usage:
#   malbox_generate_runtime_config(
#       MANIFEST ${CMAKE_CURRENT_SOURCE_DIR}/plugin.toml
#       OUTPUT   ${CMAKE_CURRENT_BINARY_DIR}/generated/malbox_runtime_config.hpp
#   )
#   target_sources(<my-plugin> PRIVATE ${MALBOX_GENERATED_HEADERS})
function(malbox_generate_runtime_config)
    cmake_parse_arguments(ARG "" "MANIFEST;OUTPUT" "" ${ARGN})
    if(NOT ARG_MANIFEST OR NOT ARG_OUTPUT)
        message(FATAL_ERROR
            "malbox_generate_runtime_config: MANIFEST and OUTPUT are required")
    endif()
    add_custom_command(
        OUTPUT  "${ARG_OUTPUT}"
        COMMAND "${MALBOX_CODEGEN_EXECUTABLE}"
                --manifest "${ARG_MANIFEST}"
                --lang cpp
                --output "${ARG_OUTPUT}"
        DEPENDS "${ARG_MANIFEST}"
        VERBATIM
        COMMENT "Generating ${ARG_OUTPUT} from ${ARG_MANIFEST}"
    )
    set(MALBOX_GENERATED_HEADERS "${ARG_OUTPUT}" PARENT_SCOPE)
endfunction()
