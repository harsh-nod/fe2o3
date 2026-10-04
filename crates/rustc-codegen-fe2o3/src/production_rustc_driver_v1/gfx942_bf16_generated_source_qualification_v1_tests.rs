//! Four fresh generated-source sessions. Parent owns all process supervision.
use super::gfx942_bf16_call_source_cpu_qualification_v1_tests::observed as fresh_cpu;
use super::gfx942_bf16_publication_tap_v1_tests as tap;
use super::gfx942_tiled_region_qualification_v1_tests::observation::cpu::observed as original_cpu;
use super::{Callbacks, Compilation, Compiler, TyCtxt};
use crate::production_tiled_region_source_v1::{
    Bf16SourcePublicationProgressV1, Bf16TileReturnOrderV1, Bf16TileSourcePublishRequestV1,
    PublishedBf16TileSourceV1, publish_bf16_tile_helper_source_v1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[path = "gfx942_bf16_generated_controls_v1_tests.rs"]
mod controls;
#[path = "gfx942_bf16_generated_inputs_v1_tests.rs"]
mod inputs;

const CONFIG_ENV: &str = "FE2O3_BF16_GENERATED_CONFIG_V1";
const PREFIX: &str = "FE2O3_BF16_GENERATED_SOURCE_V1 ";
const FRAME_CAP: usize = 131072;
const ORIGINAL_BYTES: u64 = 3950;
const ORIGINAL_SHA: &str = "fcb26135ad4f931bb8dda63d631639a34dd8461e7801c22a1f6a383e0e735a3e";
fn digest(bytes: &[u8]) -> String {
    super::lower_hex_v1(&Sha256::digest(bytes))
}
fn timely(start: Instant) -> Result<(), &'static str> {
    (start.elapsed() < std::time::Duration::from_secs(300))
        .then_some(())
        .ok_or("adapter 300-second deadline")
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    cwd: String,
    session: u32,
    record: String,
    record_sha256: String,
    candidate: String,
    helper_name: String,
    sidecar: String,
    observation: String,
}
fn read_config<T: serde::de::DeserializeOwned>() -> Result<T, String> {
    let path = PathBuf::from(std::env::var_os(CONFIG_ENV).ok_or("explicit generated config")?);
    if !path.is_absolute() {
        return Err("absolute config path".into());
    }
    let bytes =
        super::gfx942_inline_value_qualification_v30_tests::read_bounded(&path, inputs::CONFIG_CAP)
            .map_err(|e| format!("{e:?}"))?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn checked_config(config: &Config) -> Result<PathBuf, String> {
    if config.schema != "fe2o3-bf16-generated-session-config-v1"
        || config.session >= 4
        || config.helper_name.is_empty()
        || config.helper_name.len() > 64
        || config.cwd.len() > 4096
        || !inputs::lower_digest(&config.record_sha256)
    {
        return Err("closed generated session profile".into());
    }
    let cwd = PathBuf::from(&config.cwd);
    if !cwd.is_absolute()
        || cwd.canonicalize().map_err(|e| e.to_string())? != cwd
        || std::env::current_dir().map_err(|e| e.to_string())? != cwd
    {
        return Err("actual generated task cwd".into());
    }
    for path in [
        &config.record,
        &config.candidate,
        &config.sidecar,
        &config.observation,
    ] {
        inputs::relative(path).map_err(str::to_owned)?;
    }
    if config.record == config.sidecar
        || config.record == config.observation
        || config.sidecar == config.observation
        || config.candidate == config.sidecar
        || config.candidate == config.observation
    {
        return Err("distinct generated paths".into());
    }
    inputs::absent_output(&cwd, &config.sidecar).map_err(str::to_owned)?;
    inputs::absent_output(&cwd, &config.observation).map_err(str::to_owned)?;
    if config.session % 2 == 0 {
        inputs::absent_output(&cwd, &config.candidate).map_err(str::to_owned)?;
    } else {
        inputs::checked_path(&cwd, &config.candidate, true).map_err(str::to_owned)?;
    }
    Ok(cwd)
}
#[derive(Clone, Copy, Serialize)]
struct Selection {
    semantic_sha256: [u8; 32],
    canonical_sha256: [u8; 32],
    mir_sha256: [u8; 32],
    original_sha256: [u8; 32],
    original_bytes: usize,
    return_order: Bf16TileReturnOrderV1,
}
struct Body<'a> {
    config: &'a Config,
    record: &'a inputs::Record,
    stream: tap::Stream,
    calls: usize,
    original: Option<original_cpu::PublicationObservation>,
    fresh: Option<fresh_cpu::PublicationObservation>,
    original_progress: original_cpu::PublicationProgress,
    fresh_progress: fresh_cpu::PublicationProgress,
    publication_progress: Bf16SourcePublicationProgressV1,
    publication: Option<PublishedBf16TileSourceV1>,
    selection: Option<Selection>,
    phase: Option<Value>,
    accounting: Option<[usize; 4]>,
    source_postflight: bool,
    expected_normal_refusal: bool,
    failure: Option<String>,
}
fn diagnostic(value: &impl std::fmt::Display) -> String {
    struct Limited(String);
    impl std::fmt::Write for Limited {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            let room = 512usize.saturating_sub(self.0.len());
            let mut take = s.len().min(room);
            while !s.is_char_boundary(take) {
                take -= 1;
            }
            self.0.push_str(&s[..take]);
            if take < s.len() {
                Err(std::fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut out = Limited(String::with_capacity(512));
    let _ = std::fmt::write(&mut out, format_args!("{value}"));
    out.0
}
fn is_expected_normal_refusal(error: &crate::production_pipeline::ProductionPipelineError) -> bool {
    use crate::production_pipeline::ProductionPipelineError as P;
    use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as R;
    use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1 as K;
    matches!(
        error,
        P::RankedProjection(R::StructuralValidation(
            K::LocalHelperSourceConsumerUnavailable {
                consumer: "BF16 nominal source-ranked projection"
            }
        ))
    )
}
impl Callbacks for Body<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        if self.calls != 1 {
            self.failure = Some("more than one actual analysis callback".into());
            return Compilation::Stop;
        }
        let transaction = match super::transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.failure = Some(diagnostic(&error));
                return Compilation::Stop;
            }
        };
        if self.config.session % 2 == 0 {
            let (result, phase) =
                transaction.observe_bf16_mfma_source_for_test_v1(|source, budget| {
                    use fe2o3_lower_mir_kernel::ProductionTiledRegionInspectionErrorV1 as E;
                    let ledger = budget.work_ledger_identity_v1();
                    let before = [budget.storage(), budget.work()];
                    budget.reserve_storage(
                        2 * std::mem::size_of::<Bf16SourcePublicationProgressV1>()
                            + 2 * std::mem::size_of::<PublishedBf16TileSourceV1>()
                            + 2 * std::mem::size_of::<Selection>()
                            + 1024,
                    )?;
                    if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                        || super::lower_hex_v1(source.source().sha256())
                            != self.record.spec.source.sha256
                    {
                        return Err(E::Unavailable(
                            "actual original source differs from invocation",
                        ));
                    }
                    let owner = source.emission().original();
                    let order = if self.config.session == 0 {
                        Bf16TileReturnOrderV1::Identity
                    } else {
                        Bf16TileReturnOrderV1::Swap01
                    };
                    let selection = Selection {
                        semantic_sha256: *owner
                            .semantic_ssa()
                            .source_semantic()
                            .semantic_sha256()
                            .as_bytes(),
                        canonical_sha256: *owner.executable().canonical().identity().digest(),
                        mir_sha256: *source.mir_sha256(),
                        original_sha256: *source.source().sha256(),
                        original_bytes: source.source().bytes().len(),
                        return_order: order,
                    };
                    self.selection = Some(selection);
                    // Exactly the original consuming owner: no second import/transaction.
                    self.original = Some(original_cpu::observe_for_publication(
                        source,
                        budget,
                        &mut self.stream,
                        &mut self.original_progress,
                    )?);
                    let request = Bf16TileSourcePublishRequestV1 {
                        semantic_sha256: selection.semantic_sha256,
                        canonical_sha256: selection.canonical_sha256,
                        mir_sha256: selection.mir_sha256,
                        original_sha256: selection.original_sha256,
                        original_path: &self.record.spec.source.path,
                        candidate_path: &self.config.candidate,
                        helper_name: &self.config.helper_name,
                        return_order: order,
                    };
                    let published = publish_bf16_tile_helper_source_v1(
                        source,
                        &request,
                        budget,
                        &mut self.publication_progress,
                    );
                    self.accounting = Some([before[0], budget.storage(), before[1], budget.work()]);
                    if budget.work_ledger_identity_v1() != ledger
                        || budget.failed_work().is_some()
                        || budget.failed_storage().is_some()
                    {
                        return Err(E::Unavailable("original shared ledger/accounting differs"));
                    }
                    match published {
                        Ok(facts) => {
                            self.publication = Some(facts);
                            Ok(())
                        }
                        Err(error) => {
                            self.failure = Some(diagnostic(&error));
                            Err(E::Unavailable(
                                "publication failed; original progress retained",
                            ))
                        }
                    }
                });
            self.phase = phase
                .map(|value| serde_json::to_value(value).expect("fixed original phase serializes"));
            match result {
                Ok(()) => {
                    self.source_postflight = phase.is_some_and(|p| {
                        p.result_ok && p.same_ledger && !p.failed_work && !p.failed_storage
                    });
                    if !self.source_postflight {
                        self.failure = Some("original source postflight phase differs".into());
                    }
                }
                Err(error) => {
                    if self.failure.is_none() {
                        self.failure = Some(diagnostic(&error));
                    }
                }
            }
        } else {
            let (result, phase) =
                transaction.observe_bf16_call_source_cpu_for_test_v1(|source, emission, budget| {
                    use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                    let ledger = budget.work_ledger_identity_v1();
                    let before = [budget.storage(), budget.work()];
                    if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                        || super::lower_hex_v1(source.source().sha256())
                            != self.record.spec.source.sha256
                        || emission.return_permutation() != self.stream.requested_permutation()
                    {
                        return Err(E::Unavailable(
                            "actual fresh source/requested Return differs",
                        ));
                    }
                    let abi = source.helper_actual_fn_abi();
                    let mode = |m: &rustc_target::callconv::PassMode| match m {
                        rustc_target::callconv::PassMode::Ignore => 0,
                        rustc_target::callconv::PassMode::Direct(_) => 1,
                        rustc_target::callconv::PassMode::Pair(..) => 2,
                        rustc_target::callconv::PassMode::Cast { .. } => 3,
                        rustc_target::callconv::PassMode::Indirect { .. } => 4,
                    };
                    if abi.args.len() != 4
                        || [
                            mode(&abi.args[0].mode),
                            mode(&abi.args[1].mode),
                            mode(&abi.args[2].mode),
                            mode(&abi.args[3].mode),
                            mode(&abi.ret.mode),
                        ] != [1, 3, 3, 4, 4]
                    {
                        return Err(E::Unavailable(
                            "actual helper FnABI modes differ from pinned profile",
                        ));
                    }
                    self.fresh = Some(fresh_cpu::observe_for_publication(
                        source,
                        emission,
                        budget,
                        &mut self.stream,
                        &mut self.fresh_progress,
                    )?);
                    self.accounting = Some([before[0], budget.storage(), before[1], budget.work()]);
                    if budget.work_ledger_identity_v1() != ledger
                        || budget.failed_work().is_some()
                        || budget.failed_storage().is_some()
                    {
                        return Err(E::Unavailable("fresh shared ledger/accounting differs"));
                    }
                    Ok(())
                });
            self.expected_normal_refusal = result
                .as_ref()
                .err()
                .is_some_and(|error| is_expected_normal_refusal(error));
            self.phase =
                phase.map(|value| serde_json::to_value(value).expect("fixed phase serializes"));
            self.source_postflight = self.expected_normal_refusal
                && self.phase.as_ref().is_some_and(|p| {
                    p["materialized"] == true
                        && p["normal_attempted"] == true
                        && p["normal_succeeded"] == false
                        && p["same_ledger"] == true
                        && p["failed_work"] == false
                        && p["failed_storage"] == false
                });
            if !self.source_postflight {
                self.failure = Some(
                    result
                        .as_ref()
                        .err()
                        .map(|e| diagnostic(e))
                        .unwrap_or_else(|| "unexpected ordinary normal-route success".into()),
                );
            }
        }
        Compilation::Stop
    }
}
fn integral_strings(
    value: &mut Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), &'static str> {
    *nodes = nodes.checked_add(1).ok_or("frame node overflow")?;
    if depth > 32 || *nodes > 16384 {
        return Err("bounded frame shape");
    }
    match value {
        Value::Number(number) => {
            // No float and no lossy JS Number. Every final-frame integer is text.
            let text = if let Some(n) = number.as_u64() {
                n.to_string()
            } else if let Some(n) = number.as_i64() {
                n.to_string()
            } else {
                return Err("nonintegral frame number");
            };
            *value = Value::String(text);
        }
        Value::Array(rows) => {
            for row in rows {
                integral_strings(row, depth + 1, nodes)?;
            }
        }
        Value::Object(rows) => {
            for row in rows.values_mut() {
                integral_strings(row, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn exact_sidecar(body: &Body<'_>) -> bool {
    body.stream.summary.is_some_and(|s| {
        s.rows == 34 && s.bytes == super::gfx942_bf16_publication_sidecar_v1_tests::COMPLETE_BYTES
    })
}
fn accepted(body: &Body<'_>) -> bool {
    if body.calls != 1 || !body.source_postflight || body.failure.is_some() || !exact_sidecar(body)
    {
        return false;
    }
    let accounting = body.accounting.is_some_and(|a| a[1] >= a[0] && a[3] > a[2]);
    if body.config.session % 2 == 0 {
        accounting
            && body.original.is_some()
            && body.fresh.is_none()
            && body.publication.is_some()
            && body.publication == body.publication_progress.published
            && body.selection.is_some()
            && !body.expected_normal_refusal
    } else {
        accounting
            && body.original.is_none()
            && body.fresh.is_some()
            && body.publication.is_none()
            && body.selection.is_none()
            && body.expected_normal_refusal
            && body.publication_progress == Bf16SourcePublicationProgressV1::new()
    }
}

#[test]
#[ignore = "nonspawning fresh selected-package preparation; root owns all Cargo/process supervision"]
fn prepare_generated_invocation() {
    let started = Instant::now();
    let spec: inputs::Spec = read_config().expect("closed preparation config");
    let cwd = inputs::validate_spec(&spec).expect("actual preparation selection");
    let output =
        inputs::absent_output(&cwd, &format!("{}/invocation.json", spec.directory)).unwrap();
    let record = inputs::derive(&spec).expect("actual fresh invocation");
    let bytes = serde_json::to_vec(&record).unwrap();
    assert!(bytes.len() <= inputs::RECORD_CAP);
    timely(started).unwrap();
    super::publish_new_inert_output(
        &output,
        &bytes,
        inputs::RECORD_CAP,
        "BF16 generated invocation",
    )
    .unwrap();
    assert_eq!(inputs::derive(&spec).unwrap(), record);
    timely(started).unwrap();
    // This is not one of the four frontend/CPU acceptance markers.
    println!(
        "\nBF16_GENERATED_PREPARATION_V1 {} {}",
        bytes.len(),
        digest(&bytes)
    );
    timely(started).unwrap();
}

#[test]
#[ignore = "one isolated original/fresh BF16 compiler session; root owns the four-session parent"]
fn actual_generated_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed generated session config");
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    if config.session % 2 == 0 {
        assert_eq!(record.spec.source.bytes, ORIGINAL_BYTES);
        assert_eq!(record.spec.source.sha256, ORIGINAL_SHA);
        assert_ne!(record.spec.source.path, config.candidate);
    } else {
        assert_eq!(record.spec.source.path, config.candidate);
    }
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = Body {
        config: &config,
        record: &record,
        stream: tap::Stream::new(config.session, cwd.join(&config.sidecar)).unwrap(),
        calls: 0,
        original: None,
        fresh: None,
        original_progress: Default::default(),
        fresh_progress: Default::default(),
        publication_progress: Bf16SourcePublicationProgressV1::new(),
        publication: None,
        selection: None,
        phase: None,
        accounting: None,
        source_postflight: false,
        expected_normal_refusal: false,
        failure: None,
    };
    timely(started).unwrap();
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(&record.args, &mut body))
    }));
    let compiler_clean = matches!(run, Ok(Ok(())));
    // Preserve callback fields and streamed prefix even after fatal/panic.
    let recheck = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inputs::derive(&record.spec)
    }));
    let unchanged = matches!(&recheck,Ok(Ok(actual))if actual==&record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    let success = compiler_clean && unchanged && deadline && analysis_empty && accepted(&body);
    let mut frame = json!({
        "schema":"fe2o3-bf16-generated-source-observation-v1","session":config.session,
        "requested_order":if config.session<2 {"identity"}else{"swap01"},
        "source_mode":if config.session%2==0 {"original"}else{"fresh"},
        "root_function":"tiled_region_inspection_v1","helper_name":config.helper_name,
        "candidate_path":config.candidate,"record_sha256":config.record_sha256,
        "source_pin":record.spec.source,"actual_rustc_callbacks":body.calls,
        "compiler_clean":compiler_clean,"inputs_unchanged":unchanged,"deadline_met":deadline,
        "analysis_output_empty":analysis_empty,"cpu":body.original,"fresh_cpu":body.fresh,
        "original_progress":body.original_progress,"fresh_progress":body.fresh_progress,
        "source_selection":body.selection,"publication":body.publication,
        "publication_progress":body.publication_progress,"source_postflight":body.source_postflight,
        "phase":body.phase,"accounting":body.accounting,
        "expected_normal_refusal":body.expected_normal_refusal,"sidecar":body.stream.diagnostic(),
        "failure":body.failure,"accepted":success,"parent_acceptance_required":true,
        "normal_qualified":false,"hardware_observed":false,"source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false,
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    // Saving and printing failure evidence does not convert it into acceptance.
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 generated observation",
    )
    .unwrap();
    println!("\n{PREFIX}{}", std::str::from_utf8(&encoded).unwrap());
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        success && final_deadline,
        "generated-source attempt refused; retained evidence is not qualification"
    );
}

