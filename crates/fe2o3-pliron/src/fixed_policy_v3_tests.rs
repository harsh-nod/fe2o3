use super::*;
use crate::{
    KirMappedExtractionErrorV12, KirOptimizationMapErrorV12, PlironOptimizationErrorV1,
    PlironOptimizationLimitsV1, PlironOptimizationPlanV1, PlironSession, ShellLimits,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, Terminator,
    Type, ValueDef, ValueId,
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PREFIX: usize = 19;
const PRIOR: usize = 7;

fn source() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let value = |id| ValueDef::new(ValueId(id), u32_ty.clone());
    let expression = |id| {
        Operation::effect_free(
            value(id),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(8),
                rhs: ValueId(9),
            },
        )
    };
    let store = |id| {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(99),
                value: ValueId(id),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )
    };
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![expression(1), store(1)];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(33),
        then_target: BlockId(30),
        then_arguments: vec![],
        else_target: BlockId(70),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(30));
    left.operations = vec![expression(2), store(2)];
    left.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut right = BasicBlock::new(BlockId(70));
    right.operations = vec![expression(3), store(3)];
    right.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut join = BasicBlock::new(BlockId(90));
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut module = Module::new("policy3-diamond");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![pointer.clone(), Type::BOOL, u32_ty.clone(), u32_ty.clone()],
            vec![u32_ty.clone()],
        ),
        vec![ValueId(99), ValueId(33), ValueId(8), ValueId(9)],
        vec![entry, left, right, join],
    ));
    module
}

#[test]
fn historical_session_explicitly_rejects_new_step_before_mutating() {
    let mut session = PlironSession::new(ShellLimits::default(), []).unwrap();
    let root = session
        .import_operation_text_v1(
            r#"builtin.module @custody {
        ^entry():
        dead = builtin.constant <builtin.integer <7: i64>> : builtin.integer i64
    }"#,
        )
        .unwrap();
    let before = session
        .analyze_operation_graph_v1(&root)
        .unwrap()
        .replay_identity();
    let plan = PlironOptimizationPlanV1::new(
        vec![PassKind::DominancePureCommonSubexpressionElimination],
        PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        session.execute_optimization_v1(&root, &plan),
        Err(PlironOptimizationErrorV1::PassRejected(
            PassKind::DominancePureCommonSubexpressionElimination
        ))
    ));
    assert_eq!(
        session
            .analyze_operation_graph_v1(&root)
            .unwrap()
            .replay_identity(),
        before
    );
    assert!(!session.is_poisoned());
    assert_eq!(PassKind::DeadCodeElimination as usize, 0);
    assert_eq!(PassKind::SimplifyControlFlow as usize, 4);
    assert_eq!(
        PassKind::DominancePureCommonSubexpressionElimination as usize,
        5
    );
}

#[test]
fn ledger_exact_short_denial_and_cleanup_preserve_first_failure_and_prefix() {
    for short in [false, true] {
        let mut work = Work::new(PRIOR + 4 - usize::from(short));
        let mut budget = Budget::new(&mut work, PREFIX + 13);
        budget.charge_work(PRIOR).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let first;
        {
            let mut ledger = CseLedger::new(&mut budget);
            ledger.charge_work(3).unwrap();
            ledger.reserve_storage(13).unwrap();
            first = ledger.charge_work(1).err();
            ledger.release_storage(13);
            if short {
                // A later invalid cleanup must not overwrite the first denial.
                ledger.release_storage(1);
                assert_eq!(ledger.finish().unwrap_err(), first.unwrap());
                assert_eq!(ledger.reserve_storage(1).unwrap_err(), first.unwrap());
            } else {
                assert_eq!(ledger.finish().unwrap(), 4);
            }
        }
        assert_eq!(budget.storage(), PREFIX);
        assert_eq!(budget.peak_storage(), PREFIX + 13);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.work(), PRIOR + if short { 3 } else { 4 });
        assert_eq!(work.failed_work(), short.then_some(PRIOR + 4));
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, PREFIX + 13);
    budget.reserve_storage(PREFIX).unwrap();
    {
        let mut ledger = CseLedger::new(&mut budget);
        ledger.reserve_storage(13).unwrap();
        let first = ledger.reserve_storage(1).unwrap_err();
        assert!(matches!(first, Resource::Storage(_)));
        ledger.release_storage(13);
        assert_eq!(ledger.finish().unwrap_err(), first);
    }
    assert_eq!(budget.storage(), PREFIX);
    assert_eq!(budget.failed_storage(), Some(PREFIX + 14));
}

