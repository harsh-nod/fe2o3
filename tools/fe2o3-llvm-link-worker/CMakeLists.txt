cmake_minimum_required(VERSION 3.20)
project(fe2o3_llvm_link_worker LANGUAGES CXX)

option(FE2O3_ROW_SOFTMAX_RELEASE_GATE
  "Require all measured row-softmax V1 release prerequisites" OFF)

if(NOT DEFINED LLVM_DIR OR LLVM_DIR STREQUAL "")
  message(FATAL_ERROR "LLVM_DIR must name the explicit pinned LLVM CMake package")
endif()
if(NOT DEFINED LLD_DIR OR LLD_DIR STREQUAL "")
  message(FATAL_ERROR "LLD_DIR must name the matching explicit LLD CMake package")
endif()
if(NOT DEFINED FE2O3_PINNED_LLVM_VERSION OR
   FE2O3_PINNED_LLVM_VERSION STREQUAL "")
  message(FATAL_ERROR "FE2O3_PINNED_LLVM_VERSION is required")
endif()
if(NOT DEFINED FE2O3_EXPECTED_LLVM_BUILD_ID OR
   FE2O3_EXPECTED_LLVM_BUILD_ID STREQUAL "")
  message(FATAL_ERROR "FE2O3_EXPECTED_LLVM_BUILD_ID is required")
endif()
if(NOT DEFINED FE2O3_LLVM_BUILD_ID_FILE OR
   FE2O3_LLVM_BUILD_ID_FILE STREQUAL "")
  message(FATAL_ERROR "FE2O3_LLVM_BUILD_ID_FILE is required")
endif()
if(NOT EXISTS "${FE2O3_LLVM_BUILD_ID_FILE}")
  message(FATAL_ERROR "FE2O3_LLVM_BUILD_ID_FILE does not exist")
endif()

find_package(LLVM REQUIRED CONFIG NO_DEFAULT_PATH PATHS "${LLVM_DIR}")
find_package(LLD REQUIRED CONFIG NO_DEFAULT_PATH PATHS "${LLD_DIR}")
find_package(Threads REQUIRED)

file(REAL_PATH "${LLVM_DIR}" FE2O3_REAL_LLVM_DIR)
file(REAL_PATH "${LLD_DIR}" FE2O3_REAL_LLD_DIR)
get_filename_component(FE2O3_LLVM_CMAKE_ROOT "${FE2O3_REAL_LLVM_DIR}"
  DIRECTORY)
get_filename_component(FE2O3_LLD_CMAKE_ROOT "${FE2O3_REAL_LLD_DIR}"
  DIRECTORY)
if(NOT FE2O3_LLVM_CMAKE_ROOT STREQUAL FE2O3_LLD_CMAKE_ROOT)
  message(FATAL_ERROR
    "LLVM_DIR and LLD_DIR must come from one CMake package tree")
endif()

if(NOT LLVM_PACKAGE_VERSION STREQUAL FE2O3_PINNED_LLVM_VERSION)
  message(FATAL_ERROR
    "LLVM version mismatch: found '${LLVM_PACKAGE_VERSION}', expected "
    "'${FE2O3_PINNED_LLVM_VERSION}'")
endif()
file(READ "${FE2O3_LLVM_BUILD_ID_FILE}" FE2O3_MEASURED_LLVM_BUILD_ID)
string(STRIP "${FE2O3_MEASURED_LLVM_BUILD_ID}" FE2O3_MEASURED_LLVM_BUILD_ID)
string(LENGTH "${FE2O3_MEASURED_LLVM_BUILD_ID}"
  FE2O3_MEASURED_LLVM_BUILD_ID_LENGTH)
if(FE2O3_MEASURED_LLVM_BUILD_ID_LENGTH GREATER 160 OR
   NOT FE2O3_MEASURED_LLVM_BUILD_ID MATCHES
     "^[A-Za-z0-9][A-Za-z0-9._+:=~-]*$")
  message(FATAL_ERROR "LLVM build-ID is empty or noncanonical")
