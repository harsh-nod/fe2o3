//! The actual source pipeline must perform both rewrites, never a detached model.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
use crate::production_ranked_projection_v1::{
    with_backend_forwarding_erased_prefix_v1, with_backend_licm_direct_prefix_v1,
    with_backend_licm_erased_prefix_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInductionRefinementOriginV1 as Origin;
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as Coordinate, CheckedBinaryOperator, Constant, Operation,
    OperationKind,
};
use std::mem::size_of_val;
#[path = "production_pipeline_refined_forwarding_descriptor_v1_tests.rs"]
mod descriptor;
#[path = "production_pipeline_refined_forwarding_native_resources_v1_tests.rs"]
mod resources;
#[path = "production_pipeline_refined_forwarding_native_sim_v1_tests.rs"]
mod sim;

fn with_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        with_backend_forwarding_erased_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Erased(owner), budget)
        });
    } else {
        with_backend_licm_direct_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Direct(owner), budget)
        });
    }
}
fn operation(owner: &Graph, site: Coordinate) -> &Operation {
    &owner.module().functions[site.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[site.block.block as usize]
        .operations[site.operation as usize]
}
fn count(owner: &Graph) -> usize {
    owner
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum()
}
fn tails(
    owner: &Composed,
) -> (
    &fe2o3_kernel_opt::OwnedInductionRefinementContinuationV1,
    &fe2o3_kernel_opt::OwnedCrossBlockForwardingV1,
) {
    match owner {
        Composed::Direct(owner) => (owner.prefix().continuation(), owner.continuation()),
        Composed::Erased(owner) => (owner.prefix().continuation(), owner.continuation()),
    }
}
fn shape(
    value: &PreparedRefinedForwardingNativeOutputV1,
    erased: bool,
    splits: usize,
    selected: usize,
    budget: &mut Budget<'_>,
) {
    let l = value.licm_input();
    let r = value.refinement_output();
    let f = value.output();
    assert_eq!(value.refinement_origins().len(), count(l));
    assert_eq!(value.origins().len(), count(r));
    assert_eq!(count(r), count(l) + splits);
    assert_eq!(count(r), count(f));
    assert_eq!(l.module().kernels, r.module().kernels);
    assert_eq!(r.module().kernels, f.module().kernels);
    assert_eq!(f.module().kernels.len(), 2);
    assert!(!std::ptr::eq(l, r) && !std::ptr::eq(r, f));
    let mut refined = 0;
    for source in value.refinement_origins() {
        assert_eq!(source.synthetic_false_source_statement(), None);
        match source.canonical_origin() {
            Origin::Unchanged { input, output } => {
                assert_eq!(operation(l, input), operation(r, output));
                assert_eq!(source.synthetic_overflow_definition(), None);
            }
            Origin::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                ..
            } => {
                refined += 1;
                assert!(source.original_source_statement().is_some());
                assert_eq!(
                    source.synthetic_overflow_definition(),
                    Some(Definition::Result {
                        operation: input,
                        result: 1
                    })
                );
                let old = operation(l, input);
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = old.kind
                else {
                    panic!("actual source CheckedAdd");
                };
                assert_eq!(
                    operation(r, sum_output).kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs
                    }
                );
                assert_eq!(
                    operation(r, sum_output).results.as_slice(),
                    &old.results[..1]
                );
                assert_eq!(
                    operation(r, false_output).kind,
                    OperationKind::Constant(Constant::Bool(false))
                );
                assert_eq!(
                    operation(r, false_output).results.as_slice(),
                    &old.results[1..]
                );
                for site in [sum_output, false_output] {
                    let row = value
                        .origins()
                        .iter()
                        .find(|row| row.canonical_origin().input == site)
                        .unwrap();
                    assert_eq!(row.canonical_origin().store, None);
                    assert_eq!(operation(r, site), operation(f, site));
                    assert_eq!(
                        row.original_source_statement(),
                        if site == false_output {
                            None
                        } else {
                            source.original_source_statement()
                        }
                    );
                }
            }
        }
    }
    assert_eq!(
        refined, splits,
        "both genuine source recurrences must refine"
    );
    let mut forwarded = 0;
    for source in value.origins() {
        let row = source.canonical_origin();
        assert_eq!(row.input, row.output);
        let old = operation(r, row.input);
        let new = operation(f, row.output);
        if let Some(store) = row.store {
            forwarded += 1;
            assert_ne!(store.block, row.input.block);
            let OperationKind::Load { pointer, access } = old.kind else {
                panic!("actual R Load");
            };
            let OperationKind::Store {
                pointer: destination,
                access: stored_access,
                value,
            } = operation(r, store).kind
            else {
                panic!("actual R Store, not a stale L coordinate");
            };
            assert_eq!((pointer, access), (destination, stored_access));
            assert_eq!(access.address_space, AddressSpace::Private);
            assert_eq!(
                new.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: value,
                    rhs: value
                }
            );
            assert_eq!(new.results, old.results);
            let (_, block, statement) = source.original_source_statement().unwrap();
            assert_eq!((block.index(), statement), (if erased { 7 } else { 5 }, 0));
        } else {
            assert_eq!(old, new);
        }
    }
    assert_eq!(
        forwarded, selected,
        "both actual Loads must survive all original gates"
    );
    assert_eq!(value.refinement_limits(), Limits::default());
    assert_eq!(value.limits(), ForwardingLimits::default());
    assert!(!value.grants_artifact_or_launch_authority());
    assert_eq!(value.prefix_execution.policy_version(), 7);
    assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
    let floor = budget.storage();
    let receipts = {
        let (refine_tail, forward_tail) = tails(&value.owner);
        let (first, a) = refine_tail
            .replay_against(l, value.refinement_limits(), budget)
            .unwrap();
        budget.reserve_storage(a.retained_storage()).unwrap();
        let (last, b) = forward_tail.replay_against(r, budget).unwrap();
        budget.reserve_storage(b.retained_storage()).unwrap();
        assert!(std::ptr::eq(first.input(), l) && std::ptr::eq(first.output(), r));
        assert!(std::ptr::eq(first.output(), last.input()) && std::ptr::eq(last.output(), f));
        a.retained_storage() + b.retained_storage()
    };
    budget.release_storage(receipts).unwrap();
    assert_eq!(budget.storage(), floor);
    if let Composed::Erased(owner) = &value.owner {
        let source = owner
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix();
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
        assert_eq!(owner.kernels().len(), 2);
    }
}
fn release(
    value: PreparedRefinedForwardingNativeOutputV1,
    receipt: RefinedForwardingNativeStorageV1,
    budget: &mut Budget<'_>,
) {
    drop(value);
    budget.release_storage(receipt.retained_storage()).unwrap();
}
fn text_refusal(result: Result<()>) {
    assert!(matches!(
        result,
        Err(ProductionPipelineError::InductionRefinementNativeStage(
            InductionRefinementNativeStageErrorV1::ForwardingComposition(
                RefinedForwardingNativeStageErrorV1::Mismatch(
                    "exact final refined-forwarding native LLVM"
                )
            )
        ))
    ));
}
#[test]
fn refined_forwarding_native_both_actual_rewrites_share_one_middle_owner_and_final_text() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, true, |prefix, budget| {
                let original = match &prefix {
                    Prefix6::Direct(owner) => {
                        owner.source_semantic_kir().pre_ranked_executable().unwrap()
                    }
                    Prefix6::Erased(owner) => owner.original_source().executable(),
                }
                .canonical()
                .canonical_bytes()
                .as_ptr();
                let sibling = vec![0x71u8; 43];
                let bytes = size_of_val(&sibling) + sibling.capacity();
                budget.reserve_storage(bytes).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let (value, receipt) = prepare(
                    prefix,
                    profile,
                    Limits::default(),
                    ForwardingLimits::default(),
                    budget,
                )
                .unwrap();
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
                shape(&value, erased, 2, 2, budget);
                value.verify_equivalence(budget).unwrap();
                for (graph, final_graph) in [
                    (value.output(), true),
                    (value.refinement_output(), false),
                    (value.licm_input(), false),
                ] {
                    let (text, bytes) = lower_native(graph, profile, budget).unwrap();
                    budget.reserve_storage(bytes).unwrap();
                    if final_graph {
                        assert_eq!(text, value.llvm_ir());
                    } else {
                        assert_ne!(text, value.llvm_ir());
                        text_refusal(check_native_text(value.output(), profile, &text, budget));
                    }
                    drop(text);
                    budget.release_storage(bytes).unwrap();
                }
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x71; 43]);
                release(value, receipt, budget);
                assert_eq!(budget.storage(), floor);
                drop(sibling);
                budget.release_storage(bytes).unwrap();
            });
        }
    }
}
#[test]
fn refined_forwarding_native_noop_and_original_clobber_controls_remain_distinct() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for erased in [false, true] {
            with_prefix(erased, profile, false, |prefix, budget| {
                let (value, receipt) = prepare(
                    prefix,
                    profile,
                    Limits::default(),
                    ForwardingLimits::default(),
                    budget,
                )
                .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                shape(&value, erased, 0, 0, budget);
                assert_eq!(
                    value.licm_input().canonical().canonical_bytes(),
                    value.output().canonical().canonical_bytes()
                );
                value.verify_equivalence(budget).unwrap();
                release(value, receipt, budget);
            });
        }
        with_backend_licm_erased_prefix_v1(profile, true, |prefix, _, budget| {
            let (value, receipt) = prepare(
                Prefix6::Erased(prefix),
                profile,
                Limits::default(),
                ForwardingLimits::default(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            shape(&value, true, 2, 0, budget);
            assert_eq!(
                value.refinement_output().canonical().canonical_bytes(),
                value.output().canonical().canonical_bytes()
            );
            value.verify_equivalence(budget).unwrap();
            release(value, receipt, budget);
        });
    }
}
#[test]
fn refined_forwarding_native_text_target_floor_and_real_history_substitution_refuse() {
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let (mut first, receipt) = prepare(
            prefix,
            Profile::Gfx942,
            Limits::default(),
            ForwardingLimits::default(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let original = first.llvm.remove(0);
        first
            .llvm
            .insert(0, if original == ';' { ' ' } else { ';' });
        text_refusal(first.verify_equivalence(budget));
        first.llvm.remove(0);
        first.llvm.insert(0, original);
        first.profile = Profile::Gfx950;
        assert!(matches!(
            first.verify_equivalence(budget),
            Err(ProductionPipelineError::TargetLowering(_))
        ));
        first.profile = Profile::Gfx942;
        first.retained_floor += 1;
        let before = budget.work();
        assert!(matches!(
            first.verify_equivalence(budget),
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                P7Error::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!((budget.work(), budget.storage()), (before, floor));
        first.retained_floor -= 1;
        with_prefix(false, Profile::Gfx950, true, |prefix, other| {
            let (mut second, receipt) = prepare(
                prefix,
                Profile::Gfx950,
                Limits::default(),
                ForwardingLimits::default(),
                other,
            )
            .unwrap();
            other.reserve_storage(receipt.retained_storage()).unwrap();
            assert_ne!(
                first.prefix_execution.canonical_bytes(),
                second.prefix_execution.canonical_bytes()
            );
            assert_eq!(
                first.prefix_execution.retained_storage(),
                second.prefix_execution.retained_storage()
            );
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            assert!(matches!(
                first.verify_equivalence(budget),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Execution("Policy7 complete execution transcript")
                ))
            ));
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            second.verify_equivalence(other).unwrap();
            release(second, receipt, other);
        });
        first.verify_equivalence(budget).unwrap();
        assert_eq!(budget.storage(), floor);
        release(first, receipt, budget);
    });
}

