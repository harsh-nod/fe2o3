use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn id(n: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(n)
}
fn aggregate(
    n: u8,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
    bytes: u64,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([n; 32]),
        SemanticLayoutIdentityV1::from_sha256([n + 10; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(bytes),
            4,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}
fn table() -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([11; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        aggregate(2, vec![id(0)], vec![0], 4),
        aggregate(3, vec![id(1)], vec![0], 4),
        aggregate(4, vec![id(2)], vec![0], 4)
            .with_rust_type_kind(SemanticRustTypeKindV1::AtomicU32),
        aggregate(5, vec![id(0), id(3)], vec![0, 4], 8),
    ]
}
fn field(n: u32, result: u32) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(n), id(result)).unwrap()
}
fn observe(
    types: &[SemanticTypeDeclV1],
    root: u32,
    path: &[SemanticProjectionV1],
    expected: u32,
) -> Option<SourceAtomicStoragePathV41> {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    source_atomic_storage_path_shape_v41(types, id(root), path, id(expected), &mut budget).unwrap()
}
#[test]
fn atomic_storage_path_v41_exact_three_original_edges_and_single_next_field() {
    let types = table();
    let full = [field(0, 2), field(0, 1), field(0, 0)];
    for depth in 0..=3 {
        let current = [3, 2, 1, 0][depth];
        let fact = observe(&types, 3, &full[..depth], current).unwrap();
        assert_eq!(fact.wrapper, id(3));
        assert_eq!(fact.depth, depth as u8);
        assert_eq!(fact.current_type(), id(current));
        assert_eq!(fact.is_scalar_leaf(), depth == 3);
        for output in 0..=4 {
            assert_eq!(
                fact.next_field(id(output)),
                if depth < 3 && output == [2, 1, 0][depth] {
                    Some(full[depth])
                } else {
                    None
                }
            );
        }
    }
}
#[test]
fn atomic_storage_path_v41_enclosing_original_field_is_retained() {
    let path = [field(1, 3), field(0, 2), field(0, 1), field(0, 0)];
    let fact = observe(&table(), 4, &path, 0).unwrap();
    assert_eq!(fact.wrapper, id(3));
    assert!(fact.is_scalar_leaf());
    assert!(observe(&table(), 4, &[field(0, 3)], 3).is_none());
}
#[test]
fn atomic_storage_path_v41_independent_cells_scalars_and_lookalikes_are_not_roots() {
    let mut types = table();
    for (root, path) in [
        (2, vec![field(0, 1), field(0, 0)]),
        (1, vec![field(0, 0)]),
        (0, vec![]),
    ] {
        assert!(observe(&types, root, &path, 0).is_none());
    }
    types[3] = types[3]
        .clone()
        .with_rust_type_kind(SemanticRustTypeKindV1::Ordinary);
    assert!(observe(&types, 3, &[field(0, 2), field(0, 1), field(0, 0)], 0).is_none());
}
#[test]
fn atomic_storage_path_v41_skips_reorders_reverse_edges_and_other_projection_kinds_refuse() {
    let types = table();
    for path in [
        vec![field(0, 0)],
        vec![field(0, 1)],
        vec![field(1, 2)],
        vec![field(0, 2), field(0, 0)],
        vec![field(0, 2), field(0, 3)],
        vec![field(0, 2), field(0, 1), field(0, 0), field(0, 0)],
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, id(2)).unwrap()],
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, id(2)).unwrap()],
    ] {
        let expected = path.last().unwrap().result_type().index();
        assert!(observe(&types, 3, &path, expected).is_none(), "{path:?}");
    }
    assert!(observe(&types, 3, &[field(0, 2)], 1).is_none());
    assert!(observe(&types, 99, &[], 99).is_none());
}
#[test]
fn atomic_storage_path_v41_every_original_nominal_chain_fact_is_rechecked() {
    for fault in 0..5 {
        let mut types = table();
        match fault {
            0 => {
                types[3] = types[3]
                    .clone()
                    .with_rust_type_kind(SemanticRustTypeKindV1::AtomicI32)
            }
            1 => types[1] = aggregate(2, vec![id(0)], vec![1], 8),
            2 => types[2] = aggregate(3, vec![id(0)], vec![0], 4),
            3 => types[1] = aggregate(2, vec![id(3)], vec![0], 4),
            4 => {
                types[2] = types[2]
                    .clone()
                    .with_rust_type_kind(SemanticRustTypeKindV1::AtomicU32)
            }
            _ => unreachable!(),
        }
        assert!(observe(&types, 3, &[field(0, 2), field(0, 1), field(0, 0)], 0).is_none());
    }
}
#[test]
fn atomic_storage_path_v41_exact_work_and_one_short_fail_without_a_partial_fact() {
    let types = table();
    let path = [field(0, 2), field(0, 1), field(0, 0)];
    let exact = 40 + 4 * (path.len() + 1);
    for short in [false, true] {
        let mut work =
            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact - usize::from(short));
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let result = source_atomic_storage_path_shape_v41(&types, id(3), &path, id(0), &mut budget);
        if short {
            assert!(matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual()==exact&&error.limit()==exact-1));
        } else {
            assert!(result.unwrap().unwrap().is_scalar_leaf());
            assert_eq!(budget.work(), exact);
        }
    }
}

