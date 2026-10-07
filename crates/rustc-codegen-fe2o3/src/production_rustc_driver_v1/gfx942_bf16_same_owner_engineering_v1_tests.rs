//! Separately ignored real-source qualification of the non-test continuation.
//! Existing generated-session and Worker config schemas are reused unchanged.
//! Root must supply fresh finite CPU Worker custody/containment; no GPU lease,
//! normal admission, artifact publication or default dispatch is added.
use super::*;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WORKER_CONFIG_ENV: &str = "FE2O3_BF16_PRIVATE_WORKER_CONFIG_V1";
const WORKER_CONFIG_CAP: usize = 8192;
const OUTPUT_BYTES: u64 = 4 * 1024 * 1024;
const STDOUT_BYTES: usize = 8 * 1024 * 1024;
const STDERR_BYTES: usize = 64 * 1024;
const EXECUTION_MS: u64 = 120_000;
const CHILD_MS: u64 = 300_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerConfig {
    schema: String,
    executable: String,
    executable_bytes: u64,
    executable_sha256: String,
    worker_build_identity: String,
    llvm_build_identity: String,
    // Canonical decimal prevents rounding; only next 300 seconds admitted.
    // Root's outer finite lease may be stricter and is independently mandatory.
    deadline_unix_ms: String,
}
fn parse_decimal(text: &str) -> Result<u64, &'static str> {
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("canonical decimal deadline");
    }
    text.parse().map_err(|_| "bounded decimal deadline")
}
fn measurement(config: &WorkerConfig) -> Result<WorkerMeasurementV1, &'static str> {
    if config.schema != "fe2o3-bf16-private-worker-config-v1"
        || config.executable.is_empty()
        || config.executable.len() > 4096
        || !Path::new(&config.executable).is_absolute()
        || !inputs::lower_digest(&config.executable_sha256)
    {
        return Err("closed explicit Worker configuration");
    }
    let mut digest = [0u8; 32];
    for (i, pair) in config
        .executable_sha256
        .as_bytes()
        .chunks_exact(2)
        .enumerate()
    {
        let h = |v: u8| if v <= b'9' { v - b'0' } else { v - b'a' + 10 };
        digest[i] = h(pair[0]) * 16 + h(pair[1]);
    }
    parse_decimal(&config.deadline_unix_ms)?;
    WorkerMeasurementV1::new(
        ContentIdentityV1::from_parts(digest, config.executable_bytes),
        config.worker_build_identity.clone(),
        config.llvm_build_identity.clone(),
    )
    .map_err(|_| "closed measured Worker identity")
}
fn child_deadline(text: &str, now_ms: u64, now: Instant) -> Result<Instant, &'static str> {
    let left = parse_decimal(text)?
        .checked_sub(now_ms)
        .ok_or("expired Worker request")?;
    if left == 0 || left > CHILD_MS {
        return Err("finite Worker child deadline");
    }
    now.checked_add(Duration::from_millis(left))
        .ok_or("Worker monotonic deadline overflow")
}
fn wall_ms() -> Result<u64, &'static str> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "pre-epoch clock")?
            .as_millis(),
    )
    .map_err(|_| "wall clock overflow")
}
fn read_worker_config() -> Result<(WorkerConfig, String), String> {
    let path =
        PathBuf::from(std::env::var_os(WORKER_CONFIG_ENV).ok_or("explicit private Worker config")?);
    if !path.is_absolute() || path.canonicalize().map_err(|e| e.to_string())? != path {
        return Err("canonical absolute Worker config path".into());
    }
    let bytes = super::super::gfx942_inline_value_qualification_v30_tests::read_bounded(
        &path,
        WORKER_CONFIG_CAP,
    )
    .map_err(|e| format!("{e:?}"))?;
    let config = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok((config, digest(&bytes)))
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Engineering,
    WrongReturn,
    EntryRefusals,
}

