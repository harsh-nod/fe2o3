//! Constructed genuine source owners; final native lowering is a separate stage.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as RefineOrigin, CanonicalKirLoopLimitsV1 as Limits,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition, CheckedBinaryOperator, Constant, ScalarType,
};
use fe2o3_lower_mir_kernel::{
    ProductionInductionRefinementErrorV1 as RefineError,
    ProductionInductionRefinementOriginV1 as SourceOrigin,
    ProductionOwnedInductionRefinementContinuationV1 as DirectRefined,
    ProductionOwnedUnitLocalInductionRefinementContinuationV1 as ErasedRefined,
};

#[path = "production_pipeline_induction_refinement_source_resources_v1_tests.rs"]
mod resources;
#[path = "production_pipeline_induction_refinement_source_sim_v1_tests.rs"]
mod sim;

#[allow(
    clippy::large_enum_variant,
    reason = "one actual consumed source owner in a test"
)]
enum Refined {
    Direct(DirectRefined),
    Erased(ErasedRefined),
}
impl Refined {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn tail(&self) -> &fe2o3_kernel_opt::OwnedInductionRefinementContinuationV1 {
        match self {
            Self::Direct(v) => v.continuation(),
            Self::Erased(v) => v.continuation(),
        }
    }
    fn origins(&self) -> &[SourceOrigin] {
        match self {
            Self::Direct(v) => v.origins(),
            Self::Erased(v) => v.origins(),
        }
    }
    fn limits(&self) -> Limits {
        match self {
            Self::Direct(v) => v.limits(),
            Self::Erased(v) => v.limits(),
        }
    }
    fn floor(&self) -> usize {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .unwrap()
    }
    fn addition(&self) -> usize {
        match self {
            Self::Direct(v) => v.additional_retained_storage_v1(),
            Self::Erased(v) => v.additional_retained_storage_v1(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> std::result::Result<(), RefineError> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
    }
}
fn refine(
    owner: Licm,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> std::result::Result<(Refined, usize), RefineError> {
    let (owner, storage) = match owner {
        Licm::Direct(v) => {
            let (v, s) = v.continue_induction_refinement_v1(limits, budget)?;
            (Refined::Direct(v), s)
        }
        Licm::Erased(v) => {
            let (v, s) = v.continue_induction_refinement_v1(limits, budget)?;
            (Refined::Erased(v), s)
        }
    };
    Ok((owner, storage.retained_storage()))
}
type Launch = ([u64; 3], [u32; 3]);
fn with_donor(
    erased: bool,
    profile: Profile,
    mutation: bool,
    run: impl FnOnce(Licm, &[Launch], &mut Budget<'_>),
) {
    with_prefix(erased, profile, mutation, |prefix, budget| {
        let floor = budget.storage();
        let (native, receipt) = prepare(prefix, profile, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_shape(&native.owner, mutation);
        native.verify_equivalence(budget).unwrap();
        let launches: Vec<_> = (0..native.output().module().kernels.len())
            .map(|i| source_simulation_launch(&native, i))
            .collect();
        let PreparedLicmNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile: _,
            retained_floor: _,
        } = native;
        let donor_floor = budget.storage();
        // Both real donor objects remain live with their entire original receipt.
        // No historical LLVM text is relabeled as refined output or re-emitted here.
        run(owner, &launches, budget);
        assert_eq!(budget.storage(), donor_floor);
        assert!(!llvm.is_empty());
        drop((prefix_execution, llvm));
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
fn with_refined(
    erased: bool,
    profile: Profile,
    mutation: bool,
    run: impl FnOnce(&Refined, &[Launch], &mut Budget<'_>),
) {
    with_donor(erased, profile, mutation, |prefix, launches, budget| {
        let input = prefix.output().canonical().canonical_bytes().as_ptr();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let (owner, addition) = refine(prefix, Limits::default(), budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(owner.addition(), addition);
        assert_eq!(owner.input().canonical().canonical_bytes().as_ptr(), input);
        budget.reserve_storage(addition).unwrap();
        assert!(owner.floor() <= budget.storage());
        assert!(budget.work_ledger_identity_v1() == ledger);
        run(&owner, launches, budget);
        drop(owner);
        budget.release_storage(addition).unwrap();
        assert_eq!(budget.storage(), floor);
    });
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
fn assert_refinement(owner: &Refined, mutation: bool, budget: &mut Budget<'_>) {
    assert_eq!(owner.limits(), Limits::default());
    assert_eq!(owner.origins().len(), count_operations(owner.input()));
    assert_eq!(owner.tail().origins().len(), owner.origins().len());
    let mut splits = 0;
    for (source, row) in owner.origins().iter().zip(owner.tail().origins()) {
        assert_eq!(source.canonical_origin(), *row);
        assert_eq!(source.synthetic_false_source_statement(), None);
        match *row {
            RefineOrigin::Unchanged { input, output } => {
                assert_eq!(
                    operation(owner.input(), input),
                    operation(owner.output(), output)
                );
                assert_eq!(source.synthetic_overflow_definition(), None);
            }
            RefineOrigin::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                ..
            } => {
                splits += 1;
                assert!(
                    source.original_source_statement().is_some(),
                    "actual source CheckedAdd has a retained assignment origin"
                );
                assert_eq!(
                    source.synthetic_overflow_definition(),
                    Some(Definition::Result {
                        operation: input,
                        result: 1
                    })
                );
                let old = operation(owner.input(), input);
                let sum = operation(owner.output(), sum_output);
                let flag = operation(owner.output(), false_output);
                let OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs,
                    rhs,
                } = old.kind
                else {
                    panic!("actual checked induction update")
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
        "nonzero genuine final-graph refinement, not a vacuous positive"
    );
    assert_eq!(
        count_operations(owner.output()),
        count_operations(owner.input()) + splits
    );
    assert_eq!(
        owner.input().module().kernels,
        owner.output().module().kernels
    );
    assert!(!std::ptr::eq(owner.input(), owner.output()));
    if !mutation {
        assert_eq!(
            owner.input().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
    }
    match owner {
        Refined::Direct(v) => {
            assert_eq!(v.kernels().len(), 2);
            assert!(!v.grants_artifact_or_launch_authority());
        }
        Refined::Erased(v) => {
            assert_eq!(v.kernels().len(), 2);
            assert!(!v.grants_artifact_or_launch_authority());
        }
    }
    let floor = budget.storage();
    let (pair, storage) = owner
        .tail()
        .replay_against(owner.input(), owner.limits(), budget)
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(std::ptr::eq(pair.input(), owner.input()));
    assert!(std::ptr::eq(pair.output(), owner.output()));
    assert_eq!(pair.origins(), owner.tail().origins());
    // Borrowed pair has no Drop implementation; end its lexical use before credit.
    budget.release_storage(storage.retained_storage()).unwrap();
    owner.replay(budget).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_induction_refinement_actual_two_checked_updates_both_owners_and_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_refined(erased, profile, true, |owner, _, budget| {
                assert_refinement(owner, true, budget)
            });
        }
    }
}
#[test]
fn source_induction_refinement_actual_noops_keep_exact_graph_and_fresh_reports() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_refined(erased, profile, false, |owner, _, budget| {
                assert_refinement(owner, false, budget)
            });
        }
    }
}
#[test]
fn source_induction_refinement_source_cap_is_typed_and_never_clamped() {
    for erased in [false, true] {
        with_donor(erased, Profile::Gfx942, true, |prefix, _, budget| {
            // Bind the fixture's complete public default without reading private fields.
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
            let result = refine(
                prefix,
                Limits {
                    operations: source_limit + 1,
                    ..Limits::default()
                },
                budget,
            );
            assert!(
                matches!(result,Err(RefineError::SourceOperationsLimit{requested,source_limit:actual}) if requested==source_limit+1 && actual==source_limit)
            );
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn source_induction_refinement_replay_requires_its_live_source_and_output_floor() {
    for erased in [false, true] {
        with_refined(erased, Profile::Gfx942, true, |owner, _, budget| {
            let live = budget.storage();
            let required = owner.floor();
            let mut work = Work::new(1_000_000);
            let mut short = Budget::new(&mut work, live);
            short.reserve_storage(required - 1).unwrap();
            assert!(matches!(
                owner.replay(&mut short),
                Err(RefineError::Resource(Resource::Accounting))
            ));
            assert_eq!(short.storage(), required - 1);
            assert_eq!(short.work(), 0);
            assert_eq!(budget.storage(), live);
            owner.replay(budget).unwrap();
        });
    }
}
