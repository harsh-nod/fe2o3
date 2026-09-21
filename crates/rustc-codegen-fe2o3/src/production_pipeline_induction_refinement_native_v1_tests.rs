//! Genuine constructed source/ranked owners; ordinary rustc entry is a separate gate.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
use crate::production_ranked_projection_v1::{
    with_backend_licm_direct_prefix_v1, with_backend_licm_erased_prefix_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInductionRefinementOriginV1 as Origin;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as Coordinate, CheckedBinaryOperator, Constant, Operation,
    OperationKind, ScalarType,
};
use std::mem::size_of_val;

#[path = "production_pipeline_induction_refinement_native_resources_v1_tests.rs"]
mod resources;
#[path = "production_pipeline_induction_refinement_native_sim_v1_tests.rs"]
mod sim;

fn with_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        with_backend_licm_erased_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Erased(owner), budget)
        });
    } else {
        with_backend_licm_direct_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Direct(owner), budget)
        });
    }
}
fn operation(owner: &Graph, coordinate: Coordinate) -> &Operation {
    &owner.module().functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize]
}
fn tail(owner: &Refined) -> &fe2o3_kernel_opt::OwnedInductionRefinementContinuationV1 {
    match owner {
        Refined::Direct(v) => v.continuation(),
        Refined::Erased(v) => v.continuation(),
    }
}
fn count_operations(graph: &Graph) -> usize {
    graph
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|f| &f.blocks)
        .map(|b| b.operations.len())
        .sum()
}
fn assert_shape(
    value: &PreparedInductionRefinementNativeOutputV1,
    mutation: bool,
    budget: &mut Budget<'_>,
) {
    assert_eq!(value.limits(), Limits::default());
    assert_eq!(value.origins().len(), count_operations(value.licm_input()));
    assert_eq!(value.origins().len(), tail(&value.owner).origins().len());
    let mut splits = 0;
    for (source, row) in value.origins().iter().zip(tail(&value.owner).origins()) {
        assert_eq!(source.canonical_origin(), *row);
        assert_eq!(source.synthetic_false_source_statement(), None);
        match *row {
            Origin::Unchanged { input, output } => {
                assert_eq!(
                    operation(value.licm_input(), input),
                    operation(value.output(), output)
                );
                assert_eq!(source.synthetic_overflow_definition(), None);
            }
            Origin::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                ..
            } => {
                splits += 1;
                assert!(source.original_source_statement().is_some());
                assert_eq!(
                    source.synthetic_overflow_definition(),
                    Some(Definition::Result {
                        operation: input,
                        result: 1
                    })
                );
                let old = operation(value.licm_input(), input);
                let sum = operation(value.output(), sum_output);
                let flag = operation(value.output(), false_output);
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = old.kind
                else {
                    panic!("actual checked source update");
                };
                assert_eq!(old.results.len(), 2);
                assert_eq!(
                    old.results[0].ty,
                    fe2o3_kernel_ir::Type::Scalar(ScalarType::U64)
                );
                assert_eq!(
                    sum.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs
                    }
                );
                assert_eq!(sum.results.as_slice(), &old.results[..1]);
                assert_eq!(flag.kind, OperationKind::Constant(Constant::Bool(false)));
                assert_eq!(flag.results.as_slice(), &old.results[1..]);
                assert_eq!(
                    (sum_output.block, false_output.block),
                    (input.block, input.block)
                );
                assert_eq!(sum_output.operation + 1, false_output.operation);
            }
        }
    }
    assert_eq!(
        splits,
        if mutation { 2 } else { 0 },
        "actual source refinement cannot be a vacuous positive"
    );
    assert_eq!(
        count_operations(value.output()),
        count_operations(value.licm_input()) + splits
    );
    assert_eq!(
        value.licm_input().module().kernels,
        value.output().module().kernels
    );
    assert!(!std::ptr::eq(value.licm_input(), value.output()));
    if mutation {
        assert_ne!(
            value.licm_input().canonical().canonical_bytes(),
            value.output().canonical().canonical_bytes()
        );
    } else {
        assert_eq!(
            value.licm_input().canonical().canonical_bytes(),
            value.output().canonical().canonical_bytes()
        );
    }
    let (preheaders, incoming, parameters, reports) = match &value.owner {
        Refined::Direct(v) => (
            v.prefix().prefix().origins(),
            v.prefix().prefix().incoming_origins(),
            v.prefix().prefix().parameter_origins(),
            v.kernels(),
        ),
        Refined::Erased(v) => {
            let source = v.prefix().prefix().prefix().prefix().prefix().prefix();
            assert_eq!(
                source
                    .original_source()
                    .semantic_ssa()
                    .source_semantic()
                    .functions()
                    .len(),
                3
            );
            assert_eq!(
                (
                    source.erased_source().deleted_call_count(),
                    source.erased_source().deleted_function_count()
                ),
                (2, 1)
            );
            (
                v.prefix().prefix().origins(),
                v.prefix().prefix().incoming_origins(),
                v.prefix().prefix().parameter_origins(),
                v.kernels(),
            )
        }
    };
    assert_eq!(
        (preheaders.len(), incoming.len(), reports.len()),
        (
            if mutation { 2 } else { 0 },
            if mutation { 4 } else { 0 },
            2
        )
    );
    assert_eq!(parameters.is_empty(), !mutation);
    let floor = budget.storage();
    let (pair, storage) = tail(&value.owner)
        .replay_against(value.licm_input(), value.limits(), budget)
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(std::ptr::eq(pair.input(), value.licm_input()));
    assert!(std::ptr::eq(pair.output(), value.output()));
    assert_eq!(pair.origins(), tail(&value.owner).origins());
    budget.release_storage(storage.retained_storage()).unwrap();
    value.verify_equivalence(budget).unwrap();
    assert_eq!(budget.storage(), floor);
}
fn expected_text(
    value: &PreparedInductionRefinementNativeOutputV1,
    mutation: bool,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let (final_text, receipt) = lower_native(value.output(), value.profile, budget).unwrap();
    budget.reserve_storage(receipt).unwrap();
    assert_eq!(final_text, value.llvm_ir());
    drop(final_text);
    budget.release_storage(receipt).unwrap();
    let (historical_text, receipt) =
        lower_native(value.licm_input(), value.profile, budget).unwrap();
    budget.reserve_storage(receipt).unwrap();
    if mutation {
        assert_ne!(historical_text, value.llvm_ir());
        assert!(matches!(
            check_native_text(value.output(), value.profile, &historical_text, budget),
            Err(ProductionPipelineError::InductionRefinementNativeStage(
                InductionRefinementNativeStageErrorV1::Mismatch(
                    "exact induction-refinement native LLVM"
                )
            ))
        ));
    } else {
        assert_eq!(historical_text, value.llvm_ir());
    }
    drop(historical_text);
    budget.release_storage(receipt).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn induction_refinement_native_actual_two_splits_emit_only_final_source_graph_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, true, |prefix, budget| {
                let original = match &prefix {
                    Prefix6::Direct(v) => v.source_semantic_kir().pre_ranked_executable().unwrap(),
                    Prefix6::Erased(v) => v.original_source().executable(),
                }
                .canonical()
                .canonical_bytes()
                .as_ptr();
                let sibling = vec![0x39u8; 53];
                let sibling_bytes = size_of_val(&sibling) + sibling.capacity();
                budget.reserve_storage(sibling_bytes).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let (value, receipt) = prepare(prefix, profile, Limits::default(), budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                assert_eq!(
                    value
                        .original()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    original
                );
                assert_eq!(matches!(value.owner, Refined::Erased(_)), erased);
                assert_eq!(value.prefix_execution.policy_version(), 7);
                assert!(!value.grants_artifact_or_launch_authority());
                assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
                assert_shape(&value, true, budget);
                expected_text(&value, true, budget);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x39; 53]);
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
                drop(sibling);
                budget.release_storage(sibling_bytes).unwrap();
            });
        }
    }
}
#[test]
fn induction_refinement_native_actual_noops_keep_exact_graph_and_text_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, false, |prefix, budget| {
                let floor = budget.storage();
                let (value, receipt) = prepare(prefix, profile, Limits::default(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_shape(&value, false, budget);
                expected_text(&value, false, budget);
                assert!(!value.grants_artifact_or_launch_authority());
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn induction_refinement_native_fresh_preparations_are_byte_and_resource_deterministic() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let mut previous = None;
                for _ in 0..2 {
                    with_prefix(erased, profile, mutation, |prefix, budget| {
                        let (value, receipt) =
                            prepare(prefix, profile, Limits::default(), budget).unwrap();
                        let observation = (
                            value.output().canonical().canonical_bytes().to_vec(),
                            value.llvm_ir().to_owned(),
                            value.prefix_execution.canonical_bytes().to_vec(),
                            value.origins().to_vec(),
                            value.limits(),
                            receipt.retained_storage(),
                            budget.work(),
                            budget.peak_storage(),
                        );
                        if let Some(previous) = &previous {
                            assert_eq!(previous, &observation);
                        }
                        previous = Some(observation);
                    });
                }
            }
        }
    }
}
#[test]
fn induction_refinement_native_replay_refuses_same_length_text_wrong_target_and_missing_floor() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_prefix(erased, Profile::Gfx942, mutation, |prefix, budget| {
                let (mut value, receipt) =
                    prepare(prefix, Profile::Gfx942, Limits::default(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();
                let capacity = value.llvm.capacity();
                let first = value.llvm.remove(0);
                value.llvm.insert(0, if first == ';' { ' ' } else { ';' });
                assert_eq!(value.llvm.capacity(), capacity);
                assert!(matches!(
                    value.verify_equivalence(budget),
                    Err(ProductionPipelineError::InductionRefinementNativeStage(
                        InductionRefinementNativeStageErrorV1::Mismatch(
                            "exact induction-refinement native LLVM"
                        )
                    ))
                ));
                value.llvm.remove(0);
                value.llvm.insert(0, first);
                let profile = value.profile;
                value.profile = Profile::Gfx950;
                match value.verify_equivalence(budget) {
                    Err(ProductionPipelineError::TargetLowering(e)) => assert!(
                        e.contains(dialect_amdgcn::LoweringDiagnosticCode::UnsupportedCapability)
                    ),
                    _ => panic!("actual final target mismatch must fail native preflight"),
                }
                value.profile = profile;
                assert_eq!(budget.storage(), floor);
                value.retained_floor += 1;
                let work = budget.work();
                assert!(matches!(
                    value.verify_equivalence(budget),
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        P7Error::Resource(Resource::Accounting)
                    ))
                ));
                assert_eq!((budget.storage(), budget.work()), (floor, work));
                value.retained_floor -= 1;
                budget.release_storage(1).unwrap();
                assert!(matches!(
                    value.verify_equivalence(budget),
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        P7Error::Resource(Resource::Accounting)
                    ))
                ));
                assert_eq!(budget.work(), work);
                budget.reserve_storage(1).unwrap();
                value.verify_equivalence(budget).unwrap();
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
        }
    }
}
#[test]
fn induction_refinement_native_replay_requires_genuine_historical_p7_witness() {
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let (mut first, receipt) =
            prepare(prefix, Profile::Gfx942, Limits::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        with_prefix(false, Profile::Gfx950, true, |prefix, other| {
            let (mut second, other_receipt) =
                prepare(prefix, Profile::Gfx950, Limits::default(), other).unwrap();
            other
                .reserve_storage(other_receipt.retained_storage())
                .unwrap();
            assert_eq!(
                first.prefix_execution.retained_storage(),
                second.prefix_execution.retained_storage()
            );
            assert_ne!(
                first.prefix_execution.canonical_bytes(),
                second.prefix_execution.canonical_bytes()
            );
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            assert!(matches!(
                first.verify_equivalence(budget),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Execution("Policy7 complete execution transcript")
                ))
            ));
            assert_eq!(budget.storage(), floor);
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            first.verify_equivalence(budget).unwrap();
            second.verify_equivalence(other).unwrap();
            drop(second);
            other
                .release_storage(other_receipt.retained_storage())
                .unwrap();
        });
        drop(first);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}
