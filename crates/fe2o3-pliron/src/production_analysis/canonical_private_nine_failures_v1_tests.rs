//! Failure tests are a child of the existing private pipeline test module.
//! This gives the old coverage injector test-only access without production APIs.
use super::*;
use crate::production_analysis::canonical_ranked_checks_v1::private::tests::nine_oracle::{
    self as oracle, Expected,
    numbers::{self, Oracle, Triple},
};
use crate::production_analysis::canonical_ranked_checks_v1::private::tests::nine_oracle::{
    Limits, Phase,
};

fn predict(work: usize, peak: usize) -> Option<(usize, Triple, Expected)> {
    for ordinal in 0..2 {
        let o = Oracle::derive(ordinal);
        let floor = oracle::floor(ordinal);
        for gate in &o.gates {
            let total = floor.then(gate.required);
            let resource = if total.w > work {
                if gate.name == "capture reservation" {
                    "remaining identity capture work upper bound"
                } else {
                    numbers::WORK
                }
            } else if total.p > peak {
                numbers::PEAK
            } else {
                continue;
            };
            return Some((
                ordinal,
                gate.before,
                Expected::Quota {
                    phase: gate.phase,
                    cause: gate.cause,
                    resource,
                },
            ));
        }
    }
    None
}

fn assert_limit(work: usize, peak: usize) {
    match predict(work, peak) {
        Some((ordinal, local, expected)) => {
            oracle::public_failure(ordinal, Limits::new(work, peak), local, expected)
        }
        None => {
            assert_eq!((work, peak), (numbers::module().w, numbers::module().p));
            // The exact composed success is exercised in the oracle host.
        }
    }
}

#[test]
fn private_nine_work_boundary_exact_and_one_short_reach_both_real_definitions() {
    let full = numbers::module();
    for ordinal in 0..2 {
        let o = Oracle::derive(ordinal);
        for gate in o.gates.iter().filter(|gate| gate.work_cut) {
            let work = oracle::floor(ordinal).w + gate.required.w;
            assert!(work > 0, "{}", gate.name);
            assert_limit(work - 1, full.p);
            assert_limit(work, full.p);
        }
    }
}

#[test]
fn private_nine_strict_storage_boundaries_and_later_definition_retained_floor() {
    let full = numbers::module();
    let mut maximum = 0;
    let mut counts = [0usize; 2];
    for ordinal in 0..2 {
        let o = Oracle::derive(ordinal);
        let floor = oracle::floor(ordinal);
        for gate in &o.gates {
            let required = floor.then(gate.required).p;
            if required <= maximum {
                continue;
            }
            maximum = required;
            counts[ordinal] += 1;
            assert_limit(full.w, required - 1);
            assert_limit(full.w, required);
        }
    }
    assert_eq!(maximum, full.p);
    // The caller's historical trace peak masks all helper identity/storage cuts.
    // Only the helper's own trace adds a later-definition global peak.
    assert_eq!(counts, [9, 1]);
    let caller = Oracle::derive(0);
    let helper = Oracle::derive(1);
    assert_eq!(full.p, caller.complete.r + helper.complete.p);
    let (ordinal, _, _) = predict(full.w, full.p - 1).unwrap();
    assert_eq!(ordinal, 1);
    for o in [&caller, &helper] {
        for stage in &o.stages {
            assert_eq!(stage.checkpoint.p, o.complete.p);
            assert_eq!(stage.record.p, o.complete.p);
        }
    }
}

#[test]
fn private_nine_masked_later_storage_is_reported_as_the_earlier_trace_denial() {
    let full = numbers::module();
    let caller = Oracle::derive(0);
    let helper = Oracle::derive(1);
    for stage in &helper.stages {
        let hypothetical = oracle::floor(1).then(stage.checkpoint).p - 1;
        let (ordinal, _, kind) = predict(full.w, hypothetical).unwrap();
        assert_eq!(ordinal, 1);
        assert!(matches!(
            kind,
            Expected::Quota {
                phase: Phase::InvocationTrace,
                ..
            }
        ));
    }
    // Pretending the second function starts with zero retained reports is short.
    assert!(full.p > caller.complete.p.max(helper.complete.p));
    assert_limit(full.w, helper.complete.p - 1);
}

#[test]
fn private_nine_each_caller_coverage_position_has_exact_prepare_and_one_short_prefix() {
    let o = Oracle::derive(0);
    let full = numbers::module();
    for (position, stage) in o.stages.iter().enumerate() {
        for kind in 0..5 {
            let (_, reached) = with_coverage_fault(position, kind, || {
                oracle::public_failure(
                    0,
                    Limits::new(stage.prepared.w, full.p),
                    stage.prepared.held(),
                    Expected::Coverage,
                );
            });
            assert!(reached, "position={position} fault={kind}");
            let (_, reached) = with_coverage_fault(position, kind, || {
                oracle::public_failure(
                    0,
                    Limits::new(stage.prepared.w - 1, full.p),
                    stage.before,
                    Expected::Quota {
                        phase: stage.phase,
                        cause: numbers::Cause::Resource(None),
                        resource: numbers::WORK,
                    },
                );
            });
            assert!(!reached, "denied coverage must not run the injector");
        }
    }
}

