# Select one complete reviewed closure, never a union of per-file digests.
function(fe2o3_gfx950_provider_profile PROFILE)
  set(FILES
    ocml.bc ockl.bc oclc_daz_opt_off.bc oclc_unsafe_math_off.bc
    oclc_finite_only_off.bc oclc_correctly_rounded_sqrt_on.bc
    oclc_wavefrontsize64_on.bc oclc_isa_version_950.bc oclc_abi_version_600.bc)
  if(PROFILE STREQUAL "rocm-7.2.1")
    set(DIRECTORY "/opt/rocm-7.2.1/lib/llvm/lib/clang/22/lib/amdgcn/bitcode")
    set(DIGESTS
      2e3451857fcf47b931c5c5a29e9c42a6ddc3099c8359079441a9a06a217ead7e
      8320aec59c4dc87cb28fdb374a44a55088a6258b59dffae4a85e8eacec8be456
      3b2344acba86e174b87961e8a5e4a164ab61addf8c8a035e9b6dcd03ddab23fa
      a500bc03fd046bcd7806938ea323758e5c9ba8d56cfd767cef71612b3bd87d37
      e1d1fddf85577b078d02a07212f670324e1e157d1b6608a8c765ad3c171a7b29
      3b2344acba86e174b87961e8a5e4a164ab61addf8c8a035e9b6dcd03ddab23fa
      9560b0d120b9e7c6b28a56a87eeed4ae155b60dec54152700ff9f60b69de1259
      9ea1498966ac0b4d0a54677501a847cb1ee932768e78576613d42985bf394d34
      79d3d09404f5df01c484dc15cc64583c7c1803234463eee6505226f0186a71b1)
    set(MANIFEST_SHA "legacy-inline-rocm-7.2.1")
  elseif(PROFILE STREQUAL "rocm-7.2.4")
    set(MANIFEST "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../../../examples/gfx950_low_precision/gfx950-ocml-rocm-7.2.4.manifest")
    # The example manifest is an existing reviewed whole profile, not a caller input.
    set(MANIFEST_SHA "330a4190d140bfd9b9eeeefa97302241c9e422e140e0044d4cb9f49d0c24696e")
    file(SHA256 "${MANIFEST}" ACTUAL_SHA)
    if(NOT ACTUAL_SHA STREQUAL MANIFEST_SHA)
      message(FATAL_ERROR "gfx950 reviewed whole-profile manifest mismatch")
    endif()
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${MANIFEST}")
    file(STRINGS "${MANIFEST}" LINES)
    list(LENGTH LINES LINE_COUNT)
    if(NOT LINE_COUNT EQUAL 15)
      message(FATAL_ERROR "gfx950 reviewed manifest field census mismatch")
    endif()
    list(GET LINES 0 SCHEMA)
    list(GET LINES 1 ROCM)
    list(GET LINES 2 LLVM)
    list(GET LINES 3 LOCATION)
    set(DIRECTORY "/opt/rocm-7.2.4/lib/llvm/lib/clang/22/lib/amdgcn/bitcode")
    if(NOT SCHEMA STREQUAL "schema=fe2o3-gfx950-ocml-closure-v1" OR
       NOT ROCM STREQUAL "rocm_version=7.2.4" OR
       NOT LLVM STREQUAL "llvm_version=22.0.0git" OR
       NOT LOCATION STREQUAL "canonical_device_library_dir=${DIRECTORY}")
      message(FATAL_ERROR "gfx950 reviewed manifest identity mismatch")
    endif()
    set(DIGESTS)
    set(INDEX 4)
    foreach(NAME IN LISTS FILES)
      list(GET LINES ${INDEX} LINE)
      string(FIND "${LINE}" "=" SEPARATOR)
      if(SEPARATOR LESS 1)
        message(FATAL_ERROR "gfx950 reviewed manifest has a malformed row")
      endif()
      string(SUBSTRING "${LINE}" 0 ${SEPARATOR} KEY)
      math(EXPR VALUE_START "${SEPARATOR} + 1")
      string(SUBSTRING "${LINE}" ${VALUE_START} -1 VALUE)
      string(LENGTH "${VALUE}" VALUE_LENGTH)
      if(NOT KEY STREQUAL NAME OR NOT VALUE_LENGTH EQUAL 64 OR
         NOT VALUE MATCHES "^[0-9a-f]+$")
        message(FATAL_ERROR "gfx950 reviewed manifest file identity mismatch")
      endif()
      list(APPEND DIGESTS "${VALUE}")
      math(EXPR INDEX "${INDEX} + 1")
    endforeach()
  else()
    message(FATAL_ERROR "unknown gfx950 device-library profile: ${PROFILE}")
  endif()
  set(FE2O3_GFX950_DEVICE_LIB_FILES "${FILES}" PARENT_SCOPE)
  set(FE2O3_GFX950_DEVICE_LIB_SHA256 "${DIGESTS}" PARENT_SCOPE)
  set(FE2O3_GFX950_DEFAULT_DEVICE_LIB_DIR "${DIRECTORY}" PARENT_SCOPE)
  set(FE2O3_GFX950_PROFILE_MANIFEST_SHA256 "${MANIFEST_SHA}" PARENT_SCOPE)
endfunction()
