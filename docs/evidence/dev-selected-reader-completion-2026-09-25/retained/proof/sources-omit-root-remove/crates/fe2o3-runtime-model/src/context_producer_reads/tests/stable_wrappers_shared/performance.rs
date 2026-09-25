use super::*;

fn cases() -> Vec<(usize, usize, bool, bool, bool, &'static str)> {
    let mut result = Vec::new();
    for release in [false, true] {
        for distinct in [false, true] {
            for synchronous in [false, true] {
                for k in [1, 8, 64, 512, 4096] {
                    result.push((k, (k + 2).max(1024), distinct, synchronous, release, "none"));
                }
                result.push((8, 65536, distinct, synchronous, release, "none"));
            }
            for fault in if release {
                ["reference", "count", "evidence", "capacity"]
            } else {
                ["budget", "output", "extent", "slot"]
            } {
                result.push((8, 1024, distinct, false, release, fault));
            }
        }
    }
    result
}

#[test]
fn stable_wrapper_benchmark_fixtures_match_frozen_execution_and_reset() {
    assert_eq!(cases().len(), 64);
    for (k, capacity, distinct, synchronous, release, fault) in cases() {
        qualify(
            &mut setup(
                k,
                capacity,
                distinct,
                synchronous,
                release,
                "pending",
                fault,
            ),
            expected(k, fault),
        );
    }
}

#[test]
#[ignore = "manual release-mode matched producer stable-wrapper comparison"]
fn stable_wrapper_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    for (k, capacity, distinct, synchronous, release, fault) in cases() {
        let mut case = setup(
            k,
            capacity,
            distinct,
            synchronous,
            release,
            "pending",
            fault,
        );
        let wanted = expected(k, fault);
        qualify(&mut case, wanted);
        let reset = Reset::new(&case);
        let before = snapshot(&case.owner);
        let storage = case.owner.guard_owner_storage_v1();
        let buffers = (
            case.requests.as_ptr(),
            case.requests.capacity(),
            case.references.as_ptr(),
            case.references.capacity(),
            case.output.as_ptr(),
            case.output.capacity(),
        );
        let inputs = (case.requests.clone(), case.references.clone());
        assert_eq!(run(&mut case, true), wanted.0);
        let after = snapshot(&case.owner);
        let output_after = case.output.clone();
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
                assert_eq!(case.output, output_after);
                assert_eq!(case.requests, inputs.0);
                assert_eq!(case.references, inputs.1);
                assert_eq!(
                    (
                        case.requests.as_ptr(),
                        case.requests.capacity(),
                        case.references.as_ptr(),
                        case.references.capacity(),
                        case.output.as_ptr(),
                        case.output.capacity()
                    ),
                    buffers
                );
                assert_eq!(case.owner.guard_owner_storage_v1(), storage);
                reset.restore(&mut case);
                assert_eq!(snapshot(&case.owner), before);
                assert_eq!(case.owner.guard_owner_storage_v1(), storage);
                std::println!(
                    "producer_stable_{},{k},{capacity},{},{},{fault},{round},{},{iterations},{elapsed},{}",
                    if release { "release" } else { "acquire" },
                    if distinct { "distinct" } else { "grouped" },
                    if synchronous {
                        "synchronous"
                    } else {
                        "submission"
                    },
                    if candidate { "shared" } else { "baseline" },
                    wanted.1
                );
            }
        }
    }
}
