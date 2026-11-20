cmake_minimum_required(VERSION 3.28...3.30)

include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/common/bootstrap.cmake" NO_POLICY_SCOPE)

project(${_name} VERSION ${_version})

option(ENABLE_FRONTEND_API "Use obs-frontend-api for UI functionality" OFF)
option(ENABLE_QT "Use Qt functionality" OFF)
option(ENABLE_DEEPFILTER_FFI "Build the bundled DeepFilterNet FFI library" ON)

include(compilerconfig)
include(defaults)
include(helpers)

add_library(${CMAKE_PROJECT_NAME} MODULE)

find_package(libobs REQUIRED)
target_link_libraries(${CMAKE_PROJECT_NAME} PRIVATE OBS::libobs)

if(ENABLE_FRONTEND_API)
  find_package(obs-frontend-api REQUIRED)
  target_link_libraries(${CMAKE_PROJECT_NAME} PRIVATE OBS::obs-frontend-api)
endif()

if(ENABLE_QT)
  find_package(Qt6 COMPONENTS Widgets Core)
  target_link_libraries(${CMAKE_PROJECT_NAME} PRIVATE Qt6::Core Qt6::Widgets)
  target_compile_options(
    ${CMAKE_PROJECT_NAME}
    PRIVATE $<$<C_COMPILER_ID:Clang,AppleClang>:-Wno-quoted-include-in-framework-header -Wno-comma>
  )
  set_target_properties(
    ${CMAKE_PROJECT_NAME}
    PROPERTIES AUTOMOC ON AUTOUIC ON AUTORCC ON
  )
endif()

target_sources(
  ${CMAKE_PROJECT_NAME}
  PRIVATE src/plugin-main.c src/deepfilter-filter.c src/deepfilter-filter.h src/df-loader.c src/df-loader.h
)