struct OwningBody<'a> {
    target_bound: bool,
    formal_owner: bool,
    ranked_formal: bool,
    formal: bool,
    attached: bool,
    module: bool,
    roster: bool,
    config: &'a Config,
    record: &'a inputs::Record,
    calls: usize,
    completed: bool,
    phase: Option<Value>,
    failure: Option<String>,
}
impl Callbacks for OwningBody<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        if self.calls != 1 {
            self.failure = Some("more than one actual owning analysis callback".into());
            return Compilation::Stop;
        }
        match super::transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        ) {
            Ok(transaction) => {
                let (result, phase) = if self.target_bound {
                    transaction.observe_bf16_owned_target_stage_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else if self.formal_owner {
                    transaction.observe_bf16_owned_formal_stage_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else if self.ranked_formal {
                    transaction.observe_bf16_owned_ranked_formal_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else if self.formal {
                    transaction.observe_bf16_owned_formal_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else if self.attached {
                    transaction.observe_bf16_owned_attached_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else if self.module {
                    transaction.observe_bf16_owned_module_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else if self.roster {
                    transaction.observe_bf16_owned_roster_for_test_v1(
                        owning_requested_permutation(self.config.session)
                            .expect("selected Identity/Swap01"),
                        |source, emission, budget| {
                            use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                            budget.charge_work(256)?;
                            if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                                || super::lower_hex_v1(source.source().sha256())
                                    != self.record.spec.source.sha256
                                || Some(emission.return_permutation())
                                    != owning_requested_permutation(self.config.session)
                            {
                                return Err(E::Unavailable(
                                    "actual owning source/requested Return differs",
                                ));
                            }
                            Ok(())
                        },
                    )
                } else {
                    transaction.observe_bf16_owned_root_for_test_v1(|source, emission, budget| {
                        use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                        budget.charge_work(256)?;
                        if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                            || super::lower_hex_v1(source.source().sha256())
                                != self.record.spec.source.sha256
                            || Some(emission.return_permutation())
                                != owning_requested_permutation(self.config.session)
                        {
                            return Err(E::Unavailable(
                                "actual owning source/requested Return differs",
                            ));
                        }
                        Ok(())
                    })
                };
                self.phase = phase.map(|phase| serde_json::to_value(phase).unwrap());
                self.completed = result.is_ok();
                self.failure = result.err().map(|error| diagnostic(&error));
            }
            Err(error) => self.failure = Some(diagnostic(&error)),
        }
        Compilation::Stop
    }
}

fn owning_requested_permutation(session: u32) -> Option<[u8; 4]> {
    match session {
        1 => Some([0, 1, 2, 3]),
        3 => Some([1, 0, 2, 3]),
        _ => None,
    }
}

fn completed_owning_phase(phase: &Value) -> bool {
    let count = |key: &str| phase[key].as_u64();
    let retained = count("occurrence_storage")
        .and_then(|a| count("nominal_storage").and_then(|b| a.checked_add(b)));
    let floor = count("entry_storage").and_then(|a| retained.and_then(|b| a.checked_add(b)));
    phase["materialized"] == true
        && phase["same_ledger"] == true
        && phase["normal_attempted"] == false
        && phase["normal_succeeded"] == false
        && phase["failed_work"] == false
        && phase["failed_storage"] == false
        && retained.is_some_and(|n| n > 0)
        && floor.is_some()
        && count("final_storage") == floor
        && count("work").is_some_and(|n| n > 0)
        && count("phase_peak_storage")
            .is_some_and(|n| floor.is_some_and(|f| n >= f) && n <= 2 * 1024 * 1024 * 1024)
}

#[test]
#[ignore = "one isolated fresh BF16 owning continuation; root owns preparation and process supervision"]
fn actual_generated_owning_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: false,
        ranked_formal: false,
        formal: false,
        attached: false,
        module: false,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // No CPU sidecar, source publication, target emission or lowerer attachment
    // is attempted by this distinct constructor test.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let success = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-owning-source-observation-v2",
        "session":config.session, "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        // The one owning path returns only after the exact full lowerer check.
        "owning_entry_completed":body.completed, "lowerer_validation_completed":body.completed,
        "phase":body.phase,
        "failure":body.failure, "accepted":success, "parent_acceptance_required":true,
        "normal_qualified":false, "lowerer_attached":false, "hardware_observed":false,
        "numerical_cpu_qualified":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private owning observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_OWNING_SOURCE_V2 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        success && final_deadline,
        "private owning source refused; saved evidence is not qualification"
    );
}

#[test]
fn owning_source_phase_requires_original_retained_account_and_no_normal_claim() {
    // Parser controls only; these rows never stand in for a live constructor.
    assert_eq!(owning_requested_permutation(1), Some([0, 1, 2, 3]));
    assert_eq!(owning_requested_permutation(3), Some([1, 0, 2, 3]));
    for refused in [0, 2, 4, u32::MAX] {
        assert_eq!(owning_requested_permutation(refused), None);
    }
    let good = json!({
        "materialized":true, "same_ledger":true, "normal_attempted":false,
        "normal_succeeded":false, "failed_work":false, "failed_storage":false,
        "occurrence_storage":5, "nominal_storage":7, "entry_storage":3,
        "final_storage":15, "work":1, "phase_peak_storage":16
    });
    assert!(completed_owning_phase(&good));
    for (key, value) in [
        ("materialized", json!(false)),
        ("same_ledger", json!(false)),
        ("normal_attempted", json!(true)),
        ("normal_succeeded", json!(true)),
        ("failed_work", json!(true)),
        ("failed_storage", json!(true)),
        ("occurrence_storage", json!(null)),
        ("final_storage", json!(14)),
        ("work", json!(0)),
        ("phase_peak_storage", json!(14)),
        ("phase_peak_storage", json!(2u64 * 1024 * 1024 * 1024 + 1)),
    ] {
        let mut bad = good.clone();
        bad[key] = value;
        assert!(!completed_owning_phase(&bad), "{key}");
    }
}

#[test]
#[ignore = "one isolated fresh BF16 first-roster continuation; root owns preparation and process supervision"]
fn actual_generated_roster_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: false,
        ranked_formal: false,
        formal: false,
        attached: false,
        module: false,
        roster: true,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // No CPU sidecar, source publication, target emission or lowerer attachment
    // is attempted by this distinct first-roster test.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let success = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-owning-source-observation-v3",
        "session":config.session, "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        // This selected path returns only after the first actual roster conversion,
        // fresh same-account nominal validation and owning cleanup/postflights.
        "owning_entry_completed":body.completed, "lowerer_validation_completed":body.completed,
        "roster_conversion_completed":body.completed, "roster_fresh_lowerer_validation_completed":body.completed,
        "authenticated_module_roster_created":false,
        "phase":body.phase,
        "failure":body.failure, "accepted":success, "parent_acceptance_required":true,
        "normal_qualified":false, "lowerer_attached":false, "hardware_observed":false,
        "numerical_cpu_qualified":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private first roster observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_OWNING_SOURCE_V3 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        success && final_deadline,
        "private first roster source refused; saved evidence is not qualification"
    );
}

