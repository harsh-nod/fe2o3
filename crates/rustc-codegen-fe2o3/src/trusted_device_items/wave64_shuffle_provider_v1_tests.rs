// Metadata mutations only; real DefId/impl joins require actual rustc callbacks.
use crate::trusted_device_items::{TRUSTED_ITEMS, Wave64ShuffleScalarV1};

#[test]
fn wave_primitive_metadata_gate_keeps_provider_and_source_closure_bindings() {
    for scalar in [
        Wave64ShuffleScalarV1::U32,
        Wave64ShuffleScalarV1::I32,
        Wave64ShuffleScalarV1::F32,
    ] {
        let item = TrustedDeviceItem::Gfx942Wave64Shuffle(scalar);
        // This inert path deliberately is not claimed to identify an actual impl.
        let metadata = semantic_definition(
            "collective::{impl#999}::__fe2o3_wave64_shuffle_index",
            super::REVIEWED_SAFE_EXECUTION_SOURCE_CLOSURE_V1,
            [6; 32],
        );
        assert_eq!(exact_provider_compiler_definition_path_v1(item), None);
        validate_reviewed_fe2o3_device_provider_definition_v1(item, &metadata).unwrap();
        for mutation in 0..7 {
            let mut changed = metadata.clone();
            match mutation {
                0 => changed.provider.crate_name = "forged_device".into(),
                1 => changed.source_closure_identity[0] ^= 1,
                2 => changed.cargo_metadata_build_observation = [0; 32],
                3 => changed.definition_source_identity = [0; 32],
                4 => changed.provider.stable_crate_id = 0,
                5 => changed.canonical_definition_path = "fe2o3_device::forged::shuffle".into(),
                6 => changed.structural_local_definition_component = [0; 32],
                _ => unreachable!(),
            }
            assert!(
                validate_reviewed_fe2o3_device_provider_definition_v1(item, &changed).is_err(),
                "{scalar:?}: mutation {mutation}"
            );
        }
    }
}

#[test]
fn wave_scalar_registry_and_safe_wrapper_traversal_are_separate() {
    use crate::production_semantic_terminal_v1::{
        ProductionSemanticTerminalRuleV1 as Rule, ProductionTerminalExpansionV1 as Expansion,
        is_traversed_reviewed_helper_v1,
    };
    let mut roles = std::collections::BTreeSet::new();
    for (scalar, tag) in [
        (Wave64ShuffleScalarV1::U32, 135),
        (Wave64ShuffleScalarV1::I32, 136),
        (Wave64ShuffleScalarV1::F32, 137),
    ] {
        let item = TrustedDeviceItem::Gfx942Wave64Shuffle(scalar);
        assert_eq!(scalar.terminal_tag(), tag);
        assert!(roles.insert(item.canonical_path()));
        let rows: Vec<_> = TRUSTED_ITEMS
            .iter()
            .filter(|(candidate, _, _)| *candidate == item)
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, scalar.marker());
        assert_eq!(
            Rule::from_trusted_device_item(item),
            Rule::Expand(Expansion::Gfx942Wave64Shuffle(scalar))
        );
        assert!(!is_traversed_reviewed_helper_v1(item));
    }
    for item in [
        TrustedDeviceItem::Gfx942Wave64ReduceSum,
        TrustedDeviceItem::Gfx942Wave64InclusiveScanSum,
        TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum,
    ] {
        assert!(is_traversed_reviewed_helper_v1(item));
        assert!(
            matches!(Rule::from_trusted_device_item(item), Rule::Reject(actual) if actual == item)
        );
    }
}
