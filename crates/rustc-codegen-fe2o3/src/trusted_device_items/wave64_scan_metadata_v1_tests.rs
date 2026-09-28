#[test]
fn scan_helper_metadata_requires_exact_reviewed_path_and_closure() {
    let item = TrustedDeviceItem::Gfx942Wave64InclusiveScanHelper;
    let exact = semantic_definition(
        "collective::wave64_inclusive_scan",
        super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
        [6; 32],
    );
    validate_reviewed_fe2o3_device_provider_definition_v1(item, &exact).unwrap();
    for mutation in 0..8 {
        let mut changed = exact.clone();
        match mutation {
            0 => changed.provider.crate_name = "forged_device".into(),
            1 => changed.provider.stable_crate_id = 0,
            2 => changed.cargo_metadata_build_observation = [0; 32],
            3 => changed.source_closure_identity[0] ^= 1,
            4 => changed.definition_source_identity = [0; 32],
            5 => changed.structural_local_definition_component = [0; 32],
            6 => changed.canonical_definition_path = "fe2o3_device::wave64_inclusive_scan".into(),
            7 => {
                changed.canonical_definition_path =
                    "fe2o3_device::collective::wave64_inclusive_scan_lookalike".into()
            }
            _ => unreachable!(),
        }
        assert!(
            validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err(),
            "mutation {mutation}",
        );
    }
}

#[test]
fn scan_helper_is_unique_traversed_provider_not_a_semantic_terminal() {
    use crate::production_semantic_terminal_v1::{
        ProductionSemanticTerminalRuleV1 as Rule, is_traversed_reviewed_helper_v1,
    };
    let item = TrustedDeviceItem::Gfx942Wave64InclusiveScanHelper;
    let rows: Vec<_> = super::TRUSTED_ITEMS
        .iter()
        .filter(|(candidate, _, _)| *candidate == item)
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].1,
        "fe2o3_device_gfx942_wave64_inclusive_scan_helper_v1"
    );
    assert_eq!(rows[0].2, "fe2o3_device::collective::wave64_inclusive_scan");
    assert_eq!(
        exact_provider_compiler_definition_path_v1(item),
        Some(rows[0].2)
    );
    assert!(is_traversed_reviewed_helper_v1(item));
    assert_eq!(Rule::from_trusted_device_item(item), Rule::Reject(item));
}

#[test]
fn scan_helper_pin_refresh_does_not_admit_previous_provider_materializations() {
    let item = TrustedDeviceItem::Gfx942Wave64InclusiveScanHelper;
    for closure in super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURES_V1 {
        let exact = semantic_definition("collective::wave64_inclusive_scan", closure, [6; 32]);
        validate_reviewed_fe2o3_device_provider_definition_v1(item, &exact).unwrap();
    }
    for old in [
        "1e80b25587112d19ee4ede55ac86a50d77e3cbabb5503addd7ec7ac201f51535",
        "ebcb99e9b9b1fb827bdc32f1d845660e69a02be256d72af9a9cd670113c7bddd",
    ] {
        let stale = semantic_definition("collective::wave64_inclusive_scan", digest(old), [6; 32]);
        assert!(validate_reviewed_fe2o3_device_provider_definition_v1(item, &stale).is_err());
    }
}