#[test]
fn atomic_cast_descendant_v41_rechecks_each_forward_edge_without_projected_skips() {
    let types = table();
    let chain = [3, 2, 1, 0];
    let full = [field(0, 2), field(0, 1), field(0, 0)];
    for depth in 0..=3 {
        let path = observe(&types, 3, &full[..depth], chain[depth]).unwrap();
        for target in 0..=4 {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let result =
                source_atomic_cast_descendant_v41(&types, path, id(target), &mut budget).unwrap();
            let expected = chain
                .iter()
                .enumerate()
                .skip(depth)
                .find(|(_, value)| **value == target)
                .map(|(position, _)| position as u8);
            assert_eq!(result.map(|row| row.depth), expected, "{depth}->{target}");
            if let Some(result) = result {
                assert_eq!(result.wrapper, path.wrapper);
                assert_eq!(result.children, path.children);
                assert_eq!(result.current_type(), id(target));
            }
        }
    }
    // Actual source projection lists still require every explicit field edge.
    assert!(observe(&types, 3, &[field(0, 2), field(0, 0)], 0).is_none());
}

#[test]
fn atomic_cast_descendant_v41_intermediate_layout_identity_and_type_mutations_refuse() {
    let original = table();
    let path = observe(&original, 3, &[field(0, 2)], 2).unwrap();
    for fault in 0..9 {
        let mut types = original.clone();
        let mut changed = path;
        match fault {
            0 => changed.children.swap(0, 1),
            1 => changed.depth = 4,
            2 => changed.wrapper = id(4),
            3 => types[1] = aggregate(2, vec![id(0)], vec![1], 8),
            4 => types[2] = aggregate(3, vec![id(0)], vec![0], 4),
            5 => types[1] = aggregate(2, vec![id(3)], vec![0], 4),
            6 => {
                types[3] = types[3]
                    .clone()
                    .with_rust_type_kind(SemanticRustTypeKindV1::AtomicI32)
            }
            7 => {
                types[2] = types[2]
                    .clone()
                    .with_rust_type_kind(SemanticRustTypeKindV1::AtomicU32)
            }
            8 => types[1] = aggregate(2, vec![id(0), id(0)], vec![0, 0], 4),
            _ => unreachable!(),
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        assert!(
            source_atomic_cast_descendant_v41(&types, changed, id(0), &mut budget)
                .unwrap()
                .is_none(),
            "{fault}"
        );
    }
}

#[test]
fn atomic_cast_descendant_v41_exact_55_and_one_under_charge_publish_no_partial_path() {
    let types = table();
    let path = observe(&types, 3, &[field(0, 2)], 2).unwrap();
    // Source charges: fixed4 + exact nominal-chain validation40 + child IDs3
    // + two forward edges4 each. This is not measured from a successful run.
    for (limit, accepted, success) in [(55, 55, true), (54, 51, false), (0, 0, false)] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let result = source_atomic_cast_descendant_v41(&types, path, id(0), &mut budget);
        if success {
            assert_eq!(result.unwrap().unwrap().depth, 3);
        } else {
            assert!(matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.limit() == limit
                        && error.actual() == if limit == 0 { 4 } else { 55 }));
        }
        assert_eq!(budget.work(), accepted);
    }
}
