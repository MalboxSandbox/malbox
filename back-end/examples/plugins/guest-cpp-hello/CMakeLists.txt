cmake_minimum_required(VERSION 3.20)
project(guest-cpp-hello LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(MalboxPluginSDK REQUIRED)

add_executable(guest-cpp-hello src/main.cpp)
target_link_libraries(guest-cpp-hello PRIVATE MalboxPluginSDK::MalboxPluginSDK)
