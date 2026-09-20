//! Interpreter-backed tests over synthetic KIR; no ordinary-source claim.
use super::*;
use fe2o3_kir_sim::{
    SimulationDebugBarrierActionV1 as BarrierAction, SimulationDebugCheckpointPhaseV1 as Phase,
    SimulationDebugRecordKindV1 as Kind, SimulationLimitsV1,
};
use std::collections::{BTreeMap, BTreeSet};

fn before(record: &SimulationDebugRecordV1) -> bool {
    matches!(
        record.kind,
        Kind::Checkpoint {
            phase: Phase::BeforeOperation,
            ..
        }
    )
}

#[test]
fn repeated_loop_helpers_keep_runtime_tokens_and_default_capture_is_unchanged() {
    let (module, request) = fixtures::loops();
    for seeded in [false, true] {
        let (expected, legacy) = fixtures::legacy(
            &module,
            &request,
            fixtures::simulation_limits(),
            seeded,
            4096,
        );
        let (actual, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            seeded,
            Some(fixtures::origin_limits(4096)),
            4096,
        );
        assert!(actual.is_ok());
        fixtures::assert_result_eq(&actual, &expected);
        assert_eq!(observed.transcript, legacy);
        assert_eq!(observed.retained.coverage, Coverage::Complete);
        assert_eq!(observed.retained.rows.len(), legacy.records().len());
        let mut helper_activations = BTreeMap::<_, BTreeSet<_>>::new();
        let mut call_attempts = BTreeMap::<_, BTreeSet<_>>::new();
        let mut budget = ScanBudget::new(MAX_ROWS).unwrap();
        for (index, record) in observed.transcript.records().iter().enumerate() {
            let origin = observed.origin_at(index).unwrap();
            if !before(record) {
                continue;
            }
            let after = observed.paired_after(index, &mut budget).unwrap();
            let completed = observed.origin_at(after).unwrap();
            assert_eq!(record.invocation, completed.record.invocation);
            assert_eq!(record.site, completed.record.site);
            assert_eq!(origin.row, completed.row);
            if record.site.function_ordinal == 1 {
                helper_activations
                    .entry(record.invocation.global)
                    .or_default()
                    .insert(origin.row.activation);
            }
            if record.site.function_ordinal == 0
                && record.site.block.0 == 1
                && record.site.operation == 0
            {
                assert_eq!(origin.row.activation, 1);
                call_attempts
                    .entry(record.invocation.global)
                    .or_default()
                    .insert(origin.row.attempt);
            }
        }
        assert_eq!(helper_activations.len(), 2);
        for activations in helper_activations.values() {
            // Each observed helper enters two leaves, then one empty helper
            // consumes an activation without inventing an operation record.
            assert_eq!(activations.iter().copied().collect::<Vec<_>>(), [2, 6, 10]);
        }
        assert_eq!(call_attempts.len(), 2);
        assert!(call_attempts.values().all(|attempts| attempts.len() == 3));

        let (disabled_result, disabled) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            seeded,
            None,
            4096,
        );
        fixtures::assert_result_eq(&disabled_result, &expected);
        assert_eq!(disabled.transcript, legacy);
        assert_eq!(disabled.retained.rows.capacity(), 0);
        assert_eq!(disabled.origin_at(0).unwrap_err(), Missing::Disabled);
    }
}

#[test]
fn memory_records_share_the_exact_enclosing_attempt_and_pairing_skips_no_work() {
    let (module, request) = fixtures::memory();
    let (_, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    let memory: Vec<_> = observed
        .transcript
        .records()
        .iter()
        .enumerate()
        .filter_map(|(index, record)| matches!(record.kind, Kind::Memory { .. }).then_some(index))
        .collect();
    assert_eq!(memory.len(), 2);
    for index in memory {
        let origin = observed.origin_at(index).unwrap();
        let start = (0..index)
            .rev()
            .find(|prior| {
                let candidate = observed.origin_at(*prior).unwrap();
                before(candidate.record)
                    && candidate.row == origin.row
                    && candidate.record.invocation == origin.record.invocation
            })
            .unwrap();
        let mut budget = ScanBudget::new(64).unwrap();
        let after = observed.paired_after(start, &mut budget).unwrap();
        assert!(start < index && index < after);
        assert_eq!(observed.origin_at(after).unwrap().row, origin.row);
        assert_eq!(64 - budget.remaining(), after - start + 1);
    }
}

#[test]
fn yielded_calls_ignore_other_invocations_but_charge_their_rows() {
    let (module, request) = fixtures::barrier();
    for seeded in [false, true] {
        let (result, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            seeded,
            Some(fixtures::origin_limits(4096)),
            4096,
        );
        assert!(result.is_ok());
        let mut saw_interleaved = false;
        let mut releases = 0;
        for (index, record) in observed.transcript.records().iter().enumerate() {
            if matches!(
                record.kind,
                Kind::WorkgroupBarrier {
                    action: BarrierAction::Release,
                    ..
                }
            ) {
                assert_eq!(
                    observed.origin_at(index).unwrap_err(),
                    Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)
                );
                releases += 1;
            }
            if record.site.function_ordinal != 0 || !before(record) {
                continue;
            }
            let mut budget = ScanBudget::new(4096).unwrap();
            let after = observed.paired_after(index, &mut budget).unwrap();
            assert_eq!(
                observed.transcript.records()[after].invocation,
                record.invocation
            );
            assert_eq!(4096 - budget.remaining(), after - index + 1);
            saw_interleaved |= observed.transcript.records()[index + 1..after]
                .iter()
                .any(|other| other.invocation != record.invocation);
        }
        assert_eq!(releases, 2);
        assert!(saw_interleaved);
    }
}

