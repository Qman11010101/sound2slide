add_library(marmkmt_c_tap_plugin SHARED
  c_tap_plugin.c
  c_tap_plugin.def)

target_include_directories(marmkmt_c_tap_plugin PRIVATE
  ${PROJECT_SOURCE_DIR}/include)
target_compile_features(marmkmt_c_tap_plugin PRIVATE c_std_11)
target_compile_options(marmkmt_c_tap_plugin PRIVATE /W4 /utf-8)
target_link_libraries(marmkmt_c_tap_plugin PRIVATE marmkmt)

# marmkmt is a C++ static library, while the example implementation itself is C.
set_target_properties(marmkmt_c_tap_plugin PROPERTIES
  LINKER_LANGUAGE CXX
  OUTPUT_NAME c_tap_plugin)

# Retain the bridge object even though its Margrete entry points are not
# referenced by the C implementation directly.
target_link_options(marmkmt_c_tap_plugin PRIVATE
  "/WHOLEARCHIVE:$<TARGET_FILE:marmkmt>")