#[test]
fn both_local_and_canonical_release_errors_are_sticky_not_silently_ignored() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX).unwrap();
    {
        let mut ledger = CseLedger::new(&mut budget);
        ledger.reserve_storage(13).unwrap();
        ledger.release_storage(14); // Caller prefix cannot hide a local underflow.
        assert_eq!(ledger.finish().unwrap_err(), Resource::Accounting);
        ledger.release_storage(13); // Balanced cleanup still runs after failure.
    }
    assert_eq!(budget.storage(), PREFIX);
    {
        let mut ledger = CseLedger::new(&mut budget);
        ledger.reserve_storage(PREFIX + 1).unwrap();
        // Fault injection of inconsistent local custody, not a genuine core
        // path: force the underlying fallible release to reject independently.
        ledger.budget.release_storage(PREFIX + 1).unwrap();
        ledger.release_storage(PREFIX + 1);
        assert_eq!(ledger.finish().unwrap_err(), Resource::Accounting);
    }
    assert_eq!(budget.storage(), PREFIX);
    assert_eq!(budget.failed_storage(), None);
}

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}

#[test]
fn wrong_policy_finalizer_fails_before_allocating_or_retaining_output() {
    let (input, storage) = admit(&Module::new("m"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX + storage).unwrap();
    let (mut graph, witness) =
        crate::kir_bridge_v1::import_native_neutral_v1(&input, &mut budget).unwrap();
    let limits = graph
        .neutral_occurrence_limits_v1(&mut budget)
        .unwrap()
        .for_policy3()
        .unwrap();
    budget.charge_work(limits.work().unwrap()).unwrap();
    budget.reserve_storage(limits.storage().unwrap()).unwrap();
    let capture = graph
        .begin_neutral_occurrence_capture_for_policy_v1(limits, FixedPolicy::Checked3)
        .unwrap();
    let (report, profile, _) = graph
        .execute_native_policy3_v1(&mut budget, &capture, limits)
        .unwrap();
    budget.reserve_storage(profile.retained_storage()).unwrap();
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    assert!(matches!(
        graph.extract_admitted_canonical_with_map_v1(&mut budget, Some(&witness)),
        Err(KirMappedExtractionErrorV12::Mapping(
            KirOptimizationMapErrorV12::Passes
        ))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    let (output, _, map, receipt) = graph
        .extract_admitted_canonical_with_policy3_map_v1(&mut budget, &witness)
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(
        output.canonical().canonical_bytes(),
        input.canonical().canonical_bytes()
    );
    assert!(map.matches_execution(&report));
    drop((output, map, report, capture, witness, graph));
    budget
        .release_storage(budget.storage() - PREFIX - storage)
        .unwrap();
    assert_eq!(budget.storage(), PREFIX + storage);
}

#[test]
fn shared_checker_callback_error_and_unwind_discard_actual_policy3_output() {
    let (input, storage) = admit(&Module::new("m"));
    for panic in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR).unwrap();
        budget.reserve_storage(PREFIX + storage).unwrap();
        let observed =
            crate::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget).unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let before = budget.work();
        let result = observed.try_check_and_finish_with_v1::<(), _, _>(&mut budget, |_, budget| {
            budget.charge_work(1).unwrap();
            if panic {
                panic!("scoped policy3 adapter unwind");
            }
            Err::<((), usize), _>(17u32)
        });
        if panic {
            assert!(matches!(
                result,
                Err(crate::KirCheckedNeutralOptimizationErrorV1::Panicked)
            ));
        } else {
            assert!(matches!(
                result,
                Err(crate::KirCheckedNeutralOptimizationErrorV1::Origin(17))
            ));
        }
        assert!(budget.work() > before);
        assert_eq!(budget.storage(), PREFIX + storage);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn sticky_nonpanicking_occurrence_denial_after_mutation_cannot_extract_an_output() {
    let (input, storage) = admit(&source());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR).unwrap();
    budget.reserve_storage(PREFIX + storage).unwrap();
    let (mut graph, witness) =
        crate::kir_bridge_v1::import_native_neutral_v1(&input, &mut budget).unwrap();
    let mut limits = graph
        .neutral_occurrence_limits_v1(&mut budget)
        .unwrap()
        .for_policy3()
        .unwrap();
    // The real occurrence observer refuses its first mutation event without
    // unwinding. Keep the normal prepaid work envelope so denial occurs in the
    // callback, not during initial registration/census.
    let capture_work = limits.work().unwrap();
    let capture_storage = limits.storage().unwrap();
    limits.events = 0;
    budget.charge_work(capture_work).unwrap();
    budget.reserve_storage(capture_storage).unwrap();
    let capture = graph
        .begin_neutral_occurrence_capture_for_policy_v1(limits, FixedPolicy::Checked3)
        .unwrap();
    let before = graph
        .neutral_live_roster_v1(limits.nodes, &mut |_| Ok(()))
        .unwrap()
        .len();
    let result = graph.execute_native_policy3_v1(&mut budget, &capture, limits);
    assert!(result.is_err());
    assert_eq!(capture.failure(), Some(KirOptimizationMapErrorV12::Limit));
    let after = graph
        .neutral_live_roster_v1(limits.nodes, &mut |_| Ok(()))
        .unwrap()
        .len();
    assert!(
        after < before,
        "the private candidate really mutated before callback refusal was consumed"
    );
    assert!(graph.session.is_poisoned());
    assert!(
        graph
            .extract_admitted_canonical_with_policy3_map_v1(&mut budget, &witness)
            .is_err()
    );
    drop((capture, witness, graph));
    budget
        .release_storage(budget.storage() - PREFIX - storage)
        .unwrap();
    assert_eq!(budget.storage(), PREFIX + storage);
    assert_eq!(budget.failed_storage(), None);
    assert!(budget.work() > PRIOR);
}

#[test]
fn policy3_empty_consuming_check_has_independent_exact_and_short_work() {
    let (input, input_storage) = admit(&Module::new("m"));
    assert_eq!(input.canonical().canonical_bytes().len(), 37);
    for short in [false, true] {
        let mut preparation_work = Work::new(WORK);
        let mut preparation = Budget::new(&mut preparation_work, STORAGE);
        preparation.reserve_storage(input_storage).unwrap();
        let observed =
            crate::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut preparation).unwrap();
        // Independent partial adoption formula: entry1 + inventory pairs4 +
        // checker5 + two name bytes2 + input copy37 + new capacity precharge2.
        let mut work = Work::new(PRIOR + 51 - usize::from(short));
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR).unwrap();
        budget
            .reserve_storage(PREFIX + input_storage + observed.storage().retained_storage())
            .unwrap();
        let result = observed.try_check_and_finish_v1(&mut budget);
        if short {
            assert!(matches!(
                result,
                Err(crate::KirCheckedNeutralOptimizationErrorV1::Resource(
                    Resource::Work(_)
                ))
            ));
            assert_eq!(budget.work(), PRIOR + 49);
        } else {
            let checked = result.unwrap();
            assert_eq!(
                checked.owner().canonical().canonical_bytes(),
                input.canonical().canonical_bytes()
            );
            assert_eq!(budget.work(), PRIOR + 51);
            drop(checked);
        }
        assert_eq!(budget.storage(), PREFIX + input_storage);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), short.then_some(PRIOR + 51));
    }
}