if(ENABLE_DEEPFILTER_FFI)
  find_program(CARGO_EXECUTABLE cargo REQUIRED)

  set(DF_FFI_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/third_party/df-ffi")
  set(DF_FFI_MANIFEST "${DF_FFI_SOURCE_DIR}/Cargo.toml")
  set(DF_FFI_TARGET_DIR "${DF_FFI_SOURCE_DIR}/target")
  set(DF_FFI_DEPENDS "${DF_FFI_MANIFEST}" "${DF_FFI_SOURCE_DIR}/src/lib.rs")

  if(EXISTS "${DF_FFI_SOURCE_DIR}/Cargo.lock")
    list(APPEND DF_FFI_DEPENDS "${DF_FFI_SOURCE_DIR}/Cargo.lock")
  endif()

  if(APPLE)
    option(DF_FFI_UNIVERSAL "Build a universal df_ffi library" ON)

    # Xcode injects SDKROOT/ARCHS/CFLAGS into build phases. They can make
    # cargo rebuild proc-macro crates with an incompatible linker setup, so
    # strip them before invoking cargo. Rust's own minimum is macOS 11, which
    # is compatible with the plugin's macOS 13 deployment target.
    set(
      DF_FFI_ENV_COMMAND
      "${CMAKE_COMMAND}"
      -E
      env
      --unset=SDKROOT
      --unset=MACOSX_DEPLOYMENT_TARGET
      --unset=ARCHS
      --unset=CFLAGS
      --unset=CXXFLAGS
      --unset=LDFLAGS
      --unset=CPPFLAGS
    )

    if(DF_FFI_UNIVERSAL)
      set(DF_FFI_LIB "${CMAKE_CURRENT_BINARY_DIR}/libdf_ffi.dylib")
      set(DF_FFI_ARM64 "${DF_FFI_TARGET_DIR}/aarch64-apple-darwin/release/libdf_ffi.dylib")
      set(DF_FFI_X86_64 "${DF_FFI_TARGET_DIR}/x86_64-apple-darwin/release/libdf_ffi.dylib")

      add_custom_command(
        OUTPUT "${DF_FFI_LIB}"
        COMMAND
          ${DF_FFI_ENV_COMMAND} "${CARGO_EXECUTABLE}" build --release --manifest-path "${DF_FFI_MANIFEST}" --target
          aarch64-apple-darwin
        COMMAND
          ${DF_FFI_ENV_COMMAND} "${CARGO_EXECUTABLE}" build --release --manifest-path "${DF_FFI_MANIFEST}" --target
          x86_64-apple-darwin
        COMMAND /usr/bin/lipo -create "${DF_FFI_ARM64}" "${DF_FFI_X86_64}" -output "${DF_FFI_LIB}"
        COMMAND /usr/bin/install_name_tool -id "@loader_path/libdf_ffi.dylib" "${DF_FFI_LIB}"
        DEPENDS ${DF_FFI_DEPENDS}
        WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        COMMENT "Building libdf_ffi.dylib (universal)"
        VERBATIM
      )
    else()
      set(DF_FFI_RAW "${DF_FFI_TARGET_DIR}/release/libdf_ffi.dylib")
      set(DF_FFI_LIB "${CMAKE_CURRENT_BINARY_DIR}/libdf_ffi.dylib")

      add_custom_command(
        OUTPUT "${DF_FFI_LIB}"
        COMMAND ${DF_FFI_ENV_COMMAND} "${CARGO_EXECUTABLE}" build --release --manifest-path "${DF_FFI_MANIFEST}"
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${DF_FFI_RAW}" "${DF_FFI_LIB}"
        COMMAND /usr/bin/install_name_tool -id "@loader_path/libdf_ffi.dylib" "${DF_FFI_LIB}"
        DEPENDS ${DF_FFI_DEPENDS}
        WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        COMMENT "Building libdf_ffi.dylib"
        VERBATIM
      )
    endif()

    target_sources(${CMAKE_PROJECT_NAME} PRIVATE "${DF_FFI_LIB}")
    set_source_files_properties("${DF_FFI_LIB}" PROPERTIES GENERATED TRUE MACOSX_PACKAGE_LOCATION "MacOS")
  elseif(WIN32)
    set(DF_FFI_LIB "${DF_FFI_TARGET_DIR}/release/df_ffi.dll")

    add_custom_command(
      OUTPUT "${DF_FFI_LIB}"
      COMMAND "${CARGO_EXECUTABLE}" build --release --manifest-path "${DF_FFI_MANIFEST}"
      DEPENDS ${DF_FFI_DEPENDS}
      WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
      COMMENT "Building df_ffi.dll"
      VERBATIM
    )

    install(FILES "${DF_FFI_LIB}" DESTINATION "plugins/${CMAKE_PROJECT_NAME}")

    add_custom_command(
      TARGET ${CMAKE_PROJECT_NAME}
      POST_BUILD
      COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${DF_FFI_LIB}" "$<TARGET_FILE_DIR:${CMAKE_PROJECT_NAME}>"
      COMMAND
        "${CMAKE_COMMAND}" -E make_directory
        "${CMAKE_CURRENT_BINARY_DIR}/rundir/$<CONFIG>/plugins/${CMAKE_PROJECT_NAME}"
      COMMAND
        "${CMAKE_COMMAND}" -E copy_if_different "${DF_FFI_LIB}"
        "${CMAKE_CURRENT_BINARY_DIR}/rundir/$<CONFIG>/plugins/${CMAKE_PROJECT_NAME}/"
      COMMENT "Copy df_ffi.dll to the plugin directories"
      VERBATIM
    )
  else()
    set(DF_FFI_LIB "${DF_FFI_TARGET_DIR}/release/libdf_ffi.so")

    add_custom_command(
      OUTPUT "${DF_FFI_LIB}"
      COMMAND "${CARGO_EXECUTABLE}" build --release --manifest-path "${DF_FFI_MANIFEST}"
      DEPENDS ${DF_FFI_DEPENDS}
      WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
      COMMENT "Building libdf_ffi.so"
      VERBATIM
    )

    install(FILES "${DF_FFI_LIB}" DESTINATION ${CMAKE_INSTALL_LIBDIR}/obs-plugins)
  endif()

  add_custom_target(df_ffi DEPENDS "${DF_FFI_LIB}")
  add_dependencies(${CMAKE_PROJECT_NAME} df_ffi)
endif()

set_target_properties_plugin(${CMAKE_PROJECT_NAME} PROPERTIES OUTPUT_NAME ${_name})
