// Real constructor/translation-engine controls using the existing admitted
// no-op semantic fixture. These are not rustc frontend/source-authority tests.
// The fixture has no native helper Call: it covers actual charged entry scans,
// not positive template-cache allocation. Existing native helper controls
// separately cover cache exact/short storage and expansion.
use super::*;
use crate::ProductionFormalMemoryOwnerV1 as Formal;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const PREFIX_WORK: usize = 17;
const PREFIX_STORAGE: usize = 31;
const NAME: &str = "paid_translation_noop";

fn ranked_root(
    original: &ProductionPreRankedKirOwnerV1,
) -> ProductionRankedSemanticProjectionRootV1 {
    let name = original.executable().module().kernels[0].id.as_str();
    let source = original.source_launch().roots()[0];
    let layout = source.layout();
    // The generic legacy helper uses grid identity 1. Materialized source
    // custody requires the identity derived from this actual kernel binding.
    let kernel = ProductionRankedKernelV1::new(
        name,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: layout.grid_identity(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel(&format!("{name}_ranked"), kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    ProductionRankedSemanticProjectionRootV1::new(
        source.selected_root(),
        source.source_rank(),
        lowering,
        format!("func @{name} {{\n}}\n"),
        vec![],
        vec![],
    )
}

fn receipt() -> (ProductionMaterializedRankedModuleReceiptV1, usize) {
    let semantic = noop_semantic_owner(&[NAME]);
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        semantic.semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            NAME,
            [240; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let floor = materialized
        .retained_analysis_storage_v1()
        .checked_add(PREFIX_STORAGE)
        .unwrap();
    let root = ranked_root(&materialized);
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            materialized,
            vec![root],
        )
        .unwrap();
    (receipt, floor)
}

fn attached() -> (ProductionSemanticKirOwnerV1, usize) {
    let (receipt, floor) = receipt();
    (
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap(),
        floor,
    )
}

// No budget-backed owner escapes this helper; success and failure payloads
// are dropped before their test ledger. Returned facts are fixed scalars.
fn pipeline(work_limit: usize, storage_limit: Option<usize>) -> (bool, usize, usize, usize, bool) {
    let (receipt, floor) = receipt();
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit.unwrap_or(STORAGE));
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(PREFIX_WORK).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let result = ProductionSemanticKirOwnerV1::
        try_attach_materialized_ranked_checks_with_bounded_translation_budget_v1(receipt, &mut budget)
        .map_err(crate::ProductionFormalMemoryErrorV1::SemanticKir)
        .and_then(|owner| Formal::try_admit_with_bounded_translation_budget_v1(owner, &mut budget));
    let ok = result.is_ok();
    drop(result);
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), floor);
    (
        ok,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_work().is_some(),
    )
}

#[test]
fn actual_connected_attachment_matches_default_on_original_scan_budget() {
    let (legacy, _) = attached();
    let (receipt, floor) = receipt();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let paid = ProductionSemanticKirOwnerV1::
        try_attach_materialized_ranked_checks_with_bounded_translation_budget_v1(receipt, &mut budget).unwrap();
    assert_eq!(paid.module(), legacy.module());
    assert_eq!(paid.correspondence(), legacy.correspondence());
    assert_eq!(
        paid.canonical_kernel_ir_identity(),
        legacy.canonical_kernel_ir_identity()
    );
    assert_eq!(
        paid.mir_pliron_translation_validations().count(),
        legacy.mir_pliron_translation_validations().count()
    );
    assert!(budget.work() > 0);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == identity);
    drop(paid);
}

#[test]
fn actual_formal_admission_keeps_both_semantic_replays_and_both_obligation_derivations() {
    let (owner, floor) = attached();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let identity = budget.work_ledger_identity_v1();
    owner
        .verify_equivalence_with_bounded_translation_budget_v1(&mut budget)
        .unwrap();
    let one = budget.work();
    assert!(one > 0);
    let paid = Formal::try_admit_with_bounded_translation_budget_v1(owner, &mut budget).unwrap();
    assert_eq!(budget.work(), one.checked_mul(3).unwrap());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == identity);
    let (legacy, _) = attached();
    let legacy = Formal::try_admit(legacy).unwrap();
    assert_eq!(paid.kernels(), legacy.kernels());
    assert_eq!(paid.semantic_kir().module(), legacy.semantic_kir().module());
    assert_eq!(
        paid.semantic_kir().canonical_kernel_ir_identity(),
        legacy.semantic_kir().canonical_kernel_ir_identity()
    );
    assert!(!paid.grants_artifact_or_launch_authority());
    paid.verify_equivalence().unwrap();
    assert_eq!(budget.work(), one.checked_mul(3).unwrap()); // default replay still separate
    drop(paid);
}