#[test]
fn induction_refinement_native_actual_pair_rejects_changed_limits_and_historical_input() {
    use fe2o3_kernel_opt::OwnedInductionRefinementErrorV1 as E;
    for erased in [false, true] {
        with_prefix(erased, Profile::Gfx942, true, |prefix, budget| {
            let (value, receipt) =
                prepare(prefix, Profile::Gfx942, Limits::default(), budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            let expected = value.limits();
            for field in 0..7 {
                for delta in [-1isize, 1] {
                    let mut changed = expected;
                    let slot = match field {
                        0 => &mut changed.functions,
                        1 => &mut changed.blocks,
                        2 => &mut changed.edges,
                        3 => &mut changed.definitions,
                        4 => &mut changed.operations,
                        5 => &mut changed.loops,
                        _ => &mut changed.rows,
                    };
                    *slot = slot.checked_add_signed(delta).unwrap();
                    assert!(matches!(
                        tail(&value.owner).replay_against(value.licm_input(), changed, budget),
                        Err(E::LimitsMismatch)
                    ));
                    assert_eq!(budget.storage(), floor);
                }
            }
            assert_ne!(
                value.historical_p8_output().canonical().identity(),
                value.licm_input().canonical().identity()
            );
            assert!(matches!(
                tail(&value.owner).replay_against(value.historical_p8_output(), expected, budget),
                Err(E::ForeignInput)
            ));
            value.verify_equivalence(budget).unwrap();
            drop(value);
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
    }
}
#[test]
fn induction_refinement_native_preserves_exact_source_cap_refusal_without_clamping() {
    for erased in [false, true] {
        with_prefix(erased, Profile::Gfx942, true, |prefix, budget| {
            let source_limit = 1_048_576;
            assert_eq!(
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::new_with_max_operations(
                    1_024,
                    16_384,
                    1_048_576,
                    source_limit,
                ),
            );
            let floor = budget.storage();
            match prepare(
                prefix,
                Profile::Gfx942,
                Limits {
                    operations: source_limit + 1,
                    ..Limits::default()
                },
                budget,
            ) {
                Err(ProductionPipelineError::InductionRefinementNativeStage(
                    InductionRefinementNativeStageErrorV1::Admission(error),
                )) => {
                    assert!(
                        matches!(*error, RefinementAdmissionError::SourceOperationsLimit { requested, source_limit: actual }
                    if requested == source_limit + 1 && actual == source_limit)
                    );
                }
                _ => panic!("unchanged typed source cap refusal"),
            }
            assert_eq!(budget.storage(), floor);
        });
    }
}
#[test]
fn induction_refinement_native_partial_error_and_panic_drop_real_owner_before_credit() {
    use std::{cell::Cell, rc::Rc};
    struct Candidate {
        owner: Option<PreparedInductionRefinementNativeOutputV1>,
        dropped: Rc<Cell<bool>>,
    }
    impl Drop for Candidate {
        fn drop(&mut self) {
            let owner = self.owner.take().unwrap();
            assert_eq!(
                owner
                    .origins()
                    .iter()
                    .filter(|r| matches!(r.canonical_origin(), Origin::CheckedAddSplit { .. }))
                    .count(),
                2
            );
            assert!(!owner.llvm_ir().is_empty());
            drop(owner);
            self.dropped.set(true);
        }
    }
    for erased in [false, true] {
        for panic in [false, true] {
            with_prefix(erased, Profile::Gfx942, true, |prefix, budget| {
                let sibling = vec![0x43u8; 43];
                let bytes = size_of_val(&sibling) + sibling.capacity();
                budget.reserve_storage(bytes).unwrap();
                let floor = budget.storage();
                let work = budget.work();
                let ledger = budget.work_ledger_identity_v1();
                let dropped = Rc::new(Cell::new(false));
                let result: Result<()> = scoped(floor, budget, |budget| {
                    let (value, receipt) =
                        prepare(prefix, Profile::Gfx942, Limits::default(), budget)?;
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    let _live = Candidate {
                        owner: Some(value),
                        dropped: dropped.clone(),
                    };
                    if panic {
                        std::panic::panic_any(79u32);
                    }
                    Err(mismatch("injected after genuine refined native owner"))
                });
                if panic {
                    assert!(matches!(
                        result,
                        Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                            P7Error::Panicked
                        ))
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionPipelineError::InductionRefinementNativeStage(
                            InductionRefinementNativeStageErrorV1::Mismatch(
                                "injected after genuine refined native owner"
                            )
                        ))
                    ));
                }
                assert!(dropped.get());
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > work && budget.peak_storage() > floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x43; 43]);
                drop(sibling);
                budget.release_storage(bytes).unwrap();
            });
        }
    }
}
#[test]
fn induction_refinement_native_new_header_counts_active_variant_and_padding() {
    for active in [size_of::<DirectRefined>(), size_of::<ErasedRefined>()] {
        let header = size_of::<PreparedInductionRefinementNativeOutputV1>()
            - active
            - size_of::<Policy7ExecutionWitnessV1>()
            - size_of::<String>();
        assert_eq!(
            active + size_of::<Policy7ExecutionWitnessV1>() + size_of::<String>() + header,
            size_of::<PreparedInductionRefinementNativeOutputV1>()
        );
        assert!(header >= size_of::<Profile>() + size_of::<usize>());
        assert!(header >= size_of::<Refined>() - active);
    }
    assert!(
        size_of::<InductionRefinementNativeProductionCompilationV1>()
            > size_of::<PreparedInductionRefinementNativeOutputV1>()
    );
}
