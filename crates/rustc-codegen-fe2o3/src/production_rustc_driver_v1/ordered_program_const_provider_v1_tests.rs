//! Task-private CTFE/checking observation, reachable only through the enclosing
//! cfg(test, Linux) stage module. One fresh process/session; no warm claim.
//! The selected query provider is delegated exactly once with the unchanged
//! key and its original Result. No body, result, query cache or compiler flag
//! is rewritten. Attribution and JSON work happen after recording is fenced.
use super::{
    Callbacks, Compilation, Compiler, Failure, Input, MAX_CONFIG, Observation, Profile,
    RetainedBytes, TyCtxt, digest, observe_live_owner, request_ok,
    transaction_in_active_session_v1,
};
use rustc_hir::def_id::DefId;
use rustc_middle::mir::interpret::{EvalToAllocationRawResult, GlobalId};
use rustc_middle::ty::{self, InstanceKind, TypingEnv};
use rustc_middle::util::Providers;
use rustc_session::Session;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

#[path = "ordered_program_const_provider_attribution_v1_tests.rs"]
mod attribution;
#[path = "ordered_program_const_provider_controls_v1_tests.rs"]
mod controls;
#[path = "ordered_program_const_provider_ledger_v1_tests.rs"]
mod ledger;

const INPUT_ENV: &str = "FE2O3_ORDERED_CONST_PROVIDER_V1_CONFIG";
const PREFIX: &str = "fe2o3 ordered-const provider v1: ";
const OUTPUT_CAP: usize = 131_072;
type Provider = for<'tcx> fn(
    TyCtxt<'tcx>,
    ty::PseudoCanonicalInput<'tcx, GlobalId<'tcx>>,
) -> EvalToAllocationRawResult<'tcx>;
type Override = fn(&Session, &mut Providers);

static CLAIMED: AtomicBool = AtomicBool::new(false);
static PREVIOUS: OnceLock<Option<Override>> = OnceLock::new();
static ORIGINAL: OnceLock<Provider> = OnceLock::new();
static LEDGER: Mutex<ledger::Ledger> = Mutex::new(ledger::Ledger::new());

fn locked() -> MutexGuard<'static, ledger::Ledger> {
    match LEDGER.lock() {
        Ok(value) => value,
        Err(poisoned) => {
            let mut value = poisoned.into_inner();
            value.fail();
            value
        }
    }
}
fn hex32(bytes: &[u8; 32]) -> String {
    super::super::lower_hex_v1(bytes)
}
fn fence() -> ledger::Snapshot {
    let mut ledger = locked();
    ledger.fence();
    ledger.snapshot()
}
fn chain_then<P, T>(
    providers: &mut P,
    prior: impl FnOnce(&mut P),
    select: impl FnOnce(&mut P) -> T,
) -> T {
    prior(providers);
    select(providers)
}
fn install(session: &Session, providers: &mut Providers) {
    // Preserve the prior override's order and exactly one invocation. Capture
    // its selected pointer, never a hard-coded default compiler provider.
    let selected = chain_then(
        providers,
        |providers| {
            if let Some(previous) = PREVIOUS.get().copied().flatten() {
                previous(session, providers);
            }
        },
        |providers| providers.queries.eval_to_allocation_raw,
    );
    if PREVIOUS.get().is_none()
        || std::ptr::fn_addr_eq(selected, delegate as Provider)
        || ORIGINAL.get().is_some()
    {
        locked().fail();
        return; // Leave the prior override's selected provider untouched.
    }
    if ORIGINAL.set(selected).is_err() {
        locked().fail();
        return;
    }
    if !locked().arm(session as *const Session as usize, Instant::now()) {
        return;
    }
    // Publication follows ORIGINAL initialization; delegate cannot be called
    // by the compiler with an absent saved pointer.
    providers.queries.eval_to_allocation_raw = delegate;
}

