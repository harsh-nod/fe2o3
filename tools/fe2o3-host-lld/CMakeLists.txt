cmake_minimum_required(VERSION 3.20)
project(fe2o3_host_lld LANGUAGES CXX)

option(BUILD_TESTING "Build the fe2o3 host LLD tests" ON)

foreach(required IN ITEMS
    LLVM_DIR LLD_DIR FE2O3_LLVM_PACKAGE_ROOT FE2O3_LLVM_BUILD_ID_FILE
    FE2O3_EXPECTED_LLVM_BUILD_ID FE2O3_EXPECTED_LLVM_SOURCE_COMMIT
    FE2O3_EXPECTED_LLVM_SOURCE_TREE)
  if(NOT DEFINED ${required} OR "${${required}}" STREQUAL "")
    message(FATAL_ERROR "${required} is required")
  endif()
endforeach()

if(CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR "multi-config generators are not supported")
endif()
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux" OR
   NOT CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|AMD64)$")
  message(FATAL_ERROR "fe2o3-host-lld currently supports Linux x86_64 only")
endif()
if(NOT CMAKE_BUILD_TYPE STREQUAL "Release")
  message(FATAL_ERROR "fe2o3-host-lld requires CMAKE_BUILD_TYPE=Release")
endif()

file(REAL_PATH "${FE2O3_LLVM_PACKAGE_ROOT}" FE2O3_LLVM_PACKAGE_ROOT_REAL)
file(REAL_PATH "${LLVM_DIR}" FE2O3_LLVM_DIR_REAL)
file(REAL_PATH "${LLD_DIR}" FE2O3_LLD_DIR_REAL)
if(NOT FE2O3_LLVM_DIR_REAL STREQUAL
   "${FE2O3_LLVM_PACKAGE_ROOT_REAL}/lib/cmake/llvm")
  message(FATAL_ERROR "LLVM_DIR is outside the explicit LLVM package root")
endif()
if(NOT FE2O3_LLD_DIR_REAL STREQUAL
   "${FE2O3_LLVM_PACKAGE_ROOT_REAL}/lib/cmake/lld")
  message(FATAL_ERROR "LLD_DIR is outside the explicit LLVM package root")
endif()

# The package exports static archives. Restrict dependency discovery to static
# libraries too so configuration cannot silently select a DSO.
set(CMAKE_FIND_LIBRARY_SUFFIXES ".a")
set(ZLIB_USE_STATIC_LIBS ON)
set(BZIP2_USE_STATIC_LIBS ON)
set(OPENSSL_USE_STATIC_LIBS ON)
find_package(LLVM REQUIRED CONFIG NO_DEFAULT_PATH PATHS "${LLVM_DIR}")
find_package(LLD REQUIRED CONFIG NO_DEFAULT_PATH PATHS "${LLD_DIR}")
find_package(Threads REQUIRED)

if(NOT LLVM_PACKAGE_VERSION STREQUAL "22.1.8")
  message(FATAL_ERROR
    "LLVM version mismatch: found '${LLVM_PACKAGE_VERSION}', expected '22.1.8'")
endif()
if(NOT TARGET lldELF OR NOT TARGET lldCommon OR NOT TARGET LLVMObject)
  message(FATAL_ERROR
    "the pinned package does not export lldELF/lldCommon/LLVMObject")
endif()
foreach(target IN ITEMS lldELF lldCommon LLVMObject)
  get_target_property(location ${target} IMPORTED_LOCATION_RELEASE)
  if(NOT location)
    get_target_property(location ${target} IMPORTED_LOCATION)
  endif()
  file(REAL_PATH "${location}" location_real)
  if(NOT location_real MATCHES "^${FE2O3_LLVM_PACKAGE_ROOT_REAL}/lib/.*\\.a$")
    message(FATAL_ERROR "${target} is not a pinned static archive: ${location_real}")
  endif()
endforeach()

if(NOT EXISTS "${FE2O3_LLVM_BUILD_ID_FILE}")
  message(FATAL_ERROR "FE2O3_LLVM_BUILD_ID_FILE does not exist")
