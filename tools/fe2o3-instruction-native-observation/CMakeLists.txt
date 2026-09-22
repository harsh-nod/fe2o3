cmake_minimum_required(VERSION 3.20)
project(fe2o3_instruction_native_observation LANGUAGES CXX)

# Standalone diagnostic only: no parent-project or protected-worker registration.
if(NOT CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
  message(FATAL_ERROR "Configure this observation directory as a standalone project")
endif()
if(NOT CMAKE_HOST_SYSTEM_NAME STREQUAL "Linux" OR CMAKE_CROSSCOMPILING)
  message(FATAL_ERROR "The retained observer requires a native Linux host")
endif()
if(NOT CMAKE_BUILD_TYPE STREQUAL "Release" OR CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR "Use one explicit Release configuration")
endif()
if(NOT IS_ABSOLUTE "${CMAKE_CXX_COMPILER}" OR
   NOT EXISTS "${CMAKE_CXX_COMPILER}")
  message(FATAL_ERROR "Select an explicit absolute C++ compiler")
endif()

function(instruction_require_path Name Kind)
  if(NOT DEFINED ${Name} OR "${${Name}}" STREQUAL "" OR
     NOT IS_ABSOLUTE "${${Name}}" OR NOT EXISTS "${${Name}}")
    message(FATAL_ERROR "${Name} must name an explicit existing absolute path")
  endif()
  string(LENGTH "${${Name}}" PathLength)
  if(PathLength GREATER 4096 OR "${${Name}}" MATCHES "[;\r\n]")
    message(FATAL_ERROR "${Name} is not a bounded single path")
  endif()
  file(REAL_PATH "${${Name}}" Resolved)
  if(NOT Resolved STREQUAL "${${Name}}")
    message(FATAL_ERROR "${Name} must be canonical, without symlink traversal")
  endif()
  if(Kind STREQUAL "directory")
    if(NOT IS_DIRECTORY "${Resolved}")
      message(FATAL_ERROR "${Name} must be a directory")
    endif()
  elseif(IS_DIRECTORY "${Resolved}")
    message(FATAL_ERROR "${Name} must be a file")
  endif()
endfunction()

# No implicit sibling checkout, SDK discovery, download, or task-root fallback.
instruction_require_path(FE2O3_INSTRUCTION_WORKER_SOURCE directory)
instruction_require_path(LLVM_DIR directory)
instruction_require_path(LLD_DIR directory)
instruction_require_path(FE2O3_LLVM_BUILD_ID_FILE file)
instruction_require_path(FE2O3_GFX942_DEVICE_LIB_DIR directory)
instruction_require_path(FE2O3_GFX950_DEVICE_LIB_DIR directory)
if(NOT FE2O3_PINNED_LLVM_VERSION STREQUAL "22.0.0git")
  message(FATAL_ERROR "This closed observer currently supports the reviewed LLVM 22.0.0git API")
endif()
if(NOT DEFINED FE2O3_EXPECTED_LLVM_BUILD_ID OR
   FE2O3_EXPECTED_LLVM_BUILD_ID STREQUAL "")
  message(FATAL_ERROR "Supply the independently selected LLVM build claim")
endif()
if(NOT DEFINED FE2O3_INSTRUCTION_EXPECTED_WORKER_BUILD_ID OR
   NOT FE2O3_INSTRUCTION_EXPECTED_WORKER_BUILD_ID MATCHES
       "^fe2o3-worker-v1-sha256-[0-9a-f]+$")
  message(FATAL_ERROR "Supply the independently selected worker build claim")
endif()
string(REGEX REPLACE "^fe2o3-worker-v1-sha256-" ""
  InstructionWorkerDigest "${FE2O3_INSTRUCTION_EXPECTED_WORKER_BUILD_ID}")
string(LENGTH "${InstructionWorkerDigest}" InstructionWorkerDigestLength)
if(NOT InstructionWorkerDigestLength EQUAL 64 OR
   InstructionWorkerDigest STREQUAL
     "0000000000000000000000000000000000000000000000000000000000000000")
  message(FATAL_ERROR "The worker build claim needs a nonzero lowercase SHA256")
endif()

file(REAL_PATH "${CMAKE_CURRENT_SOURCE_DIR}" InstructionSource)
file(REAL_PATH "${CMAKE_BINARY_DIR}" InstructionBuild)
foreach(SourceRoot IN ITEMS "${InstructionSource}" "${FE2O3_INSTRUCTION_WORKER_SOURCE}")
  string(FIND "${InstructionBuild}/" "${SourceRoot}/" InsideSource)
  if(InsideSource EQUAL 0)
    message(FATAL_ERROR "Use a fresh build directory outside observer and worker sources")
  endif()
endforeach()
# This profile needs no OCML/device-library provider. Require caller-selected
# empty directories rather than enabling a provider through a host default.
foreach(DeviceDirectory IN ITEMS
    "${FE2O3_GFX942_DEVICE_LIB_DIR}" "${FE2O3_GFX950_DEVICE_LIB_DIR}")
  file(GLOB DeviceEntries LIST_DIRECTORIES true
    "${DeviceDirectory}/*" "${DeviceDirectory}/.[!.]*" "${DeviceDirectory}/..?*")
  if(DeviceEntries)
    message(FATAL_ERROR "This closed observation requires empty device-library directories")
  endif()
endforeach()

include("${CMAKE_CURRENT_SOURCE_DIR}/WorkerSourcePins.cmake")
instruction_check_worker_sources()

# The actual qualified SDK/link command uses STATIC component targets, not LLVM.
# Preserve the unchanged worker's component-graph branch; do not support a second
# SDK graph merely because the worker also has a monolithic-target branch.
find_package(LLVM REQUIRED CONFIG NO_DEFAULT_PATH PATHS "${LLVM_DIR}")
find_package(LLD REQUIRED CONFIG NO_DEFAULT_PATH PATHS "${LLD_DIR}")
if(TARGET LLVM)
  message(FATAL_ERROR "This observation requires the reviewed static-component SDK graph")
endif()
foreach(Component IN ITEMS
    LLVMCore LLVMBitReader LLVMLinker LLVMPasses LLVMTarget LLVMCodeGen
    LLVMObject LLVMSupport LLVMTransformUtils LLVMMC LLVMMCDisassembler
    LLVMAMDGPUCodeGen LLVMAMDGPUAsmParser LLVMAMDGPUDesc LLVMAMDGPUInfo
    LLVMAMDGPUUtils LLVMAsmParser lldELF lldCommon)
  if(NOT TARGET ${Component})
    message(FATAL_ERROR "Required reviewed SDK component is absent: ${Component}")
  endif()
  get_target_property(ComponentType ${Component} TYPE)
  if(NOT ComponentType STREQUAL "STATIC_LIBRARY")
    message(FATAL_ERROR "Required SDK component must be static: ${Component}")
  endif()
endforeach()
# Normal variable scoped to this standalone build; do not rewrite a caller's
# cache or change the worker. Only the requested observer/dependencies build.
set(BUILD_TESTING OFF)
add_subdirectory("${FE2O3_INSTRUCTION_WORKER_SOURCE}" existing-worker EXCLUDE_FROM_ALL)
instruction_check_worker_sources()
file(READ "${CMAKE_CURRENT_BINARY_DIR}/existing-worker/fe2o3-worker-build-id.txt"
  InstructionWorkerBuildId)
string(STRIP "${InstructionWorkerBuildId}" InstructionWorkerBuildId)
if(NOT InstructionWorkerBuildId STREQUAL
   FE2O3_INSTRUCTION_EXPECTED_WORKER_BUILD_ID)
  message(FATAL_ERROR
    "Worker build claim differs from the caller's reviewed configuration: ${InstructionWorkerBuildId}")
endif()

add_executable(instruction-source-candidate InstructionSourceCandidate.cpp)
target_compile_features(instruction-source-candidate PRIVATE cxx_std_20)
target_include_directories(instruction-source-candidate PRIVATE
  "${FE2O3_INSTRUCTION_WORKER_SOURCE}/tests"
  "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_definitions(instruction-source-candidate PRIVATE
  FE2O3_LLVM_BUILD_ID="${FE2O3_EXPECTED_LLVM_BUILD_ID}"
  FE2O3_WORKER_BUILD_ID="${InstructionWorkerBuildId}")
target_compile_options(instruction-source-candidate PRIVATE
  -Wall -Wextra -Wpedantic -Werror -Wno-unused-parameter -fno-exceptions -fno-rtti)
# This observer calls parseAssemblyString directly. Link AsmParser explicitly:
# BUILD_TESTING=OFF must not rely on a worker test target's component mapping.
llvm_map_components_to_libnames(InstructionObserverLlvmLibraries AsmParser)
target_link_libraries(instruction-source-candidate PRIVATE
  fe2o3_worker_pipeline ${InstructionObserverLlvmLibraries})
# No install(), add_test(), publication, loading, launch, or resume registration.