struct EngineeringBody<'a> {
    config: &'a Config,
    record: &'a inputs::Record,
    worker: &'a PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
    deadline: Instant,
    mode: Mode,
    calls: usize,
    completed: bool,
    expected_refusal: bool,
    engine_entries: Option<usize>,
    collected_rows: Option<usize>,
    postflights: Option<usize>,
    row: Option<Value>,
    failure: Option<String>,
}
fn is_source_profile_refusal(error: &crate::production_pipeline::ProductionPipelineError) -> bool {
    matches!(error,
        crate::production_pipeline::ProductionPipelineError::Bf16TileValuesInspection(inner)
        if matches!(inner.as_ref(),
            fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1::Unavailable(
                "BF16 same-owner source profile")))
}
impl Callbacks for EngineeringBody<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        if self.calls != 1 {
            self.failure = Some("more than one same-owner engineering callback".into());
            return Compilation::Stop;
        }
        let scope = if self.mode == Mode::EntryRefusals {
            None // The six genuine mutation controls own a deny-entry scope.
        } else {
            match crate::production_ranked_projection_v1::PrivateBf16WorkerHandoffV1::
                begin_same_owner_engineering_observation_for_test_v1()
            {
                Ok(scope) => Some(scope),
                Err(error) => {
                    self.failure = Some(diagnostic(&error));
                    return Compilation::Stop;
                }
            }
        };
        let source_order = owning_requested_permutation(self.config.session).unwrap();
        let requested = if self.mode == Mode::WrongReturn {
            if source_order == [0, 1, 2, 3] {
                [1, 0, 2, 3]
            } else {
                [0, 1, 2, 3]
            }
        } else {
            source_order
        };
        match super::super::transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        ) {
            Ok(transaction) => {
                let prepared =
                    transaction.prepare_bf16_same_owner_handoff_v1(requested, |source, budget| {
                        use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                        budget.charge_work(256)?;
                        if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                            || super::super::lower_hex_v1(source.source().sha256())
                                != self.record.spec.source.sha256
                            || source.relation().return_permutation() != source_order
                        {
                            return Err(E::Unavailable(
                                "same-owner engineering source/Return differs",
                            ));
                        }
                        Ok(())
                    });
                match prepared {
                    Ok(handoff) if self.mode == Mode::WrongReturn => {
                        drop(handoff);
                        self.failure = Some("opposite Return unexpectedly prepared".into());
                    }
                    Ok(handoff) => {
                        let attempted = if self.mode == Mode::EntryRefusals {
                            handoff.exercise_engineering_refusals_and_drop_for_test_v1(
                                requested,
                                self.worker,
                                OUTPUT_BYTES,
                                self.limits,
                                self.deadline,
                            )
                        } else {
                            handoff.observe_engineering_and_drop_v1(
                                requested,
                                self.worker,
                                WorkerOutputConstraintsV1::new(OUTPUT_BYTES).unwrap(),
                                self.limits,
                                self.deadline,
                            )
                        };
                        self.completed = attempted.is_ok();
                        self.failure = attempted.err().map(|error| diagnostic(&error));
                        if self.mode == Mode::EntryRefusals && self.completed {
                            // The source-owned control itself proved zero entries
                            // for all six guards and restored the same allocations.
                            self.engine_entries = Some(0);
                            self.collected_rows = Some(0);
                            self.postflights = Some(0);
                        }
                    }
                    Err(error)
                        if self.mode == Mode::WrongReturn && is_source_profile_refusal(&error) =>
                    {
                        self.expected_refusal = true;
                    }
                    Err(error) => self.failure = Some(diagnostic(&error)),
                }
            }
            Err(error) => self.failure = Some(diagnostic(&error)),
        }
        if let Some(scope) = scope {
            let observed = scope.snapshot();
            self.engine_entries = Some(observed.engine_entries);
            self.collected_rows = Some(observed.collected_rows);
            self.postflights = Some(observed.postflights);
            self.row = observed.row.map(|row| json!({
                "handoff_sha256":super::super::lower_hex_v1(row.handoff.sha256()),
                "handoff_bytes":row.handoff.byte_len(),
                "hsaco_sha256":super::super::lower_hex_v1(row.hsaco.sha256()),
                "hsaco_bytes":row.hsaco.byte_len(),
                "worker_sha256":super::super::lower_hex_v1(row.worker.sha256()),
                "worker_bytes":row.worker.byte_len(),
                "descriptor_sha256":super::super::lower_hex_v1(&row.descriptor_sha),
                "descriptor_bytes":row.descriptor_bytes,
                "bootstrap_request_sha256":super::super::lower_hex_v1(row.bootstrap_request.sha256()),
                "bootstrap_response_sha256":super::super::lower_hex_v1(row.bootstrap_response.sha256()),
                "replay_request_sha256":super::super::lower_hex_v1(row.replay_request.sha256()),
                "replay_response_sha256":super::super::lower_hex_v1(row.replay_response.sha256())
            }));
            drop(scope);
        }
        Compilation::Stop
    }
}

