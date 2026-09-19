// Constructed semantic source, real materialization/P6/deletion; not Rustc input.
use super::*;
use crate::ProductionRedundantStoreAdmissionErrorV1 as StoreError;
#[path = "production_checked_output_commutative_direct_v1_tests.rs"]
mod commutative_tests;
#[path = "production_checked_output_owned_redundant_store_direct_v1_tests.rs"]
mod owned_tests;

fn source(kill: Option<SemanticStatementKindV1>, stores: usize) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let slot = place(5, U32);
            let statement =
                |kind| SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind);
            let mut statements = vec![statement(SemanticStatementKindV1::StorageLive(
                slot.local(),
            ))];
            for ordinal in 0..stores {
                if ordinal == 1
                    && let Some(kill) = &kill
                {
                    statements.push(statement(kill.clone()));
                }
                statements.push(statement(SemanticStatementKindV1::Store(
                    SemanticMemoryStoreV1::new(
                        slot.clone(),
                        constant(U32, 99, 4),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    ),
                )));
            }
            statements.push(assignment(
                6,
                U32,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slot.clone())),
            ));
            statements.push(statement(SemanticStatementKindV1::StorageDead(
                slot.local(),
            )));
            vec![block(31, statements, SemanticTerminatorKindV1::Return)]
        },
        |_| "private_array_relation".to_owned(),
        &[U32, U32],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn with_fixture(
    profile: Profile,
    kill: Option<SemanticStatementKindV1>,
    stores: usize,
    next: impl FnOnce(&Final6, &fe2o3_kernel_opt::CheckedRedundantStoreOutputV1<'_>, usize),
) {
    let input = fixture6_from_source(profile, source(kill, stores));
    let prefix_floor = input.floor;
    let owner = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(prefix_floor).unwrap();
    let deletion =
        fe2o3_kernel_opt::optimize_checked_redundant_store_v1(owner.output(), &mut budget).unwrap();
    assert_eq!(budget.storage(), prefix_floor);
    let floor = prefix_floor + deletion.retained_storage();
    next(&owner, &deletion, floor);
}

#[test]
fn redundant_store_direct_genuine_long_run_and_noop_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for stores in [1, 4] {
            with_fixture(profile, None, stores, |owner, deletion, floor| {
                assert_eq!(deletion.rows().len(), stores - 1);
                assert_eq!(private_counts(owner.output().module()).1, stores);
                assert_eq!(private_counts(deletion.output().module()).1, 1);
                assert_eq!(
                    owner.output().canonical().canonical_bytes()
                        != deletion.output().canonical().canonical_bytes(),
                    stores > 1
                );
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let storage = {
                    let (checked, storage) = owner
                        .check_redundant_store_output_v1(deletion, &mut budget)
                        .unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert!(std::ptr::eq(checked.input(), owner.output()));
                    assert!(std::ptr::eq(checked.output(), deletion.output()));
                    assert!(!checked.grants_artifact_or_launch_authority());
                    budget.reserve_storage(storage.retained_storage()).unwrap();
                    checked.verify_equivalence(&mut budget).unwrap();
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    storage
                };
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn redundant_store_direct_zero_kir_lifetime_restart_is_not_erased() {
    with_fixture(
        Profile::Gfx942,
        Some(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(5),
        )),
        2,
        |owner, deletion, floor| {
            assert_eq!(deletion.rows().len(), 1);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let result = owner.check_redundant_store_output_v1(deletion, &mut budget);
            assert!(matches!(
                result,
                Err(StoreError::Admission(
                    crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                        phase: "redundant Store source",
                        detail: "no lifetime or Move invalidation between identical Stores",
                    }
                ))
            ));
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn redundant_store_direct_foreign_equal_graph_owner_is_refused() {
    with_fixture(Profile::Gfx942, None, 2, |owner, _, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (foreign, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(owner.output().module(), &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            foreign.canonical().canonical_bytes()
        );
        let deletion =
            fe2o3_kernel_opt::optimize_checked_redundant_store_v1(&foreign, &mut budget).unwrap();
        budget.reserve_storage(deletion.retained_storage()).unwrap();
        let before = budget.storage();
        assert!(matches!(
            owner.check_redundant_store_output_v1(&deletion, &mut budget),
            Err(StoreError::Admission(
                crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                    phase: "redundant Store",
                    detail: "exact retained Policy6 input owner",
                }
            ))
        ));
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn redundant_store_direct_exact_and_short_admission_limits_restore_floor() {
    with_fixture(Profile::Gfx942, None, 2, |owner, deletion, floor| {
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = owner.check_redundant_store_output_v1(deletion, &mut budget);
            let success = result.is_ok();
            drop(result);
            assert_eq!(budget.storage(), floor);
            (success, budget.work(), budget.peak_storage())
        };
        let (success, work, peak) = run(WORK, STORAGE);
        assert!(success);
        assert!(peak > floor);
        assert!(run(work, peak).0);
        assert!(!run(work - 1, peak).0);
        assert!(!run(work, peak - 1).0);
    });
}
