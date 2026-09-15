use super::*;
use crate::collector::exclusive_reference_v1::ExclusiveOutputSourceV1;

fn fixture(root: u8) -> ReferenceEffectIrV1 {
    let mut ir = super::tests::fixture();
    ir.relations[3] = ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
        argument: 2,
        element: ReferenceScalarTypeV1::F32,
        source: ExclusiveOutputSourceV1::test_only_v1(root, 2, ReferenceScalarTypeV1::F32),
    };
    ir
}

#[test]
fn exclusive_output_direct_point_preserves_ir_guards_order_and_bounds() {
    let ir = fixture(3);
    let old = super::tests::fixture();
    assert_eq!(ir.blocks, old.blocks);
    assert_eq!(
        ir.observable_output_writes_v1().unwrap(),
        old.observable_output_writes_v1().unwrap()
    );
    assert_eq!(
        ir.resolved_bounds_checks_with_budget_v1(&mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap(),
        old.resolved_bounds_checks_with_budget_v1(&mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap()
    );
    assert_ne!(ir.canonical_sha256_v1(), old.canonical_sha256_v1());
    assert_ne!(ir.canonical_sha256_v1(), fixture(4).canonical_sha256_v1());
}

#[test]
fn exclusive_output_retains_noescape_single_write_and_coordinate_custody() {
    for mutation in 0..5 {
        let mut ir = fixture(3);
        let expected = if mutation == 4 {
            "exact source point"
        } else {
            "no output reads or escapes"
        };
        match mutation {
            0 => {
                ir.blocks[7].assignments[0].value =
                    ReferenceValueV1::Use(ReferenceOperandV1::Copy(ReferencePlaceV1 {
                        local: 4,
                        projection: Box::default(),
                    }))
            }
            1 => ir.blocks[0].assignments[0].destination.local = 1,
            2 => ir.blocks[7].assignments[0].destination.local = 4,
            3 => {
                let mut writes = ir.blocks[7].assignments.to_vec();
                writes.push(writes[2].clone());
                ir.blocks[7].assignments = writes.into_boxed_slice();
            }
            4 => {
                ir.blocks[7].assignments[2].destination.projection[1] =
                    ReferencePlaceProjectionV1::Index(5)
            }
            _ => unreachable!(),
        }
        let error = ir.observable_output_writes_v1().unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "mutation {mutation}: {error}"
        );
    }
}

#[test]
fn exclusive_output_does_not_claim_unexecuted_frame_or_wrong_length() {
    use crate::reference_effect_bijection_v1::*;
    let ir = fixture(3);
    let writes = ir.observable_output_writes_v1().unwrap();
    let effect = CompilerExtractedGpuOutputEffectV1 {
        output_argument: 2,
        block: 7,
        operation: 0,
        coordinate: writes[0].coordinate.clone(),
        guard: writes[0].guard.clone(),
    };
    establish_reference_effect_bijection_v1(&writes, &[effect.clone()]).unwrap();
    let mut changed = effect.clone();
    changed.guard = ReferencePathPredicateV1::unconditional_v1();
    assert!(matches!(
        establish_reference_effect_bijection_v1(&writes, &[changed]),
        Err(ReferenceEffectBijectionErrorV1::GuardMismatch { .. })
    ));
    let mut changed = ir.clone();
    let ReferenceValueV1::Binary { rhs, .. } = &mut changed.blocks[0].assignments[5].value else {
        unreachable!()
    };
    *rhs = ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits: 1023,
    });
    assert!(matches!(
        establish_reference_effect_bijection_v1(
            &changed.observable_output_writes_v1().unwrap(),
            &[effect]
        ),
        Err(ReferenceEffectBijectionErrorV1::GuardMismatch { .. })
    ));
}

#[test]
fn exclusive_output_custody_uses_existing_cumulative_budget() {
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
        .unwrap();
    let error = validate(&fixture(3), &mut work).unwrap_err();
    assert!(error.to_string().contains("cumulative expression work"));
}
