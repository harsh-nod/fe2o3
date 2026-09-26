use super::*;

fn many_calls(per_root: u8) -> ProductionPreRankedKirOwnerV1 {
    assert!((1..=32).contains(&per_root));
    let seed = cpc_shared_source();
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    for root in semantic.roots() {
        let ordinal = root.index() as usize;
        let function = &semantic.functions()[ordinal];
        let template = function
            .blocks()
            .iter()
            .find_map(|block| match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => Some(call),
                _ => None,
            })
            .expect("genuine original helper call");
        assert!(template.arguments().is_empty());
        let destination = template.destination().unwrap();
        let mut blocks = Vec::new();
        for n in 0..per_root {
            let call = SemanticDirectCallV1::new_callable(
                template.callee(),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    destination.place().clone(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(u32::from(n) + 1),
                    ),
                )),
                template.unwind(),
            )
            .unwrap();
            blocks.push(body(100 + n, vec![], SemanticTerminatorKindV1::Call(call)));
        }
        blocks.push(body(
            100 + per_root,
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
        functions[ordinal] = copy_function(function, function.locals().to_vec(), blocks);
    }
    rebuild(&seed, semantic.types().to_vec(), functions)
}

#[test]
fn indexed_many_calls_keep_both_roots_private_aliases_and_real_final_nine() {
    for count in [1, 8, 32] {
        let source = many_calls(count);
        let roots = source.executable().module().kernels.clone();
        assert_eq!(roots.len(), 2);
        assert_eq!(
            cpc_counts(source.executable()),
            [1, 0, 1, 1, 2 * usize::from(count)]
        );
        let owner = cpc_prepare(source);
        let before = snapshots(&owner);
        cpc_run(&owner, |view, budget| {
            cpc_reports(&owner, view, budget)?;
            assert_eq!(view.memory_census(budget)?, [1, 2]);
            assert_eq!(
                view.final_inventory(budget)?.owner().module().kernels,
                roots
            );
            assert_eq!(view.calls(budget)?.len(), 2 * usize::from(count));
            for root in [
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(2),
            ] {
                assert_eq!(
                    view.calls(budget)?
                        .iter()
                        .filter(|row| row.root() == root)
                        .count(),
                    usize::from(count)
                );
            }
            for n in 0..view.operation_count(budget)? {
                let row = view.operation(n, budget)?;
                assert!(row.current().is_some());
                assert!(row.removal().is_none());
                assert_eq!(
                    view.source_aliases(budget)?
                        .iter()
                        .filter(|a| a.operation() == n)
                        .count(),
                    if row.kind() == PrivateKind::Call {
                        1
                    } else {
                        2
                    }
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(snapshots(&owner), before);
    }
}

#[test]
fn indexed_real_source_rechecks_have_roster_plus_logarithmic_call_bounds() {
    for count in [1, 8, 32] {
        let owner = cpc_prepare(many_calls(count));
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        canonical_assertion_v1::read_test_private_index_v1(&owner, None, &mut budget, 255).unwrap();
        assert_eq!(budget.storage(), floor);
        canonical_assertion_v1::read_test_private_index_v1(&owner, None, &mut budget, 254).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn missing_duplicate_foreign_and_stale_private_indexes_refuse_exact_source() {
    let owner = cpc_prepare(many_calls(3));
    let donor = cpc_prepare(many_calls(3));
    assert_eq!(snapshots(&owner), snapshots(&donor));
    let before = snapshots(&owner);
    for fault in 0..16 {
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.retained_storage_floor_v1() + donor.retained_storage_floor_v1() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let result = canonical_assertion_v1::read_test_private_index_v1(
            &owner,
            Some(&donor),
            &mut budget,
            fault,
        );
        assert!(
            matches!(result, Err(HistoryError::Invalid(_))),
            "fault {fault}: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(snapshots(&owner), before);
    }
    cpc_run(&owner, |view, budget| cpc_reports(&owner, view, budget)).unwrap();
}
