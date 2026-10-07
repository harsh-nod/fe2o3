use super::*;
use fe2o3_runtime_model::{
    DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
    QueueKeyV1, VmIdV1, VmKeyV1,
};

struct InjectedTailWaitCursorV1 {
    expire_after_pauses: usize,
    pauses: usize,
}

impl TailWaitCursorV1 for InjectedTailWaitCursorV1 {
    fn deadline_reached(&self) -> bool {
        self.pauses >= self.expire_after_pauses
    }

    fn pause(&mut self) -> WaitActionV1 {
        self.pauses += 1;
        WaitActionV1::Spin
    }
}

struct ScriptedTailWaitCursorV1 {
    actions: [WaitActionV1; 3],
    next_action: usize,
}

impl TailWaitCursorV1 for ScriptedTailWaitCursorV1 {
    fn deadline_reached(&self) -> bool {
        false
    }

    fn pause(&mut self) -> WaitActionV1 {
        let action = self.actions[self.next_action];
        self.next_action += 1;
        action
    }
}

fn queue_key(queue: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(7),
                generation: DeviceGenerationV1(1),
            },
            id: VmIdV1(3),
        },
        id: QueueInstanceIdV1(queue),
        generation: QueueGenerationV1(5),
    }
}

#[test]
fn exact_fence_encoding_carries_system_scope_and_snoop() {
    let packet = crate::sdma::Gfx942SdmaCopySubmissionV1::new(1, 2, 3, 4, 5).unwrap();
    assert!(gfx942_system_snoop_tail_fence_is_exact_v1());
    assert_eq!(SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1, 0x0053_0005);
    assert_eq!(packet.fence_header(), SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1);
    assert!(tail_signal_generation_is_exact_v1(7, 7));
    assert!(!tail_signal_generation_is_exact_v1(7, 8));
    assert!(!tail_signal_generation_is_exact_v1(0, 0));
}

#[test]
fn noncurrent_spin_budgets_are_confined_to_the_profiled_path() {
    for budget in [
        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros250,
        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros500,
        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Millis1,
        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros1500,
        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Millis3,
    ] {
        assert!(!diagnostic_spin_budget_is_admitted_for_profile_v1::<false>(
            budget
        ));
        assert!(diagnostic_spin_budget_is_admitted_for_profile_v1::<true>(
            budget
        ));
    }
    assert!(diagnostic_spin_budget_is_admitted_for_profile_v1::<false>(
        Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current
    ));
}

#[test]
fn bound_tail_rejects_every_ordered_roster_identity_substitution() {
    let ticket = Gfx942SdmaCopyTicketV1 {
        owner: queue_key(11),
        queue_id: 17,
        slot: 9,
        generation: 23,
    };
    let ordered = [ValidatedMultiQueueCompletionEntryV1 {
        request_index: 0,
        queue_ordinal: 2,
        ticket,
    }];
    let tail = BoundStripedTailV1 {
        queue_ordinal: 2,
        request_index: 0,
        ticket,
    };
    assert!(bound_tail_matches_ordered_roster_v1(&ordered, tail));

    let mutations: [fn(&mut BoundStripedTailV1); 6] = [
        |tail| tail.request_index = 1,
        |tail| tail.queue_ordinal = 3,
        |tail| tail.ticket.owner = queue_key(12),
        |tail| tail.ticket.queue_id = 18,
        |tail| tail.ticket.slot = 10,
        |tail| tail.ticket.generation = 24,
    ];
    for mutate in mutations {
        let mut substituted = tail;
        mutate(&mut substituted);
        assert!(!bound_tail_matches_ordered_roster_v1(&ordered, substituted));
    }
}

