//! Root-qualified genuine original pipeline controls; one child = one request.
//! Reuses the unchanged warm-series pinned config/recipe readers and serializer.
//! Not a latency, retained-owner heap or intra-phase responsiveness benchmark.
use super::*;
use crate::source_local_order_recipe_api_v1::SourceLocalOrderRecipeCancellationV1 as Token;
use crate::source_local_order_recipe_api_v1::cancellation::{Checkpoint, Hook};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering as AtomicOrdering},
};

const SELECTOR: &str = "FE2O3_RECIPE_CANCELLATION_CHECKPOINT";
const CHECKPOINTS: [Checkpoint; 20] = [
    Checkpoint::BeforeInput,
    Checkpoint::BeforeCompiler,
    Checkpoint::AfterAnalysis,
    Checkpoint::AfterTransaction,
    Checkpoint::BeforeCapture,
    Checkpoint::AfterCapture,
    Checkpoint::AfterImport,
    Checkpoint::AfterMiddleEnd,
    Checkpoint::AfterSsa,
    Checkpoint::AfterMaterialization,
    Checkpoint::AfterRankedVerification,
    Checkpoint::BeforePrefix,
    Checkpoint::AfterPrefix,
    Checkpoint::AfterSourceJoin,
    Checkpoint::BeforeContinuation,
    Checkpoint::AfterContinuation,
    Checkpoint::BeforeReplay,
    Checkpoint::AfterReplay,
    Checkpoint::AfterFinalCurrentness,
    Checkpoint::DriverCommit,
];
fn selected(text: &str) -> Result<Option<Checkpoint>, String> {
    if text == "never" {
        return Ok(None);
    }
    CHECKPOINTS
        .iter()
        .copied()
        .find(|p| format!("{p:?}") == text)
        .map(Some)
        .ok_or_else(|| "closed cancellation checkpoint".into())
}
struct DropWitness(Arc<AtomicBool>);
impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.store(true, AtomicOrdering::SeqCst);
    }
}
#[test]
fn closed_checkpoint_roster_and_no_intra_phase_selector() {
    for point in CHECKPOINTS {
        assert_eq!(selected(&format!("{point:?}")).unwrap(), Some(point));
    }
    assert_eq!(selected("never").unwrap(), None);
    for bad in [
        "",
        "materialization_loop",
        "SIGTERM",
        "WorkLimit",
        "AfterReplay ",
    ] {
        assert!(selected(bad).is_err());
    }
}
#[test]
#[ignore = "requires root-pinned genuine original config and exact independent normal oracle; one bounded child per checkpoint and Create/Replay"]
fn actual_source_local_order_recipe_cancellation_child_v1() {
    if let Err(error) = child() {
        panic!("genuine cancellation qualification refused: {error}");
    }
}
fn child() -> Result<(), String> {
    let selector = std::env::var(SELECTOR).map_err(|_| "missing cancellation selector")?;
    let special = matches!(
        selector.as_str(),
        "observer-refusal" | "observer-cancel" | "observer-fatal" | "repeat-cancel"
    );
    let target = if special {
        if selector == "repeat-cancel" {
            Some(Checkpoint::AfterReplay)
        } else {
            None
        }
    } else {
        selected(&selector)?
    };
    let guard = RetainedBytes::open(
        &PinnedFile {
            path: std::env::var(CONFIG_ENV).map_err(|_| "missing original pinned config")?,
            sha256: std::env::var(HASH_ENV).map_err(|_| "missing original config hash")?,
        },
        CONFIG_LIMIT,
    )?;
    let config: Config = serde_json::from_slice(&guard.bytes).map_err(|e| e.to_string())?;
    if config.schema != SCHEMA || config.mode != Mode::Ordinary {
        return Err("cancellation control requires original ordinary config".into());
    }
    validate_arguments(&config.rustc_args).map_err(|e| e.to_string())?;
    require_canonical_overflow_checks_v1(&config.rustc_args)?;
    let oracle = RetainedBytes::open(
        config
            .oracle
            .as_ref()
            .ok_or("independent ordinary oracle required")?,
        NORMAL_LIMIT,
    )?;
    let expected = oracle.bytes.clone(); // test-owned bounded oracle, never a compiler owner.
    let (request, mut recipe) = prepare_request(&config)?;
    let source = request.source_path().to_owned();
    let expected_source = *request.expected_current_source_sha256();
    let expected_recipe = request.replay_recipe_bytes().map(<[u8]>::to_vec);
    let created = request.is_create();
    let token = Arc::new(Token::new());
    let visited = Mutex::new(Vec::with_capacity(CHECKPOINTS.len()));
    let hook = |point: Checkpoint, token: &Token| {
        let mut trace = visited.lock().unwrap();
        assert!(trace.len() < CHECKPOINTS.len(), "bounded checkpoint census");
        trace.push(point);
        if target == Some(point) {
            token.request_cancellation();
        }
    };
    let dropped = Arc::new(AtomicBool::new(false));
    let witness = DropWitness(dropped.clone());
    let observer_token = token.clone();
    let observer_case = selector.clone();
    let observer: api::test_support::Observer = Box::new(move |_, _| {
        let _witness = witness;
        if matches!(
            observer_case.as_str(),
            "observer-refusal" | "observer-cancel" | "observer-fatal"
        ) {
            observer_token.request_cancellation();
        }
        if matches!(
            observer_case.as_str(),
            "observer-refusal" | "observer-fatal"
        ) {
            return Err("original observer refusal wins later cancellation".into());
        }
        Ok(())
    });
    let gate = token
        .claim()
        .map_err(|e| e.to_string())?
        .with_hook(&hook as &Hook<'_>);
    let attempt = if let Err(error) = gate.poll(Checkpoint::BeforeInput) {
        drop(observer);
        Attempt::new(request, Err(error), 0, 0)
    } else {
        let mut callbacks = RecipeCallbacks::new(request);
        callbacks.cancellation = Some(&gate);
        callbacks.observer = Some(observer);
        callbacks.repeat_after_first = selector == "repeat-cancel";
        callbacks.fatal_after_first = selector == "observer-fatal";
        run(&config.rustc_args, callbacks)
    };
    let attempt = gate.finish_attempt(attempt);
    assert!(
        dropped.load(AtomicOrdering::SeqCst),
        "test observer dropped on every terminal route"
    );
    assert_eq!(attempt.request().source_path(), source);
    assert_eq!(
        *attempt.request().expected_current_source_sha256(),
        expected_source
    );
    assert_eq!(
        attempt.request().replay_recipe_bytes(),
        expected_recipe.as_deref()
    );
    assert_eq!(attempt.request().is_create(), created);
    assert!(attempt.callback_stage_elapsed_v1().is_none());
    let trace = visited.lock().unwrap();
    if selector == "never" {
        let output = attempt
            .result()
            .map_err(|e| format!("never-cancel failed: {e}"))?;
        assert!(compare(&normal(output)?, Some(&expected))?);
        assert_eq!(trace.as_slice(), CHECKPOINTS);
        assert_eq!(attempt.callback_count(), 1);
        assert_eq!(attempt.compiler_callback_count(), 1);
    } else if matches!(selector.as_str(), "observer-refusal" | "observer-fatal") {
        let error = attempt.result().expect_err("original observer must refuse");
        assert_eq!(error.phase(), Phase::Observation);
        assert_eq!(
            error.diagnostic(),
            "original observer refusal wins later cancellation"
        );
        assert!(
            error.cancellation().is_none(),
            "first non-cancellation refusal remains first"
        );
        assert_eq!(error.compiler_fatal(), selector == "observer-fatal");
        assert_eq!(trace.as_slice(), &CHECKPOINTS[..18]);
    } else {
        let wanted = if selector == "observer-cancel" {
            Checkpoint::AfterFinalCurrentness
        } else {
            target.ok_or("missing target")?
        };
        let error = attempt
            .result()
            .expect_err("observed cancellation cannot return Output");
        assert_eq!(error.cancellation().map(|c| c.checkpoint()), Some(wanted));
        assert!(!error.compiler_fatal());
        let last = CHECKPOINTS.iter().position(|p| *p == wanted).unwrap();
        assert_eq!(
            trace.as_slice(),
            &CHECKPOINTS[..=last],
            "later phases must not run"
        );
        let calls = if last < 2 { 0 } else { 1 };
        assert_eq!(
            attempt.callback_count(),
            if selector == "repeat-cancel" {
                2
            } else {
                calls
            }
        );
        assert_eq!(attempt.compiler_callback_count(), calls);
    }
    recheck(&mut recipe)?;
    // Existing outer file custody remains a root responsibility; the cancelled
    // API itself makes no final source-currentness claim before that checkpoint.
    emit(
        &serde_json::json!({
            "kind":"genuine_phase_boundary_cancellation", "selector":selector,
            "create":created, "callback_count":attempt.callback_count(),
            "compiler_callback_count":attempt.compiler_callback_count(),
            "visited":trace.iter().map(|p|format!("{p:?}")).collect::<Vec<_>>(),
            "typed_cancelled":attempt.result().err().is_some_and(|e|e.cancellation().is_some()),
            "observer_drop_witness":true,"independent_normal_oracle_equal":selector=="never",
            "intra_phase_interruption":false,"stop_latency_bound":null,
            "complete_owner_heap_bytes":null,"grants_authority":false
        }),
        SUMMARY_LIMIT,
    )?;
    Ok(())
}