#[test]
#[ignore = "one isolated fresh BF16 private module-roster continuation; root owns preparation and process supervision"]
fn actual_generated_module_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: false,
        ranked_formal: false,
        formal: false,
        attached: false,
        module: true,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // No CPU sidecar, source publication, target emission or lowerer attachment
    // is attempted by this distinct structural module-roster test.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let success = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-owning-source-observation-v4",
        "session":config.session, "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        // Completion requires actual Stage A conversion, fresh same-account
        // structural revalidation and real owning-pair drop/materialization postflight.
        "owning_entry_completed":body.completed, "lowerer_validation_completed":body.completed,
        "roster_conversion_completed":body.completed,
        "module_roster_conversion_completed":body.completed,
        "module_structural_revalidation_completed":body.completed,
        "module_owning_pair_dropped":body.completed,
        "authenticated_module_roster_created":body.completed,
        "phase":body.phase,
        "failure":body.failure, "accepted":success, "parent_acceptance_required":true,
        "normal_qualified":false, "lowerer_attached":false, "hardware_observed":false,
        "numerical_cpu_qualified":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private module roster observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_OWNING_SOURCE_V4 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        success && final_deadline,
        "private module roster source refused; saved evidence is not qualification"
    );
}

#[test]
#[ignore = "one isolated fresh BF16 private intact-owner continuation; root owns preparation and process supervision"]
fn actual_generated_attached_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: false,
        ranked_formal: false,
        formal: false,
        attached: true,
        module: false,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // No CPU sidecar, source publication, target emission or legacy Connected
    // attachment is attempted. Stage B remains a private intact-owner path.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let success = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-owning-source-observation-v5",
        "session":config.session, "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        // Completion requires actual Stage A and B conversion, fresh full
        // intact-owner replay, final pair drop and materialization postflight.
        "owning_entry_completed":body.completed, "lowerer_validation_completed":body.completed,
        "roster_conversion_completed":body.completed,
        "module_roster_conversion_completed":body.completed,
        "module_structural_revalidation_completed":body.completed,
        "private_attachment_completed":body.completed,
        "intact_owner_replay_completed":body.completed,
        "intact_owner_pair_dropped":body.completed,
        "legacy_connected":false,
        "authenticated_module_roster_created":body.completed,
        "phase":body.phase,
        "failure":body.failure, "accepted":success, "parent_acceptance_required":true,
        "normal_qualified":false, "lowerer_attached":false, "hardware_observed":false,
        "numerical_cpu_qualified":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private intact attachment observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_OWNING_SOURCE_V5 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        success && final_deadline,
        "private intact attachment source refused; saved evidence is not qualification"
    );
}

