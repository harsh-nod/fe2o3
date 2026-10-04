//! Genuine paid ordinary source observer. Not a whole-memory or nominal BF16 grant.
//! Exact LLVM/KIR copying reuses the original bounded oracle unchanged.
use super::*;
use fe2o3_pliron::{
    ProductionRankedAnalysisAllowanceV1 as Analysis,
    ProductionRankedSnapshotAllowanceV1 as Presentation,
};

const ANALYSIS_WORK: usize = 281_474_976_710_656;
const ANALYSIS_STORAGE: usize = 93_323_264;
const SNAPSHOT_BYTES: usize = 1_048_576;
const WINDOW_WORK: usize = 8;
const WINDOW_SCRATCH: usize = 1_184;

fn selected_profile() -> std::result::Result<(Analysis, Presentation, Value), String> {
    if usize::BITS != 64
        || Budget::STORAGE_WINDOW_WORK_V1 != WINDOW_WORK
        || Budget::STORAGE_WINDOW_SCRATCH_V1 != WINDOW_SCRATCH
    {
        return Err("paid fixture profile requires the frozen 64-bit window layout".into());
    }
    let analysis = Analysis::new(ANALYSIS_WORK, ANALYSIS_STORAGE).map_err(|e| e.to_string())?;
    let snapshot = Presentation::new(SNAPSHOT_BYTES, ANALYSIS_WORK).map_err(|e| e.to_string())?;
    if analysis != Analysis::production_hard_ceiling()
        || snapshot != Presentation::production_hard_ceiling()
    {
        return Err("paid fixture profile differs from frozen existing hard ceilings".into());
    }
    let (work, storage) = super::super::super::ranked_allowance::quote_values(
        analysis.max_work(),
        snapshot.max_work(),
        analysis.max_peak_storage(),
    )
    .map_err(|e| e.to_string())?;
    let source_work = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
        .map_err(|_| "source work ceiling conversion")?;
    if work >= source_work || storage >= crate::production_canonical_phase_policy_v1::STORAGE_LIMIT
    {
        return Err("paid fixture profile leaves no source-prefix allowance".into());
    }
    Ok((
        analysis,
        snapshot,
        json!({
            "name": "gfx942-existing-analysis-snapshot-ceilings-v1",
            "analysis": { "work": analysis.max_work().to_string(), "peak_storage": analysis.max_peak_storage().to_string() },
            "snapshot": { "bytes": snapshot.max_bytes().to_string(), "work": snapshot.max_work().to_string() },
            "window": { "work": WINDOW_WORK.to_string(), "scratch": WINDOW_SCRATCH.to_string() },
            "prepaid_work": work.to_string(), "prepaid_peak_storage": storage.to_string(),
            "constructor_context_import_charged": false,
            "all_later_phase_work_charged": false, "whole_memory_envelope": false,
        }),
    ))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Each branch runs once in its own root-supervised frontend. The paid branch
    /// calls the actual production owning route, not a reconstructed candidate.
    pub(crate) fn observe_paid_ordinary_target_account_for_test_v1(
        self,
        paid: bool,
        target_budget: &mut Budget<'_>,
    ) -> std::result::Result<Value, String> {
        let (analysis, snapshot, profile) = selected_profile()?;
        if !paid {
            let target = self
                .lower_production_target(target_budget)
                .map_err(|e| e.to_string())?;
            let observation = target_record(&target)?;
            drop(target);
            return Ok(
                json!({ "target": observation, "source_account": null, "selected_allowances": null }),
            );
        }
        super::super::super::tests::with_terminal_account_audit_v1(
            || -> std::result::Result<_, String> {
                let target = self
                    .lower_target_with_paid_ranked_source_account_v1(
                        target_budget,
                        analysis,
                        snapshot,
                    )
                    .map_err(|e| e.to_string())?;
                let ledger = &target.account.ledger;
                let address = ledger.as_ref() as *const OwnedBudget as usize;
                let work = ledger.work();
                let storage = ledger.storage();
                let prepaid_work = ANALYSIS_WORK
                    .checked_add(ANALYSIS_WORK)
                    .and_then(|n| n.checked_add(WINDOW_WORK))
                    .ok_or("paid work overflow")?;
                if work < prepaid_work
                    || storage < ANALYSIS_STORAGE
                    || ledger.peak_storage() < storage
                    || ledger.failed_work().is_some()
                    || ledger.failed_storage().is_some()
                {
                    return Err("paid final original-account bounds/refusal history".into());
                }
                let observation = target_record(&target.payload)?;
                let report = json!({
                    "target": observation, "selected_allowances": profile,
                    "source_account": {
                        "work": work.to_string(), "storage": storage.to_string(),
                        "peak": ledger.peak_storage().to_string(),
                        "work_limit": ledger.work_limit().to_string(),
                        "storage_limit": ledger.storage_limit().to_string(),
                        "failed_work": ledger.failed_work().map(|v| v.to_string()),
                        "failed_storage": ledger.failed_storage().map(|v| v.to_string()),
                        "ranked_allowances_prepaid": true,
                        "constructor_context_import_charged": false,
                        "all_later_phase_work_charged": false,
                        "whole_memory_envelope": false,
                    },
                });
                // The audit below accepts only after the actual complete target
                // payload and original source account have dropped in order.
                drop(target);
                Ok((report, (address, work, storage)))
            },
        )
    }
}

#[test]
fn paid_fixture_profile_uses_checked_frozen_ceilings_and_exact_original_quote() {
    let (a, s, profile) = selected_profile().unwrap();
    assert_eq!(a.max_work(), 1_048_576usize * 1_048_576 * 256);
    assert_eq!(
        a.max_peak_storage(),
        1_048_576 * (8 * 2 + 8) + 16_777_216 * 4 + 1_048_576
    );
    assert_eq!(s.max_bytes(), fe2o3_pliron::HARD_MAX_OPERATION_IMPORT_BYTES);
    assert_eq!(profile["prepaid_work"], "562949953421320");
    assert_eq!(profile["prepaid_peak_storage"], "93324448");
    assert!(
        a.max_work() * 2 + WINDOW_WORK
            < crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize
    );
    assert!(
        a.max_peak_storage() + WINDOW_SCRATCH
            < crate::production_canonical_phase_policy_v1::STORAGE_LIMIT
    );
}

#[test]
fn paid_profile_does_not_relabel_unmetered_constructor_or_later_work() {
    let (_, _, profile) = selected_profile().unwrap();
    for key in [
        "constructor_context_import_charged",
        "all_later_phase_work_charged",
        "whole_memory_envelope",
    ] {
        assert_eq!(profile[key], false);
    }
    assert!(profile.get("later_phase_work_charged_to_source").is_none());
}

#[test]
fn paid_observer_calls_actual_complete_route_and_original_byte_oracle() {
    let source = include_str!("paid_target_account_v1_tests.rs");
    let body = source
        .split("pub(crate) fn observe_paid_ordinary_target_account_for_test_v1(")
        .nth(1)
        .unwrap()
        .split("\n#[test]")
        .next()
        .unwrap();
    assert_eq!(
        body.matches(".lower_target_with_paid_ranked_source_account_v1(")
            .count(),
        1
    );
    assert_eq!(body.matches(".lower_production_target(").count(), 1);
    assert_eq!(body.matches("target_record(").count(), 2);
    assert!(body.contains("with_terminal_account_audit_v1"));
    assert!(!body.contains("observe_ordinary_target_account_for_test_v1"));
}
