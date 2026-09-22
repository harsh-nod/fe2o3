//! Recovery keeps only inert transport inputs, never the producer's recipe owner.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
use fe2o3_lower_mir_kernel::encode_production_ranked_source_rows_v1;
use fe2o3_pliron::encode_production_ranked_recipe_v1;

#[path = "compiler_native_source_packet_recovery_v1_tests.rs"]
mod complete_packet;

struct DurableFixture {
    semantic: Vec<u8>,
    native: Vec<u8>,
    native_graph: Vec<u8>,
    middle: Roster,
    correspondence: Roster,
    verus: Roster,
    recipe: Vec<u8>,
    source_rows: Vec<u8>,
    signature: InertFunctionalRefinementReceiptSignatureV2,
    staging: NativeCompilerStagingCommitmentV1,
}

impl DurableFixture {
    fn packet(
        &self,
        erased: Option<&[u8]>,
        budget: &mut Budget<'_>,
    ) -> Result<(Vec<u8>, NativeCompilerSourcePacketStorageV1), E> {
        self.replay(&[self.signature], budget, |source, ranked_roots, budget| {
            encode_native_compiler_source_packet_v1(
                NativeCompilerRankedRecipeSourceProofInputsV1 {
                    source,
                    ranked_roots,
                },
                erased,
                budget,
            )
        })
    }

    fn capture(fixture: Fixture) -> Self {
        let Fixture {
            semantic,
            native,
            native_graph,
            middle,
            correspondence,
            verus,
            kernel,
            signature,
            staging,
        } = fixture;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (recipe, recipe_storage) =
            encode_production_ranked_recipe_v1(&kernel, &mut budget).unwrap();
        budget
            .reserve_storage(recipe_storage.retained_storage())
            .unwrap();
        let (source_rows, _) =
            encode_production_ranked_source_rows_v1(&access(), &[], &mut budget).unwrap();
        budget
            .release_storage(recipe_storage.retained_storage())
            .unwrap();
        drop(kernel);
        Self {
            semantic,
            native,
            native_graph,
            middle,
            correspondence,
            verus,
            recipe,
            source_rows,
            signature,
            staging,
        }
    }

    fn replay<T>(
        &self,
        receipts: &[InertFunctionalRefinementReceiptSignatureV2],
        budget: &mut Budget<'_>,
        replay: impl FnOnce(
            NativeCompilerSourceProofInputsV1<'_>,
            &[NativeCompilerRankedRecipeRootV1<'_>],
            &mut Budget<'_>,
        ) -> Result<T, E>,
    ) -> Result<T, E> {
        let launch = [ProductionSourceLaunchRootInputV1::new(
            NAME,
            BINDING,
            ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )];
        let commitments = [self.staging];
        let staging = [NativeCompilerRootStagingV1 {
            semantic_root: 0,
            commitments: &commitments,
        }];
        let roots = [NativeCompilerRankedRecipeRootV1 {
            semantic_root: 0,
            launch_rank: 1,
            recipe_bytes: &self.recipe,
            source_rows_bytes: &self.source_rows,
            ranked_ir: TEXT,
            effect_receipts: receipts,
        }];
        replay(
            NativeCompilerSourceProofInputsV1 {
                semantic_mir: &self.semantic,
                native_module: &self.native,
                middle_end_roster: self.middle.canonical_bytes(),
                correspondence_roster: self.correspondence.canonical_bytes(),
                verus_roster: self.verus.canonical_bytes(),
                launch_inputs: &launch,
                staging_roots: &staging,
            },
            &roots,
            budget,
        )
    }

    fn direct(
        &self,
        receipts: &[InertFunctionalRefinementReceiptSignatureV2],
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            ValidatedNativeCompilerRankedSourceProofV1,
            NativeCompilerRankedSourceProofStorageV1,
        ),
        E,
    > {
        self.replay(receipts, budget, |source, ranked_roots, budget| {
            validate_native_compiler_ranked_recipe_source_proof_v1(
                NativeCompilerRankedRecipeSourceProofInputsV1 {
                    source,
                    ranked_roots,
                },
                budget,
            )
        })
    }
}

fn access() -> [ProductionRankedAccessSourceV1; 1] {
    [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)]
}