endif()
if(NOT FE2O3_MEASURED_LLVM_BUILD_ID STREQUAL FE2O3_EXPECTED_LLVM_BUILD_ID)
  message(FATAL_ERROR
    "LLVM build-ID mismatch: measured '${FE2O3_MEASURED_LLVM_BUILD_ID}', "
    "expected '${FE2O3_EXPECTED_LLVM_BUILD_ID}'")
endif()
if(NOT "AMDGPU" IN_LIST LLVM_TARGETS_TO_BUILD)
  message(FATAL_ERROR "the pinned LLVM build does not contain the AMDGPU target")
endif()
if(NOT TARGET lldELF OR NOT TARGET lldCommon)
  message(FATAL_ERROR "the explicit LLD package does not export lldELF/lldCommon")
endif()
if(CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR
    "multi-config generators cannot produce one measured worker identity")
endif()

set(FE2O3_GFX942_DEVICE_LIB_DIR "/opt/rocm/amdgcn/bitcode" CACHE PATH
  "Directory containing the reviewed ROCm gfx942 device-library bitcode")
set(FE2O3_GFX942_DEVICE_LIB_FILES
  ocml.bc oclc_isa_version_942.bc oclc_unsafe_math_off.bc
  oclc_finite_only_off.bc)
set(FE2O3_GFX942_DEVICE_LIBS_ENABLED 1)
foreach(FE2O3_DEVICE_LIB_FILE IN LISTS FE2O3_GFX942_DEVICE_LIB_FILES)
  if(NOT EXISTS
     "${FE2O3_GFX942_DEVICE_LIB_DIR}/${FE2O3_DEVICE_LIB_FILE}")
    set(FE2O3_GFX942_DEVICE_LIBS_ENABLED 0)
  endif()
endforeach()
if(FE2O3_GFX942_DEVICE_LIBS_ENABLED)
  file(REAL_PATH "${FE2O3_GFX942_DEVICE_LIB_DIR}"
    FE2O3_GFX942_DEVICE_LIB_DIR)
  file(SHA256 "${FE2O3_GFX942_DEVICE_LIB_DIR}/ocml.bc"
    FE2O3_GFX942_OCML_SHA256)
  file(SHA256
    "${FE2O3_GFX942_DEVICE_LIB_DIR}/oclc_isa_version_942.bc"
    FE2O3_GFX942_ISA_SHA256)
  file(SHA256
    "${FE2O3_GFX942_DEVICE_LIB_DIR}/oclc_unsafe_math_off.bc"
    FE2O3_GFX942_UNSAFE_MATH_SHA256)
  file(SHA256
    "${FE2O3_GFX942_DEVICE_LIB_DIR}/oclc_finite_only_off.bc"
    FE2O3_GFX942_FINITE_ONLY_SHA256)
  message(STATUS
    "gfx942 OCML provider enabled from ${FE2O3_GFX942_DEVICE_LIB_DIR}")
else()
  set(FE2O3_GFX942_OCML_SHA256 "disabled")
  set(FE2O3_GFX942_ISA_SHA256 "disabled")
  set(FE2O3_GFX942_UNSAFE_MATH_SHA256 "disabled")
  set(FE2O3_GFX942_FINITE_ONLY_SHA256 "disabled")
  message(STATUS "gfx942 OCML provider disabled: reviewed bitcode is absent")
endif()
if(FE2O3_ROW_SOFTMAX_RELEASE_GATE AND
   NOT FE2O3_GFX942_DEVICE_LIBS_ENABLED)
  message(FATAL_ERROR
    "row-softmax release gate requires the measured gfx942 OCML closure")