// Intentionally synthetic framing input: only the separate kernel-opt replay
// tests obtain wire bytes from genuine execution. This cannot mint a witness.
fn synthetic_claim_frame(input: &Owner) -> [u8; POLICY3_EXECUTION_RECORD_BYTES_V1] {
    let mut bytes = [0; POLICY3_EXECUTION_RECORD_BYTES_V1];
    let mut writer = RecordWriter {
        bytes: &mut bytes,
        cursor: 0,
    };
    for value in [3, 1, 8, 0] {
        writer.u16(value);
    }
    for _ in 0..2 {
        writer.raw(input.canonical().identity().digest());
        writer.u64(input.canonical().identity().canonical_length());
    }
    for value in [0, 17, 0, 0, 8, 0] {
        writer.u64(value);
    }
    for cap in [
        POLICY3_CANONICAL_CAP,
        POLICY3_CANONICAL_CAP,
        POLICY3_MAX_PASSES,
        POLICY3_GRAPH_CAP,
        POLICY3_SESSION_WORK_CAP,
    ] {
        writer.usize(cap).unwrap();
    }
    for _ in 0..4 {
        writer.u64(0);
    }
    writer.raw(&[0; 32]);
    for _ in 0..3 {
        writer.u64(0);
    }
    writer.raw(&[0; 32]);
    for pass in POLICY3_PASSES {
        writer.raw(&[pass_tag(pass), 0]);
        writer.u16(0);
        for _ in 0..7 {
            writer.u64(0);
        }
    }
    assert_eq!(writer.cursor, POLICY3_EXECUTION_RECORD_BYTES_V1);
    bytes
}

