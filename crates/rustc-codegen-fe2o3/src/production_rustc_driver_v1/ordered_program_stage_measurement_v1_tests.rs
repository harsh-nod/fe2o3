//! Task-private genuine-source validation observation. Test builds only.
//! No query/provider substitution, public compiler API, owner reconstruction,
//! source generation timing, warm-stage claim or whole-owner storage claim.
use super::{
    Callbacks, Compilation, Compiler, TyCtxt, require_canonical_overflow_checks_v1,
    transaction_in_active_session_v1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticTerminatorKindV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Instant,
};

const INPUT_ENV: &str = "FE2O3_ORDERED_STAGE_MEASUREMENT_V1_CONFIG";
const PREFIX: &str = "fe2o3 ordered-stage measurement v1: ";
const MAX_CONFIG: usize = 65_536;
const MAX_ARGS: usize = 256;
const MAX_ARG_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Failure {
    Request,
    MissingCallback,
    RepeatedCallback,
    CompilerFatal,
    SourceCollection,
    Validation,
    ActualProfile,
    BaselineMismatch,
    ReceiptArithmetic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Profile {
    One,
    Three,
    Sixteen,
}
impl Profile {
    fn from_steps(steps: u8) -> Result<Self, Failure> {
        match steps {
            1 => Ok(Self::One),
            3 => Ok(Self::Three),
            16 => Ok(Self::Sixteen),
            _ => Err(Failure::Request),
        }
    }
    fn steps(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Three => 3,
            Self::Sixteen => 16,
        }
    }
    // Exact descriptor spelling of the frozen real ordered_program_v32.rs
    // source fixtures, not executable AMD words or substitute source evidence.
    fn descriptors(self) -> [u16; 16] {
        match self {
            Self::One => [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Self::Three => [133, 307, 413, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Self::Sixteen => [
                0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72,
            ],
        }
    }
    fn matches(self, count: u8, words: &[u16; 16]) -> bool {
        count == self.steps() && words == &self.descriptors()
    }
}

#[derive(Debug, Serialize)]
struct Observation {
    steps: u8,
    descriptors: [u16; 16],
    canonical_identity: [u8; 32],
    canonical_bytes: usize,
    semantic_identity: [u8; 32],
    source_inventory_identity: [u8; 32],
    source_preflight_identity: [u8; 32],
    baseline_equal_while_owner_live: bool,
    canonical_executable_receipt_bytes: usize,
    call_correspondence_receipt_bytes: usize,
    retained_receipts_sum_bytes: usize,
    // These components explicitly do NOT cover all SSA, source, descriptors,
    // context entries, correspondence, transaction or rustc query allocations.
    complete_retained_owner_bytes: Option<usize>,
    complete_peak_owner_bytes: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Attempt {
    steps: u8,
    compiler_invocations: u8,
    after_analysis_calls: u8,
    compiler_call_ns: Option<u128>,
    frontend_to_callback_ns: Option<u128>,
    source_collection_ns: Option<u128>,
    original_validation_transition_ns: Option<u128>,
    generation_ns: Option<u128>,
    observation: Option<Observation>,
    failure: Option<Failure>,
    warm_stage: bool,
    generation_or_whole_storage_target_qualified: bool,
}

struct StageCallbacks<'input> {
    profile: Profile,
    baseline: &'input [u8],
    calls: u8,
    compiler_start: Option<Instant>,
    frontend_ns: Option<u128>,
    collection_ns: Option<u128>,
    validation_ns: Option<u128>,
    result: Option<Result<Observation, Failure>>,
}

fn timed<T>(operation: impl FnOnce() -> T) -> (T, u128) {
    let start = Instant::now();
    let result = operation();
    (result, start.elapsed().as_nanos())
}

fn observe_live_owner(
    owner: &crate::production_pipeline::ordered_program_diagnostic_v32::OrderedProgramObservationOwnerV32,
    profile: Profile,
    baseline: &[u8],
) -> Result<Observation, Failure> {
    let materialized = owner.materialized();
    let semantic = materialized.semantic_ssa().source_semantic();
    if semantic.roots().len() != 1 || semantic.functions().len() != 1 {
        return Err(Failure::ActualProfile);
    }
    let root = semantic.roots()[0];
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or(Failure::ActualProfile)?;
    let mut actual = None;
    // The actual validated owner already enforces its closed bounded root CFG.
    // This post-timer oracle only borrows its real source call and never builds
    // another graph, planner, canonical decoder or runtime-packing benchmark.
    for (index, block) in function.blocks().iter().enumerate() {
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && matches!(
                semantic.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(_),
                    ..
                })
            )
        {
            let index = u32::try_from(index).map_err(|_| Failure::ActualProfile)?;
            let view = semantic
                .checked_gfx942_ordered_program_call_v32(root, SemanticBlockIdV1::from_index(index))
                .map_err(|_| Failure::ActualProfile)?;
            let program = view.program();
            let registers = view.registers();
            if actual.is_some()
                || !profile.matches(program.count(), program.descriptors())
                || registers.scratch() != 32
                || registers.output() != 33
                || registers.inputs() != [34, 35, 36]
            {
                return Err(Failure::ActualProfile);
            }
            actual = Some(program);
        }
    }
    let actual = actual.ok_or(Failure::ActualProfile)?;
    let executable = materialized.executable();
    let bytes = executable.canonical().canonical_bytes();
    if bytes != baseline {
        return Err(Failure::BaselineMismatch);
    }
    let executable_storage = materialized.executable_storage().retained_storage();
    let correspondence_storage = materialized.call_correspondence_storage();
    let total = executable_storage
        .checked_add(correspondence_storage)
        .ok_or(Failure::ReceiptArithmetic)?;
    let (inventory, preflight) = owner.authenticated_source_identities();
    Ok(Observation {
        steps: actual.count(),
        descriptors: *actual.descriptors(),
        canonical_identity: *executable.identity().digest(),
        canonical_bytes: bytes.len(),
        semantic_identity: *semantic.semantic_sha256().as_bytes(),
        source_inventory_identity: inventory,
        source_preflight_identity: preflight,
        baseline_equal_while_owner_live: true,
        canonical_executable_receipt_bytes: executable_storage,
        call_correspondence_receipt_bytes: correspondence_storage,
        retained_receipts_sum_bytes: total,
        complete_retained_owner_bytes: None,
        complete_peak_owner_bytes: None,
    })
}

