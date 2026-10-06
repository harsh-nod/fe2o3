// Independent switch-specific arithmetic, not a measured fixed-nine baseline.
use super::*;
use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
    InvocationReceiptFailureV1, InvocationReceiptV1,
};
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitsV1 as Limits, ProductionAnalysisResourcePhaseV1 as Phase,
    ProductionAnalysisResourceUpperBoundV1 as Bound,
};
use crate::production_analysis::pliron_switch_verification_v1::{
    SwitchVerificationCensusV1, census_switch_verification_v1,
    census_switch_verification_with_observation_v1,
};

fn expected(typed: bool, keys: usize) -> SwitchVerificationCensusV1 {
    assert_eq!(std::mem::size_of::<usize>(), 8);
    assert_eq!(std::mem::size_of::<Vec<u64>>(), 24);
    assert_eq!(std::mem::size_of::<String>(), 24);
    let edges = keys + 1;
    let (key_work, key_scratch) = if typed {
        (2 * keys + 32, 0)
    } else if keys <= 16 {
        (2 * keys + 32 + keys * keys.saturating_sub(1) / 2, 0)
    } else {
        (29 * keys + 8 * 512 + 32, 2 * keys + 256 + 6 + 16)
    };
    SwitchVerificationCensusV1 {
        // P=2E, selector's defining roster=3; both incoming definitions also
        // have roster3. Type queries cost 3+E*(2*(3+1)+2*(3+2)).
        traversal_work: 128 + 32 * edges,
        callback_work: 131 + 90 * edges + key_work,
        callback_scratch: 64 + key_scratch.max(16 + 6),
    }
}

#[test]
fn native_switch_callback_literals_cover_empty_pairwise_radix_and_typed_paths() {
    assert_eq!(
        (
            expected(false, 0).traversal_work,
            expected(false, 0).callback_work,
            expected(false, 0).callback_scratch
        ),
        (160, 253, 86)
    );
    assert_eq!(
        (
            expected(false, 1).traversal_work,
            expected(false, 1).callback_work,
            expected(false, 1).callback_scratch
        ),
        (192, 345, 86)
    );
    assert_eq!(
        (
            expected(false, 16).traversal_work,
            expected(false, 16).callback_work,
            expected(false, 16).callback_scratch
        ),
        (672, 1845, 86)
    );
    assert_eq!(
        (
            expected(false, 17).traversal_work,
            expected(false, 17).callback_work,
            expected(false, 17).callback_scratch
        ),
        (704, 6372, 376)
    );
    assert_eq!(
        (
            expected(true, 17).traversal_work,
            expected(true, 17).callback_work,
            expected(true, 17).callback_scratch
        ),
        (704, 1817, 86)
    );
    for typed in [false, true] {
        for count in [0, 1, 16, 17] {
            with_projection(&switch_module(typed, count), |projection, budget| {
                projection.test_live(|context, root| {
                    assert_eq!(
                        census_switch_verification_v1(
                            context,
                            live_switch(context, root),
                            Limits::production_hard_ceiling()
                        )
                        .unwrap(),
                        expected(typed, count)
                    );
                });
                projection.check(budget).unwrap();
            });
        }
    }
}

#[test]
fn native_switch_actual_identity_census_has_all_keys_edges_payloads_and_callback_costs() {
    for typed in [false, true] {
        for count in [0, 1, 16, 17] {
            with_projection(&switch_module(typed, count), |projection, budget| {
                projection
                    .with_function(0, budget, |context, function| {
                        let capture =
                            LivePlironStructuralIdentityProviderV1::new(context, function)
                                .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
                                .ok()
                                .expect("native switch must enter actual production identity");
                        let census = capture.input_census;
                        assert_eq!(
                            (
                                census.blocks,
                                census.operations,
                                census.results,
                                census.block_arguments
                            ),
                            (2, 2, 0, 5)
                        );
                        assert_eq!(
                            (census.operands, census.successors, census.attributes),
                            (1 + 2 * (count + 1), count + 1, 6)
                        );
                        assert_eq!(census.max_operation_arity, 1 + 2 * (count + 1));
                        assert_eq!(census.max_successor_arity, count + 1);
                        let expected = expected(typed, count);
                        assert_eq!(
                            census.native_switch_verification_work,
                            expected.callback_work
                        );
                        assert_eq!(
                            census.native_switch_verification_scratch,
                            expected.callback_scratch
                        );
                    })
                    .unwrap();
            });
        }
    }
}

