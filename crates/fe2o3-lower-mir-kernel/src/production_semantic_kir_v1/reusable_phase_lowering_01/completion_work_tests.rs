//! Completion-state accounting only; no fabricated SSA owner or source proof.
use super::*;

const LINEAR: &str = "phase emission left an unconsumed linear result";
const WORK: &str = "phase emission exhausted existing shared work";

fn state(counts: &[usize]) -> State {
    State {
        phases: counts
            .iter()
            .map(|count| PhaseState {
                owner_loan: None,
                completion: None,
                leases: (0..*count)
                    .map(|_| LeaseState {
                        loan: None,
                        value: None,
                        closed: true,
                    })
                    .collect(),
                started: true,
                ended: true,
            })
            .collect(),
        restored: Vec::new(),
    }
}

fn assert_reason(result: PhaseResult<()>, expected: &str) {
    match result.unwrap_err() {
        ProductionSemanticKirErrorV1::Unsupported { detail, .. } => assert_eq!(detail, expected),
        error => panic!("unexpected completion error: {error:?}"),
    }
}

#[test]
fn complete_charges_every_phase_and_every_visited_lease_exactly_once() {
    for counts in [vec![], vec![0], vec![1], vec![0, 3, 17, 129, 0]] {
        let state = state(&counts);
        let required = counts.len() + counts.iter().sum::<usize>();
        let mut exact = required;
        state.complete(&mut exact).unwrap();
        assert_eq!(exact, 0);
        if required != 0 {
            let mut one_less = required - 1;
            assert_reason(state.complete(&mut one_less), WORK);
            assert_eq!(one_less, 0);
        }
    }
}

#[test]
fn each_unconsumed_lease_kind_keeps_its_original_rejection_and_prefix_cost() {
    for index in [0, 2, 4] {
        for kind in 0..3 {
            let mut state = state(&[5, 9]);
            let lease = &mut state.phases[0].leases[index];
            match kind {
                0 => lease.closed = false,
                1 => lease.loan = Some(SemanticValueBindingV1::Unmaterialized),
                2 => lease.value = Some(SemanticValueBindingV1::Unmaterialized),
                _ => unreachable!(),
            }
            // Phase + visited lease prefix only: later leases/phases stay unvisited.
            let visited = 1 + index + 1;
            let mut work = 100;
            assert_reason(state.complete(&mut work), LINEAR);
            assert_eq!(work, 100 - visited);
            let mut exact_prefix = visited;
            assert_reason(state.complete(&mut exact_prefix), LINEAR);
            assert_eq!(exact_prefix, 0);
        }
    }
}

#[test]
fn outer_linear_rejection_precedes_any_lease_traversal() {
    for kind in 0..4 {
        let mut state = state(&[17, 3]);
        let phase = &mut state.phases[0];
        match kind {
            0 => phase.started = false,
            1 => phase.ended = false,
            2 => phase.owner_loan = Some(SemanticValueBindingV1::Unmaterialized),
            3 => phase.completion = Some(SemanticValueBindingV1::Unmaterialized),
            _ => unreachable!(),
        }
        let mut work = 1;
        assert_reason(state.complete(&mut work), LINEAR);
        assert_eq!(work, 0);
    }
}

#[test]
fn exhaustion_precedes_inspection_of_the_unfunded_invalid_lease() {
    let mut state = state(&[4]);
    state.phases[0].leases[3].closed = false;
    let mut missing_final_debit = 4;
    assert_reason(state.complete(&mut missing_final_debit), WORK);
    assert_eq!(missing_final_debit, 0);
    let mut funded_final_debit = 5;
    assert_reason(state.complete(&mut funded_final_debit), LINEAR);
    assert_eq!(funded_final_debit, 0);
    assert!(!state.phases[0].leases[3].closed);
}

#[test]
fn repeated_completion_checks_keep_spent_work_and_do_not_change_state() {
    let state = state(&[2, 3]);
    let mut shared = 14;
    state.complete(&mut shared).unwrap();
    assert_eq!(shared, 7);
    state.complete(&mut shared).unwrap();
    assert_eq!(shared, 0);
    assert_reason(state.complete(&mut shared), WORK);
    assert_eq!(shared, 0);
    for phase in &state.phases {
        assert!(phase.started && phase.ended);
        assert!(phase.owner_loan.is_none() && phase.completion.is_none());
        for lease in &phase.leases {
            assert!(lease.closed && lease.loan.is_none() && lease.value.is_none());
        }
    }
}