#[test]
fn injected_tail_rounds_visit_every_tail_until_ready() {
    let tails = [10_u8, 11, 12, 13];
    let mut observations = Vec::new();
    let mut wait = InjectedTailWaitCursorV1 {
        expire_after_pauses: usize::MAX,
        pauses: 0,
    };
    let ready_mask = observe_tail_rounds_until_v1(
        &tails,
        |tail| 1_u16 << *tail,
        |tail| {
            let round = observations.len() / tails.len();
            observations.push((round, *tail));
            Ok::<bool, ()>(round == 2)
        },
        &mut wait,
    )
    .unwrap();
    assert_eq!(ready_mask, 0b1111 << 10);
    assert_eq!(wait.pauses, 2);
    assert_eq!(observations.len(), 3 * tails.len());
    for observed_round in observations.chunks_exact(tails.len()) {
        assert_eq!(
            observed_round
                .iter()
                .map(|(_, tail)| *tail)
                .collect::<Vec<_>>(),
            tails
        );
    }
}

#[test]
fn profiled_tail_rounds_count_actions_and_readiness_without_changing_order() {
    let tails = [0_u8, 1];
    let mut observations = Vec::new();
    let mut wait = ScriptedTailWaitCursorV1 {
        actions: [
            WaitActionV1::Spin,
            WaitActionV1::Yield,
            WaitActionV1::Sleep(Duration::from_micros(25)),
        ],
        next_action: 0,
    };
    let mut diagnostics = Gfx942SdmaStripedWaitDiagnosticsV1::default();
    let scan_started = Instant::now();
    let ready_mask = observe_tail_rounds_profiled_until_v1::<true, _, _, _>(
        &tails,
        |tail| 1_u16 << *tail,
        |tail| {
            let round = observations.len() / tails.len();
            observations.push((round, *tail));
            Ok::<bool, ()>(round >= 3 || (round >= 1 && *tail == 0))
        },
        &mut wait,
        &mut diagnostics,
        Some(scan_started),
    )
    .unwrap();

    assert_eq!(ready_mask, 0b11);
    assert_eq!(observations.len(), 4 * tails.len());
    assert_eq!(diagnostics.tail_scan_rounds(), 4);
    assert_eq!(diagnostics.tail_observations(), 8);
    assert_eq!(diagnostics.spin_pauses(), 1);
    assert_eq!(diagnostics.yield_pauses(), 1);
    assert_eq!(diagnostics.sleep_pauses(), 1);
    assert_eq!(diagnostics.requested_sleep_ns(), 25_000);
    let first = diagnostics.first_tail_ready_ns().unwrap();
    let all = diagnostics.all_tails_ready_ns().unwrap();
    assert!(all >= first);
}

#[test]
fn compile_time_disabled_tail_scan_does_not_update_diagnostics() {
    let mut wait = InjectedTailWaitCursorV1 {
        expire_after_pauses: usize::MAX,
        pauses: 0,
    };
    let mut diagnostics = Gfx942SdmaStripedWaitDiagnosticsV1::default();
    let ready_mask = observe_tail_rounds_profiled_until_v1::<false, _, _, _>(
        &[0_u8, 1],
        |tail| 1_u16 << *tail,
        |_| Ok::<bool, ()>(true),
        &mut wait,
        &mut diagnostics,
        None,
    )
    .unwrap();
    assert_eq!(ready_mask, 0b11);
    assert_eq!(diagnostics, Gfx942SdmaStripedWaitDiagnosticsV1::default());
}