#[test]
#[ignore = "one isolated fresh BF16 formal diagnostic only; root owns finite process/output supervision"]
fn actual_generated_formal_diagnostic_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: false,
        ranked_formal: false,
        formal: true,
        attached: false,
        module: false,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // A successful test means collection and cleanup completed, not that formal
    // analysis was Complete. Exact analysis/reason/error rows are in stderr.
    // No report, waiver, formal admission, source publication or target output.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let collected = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-formal-diagnostic-observation-v1",
        "session":config.session,
        "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        "diagnostic_entry_completed":body.completed,
        "diagnostic_collection_completed":collected,
        "exact_analysis_rows_in_stderr":body.completed,
        "phase":body.phase, "failure":body.failure,
        "root_supervision_required":true, "qualification_accepted":false,
        "formal_admission":false, "normal_qualified":false,
        "legacy_connected":false, "lowerer_attached":false,
        "hardware_observed":false, "numerical_cpu_qualified":false,
        "launch_inputs_authenticated":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private formal diagnostic observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_FORMAL_DIAGNOSTIC_V1 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        collected && final_deadline,
        "formal diagnostic collection refused; analysis output is never qualification"
    );
}

#[test]
#[ignore = "one isolated fresh BF16 ranked guard proof only; root owns finite process/output supervision"]
fn actual_generated_ranked_formal_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: false,
        ranked_formal: true,
        formal: false,
        attached: false,
        module: false,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // Success requires the actual fresh guard continuation and cleanup, with
    // exact raw Incomplete reasons retained beside discharged proof evidence.
    // No ordinary admission, source publication, authenticated launch or target output.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let collected = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-ranked-formal-observation-v1",
        "session":config.session,
        "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        "ranked_formal_entry_completed":body.completed,
        "ranked_formal_collection_completed":collected,
        "exact_analysis_rows_in_stderr":body.completed,
        "ranked_guard_discharge_completed":body.completed,
        "raw_analysis_retained":body.completed,
        "exact_discharge_rows_in_stderr":body.completed,
        "phase":body.phase, "failure":body.failure,
        "root_supervision_required":true, "qualification_accepted":false,
        "formal_admission":false, "normal_qualified":false,
        "legacy_connected":false, "lowerer_attached":false,
        "hardware_observed":false, "numerical_cpu_qualified":false,
        "launch_inputs_authenticated":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private ranked formal observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_RANKED_FORMAL_V1 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        collected && final_deadline,
        "ranked formal guard discharge refused; no ordinary or launch qualification"
    );
}