#[test]
fn unauthenticated_claim_parses_closed_fields_and_rejects_endpoint_profile_roster_drift() {
    use Policy3ExecutionClaimErrorV1 as E;
    let (input, storage) = admit(&Module::new("m"));
    let bytes = synthetic_claim_frame(&input);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = PREFIX + storage + bytes.len();
    budget.reserve_storage(floor).unwrap();
    let claim =
        read_unauthenticated_policy3_execution_claim_v1(&input, &input, &bytes, &mut budget)
            .unwrap();
    assert!(std::ptr::eq(claim.canonical_bytes(), &bytes));
    assert_eq!(claim.declared_profile_work(), 17);
    assert!(!claim.grants_authority());
    // Every fixed control field, both endpoints, profile pass count, all five
    // caps, and each pass row tag/boolean/reserved field are checked directly.
    for (offset, value, expected) in [
        (0, 2, E::Framing),
        (2, 2, E::Framing),
        (4, 7, E::Framing),
        (6, 1, E::Framing),
        (8, bytes[8] ^ 1, E::Endpoint),
        (40, bytes[40] ^ 1, E::Endpoint),
        (48, bytes[48] ^ 1, E::Endpoint),
        (80, bytes[80] ^ 1, E::Endpoint),
        (120, 7, E::Profile),
    ] {
        let mut bad = bytes;
        bad[offset] = value;
        assert_eq!(
            read_unauthenticated_policy3_execution_claim_v1(&input, &input, &bad, &mut budget)
                .err(),
            Some(expected)
        );
    }
    for offset in (136..176).step_by(8) {
        let mut bad = bytes;
        bad[offset] ^= 1;
        assert_eq!(
            read_unauthenticated_policy3_execution_claim_v1(&input, &input, &bad, &mut budget)
                .err(),
            Some(E::Profile)
        );
    }
    for row in 0..8 {
        for (field, value) in [(0, 255), (1, 2), (2, 1), (3, 1)] {
            let mut bad = bytes;
            bad[296 + row * 60 + field] = value;
            assert_eq!(
                read_unauthenticated_policy3_execution_claim_v1(&input, &input, &bad, &mut budget)
                    .err(),
                Some(E::Pass)
            );
        }
    }
    for length in [0, 7, 775] {
        assert_eq!(
            read_unauthenticated_policy3_execution_claim_v1(
                &input,
                &input,
                &bytes[..length],
                &mut budget
            )
            .err(),
            Some(E::Framing)
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        read_unauthenticated_policy3_execution_claim_v1(&input, &input, &trailing, &mut budget)
            .err(),
        Some(E::Framing)
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn unauthenticated_claim_reader_has_independent_778_unit_prefix_and_no_owned_storage() {
    let (input, storage) = admit(&Module::new("m"));
    let bytes = synthetic_claim_frame(&input);
    for short in [false, true] {
        let mut work = Work::new(PRIOR + 778 - usize::from(short));
        let mut budget = Budget::new(&mut work, PREFIX + storage + bytes.len());
        budget.charge_work(PRIOR).unwrap();
        budget
            .reserve_storage(PREFIX + storage + bytes.len())
            .unwrap();
        let floor = budget.storage();
        let result =
            read_unauthenticated_policy3_execution_claim_v1(&input, &input, &bytes, &mut budget);
        if short {
            assert!(matches!(
                result,
                Err(Policy3ExecutionClaimErrorV1::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.work(), PRIOR + 2);
        } else {
            assert!(result.is_ok());
            assert_eq!(budget.work(), PRIOR + 778);
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), short.then_some(PRIOR + 778));
    }
}