#[test]
fn cpu_measurement_status_never_substitutes_missing_or_invalid_values() {
    let mut diagnostics = Gfx942SdmaStripedWaitDiagnosticsV1::default();
    record_thread_wait_cpu_measurement_v1(
        &mut diagnostics,
        ThreadWaitCpuMeasurementV1::Available {
            thread_cpu_ns: 17,
            voluntary_context_switches: 2,
            involuntary_context_switches: 3,
        },
    );
    assert_eq!(
        diagnostics.tail_scan_cpu_measurement_status(),
        Gfx942SdmaStripedWaitCpuMeasurementStatusV1::Available
    );
    assert!(diagnostics.tail_scan_cpu_measurement_available());
    assert_eq!(diagnostics.tail_scan_thread_cpu_ns(), Some(17));
    assert_eq!(diagnostics.tail_scan_voluntary_context_switches(), Some(2));
    assert_eq!(
        diagnostics.tail_scan_involuntary_context_switches(),
        Some(3)
    );

    record_thread_wait_cpu_measurement_v1(&mut diagnostics, ThreadWaitCpuMeasurementV1::Invalid);
    assert_eq!(
        diagnostics.tail_scan_cpu_measurement_status(),
        Gfx942SdmaStripedWaitCpuMeasurementStatusV1::Invalid
    );
    assert!(!diagnostics.tail_scan_cpu_measurement_available());
    assert_eq!(diagnostics.tail_scan_thread_cpu_ns(), None);
    assert_eq!(diagnostics.tail_scan_voluntary_context_switches(), None);
    assert_eq!(diagnostics.tail_scan_involuntary_context_switches(), None);
}

#[test]
fn injected_deadline_occurs_only_after_one_complete_pending_round() {
    let tails = [0_u8, 1, 2];
    let mut observations = Vec::new();
    let mut wait = InjectedTailWaitCursorV1 {
        expire_after_pauses: 0,
        pauses: 0,
    };
    let ready_mask = observe_tail_rounds_until_v1(
        &tails,
        |tail| 1_u16 << *tail,
        |tail| {
            observations.push(*tail);
            Ok::<bool, ()>(false)
        },
        &mut wait,
    )
    .unwrap();
    assert_eq!(ready_mask, 0);
    assert_eq!(observations, tails);
}

#[test]
fn injected_tail_error_stops_without_a_full_roster_observation() {
    let tails = [0_u8, 1, 2, 3];
    let mut observations = Vec::new();
    let mut wait = InjectedTailWaitCursorV1 {
        expire_after_pauses: usize::MAX,
        pauses: 0,
    };
    let result = observe_tail_rounds_until_v1(
        &tails,
        |tail| 1_u16 << *tail,
        |tail| {
            observations.push(*tail);
            if *tail == 2 {
                Err("tail error")
            } else {
                Ok(false)
            }
        },
        &mut wait,
    );
    assert_eq!(result, Err("tail error"));
    assert_eq!(observations, [0, 1, 2]);
}

#[test]
fn final_audit_distinguishes_completion_timeout_and_tail_contract_violation() {
    assert_eq!(
        classify_striped_full_audit_v1(true, 0, 0b1111),
        StripedFullAuditDispositionV1::AllReady
    );
    assert_eq!(
        classify_striped_full_audit_v1(true, 0, 0),
        StripedFullAuditDispositionV1::AllReady
    );
    assert_eq!(
        classify_striped_full_audit_v1(false, 0b1010, 0b0101),
        StripedFullAuditDispositionV1::TimedOut
    );
    assert_eq!(
        classify_striped_full_audit_v1(false, 0b0010, 0b1010),
        StripedFullAuditDispositionV1::TailOrderingViolation
    );
    // A tail that transitions to ready during the mandatory final scan is
    // still paired with pending prefixes from that same scan.
    let last_tail_round = 0_u16;
    let final_audit_ready_tails = 0b0010_u16;
    assert_eq!(
        classify_striped_full_audit_v1(false, 0b0010, last_tail_round | final_audit_ready_tails,),
        StripedFullAuditDispositionV1::TailOrderingViolation
    );
}

#[test]
fn injected_final_audit_observes_every_entry_once_before_timeout() {
    let entries = [0_u16, 1, 2, 3, 4, 5, 6];
    for pending in [0_u16, 3, 6] {
        let mut observed = Vec::new();
        let (ready, pending_queue_mask) = observe_full_ordered_roster_v1(&entries, |entry| {
            observed.push(*entry);
            Ok::<(u16, bool), ()>((1, *entry != pending))
        })
        .unwrap();
        assert!(!ready);
        assert_eq!(observed, entries);
        assert_eq!(
            classify_striped_full_audit_v1(ready, pending_queue_mask, 0),
            StripedFullAuditDispositionV1::TimedOut
        );
    }
}

