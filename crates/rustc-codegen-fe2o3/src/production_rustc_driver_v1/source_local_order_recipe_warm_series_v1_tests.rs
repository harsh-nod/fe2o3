//! Ignored, root-qualified operational child. Never selected by ordinary drivers.
//! One invocation selects ordinary/measured/35-call warm mode. All output is
//! bounded inert observation; no output files, owner export or child processes.
use super::*;
use crate::source_local_order_recipe_api_v1 as api;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[path = "source_local_order_recipe_warm_oracle_v1_tests.rs"]
mod oracle;
use oracle::*;

const SCHEMA: &str = "fe2o3-recipe-series-input-v1";
const CONFIG_LIMIT: usize = 2 * 1024 * 1024;
const TARGET_NS: u64 = 500_000_000;
const CALLS: usize = 35;
const CONFIG_ENV: &str = "FE2O3_RECIPE_SERIES_CONFIG";
const HASH_ENV: &str = "FE2O3_RECIPE_SERIES_CONFIG_SHA256";

#[derive(Clone, Copy, Debug, Serialize)]
struct SeriesEnvelope {
    calls: usize,
    maximum_selected_retained_source_read_bytes: usize,
    maximum_selected_recipe_read_bytes: usize,
    maximum_additional_capture_path_read_bytes: usize,
}
fn checked_envelope(calls: usize, replay: bool) -> Result<SeriesEnvelope, String> {
    let total = |per_call: usize| {
        calls
            .checked_mul(per_call)
            .ok_or_else(|| "aggregate reservation overflow".to_owned())
    };
    Ok(SeriesEnvelope {
        calls,
        maximum_selected_retained_source_read_bytes: total(3 * 1_048_577)?,
        maximum_selected_recipe_read_bytes: total(if replay { 3 * 8193 } else { 0 })?,
        maximum_additional_capture_path_read_bytes: total(65_537)?,
    })
}
struct Reservations {
    envelope: SeriesEnvelope,
    remaining: usize,
}
impl Reservations {
    fn new(replay: bool) -> Result<Self, String> {
        Ok(Self {
            envelope: checked_envelope(CALLS, replay)?,
            remaining: CALLS,
        })
    }
    fn next(&mut self) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("aggregate call reservations exhausted")?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Ordinary,
    Measured,
    Warm,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedFile {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Intent {
    Create {
        order: String,
        relation: String,
        strength: String,
        binding: String,
    },
    Replay {
        recipe: PinnedFile,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    mode: Mode,
    source: String,
    source_sha256: String,
    intent: Intent,
    rustc_args: Vec<String>,
    oracle: Option<PinnedFile>,
}

fn hash(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected lowercase64-bytehex SHA256".into());
    }
    let mut out = [0_u8; 32];
    let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        out[index] = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(out)
}
fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(TABLE[(byte >> 4) as usize] as char);
        text.push(TABLE[(byte & 15) as usize] as char);
    }
    text
}
fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn diagnostic(mut text: String) -> String {
    if text.len() > 512 {
        let mut end = 512;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
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
struct RetainedBytes {
    path: PathBuf,
    file: File,
    snapshot: Metadata,
    bytes: Vec<u8>,
    cap: usize,
    reads: usize,
}
impl RetainedBytes {
    // Exact shipping example descriptor/byte-currentness semantics. The helper
    // stores no legacy decoded recipe. The current Request admits bytes itself.
    fn open(pin: &PinnedFile, cap: usize) -> Result<Self, String> {
        let expected = hash(&pin.sha256)?;
        let path = PathBuf::from(&pin.path);
        let parent = path.parent().ok_or("input parent absent")?;
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.file_name().is_none()
            || fs::canonicalize(parent).map_err(|e| e.to_string())? != parent
        {
            return Err("bounded canonical input parent required".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let snapshot = file.metadata().map_err(|e| e.to_string())?;
        if !same(
            &snapshot,
            &fs::symlink_metadata(&path).map_err(|e| e.to_string())?,
        ) || snapshot.len() == 0
            || snapshot.len() > cap as u64
        {
            return Err("one bounded regular retained input required".into());
        }
        let mut retained = Self {
            path,
            file,
            snapshot,
            bytes: Vec::new(),
            cap,
            reads: 0,
        };
        retained.bytes = retained.read()?;
        if <[u8; 32]>::from(Sha256::digest(&retained.bytes)) != expected {
            return Err("retained input expected SHA256 differs".into());
        }
        retained.recheck()?;
        Ok(retained)
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {
        if self.reads >= 3 {
            return Err("per-input three-read observer envelope".into());
        }
        self.reads += 1;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        read_payload(&mut self.file, self.cap)
    }
    fn recheck(&mut self) -> Result<(), String> {
        if !same(
            &self.snapshot,
            &self.file.metadata().map_err(|e| e.to_string())?,
        ) || !same(
            &self.snapshot,
            &fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?,
        ) {
            return Err("retained input changed".into());
        }
        let bytes = self.read()?;
        if bytes != self.bytes
            || !same(
                &self.snapshot,
                &self.file.metadata().map_err(|e| e.to_string())?,
            )
            || !same(
                &self.snapshot,
                &fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?,
            )
        {
            return Err("retained input changed".into());
        }
        Ok(())
    }
}
fn read_payload(reader: &mut impl Read, cap: usize) -> Result<Vec<u8>, String> {
    let limit = cap.checked_add(1).ok_or("read bound overflow")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limit)
        .map_err(|_| "bounded input allocation")?;
    let mut chunk = [0_u8; 1024];
    loop {
        let remaining = limit
            .checked_sub(bytes.len())
            .ok_or("read bound overflow")?;
        let count = reader
            .read(&mut chunk[..remaining.min(1024)])
            .map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if bytes.len().checked_add(count).is_none_or(|n| n > cap) {
            return Err("input byte envelope exceeded".into());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    if bytes.is_empty() {
        return Err("empty retained input".into());
    }
    Ok(bytes)
}
fn prepare_request(config: &Config) -> Result<(Request, Option<RetainedBytes>), String> {
    let expected = hash(&config.source_sha256)?;
    match &config.intent {
        Intent::Replay { recipe } => {
            let retained = RetainedBytes::open(recipe, 8192)
                .map_err(|e| format!("retained_recipe_admission: {e}"))?;
            let request = Request::replay(&config.source, expected, &retained.bytes)
                .map_err(|e| format!("request_admission/{:?}: {}", e.phase(), e.diagnostic()))?;
            Ok((request, Some(retained)))
        }
        Intent::Create {
            order,
            relation,
            strength,
            binding,
        } => {
            let order = match order.as_str() {
                "source_order" => api::SourceLocalOrderOrderV1::SourceOrder,
                "reverse_ready" => api::SourceLocalOrderOrderV1::ReverseReady,
                _ => return Err("closed order".into()),
            };
            let relation = match relation.as_str() {
                "xor_before_or" => api::SourceLocalOrderRelationV1::XorBeforeOr,
                "or_before_xor" => api::SourceLocalOrderRelationV1::OrBeforeXor,
                _ => return Err("closed relation".into()),
            };
            let strength = match strength.as_str() {
                "exact" => api::SourceLocalOrderStrengthV1::Exact,
                "advisory" => api::SourceLocalOrderStrengthV1::Advisory,
                _ => return Err("closed strength".into()),
            };
            let binding = match binding.as_str() {
                "exact_revision" => api::SourceLocalOrderSourceBindingModeV1::ExactRevision,
                "rebind_current" => api::SourceLocalOrderSourceBindingModeV1::RebindCurrent,
                _ => return Err("closed binding".into()),
            };
            let request =
                Request::create(&config.source, expected, order, relation, strength, binding)
                    .map_err(|e| {
                        format!("request_admission/{:?}: {}", e.phase(), e.diagnostic())
                    })?;
            Ok((request, None))
        }
    }
}
struct Prepared {
    request: Request,
    input: RetainedInput,
    recipe: Option<RetainedBytes>,
}
fn prepare(config: &Config) -> Result<Prepared, String> {
    let (request, recipe) = prepare_request(config)?;
    let input = RetainedInput::open(request.source_path(), false)
        .map_err(|e| format!("retained_source_admission: {e}"))?;
    Ok(Prepared {
        request,
        input,
        recipe,
    })
}
fn recheck(recipe: &mut Option<RetainedBytes>) -> Result<(), String> {
    if let Some(recipe) = recipe {
        recipe.recheck()?;
    }
    Ok(())
}
fn compare(bytes: &[u8], expected: Option<&[u8]>) -> Result<bool, String> {
    match expected {
        Some(expected) if expected == bytes => Ok(true),
        Some(_) => Err("full normal LLVM/recipe/evidence byte oracle differs".into()),
        None => Ok(false),
    }
}
fn single(config: &Config, expected: Option<&[u8]>) -> Result<(), String> {
    let (request, mut recipe) = prepare_request(config)?;
    let attempt = match config.mode {
        Mode::Ordinary => run_source_local_order_recipe_driver_v1(&config.rustc_args, request),
        Mode::Measured => {
            run_source_local_order_recipe_driver_measured_v1(&config.rustc_args, request)
        }
        Mode::Warm => return Err("single mode cannot run warm".into()),
    };
    let elapsed = attempt
        .callback_stage_elapsed_v1()
        .map(|d| d.as_nanos().to_string());
    emit(
        &serde_json::json!({"kind":"attempt","mode":config.mode,
            "callback_count":attempt.callback_count(),"compiler_callback_count":attempt.compiler_callback_count(),
            "callback_stage_elapsed_ns_decimal":elapsed,"warm":false,
            "result_phase":attempt.result().err().map(|e|format!("{:?}",e.phase())),
            "compiler_fatal":attempt.result().err().is_some_and(|e|e.compiler_fatal()),
            "diagnostic":attempt.result().err().map(|e|diagnostic(e.diagnostic().to_owned()))
        }),
        SUMMARY_LIMIT,
    )?;
    let output = attempt.result().map_err(|e| e.to_string())?;
    let bytes = normal(output)?;
    recheck(&mut recipe)?;
    let equal = compare(&bytes, expected)?;
    emit_normal(&bytes)?;
    emit(
        &serde_json::json!({"kind":"complete","mode":config.mode,
            "normal_bytes":bytes.len(),"normal_sha256":sha(&bytes),
            "external_oracle_compared":expected.is_some(),"exact_external_oracle_equal":equal,
            "warm_statistics":null,"retained_logical_bytes":null,"grants_authority":false
        }),
        SUMMARY_LIMIT,
    )
}
struct WarmCallbacks<'a> {
    config: &'a Config,
    expected: &'a [u8],
    first: Option<Prepared>,
    entries: usize,
    samples: Vec<Sample>,
    result: Option<Result<(), String>>,
    current_ordinal: Option<usize>,
    current_elapsed: Option<u128>,
    current_phase: Option<Phase>,
    reservations: Reservations,
}
impl WarmCallbacks<'_> {
    fn series<'tcx>(&mut self, tcx: TyCtxt<'tcx>) -> Result<(), String> {
        for index in 0..CALLS {
            self.current_ordinal = Some(index + 1);
            self.current_elapsed = None;
            self.current_phase = None;
            let Prepared {
                request,
                input,
                mut recipe,
            } = if index == 0 {
                self.first
                    .take()
                    .ok_or("missing original pre-frontend admission")?
            } else {
                self.reservations.next()?;
                prepare(self.config)?
            };
            // Original pipeline only. No live CPU observer or storage walker is
            // installed. Every request/input/transaction is fresh, then consumed.
            let started = Instant::now();
            let outcome = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|message| Failure::new(Phase::Frontend, message))
            .and_then(|transaction| {
                transaction.compile_source_local_order_recipe_v1(input, &request)
            });
            let elapsed = started.elapsed().as_nanos();
            self.current_elapsed = Some(elapsed);
            let elapsed_ns = u64::try_from(elapsed).map_err(|_| "elapsed nanoseconds overflow")?;
            let output = match outcome {
                Ok(value) => value,
                Err(error) => {
                    self.current_phase = Some(error.phase());
                    return Err(format!(
                        "original_recipe/{:?}: {}",
                        error.phase(),
                        error.diagnostic()
                    ));
                }
            };
            let bytes = normal(&output)?;
            drop(output);
            recheck(&mut recipe)?;
            let equal = compare(&bytes, Some(self.expected))?;
            drop((request, recipe));
            let sample = Sample {
                ordinal: index + 1,
                calibration: index < 5,
                elapsed_ns,
                normal_bytes: bytes.len(),
                normal_sha256: sha(&bytes),
                exact_external_oracle_equal: equal,
            };
            // Flush each obtained sample. A later abort can lose current work,
            // but parent keeps original completed prefix and terminal failure.
            emit(
                &serde_json::json!({"kind":"sample","sample":&sample}),
                ROW_LIMIT,
            )?;
            self.samples.push(sample);
            if index == 0 {
                emit_normal(&bytes)?;
            }
            // bytes drop here; no actual output owner survives into next call.
        }
        Ok(())
    }
}
impl Callbacks for WarmCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.entries = self.entries.saturating_add(1);
        if self.entries != 1 {
            self.result = Some(Err("repeated genuine compiler callback".into()));
            return Compilation::Stop;
        }
        self.result = Some(self.series(tcx));
        Compilation::Stop
    }
}
fn warm(config: &Config, expected: &[u8]) -> Result<(), String> {
    let mut reservations = Reservations::new(matches!(&config.intent, Intent::Replay { .. }))?;
    // Exactly35 upfront call reservations. These are bounded selected-input
    // traffic envelopes, NOT retained128MiB memory measurements or total rustc I/O.
    emit(
        &serde_json::json!({"kind":"plan","calls":35,"calibration":5,"measured":30,
            "fresh_request_admissions":35,"fresh_retained_source_inputs":35,
            "upfront_selected_input_reservations":reservations.envelope,
            "retained_logical_bytes":null,"storage_walks_in_timer":false,
            "cfg_test_build":true,"source_wire_test_branch_applicability":"root_must_bind"
        }),
        SUMMARY_LIMIT,
    )?;
    reservations.next()?;
    let first = prepare(config)?;
    let mut callbacks = WarmCallbacks {
        config,
        expected,
        first: Some(first),
        entries: 0,
        samples: Vec::with_capacity(CALLS),
        result: None,
        current_ordinal: None,
        current_elapsed: None,
        current_phase: None,
        reservations,
    };
    let fatal = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(&config.rustc_args, &mut callbacks)
    })
    .is_err();
    let result = callbacks
        .result
        .take()
        .unwrap_or_else(|| Err("compiler did not complete callback".into()));
    if fatal || callbacks.entries != 1 || result.is_err() || callbacks.reservations.remaining != 0 {
        emit(
            &serde_json::json!({"kind":"incomplete","compiler_fatal":fatal,"callback_count":callbacks.entries,
                "completed_samples":callbacks.samples.len(),"remaining_call_reservations":callbacks.reservations.remaining,
                "current_ordinal":callbacks.current_ordinal,
                "current_elapsed_ns_decimal":callbacks.current_elapsed.map(|v|v.to_string()),
                "current_original_phase":callbacks.current_phase.map(|v|format!("{v:?}")),
                "diagnostic":result.as_ref().err().map(|e|diagnostic(e.clone())),
                "warm_statistics":null,"p95_within_target":null,"retained_logical_bytes":null
            }),
            SUMMARY_LIMIT,
        )?;
        return Err("incomplete genuine warm series; no budget result".into());
    }
    let measured = stats(&callbacks.samples)?;
    emit(
        &serde_json::json!({"kind":"complete","mode":"warm","callback_count":callbacks.entries,
            "raw_samples":callbacks.samples.len(),"calibration":5,"measured":30,
            "warm_statistics":measured,"percentile_method":"nearest_rank_30_rank15_rank29_rank30",
            "target_ns":TARGET_NS,"p95_within_target":measured.p95_ns<=TARGET_NS,
            "scope":"same_active_tcx_fresh_transaction_creation_through_original_consuming_recipe_return",
            "external_oracle_sha256":sha(expected),"all35_exact_external_oracle_equal":true,
            "frontend_reused":true,"admitted_owner_reused":false,
            "companion_serialization_hash_equality_in_timer":false,
            "api_internal_checks_in_timer":true,"retained_logical_bytes":null,
            "peak_heap_measured":false,"rss_measured":false,"grants_authority":false
        }),
        SUMMARY_LIMIT,
    )
}
fn child() -> Result<(), String> {
    let path = std::env::var(CONFIG_ENV).map_err(|_| "missing pinned config path")?;
    let digest = std::env::var(HASH_ENV).map_err(|_| "missing pinned config SHA256")?;
    let guard = RetainedBytes::open(
        &PinnedFile {
            path,
            sha256: digest,
        },
        CONFIG_LIMIT,
    )?;
    let config: Config = serde_json::from_slice(&guard.bytes).map_err(|e| e.to_string())?;
    if config.schema != SCHEMA {
        return Err("config schema differs".into());
    }
    validate_arguments(&config.rustc_args).map_err(|e| e.to_string())?;
    require_canonical_overflow_checks_v1(&config.rustc_args)?;
    hash(&config.source_sha256)?;
    let oracle = match &config.oracle {
        Some(pin) => Some(RetainedBytes::open(pin, NORMAL_LIMIT)?.bytes),
        None => None,
    };
    if config.mode != Mode::Ordinary && oracle.is_none() {
        return Err("measured/warm mode requires independent ordinary oracle".into());
    }
    drop(guard); // immutable decoded config/expected oracle remain; no file reload.
    match config.mode {
        Mode::Ordinary | Mode::Measured => single(&config, oracle.as_deref()),
        Mode::Warm => warm(&config, oracle.as_deref().ok_or("missing warm oracle")?),
    }
}
#[test]
#[ignore = "requires root-pinned genuine source/recipe/rustc config, baseline oracle and bounded owned process interval"]
fn actual_source_local_order_recipe_series_child_v1() {
    if let Err(error) = child() {
        let _ = emit(
            &serde_json::json!({"kind":"child_refused","diagnostic":diagnostic(error),
            "warm_statistics":null,"p95_within_target":null}),
            SUMMARY_LIMIT,
        );
        panic!("genuine recipe series refused; retain original bounded streams");
    }
}
#[path = "source_local_order_recipe_warm_controls_v1_tests.rs"]
mod controls;

#[path = "source_local_order_recipe_outcome_series_v1_tests.rs"]
mod outcome_series;

#[path = "source_local_order_recipe_cancellation_v1_tests.rs"]
mod cancellation_controls;
