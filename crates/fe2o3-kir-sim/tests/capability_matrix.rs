use std::collections::BTreeSet;
use std::process::Command;

use fe2o3_kir_sim::{
    POINTER_CAPABILITY_ROWS_V1, SCALAR_CAPABILITY_ROWS_V1,
    SEMANTIC_CAPABILITY_MATRIX_JSON_BYTES_V1, SEMANTIC_CAPABILITY_MATRIX_SCHEMA_V1,
    SimulationCapabilityDispositionV1, SimulationKirWireVersionV1, SimulationOperationSurfaceV1,
    SimulationUnsupportedReasonCodeV1, TOP_LEVEL_CAPABILITY_ROWS_V1, semantic_capability_matrix_v1,
};

#[test]
fn matrix_is_complete_unique_bounded_and_authority_free() {
    let matrix = semantic_capability_matrix_v1();
    assert_eq!(matrix.schema, SEMANTIC_CAPABILITY_MATRIX_SCHEMA_V1);
    assert_eq!(matrix.truth_origin, "declared");
    assert_eq!(matrix.authority, "none");
    assert!(!matrix.hardware_observed);
    assert!(!matrix.performance_prediction);
    assert_eq!(matrix.top_level_rows.len(), TOP_LEVEL_CAPABILITY_ROWS_V1);
    assert_eq!(matrix.scalar_rows.len(), SCALAR_CAPABILITY_ROWS_V1);
    assert_eq!(matrix.pointer_rows.len(), POINTER_CAPABILITY_ROWS_V1);

    let top_keys = matrix
        .top_level_rows
        .iter()
        .map(|row| (row.profile, row.kir_wire_version, row.operation))
        .collect::<BTreeSet<_>>();
    assert_eq!(top_keys.len(), matrix.top_level_rows.len());
    let scalar_keys = matrix
        .scalar_rows
        .iter()
        .map(|row| (row.profile, row.family, row.operation, row.lhs, row.rhs))
        .collect::<BTreeSet<_>>();
    assert_eq!(scalar_keys.len(), matrix.scalar_rows.len());
    let pointer_keys = matrix
        .pointer_rows
        .iter()
        .map(|row| {
            (
                row.profile,
                row.kir_wire_version,
                row.operation,
                row.from_access,
                row.to_access,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(pointer_keys.len(), matrix.pointer_rows.len());

    for capability in matrix
        .top_level_rows
        .iter()
        .map(|row| &row.capability)
        .chain(matrix.scalar_rows.iter().map(|row| &row.capability))
        .chain(matrix.pointer_rows.iter().map(|row| &row.capability))
    {
        match capability {
            SimulationCapabilityDispositionV1::Owned { .. }
            | SimulationCapabilityDispositionV1::Unsupported { .. } => {}
        }
    }
}

#[test]
fn pointer_access_restriction_is_typed_memory_owned_only_in_v11() {
    let matrix = semantic_capability_matrix_v1();
    for profile in matrix
        .pointer_rows
        .iter()
        .map(|row| row.profile)
        .collect::<BTreeSet<_>>()
    {
        for version in [
            SimulationKirWireVersionV1::V7,
            SimulationKirWireVersionV1::V9,
            SimulationKirWireVersionV1::V10,
        ] {
            assert!(matches!(
                matrix
                    .pointer_rows
                    .iter()
                    .find(|row| { row.profile == profile && row.kir_wire_version == version })
                    .unwrap()
                    .capability,
                SimulationCapabilityDispositionV1::Unsupported {
                    reason: SimulationUnsupportedReasonCodeV1::InvalidPointerAccessRestriction,
                }
            ));
        }
        assert!(matches!(
            matrix
                .pointer_rows
                .iter()
                .find(|row| {
                    row.profile == profile
                        && row.kir_wire_version == SimulationKirWireVersionV1::V11
                })
                .unwrap()
                .capability,
            SimulationCapabilityDispositionV1::Owned { .. }
        ));
    }
}

#[test]
fn memory_intrinsic_ownership_is_explicitly_additive_v10() {
    let matrix = semantic_capability_matrix_v1();
    for profile in matrix
        .top_level_rows
        .iter()
        .map(|row| row.profile)
        .collect::<BTreeSet<_>>()
    {
        let capability = |version| {
            &matrix
                .top_level_rows
                .iter()
                .find(|row| {
                    row.profile == profile
                        && row.kir_wire_version == version
                        && row.operation == SimulationOperationSurfaceV1::MemoryIntrinsic
                })
                .unwrap()
                .capability
        };
        assert_eq!(
            capability(SimulationKirWireVersionV1::V7),
            &SimulationCapabilityDispositionV1::Unsupported {
                reason: SimulationUnsupportedReasonCodeV1::MemoryIntrinsic,
            }
        );
        assert_eq!(
            capability(SimulationKirWireVersionV1::V9),
            &SimulationCapabilityDispositionV1::Unsupported {
                reason: SimulationUnsupportedReasonCodeV1::MemoryIntrinsic,
            }
        );
        assert!(matches!(
            capability(SimulationKirWireVersionV1::V10),
            SimulationCapabilityDispositionV1::Owned { .. }
        ));
    }
}

#[test]
fn f32_wave_ownership_is_explicitly_additive_v9_and_v10() {
    let matrix = semantic_capability_matrix_v1();
    for profile in matrix
        .top_level_rows
        .iter()
        .map(|row| row.profile)
        .collect::<BTreeSet<_>>()
    {
        let capability = |version| {
            &matrix
                .top_level_rows
                .iter()
                .find(|row| {
                    row.profile == profile
                        && row.kir_wire_version == version
                        && row.operation == SimulationOperationSurfaceV1::Wave
                })
                .unwrap()
                .capability
        };
        assert!(matches!(
            capability(SimulationKirWireVersionV1::V7),
            SimulationCapabilityDispositionV1::Owned {
                typed_rejections,
                ..
            } if *typed_rejections == [SimulationUnsupportedReasonCodeV1::Wave]
        ));
        for version in [
            SimulationKirWireVersionV1::V9,
            SimulationKirWireVersionV1::V10,
        ] {
            assert!(matches!(
                capability(version),
                SimulationCapabilityDispositionV1::Owned {
                    typed_rejections,
                    ..
                } if typed_rejections.is_empty()
            ));
        }
    }
}

#[test]
fn matrix_lds_and_v9_transpose_ownership_keep_numerical_rejection_explicit() {
    let matrix = semantic_capability_matrix_v1();
    for profile in matrix
        .top_level_rows
        .iter()
        .map(|row| row.profile)
        .collect::<BTreeSet<_>>()
    {
        let capability = |version, operation| {
            &matrix
                .top_level_rows
                .iter()
                .find(|row| {
                    row.profile == profile
                        && row.kir_wire_version == version
                        && row.operation == operation
                })
                .unwrap()
                .capability
        };
        for version in [
            SimulationKirWireVersionV1::V7,
            SimulationKirWireVersionV1::V9,
            SimulationKirWireVersionV1::V10,
        ] {
            assert!(matches!(
                capability(version, SimulationOperationSurfaceV1::Matrix),
                SimulationCapabilityDispositionV1::Owned {
                    typed_rejections,
                    ..
                } if *typed_rejections
                    == [SimulationUnsupportedReasonCodeV1::UnsupportedNumericalContract]
            ));
        }
        assert!(matches!(
            capability(
                SimulationKirWireVersionV1::V7,
                SimulationOperationSurfaceV1::Gfx950LdsTranspose
            ),
            SimulationCapabilityDispositionV1::Unsupported {
                reason: SimulationUnsupportedReasonCodeV1::Gfx950LdsTranspose
            }
        ));
        for version in [
            SimulationKirWireVersionV1::V9,
            SimulationKirWireVersionV1::V10,
        ] {
            assert!(matches!(
                capability(version, SimulationOperationSurfaceV1::Gfx950LdsTranspose),
                SimulationCapabilityDispositionV1::Owned {
                    typed_rejections,
                    ..
                } if typed_rejections.is_empty()
            ));
        }
    }
}

#[test]
fn explicit_dynamic_lds_request_is_owned_without_changing_legacy_lds_admission() {
    let matrix = semantic_capability_matrix_v1();
    for profile in matrix
        .top_level_rows
        .iter()
        .map(|row| row.profile)
        .collect::<BTreeSet<_>>()
    {
        for version in [
            SimulationKirWireVersionV1::V7,
            SimulationKirWireVersionV1::V9,
            SimulationKirWireVersionV1::V10,
        ] {
            let capability = |operation| {
                &matrix
                    .top_level_rows
                    .iter()
                    .find(|row| {
                        row.profile == profile
                            && row.kir_wire_version == version
                            && row.operation == operation
                    })
                    .unwrap()
                    .capability
            };
            assert!(matches!(
                capability(SimulationOperationSurfaceV1::WorkgroupMemory),
                SimulationCapabilityDispositionV1::Owned {
                    typed_rejections,
                    ..
                } if typed_rejections.contains(
                    &SimulationUnsupportedReasonCodeV1::DynamicWorkgroupMemory
                )
            ));
            assert!(matches!(
                capability(SimulationOperationSurfaceV1::DynamicWorkgroupMemoryRequest),
                SimulationCapabilityDispositionV1::Owned {
                    typed_rejections,
                    ..
                } if *typed_rejections == [
                    SimulationUnsupportedReasonCodeV1::DynamicWorkgroupMemoryMissingBase,
                    SimulationUnsupportedReasonCodeV1::DynamicWorkgroupMemoryAmbiguousBases,
                    SimulationUnsupportedReasonCodeV1::DynamicWorkgroupMemoryAuthenticatedMinimum,
                    SimulationUnsupportedReasonCodeV1::DynamicWorkgroupMemoryExtentLayout,
                    SimulationUnsupportedReasonCodeV1::NonScalarMemory,
                ]
            ));
        }
    }
}

#[test]
fn json_command_emits_the_same_stable_matrix() {
    let first = Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim-capabilities"))
        .output()
        .unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim-capabilities"))
        .output()
        .unwrap();
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout.len(), SEMANTIC_CAPABILITY_MATRIX_JSON_BYTES_V1);
    let value: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["schema"], SEMANTIC_CAPABILITY_MATRIX_SCHEMA_V1);
    assert_eq!(value["truth_origin"], "declared");
    assert_eq!(value["authority"], "none");
    assert_eq!(value["hardware_observed"], false);
    assert_eq!(value["performance_prediction"], false);
    assert_eq!(
        value["top_level_rows"].as_array().unwrap().len(),
        TOP_LEVEL_CAPABILITY_ROWS_V1
    );
    assert_eq!(
        value["scalar_rows"].as_array().unwrap().len(),
        SCALAR_CAPABILITY_ROWS_V1
    );
    assert_eq!(
        value["pointer_rows"].as_array().unwrap().len(),
        POINTER_CAPABILITY_ROWS_V1
    );
}