fn run_same_owner_engineering(mode: Mode) {
    let started = Instant::now();
    let config: Config = read_config().expect("closed generated session config");
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let (worker_config, worker_config_sha256) =
        read_worker_config().expect("separately pinned finite Worker config");
    let expected = measurement(&worker_config).unwrap();
    let deadline = child_deadline(
        &worker_config.deadline_unix_ms,
        wall_ms().unwrap(),
        Instant::now(),
    )
    .unwrap()
    .min(
        started
            .checked_add(Duration::from_millis(CHILD_MS))
            .unwrap(),
    );
    let limits = WorkerExecutionLimitsV1::new(
        Duration::from_millis(EXECUTION_MS),
        STDOUT_BYTES,
        STDERR_BYTES,
    )
    .unwrap();
    // Capture/seal only. Loader/currentness/whole-family cleanup are outer duties.
    let worker = PinnedWorkerV1::open(Path::new(&worker_config.executable), expected).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = EngineeringBody {
        config: &config,
        record: &record,
        worker: &worker,
        limits,
        deadline,
        mode,
        calls: 0,
        completed: false,
        expected_refusal: false,
        engine_entries: None,
        collected_rows: None,
        postflights: None,
        row: None,
        failure: None,
    };
    timely(started).unwrap();
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(&record.args, &mut body))
    }));
    let compiler_clean = matches!(run, Ok(Ok(())));
    let recheck = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inputs::derive(&record.spec)
    }));
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline_met = Instant::now() < deadline && timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let worker_config_unchanged =
        read_worker_config().is_ok_and(|(_, sha)| sha == worker_config_sha256);
    let zero_entry = body.engine_entries == Some(0)
        && body.collected_rows == Some(0)
        && body.postflights == Some(0)
        && body.row.is_none();
    let mode_ok = match mode {
        Mode::Engineering => {
            body.completed
                && !body.expected_refusal
                && body.engine_entries == Some(1)
                && body.collected_rows == Some(1)
                && body.postflights == Some(1)
                && body.row.is_some()
        }
        Mode::WrongReturn => !body.completed && body.expected_refusal && zero_entry,
        Mode::EntryRefusals => body.completed && !body.expected_refusal && zero_entry,
    };
    let collected = compiler_clean
        && unchanged
        && deadline_met
        && analysis_empty
        && sidecar_absent
        && worker_config_unchanged
        && body.calls == 1
        && body.failure.is_none()
        && mode_ok;
    let engine_completed = body.completed && mode == Mode::Engineering;
    let mut frame = json!({
        "schema":"fe2o3-bf16-same-owner-engineering-test-v1",
        "session":config.session,
        "mode":match mode { Mode::Engineering=>"engineering", Mode::WrongReturn=>"wrong-return", Mode::EntryRefusals=>"entry-refusals" },
        "record_sha256":config.record_sha256,"source_pin":record.spec.source,
        "worker_config_sha256":worker_config_sha256,"worker_config_unchanged":worker_config_unchanged,
        "worker_executable_sha256":worker_config.executable_sha256,
        "worker_executable_bytes":worker_config.executable_bytes,
        "worker_build_identity":worker_config.worker_build_identity,
        "llvm_build_identity":worker_config.llvm_build_identity,
        "actual_rustc_callbacks":body.calls,"compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged,"deadline_met":deadline_met,
        "analysis_output_empty":analysis_empty,"sidecar_absent":sidecar_absent,
    });
    let continuation = json!({
        "non_test_engineering_completed":engine_completed,
        "expected_source_profile_refusal":body.expected_refusal,
        "six_actual_owner_entry_refusals":body.completed && mode==Mode::EntryRefusals,
        "engine_entries":body.engine_entries,"collected_rows":body.collected_rows,
        "source_postflights":body.postflights,"engine_row":body.row,
        "worker_invoked":if engine_completed {Some(true)} else if zero_entry {Some(false)} else {None},
        "worker_invocations":if engine_completed {Some(2u32)} else if zero_entry {Some(0u32)} else {None},
        "source_descriptor_handoff_replayed":engine_completed,
        "full_retained_descriptor_replayed":engine_completed,
        "original_accounts_retained":engine_completed,
        "opaque_owner_dropped":body.completed,
        "engine_owners_dropped":engine_completed,
        "selected_storage_restored":engine_completed,
    });
    let qualification = json!({
        "external_engine_allocations_excluded":true,"root_cleanup_required":true,
        "collected":collected,"failure":body.failure,
        "normal_admission":false,"formal_admission":false,"hardware_observed":false,
        "numerical_qualification":false,"publication_authority":false,
        "load_authority":false,"launch_authority":false
    });
    // Separate bounded macro expansions preserve one exact closed wire object.
    for group in [continuation, qualification] {
        let serde_json::Value::Object(fields) = group else {
            unreachable!("fixed JSON object");
        };
        for (key, value) in fields {
            assert!(frame.as_object_mut().unwrap().insert(key, value).is_none());
        }
    }
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= 16 * 1024 && encoded.len() <= FRAME_CAP);
    super::super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 non-test same-owner engineering observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_SAME_OWNER_ENGINEERING_V1 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = Instant::now() < deadline && timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    drop(worker);
    assert!(
        collected && final_deadline,
        "same-owner engineering refused; no normal/launch qualification"
    );
}