#[test]
fn injected_all_ready_epoch_counts_tail_rounds_one_audit_and_separate_moves() {
    let tails = [0_u8, 1, 2, 3];
    let entries = [0_u16, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut tail_observations = 0_usize;
    let mut wait = InjectedTailWaitCursorV1 {
        expire_after_pauses: usize::MAX,
        pauses: 0,
    };
    let ready_tail_queue_mask = observe_tail_rounds_until_v1(
        &tails,
        |tail| 1_u16 << *tail,
        |_| {
            let round = tail_observations / tails.len();
            tail_observations += 1;
            Ok::<bool, ()>(round == 2)
        },
        &mut wait,
    )
    .unwrap();
    let mut final_observations = 0_usize;
    let (all_ready, pending_mask) = observe_full_ordered_roster_v1(&entries, |_| {
        final_observations += 1;
        Ok::<(u16, bool), ()>((1, true))
    })
    .unwrap();
    assert_eq!(
        classify_striped_full_audit_v1(all_ready, pending_mask, ready_tail_queue_mask,),
        StripedFullAuditDispositionV1::AllReady
    );
    let retirement_moves = entries.len();
    assert_eq!(tail_observations, 3 * tails.len());
    assert_eq!(final_observations, entries.len());
    assert_eq!(retirement_moves, entries.len());
    assert_eq!(
        tail_observations + final_observations,
        3 * tails.len() + entries.len()
    );
}

#[test]
fn injected_identity_substitution_fails_at_exact_tail_or_roster_entry() {
    let expected_tails = [(0_u16, 7_u32, 4_u16, 11_u32), (1, 8, 9, 12)];
    let mut observed_tails = expected_tails;
    observed_tails[1].3 += 1;
    let mut visited = Vec::new();
    let mut wait = InjectedTailWaitCursorV1 {
        expire_after_pauses: usize::MAX,
        pauses: 0,
    };
    let result = observe_tail_rounds_until_v1(
        &observed_tails,
        |tail| 1_u16 << tail.0,
        |tail| {
            let index = visited.len();
            visited.push(*tail);
            if expected_tails.get(index) == Some(tail) {
                Ok(true)
            } else {
                Err("tail generation substitution")
            }
        },
        &mut wait,
    );
    assert_eq!(result, Err("tail generation substitution"));
    assert_eq!(visited, observed_tails);

    let expected_roster = [(0_u16, 0_u16, 4_u16, 11_u32), (1, 1, 9, 12), (2, 0, 5, 13)];
    let mut observed_roster = expected_roster;
    observed_roster[2].1 = 1;
    let mut visited = Vec::new();
    let result = observe_full_ordered_roster_v1(&observed_roster, |entry| {
        let index = visited.len();
        visited.push(*entry);
        if expected_roster.get(index) == Some(entry) {
            Ok((1, true))
        } else {
            Err("ordered roster substitution")
        }
    });
    assert_eq!(result, Err("ordered roster substitution"));
    assert_eq!(visited, observed_roster);
}

#[test]
fn injected_fallible_currentness_step_retains_exact_outer_custody() {
    let retained = Some(super::super::striped_submission_for_unwind_test());
    let exact = retained.as_ref().unwrap().exact_identity_for_unwind_test();
    let result = with_borrowed_striped_submission_v1(&retained, |submission| {
        assert_eq!(submission.exact_identity_for_unwind_test(), exact);
        Err::<(), _>(Gfx942SdmaErrorV1::Contract(
            "injected closing currentness failure",
        ))
    });
    assert!(matches!(
        result,
        Err(Gfx942SdmaErrorV1::Contract(
            "injected closing currentness failure"
        ))
    ));
    assert_eq!(
        retained.as_ref().unwrap().exact_identity_for_unwind_test(),
        exact
    );
}

#[test]
fn every_bound_queue_requires_the_exact_modulo_two_engine() {
    for queue_ordinal in 0..GFX942_SDMA_MAX_STRIPED_QUEUES_V1 {
        let expected = (queue_ordinal % 2) as u32;
        assert!(striped_owner_engine_matches_v1(
            queue_ordinal,
            Some(expected)
        ));
        assert!(!striped_owner_engine_matches_v1(queue_ordinal, None));
        assert!(!striped_owner_engine_matches_v1(
            queue_ordinal,
            Some(expected ^ 1)
        ));
        assert!(!striped_owner_engine_matches_v1(queue_ordinal, Some(2)));
    }
}

#[test]
fn optimized_source_has_no_post_bind_allocation_or_second_full_scan() {
    let source = include_str!("../tail_wait.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    assert!(!source.contains("Vec::"));
    assert!(!source.contains("collect::<"));
    assert!(!source.contains("observe_entire_completion_roster("));
    assert!(!source.contains("retire_prepared_striped_multi_queue_completion"));
    assert!(
        source.contains("GFX942_STRIPED_SDMA_MAX_SLEEP_V1: Duration = Duration::from_micros(25)")
    );
    assert_eq!(
        source
            .matches("MonotonicWaitV1::until_with_sleep_ceiling")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("MonotonicWaitV1::until_with_active_spin_floor_and_sleep_ceiling")
            .count(),
        1
    );
    assert_eq!(source.matches("for entry in entries").count(), 1);
    assert!(source.contains("Gfx942SdmaStripedAllReadyAuditV1<'a>"));
    assert!(source.contains("submission: &'a Gfx942SdmaMultiQueueSubmissionV1"));
    assert!(!source.contains("pub(crate) struct Gfx942SdmaStripedAllReadyAuditV1"));
    assert!(!source.contains("pub(crate) struct Gfx942SdmaStripedRetirementPermitV1"));
    assert!(!source.contains("*const Gfx942SdmaMultiQueueSubmissionV1"));
    assert!(!source.contains("core::ptr"));
    let retirement = source
        .split("fn retire_after_striped_full_audit_infallible_v1")
        .nth(1)
        .unwrap();
    assert!(retirement.contains("permit: Gfx942SdmaStripedRetirementPermitV1"));
    assert!(
        retirement.find("completed.capacity()").unwrap() < retirement.find("for entry in").unwrap()
    );
    assert!(!retirement.contains("observe_validated_slot_in_current_scope"));
    assert!(!retirement.contains("validate_ticket"));
    assert!(!retirement.contains("validated_slot_remains_present"));
    let full_audit = source
        .split("fn full_ordered_striped_audit_v1")
        .nth(1)
        .unwrap()
        .split("fn observe_full_ordered_striped_roster_v1")
        .next()
        .unwrap();
    assert!(!full_audit.contains("diagnostic_spin_budget"));
    assert!(!full_audit.contains("tail_scan_"));
    let no_unwind = source
        .split("fn retire_after_striped_full_audit_no_unwind_v1")
        .nth(1)
        .unwrap()
        .split("fn retire_after_striped_full_audit_infallible_v1")
        .next()
        .unwrap();
    assert!(no_unwind.contains("abort_if_striped_retirement_unwinds_v1"));
}

#[test]
fn retained_record_header_is_derived_from_each_emitted_ordinary_packet() {
    let source = include_str!("../../../sdma.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    assert_eq!(
        source
            .matches("fence_header: packet.fence_header()")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("fence_header: copy.packet.fence_header()")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("fence_header: item.packet.fence_header()")
            .count(),
        1
    );
}
