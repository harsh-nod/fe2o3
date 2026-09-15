use super::*;

#[path = "publish_abi_tests.rs"]
mod publish_abi_tests;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1(index)
}
fn identity(value: u8) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1([value; 32])
}

fn operations() -> [SemanticGfx950TransposeOperationV1; 4] {
    use SemanticGfx950TransposeOperationV1 as T;
    [
        T::Issue {
            partition_reference: id(0),
            partition: id(1),
            tile: id(2),
        },
        T::Stage {
            input_tile: id(2),
            output_tile: id(3),
            view_reference: id(4),
            view: id(5),
            index: id(6),
            global_reference: id(7),
            global: id(8),
        },
        T::Publish {
            input_tile: id(3),
            input_workgroup: id(9),
            transition: id(10),
            output_workgroup: id(11),
            output_tile: id(12),
        },
        T::Read {
            tile: id(12),
            lane_reference: id(13),
            lane: id(14),
            fragment: id(15),
            registers: id(16),
            word: id(17),
        },
    ]
}

fn transpose(index: usize) -> SemanticGfx950TransposeContractV1 {
    SemanticGfx950TransposeContractV1::new(
        operations()[index],
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        identity(if index == 3 { 22 } else { 21 }),
        (index == 2).then_some(identity(22)),
    )
    .unwrap()
}

fn signature(index: usize) -> SemanticExecutionCapabilitySignatureV1 {
    let (arguments, output): (&[u32], _) = match index {
        0 => (&[0], 2),
        1 => (&[2, 4, 6, 6], 3),
        2 => (&[3, 9], 10),
        3 => (&[12, 13], 15),
        _ => unreachable!(),
    };
    SemanticExecutionCapabilitySignatureV1::new(
        &arguments.iter().copied().map(id).collect::<Vec<_>>(),
        id(output),
    )
    .unwrap()
}

fn contract(index: usize) -> SemanticExecutionCapabilityContractV1 {
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1(0),
        SemanticKernelBindingIdentityV1([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1([2; 32]),
        identity(3),
        SemanticKernelCapabilityTargetBrandIdentityV1([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1([6; 32]),
    )
    .unwrap();
    SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose(index)),
        signature(index),
        provenance,
        identity(23),
        identity(if index == 3 { 25 } else { 24 }),
        (index == 2).then_some(identity(25)),
        SemanticFunctionIdentityV1([index as u8 + 30; 32]),
    )
    .unwrap()
}

#[test]
fn transpose_source_signatures_preserve_consumed_values_and_borrow_edges() {
    for i in 0..4 {
        let t = transpose(i);
        assert!(t.signature_matches(signature(i)));
        assert!(!t.signature_matches(
            SemanticExecutionCapabilitySignatureV1::new(&[], signature(i).output()).unwrap()
        ));
        assert!(
            !t.signature_matches(
                SemanticExecutionCapabilitySignatureV1::new(
                    &signature(i).arguments().collect::<Vec<_>>(),
                    id(100)
                )
                .unwrap()
            )
        );
        assert_eq!(contract(i).obligations().bits(), t.obligations());
    }
    assert_eq!(transpose(0).shared_reference_pair(0), Some((id(0), id(1))));
    assert_eq!(transpose(1).shared_reference_pair(1), Some((id(4), id(5))));
    assert_eq!(
        transpose(3).shared_reference_pair(1),
        Some((id(13), id(14)))
    );
    for i in 1..4 {
        assert_eq!(transpose(i).shared_reference_pair(0), None);
    }
    for i in 0..4 {
        for arg in 2..4 {
            assert_eq!(transpose(i).shared_reference_pair(arg), None);
        }
    }
}

#[test]
fn transpose_requires_nonzero_full_brands_and_only_publish_advances_epoch() {
    for (i, operation) in operations().into_iter().enumerate() {
        assert_eq!(
            SemanticGfx950TransposeContractV1::new(
                operation,
                transpose(i).format(),
                identity(0),
                (i == 2).then_some(identity(22))
            ),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
        let bad_next = if i == 2 { None } else { Some(identity(22)) };
        assert_eq!(
            SemanticGfx950TransposeContractV1::new(
                operation,
                transpose(i).format(),
                identity(21),
                bad_next
            ),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
        let c = contract(i);
        let wrong_epoch = if i == 2 { None } else { Some(identity(90)) };
        assert_eq!(
            SemanticExecutionCapabilityContractV1::new(
                c.operation(),
                c.signature(),
                c.provenance(),
                c.workgroup_brand().unwrap(),
                c.epoch_before().unwrap(),
                wrong_epoch,
                c.source_identity()
            ),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
        assert_eq!(
            SemanticExecutionCapabilityContractV1::new_kernel_scoped(
                c.operation(),
                c.signature(),
                c.provenance(),
                c.source_identity()
            ),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
}

#[test]
fn transpose_rejects_aliasing_or_unbounded_type_edges() {
    for bad in [id(0), id(HARD_MAX_TYPES_V1 as u32)] {
        let operation = SemanticGfx950TransposeOperationV1::Issue {
            partition_reference: id(0),
            partition: id(1),
            tile: bad,
        };
        assert_eq!(
            SemanticGfx950TransposeContractV1::new(
                operation,
                transpose(0).format(),
                identity(21),
                None
            ),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
}

#[test]
fn transpose_claim_roster_joins_each_tile_phase_and_both_workgroup_epochs() {
    let mut claims = IntrinsicCapabilityClaimsV1::default();
    for i in 0..4 {
        assert!(claims.record_execution_contract(contract(i)));
    }
    assert_eq!(claims.transpose_tiles.len(), 3);
    assert_eq!(claims.borrowed_workgroups.len(), 2);
    for field in 0..4 {
        let mut claims = IntrinsicCapabilityClaimsV1::default();
        assert!(claims.record_execution_contract(contract(0)));
        let mut changed = contract(1);
        match field {
            0 => changed.epoch_before = Some(identity(90)),
            1 => changed.workgroup_brand = Some(identity(90)),
            2 => {
                let mut t = transpose(1);
                t.subgroup_brand = identity(90);
                changed.operation = SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t);
            }
            3 => {
                let mut t = transpose(1);
                t.format = SemanticGfx950LdsTransposeFormatV1::Fp8E4M3;
                changed.operation = SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t);
            }
            _ => unreachable!(),
        }
        assert!(
            !claims.record_execution_contract(changed),
            "changed tile claim axis {field}"
        );
    }
}

#[test]
fn transpose_obligations_do_not_claim_partition_only_publish_or_initialized_issuance() {
    use SemanticExecutionSafetyObligationsV1 as O;
    assert_ne!(transpose(0).obligations() & O::DISJOINT_LDS_ALLOCATION, 0);
    assert_eq!(transpose(0).obligations() & O::INITIALIZATION, 0);
    for i in 1..4 {
        assert_ne!(transpose(i).obligations() & O::INITIALIZATION, 0);
    }
    assert_ne!(transpose(2).obligations() & O::WORKGROUP_CONVERGENCE, 0);
    assert_ne!(transpose(2).obligations() & O::MEMORY_MODEL, 0);
    assert_ne!(transpose(3).obligations() & O::LIFETIME_VALIDITY, 0);
}
