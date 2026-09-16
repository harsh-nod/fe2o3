#[test]
fn generative_provider_items_require_exact_reviewed_identity() {
    for item in [
        TrustedDeviceItem::KernelContext,
        TrustedDeviceItem::KernelContextIssue,
        TrustedDeviceItem::ExecutionWorkgroupCapability,
        TrustedDeviceItem::ExecutionWorkgroupCurrent,
        TrustedDeviceItem::MaskedTile1D,
        TrustedDeviceItem::LaneFragment1D,
        TrustedDeviceItem::MaskedTile1DLoadMasked,
        TrustedDeviceItem::MaskedTile1DIntoFragment,
        TrustedDeviceItem::LaneFragment1DIntoParts,
    ] {
        let path = exact_provider_compiler_definition_path_v1(item).unwrap();
        let exact = semantic_definition(
            path.strip_prefix("fe2o3_device::").unwrap(),
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
            [6; 32],
        );
        validate_reviewed_fe2o3_device_provider_definition_v1(item, &exact).unwrap();
        for mutation in 0..4 {
            let mut changed = exact.clone();
            match mutation {
                0 => changed.provider.crate_name = "forged_device".into(),
                1 => changed.source_closure_identity[0] ^= 1,
                2 => changed.cargo_metadata_build_observation = [0; 32],
                _ => {
                    changed = semantic_definition(
                        "wrong_module::{impl#0}::same_name",
                        super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
                        [6; 32],
                    )
                }
            }
            assert!(
                validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err(),
                "{item:?} accepted mutation {mutation}",
            );
        }
    }
}
