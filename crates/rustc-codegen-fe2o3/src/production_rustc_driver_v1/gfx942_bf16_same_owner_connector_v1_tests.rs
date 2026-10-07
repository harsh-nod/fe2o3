//! Separately ignored source acceptance for the non-test same-owner connector.
//! Reuses the existing generated-session configuration and bounded input reader.
//! No Worker/native endpoint, new transport, or default command is selected.
use super::*;

struct SameOwnerBody<'a> {
    config: &'a Config,
    record: &'a inputs::Record,
    wrong_return: bool,
    calls: usize,
    completed: bool,
    expected_refusal: bool,
    failure: Option<String>,
}

fn is_source_profile_refusal(error: &crate::production_pipeline::ProductionPipelineError) -> bool {
    matches!(error,
    crate::production_pipeline::ProductionPipelineError::Bf16TileValuesInspection(inner)
    if matches!(inner.as_ref(),
        fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1::Unavailable(
            "BF16 same-owner source profile"
        )))
}

impl Callbacks for SameOwnerBody<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        if self.calls != 1 {
            self.failure = Some("more than one same-owner source callback".into());
            return Compilation::Stop;
        }
        let source_order = owning_requested_permutation(self.config.session).unwrap();
        let requested = if self.wrong_return {
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
                let result =
                    transaction.prepare_bf16_same_owner_handoff_v1(requested, |source, budget| {
                        use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
                        budget.charge_work(256)?;
                        if source.source().bytes().len() as u64 != self.record.spec.source.bytes
                            || super::super::lower_hex_v1(source.source().sha256())
                                != self.record.spec.source.sha256
                            || source.relation().return_permutation() != source_order
                        {
                            return Err(E::Unavailable("same-owner actual source/Return differs"));
                        }
                        Ok(())
                    });
                match result {
                    Ok(handoff) => {
                        let denied = !handoff.grants_artifact_or_launch_authority();
                        drop(handoff);
                        if denied && !self.wrong_return {
                            self.completed = true;
                        } else {
                            self.failure = Some("unexpected same-owner admission/authority".into());
                        }
                    }
                    Err(error) if self.wrong_return && is_source_profile_refusal(&error) => {
                        self.expected_refusal = true;
                    }
                    Err(error) => self.failure = Some(diagnostic(&error)),
                }
            }
            Err(error) => self.failure = Some(diagnostic(&error)),
        }
        Compilation::Stop
    }
}

fn run_same_owner_source(wrong_return: bool) {
    let started = Instant::now();
    let config: Config = read_config().expect("closed generated session config");
    assert!(matches!(config.session, 1 | 3));
    let cwd = checked_config(&config).unwrap();
    let record = inputs::read_record(&cwd, &config.record, &config.record_sha256).unwrap();
    assert_eq!(record.spec.cwd, config.cwd);
    assert_eq!(record.spec.source.path, config.candidate);
    inputs::environment(&record).unwrap();
    // The same copied-input/output envelope as the existing source endpoint.
    // This is not a total rustc/RSS bound or a replacement materialization meter.
    let mut copied_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4_000_000);
    let mut copied_budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut copied_work,
        64 * 1024 + FRAME_CAP,
    );
    copied_budget
        .reserve_storage(64 * 1024 + FRAME_CAP)
        .unwrap();
    copied_budget.charge_work(1_000_000).unwrap();
    let mut body = SameOwnerBody {
        config: &config,
        record: &record,
        wrong_return,
        calls: 0,
        completed: false,
        expected_refusal: false,
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
    let sidecar_absent = inputs::absent_output(&cwd, &config.sidecar).is_ok();
    let accepted = compiler_clean
        && unchanged
        && deadline
        && analysis_empty
        && sidecar_absent
        && body.calls == 1
        && body.failure.is_none()
        && if wrong_return {
            body.expected_refusal && !body.completed
        } else {
            body.completed && !body.expected_refusal
        };
    let mut frame = json!({
        "schema":"fe2o3-bf16-same-owner-connector-test-v1",
        "session":config.session, "wrong_return_control":wrong_return,
        "record_sha256":config.record_sha256, "source_pin":record.spec.source,
        "actual_rustc_callbacks":body.calls, "compiler_clean":compiler_clean,
        "inputs_unchanged":unchanged, "deadline_met":deadline,
        "analysis_output_empty":analysis_empty, "sidecar_absent":sidecar_absent,
        "non_test_connector_completed":body.completed,
        "opaque_handoff_dropped":body.completed,
        "expected_source_profile_refusal":body.expected_refusal,
        "accepted":accepted, "failure":body.failure,
        "worker_invoked":false, "hardware_observed":false,
        "normal_admission":false, "formal_admission":false,
        "grants_artifact_or_launch_authority":false
    });
    integral_strings(&mut frame, 0, &mut 0).unwrap();
    let encoded = serde_json::to_vec(&frame).unwrap();
    assert!(encoded.len() <= 16 * 1024 && encoded.len() <= FRAME_CAP);
    super::super::publish_new_inert_output(
        &cwd.join(&config.observation),
        &encoded,
        FRAME_CAP,
        "BF16 non-test same-owner connector observation",
    )
    .unwrap();
    println!(
        "\nFE2O3_BF16_SAME_OWNER_CONNECTOR_V1 {}",
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
        accepted && final_deadline,
        "same-owner connector source acceptance failed"
    );
}

#[test]
#[ignore = "fresh authenticated Identity or Swap01 source; root owns finite process/input supervision; no Worker"]
fn actual_generated_same_owner_connector_source() {
    run_same_owner_source(false);
}

#[test]
#[ignore = "fresh authenticated source and output; exact opposite Return refusal; no Worker"]
fn actual_generated_same_owner_connector_wrong_return() {
    run_same_owner_source(true);
}

#[test]
fn wrong_return_requires_the_exact_typed_source_profile_refusal() {
    use crate::production_pipeline::ProductionPipelineError as P;
    use fe2o3_lower_mir_kernel::Bf16CallInstanceErrorV1 as E;
    assert!(is_source_profile_refusal(&P::Bf16TileValuesInspection(
        Box::new(E::Unavailable("BF16 same-owner source profile"))
    )));
    for message in [
        "",
        "same-owner actual source/Return differs",
        "BF16 source profile",
    ] {
        assert!(!is_source_profile_refusal(&P::Bf16TileValuesInspection(
            Box::new(E::Unavailable(message))
        )));
    }
}

#[test]
fn genuine_hook_uses_only_the_non_test_connector_and_existing_input_custody() {
    let source = include_str!("gfx942_bf16_same_owner_connector_v1_tests.rs");
    let body = source.split("#[test]").next().unwrap();
    for required in [
        "transaction.prepare_bf16_same_owner_handoff_v1(",
        "inputs::read_record(",
        "inputs::environment(&record)",
        "inputs::derive(&record.spec)",
        "drop(handoff)",
        "source.relation().return_permutation() != source_order",
    ] {
        assert!(body.contains(required), "{required}");
    }
    for forbidden in [
        "observe_bf16_owned_worker_for_test_v1(",
        "PinnedWorkerV1",
        "execute_v2(",
    ] {
        assert!(!body.contains(forbidden), "{forbidden}");
    }
}
