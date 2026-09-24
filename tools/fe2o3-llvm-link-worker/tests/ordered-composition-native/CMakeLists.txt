cmake_minimum_required(VERSION 3.20)
project(fe2o3_ordered_composition_native LANGUAGES CXX)
if(NOT IS_ABSOLUTE "${FE2O3_WORKER_SOURCE}" OR
   NOT EXISTS "${FE2O3_WORKER_SOURCE}/include/WorkerPipeline.h")
  message(FATAL_ERROR "FE2O3_WORKER_SOURCE must name the existing pinned worker")
endif()
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
add_subdirectory("${FE2O3_WORKER_SOURCE}" existing-worker)
file(READ "${CMAKE_CURRENT_BINARY_DIR}/existing-worker/fe2o3-worker-build-id.txt" PROBE_WORKER_BUILD_ID)
string(STRIP "${PROBE_WORKER_BUILD_ID}" PROBE_WORKER_BUILD_ID)
add_executable(ordered-composition-native-observer CompositionNativeObservation.cpp)
target_compile_features(ordered-composition-native-observer PRIVATE cxx_std_20)
target_include_directories(ordered-composition-native-observer PRIVATE "${FE2O3_WORKER_SOURCE}/tests")
target_compile_definitions(ordered-composition-native-observer PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_EXPECTED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${PROBE_WORKER_BUILD_ID}")
target_compile_options(ordered-composition-native-observer PRIVATE
  -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(ordered-composition-native-observer PRIVATE fe2o3_worker_pipeline)
add_executable(ordered-composition-resource-tests CompositionResourceRelationTests.cpp)
target_compile_features(ordered-composition-resource-tests PRIVATE cxx_std_20)
target_compile_options(ordered-composition-resource-tests PRIVATE
  -Wall -Wextra -Wpedantic -Werror -fno-exceptions -fno-rtti)