endif()
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/fe2o3-llvm-build-id.txt"
  "${FE2O3_MEASURED_LLVM_BUILD_ID}\n")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/fe2o3-gfx942-ocml-provider.txt"
  "enabled=${FE2O3_GFX942_DEVICE_LIBS_ENABLED}\n"
  "directory=${FE2O3_GFX942_DEVICE_LIB_DIR}\n"
  "ocml=${FE2O3_GFX942_OCML_SHA256}\n"
  "isa942=${FE2O3_GFX942_ISA_SHA256}\n"
  "unsafe_math_off=${FE2O3_GFX942_UNSAFE_MATH_SHA256}\n"
  "finite_only_off=${FE2O3_GFX942_FINITE_ONLY_SHA256}\n")
string(REPLACE "\\" "\\\\" FE2O3_GFX942_DEVICE_LIB_DIR_ESCAPED
  "${FE2O3_GFX942_DEVICE_LIB_DIR}")
string(REPLACE "\"" "\\\"" FE2O3_GFX942_DEVICE_LIB_DIR_ESCAPED
  "${FE2O3_GFX942_DEVICE_LIB_DIR_ESCAPED}")
configure_file(include/WorkerBuildConfig.h.in
  "${CMAKE_CURRENT_BINARY_DIR}/generated/WorkerBuildConfig.h" @ONLY)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(TARGET LLVM)
  # A package built with LLVM_LINK_LLVM_DYLIB exposes LLD against the monolithic
  # LLVM target. Mixing component archives into that graph registers LLVM
  # command-line options twice and aborts before main().
  set(FE2O3_LLVM_LIBS LLVM)
  set(FE2O3_LLVM_PROTOCOL_LIBS LLVM)
  set(FE2O3_LLVM_TEST_LIBS LLVM)
else()
  llvm_map_components_to_libnames(FE2O3_LLVM_LIBS
    Core BitReader Linker Passes Target CodeGen Object Support TransformUtils
    MC MCDisassembler AMDGPUCodeGen AMDGPUAsmParser AMDGPUDesc AMDGPUInfo
    AMDGPUUtils)
  llvm_map_components_to_libnames(FE2O3_LLVM_PROTOCOL_LIBS Support)
  llvm_map_components_to_libnames(FE2O3_LLVM_TEST_LIBS BitWriter)
endif()

set(FE2O3_WORKER_MEASUREMENT_PREIMAGE
  "llvm=${FE2O3_MEASURED_LLVM_BUILD_ID}|version=${LLVM_PACKAGE_VERSION}"
  "|compiler=${CMAKE_CXX_COMPILER}"
  "|compiler_id=${CMAKE_CXX_COMPILER_ID}-${CMAKE_CXX_COMPILER_VERSION}"
  "|build_type=${CMAKE_BUILD_TYPE}|cxx_flags=${CMAKE_CXX_FLAGS}"
  "|cxx_flags_debug=${CMAKE_CXX_FLAGS_DEBUG}"
  "|cxx_flags_release=${CMAKE_CXX_FLAGS_RELEASE}"
  "|cxx_flags_relwithdebinfo=${CMAKE_CXX_FLAGS_RELWITHDEBINFO}"
  "|cxx_flags_minsizerel=${CMAKE_CXX_FLAGS_MINSIZEREL}"
  "|exe_linker_flags=${CMAKE_EXE_LINKER_FLAGS}"
  "|ipo=${CMAKE_INTERPROCEDURAL_OPTIMIZATION}"
  "|pic=${CMAKE_POSITION_INDEPENDENT_CODE}"
  "|gfx942_device_libs=${FE2O3_GFX942_DEVICE_LIBS_ENABLED}"
  "|ocml=${FE2O3_GFX942_OCML_SHA256}"
  "|isa942=${FE2O3_GFX942_ISA_SHA256}"
  "|unsafe_math_off=${FE2O3_GFX942_UNSAFE_MATH_SHA256}"
  "|finite_only_off=${FE2O3_GFX942_FINITE_ONLY_SHA256}"
  "|cxx=20|target_flags=-fno-exceptions,-fno-rtti")
