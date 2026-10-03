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
fn integer_assembly_is_scalar_bits_owned_with_explicit_remaining_rejection() {
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| {
            row.kir_wire_version != SimulationKirWireVersionV1::V21
                && row.kir_wire_version != SimulationKirWireVersionV1::V22
                && row.operation == SimulationOperationSurfaceV1::InlineAssembly
        })
        .collect();
    assert_eq!(rows.len(), 4 * 10); // Four target profiles, ten wire versions.
    for row in rows {
        assert_eq!(
            row.capability,
            SimulationCapabilityDispositionV1::Owned {
                owner: fe2o3_kir_sim::SimulationSemanticOwnerV1::ScalarBits,
                typed_rejections: &[SimulationUnsupportedReasonCodeV1::InlineAssembly],
            }
        );
    }
}

#[test]
fn inert_v12_surfaces_keep_stable_ids_and_have_no_simulation_owner() {
    use SimulationOperationSurfaceV1 as Surface;
    assert_eq!(Surface::Constant as u8, 0);
    assert_eq!(Surface::Unreachable as u8, 31);
    assert_eq!(Surface::DynamicWorkgroupMemoryRequest as u8, 32);
    let surfaces = [
        Surface::VectorLoad,
        Surface::VectorStore,
        Surface::VectorLayoutConvert,
        Surface::VerificationContract,
    ];
    let matrix = semantic_capability_matrix_v1();
    for (ordinal, surface) in surfaces.into_iter().enumerate() {
        assert_eq!(surface as usize, 33 + ordinal);
        let rows: Vec<_> = matrix
            .top_level_rows
            .iter()
            .filter(|row| {
                row.kir_wire_version != SimulationKirWireVersionV1::V21
                    && row.kir_wire_version != SimulationKirWireVersionV1::V22
                    && row.operation == surface
            })
            .collect();
        assert_eq!(rows.len(), 4 * 10); // Four target profiles, ten wire versions.
        for row in rows {
            assert!(matches!(&row.capability,
                SimulationCapabilityDispositionV1::Unsupported { reason }
                    if *reason == SimulationUnsupportedReasonCodeV1::InertV12Carrier
            ));
        }
    }
}

#[test]
fn pointer_access_restriction_is_typed_memory_owned_from_v11() {
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
        for version in [
            SimulationKirWireVersionV1::V11,
            SimulationKirWireVersionV1::V12,
            SimulationKirWireVersionV1::V16,
            SimulationKirWireVersionV1::V17,
            SimulationKirWireVersionV1::V18,
            SimulationKirWireVersionV1::V19,
            SimulationKirWireVersionV1::V20,
        ] {
            assert!(matches!(
                matrix
                    .pointer_rows
                    .iter()
                    .find(|row| row.profile == profile && row.kir_wire_version == version)
                    .unwrap()
                    .capability,
                SimulationCapabilityDispositionV1::Owned { .. }
            ));
        }
    }
}

#[test]
fn execution_v15_surface_is_additive_and_has_no_simulation_owner() {
    assert_eq!(SimulationOperationSurfaceV1::Execution as u8, 37);
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| {
            row.kir_wire_version != SimulationKirWireVersionV1::V21
                && row.kir_wire_version != SimulationKirWireVersionV1::V22
                && row.operation == SimulationOperationSurfaceV1::Execution
        })
        .collect();
    assert_eq!(rows.len(), 4 * 10); // Four target profiles, ten wire versions.
    assert!(rows.iter().all(|row| matches!(
        &row.capability,
        SimulationCapabilityDispositionV1::Unsupported {
            reason: SimulationUnsupportedReasonCodeV1::InertExecutionV15,
        }
    )));
}

#[test]
fn v12_inherits_every_v11_disposition_without_activating_inert_carriers() {
    let matrix = semantic_capability_matrix_v1();
    let inherited = matrix
        .top_level_rows
        .iter()
        .filter(|row| row.kir_wire_version == SimulationKirWireVersionV1::V12)
        .collect::<Vec<_>>();
    assert_eq!(inherited.len(), 4 * 49); // Every surface, including distinct V20/V21 refusals.
    for row in inherited {
        let previous = matrix
            .top_level_rows
            .iter()
            .find(|previous| {
                previous.profile == row.profile
                    && previous.operation == row.operation
                    && previous.kir_wire_version == SimulationKirWireVersionV1::V11
            })
            .unwrap();
        assert_eq!(row.capability, previous.capability);
    }
    assert_eq!(
        serde_json::to_string(&SimulationKirWireVersionV1::V12).unwrap(),
        "\"v12\""
    );
}

