//! Test-only original recipe outcomes. No new production route or cancellation API.
use super::*;
use crate::source_local_order_recipe_v1 as codec;

const INPUT_SCHEMA: &str = "fe2o3-recipe-outcome-series-input-v1";
const OUTCOME_SCHEMA: &str = "fe2o3-recipe-outcome-v1";
const OUTCOME_ENV: &str = "FE2O3_RECIPE_OUTCOME_CONFIG";
const OUTCOME_HASH_ENV: &str = "FE2O3_RECIPE_OUTCOME_CONFIG_SHA256";
const OUTCOME_PREFIX: &[u8] = b"FE2O3_RECIPE_OUTCOME_V1 ";
const SERIES_PREFIX: &[u8] = b"FE2O3_RECIPE_OUTCOME_SERIES_V1 ";
const OUTCOME_LIMIT: usize = 2 * NORMAL_LIMIT + 8192;
const CALLBACK_SECONDS: u64 = 60;
const REFUSAL: &str = "local-order recipe source revision changed";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Workload {
    CheckedRebind,
    ExactRevisionRefusal,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RunMode {
    Ordinary,
    Series,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema: String,
    mode: RunMode,
    workload: Workload,
    invocation: Config,
    origin_oracle: PinnedFile,
    ordinary_oracle: Option<PinnedFile>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Binding {
    workload: Workload,
    source: String,
    current_source_sha256: String,
    origin_source_sha256: String,
    origin_normal_sha256: String,
    origin_source_initializer: [u32; 4],
    recipe_sha256: String,
    instance_axes: [[u8; 32]; 5],
}
fn validate_input(input: &Input) -> Result<(), String> {
    if input.schema != INPUT_SCHEMA
        || input.invocation.schema != SCHEMA
        || input.invocation.mode != Mode::Ordinary
        || input.invocation.oracle.is_some()
        || !matches!(&input.invocation.intent, Intent::Replay { .. })
        || (input.mode == RunMode::Series) != input.ordinary_oracle.is_some()
    {
        return Err("closed outcome mode/replay/oracle contract".into());
    }
    hash(&input.invocation.source_sha256)?;
    validate_arguments(&input.invocation.rustc_args).map_err(|e| e.to_string())?;
    require_canonical_overflow_checks_v1(&input.invocation.rustc_args)
}
fn binding(input: &Input, recipe_bytes: &[u8], origin_bytes: &[u8]) -> Result<Binding, String> {
    let recipe = codec::Recipe::decode(recipe_bytes).map_err(|e| e.to_string())?;
    if recipe.preference() != codec::Order::ReverseReady
        || recipe.constraint().relation != codec::Relation::OrBeforeXor
        || recipe.constraint().strength != codec::Strength::Exact
    {
        return Err("fixed reverse-ready exact recipe required".into());
    }
    let current = hash(&input.invocation.source_sha256)?;
    let origin = recipe.origin().source;
    if origin == current {
        return Err("actual source revision must differ from recipe origin".into());
    }
    let binding_mode = match (input.workload, recipe.source_binding()) {
        (Workload::CheckedRebind, codec::SourceBinding::RebindCurrent {}) => "rebind_current",
        (
            Workload::ExactRevisionRefusal,
            codec::SourceBinding::ExactRevision {
                expected_source_sha256,
                ..
            },
        ) if expected_source_sha256 == origin => "exact_revision",
        _ => return Err("workload recipe source-binding mode differs".into()),
    };
    let axes = recipe.binding();
    let axes = [
        axes.function,
        axes.item,
        axes.monomorphization,
        axes.generic_types,
        axes.const_arguments,
    ];
    let value: serde_json::Value =
        serde_json::from_slice(origin_bytes).map_err(|e| e.to_string())?;
    let evidence = &value["evidence"];
    let created: Vec<u8> =
        serde_json::from_value(value["created_recipe"].clone()).map_err(|e| e.to_string())?;
    let observed_axes: [[u8; 32]; 5] =
        serde_json::from_value(evidence["instance_axes"].clone()).map_err(|e| e.to_string())?;
    let source: [u8; 32] =
        serde_json::from_value(evidence["source_sha256"].clone()).map_err(|e| e.to_string())?;
    let recipe_hash: [u8; 32] =
        serde_json::from_value(evidence["recipe_sha256"].clone()).map_err(|e| e.to_string())?;
    let initializer: [u32; 4] = serde_json::from_value(evidence["source_initializer"].clone())
        .map_err(|e| e.to_string())?;
    if created != recipe_bytes
        || observed_axes != axes
        || source != origin
        || recipe_hash != <[u8; 32]>::from(Sha256::digest(recipe_bytes))
        || evidence["created"] != true
        || evidence["source_binding_mode"] != binding_mode
        || evidence["requested_order"] != "reverse_ready"
        || evidence["requested_relation"] != "or_before_xor"
        || evidence["strength"] != "exact"
        || evidence["actual_relation"] != "or_before_xor"
        || evidence["constraint_outcome"]
            != serde_json::json!({"status":"honored","relation":"or_before_xor"})
        || evidence["grants_authority"] != false
        || value["grants_artifact_or_launch_authority"] != false
    {
        return Err("full ordinary Create oracle/recipe/source bindings differ".into());
    }
    Ok(Binding {
        workload: input.workload,
        source: input.invocation.source.clone(),
        current_source_sha256: hex(&current),
        origin_source_sha256: hex(&origin),
        origin_normal_sha256: sha(origin_bytes),
        origin_source_initializer: initializer,
        recipe_sha256: sha(recipe_bytes),
        instance_axes: axes,
    })
}
fn prepared_binding(
    input: &Input,
    recipe: &Option<RetainedBytes>,
    origin: &[u8],
) -> Result<Binding, String> {
    binding(
        input,
        &recipe.as_ref().ok_or("missing current recipe guard")?.bytes,
        origin,
    )
}
fn check_refusal(error: &Failure) -> Result<(), String> {
    if error.phase() != Phase::RecipeBinding
        || error.diagnostic() != REFUSAL
        || error.compiler_fatal()
    {
        return Err(format!(
            "unexpected original failure/{:?}: {}",
            error.phase(),
            error.diagnostic()
        ));
    }
    Ok(())
}
fn outcome(
    value: Result<&Output, &Failure>,
    bound: &Binding,
    calls: usize,
    entries: usize,
) -> Result<Vec<u8>, String> {
    if calls != 1 || entries != 1 {
        return Err("outcome requires one genuine callback".into());
    }
    let (status, normal_utf8, failure) = match (bound.workload, value) {
        (Workload::CheckedRebind, Ok(output)) => {
            let e = output.evidence();
            if e.source_sha256() != &hash(&bound.current_source_sha256)?
                || e.instance_axes() != &bound.instance_axes
                || e.source_initializer() == &bound.origin_source_initializer
                || e.recipe_sha256() != &hash(&bound.recipe_sha256)?
                || e.source_binding_mode()
                    != api::SourceLocalOrderSourceBindingModeV1::RebindCurrent
                || e.requested_order() != api::SourceLocalOrderOrderV1::ReverseReady
                || e.requested_relation() != api::SourceLocalOrderRelationV1::OrBeforeXor
                || e.actual_relation() != api::SourceLocalOrderRelationV1::OrBeforeXor
                || e.strength() != api::SourceLocalOrderStrengthV1::Exact
                || e.constraint_outcome()
                    != (api::SourceLocalOrderConstraintOutcomeV1::Honored {
                        relation: api::SourceLocalOrderRelationV1::OrBeforeXor,
                    })
                || e.created()
                || output.created_recipe_bytes().is_some()
                || e.grants_authority()
                || output.grants_artifact_or_launch_authority()
            {
                return Err("genuine checked-rebind evidence differs".into());
            }
            (
                "accepted",
                Some(String::from_utf8(normal(output)?).map_err(|e| e.to_string())?),
                serde_json::Value::Null,
            )
        }
        (Workload::ExactRevisionRefusal, Err(error)) => {
            check_refusal(error)?;
            (
                "refused",
                None,
                serde_json::json!({
                    "phase":"RecipeBinding","diagnostic":error.diagnostic(),"compiler_fatal":false
                }),
            )
        }
        (_, Err(error)) => {
            return Err(format!(
                "unexpected original failure/{:?}: {}",
                error.phase(),
                error.diagnostic()
            ));
        }
        (_, Ok(_)) => return Err("designated exact-revision refusal unexpectedly succeeded".into()),
    };
    json_bytes(
        &serde_json::json!({
            "schema":OUTCOME_SCHEMA,"binding":bound,"callback_count":calls,"compiler_callback_count":entries,
            "status":status,"normal_utf8":normal_utf8,"failure":failure,"grants_authority":false
        }),
        OUTCOME_LIMIT,
    )
}
fn record_to(out: &mut impl std::io::Write, prefix: &[u8], bytes: &[u8]) -> Result<(), String> {
    out.write_all(b"\n")
        .and_then(|_| out.write_all(prefix))
        .and_then(|_| out.write_all(bytes))
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}
fn emit_outcome(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > OUTCOME_LIMIT {
        return Err("outcome output cap".into());
    }
    record_to(&mut std::io::stdout().lock(), OUTCOME_PREFIX, bytes)
}
fn emit_row(value: &impl Serialize, cap: usize) -> Result<(), String> {
    record_to(
        &mut std::io::stderr().lock(),
        SERIES_PREFIX,
        &json_bytes(value, cap)?,
    )
}
#[derive(Clone, Debug, Serialize)]
struct Row {
    ordinal: usize,
    calibration: bool,
    elapsed_ns: u64,
    outcome_bytes: usize,
    outcome_sha256: String,
    exact_ordinary_oracle_equal: bool,
}
fn statistics(rows: &[Row], expected: &[u8]) -> Result<Stats, String> {
    if rows.len() != CALLS {
        return Err("exactly35 complete outcome samples required".into());
    }
    let digest = sha(expected);
    let mut times = [0_u64; 30];
    for (i, row) in rows.iter().enumerate() {
        if row.ordinal != i + 1
            || row.calibration != (i < 5)
            || !row.exact_ordinary_oracle_equal
            || row.outcome_bytes != expected.len()
            || row.outcome_sha256 != digest
        {
            return Err("outcome order/calibration/full-oracle mismatch".into());
        }
        if i >= 5 {
            times[i - 5] = row.elapsed_ns;
        }
    }
    times.sort_unstable();
    Ok(Stats {
        p50_ns: times[14],
        p95_ns: times[28],
        max_ns: times[29],
    })
}
fn ordinary(input: &Input, origin: &mut RetainedBytes) -> Result<(), String> {
    let (request, mut recipe) = prepare_request(&input.invocation)?;
    let bound = prepared_binding(input, &recipe, &origin.bytes)?;
    let attempt = run_source_local_order_recipe_driver_v1(&input.invocation.rustc_args, request);
    let bytes = outcome(
        attempt.result(),
        &bound,
        attempt.callback_count(),
        attempt.compiler_callback_count(),
    )?;
    recheck(&mut recipe)?;
    origin.recheck()?;
    emit_outcome(&bytes)?;
    emit_row(
        &serde_json::json!({
            "kind":"ordinary_complete","schema":OUTCOME_SCHEMA,"binding":bound,
            "outcome_bytes":bytes.len(),"outcome_sha256":sha(&bytes),"elapsed_ns":null,
            "statistics":null,"retained_logical_bytes":null,"grants_authority":false
        }),
        SUMMARY_LIMIT,
    )
}
struct OutcomeCallbacks<'a> {
    input: &'a Input,
    origin: &'a [u8],
    expected: &'a [u8],
    first: Option<Prepared>,
    binding: Binding,
    entries: usize,
    reservations: Reservations,
    rows: Vec<Row>,
    result: Option<Result<(), String>>,
    current_ordinal: Option<usize>,
    current_elapsed: Option<u128>,
    current_phase: Option<Phase>,
}
impl OutcomeCallbacks<'_> {
    fn run_series<'tcx>(&mut self, tcx: TyCtxt<'tcx>) -> Result<(), String> {
        let series_started = Instant::now();
        for index in 0..CALLS {
            self.current_ordinal = Some(index + 1);
            self.current_elapsed = None;
            self.current_phase = None;
            if series_started.elapsed() >= Duration::from_secs(CALLBACK_SECONDS) {
                return Err("60-second between-call stopping bound reached".into());
            }
            let Prepared {
                request,
                input,
                mut recipe,
            } = if index == 0 {
                self.first
                    .take()
                    .ok_or("missing first original admission")?
            } else {
                self.reservations.next()?;
                prepare(&self.input.invocation)?
            };
            if prepared_binding(self.input, &recipe, self.origin)? != self.binding {
                return Err("per-call source/recipe/origin binding changed".into());
            }
            // Exactly the original fresh collector and consuming operational route.
            // No observer, synthetic source graph, alternate Work, or storage walk.
            let started = Instant::now();
            let result = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|message| Failure::new(Phase::Frontend, message))
            .and_then(|transaction| {
                transaction.compile_source_local_order_recipe_v1(input, &request)
            });
            let elapsed = started.elapsed().as_nanos();
            self.current_elapsed = Some(elapsed);
            self.current_phase = result.as_ref().err().map(|error| error.phase());
            let bytes = outcome(result.as_ref(), &self.binding, 1, self.entries)?;
            drop(result);
            recheck(&mut recipe)?;
            compare(&bytes, Some(self.expected))?;
            drop((request, recipe));
            let row = Row {
                ordinal: index + 1,
                calibration: index < 5,
                elapsed_ns: u64::try_from(elapsed).map_err(|_| "elapsed overflow")?,
                outcome_bytes: bytes.len(),
                outcome_sha256: sha(&bytes),
                exact_ordinary_oracle_equal: true,
            };
            // Flush obtained rows before retaining them; root retains partial streams.
            emit_row(
                &serde_json::json!({"kind":"sample","sample":&row}),
                ROW_LIMIT,
            )?;
            self.rows.push(row);
            if index == 0 {
                emit_outcome(&bytes)?;
            }
        }
        Ok(())
    }
}
impl Callbacks for OutcomeCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.entries = self.entries.saturating_add(1);
        self.result = Some(if self.entries == 1 {
            self.run_series(tcx)
        } else {
            Err("repeated genuine callback".into())
        });
        Compilation::Stop
    }
}
fn series(
    input: &Input,
    origin: &mut RetainedBytes,
    expected: &mut RetainedBytes,
) -> Result<(), String> {
    let mut reservations = Reservations::new(true)?;
    reservations.next()?;
    let first = prepare(&input.invocation)?;
    let bound = prepared_binding(input, &first.recipe, &origin.bytes)?;
    emit_row(
        &serde_json::json!({
            "kind":"plan","schema":OUTCOME_SCHEMA,"binding":bound,
            "calls":35,"calibration":5,"measured":30,"upfront_selected_input_reservations":reservations.envelope,
            "additional_origin_read_bytes":3 * (NORMAL_LIMIT + 1),
            "additional_ordinary_oracle_read_bytes":3 * (OUTCOME_LIMIT + 1),
            "callback_stopping_seconds":CALLBACK_SECONDS,"external_deadline_required":true,
            "ordinary_oracle_sha256":sha(&expected.bytes),"ordinary_oracle_bytes":expected.bytes.len(),
            "retained_logical_bytes":null,"storage_walks_in_timer":false,"grants_authority":false
        }),
        SUMMARY_LIMIT,
    )?;
    let mut callbacks = OutcomeCallbacks {
        input,
        origin: &origin.bytes,
        expected: &expected.bytes,
        first: Some(first),
        binding: bound,
        entries: 0,
        reservations,
        rows: Vec::with_capacity(CALLS),
        result: None,
        current_ordinal: None,
        current_elapsed: None,
        current_phase: None,
    };
    let fatal = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(&input.invocation.rustc_args, &mut callbacks)
    })
    .is_err();
    let result = callbacks
        .result
        .take()
        .unwrap_or_else(|| Err("missing genuine callback".into()));
    if fatal || callbacks.entries != 1 || result.is_err() || callbacks.reservations.remaining != 0 {
        emit_row(
            &serde_json::json!({
                "kind":"incomplete","compiler_fatal":fatal,"callback_count":callbacks.entries,
                "completed_samples":callbacks.rows.len(),"remaining_call_reservations":callbacks.reservations.remaining,
                "current_ordinal":callbacks.current_ordinal,
                "current_elapsed_ns_decimal":callbacks.current_elapsed.map(|v|v.to_string()),
                "current_original_phase":callbacks.current_phase.map(|v|format!("{v:?}")),
                "diagnostic":result.err().map(diagnostic),"statistics":null,"budget_accepted":false
            }),
            SUMMARY_LIMIT,
        )?;
        return Err("incomplete original outcome series; no qualification".into());
    }
    let measured = statistics(&callbacks.rows, &expected.bytes)?;
    let bound = callbacks.binding.clone();
    drop(callbacks);
    // Check both original descriptors after all calls, before completion.
    origin.recheck()?;
    expected.recheck()?;
    emit_row(
        &serde_json::json!({
            "kind":"series_complete","schema":OUTCOME_SCHEMA,"binding":bound,
            "callback_count":1,"raw_samples":35,"calibration":5,"measured":30,"statistics":measured,
            "percentile_method":"nearest_rank_30_rank15_rank29_rank30",
            "ordinary_oracle_sha256":sha(&expected.bytes),"ordinary_oracle_bytes":expected.bytes.len(),
            "all35_exact_ordinary_oracle_equal":true,"frontend_reused":true,"admitted_owner_reused":false,
            "scope":"fresh_transaction_creation_through_original_consuming_recipe_return",
            "companion_serialization_hash_equality_in_timer":false,"api_internal_checks_in_timer":true,
            "retained_logical_bytes":null,"peak_heap_measured":false,"rss_measured":false,
            "budget_accepted":false,"grants_authority":false
        }),
        SUMMARY_LIMIT,
    )
}
fn child() -> Result<(), String> {
    let guard = RetainedBytes::open(
        &PinnedFile {
            path: std::env::var(OUTCOME_ENV).map_err(|_| "missing outcome config")?,
            sha256: std::env::var(OUTCOME_HASH_ENV).map_err(|_| "missing outcome config pin")?,
        },
        CONFIG_LIMIT,
    )?;
    let input: Input = serde_json::from_slice(&guard.bytes).map_err(|e| e.to_string())?;
    validate_input(&input)?;
    let mut origin = RetainedBytes::open(&input.origin_oracle, NORMAL_LIMIT)?;
    drop(guard);
    match input.mode {
        RunMode::Ordinary => ordinary(&input, &mut origin),
        RunMode::Series => {
            let mut expected = RetainedBytes::open(
                input
                    .ordinary_oracle
                    .as_ref()
                    .ok_or("missing ordinary oracle")?,
                OUTCOME_LIMIT,
            )?;
            series(&input, &mut origin, &mut expected)
        }
    }
}
#[test]
#[ignore = "root-pinned actual edited source, genuine Create and ordinary outcome oracles, bounded process interval"]
fn actual_source_local_order_recipe_outcome_series_v1() {
    if let Err(error) = child() {
        let _ = emit_row(
            &serde_json::json!({
                "kind":"child_refused","diagnostic":diagnostic(error),"statistics":null,
                "budget_accepted":false,"grants_authority":false
            }),
            SUMMARY_LIMIT,
        );
        panic!("original outcome series refused; retain the bounded original streams");
    }
}
#[path = "source_local_order_recipe_outcome_controls_v1.rs"]
mod controls;
