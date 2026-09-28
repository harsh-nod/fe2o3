use super::*;
use crate::production_analysis::canonical_ranked_checks_v1::private::tests::{
    fixture, with_projection,
};

std::thread_local! {
    static FAULT: std::cell::Cell<Option<(usize, u8)>> = const { std::cell::Cell::new(None) };
}
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        FAULT.with(|fault| fault.set(None));
    }
}

pub(crate) fn with_coverage_fault<T>(
    position: usize,
    kind: u8,
    run: impl FnOnce() -> T,
) -> (T, bool) {
    assert!(position < 9 && kind < 5);
    assert!(FAULT.with(|fault| fault.get()).is_none());
    let _reset = Reset;
    FAULT.with(|fault| fault.set(Some((position, kind))));
    let result = run();
    let reached = FAULT.with(|fault| fault.get()).is_none();
    (result, reached)
}
pub(super) fn inject(position: usize, pending: &mut Option<PrivateStageCoverageV1>) {
    let selected = FAULT.with(|fault| fault.get());
    let Some((wanted, kind)) = selected else {
        return;
    };
    if position != wanted {
        return;
    }
    FAULT.with(|fault| fault.set(None));
    if kind == 0 {
        *pending = None;
        return;
    }
    let row = pending.as_mut().expect("production coverage prepared");
    match kind {
        1 => row.pass = PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2[(position + 1) % 9],
        2 => row.epoch = row.epoch.wrapping_add(1),
        3 => row.operations += 1,
        _ => row.private_counts[4] += 1,
    }
}

#[test]
fn each_of_nine_positions_requires_its_own_real_coverage_before_the_producer() {
    for position in 0..9 {
        for kind in 0..5 {
            let _reset = Reset;
            with_projection(&fixture(), |projection, budget| {
                projection.with_function(1, budget, |input| {
                    FAULT.with(|fault| fault.set(Some((position, kind))));
                    let error = run(
                        input,
                        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                        None,
                    )
                    .err()
                    .expect("missing or stale coverage must reject");
                    assert!(matches!(error, PipelineErrorV1::CanonicalPrivateInput));
                    assert!(
                        FAULT.with(|fault| fault.get()).is_none(),
                        "fault must be reached at actual stage"
                    );
                })?;
                Ok(())
            })
            .unwrap();
        }
    }
}

#[test]
fn literal_private_increment_formulas_and_each_exact_one_under_limit() {
    let phase = ProductionAnalysisResourcePhaseV1::ReportValidation;
    let setup = resources::setup().unwrap();
    assert_eq!(
        (
            setup.work_upper_bound(),
            setup.retained_storage_upper_bound(),
            setup.peak_storage_upper_bound()
        ),
        (128, 128, 128)
    );
    let record = resources::record().unwrap();
    assert_eq!(
        (
            record.work_upper_bound(),
            record.retained_storage_upper_bound(),
            record.peak_storage_upper_bound()
        ),
        (32, 0, 4)
    );
    let finish = resources::finish().unwrap();
    assert_eq!(
        (
            finish.work_upper_bound(),
            finish.retained_storage_upper_bound(),
            finish.peak_storage_upper_bound()
        ),
        (116, 0, 4)
    );
    for bound in [setup, record, finish] {
        let work = bound.work_upper_bound();
        let peak = bound.peak_storage_upper_bound();
        assert!(
            ProductionAnalysisResourceLimitsV1::new(work, peak)
                .require(phase, bound)
                .is_ok()
        );
        assert!(
            ProductionAnalysisResourceLimitsV1::new(work - 1, peak)
                .require(phase, bound)
                .is_err()
        );
        assert!(
            ProductionAnalysisResourceLimitsV1::new(work, peak - 1)
                .require(phase, bound)
                .is_err()
        );
    }
    with_projection(&fixture(), |projection, budget| {
        for (ordinal, rows) in [(0, 2usize), (1, 5usize)] {
            projection.with_function(ordinal, budget, |input| {
                assert_eq!(input.operation_count(), rows);
                let stage = resources::stage(input, phase).unwrap();
                let expected = 32 * (rows + 1) * (rows + 1) + 32;
                assert_eq!(
                    (
                        stage.work_upper_bound(),
                        stage.retained_storage_upper_bound(),
                        stage.peak_storage_upper_bound()
                    ),
                    (expected, 0, 6)
                );
                assert_eq!(
                    input.identity_lookup_work(),
                    Some(512 * (rows + 1) * (rows + 1))
                );
                assert!(
                    ProductionAnalysisResourceLimitsV1::new(expected, 6)
                        .require(phase, stage)
                        .is_ok()
                );
                assert!(
                    ProductionAnalysisResourceLimitsV1::new(expected - 1, 6)
                        .require(phase, stage)
                        .is_err()
                );
                assert!(
                    ProductionAnalysisResourceLimitsV1::new(expected, 5)
                        .require(phase, stage)
                        .is_err()
                );
            })?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn ordinary_native_entry_is_still_closed_while_private_entry_keeps_real_reports() {
    with_projection(&fixture(), |projection, budget| {
        for ordinal in 0..2 {
            projection.with_function(ordinal, budget, |input| {
                let ordinary = require_production_pliron_checks_v2(
                    input.context(),
                    input.function(),
                    None,
                    None,
                    ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                );
                assert!(ordinary.is_err());
                let private = run(
                    input,
                    ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                    None,
                )
                .unwrap();
                assert_eq!(private.report.paired_stage_count(), 9);
                assert!(private.report.reports().is_clean());
                assert_eq!(private.report.reports().pass_order().len(), 9);
            })?;
        }
        Ok(())
    })
    .unwrap();
}

#[path = "canonical_private_nine_failures_v1_tests.rs"]
mod nine_failures;
