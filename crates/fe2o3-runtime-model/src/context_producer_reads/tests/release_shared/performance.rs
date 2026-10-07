use super::*;

fn cases() -> Vec<(usize, usize, bool, &'static str, &'static str)> {
    let mut result = Vec::new();
    for k in [1, 8, 64, 512, 4096] {
        for distinct in [false, true] {
            for phase in ["pending", "unknown", "success", "no_effect"] {
                result.push((k, (k + 2).max(1024), distinct, phase, "none"));
            }
        }
    }
    for distinct in [false, true] {
        for phase in ["pending", "unknown", "success", "no_effect"] {
            for fault in ["reference", "count", "evidence", "capacity"] {
                result.push((8, 1024, distinct, phase, fault));
            }
            result.push((8, 65536, distinct, phase, "none"));
        }
    }
    result
}

#[test]
fn shared_producer_release_benchmark_fixtures_match_frozen_execution_and_reset() {
    for (k, capacity, distinct, phase, fault) in cases() {
        qualify(
            &mut setup(k, capacity, distinct, phase, fault),
            phase,
            fault,
        );
    }
}

#[test]
#[ignore = "manual release-mode matched producer release comparison"]
fn shared_producer_release_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    for (k, capacity, distinct, phase, fault) in cases() {
        let mut case = setup(k, capacity, distinct, phase, fault);
        qualify(&mut case, phase, fault);
        let reset = Reset::new(&case);
        let before = snapshot(&case.owner);
        let storage = case.owner.guard_owner_storage_v1();
        let references = case.references.clone();
        let reference_storage = (case.references.as_ptr(), case.references.capacity());
        let wanted = expected(k, phase, fault);
        assert_eq!(run(&mut case, true), wanted.0);
        let after = snapshot(&case.owner);
        reset.restore(&mut case);
        let iterations = (131072 / k).clamp(64, 8192);
        for round in 0..7 {
            for turn in 0..2 {
                let candidate = (round + turn) % 2 == 0;
                let mut elapsed = 0u128;
                for _ in 0..iterations {
                    reset.restore(&mut case);
                    let start = Instant::now();
                    let result = black_box(run(black_box(&mut case), black_box(candidate)));
                    elapsed += start.elapsed().as_nanos();
                    assert_eq!(result, wanted.0);
                    assert_eq!(case.owner.guard_accesses_for_test_v1(), wanted.1);
                }
                assert_eq!(snapshot(&case.owner), after);
                assert_eq!(case.references, references);
                assert_eq!(
                    (case.references.as_ptr(), case.references.capacity()),
                    reference_storage
                );
                assert_eq!(case.owner.guard_owner_storage_v1(), storage);
                reset.restore(&mut case);
                assert_eq!(snapshot(&case.owner), before);
                assert_eq!(case.owner.guard_owner_storage_v1(), storage);
                std::println!(
                    "producer_release,{k},{capacity},{},{phase},{fault},{round},{},{iterations},{elapsed},{}",
                    if distinct { "distinct" } else { "grouped" },
                    if candidate { "shared" } else { "baseline" },
                    wanted.1
                );
            }
        }
    }
}