#[test]
fn native_switch_each_census_prefix_preserves_exact_accepted_history_with_nonzero_floor() {
    let phase_kind = Phase::StructuralIdentity;
    let floor = Bound::checked_phase(phase_kind, 11, 5, 2).unwrap();
    for typed in [false, true] {
        for count in [0, 1, 16, 17] {
            let expected = expected(typed, count);
            let full_work = expected.traversal_work + expected.callback_work;
            for cut in 0..6 {
                let (work, peak, accepted_work, accepted_peak, resource) = match cut {
                    0 => (127, usize::MAX - 5, 0, 0, Some("work upper bound")),
                    1 => (
                        expected.traversal_work - 1,
                        usize::MAX - 5,
                        128,
                        64,
                        Some("work upper bound"),
                    ),
                    2 => (
                        full_work - 1,
                        usize::MAX - 5,
                        expected.traversal_work,
                        64,
                        Some("work upper bound"),
                    ),
                    3 => (usize::MAX - 11, 63, 0, 0, Some("peak storage upper bound")),
                    4 => (
                        usize::MAX - 11,
                        expected.callback_scratch - 1,
                        expected.traversal_work,
                        64,
                        Some("peak storage upper bound"),
                    ),
                    _ => (
                        full_work,
                        expected.callback_scratch,
                        full_work,
                        expected.callback_scratch,
                        None,
                    ),
                };
                with_projection(&switch_module(typed, count), |projection, _| {
                    projection.test_live(|context, root| {
                        let limits = Limits::new(11 + work, 5 + peak);
                        let mut receipt = InvocationReceiptV1::new(floor, limits).unwrap();
                        let phase = receipt.phase(phase_kind, 0).unwrap();
                        let result = census_switch_verification_with_observation_v1(
                            context,
                            live_switch(context, root),
                            Limits::production_hard_ceiling(),
                            Some(&phase.observer(&Ok)),
                        );
                        if result.is_ok() {
                            phase
                                .commit(
                                    Bound::checked_phase(
                                        phase_kind,
                                        full_work,
                                        0,
                                        expected.callback_scratch,
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                        } else {
                            // An uncommitted phase holds its accepted peak
                            // reservation until the outer owner is dropped.
                            drop(phase);
                        }
                        let state = receipt.snapshot();
                        assert_eq!(
                            state.committed.work_upper_bound(),
                            accepted_work,
                            "typed={typed} count={count} cut={cut}"
                        );
                        assert_eq!(
                            state.committed.retained_storage_upper_bound(),
                            if resource.is_some() { accepted_peak } else { 0 }
                        );
                        assert_eq!(state.committed.peak_storage_upper_bound(), accepted_peak);
                        assert_eq!(state.current, state.committed);
                        assert!(!state.caught_panic);
                        if let Some(resource) = resource {
                            let error = result.unwrap_err();
                            assert_eq!(error.phase, phase_kind);
                            assert_eq!(error.resource, resource);
                            assert_eq!(state.first_denial, Some(error));
                            assert_eq!(
                                receipt.complete(),
                                Err(InvocationReceiptFailureV1::Denied(error))
                            );
                        } else {
                            assert_eq!(result.unwrap(), expected);
                            assert_eq!(state.first_denial, None);
                            assert_eq!(receipt.complete(), Ok(state.committed));
                        }
                    });
                });
            }
        }
    }
}