#[test]
#[ignore = "one isolated fresh BF16 owning formal stage only; root owns finite process/output supervision"]
fn actual_generated_formal_owner_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: false,
        formal_owner: true,
        ranked_formal: false,
        formal: false,
        attached: false,
        module: false,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // Success requires the real owning production stage, two consuming stage
    // boundaries, wrong-Return refusal and fresh replay, then real cleanup.
    // No ordinary admission, source publication, authenticated launch or target output.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let collected = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-formal-owner-observation-v1",
        "session":config.session,
        "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        "formal_owner_entry_completed":body.completed,
        "formal_owner_collection_completed":collected,
        "exact_retained_reason_rows_in_stderr":body.completed,
        "ranked_guard_discharge_completed":body.completed,
        "raw_analysis_retained":body.completed,
        "production_stage_constructed":body.completed,
        "production_stage_replayed":body.completed,
        "consuming_stage_boundaries_completed":body.completed,
        "wrong_return_refused":body.completed,
        "exact_owner_rows_in_stderr":body.completed,
        "phase":body.phase, "failure":body.failure,
        "root_supervision_required":true, "qualification_accepted":false,
        "formal_admission":false, "normal_qualified":false,
        "legacy_connected":false, "lowerer_attached":false,
        "hardware_observed":false, "numerical_cpu_qualified":false,
        "launch_inputs_authenticated":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private owning formal-stage observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_FORMAL_OWNER_V1 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        collected && final_deadline,
        "owning formal stage refused; no ordinary or launch qualification"
    );
}

