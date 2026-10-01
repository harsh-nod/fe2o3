//! Warm, retained-snapshot timing for the four primary authoring APIs.
//! Each run admits one actual bundle once, makes five calibration and thirty
//! measured sequential calls, and retains every timing without retries.
//! Only invocation through return is timed. Existing API-internal report-limit
//! serialization stays included; the companion's output serialization, hashing,
//! equality, output, admission and storage accounting are outside the clock.
//! No process creation, source mutation, compiler resume or GPU authority.

#![forbid(unsafe_code)]

use fe2o3_kernel_ir::{MAX_SIMULATION_BUNDLE_BYTES_V6, VerifiedSimulationBundleV6};
use fe2o3_source_isa_observation::multilevel_authoring_v1::{
    AuthoringRegionSelectorV1, AuthoringSnapshotV1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::time::Instant;

// Preserve the shipping Query payload declarations. Neither adapter is called.
#[path = "../src/bin/fe2o3_author/candidate.rs"]
mod candidate;
#[path = "../src/bin/fe2o3_author/preview.rs"]
mod preview;

const CALIBRATION: usize = 5;
const MEASURED: usize = 30;
const TOTAL: usize = CALIBRATION + MEASURED;
const TARGET_NS: u64 = 250_000_000;
const MAX_SELECTOR_BYTES: usize = 16 * 1024;
const MAX_ARGUMENTS: usize = 7;
const MAX_ARGUMENT_BYTES: usize = 64 * 1024;
const MAX_NORMAL_OUTPUT_BYTES: usize = 262_145;
const MAX_RECORD_BYTES: usize = 32_768;
const PREFIX: &[u8] = b"FE2O3_AUTHOR_WARM_ACTION_V1 ";
const USAGE: &str = "warm qualification requires one of: inspect; operations --bundle-identity HEX --start N --limit N; select --selector JSON; materialize --selector JSON --helper NAME";

#[allow(dead_code)]
enum Query {
    Inspect,
    Operations {
        identity: String,
        start: u32,
        limit: u32,
    },
    Select(AuthoringRegionSelectorV1),
    CallTarget(AuthoringRegionSelectorV1),
    Materialize(AuthoringRegionSelectorV1, String),
    MaterializeConstU32(AuthoringRegionSelectorV1, String),
    PreviewHelperInsertion {
        selector: AuthoringRegionSelectorV1,
        helper: String,
        source: String,
        expected_sha256: String,
    },
    CreateSourceCandidate(candidate::Options),
}

fn decimal(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "expected a bounded decimal integer")?;
    if parsed.to_string() != value {
        return Err("expected canonical decimal integer".into());
    }
    Ok(parsed)
}
fn selector(value: &str) -> Result<AuthoringRegionSelectorV1, String> {
    if value.len() > MAX_SELECTOR_BYTES {
        return Err("selector exceeds 16 KiB".into());
    }
    serde_json::from_str(value).map_err(|error| format!("invalid selector: {error}"))
}
fn parse(arguments: &[String]) -> Result<Query, String> {
    // Exactly the four shipping CLI forms, with no measurement-only action args.
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["inspect"] => Ok(Query::Inspect),
        [
            "operations",
            "--bundle-identity",
            identity,
            "--start",
            start,
            "--limit",
            limit,
        ] => Ok(Query::Operations {
            identity: (*identity).into(),
            start: decimal(start)?,
            limit: decimal(limit)?,
        }),
        ["select", "--selector", value] => Ok(Query::Select(selector(value)?)),
        ["materialize", "--selector", value, "--helper", helper] => {
            Ok(Query::Materialize(selector(value)?, (*helper).into()))
        }
        _ => Err(USAGE.into()),
    }
}

