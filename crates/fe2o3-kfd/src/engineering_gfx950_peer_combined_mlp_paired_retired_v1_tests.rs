use super::*;

fn gate(writes: [u64; 2], reads: [u64; 2]) -> Result<()> {
    retired_gate(
        [[0; PACKETS]; 2],
        [(5, 3); 2],
        [(5, 3); 2],
        writes,
        writes,
        [3; 2],
        std::array::from_fn(|rank| (writes[rank], reads[rank])),
    )
}

#[test]
fn retired_gate_accepts_intervening_completed_work_without_read_credit() {
    assert!(gate([5; 2], [3; 2]).is_ok());
    assert!(gate([10, 15], [7, 12]).is_ok());
    // The legacy in-flight gate must still reject exactly this advancement.
    assert!(completion_gate([[0; PACKETS]; 2], [true; 2], [(10, 7); 2], [5; 2], [3; 2]).is_err());
    assert!(
        retired_gate(
            [[0; PACKETS]; 2],
            [(5, 3); 2],
            [(10, 7); 2],
            [15; 2],
            [15; 2],
            [7; 2],
            [(15, 12); 2]
        )
        .is_ok()
    );
}

#[test]
fn retired_gate_refuses_every_nonzero_signal_and_unknown_kind() {
    for rank in 0..2 {
        for slot in 0..PACKETS {
            for value in [i64::MIN, -1, 1, 2, i64::MAX] {
                let mut signals = [[0; PACKETS]; 2];
                signals[rank][slot] = value;
                assert!(
                    retired_gate(
                        signals,
                        [(5, 3); 2],
                        [(5, 3); 2],
                        [10; 2],
                        [10; 2],
                        [3; 2],
                        [(10, 7); 2]
                    )
                    .is_err()
                );
            }
        }
    }
    for kind in [-1, 0, 2, i64::MAX] {
        if kind != AMD_SIGNAL_KIND_USER_V1 {
            assert!(signal_value(kind, 0).is_err());
        }
    }
}

#[test]
fn retired_gate_refuses_busy_regressed_or_invented_frontiers() {
    for rank in 0..2 {
        for mutation in 0..9 {
            let mut sealed = [(5, 3); 2];
            let mut previous = [(10, 7); 2];
            let mut writes = [15; 2];
            let mut completed = [15; 2];
            let mut reads = [7; 2];
            let mut observed = [(15, 12); 2];
            match mutation {
                0 => completed[rank] = 10,
                1 => writes[rank] = 9,
                2 => reads[rank] = 6,
                3 => observed[rank].0 = 16,
                4 => observed[rank].1 = 6,
                5 => observed[rank].1 = 16,
                6 => previous[rank].0 = 4,
                7 => previous[rank].1 = 2,
                8 => sealed[rank].1 = 6,
                _ => unreachable!(),
            }
            assert!(
                retired_gate(
                    [[0; PACKETS]; 2],
                    sealed,
                    previous,
                    writes,
                    completed,
                    reads,
                    observed
                )
                .is_err()
            );
        }
    }
    assert!(gate([0; 2], [0; 2]).is_err());
    assert!(gate([u64::MAX; 2], [3; 2]).is_err());
}
