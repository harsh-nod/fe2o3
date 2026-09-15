use super::*;

fn event(invocation: usize, sequence: usize, index: u64, write: bool) -> MemoryEvent {
    MemoryEvent {
        invocation,
        sequence,
        block: 0,
        operation: sequence,
        allocation: 1,
        indices: vec![index],
        write,
    }
}

#[test]
fn pointwise_read_modify_write_retains_initial_snapshot() {
    let events = (0..64)
        .flat_map(|i| [event(i, 0, i as u64, false), event(i, 1, i as u64, true)])
        .collect::<Vec<_>>();
    let result = versions(&events, 4096).unwrap();
    for pair in result.chunks_exact(2) {
        assert_eq!(pair, [Some(PlironSemanticMemoryVersionV1::Initial), None]);
    }
}

#[test]
fn same_owner_read_after_write_has_distinct_exact_version() {
    let events = vec![
        event(0, 0, 0, false),
        event(0, 1, 0, true),
        event(0, 2, 0, false),
        event(0, 3, 0, true),
        event(0, 4, 0, false),
    ];
    let result = versions(&events, 4096).unwrap();
    assert_eq!(result[0], Some(PlironSemanticMemoryVersionV1::Initial));
    for (read, write) in [(2, 1), (4, 3)] {
        assert_eq!(
            result[read],
            Some(PlironSemanticMemoryVersionV1::AfterWrite {
                invocation: 0,
                event: write,
                block: 0,
                operation: write,
            })
        );
    }
}

#[test]
fn unrelated_write_does_not_advance_read_cell() {
    let events = vec![
        event(0, 0, 0, false),
        event(0, 1, 1, true),
        event(0, 2, 0, false),
    ];
    assert_eq!(
        versions(&events, 4096).unwrap(),
        vec![
            Some(PlironSemanticMemoryVersionV1::Initial),
            None,
            Some(PlironSemanticMemoryVersionV1::Initial),
        ]
    );
}

#[test]
fn cross_invocation_read_write_and_write_write_are_rejected() {
    for first_write in [false, true] {
        let events = vec![event(0, 0, 0, first_write), event(1, 0, 0, true)];
        assert_eq!(
            versions(&events, 4096),
            Err(MemoryFailure::Interference {
                first: usize::from(!first_write),
                second: usize::from(first_write),
            })
        );
    }
    let reads = vec![event(0, 0, 0, false), event(1, 0, 0, false)];
    assert_eq!(
        versions(&reads, 4096).unwrap(),
        vec![Some(PlironSemanticMemoryVersionV1::Initial); 2]
    );
}

#[test]
fn distinct_allocations_do_not_share_versions() {
    let mut events = vec![event(0, 0, 0, true), event(1, 0, 0, false)];
    events[1].allocation = 2;
    assert_eq!(
        versions(&events, 4096).unwrap(),
        vec![None, Some(PlironSemanticMemoryVersionV1::Initial)]
    );
}

#[test]
fn exact_logical_budget_boundary_and_duplicate_event() {
    let events = vec![event(0, 0, 0, false)];
    assert_eq!(versions(&events, 13), Err(MemoryFailure::ResourceLimit));
    assert_eq!(
        versions(&events, 14).unwrap(),
        vec![Some(PlironSemanticMemoryVersionV1::Initial)]
    );
    let events = vec![event(0, 0, 0, false), event(0, 0, 0, true)];
    assert_eq!(versions(&events, 4096), Err(MemoryFailure::DuplicateEvent));
}

// Independent quadratic reference model: no sorting or per-cell state table.
fn reference(events: &[MemoryEvent]) -> Result<Vec<Option<PlironSemanticMemoryVersionV1>>, ()> {
    let mut result = Vec::new();
    for event in events {
        let cell = |other: &&MemoryEvent| {
            other.allocation == event.allocation && other.indices == event.indices
        };
        for other in events.iter().filter(cell) {
            if event.invocation != other.invocation && (event.write || other.write) {
                return Err(());
            }
        }
        let previous = events
            .iter()
            .filter(cell)
            .filter(|other| {
                other.invocation == event.invocation
                    && other.write
                    && other.sequence < event.sequence
            })
            .max_by_key(|other| other.sequence);
        result.push((!event.write).then(|| {
            previous.map_or(PlironSemanticMemoryVersionV1::Initial, |write| {
                PlironSemanticMemoryVersionV1::AfterWrite {
                    invocation: write.invocation,
                    event: write.sequence,
                    block: write.block,
                    operation: write.operation,
                }
            })
        }));
    }
    Ok(result)
}

#[test]
fn exhaustive_small_traces_match_quadratic_reference() {
    // Each event chooses owner, cell and read/write. Original sequence stays
    // unique, so a failure is specifically an interference disagreement.
    for length in 0..=5 {
        for mut bits in 0..8usize.pow(length) {
            let events = (0..length as usize)
                .map(|sequence| {
                    let choice = bits % 8;
                    bits /= 8;
                    event(
                        choice & 1,
                        sequence,
                        ((choice >> 1) & 1) as u64,
                        choice & 4 != 0,
                    )
                })
                .collect::<Vec<_>>();
            let actual = versions(&events, 4096);
            match reference(&events) {
                Ok(expected) => assert_eq!(actual.unwrap(), expected),
                Err(()) => assert!(matches!(actual, Err(MemoryFailure::Interference { .. }))),
            }
        }
    }
}
