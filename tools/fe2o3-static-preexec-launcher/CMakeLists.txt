cmake_minimum_required(VERSION 3.20)
project(fe2o3_static_preexec_launcher LANGUAGES C ASM)

option(BUILD_TESTING "Build the static pre-exec launcher tests" ON)

if(CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR "multi-config generators are not supported")
endif()
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux" OR
   NOT CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|AMD64)$")
  message(FATAL_ERROR
    "fe2o3-static-preexec-launcher currently supports Linux x86_64 only")
endif()
if(NOT CMAKE_BUILD_TYPE STREQUAL "Release")
  message(FATAL_ERROR
    "fe2o3-static-preexec-launcher requires CMAKE_BUILD_TYPE=Release")
endif()

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

function(fe2o3_configure_static_launcher target)
  set(FE2O3_STATIC_LAUNCHER_LINKER_SCRIPT
    "${CMAKE_CURRENT_SOURCE_DIR}/src/layout.ld")
  target_include_directories(${target} PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/include")
  target_compile_options(${target} PRIVATE
    $<$<COMPILE_LANGUAGE:C>:-Wall;-Wextra;-Wpedantic;-Werror;-Wconversion;-Wsign-conversion;-Wformat=2;-Wshadow;-Wstrict-prototypes;-ffreestanding;-fno-builtin;-fno-stack-protector;-fno-asynchronous-unwind-tables;-fno-unwind-tables>
    -fno-ident -fno-pie
    "-ffile-prefix-map=${CMAKE_CURRENT_SOURCE_DIR}=fe2o3-static-preexec-source"
    "-ffile-prefix-map=${CMAKE_CURRENT_BINARY_DIR}=fe2o3-static-preexec-build"
    "-fdebug-prefix-map=${CMAKE_CURRENT_SOURCE_DIR}=fe2o3-static-preexec-source"
    "-fdebug-prefix-map=${CMAKE_CURRENT_BINARY_DIR}=fe2o3-static-preexec-build")
  target_link_options(${target} PRIVATE
    -nostdlib -nostartfiles -nodefaultlibs -static -no-pie
    "LINKER:-T,${FE2O3_STATIC_LAUNCHER_LINKER_SCRIPT}"
    LINKER:--entry=_start LINKER:--no-undefined LINKER:--build-id=none
    LINKER:--no-dynamic-linker LINKER:-z,noexecstack LINKER:-z,separate-code)
  set_property(TARGET ${target} APPEND PROPERTY LINK_DEPENDS
    "${FE2O3_STATIC_LAUNCHER_LINKER_SCRIPT}")
endfunction()

add_executable(fe2o3-static-preexec-launcher src/start.S src/main.c)
target_compile_definitions(fe2o3-static-preexec-launcher PRIVATE
  FE2O3_PREEXEC_TEST_ONLY=0)
fe2o3_configure_static_launcher(fe2o3-static-preexec-launcher)

install(TARGETS fe2o3-static-preexec-launcher RUNTIME DESTINATION bin
  PERMISSIONS OWNER_READ OWNER_EXECUTE GROUP_READ GROUP_EXECUTE
              WORLD_READ WORLD_EXECUTE)

if(BUILD_TESTING)
  enable_testing()
  add_executable(fe2o3-static-preexec-launcher-fallback
    src/start.S src/main.c)
  target_compile_definitions(fe2o3-static-preexec-launcher-fallback PRIVATE
    FE2O3_PREEXEC_TEST_ONLY=1
    FE2O3_PREEXEC_TEST_FORCE_CLOSE_FALLBACK=1)
  fe2o3_configure_static_launcher(fe2o3-static-preexec-launcher-fallback)

  function(fe2o3_configure_test_fixture target)
    target_include_directories(${target} PRIVATE
      "${CMAKE_CURRENT_SOURCE_DIR}/include")
    target_compile_options(${target} PRIVATE
      -Wall -Wextra -Wpedantic -Werror -Wconversion -Wsign-conversion
      -Wformat=2 -Wshadow -Wstrict-prototypes)
  endfunction()

  set(FE2O3_HOSTILE_MARKER
    "${CMAKE_CURRENT_BINARY_DIR}/hostile-preload-constructor-ran")
  add_library(fe2o3-hostile-preload MODULE tests/hostile_preload.c)
  target_compile_definitions(fe2o3-hostile-preload PRIVATE
    "FE2O3_HOSTILE_MARKER_PATH=\"${FE2O3_HOSTILE_MARKER}\"")
  fe2o3_configure_test_fixture(fe2o3-hostile-preload)

  add_executable(fe2o3-static-preexec-target-probe tests/target_probe.c)
  fe2o3_configure_test_fixture(fe2o3-static-preexec-target-probe)

  add_executable(fe2o3-static-preexec-parent-death-target
    tests/parent_death_target.c)
  fe2o3_configure_test_fixture(fe2o3-static-preexec-parent-death-target)

  add_executable(fe2o3-static-preexec-supervisor tests/supervisor.c)
  fe2o3_configure_test_fixture(fe2o3-static-preexec-supervisor)

  add_executable(fe2o3-static-preexec-parent-death tests/parent_death.c)
  fe2o3_configure_test_fixture(fe2o3-static-preexec-parent-death)

  add_test(NAME fe2o3-static-preexec-static-elf
    COMMAND /usr/bin/bash
      "${CMAKE_CURRENT_SOURCE_DIR}/tests/check_static_elf.sh"
      "$<TARGET_FILE:fe2o3-static-preexec-launcher>"
      "${CMAKE_CURRENT_BINARY_DIR}/fe2o3-static-preexec.readelf.txt")

  foreach(case IN ITEMS success signal-state alias alias-mutable-metadata
      manifest-executable-alias target-substitution extra-source missing-source
      standard-fd duplicate-destination parent-identity pidfd-class-on-pipe)
    string(REPLACE "-" "_" case_name "${case}")
    add_test(NAME "fe2o3-static-preexec-${case}"
      COMMAND "$<TARGET_FILE:fe2o3-static-preexec-supervisor>"
        "${case}"
        "$<TARGET_FILE:fe2o3-static-preexec-launcher>"
        "$<TARGET_FILE:fe2o3-static-preexec-target-probe>"
        "$<TARGET_FILE:fe2o3-hostile-preload>"
        "${FE2O3_HOSTILE_MARKER}")
    set_tests_properties("fe2o3-static-preexec-${case}" PROPERTIES
      RESOURCE_LOCK hostile-preload-marker)
  endforeach()

  add_test(NAME fe2o3-static-preexec-close-fallback
    COMMAND "$<TARGET_FILE:fe2o3-static-preexec-supervisor>"
      success-low-limit
      "$<TARGET_FILE:fe2o3-static-preexec-launcher-fallback>"
      "$<TARGET_FILE:fe2o3-static-preexec-target-probe>"
      "$<TARGET_FILE:fe2o3-hostile-preload>"
      "${FE2O3_HOSTILE_MARKER}")
  set_tests_properties(fe2o3-static-preexec-close-fallback PROPERTIES
    RESOURCE_LOCK hostile-preload-marker)

  add_test(NAME fe2o3-static-preexec-parent-death
    COMMAND "$<TARGET_FILE:fe2o3-static-preexec-parent-death>"
      "$<TARGET_FILE:fe2o3-static-preexec-launcher>"
      "$<TARGET_FILE:fe2o3-static-preexec-parent-death-target>")

  set(FE2O3_STATIC_PREEXEC_TEST_LABELS
    "security;static-preexec;authority-none")
  # Static binutils inspection can be CPU-starved on saturated hosted runners.
  set_tests_properties(fe2o3-static-preexec-static-elf PROPERTIES
    TIMEOUT 60 LABELS "${FE2O3_STATIC_PREEXEC_TEST_LABELS}")
  set_tests_properties(
    fe2o3-static-preexec-success
    fe2o3-static-preexec-signal-state
    fe2o3-static-preexec-alias
    fe2o3-static-preexec-alias-mutable-metadata
    fe2o3-static-preexec-manifest-executable-alias
    fe2o3-static-preexec-target-substitution
    fe2o3-static-preexec-extra-source
    fe2o3-static-preexec-missing-source
    fe2o3-static-preexec-standard-fd
    fe2o3-static-preexec-duplicate-destination
    fe2o3-static-preexec-parent-identity
    fe2o3-static-preexec-close-fallback
    fe2o3-static-preexec-parent-death
    PROPERTIES TIMEOUT 10 LABELS "${FE2O3_STATIC_PREEXEC_TEST_LABELS}")
endif()
