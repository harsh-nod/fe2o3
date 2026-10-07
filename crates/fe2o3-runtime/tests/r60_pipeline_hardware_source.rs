//! Source-shape checks complement the profile and hardware qualification gates.

const EXAMPLE: &str = include_str!("../examples/gfx942-runtime-r60-ordinary-pipeline.rs");

#[test]
fn each_enqueued_launch_is_explicitly_published_without_observing_completion() {
    let issue = EXAMPLE
        .split("fn issue_batch(")
        .nth(1)
        .unwrap()
        .split("fn qualify(")
        .next()
        .unwrap();
    let loop_start = issue.find("for index in 0..DEPTH {").unwrap();
    let launch = issue.find(".launch(").unwrap();
    let flush = issue.find(".flush_stream(self.stream)").unwrap();
    let loop_end = issue.rfind("}\n            Ok(())").unwrap();
    assert!(loop_start < launch && launch < flush && flush < loop_end);
    assert_eq!(issue.matches(".launch(").count(), 1);
    assert_eq!(issue.matches(".flush_stream(").count(), 1);
    for forbidden in [
        ".poll(",
        ".wait(",
        ".read_allocation(",
        ".synchronize_stream(",
    ] {
        assert!(!issue.contains(forbidden));
    }
}

#[test]
fn benchmark_times_both_enqueue_and_publication_before_tail_wait() {
    let benchmark = EXAMPLE
        .split("fn benchmark(")
        .nth(1)
        .unwrap()
        .split("fn hex(")
        .next()
        .unwrap();
    let start = benchmark.find("let started = Instant::now()").unwrap();
    let issue = benchmark
        .find("self.issue_batch(&mut submissions)")
        .unwrap();
    let issued = benchmark.find("let issued = Instant::now()").unwrap();
    let wait = benchmark
        .find("self.context.wait(tail, remaining)")
        .unwrap();
    let completed = benchmark.find("let completed = Instant::now()").unwrap();
    let readback = benchmark.find(".read_allocation(").unwrap();
    assert!(start < issue && issue < issued && issued < wait);
    assert!(wait < completed && completed < readback);
    assert!(benchmark.contains("\"issue_api_calls_per_batch\": DEPTH * 2"));
    assert!(benchmark.contains("\"explicit_allocation_api_timed\": false"));
}
