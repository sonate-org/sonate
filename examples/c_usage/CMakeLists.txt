cmake_minimum_required(VERSION 3.16)

project(c_usage C)

add_executable(c_usage c_usage.c)

set(LOLITE_ROOT "${CMAKE_CURRENT_LIST_DIR}/../..")
set(LOLITE_INCLUDE_DIR "${LOLITE_ROOT}/crates/lolite_lib/include")
set(LOLITE_LIB_DIR "${LOLITE_ROOT}/target/release")

target_include_directories(c_usage PRIVATE
    "${LOLITE_INCLUDE_DIR}"
)

# Link against the import library produced by Cargo (MSVC).
# This requires that you've built the DLL first (e.g. `cargo build --release` at repo root).
target_link_libraries(c_usage PRIVATE "${LOLITE_LIB_DIR}/lolite.dll.lib")

# On Windows, ensure the runtime DLL is next to the built executable so it can run
# directly from the build output directory.
if(WIN32)
    add_custom_command(TARGET c_usage POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            "${LOLITE_LIB_DIR}/lolite.dll"
            "$<TARGET_FILE_DIR:c_usage>"
        VERBATIM
    )
endif()
