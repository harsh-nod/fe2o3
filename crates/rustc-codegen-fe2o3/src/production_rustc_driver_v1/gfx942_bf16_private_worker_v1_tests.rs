//! Separately named, ignored genuine Worker route. This is not an execution
//! lease, production qualification, artifact publication, or GPU launch path.
//! Root must authenticate the entire native/runtime closure and containment
//! before selecting this ignored test; R70 CPU scope alone is insufficient.
//! Request/config/engine/native allocations are bounded external exclusions;
//! selected owner loans use original projection/materialization accounts.
//! The existing 63-field handoff endpoint is unchanged.
use super::*;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerMeasurementV1,
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

struct WorkerBody<'a> {
    config: &'a Config,
    record: &'a inputs::Record,
    worker: &'a PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
    deadline: Instant,
    calls: usize,
    completed: bool,
    phase: Option<Value>,
    failure: Option<String>,
}
impl Callbacks for WorkerBody<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        if self.calls != 1 {
            self.failure = Some("more than one actual Worker analysis callback".into());
            return Compilation::Stop;
        }
        match super::super::transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        ) {
            Ok(transaction) => {
                let (result, phase) = transaction.observe_bf16_owned_worker_for_test_v1(
                    owning_requested_permutation(self.config.session)
                        .expect("selected Identity/Swap01"),
                    self.worker,
                    OUTPUT_BYTES,
                    self.limits,
                    self.deadline,
                    |source, emission, budget| {
                        use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                        budget.charge_work(256)?;
                        if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                            || super::super::lower_hex_v1(source.source().sha256())
                                != self.record.spec.source.sha256
                            || Some(emission.return_permutation())
                                != owning_requested_permutation(self.config.session)
                        {
                            return Err(E::Unavailable("actual Worker source/Return differs"));
                        }
                        Ok(())
                    },
                );
                self.phase = phase.map(|p| serde_json::to_value(p).expect("fixed phase"));
                self.completed = result.is_ok();
                self.failure = result.err().map(|e| diagnostic(&e));
            }
            Err(error) => self.failure = Some(diagnostic(&error)),
        }
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires separately authenticated finite native Worker request and root containment; no GPU/launch authority"]
fn actual_generated_owning_worker_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let (worker_config, worker_config_sha256) =
        read_worker_config().expect("separate pinned Worker request");
    let expected = measurement(&worker_config).expect("closed Worker measurement");
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
    // Capture/seal is not execution. Native loader/library identity and hostile
    // descendant containment remain root-owned, not claims of this image API.
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
    let mut body = WorkerBody {
        config: &config,
        record: &record,
        worker: &worker,
        limits,
        deadline,
        calls: 0,
        completed: false,
        phase: None,
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
    let unchanged = matches!(&recheck,Ok(Ok(actual)) if actual==&record);
    let deadline_met = Instant::now() < deadline && timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let worker_config_unchanged =
        read_worker_config().is_ok_and(|(_, sha)| sha == worker_config_sha256);
    let collected = compiler_clean
        && unchanged
        && deadline_met
        && analysis_empty
        && sidecar_absent
        && worker_config_unchanged
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-owning-worker-observation-v1",
        "session":config.session,"requested_order":if config.session==1 {"identity"} else {"swap01"},
        "record_sha256":config.record_sha256,"source_pin":record.spec.source,
        "worker_config_sha256":worker_config_sha256,"worker_config_unchanged":worker_config_unchanged,
        "worker_executable_sha256":worker_config.executable_sha256,
        "worker_executable_bytes":worker_config.executable_bytes,
        "worker_build_identity":worker_config.worker_build_identity,
        "llvm_build_identity":worker_config.llvm_build_identity,
        "actual_rustc_callbacks":body.calls,"compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged,"deadline_met":deadline_met,
        "analysis_output_empty":analysis_empty,"sidecar_absent":sidecar_absent,
        "worker_owner_entry_completed":body.completed,"worker_owner_collection_completed":collected,
        "phase":body.phase,"failure":body.failure
    });
    let Value::Object(fields) = json!({
        "source_first_worker_return_refused":body.completed,
        "full_retained_descriptor_replayed":body.completed,
        "source_descriptor_handoff_replayed":body.completed,
        "original_materialization_account_retained":body.completed,
        "selected_storage_restored":body.completed,"engine_owners_dropped":body.completed,
        "worker_owner_drop_completed":body.completed,"runtime_bounds_alias_duties_preserved":body.completed,
        "worker_invoked":if body.completed {Some(true)} else {None},
        "worker_invocations":if body.completed {Some(2u32)} else {None},
        "external_engine_allocations_excluded":true,"root_supervision_required":true,
        "qualification_accepted":false,"normal_qualified":false,
        "formal_admission":false,"optimized_formal_admission":false,
        "hardware_observed":false,"numerical_cpu_qualified":false,
        "publication_authority":false,"load_authority":false,"launch_authority":false,
        "launch_inputs_authenticated":false,"source_authority_in_report":false
    }) else {
        unreachable!("object literal")
    };
    frame.as_object_mut().unwrap().extend(fields);
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private owning Worker observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_OWNING_WORKER_V1 {}",
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
        "private owning Worker refused; no ordinary/launch qualification"
    );
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
fn genuine_worker_route_preserves_old_endpoint_and_closed_borrow() {
    let driver = include_str!("gfx942_bf16_generated_source_qualification_v1_tests.rs");
    let old = driver
        .split("\n#[path = \"gfx942_bf16_private_worker_v1_tests.rs\"]")
        .next()
        .unwrap();
    assert!(old.contains("fn actual_generated_owning_handoff_source()"));
    assert!(!old.contains("actual_generated_owning_worker_source"));
    assert!(!old.contains("observe_bf16_owned_worker_for_test_v1"));
    let pipeline = include_str!("../production_pipeline/bf16_tile_values_cpu_v1_tests.rs");
    let start = pipeline
        .find("if let NominalQueryProfileV1::OwnedHandoff { requested_return } = profile")
        .unwrap();
    let handoff = pipeline[start..]
        .split("if let NominalQueryProfileV1::OwnedDescriptor")
        .next()
        .unwrap();
    assert!(!handoff.contains("observe_private_bf16_worker_for_test_v1"));
    let start = pipeline
        .find("if let NominalQueryProfileV1::OwnedWorker { requested_return } = profile")
        .unwrap();
    let loan = pipeline[start..]
        .split("if let NominalQueryProfileV1::OwnedHandoff")
        .next()
        .unwrap();
    assert!(
        loan.find("observe_private_bf16_worker_for_test_v1")
            .unwrap()
            < loan.find("drop(stage)").unwrap()
    );
    assert!(
        loan.find("drop(stage)").unwrap()
            < loan
                .find("fe2o3-bf16-private-worker-owner-drop-v1")
                .unwrap()
    );
    assert!(loan.contains("CorrespondenceMismatch"));
    assert!(loan.contains("attempted.map_err(Box::new)?"));
    assert!(!loan.contains("Budget::new"));
}