struct Flight {
    ordinal: Option<usize>,
    start: Instant,
    completed: bool,
}
impl Flight {
    fn finish(mut self, end: Instant, returned_ok: bool) {
        if let Some(ordinal) = self.ordinal {
            locked().complete(ordinal, self.start, end, Some(returned_ok));
        }
        self.completed = true;
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        if !self.completed
            && let Some(ordinal) = self.ordinal
        {
            // The provider's panic still unwinds unchanged. A recoverable
            // parent may retain this partial row; abort/kill can lose telemetry.
            locked().complete(ordinal, self.start, Instant::now(), None);
        }
    }
}
fn call_once<T>(original: impl FnOnce() -> T) -> (T, Instant, Instant) {
    let start = Instant::now();
    let result = original();
    let end = Instant::now();
    (result, start, end)
}
fn delegate<'tcx>(
    tcx: TyCtxt<'tcx>,
    key: ty::PseudoCanonicalInput<'tcx, GlobalId<'tcx>>,
) -> EvalToAllocationRawResult<'tcx> {
    // This invariant is established before the only publication above. No
    // fallible observer admission lies between the original call and return.
    let original = *ORIGINAL
        .get()
        .expect("delegate published after original provider");
    if !key.value.instance.def_id().is_local() {
        return original(tcx, key);
    }
    let observation_key = ledger::Key {
        definition: key.value.instance.def_id(),
        item: matches!(key.value.instance.def, InstanceKind::Item(_)),
        empty_args: key.value.instance.args.is_empty(),
        no_promoted: key.value.promoted.is_none(),
        fully_monomorphized: key.typing_env == TypingEnv::fully_monomorphized(),
    };
    let ordinal = locked().begin(
        observation_key,
        tcx.sess as *const Session as usize,
        std::thread::current().id(),
    );
    if ordinal.is_none() {
        return original(tcx, key);
    }
    // Reserve the bounded row before taking the provider clock. The saved
    // provider is always called, including cap/thread/session refusals.
    let mut flight = Flight {
        ordinal,
        start: Instant::now(),
        completed: false,
    };
    let (result, start, end) = call_once(|| original(tcx, key));
    // The call_once start is the successful interval's actual boundary.
    // Flight's immediately preceding clock only retains an honest partial
    // envelope if unwinding prevents that successful return.
    flight.start = start;
    flight.finish(end, result.is_ok());
    result
}

#[derive(Serialize)]
struct FailureRecord {
    stage: Option<Failure>,
    refusal: Option<&'static str>,
}
impl FailureRecord {
    fn stage(stage: Failure) -> Self {
        Self {
            stage: Some(stage),
            refusal: None,
        }
    }
    fn refused(refusal: &'static str) -> Self {
        Self {
            stage: None,
            refusal: Some(refusal),
        }
    }
}
type Observed = Result<(Observation, attribution::Attribution), FailureRecord>;
struct ProviderCallbacks<'a> {
    profile: Profile,
    baseline: &'a [u8],
    config_calls: u8,
    calls: u8,
    configured: bool,
    observed: Option<Observed>,
}
impl Callbacks for ProviderCallbacks<'_> {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        self.config_calls = self.config_calls.saturating_add(1).min(2);
        if self.config_calls != 1 || PREVIOUS.set(config.override_queries).is_err() {
            locked().fail();
            return;
        }
        self.configured = true;
        config.override_queries = Some(install);
    }
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls = self.calls.saturating_add(1).min(2);
        if self.calls != 1 {
            fence();
            self.observed = Some(Err(FailureRecord::stage(Failure::RepeatedCallback)));
            return Compilation::Stop;
        }
        // Original transaction and consuming validation transition, exactly
        // once. This does not invoke a substitute runtime packing helper.
        let owner = transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
        .map_err(|_| FailureRecord::stage(Failure::SourceCollection))
        .and_then(|transaction| {
            transaction
                .observe_ordered_program_v32()
                .map_err(|_| FailureRecord::stage(Failure::Validation))
        });
        let snapshot = fence();
        self.observed = Some(match owner {
            Err(error) => Err(error),
            Ok(owner) => {
                // Complete independent baseline equality is checked while the
                // actual original owner is live, outside the provider clocks.
                observe_live_owner(&owner, self.profile, self.baseline)
                    .map_err(FailureRecord::stage)
                    .and_then(|observation| {
                        let semantic = owner.materialized().semantic_ssa().source_semantic();
                        let root = semantic.roots()[0];
                        let function = &semantic.functions()[root.index() as usize];
                        let identity = *function.identity().as_bytes();
                        attribution::capture(tcx, snapshot, identity)
                            .map(|attribution| (observation, attribution))
                            .map_err(FailureRecord::refused)
                    })
            }
        });
        Compilation::Stop
    }
}
fn final_observation(
    observed: Option<Observed>,
    config_calls: u8,
    configured: bool,
    calls: u8,
    fatal: bool,
    panicked: bool,
    snapshot: ledger::Snapshot,
) -> Observed {
    if panicked {
        return Err(FailureRecord::refused(
            "compiler unwound; no qualified sample",
        ));
    }
    if fatal {
        return Err(FailureRecord::stage(Failure::CompilerFatal));
    }
    if config_calls != 1 || !configured {
        return Err(FailureRecord::refused(
            "one configured compiler session required",
        ));
    }
    if calls > 1 {
        return Err(FailureRecord::stage(Failure::RepeatedCallback));
    }
    if calls != 1 {
        return Err(FailureRecord::stage(Failure::MissingCallback));
    }
    if !snapshot.ready() {
        return Err(FailureRecord::refused(
            "provider ledger incomplete or refused",
        ));
    }
    observed.unwrap_or(Err(FailureRecord::stage(Failure::MissingCallback)))
}