#[test]
fn durable_recipe_direct_recovery_reimports_and_retains_independent_owner() {
    let original = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (typed, typed_storage) = original
        .validate(&original.kernel, &access(), &mut budget)
        .unwrap();
    drop(typed);
    let durable = DurableFixture::capture(original);
    budget.reserve_storage(37).unwrap();
    let (checked, storage) = durable.direct(&[durable.signature], &mut budget).unwrap();
    assert_eq!(budget.storage(), 37);
    // Discarded decoded recipes must not inflate the final retained receipt.
    assert_eq!(storage.retained_storage(), typed_storage.retained_storage());
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        checked
            .source()
            .source()
            .pre_ranked_executable()
            .unwrap()
            .canonical()
            .canonical_bytes(),
        durable.native_graph
    );
    assert_eq!(
        checked.middle_end_roster().canonical_bytes(),
        durable.middle.canonical_bytes()
    );
    assert_eq!(
        checked.correspondence_roster().canonical_bytes(),
        durable.correspondence.canonical_bytes()
    );
    assert_eq!(
        checked.verus_roster().canonical_bytes(),
        durable.verus.canonical_bytes()
    );
    drop(durable);
    checked.source().source().verify_equivalence().unwrap();
    assert_eq!(checked.root_count(), 1);
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn durable_recipe_unit_local_recovery_uses_freshly_admitted_e_not_erasure_producer() {
    let (fixture, producer) = unit_local_erased_fixture::unit_fixture();
    let erased_bytes = producer.erased().canonical().canonical_bytes().to_vec();
    let durable = DurableFixture::capture(fixture);
    drop(producer);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(73).unwrap();
    let (erased, erased_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
            &erased_bytes,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(erased_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (checked, storage) = durable
        .replay(
            &[durable.signature],
            &mut budget,
            |original, ranked_roots, budget| {
                validate_native_compiler_unit_local_erased_recipe_source_proof_v1(
                    NativeCompilerUnitLocalErasedRecipeSourceProofInputsV1 {
                        original,
                        ranked_roots,
                        erased: &erased,
                    },
                    budget,
                )
            },
        )
        .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        checked
            .source()
            .source()
            .erased()
            .canonical()
            .canonical_bytes(),
        erased_bytes
    );
    assert_eq!(
        checked
            .source()
            .source()
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        durable.native_graph
    );
    assert_eq!(
        checked.middle_end_roster().canonical_bytes(),
        durable.middle.canonical_bytes()
    );
    assert_eq!(
        checked.correspondence_roster().canonical_bytes(),
        durable.correspondence.canonical_bytes()
    );
    assert_eq!(
        checked.verus_roster().canonical_bytes(),
        durable.verus.canonical_bytes()
    );
    drop(durable);
    drop(erased);
    budget
        .release_storage(erased_storage.retained_storage())
        .unwrap();
    assert_eq!(checked.root_count(), 1);
    assert!(!checked.proves_whole_operational_or_indexed_address_equivalence());
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 73);
}

#[test]
fn durable_recipe_rejects_missing_extra_and_foreign_receipts_and_wrong_source_map() {
    let mut durable = DurableFixture::capture(fixture());
    let foreign = InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
        *durable.signature.wire(),
        [91; 32],
    );
    for receipts in [
        vec![],
        vec![durable.signature, durable.signature],
        vec![foreign],
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        assert!(durable.direct(&receipts, &mut budget).is_err());
        assert_eq!(budget.storage(), 37);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    durable.source_rows = encode_production_ranked_source_rows_v1(&[], &[], &mut budget)
        .unwrap()
        .0;
    budget.reserve_storage(37).unwrap();
    assert!(durable.direct(&[durable.signature], &mut budget).is_err());
    assert_eq!(budget.storage(), 37);
}