#[test]
fn pair_budget_is_cumulative_and_fault_attempt_is_not_completion() {
    let (module, request) = fixtures::memory();
    let (_, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    let mut budget = ScanBudget::new(4).unwrap();
    assert_eq!(observed.paired_after(0, &mut budget), Ok(1));
    assert_eq!(budget.remaining(), 2);
    assert_eq!(observed.paired_after(0, &mut budget), Ok(1));
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        observed.paired_after(0, &mut budget),
        Err(PairError::WorkLimit)
    );
    assert!(ScanBudget::new(MAX_ROWS + 1).is_err());
    assert_eq!(
        observed.paired_after(1, &mut ScanBudget::new(1).unwrap()),
        Err(PairError::NotBefore)
    );
    let tiny = SimulationLimitsV1 {
        max_steps: 1,
        ..fixtures::simulation_limits()
    };
    let (result, failed) = fixtures::observed(
        &module,
        &request,
        tiny,
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    let (legacy_result, legacy) = fixtures::legacy(&module, &request, tiny, false, 4096);
    assert!(result.is_err());
    fixtures::assert_result_eq(&result, &legacy_result);
    assert_eq!(failed.transcript, legacy);
    assert!(failed.transcript.terminal_fault().is_some());
    let final_index = failed.transcript.records().len() - 1;
    assert!(before(&failed.transcript.records()[final_index]));
    assert_eq!(
        failed.paired_after(final_index, &mut ScanBudget::new(64).unwrap()),
        Err(PairError::MissingAfter)
    );
}

#[test]
fn changed_pair_site_token_missing_prefix_and_wrong_phase_are_not_pairs() {
    let (module, request) = fixtures::memory();
    for mode in 0..4 {
        let (_, mut observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            false,
            Some(fixtures::origin_limits(64)),
            4096,
        );
        match mode {
            0 => observed.transcript.records[1].site.operation += 1,
            1 => observed.retained.rows[1].attempt += 1,
            2 => {
                observed.retained.rows[1].status =
                    Status::RuntimeUnavailable(RuntimeMissing::IdentityInvariant)
            }
            3 => {
                observed.retained.rows.truncate(1);
                observed.retained.coverage = Coverage::PrefixTruncated(Cutoff::RowLimit);
            }
            _ => unreachable!(),
        }
        assert!(
            observed
                .paired_after(0, &mut ScanBudget::new(64).unwrap())
                .is_err()
        );
    }
}

#[test]
fn borrowed_lookups_are_stable_but_fresh_capture_storage_is_not_an_exchangeable_owner() {
    let (module, request) = fixtures::memory();
    let (first_result, first) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    let (second_result, second) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(64)),
        4096,
    );
    assert!(first_result.is_ok() && second_result.is_ok());
    assert!(!first.transcript.records().is_empty());
    assert_eq!(first.transcript, second.transcript);
    for index in (0..first.transcript.records().len()).rev() {
        let left = first.origin_at(index).unwrap();
        let right = second.origin_at(index).unwrap();
        assert_eq!(left.row, right.row); // Tokens alone intentionally repeat.
        assert!(!std::ptr::eq(left.record, right.record));
        assert!(std::ptr::eq(
            left.record,
            first.origin_at(index).unwrap().record
        ));
    }
    assert_eq!(
        first
            .origin_at(first.transcript.records().len())
            .unwrap_err(),
        Missing::NoSuchRecord
    );
    // No query accepts a detached Observation or caller-supplied transcript.
}