#[test]
#[ignore = "requires fresh exact source/Worker runtime custody and finite root containment; no GPU"]
fn actual_generated_same_owner_engineering_source() {
    run_same_owner_engineering(Mode::Engineering);
}
#[test]
#[ignore = "requires fresh source and sealed Worker image; exact wrong Return with zero engine entry"]
fn actual_generated_same_owner_engineering_wrong_return() {
    run_same_owner_engineering(Mode::WrongReturn);
}
#[test]
#[ignore = "requires actual source owner and measured image; six restored mutations must enter no engine"]
fn actual_generated_same_owner_engineering_entry_refusals() {
    run_same_owner_engineering(Mode::EntryRefusals);
}

#[test]
fn worker_config_is_closed_canonical_and_uses_exact_measurement() {
    let value = json!({"schema":"fe2o3-bf16-private-worker-config-v1","executable":"/inert/worker",
        "executable_bytes":64,"executable_sha256":"ab".repeat(32),"worker_build_identity":"worker-test",
        "llvm_build_identity":"llvm-test","deadline_unix_ms":"123456"});
    let config: WorkerConfig = serde_json::from_value(value.clone()).unwrap();
    let m = measurement(&config).unwrap();
    assert_eq!(
        m.executable(),
        ContentIdentityV1::from_parts([0xab; 32], 64)
    );
    assert_eq!(m.worker_build_identity(), "worker-test");
    assert_eq!(m.llvm_build_identity(), "llvm-test");
    let mut unknown = value.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("authority".into(), json!(true));
    assert!(serde_json::from_value::<WorkerConfig>(unknown).is_err());
    for (key, bad) in [
        ("schema", json!("other")),
        ("executable", json!("relative")),
        ("executable_bytes", json!(0)),
        ("executable_sha256", json!("AB".repeat(32))),
        ("worker_build_identity", json!("bad\nidentity")),
        ("deadline_unix_ms", json!("0123")),
    ] {
        let mut changed = value.clone();
        changed[key] = bad;
        assert!(measurement(&serde_json::from_value::<WorkerConfig>(changed).unwrap()).is_err());
    }
}
#[test]
fn worker_deadline_is_finite_without_reusable_lease() {
    let now = Instant::now();
    assert_eq!(
        child_deadline("301000", 1000, now)
            .unwrap()
            .duration_since(now),
        Duration::from_millis(CHILD_MS)
    );
    for value in [
        "0",
        "999",
        "1000",
        "301001",
        "+1001",
        "01",
        "18446744073709551616",
    ] {
        assert!(child_deadline(value, 1000, now).is_err());
    }
}

#[test]
fn wrong_return_is_only_the_exact_typed_source_profile_error() {
    use crate::production_pipeline::ProductionPipelineError as P;
    use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
    assert!(is_source_profile_refusal(&P::Bf16TileValuesInspection(
        Box::new(E::Unavailable("BF16 same-owner source profile"),)
    )));
    for text in [
        "",
        "same-owner engineering source/Return differs",
        "BF16 source profile",
    ] {
        assert!(!is_source_profile_refusal(&P::Bf16TileValuesInspection(
            Box::new(E::Unavailable(text))
        )));
    }
}
#[test]
fn genuine_hook_enters_non_test_connector_not_the_old_test_owner_constructor() {
    let source = include_str!("gfx942_bf16_same_owner_engineering_v1_tests.rs");
    let body = source.split("#[test]").next().unwrap();
    let compact = body.split_whitespace().collect::<String>();
    for required in [
        "transaction.prepare_bf16_same_owner_handoff_v1(",
        "handoff.observe_engineering_and_drop_v1(",
        "inputs::read_record(",
        "inputs::derive(&record.spec)",
        "read_worker_config()",
        "body.engine_entries == Some(1)",
        "body.postflights == Some(1)",
    ] {
        assert!(
            compact.contains(&required.split_whitespace().collect::<String>()),
            "{required}"
        );
    }
    for forbidden in [
        "observe_bf16_owned_worker_for_test_v1(",
        "observe_private_bf16_worker_for_test_v1(",
        "execute_v2(",
        "observe_engineering_hsaco_v1(",
    ] {
        assert!(
            !compact.contains(&forbidden.split_whitespace().collect::<String>()),
            "{forbidden}"
        );
    }
    let donor = include_str!("gfx942_bf16_private_worker_v1_tests.rs");
    let ours = body
        .split("const WORKER_CONFIG_ENV:")
        .nth(1)
        .unwrap()
        .split("\n#[derive(Clone, Copy, Eq, PartialEq)]")
        .next()
        .unwrap();
    let old = donor
        .split("const WORKER_CONFIG_ENV:")
        .nth(1)
        .unwrap()
        .split("\nstruct WorkerBody")
        .next()
        .unwrap();
    assert!(body.contains("fe2o3-bf16-private-worker-config-v1"));
    assert_eq!(
        ours.trim(),
        old.trim(),
        "exact existing config/measurement/deadline helpers"
    );
}