#[test]
fn refined_forwarding_native_fresh_preparations_match_all_graphs_origins_text_and_resources() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let mut previous = None;
                for _ in 0..2 {
                    with_prefix(erased, profile, mutation, |prefix, budget| {
                        let start = budget.work();
                        let floor = budget.storage();
                        let (value, receipt) = prepare(
                            prefix,
                            profile,
                            Limits::default(),
                            ForwardingLimits::default(),
                            budget,
                        )
                        .unwrap();
                        assert_eq!(budget.storage(), floor);
                        let observation = (
                            value
                                .original()
                                .unwrap()
                                .canonical()
                                .canonical_bytes()
                                .to_vec(),
                            value.licm_input().canonical().canonical_bytes().to_vec(),
                            value
                                .refinement_output()
                                .canonical()
                                .canonical_bytes()
                                .to_vec(),
                            value.output().canonical().canonical_bytes().to_vec(),
                            value.refinement_origins().to_vec(),
                            value.origins().to_vec(),
                            value.llvm_ir().to_owned(),
                            receipt.retained_storage(),
                            budget.work() - start,
                            budget.peak_storage(),
                        );
                        if let Some(previous) = &previous {
                            assert_eq!(previous, &observation);
                        }
                        previous = Some(observation);
                        drop(value);
                    });
                }
            }
        }
    }
}