impl Callbacks for StageCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls = self.calls.saturating_add(1).min(2);
        if self.calls != 1 {
            self.result = Some(Err(Failure::RepeatedCallback));
            return Compilation::Stop;
        }
        self.frontend_ns = self.compiler_start.map(|start| start.elapsed().as_nanos());
        let (transaction, elapsed) = timed(|| {
            transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
        });
        self.collection_ns = Some(elapsed);
        self.result = Some(match transaction {
            Err(_) => Err(Failure::SourceCollection),
            Ok(transaction) => {
                // This is the unchanged production transition. The timer covers
                // semantic import, middle-end, SSA, descriptor/launch checks and
                // V17 materialization/retained canonical reservations. It stops
                // before the borrowed result oracle and source/log serialization.
                let (result, elapsed) = timed(|| transaction.observe_ordered_program_v32());
                self.validation_ns = Some(elapsed);
                match result {
                    Err(_) => Err(Failure::Validation),
                    Ok(owner) => observe_live_owner(&owner, self.profile, self.baseline),
                }
            }
        });
        Compilation::Stop
    }
}

fn finish<T>(result: Option<Result<T, Failure>>, calls: u8, fatal: bool) -> Result<T, Failure> {
    if fatal {
        Err(Failure::CompilerFatal)
    } else if calls > 1 {
        Err(Failure::RepeatedCallback)
    } else if calls != 1 {
        Err(Failure::MissingCallback)
    } else {
        result.unwrap_or(Err(Failure::MissingCallback))
    }
}

