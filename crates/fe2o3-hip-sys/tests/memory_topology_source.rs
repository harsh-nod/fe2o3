const ABI_SHIM: &str = include_str!("../native/memory_topology_abi.c");
const BUILD_SCRIPT: &str = include_str!("../build.rs");

#[test]
fn shim_is_version_gated_and_owns_unstable_hip_structs() {
    assert!(ABI_SHIM.contains("!defined(HIP_VERSION_MAJOR) || HIP_VERSION_MAJOR < 5"));
    assert!(ABI_SHIM.contains("hipMemAllocationProp properties"));
    assert!(ABI_SHIM.contains("hipMemAccessDesc descriptor"));
    assert!(BUILD_SCRIPT.contains("compile_memory_topology_abi"));
    assert!(BUILD_SCRIPT.contains("cfg(fe2o3_hip_memory_topology)"));
}

#[test]
fn every_wrapped_native_entry_has_a_c_type_assertion() {
    for function in [
        "hipDeviceGetUuid",
        "hipDeviceGetPCIBusId",
        "hipMallocManaged",
        "hipMemPrefetchAsync",
        "hipMemAdvise",
        "hipMemRangeGetAttribute",
        "hipMemAddressReserve",
        "hipMemAddressFree",
        "hipMemCreate",
        "hipMemGetAllocationGranularity",
        "hipMemMap",
        "hipMemSetAccess",
        "hipMemGetAccess",
        "hipMemUnmap",
        "hipMemRelease",
    ] {
        assert!(
            ABI_SHIM.contains(&format!("FE2O3_ASSERT_FUNCTION_TYPE({function}")),
            "missing C ABI assertion for {function}"
        );
    }
}

#[test]
fn invalid_stable_operations_are_rejected_before_hip() {
    assert!(ABI_SHIM.contains("default:\n    return hipErrorInvalidValue;"));
    assert!(ABI_SHIM.contains("native_access != hipMemAccessFlagsProtNone"));
}
