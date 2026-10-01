//! Calls the unchanged producer conversion on a small inert raw fixture.
//! This is original conversion coverage, not a fresh rustc-session or admitted
//! production-bindings fixture. Raw inputs remain separate owners throughout.

use super::*;
use crate::production_pipeline::bindings_debug_retained_storage_v1::charge_retained_debug_sources_v1;
use fe2o3_kernel_ir::{DebugSourceMapFileV1, LogicalStorageCounterV1, LogicalStorageLimitsV1};

fn source() -> RetainedSemanticSourceProducerV1 {
    RetainedSemanticSourceProducerV1 {
        provenance: SemanticSourceProvenanceV1::unavailable(),
        expansion_chain_sha256: [7; 32],
    }
}

fn raw_scopes() -> [RetainedRawDebugSourceScopeV2; 2] {
    [
        RetainedRawDebugSourceScopeV2 {
            raw_scope: 0,
            parent_raw_scope: None,
            depth: 0,
            source: source(),
        },
        RetainedRawDebugSourceScopeV2 {
            raw_scope: 1,
            parent_raw_scope: Some(0),
            depth: 1,
            source: source(),
        },
    ]
}

#[test]
fn original_conversion_outputs_are_observed_without_charging_raw_inputs_twice() {
    let mut raw_name = String::with_capacity(173);
    raw_name.push_str("value");
    let raw_capacity = raw_name.capacity();
    let raw_variables = [
        RetainedRawDebugSourceVariableV2 {
            ordinal: 0,
            exact_name: Some(raw_name),
            name_sha256: [8; 32],
            raw_scope: 1,
            class: RetainedRawDebugSourceVariableClassV2::Local(0),
            entry_value_preserved: true,
        },
        RetainedRawDebugSourceVariableV2 {
            ordinal: 1,
            exact_name: None,
            name_sha256: [9; 32],
            raw_scope: 0,
            class: RetainedRawDebugSourceVariableClassV2::Unrepresented,
            entry_value_preserved: false,
        },
    ];
    let local = RetainedSemanticLocalProducerV1 {
        identity: SemanticLocalIdentityV1::from_sha256([4; 32]),
        rustc_local: 0,
        ty: SemanticTypeIdV1::from_index(0),
        source: source(),
    };
    let (scopes, variables) = convert_debug_sources_v2(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdentityV1::from_sha256([3; 32]),
        &raw_scopes(),
        &raw_variables,
        &[SemanticLocalIdV1::from_index(0)],
        &[local],
    )
    .unwrap();
    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[1].parent_identity, Some(scopes[0].identity));
    assert_eq!(variables[0].scope_identity, scopes[1].identity);
    assert_eq!(
        variables[0].class,
        RetainedDebugSourceVariableClassV2::Local(SemanticLocalIdV1::from_index(0))
    );
    assert_eq!(variables[0].name.as_deref(), Some("value"));
    assert!(variables[0].entry_value_preserved);
    assert_eq!(
        variables[1].class,
        RetainedDebugSourceVariableClassV2::Unrepresented
    );
    assert!(!variables[1].entry_value_preserved);
    let mut path = String::with_capacity(89);
    path.push_str("actual.rs");
    let path_capacity = path.capacity();
    let files = vec![DebugSourceMapFileV1::new([1; 32], 32, path).unwrap()].into_boxed_slice();
    let expected = files.len() * size_of::<DebugSourceMapFileV1>()
        + scopes.len() * size_of::<RetainedDebugSourceScopeV2>()
        + variables.len() * size_of::<RetainedDebugSourceVariableV2>()
        + path_capacity
        + variables[0].name.as_ref().unwrap().capacity();
    let original_identity = variables[0].identity;
    let mut c = LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: Some(expected),
        max_items: 11,
    });
    charge_retained_debug_sources_v1(&files, &scopes, &variables, &None, &mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, 11));
    assert_eq!(variables[0].identity, original_identity);
    assert_eq!(
        raw_variables[0].exact_name.as_ref().unwrap().capacity(),
        raw_capacity
    );
    assert_eq!(raw_variables[0].exact_name.as_deref(), Some("value"));
}

#[test]
fn original_conversion_still_refuses_missing_scope_before_observation() {
    let raw = [RetainedRawDebugSourceVariableV2 {
        ordinal: 0,
        exact_name: Some("value".into()),
        name_sha256: [8; 32],
        raw_scope: 77,
        class: RetainedRawDebugSourceVariableClassV2::Unrepresented,
        entry_value_preserved: false,
    }];
    let result = convert_debug_sources_v2(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdentityV1::from_sha256([3; 32]),
        &raw_scopes(),
        &raw,
        &[],
        &[],
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)
    ));
}