fn request_ok(args: &[String], baseline: &[u8]) -> bool {
    !args.is_empty()
        && args.len() <= MAX_ARGS
        && args
            .iter()
            .all(|value| !value.is_empty() && value.len() <= 4096 && !value.contains('\0'))
        && args
            .iter()
            .try_fold(0_usize, |n, value| n.checked_add(value.len()))
            .is_some_and(|n| n <= MAX_ARG_BYTES)
        && !baseline.is_empty()
        && baseline.len() <= fe2o3_kernel_ir::MAX_MODULE_BYTES_V1
        && require_canonical_overflow_checks_v1(args).is_ok()
}

fn run(args: &[String], profile: Profile, baseline: &[u8]) -> Attempt {
    let mut attempt = Attempt {
        steps: profile.steps(),
        compiler_invocations: 0,
        after_analysis_calls: 0,
        compiler_call_ns: None,
        frontend_to_callback_ns: None,
        source_collection_ns: None,
        original_validation_transition_ns: None,
        generation_ns: None,
        observation: None,
        failure: None,
        warm_stage: false,
        generation_or_whole_storage_target_qualified: false,
    };
    if !request_ok(args, baseline) {
        attempt.failure = Some(Failure::Request);
        return attempt;
    }
    let mut callbacks = StageCallbacks {
        profile,
        baseline,
        calls: 0,
        compiler_start: None,
        frontend_ns: None,
        collection_ns: None,
        validation_ns: None,
        result: None,
    };
    attempt.compiler_invocations = 1;
    let start = Instant::now();
    callbacks.compiler_start = Some(start);
    let fatal =
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, &mut callbacks))
            .is_err();
    attempt.compiler_call_ns = Some(start.elapsed().as_nanos());
    attempt.after_analysis_calls = callbacks.calls;
    attempt.frontend_to_callback_ns = callbacks.frontend_ns;
    attempt.source_collection_ns = callbacks.collection_ns;
    attempt.original_validation_transition_ns = callbacks.validation_ns;
    match finish(callbacks.result, callbacks.calls, fatal) {
        Ok(observation) => attempt.observation = Some(observation),
        Err(error) => attempt.failure = Some(error),
    }
    attempt
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema: String,
    steps: u8,
    rustc_args: Vec<String>,
    baseline_path: PathBuf,
    baseline_bytes: usize,
    baseline_sha256: String,
}

