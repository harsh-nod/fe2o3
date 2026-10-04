cmake_minimum_required(VERSION 3.20)
project(fe2o3_private_composition_transport LANGUAGES CXX)
if(NOT IS_ABSOLUTE "${FE2O3_WORKER_SOURCE}" OR
   NOT EXISTS "${FE2O3_WORKER_SOURCE}/src/WorkerMachineEffect.cpp")
  message(FATAL_ERROR "FE2O3_WORKER_SOURCE must name the reviewed existing worker")
endif()
file(REAL_PATH "${LLVM_DIR}" TRANSPORT_LLVM_DIR)
if(NOT TRANSPORT_LLVM_DIR STREQUAL
   "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/native-sdk-r2/opt/rocm-7.2.1/lib/llvm/lib/cmake/llvm")
  message(FATAL_ERROR "Only the exact reviewed private SDK is supported")
endif()
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
add_subdirectory("${FE2O3_WORKER_SOURCE}" existing-worker)
file(READ "${CMAKE_CURRENT_BINARY_DIR}/existing-worker/fe2o3-worker-build-id.txt"
     TRANSPORT_WORKER_BUILD_ID)
string(STRIP "${TRANSPORT_WORKER_BUILD_ID}" TRANSPORT_WORKER_BUILD_ID)
add_executable(composition-transport-controls TransportTests.cpp)
target_compile_features(composition-transport-controls PRIVATE cxx_std_20)
target_compile_definitions(composition-transport-controls PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_EXPECTED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${TRANSPORT_WORKER_BUILD_ID}")
target_compile_options(composition-transport-controls PRIVATE
  -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(composition-transport-controls PRIVATE fe2o3_worker_pipeline)
# Only this separate test TU includes the analyzer with private hooks enabled.
# The ordinary pipeline archive and worker target retain their default route.
