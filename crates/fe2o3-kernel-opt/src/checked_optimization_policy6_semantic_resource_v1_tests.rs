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

type CauseP3 = crate::CanonicalPolicy3ExecutionReceiptErrorV1;
type CauseP4 = crate::CanonicalPolicy4ExecutionReceiptErrorV1;
type CauseP5 = crate::CanonicalPolicy5SemanticErrorV1;
type CauseSemantic = crate::KernelIrCheckedOptimizationReceiptErrorV1;
type CauseCodec = CanonicalKirTransitionReceiptErrorV1;
type CauseClaim3 = fe2o3_pliron::Policy3ExecutionClaimErrorV1;
type CauseLoad = fe2o3_kernel_analysis::CanonicalKirLoadForwardingErrorV1;
type CauseStore = fe2o3_kernel_analysis::CanonicalKirStoreForwardingErrorV1;
type CauseInventory = fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1;
type CauseMemory = fe2o3_kernel_analysis::CanonicalKirMemorySsaErrorV1;
type CauseTransition = fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1;

fn semantic_cause_borrow<T: std::error::Error + 'static>(
    parent: &dyn std::error::Error,
    child: &T,
) {
    assert!(std::ptr::eq(
        parent.source().unwrap().downcast_ref::<T>().unwrap(),
        child
    ));
}
fn semantic_cause_walk(
    mut error: &(dyn std::error::Error + 'static),
    depth: usize,
    expected: Resource,
) {
    for _ in 0..depth {
        error = error.source().unwrap();
    }
    let resource = error.downcast_ref::<Resource>().unwrap();
    assert_eq!(*resource, expected);
    match resource {
        Resource::Work(child) => semantic_cause_borrow(resource, child),
        Resource::Storage(child) => semantic_cause_borrow(resource, child),
        Resource::Allocation | Resource::Accounting | Resource::Arithmetic => {
            assert!(std::error::Error::source(resource).is_none())
        }
    }
}
fn semantic_cause_cases() -> [Resource; 5] {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = Budget::new(&mut work, 5);
    let denied_work = budget.charge_work(4).unwrap_err();
    let denied_storage = budget.reserve_storage(6).unwrap_err();
    let accounting = budget.release_storage(1).unwrap_err();
    assert_eq!(accounting, Resource::Accounting);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(budget.failed_storage(), Some(6));
    drop(budget);
    assert_eq!(work.failed_work(), Some(4));
    [
        denied_work,
        denied_storage,
        accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ]
}

#[test]
fn semantic_cause_links_borrow_every_immediate_typed_child() {
    // Diagnostic envelopes only, not reconstructed execution receipts.
    for resource in semantic_cause_cases() {
        macro_rules! check {
            ($ty:ident, $variant:ident, $child:expr, $depth:expr) => {{
                let error = $ty::$variant($child);
                let $ty::$variant(child) = &error else {
                    unreachable!()
                };
                semantic_cause_borrow(&error, child);
                semantic_cause_walk(&error, $depth, resource);
            }};
        }
        check!(CauseP3, Resource, resource, 1);
        check!(CauseP3, ExecutionClaim, CauseClaim3::Resource(resource), 2);
        check!(CauseP3, Codec, CauseCodec::Resource(resource), 2);
        check!(CauseP3, Semantic, CauseSemantic::Resource(resource), 2);
        check!(CauseP4, Resource, resource, 1);
        check!(CauseP4, Policy3, CauseP3::Resource(resource), 2);
        check!(CauseP4, Forwarding, CauseStore::Resource(resource), 2);
        check!(CauseP5, Resource, resource, 1);
        check!(CauseP5, Policy4, CauseP4::Resource(resource), 2);
        check!(CauseP5, Forwarding, CauseLoad::Resource(resource), 2);
        check!(Error, Resource, resource, 1);
        check!(
            Error,
            Claim,
            IntegerContinuationClaimErrorV1::Resource(resource),
            2
        );
        check!(Error, Policy5, CauseP5::Resource(resource), 2);
        check!(Error, Transition, CauseSemantic::Resource(resource), 2);
        check!(
            Error,
            Map,
            KirOptimizationMapErrorV12::Resources(resource),
            2
        );
    }
}

#[test]
fn semantic_cause_source_only_walks_reach_deep_shared_descendants() {
    // These complete type-level chains are not evidence that every source path ran.
    for resource in semantic_cause_cases() {
        for semantic in [
            CauseSemantic::Inventory(CauseInventory::Resource(resource)),
            CauseSemantic::Transition(CauseTransition::Resource(resource)),
            CauseSemantic::Codec(CauseCodec::Resource(resource)),
        ] {
            let error = Error::Policy5(CauseP5::Policy4(CauseP4::Policy3(CauseP3::Semantic(
                semantic,
            ))));
            semantic_cause_walk(&error, 6, resource);
        }
        for policy3 in [
            CauseP3::ExecutionClaim(CauseClaim3::Resource(resource)),
            CauseP3::Codec(CauseCodec::Resource(resource)),
            CauseP3::Semantic(CauseSemantic::Resource(resource)),
        ] {
            let error = Error::Policy5(CauseP5::Policy4(CauseP4::Policy3(policy3)));
            semantic_cause_walk(&error, 5, resource);
        }
        for forwarding in [
            CauseLoad::Inventory(CauseInventory::Resource(resource)),
            CauseLoad::MemorySsa(CauseMemory::Resource(resource)),
        ] {
            let error = Error::Policy5(CauseP5::Forwarding(forwarding));
            semantic_cause_walk(&error, 4, resource);
        }
        let error = Error::Policy5(CauseP5::Policy4(CauseP4::Forwarding(
            CauseStore::MemorySsa(CauseMemory::Resource(resource)),
        )));
        semantic_cause_walk(&error, 5, resource);
    }
}

#[test]
fn semantic_cause_links_preserve_nonresource_children_and_all_markers() {
    macro_rules! child {
        ($ty:ident, $variant:ident, $value:expr) => {{
            let error = $ty::$variant($value);
            let $ty::$variant(child) = &error else {
                unreachable!()
            };
            semantic_cause_borrow(&error, child);
            assert!(std::error::Error::source(child).is_none());
        }};
    }
    child!(CauseP3, ExecutionClaim, CauseClaim3::Framing);
    child!(CauseP3, Codec, CauseCodec::Limit);
    child!(CauseP3, Semantic, CauseSemantic::InputHistory);
    child!(CauseP4, Policy3, CauseP3::Header);
    child!(CauseP4, Forwarding, CauseStore::ForeignSubject);
    child!(CauseP5, Policy4, CauseP4::Header);
    child!(CauseP5, Forwarding, CauseLoad::ForeignSubject);
    child!(Error, Claim, IntegerContinuationClaimErrorV1::Framing);
    child!(Error, Policy5, CauseP5::Header);
    child!(Error, Transition, CauseSemantic::InputHistory);
    child!(Error, Map, KirOptimizationMapErrorV12::Identity);
    for error in [
        CauseP3::Header,
        CauseP3::Limit,
        CauseP3::InputHistory,
        CauseP3::ExecutionWitness,
        CauseP3::Panicked,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
    for error in [
        CauseP4::Header,
        CauseP4::Limit,
        CauseP4::ExecutionClaim,
        CauseP4::ExecutionWitness,
        CauseP4::Panicked,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
    for error in [
        CauseP5::Header,
        CauseP5::ExecutionClaim,
        CauseP5::ExecutionWitness,
        CauseP5::Panicked,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
    for error in [
        Error::Composition,
        Error::ExecutionWitness,
        Error::InputHistory,
        Error::Occurrences,
        Error::Panicked,
    ] {
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn reached_integer_execution_causes_preserve_exact_reservation_and_first_work_charge() {
    use crate::checked_load_forwarding_v1::tests::{STORAGE, WORK, fixture};
    const PRIOR: usize = 11;
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
    let record = INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1;
    assert_eq!(record, 416);
    // record() admits 4 + 2 * the fixed two-pass roster before reading it.
    const FIRST_WORK: usize = 8;
    for case in 0..3 {
        let storage_limit = floor + record - usize::from(case == 1);
        let work_limit = if case == 2 {
            PRIOR + FIRST_WORK - 1
        } else {
            WORK
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        work.charge_work(PRIOR).unwrap();
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            check_integer_execution(&checked, budget)
        });
        if case == 0 {
            result.unwrap();
            assert!(budget.work() > PRIOR + record);
        } else {
            let error = result.unwrap_err();
            let Error::Resource(child) = &error else {
                panic!("{error:?}")
            };
            semantic_cause_borrow(&error, child);
            semantic_cause_walk(&error, 1, *child);
            if case == 1 {
                let Resource::Storage(leaf) = child else {
                    panic!("{child:?}")
                };
                assert_eq!(
                    (leaf.actual(), leaf.limit()),
                    (floor + record, storage_limit)
                );
            } else {
                let Resource::Work(leaf) = child else {
                    panic!("{child:?}")
                };
                assert_eq!(
                    (leaf.actual(), leaf.limit()),
                    (PRIOR + FIRST_WORK, work_limit)
                );
            }
            assert_eq!(budget.work(), PRIOR);
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.peak_storage(),
            if case == 1 { floor } else { floor + record }
        );
        assert_eq!(
            budget.failed_storage(),
            if case == 1 {
                Some(floor + record)
            } else {
                None
            }
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(budget);
        assert_eq!(
            work.failed_work(),
            if case == 2 {
                Some(PRIOR + FIRST_WORK)
            } else {
                None
            }
        );
    }
    let retained = checked.retained_storage();
    drop(checked);
    preparation.release_storage(retained).unwrap();
    drop(input);
    preparation
        .release_storage(input_size.retained_storage())
        .unwrap();
    assert_eq!(preparation.storage(), 0);
}