struct RetainedBytes {
    path: PathBuf,
    file: File,
    stat: Metadata,
    bytes: Vec<u8>,
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.nlink() == 1
        && b.nlink() == 1
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
impl RetainedBytes {
    fn open(path: &Path, cap: usize) -> Result<Self, String> {
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || fs::canonicalize(path).map_err(|_| "input canonical path")? != path
        {
            return Err("input must be one exact canonical absolute file".into());
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| "input open")?;
        let stat = file.metadata().map_err(|_| "input metadata")?;
        let length = usize::try_from(stat.len()).map_err(|_| "input size conversion")?;
        if length == 0
            || length > cap
            || !same(
                &stat,
                &fs::symlink_metadata(path).map_err(|_| "input path metadata")?,
            )
        {
            return Err("input regular bounded identity".into());
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| "input allocation")?;
        let mut chunk = [0_u8; 8192];
        while bytes.len() < length {
            let remaining = (length - bytes.len()).min(chunk.len());
            let count = file
                .read(&mut chunk[..remaining])
                .map_err(|_| "input read")?;
            if count == 0 {
                return Err("input shortened".into());
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        if file
            .read(&mut chunk[..1])
            .map_err(|_| "input trailing read")?
            != 0
        {
            return Err("input grew".into());
        }
        let mut retained = Self {
            path: path.to_owned(),
            file,
            stat,
            bytes,
        };
        retained.recheck()?;
        Ok(retained)
    }
    fn recheck(&mut self) -> Result<(), String> {
        if !same(
            &self.stat,
            &self
                .file
                .metadata()
                .map_err(|_| "retained descriptor metadata")?,
        ) || !same(
            &self.stat,
            &fs::symlink_metadata(&self.path).map_err(|_| "retained path metadata")?,
        ) || fs::canonicalize(&self.path).map_err(|_| "retained canonical path")? != self.path
        {
            return Err("retained input identity changed".into());
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| "retained seek")?;
        let mut offset = 0_usize;
        let mut chunk = [0_u8; 8192];
        loop {
            let count = self.file.read(&mut chunk).map_err(|_| "retained read")?;
            if count == 0 {
                break;
            }
            let end = offset
                .checked_add(count)
                .ok_or("retained size arithmetic")?;
            if self.bytes.get(offset..end) != Some(&chunk[..count]) {
                return Err("retained input bytes changed".into());
            }
            offset = end;
        }
        if offset != self.bytes.len()
            || !same(
                &self.stat,
                &self
                    .file
                    .metadata()
                    .map_err(|_| "retained final metadata")?,
            )
            || !same(
                &self.stat,
                &fs::symlink_metadata(&self.path).map_err(|_| "retained final path")?,
            )
        {
            return Err("retained final identity changed".into());
        }
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> String {
    super::lower_hex_v1(&<[u8; 32]>::from(Sha256::digest(bytes)))
}

#[test]
#[ignore = "root-owned genuine1/3/16 source, exact argv and unchanged-route baseline required"]
fn actual_source_probe() {
    let path = PathBuf::from(
        std::env::var_os(INPUT_ENV).expect("root must select the exact probe config"),
    );
    let mut config = RetainedBytes::open(&path, MAX_CONFIG).expect("retained config");
    let input: Input = serde_json::from_slice(&config.bytes).expect("closed probe config");
    assert_eq!(input.schema, "fe2o3-ordered-stage-measurement-input-v1");
    let profile = Profile::from_steps(input.steps).expect("exact1/3/16 profile");
    assert!(
        input.baseline_bytes > 0 && input.baseline_bytes <= fe2o3_kernel_ir::MAX_MODULE_BYTES_V1
    );
    assert_eq!(input.baseline_sha256.len(), 64);
    assert!(
        input
            .baseline_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let mut baseline =
        RetainedBytes::open(&input.baseline_path, fe2o3_kernel_ir::MAX_MODULE_BYTES_V1)
            .expect("retained baseline");
    assert_eq!(baseline.bytes.len(), input.baseline_bytes);
    assert_eq!(digest(&baseline.bytes), input.baseline_sha256);
    let attempt = run(&input.rustc_args, profile, &baseline.bytes);
    // Recheck both original retained files before publishing the result line.
    // Root retains any panic/error and never treats a missing line as zero time.
    config.recheck().expect("post-compiler config");
    baseline.recheck().expect("post-compiler baseline");
    let value = serde_json::json!({
        "schema": "fe2o3-ordered-stage-measurement-v1",
        "attempt": &attempt,
        "config_sha256": digest(&config.bytes),
        "baseline_sha256": digest(&baseline.bytes),
        "caller_baseline_capacity_bytes": baseline.bytes.capacity(),
        "caller_config_capacity_bytes": config.bytes.capacity(),
        "generation_ns": null,
        "complete_owner_logical_bytes": null,
        "warm_stage": false,
        "target_compliance": "unqualified",
        "source_authentication_exported": false,
        "artifact_or_launch_authority": false
    });
    let text = serde_json::to_string(&value).expect("fixed observation JSON");
    assert!(text.len() <= 8192);
    println!("\n{PREFIX}{text}");
    assert!(
        attempt.failure.is_none(),
        "actual source measurement failed: {:?}",
        attempt.failure
    );
}

#[path = "ordered_program_stage_measurement_controls_v1.rs"]
mod controls;

#[path = "ordered_program_stage_operational_v1_tests.rs"]
mod operational;
