cmake_minimum_required(VERSION 3.20)
project(fe2o3_private_composition_transport_native LANGUAGES CXX)
if(NOT IS_ABSOLUTE "${FE2O3_WORKER_SOURCE}" OR
   NOT EXISTS "${FE2O3_WORKER_SOURCE}/tests/ordered-composition-transport/TransportTestV1.h")
  message(FATAL_ERROR "Reviewed typed MC donor must be integrated in the existing worker")
endif()
file(REAL_PATH "${LLVM_DIR}" TRANSPORT_LLVM_DIR)
if(NOT TRANSPORT_LLVM_DIR STREQUAL
   "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/native-sdk-r2/opt/rocm-7.2.1/lib/llvm/lib/cmake/llvm")
  message(FATAL_ERROR "Only the exact reviewed private SDK is supported")
endif()
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
add_subdirectory("${FE2O3_WORKER_SOURCE}" existing-worker)
file(READ "${CMAKE_CURRENT_BINARY_DIR}/existing-worker/fe2o3-worker-build-id.txt" TRANSPORT_WORKER_BUILD_ID)
string(STRIP "${TRANSPORT_WORKER_BUILD_ID}" TRANSPORT_WORKER_BUILD_ID)
foreach(NAME IN ITEMS composition-transport-native-observer composition-transport-source-controls composition-caller-transport-native-observer)
  if(NAME STREQUAL "composition-transport-native-observer")
    set(LEAF TransportNativeObservation.cpp)
  elseif(NAME STREQUAL "composition-caller-transport-native-observer")
    set(LEAF TransportCallerObservationV1.cpp)
  else()
    set(LEAF TransportSourceTests.cpp)
  endif()
  add_executable(${NAME} ${LEAF} TransportAnalyzerV1.cpp)
  target_compile_features(${NAME} PRIVATE cxx_std_20)
  target_include_directories(${NAME} PRIVATE "${FE2O3_WORKER_SOURCE}/tests")
  target_compile_definitions(${NAME} PRIVATE
    FE2O3_LLVM_BUILD_ID="${FE2O3_EXPECTED_LLVM_BUILD_ID}"
    FE2O3_WORKER_BUILD_ID="${TRANSPORT_WORKER_BUILD_ID}")
  target_compile_options(${NAME} PRIVATE
    -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
  target_link_libraries(${NAME} PRIVATE fe2o3_worker_pipeline)
endforeach()
# Private analyzer TU supplies the same default entrypoint plus the test sibling.
# The ordinary archive itself is not compiled with the private macro.
