use super::*;

fn cases() -> Vec<(usize, usize, bool, &'static str)> {
    let mut cases = Vec::new();
    for k in [2, 8, 64, 512, 4096] {
        for distinct in [false, true] {
            for fault in [
                "none", "device", "count", "free", "output", "epoch", "alias",
            ] {
                cases.push((k, core::cmp::max(k + 2, 1024), distinct, fault));
            }
        }
    }
    cases.extend([(8, 65536, false, "none"), (8, 65536, true, "none")]);
    cases
}

#[test]
fn shared_producer_admission_benchmark_fixtures_match_frozen_execution_and_reset() {
    for (k, capacity, distinct, fault) in cases() {
        qualify(&mut setup(k, capacity, distinct, fault), fault);
    }
}

#[test]
#[ignore = "manual release-mode matched producer admission comparison"]
fn shared_producer_acquire_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    for (k, capacity, distinct, fault) in cases() {
        let mut case = setup(k, capacity, distinct, fault);
        qualify(&mut case, fault);
        let reset = Reset::new(&case);
        let before = snapshot(&case.owner);
        let storage = case.owner.guard_owner_storage_v1();
        let output_storage = (case.output.as_ptr(), case.output.capacity());
        let wanted = expected(k, fault);
        assert_eq!(run(&mut case, true), wanted.0);
        let after = snapshot(&case.owner);
        let after_output = case.output.clone();
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
                assert_eq!(case.output, after_output);
                assert_eq!(case.owner.guard_owner_storage_v1(), storage);
                assert_eq!(
                    (case.output.as_ptr(), case.output.capacity()),
                    output_storage
                );
                reset.restore(&mut case);
                assert_eq!(snapshot(&case.owner), before);
                assert_eq!(case.output, reset.output);
                assert_eq!(case.owner.guard_owner_storage_v1(), storage);
                std::println!(
                    "producer_acquire,{k},{capacity},{},{fault},{round},{},{iterations},{elapsed},{}",
                    if distinct { "distinct" } else { "grouped" },
                    if candidate { "shared" } else { "baseline" },
                    wanted.1
                );
            }
        }
    }
}
