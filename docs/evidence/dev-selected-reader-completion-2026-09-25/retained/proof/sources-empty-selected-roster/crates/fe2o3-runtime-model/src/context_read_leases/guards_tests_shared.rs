// Instantiated under each owner's private test module, without exposing mutable extraction.
fn guard_key(local: u64) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 7,
        local,
        kind: ContextWriterKindV1::Submission,
    }
}

fn read_view(owner: &Owner, destination: ContextAllocationWriteV1) -> ContextAllocationReadV1 {
    let state = owner.lookup_allocation(destination.allocation).unwrap();
    ContextAllocationReadV1 {
        allocation: destination.allocation,
        device: state.device,
        byte_extent: state.byte_extent,
        byte_offset: 0,
        byte_len: 8,
        attempt_epoch: state.attempt_epoch,
        content_lineage: state.content_lineage,
    }
}

fn guard_cases() -> Vec<(usize, usize, &'static str)> {
    let mut cases = alloc::vec![(0, 1024, "normal"), (0, 1024, "invalid_writer")];
    for count in [1, 8, 64, 512, 4096] {
        for pattern in [
            "normal",
            "invalid_first",
            "invalid_last",
            "busy_first",
            "busy_last",
            "pending_last",
            "unrelated",
            "invalid_writer",
        ] {
            cases.push((count, (2 * count).max(1024), pattern));
        }
    }
    cases.extend([(8, 65536, "normal"), (8, 65536, "unrelated")]);
    cases
}

fn guard_fixture(
    count: usize,
    capacity: usize,
    pattern: &str,
) -> (
    Owner,
    ContextWriterReferenceV1,
    Vec<ContextAllocationWriteV1>,
) {
    let mut owner = Owner::new(7, capacity, 4, capacity).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let entries: Vec<_> = (0..count + 1)
        .map(|index| ContextAllocationEnrollmentV1 {
            key: ContextAllocationKeyV1 {
                context_generation: 7,
                local: index as u64 + 1,
            },
            device,
            byte_extent: 64,
        })
        .collect();
    let mut output = alloc::vec![None; entries.len()];
    owner.enroll_allocations(&entries, &mut output).unwrap();
    let mut roster: Vec<_> = output
        .iter()
        .map(|reference| ContextAllocationWriteV1 {
            allocation: reference.unwrap(),
            device,
            byte_extent: 64,
        })
        .collect();
    let unrelated = roster.pop().unwrap();
    match pattern {
        "busy_first" => protect(&mut owner, roster[0]),
        "busy_last" => protect(&mut owner, roster[count - 1]),
        "unrelated" => protect(&mut owner, unrelated),
        "pending_last" => {
            let producer = owner.register_writer(guard_key(10)).unwrap();
            owner.begin_write(producer, &[roster[count - 1]]).unwrap();
        }
        "invalid_first" => roster[0].allocation.slot = usize::MAX,
        "invalid_last" => roster[count - 1].allocation.slot = usize::MAX,
        _ => {}
    }
    let mut writer = owner.register_writer(guard_key(50)).unwrap();
    if pattern == "invalid_writer" {
        writer.slot = usize::MAX;
    }
    (owner, writer, roster)
}

fn guard_expected(count: usize, pattern: &str, scope: &str) -> (Result<(), Error>, usize) {
    match pattern {
        "invalid_first" => (Err(Error::InvalidAllocationReference), 1),
        "invalid_last" => (Err(Error::InvalidAllocationReference), count),
        "busy_first" => (Err(Error::AllocationBusy), 1),
        "busy_last" => (Err(Error::AllocationBusy), count),
        _ if scope == "guard" => (Ok(()), count + usize::from(pattern == "pending_last")),
        "pending_last" => (
            Err(Error::AllocationBusy),
            (if OWNER == "stable" { 2 } else { 3 }) * (count + 1),
        ),
        "invalid_writer" => (
            Err(Error::InvalidReference),
            (if OWNER == "stable" { 1 } else { 2 }) * count + 1,
        ),
        _ => (
            Ok(()),
            (if OWNER == "stable" { 13 } else { 14 }) * count + 2,
        ),
    }
}

fn guard_run(
    owner: &mut Owner,
    writer: ContextWriterReferenceV1,
    roster: &[ContextAllocationWriteV1],
    scope: &str,
    candidate: bool,
) -> Result<(), Error> {
    match (scope, candidate) {
        ("guard", true) => owner.require_unread_writes(roster),
        ("guard", false) => owner
            .baseline_require_unread_v1(roster.iter().map(|destination| destination.allocation)),
        ("begin", true) => owner.begin_write(writer, roster),
        ("begin", false) => owner.baseline_begin_write_v1(writer, roster),
        _ => unreachable!(),
    }
}

