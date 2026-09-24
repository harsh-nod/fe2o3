cmake_minimum_required(VERSION 3.20)
project(fe2o3_physical_entry_abi_mechanism LANGUAGES CXX)
if(NOT IS_ABSOLUTE "${FE2O3_WORKER_SOURCE}" OR
   NOT EXISTS "${FE2O3_WORKER_SOURCE}/include/WorkerPipeline.h")
  message(FATAL_ERROR "FE2O3_WORKER_SOURCE must name the existing worker source")
endif()
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
add_subdirectory("${FE2O3_WORKER_SOURCE}" existing-worker)
file(READ "${CMAKE_CURRENT_BINARY_DIR}/existing-worker/fe2o3-worker-build-id.txt" PROBE_WORKER_BUILD_ID)
string(STRIP "${PROBE_WORKER_BUILD_ID}" PROBE_WORKER_BUILD_ID)
add_executable(physical-entry-abi-candidate PhysicalEntryAbiCandidate.cpp)
target_compile_features(physical-entry-abi-candidate PRIVATE cxx_std_20)
target_include_directories(physical-entry-abi-candidate PRIVATE "${FE2O3_WORKER_SOURCE}/tests")
target_compile_definitions(physical-entry-abi-candidate PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_EXPECTED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${PROBE_WORKER_BUILD_ID}")
target_compile_options(physical-entry-abi-candidate PRIVATE
  -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(physical-entry-abi-candidate PRIVATE fe2o3_worker_pipeline)