#[test]
#[ignore = "one root-owned fresh process; genuine flat1/3/16 source and independent full baseline required"]
fn actual_const_provider_probe() {
    let path =
        PathBuf::from(std::env::var_os(INPUT_ENV).expect("root must select exact const config"));
    let mut config = RetainedBytes::open(&path, MAX_CONFIG).expect("retained const config");
    let input: Input = serde_json::from_slice(&config.bytes).expect("closed const config");
    assert_eq!(input.schema, "fe2o3-ordered-const-provider-input-v1");
    let profile = Profile::from_steps(input.steps).expect("flat1/3/16 profile");
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
            .expect("retained full baseline");
    assert_eq!(baseline.bytes.len(), input.baseline_bytes);
    assert_eq!(digest(&baseline.bytes), input.baseline_sha256);
    assert!(request_ok(&input.rustc_args, &baseline.bytes));
    // Parent launches only this exact ignored test in a fresh process. No
    // retries, query-cache resets or repeated compiler sessions in this child.
    assert!(
        !CLAIMED.swap(true, Ordering::SeqCst),
        "one compiler session per process"
    );
    let mut callbacks = ProviderCallbacks {
        profile,
        baseline: &baseline.bytes,
        config_calls: 0,
        calls: 0,
        configured: false,
        observed: None,
    };
    let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::catch_fatal_errors(|| {
            rustc_driver::run_compiler(&input.rustc_args, &mut callbacks)
        })
    }));
    let snapshot = fence();
    let panicked = execution.is_err();
    let fatal = matches!(&execution, Ok(Err(_)));
    let config_calls = callbacks.config_calls;
    let calls = callbacks.calls;
    let result = final_observation(
        callbacks.observed.take(),
        config_calls,
        callbacks.configured,
        calls,
        fatal,
        panicked,
        snapshot,
    );
    drop(callbacks); // End the borrowed baseline before its full custody recheck.
    // Preserve completed and incomplete provider rows even when a handled
    // compiler/callback/attribution failure prevents a selected sample.
    let rows: Vec<_> = snapshot.rows[..snapshot.count]
        .iter()
        .enumerate()
        .map(|(ordinal, row)| row.map(|row| row.wire(ordinal)))
        .collect();
    let post_inputs_unchanged = config.recheck().is_ok() && baseline.recheck().is_ok();
    let (observation, attribution, failure) = match result {
        Ok((observation, attribution)) => (Some(observation), Some(attribution), None),
        Err(error) => (None, None, Some(error)),
    };
    let success = post_inputs_unchanged && failure.is_none();
    let value = serde_json::json!({
        "schema": "fe2o3-ordered-const-provider-v1",
        "compiler_invocations": 1,
        "config_calls": config_calls,
        "after_analysis_calls": calls,
        "provider_installed": snapshot.installed,
        "provider_ledger_failed": snapshot.failed,
        "provider_recording_fenced": snapshot.fenced,
        "active_provider_depth": snapshot.active_depth,
        "rows": rows,
        "observation": observation,
        "attribution": attribution,
        "failure": failure,
        "compiler_fatal": fatal,
        "compiler_panicked": panicked,
        "post_inputs_unchanged": post_inputs_unchanged,
        "config_sha256": digest(&config.bytes),
        "baseline_sha256": digest(&baseline.bytes),
        "success": success,
        "provider_row_cap": ledger::ROW_CAP,
        "provider_depth_cap": ledger::DEPTH_CAP,
        "metric": "one actual inclusive allocation-provider CTFE-and-checking invocation",
        "nested_provider_observer_overhead_included": true,
        "all_nested_queries_uncached": null,
        "query_cache_hit_time": null,
        "pure_helper_interpreter_loop_ns": null,
        "warm_generation": false,
        "generation_target_compliance": "unqualified",
        "complete_owner_or_peak_bytes": null,
        "source_authentication_exported": false,
        "artifact_or_launch_authority": false
    });
    let text = serde_json::to_string(&value).expect("bounded provider observation JSON");
    assert!(
        text.len()
            .checked_add(PREFIX.len())
            .and_then(|n| n.checked_add(2))
            .is_some_and(|n| n <= OUTPUT_CAP),
        "whole const observation line bound"
    );
    println!("\n{PREFIX}{text}");
    // Never turn an original panic into a successful compiler return. If
    // publication itself fails, the parent must retain the failed attempt.
    if let Err(payload) = execution {
        std::panic::resume_unwind(payload);
    }
    assert!(
        success,
        "const provider observation refused; retain raw failure"
    );
}
