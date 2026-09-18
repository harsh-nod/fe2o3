use std::collections::{BTreeMap, VecDeque};
use std::io;

use super::*;

fn arguments(mode: &str) -> Vec<String> {
    ["0x54f88318ca05093d", "268435456", "3", "10", mode]
        .map(String::from)
        .to_vec()
}

#[test]
fn exact_diagnostic_modes_do_not_change_legacy_admission() {
    for (mode, policy) in [
        ("diagnostic-slice50us", WaitPolicyV1::Slice50us),
        ("diagnostic-window-deadline", WaitPolicyV1::WindowDeadline),
    ] {
        let args = arguments(mode);
        let config = parse_config_v1(&args).unwrap().unwrap();
        assert_eq!(config.policy, policy);
        assert_eq!(config.rounds, 13);
        assert_eq!(config.bytes, MAX_COPY_BYTES);
        assert_eq!(config.unique_id, 0x54f88318ca05093d);
        assert_eq!(parse_config_v1(&args[..4]).unwrap(), None);
    }
    assert_eq!(
        parse_config_v1(&["invalid".into(), "".into(), "-1".into(), "0".into()]).unwrap(),
        None,
        "legacy parsing and error order remain owned by the original main path"
    );
}

#[test]
fn malformed_diagnostic_controls_reject_before_execution() {
    let original = arguments("diagnostic-slice50us");
    for length in 0..4 {
        assert_eq!(
            parse_config_v1(&original[..length])
                .unwrap_err()
                .to_string(),
            USAGE
        );
    }
    let mut extra = original.clone();
    extra.push("extra".into());
    assert!(parse_config_v1(&extra).is_err());
    for mode in [
        "",
        "slice50us",
        "window-deadline",
        "diagnostic-poll",
        "diagnostic-SLICE50US",
    ] {
        assert_eq!(
            parse_config_v1(&arguments(mode)).unwrap_err().to_string(),
            DIAGNOSTIC_USAGE
        );
    }
    for (index, value) in [
        (0, "0".to_owned()),
        (0, "-1".to_owned()),
        (1, "0".to_owned()),
        (1, (MAX_COPY_BYTES + 1).to_string()),
        (2, "-1".to_owned()),
        (2, usize::MAX.to_string()),
        (3, "0".to_owned()),
        (3, MAX_ROUNDS.to_string()),
    ] {
        let mut args = original.clone();
        args[index] = value;
        assert!(parse_config_v1(&args).is_err());
    }
    let mut bounded = original;
    bounded[2] = "0".into();
    bounded[3] = MAX_ROUNDS.to_string();
    assert_eq!(
        parse_config_v1(&bounded).unwrap().unwrap().rounds,
        MAX_ROUNDS
    );
}

enum Step {
    Flush(Result<(), &'static str>),
    Wait(Result<RuntimePollV1, &'static str>),
}

struct Driver {
    steps: VecDeque<Step>,
    waits: Vec<Duration>,
    flushes: usize,
}

impl Driver {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            waits: Vec::new(),
            flushes: 0,
        }
    }
}

impl CopyProgressDriverV1 for Driver {
    fn wait(&mut self, timeout: Duration) -> BenchmarkResult<RuntimePollV1> {
        self.waits.push(timeout);
        let Some(Step::Wait(result)) = self.steps.pop_front() else {
            panic!("unexpected wait");
        };
        result.map_err(Into::into)
    }

    fn flush(&mut self) -> BenchmarkResult<()> {
        self.flushes += 1;
        let Some(Step::Flush(result)) = self.steps.pop_front() else {
            panic!("unexpected flush");
        };
        result.map_err(Into::into)
    }
}

#[test]
fn window_frontier_flushes_and_wait_budgets_follow_the_selected_policy() {
    for (policy, first_budget) in [
        (WaitPolicyV1::Slice50us, Duration::from_nanos(50_000)),
        (WaitPolicyV1::WindowDeadline, Duration::from_secs(1)),
    ] {
        let mut driver = Driver::new([
            Step::Flush(Ok(())),
            Step::Wait(Ok(RuntimePollV1::Pending)),
            Step::Flush(Ok(())),
            Step::Wait(Ok(RuntimePollV1::Succeeded)),
        ]);
        let mut remaining = [Duration::from_secs(1), Duration::from_nanos(17)].into_iter();
        let counts = drive_v1(&mut driver, policy, || remaining.next().unwrap()).unwrap();
        assert_eq!(
            counts,
            ProgressCountsV1 {
                waits: 2,
                flushes: 2
            }
        );
        assert_eq!(driver.flushes, 2);
        assert_eq!(driver.waits, [first_budget, Duration::from_nanos(17)]);
        assert!(remaining.next().is_none());
        assert!(driver.steps.is_empty());
    }
}