foreach(FE2O3_WORKER_SOURCE IN ITEMS
    CMakeLists.txt include/WorkerProtocol.h include/WorkerPipeline.h
    include/WorkerDeviceLibraryPolicy.h include/WorkerBuildConfig.h.in
    include/WorkerLldPolicy.h include/WorkerMachineEffect.h
    src/WorkerProtocol.cpp src/WorkerPipeline.cpp
    src/WorkerDeviceLibraryPolicy.cpp src/WorkerMachineEffect.cpp src/main.cpp)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/${FE2O3_WORKER_SOURCE}")
  file(SHA256 "${CMAKE_CURRENT_SOURCE_DIR}/${FE2O3_WORKER_SOURCE}"
    FE2O3_WORKER_SOURCE_SHA256)
  string(APPEND FE2O3_WORKER_MEASUREMENT_PREIMAGE
    "|${FE2O3_WORKER_SOURCE}=${FE2O3_WORKER_SOURCE_SHA256}")
endforeach()
string(SHA256 FE2O3_WORKER_MEASUREMENT_SHA256
  "${FE2O3_WORKER_MEASUREMENT_PREIMAGE}")
set(FE2O3_WORKER_MEASUREMENT
  "fe2o3-worker-v1-sha256-${FE2O3_WORKER_MEASUREMENT_SHA256}")
message(STATUS "fe2o3 worker asserted build claim: ${FE2O3_WORKER_MEASUREMENT}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/fe2o3-worker-build-id.txt"
  "${FE2O3_WORKER_MEASUREMENT}\n")

