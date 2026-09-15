// Inert type/contract components. Only the unchanged actual AMD callback can
// establish source replay and the complete typed custody path.
fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn types() -> Vec<SemanticTypeDeclV1> {
    (0..10_u8)
        .map(|index| {
            let pointee = match index {
                1 => Some(ty(0)),
                4 | 8 | 9 => Some(ty(3)),
                7 => Some(ty(6)),
                _ => None,
            };
            let shape = if let Some(pointee) = pointee {
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        if index == 9 {
                            SemanticPointerKindV1::Raw
                        } else {
                            SemanticPointerKindV1::Reference
                        },
                        if index == 8 {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                )
            } else {
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap())
            };
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([index + 1; 32]),
                SemanticTypeLayoutV1::new(
                    Some(if pointee.is_some() { 8 } else { 0 }),
                    if pointee.is_some() { 8 } else { 1 },
                )
                .unwrap(),
                shape,
            )
        })
        .collect()
}

fn issue(
    format: SemanticGfx950LdsTransposeFormatV1,
    reference: u32,
) -> SemanticGfx950TransposeContractV1 {
    SemanticGfx950TransposeContractV1::new(
        SemanticGfx950TransposeOperationV1::Issue {
            partition_reference: ty(reference),
            partition: ty(3),
            tile: ty(5),
        },
        format,
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        None,
    )
    .unwrap()
}

fn derive(width: u32, partition_width: u32) -> SemanticSubgroupPartitionOperationV1 {
    SemanticSubgroupPartitionOperationV1::Derive {
        subgroup_reference: ty(1),
        subgroup: ty(0),
        epoch: ty(2),
        partition: ty(3),
        width,
        partition_width,
    }
}

#[test]
fn transpose_issue_accepts_only_its_wave64_wave16_source_geometry() {
    let types = types();
    for format in [
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
    ] {
        for width in [0, 16, 32, 64, 128] {
            for partition_width in [0, 1, 2, 4, 8, 16, 32, 64, 128] {
                assert_eq!(
                    transpose_partition_source_types_match(
                        &types,
                        issue(format, 4),
                        derive(width, partition_width),
                        &[],
                        ty(3)
                    ),
                    (width, partition_width) == (64, 16),
                    "format={format:?}, width={width}, partition_width={partition_width}",
                );
            }
        }
    }
}

#[test]
fn transpose_issue_rejects_substituted_owned_and_shared_source_edges() {
    let types = types();
    let format = SemanticGfx950LdsTransposeFormatV1::Fp4E2M1;
    let good = issue(format, 4);
    let original = derive(64, 16);
    assert!(transpose_partition_source_types_match(
        &types,
        good,
        original,
        &[],
        ty(3)
    ));
    for reference in [7, 8, 9] {
        assert!(!transpose_partition_source_types_match(
            &types,
            issue(format, reference),
            original,
            &[],
            ty(3)
        ));
    }
    for target in [0, 1, 4, 5, 6] {
        assert!(!transpose_partition_source_types_match(
            &types,
            good,
            original,
            &[],
            ty(target)
        ));
    }
    for changed in [
        SemanticSubgroupPartitionOperationV1::Derive {
            subgroup_reference: ty(1),
            subgroup: ty(0),
            epoch: ty(2),
            partition: ty(6),
            width: 64,
            partition_width: 16,
        },
        SemanticSubgroupPartitionOperationV1::Derive {
            subgroup_reference: ty(1),
            subgroup: ty(6),
            epoch: ty(2),
            partition: ty(3),
            width: 64,
            partition_width: 16,
        },
        SemanticSubgroupPartitionOperationV1::Derive {
            subgroup_reference: ty(7),
            subgroup: ty(0),
            epoch: ty(2),
            partition: ty(3),
            width: 64,
            partition_width: 16,
        },
        SemanticSubgroupPartitionOperationV1::ReduceSumF32 {
            partition_reference: ty(4),
            partition: ty(3),
            element: ty(6),
            width: 64,
            partition_width: 16,
        },
    ] {
        assert!(!transpose_partition_source_types_match(
            &types,
            good,
            changed,
            &[],
            ty(3)
        ));
    }
    let projected =
        [SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(3)).unwrap()];
    assert!(!transpose_partition_source_types_match(
        &types,
        good,
        original,
        &projected,
        ty(3)
    ));
}

#[test]
fn transpose_other_terminal_cannot_select_partition_geometry() {
    let stage = SemanticGfx950TransposeContractV1::new(
        SemanticGfx950TransposeOperationV1::Stage {
            input_tile: ty(0),
            output_tile: ty(1),
            view_reference: ty(2),
            view: ty(3),
            index: ty(4),
            global_reference: ty(5),
            global: ty(6),
        },
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        None,
    )
    .unwrap();
    assert!(!transpose_partition_source_types_match(
        &types(),
        stage,
        derive(64, 16),
        &[],
        ty(3)
    ));
}