#[test]
fn every_pending_observation_keeps_an_explicit_flush() {
    for policy in [WaitPolicyV1::Slice50us, WaitPolicyV1::WindowDeadline] {
        for pending in 0..4 {
            let mut steps = vec![Step::Flush(Ok(()))];
            for _ in 0..pending {
                steps.extend([Step::Wait(Ok(RuntimePollV1::Pending)), Step::Flush(Ok(()))]);
            }
            steps.push(Step::Wait(Ok(RuntimePollV1::Succeeded)));
            let mut driver = Driver::new(steps);
            let counts = drive_v1(&mut driver, policy, || Duration::from_secs(1)).unwrap();
            assert_eq!(counts.waits, pending + 1);
            assert_eq!(counts.flushes, pending + 1);
            assert!(driver.steps.is_empty());
        }
    }
}

#[test]
fn zero_remaining_times_out_after_initial_flush_without_waiting() {
    for policy in [WaitPolicyV1::Slice50us, WaitPolicyV1::WindowDeadline] {
        let mut driver = Driver::new([Step::Flush(Ok(()))]);
        assert_eq!(
            drive_v1(&mut driver, policy, || Duration::ZERO)
                .unwrap_err()
                .to_string(),
            "directional SDMA copy timed out"
        );
        assert_eq!(driver.flushes, 1);
        assert!(driver.waits.is_empty());
        assert!(driver.steps.is_empty());
    }
}

#[test]
fn deadline_exhaustion_after_pending_does_not_start_another_wait() {
    let mut driver = Driver::new([
        Step::Flush(Ok(())),
        Step::Wait(Ok(RuntimePollV1::Pending)),
        Step::Flush(Ok(())),
    ]);
    let mut times = [Duration::from_nanos(1), Duration::ZERO].into_iter();
    assert!(
        drive_v1(&mut driver, WaitPolicyV1::WindowDeadline, || times
            .next()
            .unwrap())
        .is_err()
    );
    assert_eq!(driver.waits, [Duration::from_nanos(1)]);
    assert_eq!(driver.flushes, 2);
    assert!(driver.steps.is_empty());
}

#[test]
fn progress_errors_stop_without_later_publication_or_observation() {
    for policy in [WaitPolicyV1::Slice50us, WaitPolicyV1::WindowDeadline] {
        let cases = [
            (
                vec![Step::Flush(Err("initial flush"))],
                "initial flush",
                0,
                1,
            ),
            (
                vec![Step::Flush(Ok(())), Step::Wait(Err("wait error"))],
                "wait error",
                1,
                1,
            ),
            (
                vec![
                    Step::Flush(Ok(())),
                    Step::Wait(Ok(RuntimePollV1::Failed { code: 17 })),
                ],
                "directional SDMA copy failed with code 17",
                1,
                1,
            ),
            (
                vec![
                    Step::Flush(Ok(())),
                    Step::Wait(Ok(RuntimePollV1::Pending)),
                    Step::Flush(Err("continuation flush")),
                ],
                "continuation flush",
                1,
                2,
            ),
        ];
        for (steps, expected, waits, flushes) in cases {
            let mut driver = Driver::new(steps);
            assert_eq!(
                drive_v1(&mut driver, policy, || Duration::from_secs(1))
                    .unwrap_err()
                    .to_string(),
                expected
            );
            assert_eq!(driver.waits.len(), waits);
            assert_eq!(driver.flushes, flushes);
            assert!(driver.steps.is_empty());
        }
    }
}

#[test]
fn counters_fail_closed_instead_of_wrapping_or_saturating() {
    let mut count = u64::MAX - 1;
    increment(&mut count).unwrap();
    assert_eq!(count, u64::MAX);
    assert_eq!(
        increment(&mut count).unwrap_err().to_string(),
        "diagnostic call count overflow"
    );
    assert_eq!(count, u64::MAX);
}

#[test]
fn shared_timestamps_preserve_exact_decomposition_and_reject_invalid_results() {
    let start = Instant::now();
    let counts = ProgressCountsV1 {
        waits: 2,
        flushes: 2,
    };
    let timing = CopyTimingV1::from_instants(
        start,
        start + Duration::from_nanos(3),
        start + Duration::from_nanos(10),
        counts,
    )
    .unwrap();
    assert_eq!(
        timing,
        CopyTimingV1 {
            submit_ns: 3,
            progress_ns: 7,
            total_ns: 10,
            counts
        }
    );
    let zero_submit =
        CopyTimingV1::from_instants(start, start, start + Duration::from_nanos(1), counts).unwrap();
    assert_eq!(zero_submit.submit_ns, 0);
    assert_eq!(zero_submit.total_ns, 1);
    assert!(CopyTimingV1::from_instants(start, start, start, counts).is_err());
    assert!(
        CopyTimingV1::from_instants(
            start + Duration::from_nanos(1),
            start,
            start + Duration::from_nanos(2),
            counts
        )
        .is_err()
    );
    assert!(
        CopyTimingV1::from_instants(
            start,
            start + Duration::from_nanos(2),
            start + Duration::from_nanos(1),
            counts
        )
        .is_err()
    );
    for invalid in [
        ProgressCountsV1::default(),
        ProgressCountsV1 {
            waits: 2,
            flushes: 1,
        },
    ] {
        assert!(
            CopyTimingV1::from_instants(start, start, start + Duration::from_nanos(1), invalid)
                .is_err()
        );
    }
}

