cmake_minimum_required(VERSION 3.20)
project(malbox-plugin-sdk-cpp-tests LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

list(APPEND CMAKE_PREFIX_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../dist-linux")
find_package(MalboxPluginSDK REQUIRED)

enable_testing()

set(TEST_SOURCES
    test_types.cpp
    test_result.cpp
    test_plugin.cpp
)

foreach(TEST_SRC ${TEST_SOURCES})
    get_filename_component(TEST_NAME ${TEST_SRC} NAME_WE)
    add_executable(${TEST_NAME} ${TEST_SRC})
    target_link_libraries(${TEST_NAME} PRIVATE MalboxPluginSDK::MalboxPluginSDK)
    add_test(NAME ${TEST_NAME} COMMAND ${TEST_NAME})
endforeach()