struct GuardQualification {
    raw: ContextVersionJournalV1,
    before: String,
    storage: Vec<(usize, usize)>,
    expected: Result<(), Error>,
    accesses: usize,
}

fn guard_qualify(
    owner: &mut Owner,
    writer: ContextWriterReferenceV1,
    roster: &[ContextAllocationWriteV1],
    pattern: &str,
    scope: &str,
) -> GuardQualification {
    let before = snapshot(owner);
    let raw = owner.guard_copy_for_test_v1();
    let storage = owner.guard_owner_storage_v1();
    let (expected, accesses) = guard_expected(roster.len(), pattern, scope);
    let mut after = None;
    for candidate in [false, true] {
        owner.reset_access_count_for_test_v1();
        assert_eq!(guard_run(owner, writer, roster, scope, candidate), expected);
        assert_eq!(
            owner.guard_accesses_for_test_v1(),
            accesses,
            "{OWNER}/{scope}/{pattern}/{}",
            roster.len()
        );
        let observed = snapshot(owner);
        if let Some(ref baseline) = after {
            assert_eq!(&observed, baseline);
        } else {
            after = Some(observed.clone());
        }
        if expected.is_err() || scope == "guard" {
            assert_eq!(observed, before);
        }
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        owner.guard_restore_owner_v1(&raw, writer, roster);
        assert_eq!(snapshot(owner), before);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
    }
    GuardQualification {
        raw,
        before,
        storage,
        expected,
        accesses,
    }
}

#[test]
fn shared_guards_match_frozen_results_state_storage_accesses_and_reset() {
    for (count, capacity, pattern) in guard_cases() {
        for scope in ["guard", "begin"] {
            let (mut owner, writer, roster) = guard_fixture(count, capacity, pattern);
            let _ = guard_qualify(&mut owner, writer, &roster, pattern, scope);
        }
    }
}

#[test]
fn shared_guards_preserve_caller_order_and_outrank_raw_faults() {
    for protected in 0..3 {
        for invalid in 0..3 {
            let (mut owner, mut writer, mut roster) = guard_fixture(3, 4, "normal");
            protect(&mut owner, roster[protected]);
            roster[invalid].allocation.slot = usize::MAX;
            writer.slot = usize::MAX;
            roster[0].byte_extent = 0;
            let expected = if invalid <= protected {
                Error::InvalidAllocationReference
            } else {
                Error::AllocationBusy
            };
            let before = snapshot(&owner);
            let storage = owner.guard_owner_storage_v1();
            for candidate in [false, true] {
                assert_eq!(
                    guard_run(&mut owner, writer, &roster, "begin", candidate),
                    Err(expected)
                );
                assert_eq!(snapshot(&owner), before);
                assert_eq!(owner.guard_owner_storage_v1(), storage);
            }
        }
    }
}

#[test]
#[ignore = "manual release-mode matched Begin guard comparison"]
fn shared_guards_performance() {
    use std::hint::black_box;
    use std::time::Instant;
    for (count, capacity, pattern) in guard_cases() {
        for scope in ["guard", "begin"] {
            let (mut owner, writer, roster) = guard_fixture(count, capacity, pattern);
            let GuardQualification {
                raw,
                before,
                storage,
                expected,
                accesses,
            } = guard_qualify(&mut owner, writer, &roster, pattern, scope);
            let iterations = (262144 / count.max(1)).clamp(128, 16384);
            for round in 0..7 {
                for turn in 0..2 {
                    let candidate = (round + turn) % 2 == 0;
                    let mut elapsed = 0u128;
                    for _ in 0..iterations {
                        owner.guard_restore_owner_v1(&raw, writer, &roster);
                        let start = Instant::now();
                        let result = guard_run(
                            black_box(&mut owner),
                            black_box(writer),
                            black_box(&roster),
                            black_box(scope),
                            black_box(candidate),
                        );
                        let result = black_box(result);
                        elapsed += start.elapsed().as_nanos();
                        assert_eq!(result, expected);
                        assert_eq!(owner.guard_accesses_for_test_v1(), accesses);
                    }
                    owner.guard_restore_owner_v1(&raw, writer, &roster);
                    assert_eq!(snapshot(&owner), before);
                    assert_eq!(owner.guard_owner_storage_v1(), storage);
                    std::println!(
                        "begin_guards,{OWNER},{scope},{count},{capacity},{pattern},{round},{},{iterations},{elapsed},{accesses}",
                        if candidate { "shared" } else { "baseline" }
                    );
                }
            }
        }
    }
}