fn completed_fixture() -> CompletedRunV1 {
    let mut args = arguments("diagnostic-window-deadline");
    args[2] = "1".into();
    args[3] = "2".into();
    let config = parse_config_v1(&args).unwrap().unwrap();
    let start = Instant::now();
    let timing = |submit, progress, calls| {
        CopyTimingV1::from_instants(
            start,
            start + Duration::from_nanos(submit),
            start + Duration::from_nanos(submit + progress),
            ProgressCountsV1 {
                waits: calls,
                flushes: calls,
            },
        )
        .unwrap()
    };
    CompletedRunV1 {
        config,
        rounds: (0..3)
            .map(|index| RoundV1 {
                h2d: timing(index + 3, index + 7, index + 1),
                d2h: timing(index + 11, index + 17, index + 4),
            })
            .collect(),
    }
}

fn fields(line: &str) -> BTreeMap<&str, &str> {
    let mut result = BTreeMap::new();
    for field in line.split_whitespace() {
        let (key, value) = field.split_once('=').unwrap();
        assert!(result.insert(key, value).is_none());
    }
    result
}

#[test]
fn completed_output_has_a_distinct_schema_and_exact_ordered_rounds() {
    let completed = completed_fixture();
    let mut output = Vec::new();
    completed.write(&mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("fe2o3.async-copy-benchmark.v1"));
    let rows = text.lines().map(fields).collect::<Vec<_>>();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|row| row["schema"] == SCHEMA));
    assert_eq!(rows[0]["record"], "config");
    assert_eq!(rows[0]["wait_policy"], "window-deadline");
    assert_eq!(rows[0]["wait_slice_ns"], "0");
    assert_eq!(rows[0]["packets_per_transfer"], "65");
    assert_eq!(rows[0]["windows_per_transfer"], "2");
    for (index, row) in rows[1..4].iter().enumerate() {
        assert_eq!(row["record"], "round");
        assert_eq!(row["index"], index.to_string());
        assert_eq!(row["phase"], if index == 0 { "warmup" } else { "sample" });
        assert_eq!(row["pattern"], pattern(index).to_string());
        assert_eq!(row["checked_bytes"], "268435456");
        for (direction, submit, progress, calls) in [
            ("h2d", index + 3, index + 7, index + 1),
            ("d2h", index + 11, index + 17, index + 4),
        ] {
            assert_eq!(
                row[format!("{direction}_submit_ns").as_str()],
                submit.to_string()
            );
            assert_eq!(
                row[format!("{direction}_progress_ns").as_str()],
                progress.to_string()
            );
            assert_eq!(
                row[format!("{direction}_total_ns").as_str()],
                (submit + progress).to_string()
            );
            assert_eq!(
                row[format!("{direction}_wait_calls").as_str()],
                calls.to_string()
            );
            assert_eq!(
                row[format!("{direction}_flush_calls").as_str()],
                calls.to_string()
            );
        }
    }
    assert_eq!(rows[4]["record"], "complete");
    assert_eq!(rows[4]["validated_rounds"], "3");
    assert_eq!(rows[4]["measured_rounds"], "2");
    assert_eq!(rows[4]["validation"], "full-returned-buffer-every-round");
    assert_eq!(rows[4]["teardown"], "explicit-complete");
    let mut incomplete = completed;
    incomplete.rounds.pop();
    let mut untouched = Vec::new();
    assert!(incomplete.write(&mut untouched).is_err());
    assert!(untouched.is_empty());
}

#[test]
fn sliced_diagnostic_configuration_reports_the_exact_budget() {
    let mut completed = completed_fixture();
    completed.config.policy = WaitPolicyV1::Slice50us;
    let mut output = Vec::new();
    completed.write(&mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    let config = fields(text.lines().next().unwrap());
    assert_eq!(config["wait_policy"], "slice50us");
    assert_eq!(config["wait_slice_ns"], "50000");
    assert_eq!(config["outer_timeout_ns"], "60000000000");
}

struct LimitedWriter {
    remaining: usize,
    fail_flush: bool,
    bytes: Vec<u8>,
}

impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.remaining.min(bytes.len());
        if count == 0 {
            return Err(io::Error::other("output rejected"));
        }
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::other("flush rejected"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn early_late_and_flush_output_failures_are_errors() {
    let completed = completed_fixture();
    let mut complete = Vec::new();
    completed.write(&mut complete).unwrap();
    for remaining in [0, 100, complete.len() - 1] {
        let mut writer = LimitedWriter {
            remaining,
            fail_flush: false,
            bytes: Vec::new(),
        };
        assert!(completed.write(&mut writer).is_err());
        assert_eq!(writer.bytes, complete[..remaining]);
    }
    let mut writer = LimitedWriter {
        remaining: complete.len(),
        fail_flush: true,
        bytes: Vec::new(),
    };
    assert_eq!(
        completed.write(&mut writer).unwrap_err().to_string(),
        "flush rejected"
    );
    assert_eq!(
        writer.bytes, complete,
        "even complete stdout requires a zero process exit"
    );
}
