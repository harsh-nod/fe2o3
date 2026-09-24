#[test]
fn complete_body_v36_provider_policy_requires_exact_definition_and_full_closure() {
    let item = TrustedDeviceItem::AmdGpuCompleteBodyE32;
    // Synthetic policy fixture only: this does NOT qualify the changed real device closure.
    let exact = semantic_definition(
        "diagnostics::__amdgpu_complete_body_gfx942_v1",
        super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
        [6; 32],
    );
    assert_eq!(
        exact_provider_compiler_definition_path_v1(item),
        Some("fe2o3_device::diagnostics::__amdgpu_complete_body_gfx942_v1")
    );
    validate_reviewed_fe2o3_device_provider_definition_v1(item, &exact).unwrap();
    for path in [
        "local::__amdgpu_complete_body_gfx942_v1",
        "diagnostics::lookalike::__amdgpu_complete_body_gfx942_v1",
        "diagnostics::__amdgpu_ordered_program_e32_v1",
    ] {
        let changed = semantic_definition(
            path,
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
            [6; 32],
        );
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err());
    }
    for name in ["local_marker", "fe2o3_device_lookalike"] {
        let mut changed = exact.clone();
        changed.provider.crate_name = name.into();
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err());
    }
    let mut stale = exact;
    stale.source_closure_identity[0] ^= 1;
    assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &stale).is_err());
}

#[test]
fn complete_body_v36_marker_is_one_closed_terminal_not_a_traversed_helper() {
    use crate::production_semantic_terminal_v1::{
        ProductionSemanticTerminalRuleV1, ProductionTerminalExpansionV1,
        is_traversed_reviewed_helper_v1,
    };
    let item = TrustedDeviceItem::AmdGpuCompleteBodyE32;
    let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
    assert_eq!(
        rule,
        ProductionSemanticTerminalRuleV1::Expand(
            ProductionTerminalExpansionV1::Gfx942CompleteBodyE32
        )
    );
    assert!(!is_traversed_reviewed_helper_v1(item));
}
