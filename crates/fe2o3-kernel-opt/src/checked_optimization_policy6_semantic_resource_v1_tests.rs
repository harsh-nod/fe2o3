use super::*;
use fe2o3_kernel_ir::*;
use std::mem::size_of_val;

#[test]
fn actual_integer_execution_replay_prepays_its_additional_fixed_record() {
    use crate::checked_load_forwarding_v1::tests::{STORAGE, WORK, fixture};
    let mut preparation_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut preparation = Budget::new(&mut preparation_work, STORAGE);
    let (input, input_size) =
        Owner::from_module_ref_with_verification_budget_v12(&fixture(), &mut preparation).unwrap();
    preparation
        .reserve_storage(input_size.retained_storage())
        .unwrap();
    let p5 =
        crate::optimize_checked_canonical_kernel_ir_policy5_v1(&input, &mut preparation).unwrap();
    preparation.reserve_storage(p5.retained_storage()).unwrap();
    let checked =
        crate::continue_checked_canonical_kernel_ir_policy6_v1(&input, p5, &mut preparation)
            .unwrap();
    preparation
        .reserve_storage(checked.retained_storage())
        .unwrap();
    let floor = 37 + input_size.retained_storage() + checked.retained_storage();
    let required = INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1;
    let run = |work_limit, scratch| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, floor + scratch);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            check_integer_execution(&checked, budget)
        });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if scratch == required {
            assert_eq!(budget.peak_storage(), floor + required);
        }
        (result, budget.work())
    };
    let (result, spent) = run(WORK, required);
    result.unwrap();
    assert!(spent > required);
    assert!(matches!(
        run(WORK, required - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert!(matches!(
        run(WORK, 0).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(run(WORK, required - 1).1, 0);
    assert_eq!(run(spent, required).1, spent);
    run(spent, required).0.unwrap();
    assert!(matches!(
        run(spent - 1, required).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
}

#[test]
fn all_nine_exact_occurrence_axes_are_checked_without_portable_seal_construction() {
    // Equality-helper controls only: these inert rows are not a semantic receipt.
    let f = CanonicalKirFunctionCoordinateV1(0);
    let block = CanonicalKirBlockCoordinateV1 {
        function: f,
        block: 0,
    };
    let op = CanonicalKirOperationCoordinateV1 {
        block,
        operation: 0,
    };
    let definition = CanonicalKirDefinitionCoordinateV1::Result {
        operation: op,
        result: 0,
    };
    let use_ = CanonicalKirUseCoordinateV1::OperationOperand {
        operation: op,
        operand: 0,
    };
    let edge = CanonicalKirEdgeCoordinateV1 {
        source: block,
        successor: 0,
    };
    let argument = CanonicalKirEdgeArgumentCoordinateV1 { edge, argument: 0 };
    let range = CanonicalKirTransitionRangeV1 { start: 0, len: 1 };
    let functions = [CanonicalKirFunctionTransitionV1 {
        input: f,
        output: f,
    }];
    let blocks = [CanonicalKirBlockTransitionV1 {
        output: block,
        segments: range,
    }];
    let segments = [CanonicalKirBlockSegmentV1 {
        input: block,
        connector: None,
    }];
    let operations = [CanonicalKirOperationTransitionV1 {
        output: op,
        origin: CanonicalKirOperationOriginV1::Retained(op),
    }];
    let definitions = [CanonicalKirDefinitionTransitionV1 {
        input: definition,
        outputs: range,
    }];
    let definition_outputs = [CanonicalKirDefinitionDescendantV1 {
        output: definition,
        kind: CanonicalKirDefinitionDescendantKindV1::Retained,
    }];
    let uses = [CanonicalKirUseTransitionV1 {
        input: use_,
        output: use_,
    }];
    let edges = [CanonicalKirEdgeTransitionV1 {
        input: edge,
        output: edge,
    }];
    let edge_arguments = [CanonicalKirEdgeArgumentTransitionV1 {
        input: argument,
        output: argument,
    }];
    let candidate = Candidate {
        functions: &functions,
        blocks: &blocks,
        segments: &segments,
        operations: &operations,
        definitions: &definitions,
        definition_outputs: &definition_outputs,
        uses: &uses,
        edges: &edges,
        edge_arguments: &edge_arguments,
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 19);
    budget.reserve_storage(19).unwrap();
    let expected = 9 + 2
        * (size_of_val(&functions)
            + size_of_val(&blocks)
            + size_of_val(&segments)
            + size_of_val(&operations)
            + size_of_val(&definitions)
            + size_of_val(&definition_outputs)
            + size_of_val(&uses)
            + size_of_val(&edges)
            + size_of_val(&edge_arguments));
    exact_occurrences(candidate, candidate, &mut budget).unwrap();
    assert_eq!(budget.work(), expected);
    macro_rules! missing {
        ($($field:ident),+ $(,)?) => { $(
            assert!(matches!(exact_occurrences(candidate, Candidate { $field: &[], ..candidate },
                &mut budget), Err(Error::Occurrences)), stringify!($field));
            let doubled = [candidate.$field[0]; 2];
            assert!(matches!(exact_occurrences(candidate, Candidate { $field: &doubled, ..candidate },
                &mut budget), Err(Error::Occurrences)), stringify!($field));
        )+ };
    }
    missing!(
        functions,
        blocks,
        segments,
        operations,
        definitions,
        definition_outputs,
        uses,
        edges,
        edge_arguments
    );
    let mut changed = functions;
    changed[0].input.0 = 99;
    assert!(matches!(
        exact_occurrences(
            candidate,
            Candidate {
                functions: &changed,
                ..candidate
            },
            &mut budget
        ),
        Err(Error::Occurrences)
    ));
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.peak_storage(), 19);
}

#[test]
fn row_comparison_exact_and_short_work_never_allocates_or_refunds_work() {
    for (limit, success) in [(17, true), (16, false)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        let result = same_rows(&[4_u64], &[4_u64], &mut budget);
        assert_eq!(result.is_ok(), success);
        if success {
            assert!(result.unwrap());
        }
        assert_eq!(budget.work(), if success { 17 } else { 1 });
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.peak_storage(), 19);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(17);
    let mut budget = Budget::new(&mut work, 0);
    assert!(!same_rows(&[4_u64], &[5_u64], &mut budget).unwrap());
}

#[test]
fn cleanup_defers_hostile_payload_drop_and_preserves_work_and_foreign_ledgers() {
    struct Hostile;
    impl Drop for Hostile {
        fn drop(&mut self) {
            std::panic::panic_any("payload drop");
        }
    }
    for hostile in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, 200);
        budget.reserve_storage(19).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            scoped::<()>(&mut budget, |budget| {
                budget.reserve_storage(71)?;
                budget.charge_work(13)?;
                if hostile {
                    std::panic::panic_any(Hostile);
                }
                std::panic::panic_any("normal payload");
            })
        }));
        if hostile {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(outcome.unwrap(), Err(Error::Panicked)));
        }
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), 13);
    }
    for panics in [false, true] {
        let replacement = Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(100)));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, 200);
        budget.reserve_storage(19).unwrap();
        assert!(matches!(
            scoped::<()>(&mut budget, |budget| {
                *budget = Budget::new(replacement, 200);
                budget.reserve_storage(91)?;
                if panics {
                    std::panic::panic_any("foreign ledger");
                }
                Ok(())
            }),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), 91);
    }
}
