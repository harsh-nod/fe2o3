cmake_minimum_required(VERSION 3.20)
project(fe2o3_llvm_link_worker LANGUAGES CXX)

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

if(NOT LLVM_PACKAGE_VERSION STREQUAL FE2O3_PINNED_LLVM_VERSION)
  message(FATAL_ERROR
    "LLVM version mismatch: found '${LLVM_PACKAGE_VERSION}', expected "
    "'${FE2O3_PINNED_LLVM_VERSION}'")
endif()
file(READ "${FE2O3_LLVM_BUILD_ID_FILE}" FE2O3_MEASURED_LLVM_BUILD_ID)
string(STRIP "${FE2O3_MEASURED_LLVM_BUILD_ID}" FE2O3_MEASURED_LLVM_BUILD_ID)
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

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

llvm_map_components_to_libnames(FE2O3_LLVM_LIBS
  Core BitReader Linker Passes Target CodeGen Object Support
  AMDGPUCodeGen AMDGPUAsmParser AMDGPUDesc AMDGPUInfo AMDGPUUtils)

set(FE2O3_WORKER_MEASUREMENT_PREIMAGE
  "llvm=${FE2O3_MEASURED_LLVM_BUILD_ID}|version=${LLVM_PACKAGE_VERSION}"
  "|compiler=${CMAKE_CXX_COMPILER_ID}-${CMAKE_CXX_COMPILER_VERSION}"
  "|cxx=20|flags=-fno-exceptions,-fno-rtti")
foreach(FE2O3_WORKER_SOURCE IN ITEMS
    include/WorkerProtocol.h include/WorkerPipeline.h
    src/WorkerProtocol.cpp src/WorkerPipeline.cpp src/main.cpp)
  file(SHA256 "${CMAKE_CURRENT_SOURCE_DIR}/${FE2O3_WORKER_SOURCE}"
    FE2O3_WORKER_SOURCE_SHA256)
  string(APPEND FE2O3_WORKER_MEASUREMENT_PREIMAGE
    "|${FE2O3_WORKER_SOURCE}=${FE2O3_WORKER_SOURCE_SHA256}")
endforeach()
string(SHA256 FE2O3_WORKER_MEASUREMENT_SHA256
  "${FE2O3_WORKER_MEASUREMENT_PREIMAGE}")
set(FE2O3_WORKER_MEASUREMENT
  "fe2o3-worker-v1-sha256-${FE2O3_WORKER_MEASUREMENT_SHA256}")
message(STATUS "fe2o3 worker measurement: ${FE2O3_WORKER_MEASUREMENT}")

add_library(fe2o3_worker_protocol STATIC src/WorkerProtocol.cpp)
target_include_directories(fe2o3_worker_protocol
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include"
  SYSTEM PRIVATE ${LLVM_INCLUDE_DIRS})
target_compile_definitions(fe2o3_worker_protocol PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
target_compile_options(fe2o3_worker_protocol PRIVATE -Wall -Wextra -Wpedantic
  -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(fe2o3_worker_protocol PUBLIC LLVMSupport)

add_library(fe2o3_worker_pipeline STATIC src/WorkerPipeline.cpp)
target_include_directories(fe2o3_worker_pipeline
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include"
  SYSTEM PRIVATE ${LLVM_INCLUDE_DIRS} ${LLD_INCLUDE_DIRS})
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
target_link_libraries(fe2o3-llvm-link-worker PRIVATE fe2o3_worker_pipeline)

include(CTest)
if(BUILD_TESTING)
  add_executable(fe2o3-worker-codec-tests tests/CodecTests.cpp)
  target_compile_definitions(fe2o3-worker-codec-tests PRIVATE
    FE2O3_LLVM_BUILD_ID="${FE2O3_MEASURED_LLVM_BUILD_ID}"
    FE2O3_WORKER_BUILD_ID="${FE2O3_WORKER_MEASUREMENT}")
  target_compile_options(fe2o3-worker-codec-tests PRIVATE -Wall -Wextra
    -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
  target_link_libraries(fe2o3-worker-codec-tests PRIVATE fe2o3_worker_protocol)
  add_test(NAME fe2o3-worker-codec-tests COMMAND fe2o3-worker-codec-tests)
endif()
