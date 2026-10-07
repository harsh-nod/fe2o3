use super::*;

#[test]
#[ignore = "manual release-mode matched producer query comparison"]
fn shared_producer_query_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    let iterations = 8192;
    for capacity in [8, 65536] {
        for scenario in SCENARIOS {
            let (f, reference) = setup(capacity, scenario);
            for operation in OPERATIONS {
                qualify(&f, reference, scenario, operation);
                let before = snapshot(&f.journal);
                let storage = f.journal.guard_owner_storage_v1();
                let expected = expected(&f, scenario, operation);
                for round in 0..7 {
                    for turn in 0..2 {
                        let candidate = (round + turn) % 2 == 0;
                        let expected_accesses = accesses(scenario, operation, candidate);
                        let mut elapsed = 0u128;
                        for _ in 0..iterations {
                            f.journal.reset_access_count_for_test_v1();
                            let start = Instant::now();
                            let result = black_box(run(
                                black_box(&f),
                                black_box(reference),
                                black_box(operation),
                                black_box(candidate),
                            ));
                            elapsed += start.elapsed().as_nanos();
                            assert_eq!(result, expected);
                            assert_eq!(f.journal.guard_accesses_for_test_v1(), expected_accesses);
                        }
                        assert_eq!(snapshot(&f.journal), before);
                        assert_eq!(f.journal.guard_owner_storage_v1(), storage);
                        std::println!(
                            "producer_query,{capacity},{scenario},{operation},{round},{},{iterations},{elapsed},{expected_accesses}",
                            if candidate { "shared" } else { "baseline" }
                        );
                    }
                }
            }
        }
    }
}