#[test]
#[ignore = "one isolated fresh BF16 private target-binding stage only; root owns finite process/output supervision"]
fn actual_generated_target_bound_source() {
    let started = Instant::now();
    let config: Config = read_config().expect("closed owning session config");
    // Reuse exact existing fresh-input configuration/preflight, not a new
    // frontend or source reconstruction. Both fresh orders get independent runs.
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = OwningBody {
        target_bound: true,
        formal_owner: false,
        ranked_formal: false,
        formal: false,
        attached: false,
        module: false,
        roster: false,
        config: &config,
        record: &record,
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
    let unchanged = matches!(&recheck, Ok(Ok(actual)) if actual == &record);
    let deadline = timely(started).is_ok();
    let analysis_empty =
        std::fs::read_dir(cwd.join(&record.spec.directory).join("analysis-output"))
            .is_ok_and(|mut entries| entries.next().is_none());
    // Success requires the real owning formal and target stages, actual binding,
    // fresh replay and source-bound negative controls, then original cleanup.
    // This is private target binding, not ordinary/optimizer/LLVM or launch admission.
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let collected = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.completed
        && body.failure.is_none()
        && body.phase.as_ref().is_some_and(completed_owning_phase);
    let mut frame = json!({
        "schema":"fe2o3-bf16-private-target-bound-observation-v1",
        "session":config.session,
        "requested_order":if config.session == 1 {"identity"}else{"swap01"},
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        "target_bound_entry_completed":body.completed,
        "target_bound_collection_completed":collected,
        "exact_retained_reason_rows_in_stderr":body.completed,
        "ranked_guard_discharge_completed":body.completed,
        "raw_analysis_retained":body.completed,
        "production_stage_constructed":body.completed,
        "production_stage_replayed":body.completed,
        "consuming_stage_boundaries_completed":body.completed,
        "wrong_return_refused":body.completed,
    });
    // Split the test-only observation literal without widening crate recursion.
    // Both disjoint field groups retain the identical flat wire shape.
    let serde_json::Value::Object(target_fields) = json!({
        "unused_callback_dropped":body.completed,
        "target_binding_completed":body.completed,
        "geometry_checked":body.completed,
        "wrong_expected_target_refused":body.completed,
        "wrong_kernel_refused":body.completed,
        "wrong_entry_refused":body.completed,
        "wrong_binding_refused":body.completed,
        "changed_bound_refused":body.completed,
        "exact_target_rows_in_stderr":body.completed,
        "phase":body.phase, "failure":body.failure,
        "root_supervision_required":true, "qualification_accepted":false,
        "formal_admission":false, "normal_qualified":false,
        "legacy_connected":false, "legacy_lowerer_attached":false,
        "hardware_observed":false, "numerical_cpu_qualified":false,
        "optimizer_invoked":false, "llvm_emitted":false,
        "launch_inputs_authenticated":false, "source_authority_in_report":false,
        "grants_artifact_or_launch_authority":false
    }) else {
        unreachable!("object literal")
    };
    frame.as_object_mut().unwrap().extend(target_fields);
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= FRAME_CAP);
    super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 private target-binding-stage observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_PRIVATE_TARGET_BOUND_V1 {}",
        std::str::from_utf8(&encoded).unwrap()
    );
    let final_deadline = timely(started).is_ok();
    drop(encoded);
    drop(frame);
    drop(body);
    copied_budget
        .release_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    assert!(
        collected && final_deadline,
        "private target binding refused; no ordinary/LLVM/launch qualification"
    );
}