// A measurement-output envelope, not a constructor/authoring semantic limit.
struct BoundedBytes {
    bytes: Vec<u8>,
    cap: usize,
}
impl BoundedBytes {
    fn new(cap: usize) -> Self {
        Self {
            bytes: Vec::new(),
            cap,
        }
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("measurement output length overflow"))?;
        if end > self.cap {
            return Err(io::Error::other("measurement output envelope exceeded"));
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| io::Error::other("measurement output allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn normal_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut output = BoundedBytes::new(MAX_NORMAL_OUTPUT_BYTES);
    serde_json::to_writer(&mut output, value).map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    Ok(output.bytes)
}
fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in hash {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    text
}

#[derive(Debug, Serialize)]
struct Sample {
    ordinal: usize,
    calibration: bool,
    elapsed_ns: u64,
    normal_output_bytes: usize,
    normal_output_sha256: String,
    exact_equal_to_first: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
struct Statistics {
    p50_ns: u64,
    p95_ns: u64,
    max_ns: u64,
}
fn measured_statistics(samples: &[Sample]) -> Result<Statistics, String> {
    if samples.len() != TOTAL {
        return Err("warm action requires exactly 35 raw samples".into());
    }
    let mut measured = [0_u64; MEASURED];
    for (index, sample) in samples.iter().enumerate() {
        if sample.ordinal != index + 1 || sample.calibration != (index < CALIBRATION) {
            return Err("warm action sample order/calibration mismatch".into());
        }
        if index >= CALIBRATION {
            measured[index - CALIBRATION] = sample.elapsed_ns;
        }
    }
    measured.sort_unstable();
    // Nearest rank: ceil(30 * 0.50) = 15; ceil(30 * 0.95) = 29.
    Ok(Statistics {
        p50_ns: measured[14],
        p95_ns: measured[28],
        max_ns: measured[29],
    })
}
#[derive(Debug, Serialize)]
struct Record {
    schema: &'static str,
    action: &'static str,
    scope: &'static str,
    complete: bool,
    calibration_count: usize,
    measured_count: usize,
    raw_samples: Vec<Sample>,
    measured: Statistics,
    percentile_method: &'static str,
    target_ns: u64,
    p95_within_target: bool,
    all_exact_equal_to_first: bool,
    independent_cli_comparison: &'static str,
    snapshot_admissions: usize,
    previous_result_dropped_before_next_call: bool,
    companion_serialization_digest_equality_in_timer: bool,
    api_internal_report_limit_serialization: &'static str,
    storage_walk_in_timer: bool,
    end_to_end_measured: bool,
    storage_measured: bool,
    peak_heap_measured: bool,
    rss_measured: bool,
    grants_authority: bool,
}

#[derive(Debug, Serialize)]
struct Failure {
    schema: &'static str,
    action: &'static str,
    scope: &'static str,
    complete: bool,
    expected_calibration_count: usize,
    expected_measured_count: usize,
    raw_samples: Vec<Sample>,
    failed_ordinal: Option<usize>,
    current_elapsed_ns_decimal: Option<String>,
    stage: &'static str,
    diagnostic: String,
    measured: Option<Statistics>,
    p95_within_target: Option<bool>,
    grants_authority: bool,
}
fn failure(
    action: &'static str,
    samples: Vec<Sample>,
    ordinal: Option<usize>,
    elapsed_ns: Option<u128>,
    stage: &'static str,
    mut diagnostic: String,
) -> Failure {
    // Bound the observer diagnostic, not the original API or its admission.
    if diagnostic.len() > 512 {
        let mut end = 512;
        while !diagnostic.is_char_boundary(end) {
            end -= 1;
        }
        diagnostic.truncate(end);
    }
    Failure {
        schema: "fe2o3-author-warm-action-v1",
        action,
        scope: "one_retained_snapshot_original_api_invocation_to_return",
        complete: false,
        expected_calibration_count: CALIBRATION,
        expected_measured_count: MEASURED,
        raw_samples: samples,
        failed_ordinal: ordinal,
        current_elapsed_ns_decimal: elapsed_ns.map(|value| value.to_string()),
        stage,
        diagnostic,
        measured: None,
        p95_within_target: None,
        grants_authority: false,
    }
}
fn collect<T: Serialize, R>(
    action: &'static str,
    mut invoke: impl FnMut() -> R,
    returned: impl Fn(R) -> Result<T, String>,
) -> Result<(Vec<u8>, Record), Failure> {
    let mut samples = Vec::with_capacity(TOTAL);
    let mut first = None::<Vec<u8>>;
    for index in 0..TOTAL {
        // Keep the API's original return type within the timer, including its
        // own bounded_report serialization. Companion error conversion, output
        // serialization, hash and equality are after this exact return endpoint.
        let started = Instant::now();
        let result = invoke();
        let elapsed = started.elapsed();
        let elapsed_raw = elapsed.as_nanos();
        let elapsed_ns = match u64::try_from(elapsed_raw) {
            Ok(value) => value,
            Err(_) => {
                return Err(failure(
                    action,
                    samples,
                    Some(index + 1),
                    Some(elapsed_raw),
                    "clock_conversion",
                    "warm action clock overflow".into(),
                ));
            }
        };
        let result = match returned(result) {
            Ok(value) => value,
            Err(error) => {
                return Err(failure(
                    action,
                    samples,
                    Some(index + 1),
                    Some(elapsed_raw),
                    "api_refusal",
                    error,
                ));
            }
        };
        let bytes = normal_bytes(&result);
        drop(result); // No original result survives into the next call.
        let bytes = match bytes {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(failure(
                    action,
                    samples,
                    Some(index + 1),
                    Some(elapsed_raw),
                    "companion_output_serialization",
                    error,
                ));
            }
        };
        let equal = first.as_ref().is_none_or(|first| first == &bytes);
        let sample = Sample {
            ordinal: index + 1,
            calibration: index < CALIBRATION,
            elapsed_ns,
            normal_output_bytes: bytes.len(),
            normal_output_sha256: digest(&bytes),
            exact_equal_to_first: equal,
        };
        samples.push(sample);
        if !equal {
            // Preserve the mismatching row too. current_elapsed repeats that
            // row explicitly; it is not an additional API call/sample.
            return Err(failure(
                action,
                samples,
                Some(index + 1),
                Some(elapsed_raw),
                "normal_output_mismatch",
                format!("normal output differs at call {}", index + 1),
            ));
        }
        if first.is_none() {
            first = Some(bytes);
        }
        // Subsequent serialization bytes drop here. First normal bytes and raw
        // telemetry persist between calls; no storage/peak claim is made.
    }
    let measured = match measured_statistics(&samples) {
        Ok(value) => value,
        Err(error) => return Err(failure(action, samples, None, None, "statistics", error)),
    };
    let first = match first {
        Some(value) => value,
        None => {
            return Err(failure(
                action,
                samples,
                None,
                None,
                "normal_output_missing",
                "warm action produced no output".into(),
            ));
        }
    };
    let record = Record {
        schema: "fe2o3-author-warm-action-v1",
        action,
        scope: "one_retained_snapshot_original_api_invocation_to_return",
        complete: true,
        calibration_count: CALIBRATION,
        measured_count: MEASURED,
        raw_samples: samples,
        measured,
        percentile_method: "nearest_rank_measured_30_only_p50_rank15_p95_rank29",
        target_ns: TARGET_NS,
        p95_within_target: measured.p95_ns <= TARGET_NS,
        all_exact_equal_to_first: true,
        independent_cli_comparison: "required_by_parent_raw_stdout_comparison_not_performed_here",
        snapshot_admissions: 1,
        previous_result_dropped_before_next_call: true,
        companion_serialization_digest_equality_in_timer: false,
        api_internal_report_limit_serialization: "included_when_performed_by_original_api",
        storage_walk_in_timer: false,
        end_to_end_measured: false,
        storage_measured: false,
        peak_heap_measured: false,
        rss_measured: false,
        grants_authority: false,
    };
    Ok((first, record))
}
fn emit_record(record: &impl Serialize) -> Result<(), String> {
    let mut telemetry = BoundedBytes::new(MAX_RECORD_BYTES);
    telemetry
        .write_all(PREFIX)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut telemetry, record).map_err(|error| error.to_string())?;
    telemetry
        .write_all(b"\n")
        .map_err(|error| error.to_string())?;
    io::stderr()
        .lock()
        .write_all(&telemetry.bytes)
        .map_err(|error| error.to_string())
}
fn emit(normal: &[u8], record: &Record) -> Result<(), String> {
    // Preserve all35 rows even if later stdout fails. complete=true means the
    // API loop completed, not that independent stdout equality or process exit
    // already passed. Parent must require both plus this complete record.
    emit_record(record)?;
    io::stdout()
        .lock()
        .write_all(normal)
        .map_err(|error| error.to_string())
}
fn run(query: &Query) -> Result<(), String> {
    if !matches!(
        query,
        Query::Inspect | Query::Operations { .. } | Query::Select(_) | Query::Materialize(..)
    ) {
        return Err(USAGE.into());
    }
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(MAX_SIMULATION_BUNDLE_BYTES_V6 as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read bundle: {error}"))?;
    if bytes.len() > MAX_SIMULATION_BUNDLE_BYTES_V6 {
        return Err("bundle exceeds the canonical V6 byte limit".into());
    }
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(bytes)
        .map_err(|error| format!("bundle rejected: {error}"))?;
    let snapshot =
        AuthoringSnapshotV1::from_bundle_v6(bundle).map_err(|error| error.to_string())?;
    let measured = match query {
        Query::Inspect => collect("inspect", || snapshot.summary(), Ok),
        Query::Operations {
            identity,
            start,
            limit,
        } => collect(
            "operations",
            || snapshot.operation_page(identity, *start, *limit),
            |value| value.map_err(|error| error.to_string()),
        ),
        Query::Select(selector) => collect(
            "select",
            || snapshot.select_region(selector),
            |value| value.map_err(|error| error.to_string()),
        ),
        Query::Materialize(selector, helper) => collect(
            "materialize",
            || snapshot.materialize_typed_rust(selector, helper),
            |value| value.map_err(|error| error.to_string()),
        ),
        Query::CallTarget(_)
        | Query::MaterializeConstU32(..)
        | Query::PreviewHelperInsertion { .. }
        | Query::CreateSourceCandidate(_) => return Err(USAGE.into()),
    };
    match measured {
        Ok((normal, record)) => emit(&normal, &record),
        Err(failure) => {
            emit_record(&failure)?;
            Err("warm action qualification failed; incomplete raw samples retained".into())
        }
    }
}
fn utf8_arguments(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut total = 0_usize;
    for argument in arguments {
        if result.len() >= MAX_ARGUMENTS {
            return Err(USAGE.into());
        }
        let argument = argument
            .into_string()
            .map_err(|_| "arguments must be valid UTF-8")?;
        total = total
            .checked_add(argument.len())
            .ok_or("argument length overflow")?;
        if total > MAX_ARGUMENT_BYTES {
            return Err("qualification arguments exceed 64 KiB".into());
        }
        result.push(argument);
    }
    Ok(result)
}
fn main() -> ExitCode {
    let outcome = utf8_arguments(std::env::args_os().skip(1))
        .and_then(|arguments| parse(&arguments))
        .and_then(|query| run(&query));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Incomplete/missing telemetry is refusal, never zero-duration or
            // a budget pass. No automatic retry or replacement follows.
            eprintln!("fe2o3-author: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_source_isa_observation::multilevel_authoring_v1::AuthoringOperationCoordinateV1;
    use std::cell::Cell;
    use std::rc::Rc;

    fn samples() -> Vec<Sample> {
        (0..TOTAL)
            .map(|index| Sample {
                ordinal: index + 1,
                calibration: index < CALIBRATION,
                elapsed_ns: if index < CALIBRATION {
                    u64::MAX
                } else {
                    (index - CALIBRATION + 1) as u64
                },
                normal_output_bytes: 2,
                normal_output_sha256: String::new(),
                exact_equal_to_first: true,
            })
            .collect()
    }
    #[test]
    fn nearest_rank_excludes_exactly_first_five_and_keeps_all_thirty() {
        let samples = samples();
        assert_eq!(
            measured_statistics(&samples).unwrap(),
            Statistics {
                p50_ns: 15,
                p95_ns: 29,
                max_ns: 30
            }
        );
        assert!(measured_statistics(&samples[..TOTAL - 1]).is_err());
        let mut bad = samples;
        bad[5].calibration = true;
        assert!(measured_statistics(&bad).is_err());
        bad[5].calibration = false;
        bad[6].ordinal = 99;
        assert!(measured_statistics(&bad).is_err());
    }
    #[test]
    fn threshold_is_inclusive_p95_comparison_not_max_or_admission() {
        let mut samples = samples();
        for sample in &mut samples[CALIBRATION..] {
            sample.elapsed_ns = TARGET_NS;
        }
        samples[TOTAL - 1].elapsed_ns = TARGET_NS + 1;
        let stats = measured_statistics(&samples).unwrap();
        assert_eq!(stats.p95_ns, TARGET_NS);
        assert!(stats.p95_ns <= TARGET_NS);
        assert!(stats.max_ns > TARGET_NS);
        samples[TOTAL - 2].elapsed_ns = TARGET_NS + 1;
        assert!(measured_statistics(&samples).unwrap().p95_ns > TARGET_NS);
    }
    struct Guard(Rc<Cell<bool>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }
    #[derive(Serialize)]
    struct Value {
        value: u32,
        #[serde(skip)]
        _guard: Guard,
    }
    #[test]
    fn all_35_outputs_are_independently_serialized_and_results_drop_before_next() {
        let live = Rc::new(Cell::new(false));
        let mut calls = 0;
        let (bytes, record) = collect(
            "test",
            || {
                assert!(!live.get());
                live.set(true);
                calls += 1;
                Value {
                    value: 7,
                    _guard: Guard(Rc::clone(&live)),
                }
            },
            Ok,
        )
        .unwrap();
        assert_eq!(calls, TOTAL);
        assert!(!live.get());
        assert_eq!(bytes, b"{\"value\":7}\n");
        assert_eq!(record.raw_samples.len(), TOTAL);
        for (index, sample) in record.raw_samples.iter().enumerate() {
            assert_eq!(sample.ordinal, index + 1);
            assert_eq!(sample.calibration, index < CALIBRATION);
            assert!(sample.exact_equal_to_first);
            assert_eq!(sample.normal_output_bytes, bytes.len());
            assert_eq!(sample.normal_output_sha256, digest(&bytes));
        }
        assert!(!record.end_to_end_measured && !record.storage_measured);
    }
    #[test]
    fn mismatched_output_and_action_refusal_produce_no_completed_campaign() {
        let mut calls = 0_u32;
        let outcome = collect(
            "test",
            || {
                calls += 1;
                calls
            },
            Ok,
        );
        let failure = outcome.unwrap_err();
        assert!(!failure.complete);
        assert_eq!(failure.raw_samples.len(), 2);
        assert_eq!(failure.failed_ordinal, Some(2));
        assert!(failure.current_elapsed_ns_decimal.is_some());
        assert!(!failure.raw_samples[1].exact_equal_to_first);
        assert!(failure.measured.is_none() && failure.p95_within_target.is_none());
        assert_eq!(calls, 2);
        let mut calls = 0;
        let outcome = collect::<u32, _>(
            "test",
            || {
                calls += 1;
                Err::<u32, _>("refused")
            },
            |value| value.map_err(str::to_owned),
        );
        let failure = outcome.unwrap_err();
        assert_eq!(failure.stage, "api_refusal");
        assert!(failure.raw_samples.is_empty());
        assert_eq!(failure.failed_ordinal, Some(1));
        assert!(failure.current_elapsed_ns_decimal.is_some());
        assert_eq!(calls, 1);
    }
    #[test]
    fn serialization_refusal_keeps_completed_prefix_and_current_timing() {
        let mut calls = 0;
        let outcome = collect(
            "test",
            || {
                calls += 1;
                if calls == 3 {
                    "x".repeat(MAX_NORMAL_OUTPUT_BYTES)
                } else {
                    "ok".to_owned()
                }
            },
            Ok,
        );
        let failure = outcome.unwrap_err();
        assert_eq!(calls, 3);
        assert_eq!(failure.stage, "companion_output_serialization");
        assert_eq!(failure.raw_samples.len(), 2);
        assert_eq!(failure.failed_ordinal, Some(3));
        assert!(failure.current_elapsed_ns_decimal.is_some());
        assert!(failure.measured.is_none() && failure.p95_within_target.is_none());
    }
    #[test]
    fn incomplete_receipt_keeps_wide_clock_and_bounded_utf8_diagnostic() {
        let value = failure(
            "test",
            samples(),
            Some(35),
            Some(u128::MAX),
            "clock_conversion",
            "é".repeat(3000),
        );
        assert!(!value.complete);
        assert_eq!(
            value.current_elapsed_ns_decimal,
            Some(u128::MAX.to_string())
        );
        assert_eq!(value.diagnostic.len(), 512);
        let mut bytes = BoundedBytes::new(MAX_RECORD_BYTES);
        bytes.write_all(PREFIX).unwrap();
        serde_json::to_writer(&mut bytes, &value).unwrap();
        bytes.write_all(b"\n").unwrap();
        assert!(bytes.bytes.len() <= MAX_RECORD_BYTES);
    }
    #[test]
    fn bounded_output_counts_newline_and_does_not_silently_truncate() {
        assert_eq!(normal_bytes(&7_u32).unwrap(), b"7\n");
        let mut output = BoundedBytes::new(3);
        output.write_all(b"ab").unwrap();
        output.write_all(b"\n").unwrap();
        assert!(output.write_all(b"x").is_err());
        assert_eq!(output.bytes, b"ab\n");
        assert!(normal_bytes(&"x".repeat(MAX_NORMAL_OUTPUT_BYTES)).is_err());
        assert_eq!(
            digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
    #[test]
    fn only_four_primary_shipping_forms_are_admitted() {
        let selector = AuthoringRegionSelectorV1 {
            bundle_identity: "11".repeat(32),
            canonical_kir_digest: "22".repeat(32),
            target: "gfx942:xnack-".into(),
            operations: vec![AuthoringOperationCoordinateV1 {
                function: 1,
                block: 0,
                operation: 0,
            }],
        };
        let encoded = serde_json::to_string(&selector).unwrap();
        for values in [
            vec!["inspect"],
            vec![
                "operations",
                "--bundle-identity",
                "11",
                "--start",
                "0",
                "--limit",
                "1",
            ],
            vec!["select", "--selector", &encoded],
            vec![
                "materialize",
                "--selector",
                &encoded,
                "--helper",
                "candidate",
            ],
        ] {
            assert!(parse(&values.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_ok());
        }
        for values in [
            vec![],
            vec!["--help"],
            vec!["call-target"],
            vec!["materialize-const-u32"],
            vec!["preview-helper-insertion"],
            vec!["create-source-candidate"],
            vec!["inspect", "--resume"],
            vec!["select", "--selector", "{}"],
            vec![
                "operations",
                "--bundle-identity",
                "11",
                "--start",
                "01",
                "--limit",
                "1",
            ],
        ] {
            assert!(parse(&values.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(super::selector(&" ".repeat(MAX_SELECTOR_BYTES + 1)).is_err());
    }
    #[test]
    fn arguments_have_a_separate_finite_measurement_envelope() {
        assert!(
            utf8_arguments(std::iter::repeat_n(
                std::ffi::OsString::from("x"),
                MAX_ARGUMENTS + 1
            ))
            .is_err()
        );
        assert!(
            utf8_arguments([std::ffi::OsString::from("x".repeat(MAX_ARGUMENT_BYTES + 1))]).is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn native_non_utf8_arguments_refuse_without_panicking() {
        use std::os::unix::ffi::OsStringExt;
        assert!(utf8_arguments([std::ffi::OsString::from_vec(vec![0xff])]).is_err());
    }
}
