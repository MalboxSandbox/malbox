# CMake toolchain file for cross-compiling to Windows with MinGW.
#
# Usage:
#   cmake -B build -DCMAKE_TOOLCHAIN_FILE=<path>/mingw-w64-x86_64.cmake \
#         -DCMAKE_PREFIX_PATH=<path>/dist-windows
#
# Requirements:
#   - x86_64-w64-mingw32-g++ must be on PATH
#   - MINGW_LIB_PATH env var: colon-separated library search paths
#   - MINGW_INCLUDE_PATH env var: colon-separated include search paths
#   The devenv.nix sets these automatically.

set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(CMAKE_C_COMPILER   x86_64-w64-mingw32-gcc)
set(CMAKE_CXX_COMPILER x86_64-w64-mingw32-g++)
set(CMAKE_RC_COMPILER  x86_64-w64-mingw32-windres)

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# Add MinGW library search paths from MINGW_LIB_PATH env var.
if(DEFINED ENV{MINGW_LIB_PATH})
    string(REPLACE ":" ";" _mingw_lib_dirs "$ENV{MINGW_LIB_PATH}")
    foreach(_dir ${_mingw_lib_dirs})
        link_directories(${_dir})
    endforeach()
endif()

# Add MinGW include search paths from MINGW_INCLUDE_PATH env var.
if(DEFINED ENV{MINGW_INCLUDE_PATH})
    string(REPLACE ":" ";" _mingw_inc_dirs "$ENV{MINGW_INCLUDE_PATH}")
    foreach(_dir ${_mingw_inc_dirs})
        include_directories(SYSTEM ${_dir})
    endforeach()
endif()
