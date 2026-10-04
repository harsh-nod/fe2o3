const ABI_PROBE: &str = include_str!("../native/cooperative_peer_abi.c");
const BUILD_SCRIPT: &str = include_str!("../build.rs");

#[test]
fn abi_probe_is_explicitly_version_gated() {
    assert!(ABI_PROBE.contains("!defined(HIP_VERSION_MAJOR) || HIP_VERSION_MAJOR < 5"));
    assert!(ABI_PROBE.contains("hipDeviceAttributeCooperativeLaunch == 10"));
    assert!(ABI_PROBE.contains("hipDeviceAttributeCooperativeMultiDeviceLaunch == 11"));
    assert!(BUILD_SCRIPT.contains("compile_cooperative_peer_abi"));
    assert!(BUILD_SCRIPT.contains("try_compile"));
}

#[test]
fn abi_probe_covers_every_exposed_entry_point() {
    for function in [
        "hipDeviceGetAttribute",
        "hipDeviceCanAccessPeer",
        "hipDeviceEnablePeerAccess",
        "hipDeviceDisablePeerAccess",
        "hipModuleLaunchCooperativeKernel",
        "hipLaunchCooperativeKernel",
    ] {
        assert!(
            ABI_PROBE.contains(&format!("FE2O3_ASSERT_FUNCTION_TYPE({function}")),
            "missing C ABI assertion for {function}"
        );
    }
}

#[test]
fn no_runtime_build_switch_is_fail_closed() {
    assert!(BUILD_SCRIPT.contains("FE2O3_HIP_SYS_DISABLE"));
    assert!(BUILD_SCRIPT.contains("return None"));
    assert!(BUILD_SCRIPT.contains("cfg(fe2o3_hip_cooperative_peer)"));
}