endif()
file(READ "${FE2O3_LLVM_BUILD_ID_FILE}" FE2O3_MEASURED_LLVM_BUILD_ID)
string(STRIP "${FE2O3_MEASURED_LLVM_BUILD_ID}" FE2O3_MEASURED_LLVM_BUILD_ID)
string(LENGTH "${FE2O3_MEASURED_LLVM_BUILD_ID}"
  FE2O3_MEASURED_LLVM_BUILD_ID_LENGTH)
if(FE2O3_MEASURED_LLVM_BUILD_ID_LENGTH GREATER 160 OR
   NOT FE2O3_MEASURED_LLVM_BUILD_ID MATCHES
     "^[A-Za-z0-9][A-Za-z0-9._+:=~-]*$")
  message(FATAL_ERROR "LLVM build identity is noncanonical")
endif()
if(NOT FE2O3_MEASURED_LLVM_BUILD_ID STREQUAL
   FE2O3_EXPECTED_LLVM_BUILD_ID)
  message(FATAL_ERROR
    "LLVM build identity mismatch: measured '${FE2O3_MEASURED_LLVM_BUILD_ID}'")
endif()
foreach(identity IN ITEMS FE2O3_EXPECTED_LLVM_SOURCE_COMMIT
    FE2O3_EXPECTED_LLVM_SOURCE_TREE)
  string(LENGTH "${${identity}}" identity_length)
  if(NOT identity_length EQUAL 40 OR
     NOT "${${identity}}" MATCHES "^[0-9a-f][0-9a-f]*$")
    message(FATAL_ERROR "${identity} is not a canonical Git object identity")
  endif()
endforeach()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

configure_file(include/BuildConfig.h.in generated/BuildConfig.h @ONLY)

add_executable(fe2o3-host-lld src/main.cpp src/OutputPolicy.cpp)
target_include_directories(fe2o3-host-lld PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/generated"
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_include_directories(fe2o3-host-lld SYSTEM PRIVATE
  ${LLVM_INCLUDE_DIRS} ${LLD_INCLUDE_DIRS})
target_compile_options(fe2o3-host-lld PRIVATE
  -Wall -Wextra -Wpedantic -Werror -Wconversion -Wsign-conversion
  -fno-exceptions -fno-rtti -fno-ident -fno-pie
  "-ffile-prefix-map=${CMAKE_CURRENT_SOURCE_DIR}=fe2o3-host-lld-source"
  "-ffile-prefix-map=${CMAKE_CURRENT_BINARY_DIR}=fe2o3-host-lld-build"
  "-fdebug-prefix-map=${CMAKE_CURRENT_SOURCE_DIR}=fe2o3-host-lld-source"
  "-fdebug-prefix-map=${CMAKE_CURRENT_BINARY_DIR}=fe2o3-host-lld-build")
target_link_options(fe2o3-host-lld PRIVATE
  -static -static-libgcc -static-libstdc++ -no-pie
  LINKER:--wrap=_ZN4llvm16FileOutputBuffer6createENS_9StringRefEmj
  LINKER:--build-id=none LINKER:--no-dynamic-linker
  LINKER:-z,noexecstack LINKER:-z,separate-code)
target_link_libraries(fe2o3-host-lld PRIVATE lldELF lldCommon LLVMObject
  Threads::Threads ${CMAKE_DL_LIBS})

install(TARGETS fe2o3-host-lld RUNTIME DESTINATION bin
  PERMISSIONS OWNER_READ OWNER_EXECUTE GROUP_READ GROUP_EXECUTE
              WORLD_READ WORLD_EXECUTE)

if(BUILD_TESTING)
  enable_testing()
  add_test(NAME fe2o3-host-lld-secure-protocol-v2
    COMMAND /usr/bin/bash
      "${CMAKE_CURRENT_SOURCE_DIR}/tests/ctest_secure_protocol.sh"
      "${CMAKE_CURRENT_SOURCE_DIR}"
      "$<TARGET_FILE:fe2o3-host-lld>"
      "${CMAKE_CURRENT_BINARY_DIR}")
  set_tests_properties(fe2o3-host-lld-secure-protocol-v2 PROPERTIES
    TIMEOUT 900
    LABELS "host-link;security;protocol-v2")
endif()