add_library(fe2o3_worker_protocol STATIC src/WorkerProtocol.cpp)
target_include_directories(fe2o3_worker_protocol
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_include_directories(fe2o3_worker_protocol SYSTEM PUBLIC
  ${LLVM_INCLUDE_DIRS})
target_compile_definitions(fe2o3_worker_protocol PUBLIC
  FE2O3_PINNED_LLVM_VERSION_MAJOR=${LLVM_VERSION_MAJOR})
target_compile_definitions(fe2o3_worker_protocol PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
target_compile_options(fe2o3_worker_protocol PRIVATE -Wall -Wextra -Wpedantic
  -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(fe2o3_worker_protocol PUBLIC
  ${FE2O3_LLVM_PROTOCOL_LIBS})

add_library(fe2o3_worker_pipeline STATIC src/WorkerPipeline.cpp
  src/WorkerDeviceLibraryPolicy.cpp src/WorkerMachineEffect.cpp)
target_include_directories(fe2o3_worker_pipeline
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_include_directories(fe2o3_worker_pipeline SYSTEM PRIVATE
  ${LLVM_INCLUDE_DIRS} ${LLD_INCLUDE_DIRS})
target_include_directories(fe2o3_worker_pipeline PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/generated")
target_compile_definitions(fe2o3_worker_pipeline PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
target_compile_options(fe2o3_worker_pipeline PRIVATE -Wall -Wextra -Wpedantic
  -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(fe2o3_worker_pipeline PUBLIC fe2o3_worker_protocol
  lldELF lldCommon ${FE2O3_LLVM_LIBS})

add_executable(fe2o3-llvm-link-worker src/main.cpp)
target_compile_definitions(fe2o3-llvm-link-worker PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
target_compile_options(fe2o3-llvm-link-worker PRIVATE -Wall -Wextra -Wpedantic
  -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(fe2o3-llvm-link-worker PRIVATE fe2o3_worker_pipeline
  Threads::Threads)

include(CTest)
if(FE2O3_ROW_SOFTMAX_RELEASE_GATE AND NOT BUILD_TESTING)
  message(FATAL_ERROR "row-softmax release gate requires BUILD_TESTING=ON")
endif()
if(BUILD_TESTING)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/tests/fixtures/row-softmax-v1-llvm22-body.ll")
  add_executable(fe2o3-worker-codec-tests tests/CodecTests.cpp)
  target_compile_definitions(fe2o3-worker-codec-tests PRIVATE
    FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
    FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
  target_compile_options(fe2o3-worker-codec-tests PRIVATE -Wall -Wextra
    -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
  target_link_libraries(fe2o3-worker-codec-tests PRIVATE fe2o3_worker_protocol)
  add_test(NAME fe2o3-worker-codec-tests COMMAND fe2o3-worker-codec-tests)

  add_executable(fe2o3-worker-pipeline-tests tests/PipelineTests.cpp)
  target_include_directories(fe2o3-worker-pipeline-tests SYSTEM PRIVATE
    ${LLVM_INCLUDE_DIRS} ${LLD_INCLUDE_DIRS})
  target_compile_definitions(fe2o3-worker-pipeline-tests PRIVATE
    FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
    FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
  target_compile_options(fe2o3-worker-pipeline-tests PRIVATE -Wall -Wextra
    -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
  target_link_libraries(fe2o3-worker-pipeline-tests PRIVATE
    fe2o3_worker_pipeline ${FE2O3_LLVM_TEST_LIBS})
  add_test(NAME fe2o3-worker-pipeline-tests
    COMMAND fe2o3-worker-pipeline-tests)
  add_test(NAME fe2o3-worker-exact-row-softmax-v1-tests
    COMMAND fe2o3-worker-pipeline-tests --exact-row-softmax-only)

  add_executable(fe2o3-row-softmax-llvm22-layout-probe
    "${CMAKE_CURRENT_SOURCE_DIR}/../../crates/rustc-codegen-fe2o3/tests/fixtures/row-softmax-llvm22-target-layout.cpp")
  target_include_directories(fe2o3-row-softmax-llvm22-layout-probe SYSTEM
    PRIVATE ${LLVM_INCLUDE_DIRS})
  target_compile_definitions(fe2o3-row-softmax-llvm22-layout-probe PRIVATE
    FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}")
  target_compile_options(fe2o3-row-softmax-llvm22-layout-probe PRIVATE
    -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter
    -fno-exceptions -fno-rtti)
  target_link_libraries(fe2o3-row-softmax-llvm22-layout-probe PRIVATE
    ${FE2O3_LLVM_LIBS})
  add_test(NAME fe2o3-row-softmax-llvm22-layout-probe
    COMMAND fe2o3-row-softmax-llvm22-layout-probe)

  add_executable(fe2o3-worker-device-library-policy-tests
    tests/DeviceLibraryPolicyTests.cpp)
  add_executable(fe2o3-worker-machine-effect-tests
    tests/MachineEffectTests.cpp)
  target_include_directories(fe2o3-worker-machine-effect-tests SYSTEM PRIVATE
    ${LLVM_INCLUDE_DIRS} ${LLD_INCLUDE_DIRS})
  target_compile_definitions(fe2o3-worker-machine-effect-tests PRIVATE
    FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
    FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
  target_compile_options(fe2o3-worker-machine-effect-tests PRIVATE -Wall
    -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions
    -fno-rtti)
  target_link_libraries(fe2o3-worker-machine-effect-tests PRIVATE
    fe2o3_worker_pipeline ${FE2O3_LLVM_TEST_LIBS})
  add_test(NAME fe2o3-worker-machine-effect-tests
    COMMAND fe2o3-worker-machine-effect-tests)

  target_include_directories(fe2o3-worker-device-library-policy-tests SYSTEM
    PRIVATE ${LLVM_INCLUDE_DIRS})
  target_compile_options(fe2o3-worker-device-library-policy-tests PRIVATE
    -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter
    -fno-exceptions -fno-rtti)
  target_link_libraries(fe2o3-worker-device-library-policy-tests PRIVATE
    fe2o3_worker_pipeline ${FE2O3_LLVM_TEST_LIBS})
  add_test(NAME fe2o3-worker-device-library-policy-tests
    COMMAND fe2o3-worker-device-library-policy-tests)
endif()
