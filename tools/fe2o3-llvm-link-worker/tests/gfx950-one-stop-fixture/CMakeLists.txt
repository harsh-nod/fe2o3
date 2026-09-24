cmake_minimum_required(VERSION 3.20)
project(fe2o3_gfx950_one_stop_fixture LANGUAGES CXX)
if(NOT IS_ABSOLUTE "${FE2O3_WORKER_SOURCE}" OR
   NOT EXISTS "${FE2O3_WORKER_SOURCE}/include/WorkerPipeline.h")
  message(FATAL_ERROR "FE2O3_WORKER_SOURCE must name the existing pinned worker source")
endif()
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
add_subdirectory("${FE2O3_WORKER_SOURCE}" existing-worker)
file(READ "${CMAKE_CURRENT_BINARY_DIR}/existing-worker/fe2o3-worker-build-id.txt" PROBE_WORKER_BUILD_ID)
string(STRIP "${PROBE_WORKER_BUILD_ID}" PROBE_WORKER_BUILD_ID)
add_executable(gfx950-one-stop-fixture Gfx950OneStopFixture.cpp)
target_compile_features(gfx950-one-stop-fixture PRIVATE cxx_std_20)
target_compile_definitions(gfx950-one-stop-fixture PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_EXPECTED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${PROBE_WORKER_BUILD_ID}")
target_compile_options(gfx950-one-stop-fixture PRIVATE
  -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
target_link_libraries(gfx950-one-stop-fixture PRIVATE fe2o3_worker_pipeline)
