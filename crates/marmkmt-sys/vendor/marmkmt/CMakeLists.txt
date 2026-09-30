cmake_minimum_required(VERSION 3.24)

# Margrete plugins are distributed as standalone DLLs. Match the official
# MargretePluginSample and statically link the MSVC runtime so loading a plugin
# does not depend on a separately installed VC++ Redistributable. Callers can
# still override this choice with -DCMAKE_MSVC_RUNTIME_LIBRARY=...
if(NOT DEFINED CMAKE_MSVC_RUNTIME_LIBRARY)
  set(CMAKE_MSVC_RUNTIME_LIBRARY
      "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

project(marmkmt VERSION 0.1.0 LANGUAGES C CXX)

if(NOT WIN32 OR NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
  message(FATAL_ERROR "marmkmt supports only x64 Windows")
endif()

set(MARGRETE_PLUGIN_SDK_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/third_party/MargretePluginSDK")
if(NOT EXISTS "${MARGRETE_PLUGIN_SDK_DIR}/include/MargretePlugin.h")
  message(FATAL_ERROR
    "MargretePluginSDK is missing. Initialize it with: "
    "git submodule update --init --recursive")
endif()

add_library(marmkmt STATIC src/marmkmt.cpp)
target_compile_features(marmkmt PUBLIC cxx_std_17)
target_include_directories(marmkmt
  PUBLIC $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
  PRIVATE ${MARGRETE_PLUGIN_SDK_DIR}/include)
target_compile_definitions(marmkmt PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(marmkmt PRIVATE /W4 /permissive- /EHsc /utf-8)
set_target_properties(marmkmt PROPERTIES OUTPUT_NAME marmkmt)

option(MARMKMT_BUILD_EXAMPLES "Build marmkmt examples" ON)
if(MARMKMT_BUILD_EXAMPLES)
  add_subdirectory(examples)
endif()

include(CTest)
if(BUILD_TESTING)
  add_executable(marmkmt_tests tests/marmkmt_tests.cpp)
  target_compile_features(marmkmt_tests PRIVATE cxx_std_17)
  target_include_directories(marmkmt_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${MARGRETE_PLUGIN_SDK_DIR}/include)
  target_compile_definitions(marmkmt_tests PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
  target_compile_options(marmkmt_tests PRIVATE /W4 /permissive- /EHsc /utf-8)
  target_link_libraries(marmkmt_tests PRIVATE marmkmt)
  add_test(NAME marmkmt_tests COMMAND marmkmt_tests)

  add_executable(marmkmt_c_header_test tests/c_header_test.c)
  target_include_directories(marmkmt_c_header_test PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/include)
  target_compile_options(marmkmt_c_header_test PRIVATE /W4 /utf-8)
  add_test(NAME marmkmt_c_header_test COMMAND marmkmt_c_header_test)
endif()

install(TARGETS marmkmt ARCHIVE DESTINATION lib)
install(FILES include/marmkmt.h DESTINATION include)
install(FILES LICENSE.md THIRD_PARTY_NOTICES.md DESTINATION .)
install(FILES "${MARGRETE_PLUGIN_SDK_DIR}/LICENSE.md"
        DESTINATION . RENAME MargretePluginSDK-LICENSE.md)
