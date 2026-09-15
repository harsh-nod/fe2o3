use super::*;
use fe2o3_kernel_ir::*;

#[path = "../../../../fe2o3-kernel-ir/src/execution_capability_v1/workgroup_memory_index_v2_tests/fixture.rs"]
mod fixture;

fn state() -> ExecutionStateV1 {
    ExecutionStateV1 {
        block: BlockId(0),
        values: BTreeMap::from([(ValueId(1), SymbolicValueV1::Opaque)]),
        memories: BTreeMap::new(),
        written_roots: BTreeSet::new(),
        events: vec![],
        path: bool_true(),
        visits: BTreeMap::new(),
        next_private_allocation: 0,
        next_workgroup_allocation: 0,
    }
}

fn evaluate(expression: &ExpressionV1, local: [u64; 3]) -> u64 {
    match &expression.kind {
        ExpressionKindV1::Intrinsic(IntrinsicKind::InvocationIndex {
            kind: IndexKind::Local,
            axis,
        }) => {
            local[match axis {
                Axis::X => 0,
                Axis::Y => 1,
                Axis::Z => 2,
            }]
        }
        ExpressionKindV1::Binary {
            operation: BinaryOp::Add,
            lhs,
            rhs,
        } => evaluate(lhs, local) + evaluate(rhs, local),
        ExpressionKindV1::Binary {
            operation: BinaryOp::Multiply,
            lhs,
            rhs,
        } => evaluate(lhs, local) * evaluate(rhs, local),
        _ => u64::try_from(evaluate_constant(expression).expect("physical size constant")).unwrap(),
    }
}

#[test]
fn final_symbolic_scoped_index_is_flat_and_conversion_preserves_expression() {
    let module = fixture::module();
    verify_module(&module).unwrap();
    let operations = fixture::operations(&module);
    for size in [WorkgroupSize::new(64, 1, 1), WorkgroupSize::new(4, 4, 4)] {
        let mut state = state();
        let mut intrinsics = BTreeSet::new();
        let issued = issue(
            fixture::operation_contract(&operations[2]),
            &state,
            &mut intrinsics,
            Some(size),
        )
        .unwrap();
        state.values.insert(ValueId(2), issued);
        let converted = into_disjoint(fixture::operation_contract(&operations[3]), &state).unwrap();
        let SymbolicValueV1::WorkgroupIndex(converted) = converted else {
            panic!("scoped result");
        };
        let SymbolicValueV1::WorkgroupIndex(original) = &state.values[&ValueId(2)] else {
            unreachable!()
        };
        assert_eq!(format!("{converted:?}"), format!("{original:?}"));
        for z in 0..u64::from(size.z) {
            for y in 0..u64::from(size.y) {
                for x in 0..u64::from(size.x) {
                    assert_eq!(
                        evaluate(&converted, [x, y, z]),
                        (z * u64::from(size.y) + y) * u64::from(size.x) + x
                    );
                }
            }
        }
    }
}

#[test]
fn final_symbolic_conversion_rejects_plain_scalar_and_missing_source() {
    let module = fixture::module();
    let operations = fixture::operations(&module);
    let mut state = state();
    for size in [
        WorkgroupSize::new(0, 1, 1),
        WorkgroupSize::new(u32::MAX, u32::MAX, 2),
    ] {
        assert!(
            issue(
                fixture::operation_contract(&operations[2]),
                &state,
                &mut BTreeSet::new(),
                Some(size)
            )
            .is_err()
        );
    }
    assert!(into_disjoint(fixture::operation_contract(&operations[3]), &state).is_err());
    state
        .values
        .insert(ValueId(2), SymbolicValueV1::Scalar(index_zero()));
    assert!(into_disjoint(fixture::operation_contract(&operations[3]), &state).is_err());
    assert!(
        issue(
            fixture::operation_contract(&operations[2]),
            &state,
            &mut BTreeSet::new(),
            None
        )
        .is_err()
    );
}