#[test]
fn combined_translation_work_is_exact_and_one_short_refuses_without_refund() {
    let measured = pipeline(WORK, None);
    assert!(measured.0 && measured.1 > PREFIX_WORK && !measured.4);
    let exact = pipeline(measured.1, Some(measured.3));
    assert_eq!(exact, measured);
    let short = pipeline(measured.1 - 1, Some(measured.3));
    assert!(!short.0 && short.4);
    assert!(short.1 <= measured.1 - 1);
    assert_eq!(short.2, measured.2);
}

#[test]
fn attachment_refuses_zero_remaining_work_and_preserves_live_floor() {
    let (receipt, floor) = receipt();
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let error = ProductionSemanticKirOwnerV1::
        try_attach_materialized_ranked_checks_with_bounded_translation_budget_v1(receipt, &mut budget);
    assert!(matches!(
        error,
        Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::ResourceLimit
        ))
    ));
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn no_helper_fixture_adds_work_not_a_fabricated_cache_reservation() {
    let measured = pipeline(WORK, None);
    assert!(measured.0);
    assert_eq!(measured.2, measured.3);
    assert_eq!(pipeline(measured.1, Some(measured.2)), measured);
}

#[test]
fn formal_first_semantic_replay_refusal_is_not_formal_admission() {
    let (owner, floor) = attached();
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result = Formal::try_admit_with_bounded_translation_budget_v1(owner, &mut budget);
    assert!(matches!(
        result,
        Err(crate::ProductionFormalMemoryErrorV1::SemanticKir(
            ProductionSemanticKirErrorV1::MirPlironTranslation(
                ProductionMirPlironTranslationErrorV1::ResourceLimit
            )
        ))
    ));
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn envelope_admission_preserves_both_paid_replays_and_original_reports() {
    let (owner, floor) = attached();
    let extents = [[128, 1, 1]];
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let identity = budget.work_ledger_identity_v1();
    owner
        .verify_equivalence_with_bounded_translation_budget_v1(&mut budget)
        .unwrap();
    let one = budget.work();
    assert!(one > 0);
    let paid = Formal::try_admit_for_launch_envelopes_with_bounded_translation_budget_v2(
        owner,
        &extents,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.work(), one.checked_mul(3).unwrap());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == identity);
    let (original, _) = attached();
    let original = Formal::try_admit_for_launch_envelopes_v2(original, &extents).unwrap();
    assert_eq!(paid.kernels(), original.kernels());
    assert_eq!(paid.launch_envelopes_v2(), original.launch_envelopes_v2());
    assert_eq!(paid.launch_envelopes_v2().unwrap()[0].extents(), extents[0]);
    assert_eq!(
        paid.semantic_kir().module(),
        original.semantic_kir().module()
    );
    assert!(!paid.grants_artifact_or_launch_authority());
    paid.verify_equivalence().unwrap();
    assert_eq!(budget.work(), one.checked_mul(3).unwrap());
    drop(paid);
}

fn envelope_run(limit: usize, extents: &[[u64; 3]]) -> (bool, usize, bool) {
    let (owner, floor) = attached();
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let result = Formal::try_admit_for_launch_envelopes_with_bounded_translation_budget_v2(
        owner,
        extents,
        &mut budget,
    );
    let ok = result.is_ok();
    drop(result);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == identity);
    (ok, budget.work(), budget.failed_work().is_some())
}

#[test]
fn envelope_admission_exact_and_short_work_preserve_the_original_floor() {
    let extents = [[128, 1, 1]];
    let full = envelope_run(WORK, &extents);
    assert!(full.0 && full.1 > 0 && !full.2);
    assert_eq!(envelope_run(full.1, &extents), full);
    let short = envelope_run(full.1 - 1, &extents);
    assert!(!short.0 && short.2);
    assert!(short.1 < full.1);
    assert_eq!(envelope_run(0, &extents), (false, 0, true));
}

#[test]
fn paid_envelope_admission_still_rejects_missing_or_invalid_envelopes() {
    for extents in [
        vec![],
        vec![[128, 1, 1]; 2],
        vec![[63, 1, 1]],
        vec![[0, 1, 1]],
        vec![[128, 2, 1]],
    ] {
        let result = envelope_run(WORK, &extents);
        assert!(!result.0 && result.1 > 0 && !result.2);
    }
}
