//! Test-only repeated validation of fresh authenticated source transactions.
//! Calibration never measures CTFE generation or claims complete owner storage.
use super::{
    Callbacks, Compilation, Compiler, Failure, Input, MAX_CONFIG, Observation, Profile,
    RetainedBytes, TyCtxt, digest, observe_live_owner, request_ok, timed,
    transaction_in_active_session_v1,
};
use serde::Serialize;
use std::{
    io::{self, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

const INPUT_ENV: &str = "FE2O3_ORDERED_VALIDATION_SERIES_V1_CONFIG";
const SAMPLE_PREFIX: &str = "fe2o3 ordered-validation sample v1: ";
const PREFIX: &str = "fe2o3 ordered-validation series v1: ";
const CALIBRATION: usize = 5;
const MEASURED: usize = 30;
const TOTAL: usize = CALIBRATION + MEASURED;
const STOP_AFTER: Duration = Duration::from_secs(60);
const SAMPLE_CAP: usize = 2048;
const REPORT_CAP: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SeriesFailure {
    MissingCallback,
    RepeatedCallback,
    CompilerFatal,
    SourceCollection,
    Validation,
    Observation,
    IdentityDrift,
    TimeBudget,
    Publication,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Identity {
    canonical_identity: String,
    canonical_bytes: usize,
    semantic_identity: String,
    source_inventory_identity: String,
    source_preflight_identity: String,
    canonical_executable_receipt_bytes: usize,
    call_correspondence_receipt_bytes: usize,
    retained_receipts_sum_bytes: usize,
}
impl Identity {
    fn observed(value: &Observation) -> Self {
        Self {
            canonical_identity: super::super::lower_hex_v1(&value.canonical_identity),
            canonical_bytes: value.canonical_bytes,
            semantic_identity: super::super::lower_hex_v1(&value.semantic_identity),
            source_inventory_identity: super::super::lower_hex_v1(&value.source_inventory_identity),
            source_preflight_identity: super::super::lower_hex_v1(&value.source_preflight_identity),
            canonical_executable_receipt_bytes: value.canonical_executable_receipt_bytes,
            call_correspondence_receipt_bytes: value.call_correspondence_receipt_bytes,
            retained_receipts_sum_bytes: value.retained_receipts_sum_bytes,
        }
    }
}

#[derive(Debug, Serialize)]
struct Sample {
    index: usize,
    phase: &'static str,
    source_collection_ns: Option<u128>,
    original_validation_transition_ns: Option<u128>,
    observation: Option<Identity>,
    baseline_equal_while_owner_live: bool,
    owner_constructed: bool,
    owner_dropped_before_sample_publication: bool,
    observation_failure: Option<Failure>,
    failure: Option<SeriesFailure>,
}
impl Sample {
    fn new(index: usize) -> Self {
        Self {
            index,
            phase: if index < CALIBRATION {
                "calibration"
            } else {
                "measurement"
            },
            source_collection_ns: None,
            original_validation_transition_ns: None,
            observation: None,
            baseline_equal_while_owner_live: false,
            owner_constructed: false,
            owner_dropped_before_sample_publication: false,
            observation_failure: None,
            failure: None,
        }
    }
}

// The production use receives the actual non-clone owner returned by the
// unchanged consuming transition. Neither the closure nor the result can retain
// a borrow from it. Panics unwind the original owner as well.
fn observe_then_drop<O, T>(owner: O, observe: impl FnOnce(&O) -> T) -> T {
    let result = observe(&owner);
    drop(owner);
    result
}

fn collect_sample(tcx: TyCtxt<'_>, profile: Profile, baseline: &[u8], index: usize) -> Sample {
    let mut sample = Sample::new(index);
    let (transaction, elapsed) = timed(|| {
        transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
    });
    sample.source_collection_ns = Some(elapsed);
    let transaction = match transaction {
        Ok(value) => value,
        Err(_) => {
            sample.failure = Some(SeriesFailure::SourceCollection);
            return sample;
        }
    };
    // Original per-transaction Work is created inside this unchanged method.
    // No retained/shared account is reset, refunded or replaced by this caller.
    let (result, elapsed) = timed(|| transaction.observe_ordered_program_v32());
    sample.original_validation_transition_ns = Some(elapsed);
    let owner = match result {
        Ok(value) => value,
        Err(_) => {
            sample.failure = Some(SeriesFailure::Validation);
            return sample;
        }
    };
    sample.owner_constructed = true;
    let result = observe_then_drop(owner, |owner| observe_live_owner(owner, profile, baseline));
    sample.owner_dropped_before_sample_publication = true;
    match result {
        Ok(value) => {
            sample.baseline_equal_while_owner_live = value.baseline_equal_while_owner_live;
            sample.observation = Some(Identity::observed(&value));
        }
        Err(error) => {
            sample.observation_failure = Some(error);
            sample.failure = Some(SeriesFailure::Observation);
        }
    }
    sample
}

fn emit_sample(sample: &Sample) -> Result<(), SeriesFailure> {
    let text = serde_json::to_string(&serde_json::json!({
        "schema":"fe2o3-ordered-validation-sample-v1", "sample":sample,
    }))
    .map_err(|_| SeriesFailure::Publication)?;
    if text.len() > SAMPLE_CAP {
        return Err(SeriesFailure::Publication);
    }
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "\n{SAMPLE_PREFIX}{text}").map_err(|_| SeriesFailure::Publication)?;
    output.flush().map_err(|_| SeriesFailure::Publication)
}

#[derive(Debug, Eq, PartialEq, Serialize)]
struct Summary {
    count: usize,
    p50_ns: u128,
    p95_ns: u128,
    maximum_ns: u128,
}
fn summary(samples: &[Sample], collection: bool) -> Option<Summary> {
    if samples.len() != TOTAL
        || samples.iter().enumerate().any(|(index, sample)| {
            sample.index != index
                || sample.failure.is_some()
                || sample.observation_failure.is_some()
                || sample.phase
                    != if index < CALIBRATION {
                        "calibration"
                    } else {
                        "measurement"
                    }
                || !sample.owner_constructed
                || !sample.owner_dropped_before_sample_publication
                || !sample.baseline_equal_while_owner_live
                || sample.observation.is_none()
                || sample.source_collection_ns.is_none()
                || sample.original_validation_transition_ns.is_none()
                || sample.observation != samples[0].observation
        })
    {
        return None;
    }
    let mut values = [0_u128; MEASURED];
    for (slot, sample) in values.iter_mut().zip(&samples[CALIBRATION..]) {
        *slot = if collection {
            sample.source_collection_ns?
        } else {
            sample.original_validation_transition_ns?
        };
    }
    values.sort_unstable();
    // Nearest-rank quantiles: ceil(0.50 * 30)=15; ceil(0.95 * 30)=29.
    Some(Summary {
        count: MEASURED,
        p50_ns: values[14],
        p95_ns: values[28],
        maximum_ns: values[29],
    })
}

struct SeriesCallbacks<'input> {
    profile: Profile,
    baseline: &'input [u8],
    calls: u8,
    compiler_start: Instant,
    frontend_ns: Option<u128>,
    callback_series_ns: Option<u128>,
    samples: Vec<Sample>,
    started_transactions: usize,
    failure: Option<SeriesFailure>,
}
impl Callbacks for SeriesCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls = self.calls.saturating_add(1).min(2);
        if self.calls != 1 {
            self.failure = Some(SeriesFailure::RepeatedCallback);
            return Compilation::Stop;
        }
        self.frontend_ns = Some(self.compiler_start.elapsed().as_nanos());
        let start = Instant::now();
        let mut identity: Option<Identity> = None;
        for index in 0..TOTAL {
            // This cannot interrupt an in-flight production operation; root's
            // independently bounded enclosing process remains mandatory.
            if start.elapsed() >= STOP_AFTER {
                self.failure = Some(SeriesFailure::TimeBudget);
                break;
            }
            self.started_transactions += 1;
            let mut sample = collect_sample(tcx, self.profile, self.baseline, index);
            if sample.failure.is_none() {
                if let Some(expected) = &identity {
                    if sample.observation.as_ref() != Some(expected) {
                        sample.failure = Some(SeriesFailure::IdentityDrift);
                    }
                } else {
                    identity = sample.observation.clone();
                }
            }
            if start.elapsed() >= STOP_AFTER {
                // Keep any first operation refusal instead of hiding it.
                sample.failure.get_or_insert(SeriesFailure::TimeBudget);
            }
            let failure = sample.failure;
            self.samples.push(sample);
            // Each completed prefix is flushed before another transaction is
            // created. A final report refusal cannot erase previous samples.
            if let Err(error) = emit_sample(self.samples.last().expect("just pushed")) {
                self.failure = Some(error);
                break;
            }
            if failure.is_some() {
                self.failure = failure;
                break;
            }
        }
        let elapsed = start.elapsed();
        self.callback_series_ns = Some(elapsed.as_nanos());
        if elapsed >= STOP_AFTER {
            self.failure.get_or_insert(SeriesFailure::TimeBudget);
        }
        Compilation::Stop
    }
}

