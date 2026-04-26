cmake_minimum_required(VERSION 3.20)
project(guest-cpp-hello LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(MalboxPluginSDK REQUIRED)

malbox_generate_runtime_config(
    MANIFEST ${CMAKE_CURRENT_SOURCE_DIR}/plugin.toml
    OUTPUT   ${CMAKE_CURRENT_BINARY_DIR}/generated/malbox_runtime_config.hpp
)

add_executable(guest-cpp-hello
    src/main.cpp
    ${MALBOX_GENERATED_HEADERS}
)
target_include_directories(guest-cpp-hello PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/generated)
target_link_libraries(guest-cpp-hello PRIVATE MalboxPluginSDK::MalboxPluginSDK)