#[test]
fn private_nine_each_helper_coverage_position_is_direct_state_not_consumer_injection() {
    let o = Oracle::derive(1);
    for (position, stage) in o.stages.iter().enumerate() {
        for kind in 0..5 {
            oracle::direct_failure(1, stage.prepared.held(), Expected::Coverage, |run| {
                let (result, reached) = with_coverage_fault(position, kind, run);
                assert!(reached, "position={position} fault={kind}");
                result
            });
        }
    }
}

#[test]
fn private_nine_completed_pairs_retain_actual_private_occurrences_and_checkpoint_identity() {
    with_projection(&fixture(), |projection, budget| {
        for ordinal in 0..2 {
            projection.with_function(ordinal, budget, |input| {
                let expected = Oracle::derive(ordinal).complete;
                let outcome = run(input, oracle::exact_limits(expected), None).unwrap();
                assert_eq!(oracle::triple(outcome.resource_upper_bound), expected);
                let counts = if ordinal == 0 {
                    [0, 0, 0, 0, 1]
                } else {
                    [1, 0, 1, 1, 0]
                };
                let report = outcome.report;
                for (position, pair) in report.coverage.stages.iter().enumerate() {
                    let checkpoint = &report.reports().preservation().certificates()[position];
                    assert_eq!(pair.position, position);
                    assert_eq!(pair.pass, oracle::PASS_ORDER[position]);
                    assert_eq!(pair.pass, checkpoint.pass());
                    assert_eq!(pair.epoch, input.epoch());
                    assert_eq!(pair.epoch, checkpoint.mutation_epoch());
                    assert_eq!(pair.identity, checkpoint.identity());
                    assert_eq!(pair.operations, [2, 5][ordinal]);
                    assert_eq!(pair.private_counts, counts);
                }
                oracle::assert_report(&report, ordinal);
            })?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn private_nine_entry_and_actual_tensor_panic_have_exact_distinct_prefixes() {
    let o = Oracle::derive(0);
    let full = numbers::module();
    pipeline_entry_panic();
    oracle::public_failure(
        0,
        oracle::exact_limits(full),
        Triple::ZERO,
        Expected::EntryPanic,
    );
    {
        let _reset = crate::production_analysis::pliron_pipeline::panic_after_first_production_stage_for_test_v1();
        oracle::public_failure(
            0,
            oracle::exact_limits(full),
            o.tensor_panic(),
            Expected::TensorPanic,
        );
    }
    // Re-entry uses the actual private producer after both test hooks reset.
    with_projection(&fixture(), |projection, budget| {
        projection.with_function(0, budget, |input| {
            let outcome = run(input, oracle::exact_limits(o.complete), None).unwrap();
            oracle::assert_report(&outcome.report, 0);
            assert_eq!(oracle::triple(outcome.resource_upper_bound), o.complete);
        })?;
        Ok(())
    })
    .unwrap();
}

fn pipeline_entry_panic() {
    crate::production_analysis::pliron_pipeline::panic_next_production_analysis_for_test_v1();
}

#[test]
fn private_nine_helper_entry_and_tensor_panic_preserve_successful_caller_in_direct_state() {
    let o = Oracle::derive(1);
    oracle::direct_failure(1, Triple::ZERO, Expected::EntryPanic, |run| {
        pipeline_entry_panic();
        run()
    });
    oracle::direct_failure(1, o.tensor_panic(), Expected::TensorPanic, |run| {
        let _reset = crate::production_analysis::pliron_pipeline::panic_after_first_production_stage_for_test_v1();
        run()
    });
}

#[test]
fn private_nine_mutate_restore_keeps_private_text_prefix_before_identity_authentication_refusal() {
    let full = numbers::module();
    let o = Oracle::derive(0);
    crate::production_analysis::pliron_pipeline::transiently_mutate_next_production_analysis_for_test_v1();
    oracle::public_failure(
        0,
        oracle::exact_limits(full),
        o.mutation(),
        Expected::Mutation,
    );
    let helper = Oracle::derive(1);
    oracle::direct_failure(1, helper.mutation(), Expected::Mutation, |run| {
        crate::production_analysis::pliron_pipeline::transiently_mutate_next_production_analysis_for_test_v1();
        run()
    });
}

#[test]
fn private_nine_scoped_fault_reset_survives_success_error_and_unwind() {
    for mode in 0..3 {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_coverage_fault(0, 0, || match mode {
                0 => Ok::<_, ()>(()),
                1 => Err(()),
                _ => panic!("coverage guard reset"),
            })
        }));
        assert_eq!(result.is_err(), mode == 2);
        assert!(FAULT.with(|fault| fault.get()).is_none());
    }
}

#[test]
fn private_nine_callback_failure_and_foreign_ledger_poison_release_only_after_reports_drop() {
    oracle::callback_failure_matrix();
}