#[test]
#[ignore = "root-owned genuine source, independent baseline and bounded 35-transaction process required"]
fn actual_source_validation_series() {
    let path = PathBuf::from(std::env::var_os(INPUT_ENV).expect("root-selected series config"));
    let mut config = RetainedBytes::open(&path, MAX_CONFIG).expect("retained series config");
    let input: Input = serde_json::from_slice(&config.bytes).expect("closed series config");
    assert_eq!(input.schema, "fe2o3-ordered-validation-series-input-v1");
    let profile = Profile::from_steps(input.steps).expect("exact 1/3/16 profile");
    let mut baseline =
        RetainedBytes::open(&input.baseline_path, fe2o3_kernel_ir::MAX_MODULE_BYTES_V1)
            .expect("retained original baseline");
    assert_eq!(baseline.bytes.len(), input.baseline_bytes);
    assert_eq!(digest(&baseline.bytes), input.baseline_sha256);
    assert!(
        request_ok(&input.rustc_args, &baseline.bytes),
        "bounded original invocation"
    );
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(TOTAL)
        .expect("fixed sample storage");
    let start = Instant::now();
    let mut callbacks = SeriesCallbacks {
        profile,
        baseline: &baseline.bytes,
        calls: 0,
        compiler_start: start,
        frontend_ns: None,
        callback_series_ns: None,
        samples,
        started_transactions: 0,
        failure: None,
    };
    let fatal = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(&input.rustc_args, &mut callbacks);
    })
    .is_err();
    let compiler_call_ns = start.elapsed().as_nanos();
    if fatal {
        callbacks.failure = Some(SeriesFailure::CompilerFatal);
    } else if callbacks.calls == 0 {
        callbacks.failure = Some(SeriesFailure::MissingCallback);
    } else if callbacks.calls != 1 {
        callbacks.failure = Some(SeriesFailure::RepeatedCallback);
    }
    let source_collection = if callbacks.failure.is_none() {
        summary(&callbacks.samples, true)
    } else {
        None
    };
    let validation = if callbacks.failure.is_none() {
        summary(&callbacks.samples, false)
    } else {
        None
    };
    let complete = source_collection.is_some() && validation.is_some();
    // Release the callback's baseline borrow before rechecking held descriptors.
    let calls = callbacks.calls;
    let frontend_ns = callbacks.frontend_ns;
    let callback_series_ns = callbacks.callback_series_ns;
    let failure = callbacks.failure;
    let started_transactions = callbacks.started_transactions;
    let samples = callbacks.samples;
    config.recheck().expect("post-compiler series config");
    baseline.recheck().expect("post-compiler series baseline");
    let value = serde_json::json!({
        "schema":"fe2o3-ordered-validation-series-v1",
        "series_mode":"same_callback_fresh_transactions_after_calibration",
        "steps":profile.steps(), "descriptors":profile.descriptors(),
        "calibration_samples":CALIBRATION, "measured_samples":MEASURED,
        "planned_transactions":TOTAL, "started_transactions":started_transactions,
        "completed_transactions":samples.len(),
        "compiler_invocations":1, "after_analysis_calls":calls,
        "compiler_call_ns":compiler_call_ns, "frontend_to_callback_ns":frontend_ns,
        "callback_series_ns":callback_series_ns, "callback_stopping_budget_ns":STOP_AFTER.as_nanos(),
        "samples":samples, "source_collection_summary":source_collection,
        "validation_summary":validation, "failure":failure, "complete":complete,
        "config_sha256":digest(&config.bytes), "baseline_sha256":digest(&baseline.bytes),
        "baseline_bytes":baseline.bytes.len(),
        "generation_ns":null, "complete_owner_logical_bytes":null,
        "warm_stage":false, "target_compliance":"unqualified",
        "source_authentication_exported":false, "artifact_or_launch_authority":false,
    });
    let text = serde_json::to_string(&value).expect("fixed series JSON");
    assert!(
        text.len() <= REPORT_CAP,
        "series report bounded; raw sample prefix retained"
    );
    println!("\n{PREFIX}{text}");
    io::stdout().flush().expect("series report flush");
    assert!(complete && failure.is_none(), "series failed: {failure:?}");
}

#[path = "ordered_program_validation_series_controls_v1.rs"]
mod controls;