#[test]
fn durable_recipe_decode_does_not_approve_changed_typed_operand() {
    let mut fixture = fixture();
    let mut operations = fixture.kernel.blocks()[0].operations().to_vec();
    let ProductionRankedOperationV1::SemanticExpression {
        expression: ProductionSemanticExpressionV2::Constant { bits, .. },
        ..
    } = &mut operations[3]
    else {
        panic!()
    };
    *bits = 8;
    fixture.kernel = ProductionRankedKernelV1::new(
        NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let durable = DurableFixture::capture(fixture);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    let error = durable
        .direct(&[durable.signature], &mut budget)
        .err()
        .unwrap();
    assert!(matches!(error, E::RankedCompile(_)), "{error:?}");
    assert_eq!(budget.storage(), 37);
}

#[test]
fn durable_recipe_rejects_incomplete_or_substituted_root_metadata() {
    let durable = DurableFixture::capture(fixture());
    for change in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        let result = durable.replay(
            &[durable.signature],
            &mut budget,
            |source, roots, budget| {
                let mut changed = vec![roots[0]];
                match change {
                    0 => changed.clear(),
                    1 => changed.push(roots[0]),
                    2 => changed[0].semantic_root = 1,
                    3 => changed[0].launch_rank = 2,
                    4 => changed[0].ranked_ir = "changed",
                    _ => unreachable!(),
                }
                validate_native_compiler_ranked_recipe_source_proof_v1(
                    NativeCompilerRankedRecipeSourceProofInputsV1 {
                        source,
                        ranked_roots: &changed,
                    },
                    budget,
                )
            },
        );
        assert!(matches!(result, Err(E::Mismatch(_))));
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn durable_recipe_partial_progress_denials_restore_floor_and_history() {
    let durable = DurableFixture::capture(fixture());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    durable.direct(&[durable.signature], &mut budget).unwrap();
    let full_work = budget.work();
    let peak = budget.peak_storage();
    for (work_limit, storage_limit) in [
        (0, STORAGE),
        (8, STORAGE),
        (full_work - 1, STORAGE),
        (WORK, 37),
        (WORK, peak - 1),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(37).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(durable.direct(&[durable.signature], &mut budget).is_err());
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
}

#[test]
fn durable_recipe_rejects_truncated_source_rows_after_recipe_import() {
    let mut durable = DurableFixture::capture(fixture());
    durable.source_rows.pop().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let error = durable
        .direct(&[durable.signature], &mut budget)
        .err()
        .unwrap();
    assert!(matches!(error, E::RankedSourceRowsWire(_)), "{error:?}");
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert!(budget.peak_storage() > 37);
}

#[test]
fn durable_recipe_does_not_replace_decoded_source_rows_with_expected_rows() {
    use fe2o3_lower_mir_kernel::{
        ProductionRankedExecutableEffectOriginV1 as Origin,
        ProductionRankedExecutableEffectSourceV1 as Effect,
        decode_production_ranked_source_rows_v1,
    };
    let mut durable = DurableFixture::capture(fixture());
    let extra_effect = Effect::new(0, 0, 0, 8, Origin::GeneratedFromSemanticTerminator, [3; 32]);
    for (access, effects) in [
        (
            vec![ProductionRankedAccessSourceV1::new(0, Some(0), 0, 0, 8)],
            vec![],
        ),
        (
            vec![ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 7)],
            vec![],
        ),
        (vec![access()[0], access()[0]], vec![]),
        (access().to_vec(), vec![extra_effect]),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        durable.source_rows =
            encode_production_ranked_source_rows_v1(&access, &effects, &mut budget)
                .unwrap()
                .0;
        let (decoded, _) =
            decode_production_ranked_source_rows_v1(&durable.source_rows, &mut budget).unwrap();
        assert_eq!(decoded.access_sources(), access);
        assert_eq!(decoded.executable_effect_sources(), effects);
        drop(decoded);
        budget.reserve_storage(37).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let error = durable
            .direct(&[durable.signature], &mut budget)
            .err()
            .unwrap();
        assert!(matches!(error, E::Source(_)), "{error:?}");
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
}