#[test]
fn v17_extends_only_the_v12_baseline_and_never_owns_the_v16_pair() {
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| row.kir_wire_version == SimulationKirWireVersionV1::V17)
        .collect();
    assert_eq!(rows.len(), 4 * 49); // Every surface, including distinct V20/V21 refusals.
    for row in rows {
        if row.operation == SimulationOperationSurfaceV1::OrderedProgram {
            continue; // Its exact single owned profile is tested by canonical_v17.
        }
        let baseline = matrix
            .top_level_rows
            .iter()
            .find(|previous| {
                previous.kir_wire_version == SimulationKirWireVersionV1::V12
                    && previous.profile == row.profile
                    && previous.operation == row.operation
            })
            .unwrap();
        assert_eq!(row.capability, baseline.capability);
        if row.operation == SimulationOperationSurfaceV1::OrderedRegion {
            assert_eq!(
                row.capability,
                SimulationCapabilityDispositionV1::Unsupported {
                    reason: SimulationUnsupportedReasonCodeV1::OrderedRegionProfile,
                }
            );
        }
    }
    assert_eq!(
        serde_json::to_string(&SimulationKirWireVersionV1::V17).unwrap(),
        "\"v17\""
    );
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
#[test]
fn storage_rows_are_additive_and_do_not_change_old_profile_bytes() {
    let mut matrix = semantic_capability_matrix_v1();
    // Preserve the historical pre-V18 projection independently of new rows.
    matrix
        .top_level_rows
        .retain(|row| row.kir_wire_version != SimulationKirWireVersionV1::V18);
    matrix
        .pointer_rows
        .retain(|row| row.kir_wire_version != SimulationKirWireVersionV1::V18);
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| row.operation == SimulationOperationSurfaceV1::Storage)
        .collect();
    assert_eq!(rows.len(), 44);
    let delta: usize = rows
        .iter()
        .map(|row| serde_json::to_vec(row).unwrap().len() + 1)
        .sum();
    assert_eq!(delta, 5932);
    assert!(rows.iter().all(|row| matches!(
        row.capability,
        SimulationCapabilityDispositionV1::Unsupported {
            reason: SimulationUnsupportedReasonCodeV1::InertStorage
        }
    )));
    assert_eq!(serde_json::to_vec(&matrix).unwrap().len() + 1, 5_061_127);
    matrix
        .top_level_rows
        .retain(|row| row.operation != SimulationOperationSurfaceV1::Storage);
    assert_eq!(serde_json::to_vec(&matrix).unwrap().len() + 1, 5_055_195);
}

#[test]
fn v18_adds_only_private_scalar_storage_and_preserves_remaining_refusals() {
    let matrix = semantic_capability_matrix_v1();
    assert_eq!(matrix.top_level_rows.len(), 2_352);
    assert_eq!(matrix.pointer_rows.len(), 48);
    let mut rows = 0;
    let mut storage = 0;
    let mut added_bytes = 0;
    for row in matrix
        .top_level_rows
        .iter()
        .filter(|row| row.kir_wire_version == SimulationKirWireVersionV1::V18)
    {
        let previous = matrix
            .top_level_rows
            .iter()
            .find(|previous| {
                previous.kir_wire_version == SimulationKirWireVersionV1::V12
                    && previous.profile == row.profile
                    && previous.operation == row.operation
            })
            .unwrap();
        let mut projected = row.clone();
        projected.kir_wire_version = SimulationKirWireVersionV1::V12;
        if row.operation == SimulationOperationSurfaceV1::Storage {
            assert_eq!(
                row.capability,
                SimulationCapabilityDispositionV1::Owned {
                    owner: fe2o3_kir_sim::SimulationSemanticOwnerV1::TypedMemory,
                    typed_rejections: &[SimulationUnsupportedReasonCodeV1::InertStorage],
                }
            );
            projected.capability = SimulationCapabilityDispositionV1::Unsupported {
                reason: SimulationUnsupportedReasonCodeV1::InertStorage,
            };
            storage += 1;
        }
        assert_eq!(
            serde_json::to_vec(&projected).unwrap(),
            serde_json::to_vec(previous).unwrap()
        );
        added_bytes += serde_json::to_vec(row).unwrap().len() + 1;
        rows += 1;
    }
    assert_eq!((rows, storage), (196, 4));
    let mut pointers = 0;
    for row in matrix
        .pointer_rows
        .iter()
        .filter(|row| row.kir_wire_version == SimulationKirWireVersionV1::V18)
    {
        assert_eq!(row.operation, "restrict_pointer_access");
        let previous = matrix
            .pointer_rows
            .iter()
            .find(|previous| {
                previous.kir_wire_version == SimulationKirWireVersionV1::V12
                    && previous.profile == row.profile
            })
            .unwrap();
        let mut projected = row.clone();
        projected.kir_wire_version = SimulationKirWireVersionV1::V12;
        assert_eq!(
            serde_json::to_vec(&projected).unwrap(),
            serde_json::to_vec(previous).unwrap()
        );
        added_bytes += serde_json::to_vec(row).unwrap().len() + 1;
        pointers += 1;
    }
    assert_eq!(pointers, 4);
    assert_eq!(added_bytes, 35_662); // Four storage rows each add 29 JSON bytes.
    assert_eq!(
        SEMANTIC_CAPABILITY_MATRIX_JSON_BYTES_V1,
        5_061_127 + added_bytes
    );
    for (version, spelling) in [
        (SimulationKirWireVersionV1::V7, "v7"),
        (SimulationKirWireVersionV1::V9, "v9"),
        (SimulationKirWireVersionV1::V10, "v10"),
        (SimulationKirWireVersionV1::V11, "v11"),
        (SimulationKirWireVersionV1::V12, "v12"),
        (SimulationKirWireVersionV1::V16, "v16"),
        (SimulationKirWireVersionV1::V17, "v17"),
        (SimulationKirWireVersionV1::V19, "v19"),
        (SimulationKirWireVersionV1::V20, "v20"),
        (SimulationKirWireVersionV1::V21, "v21"),
        (SimulationKirWireVersionV1::V22, "v22"),
        (SimulationKirWireVersionV1::V18, "v18"),
    ] {
        assert_eq!(serde_json::to_value(version).unwrap(), spelling);
    }
}
