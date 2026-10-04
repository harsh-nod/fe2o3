#![cfg(target_os = "linux")]

use fe2o3_kernel_analysis::{
    AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    Gfx942ScalarF32RecurrenceStepAnalysisErrorV1, Gfx942ScalarF32RecurrenceStepArtifactErrorV1,
    Gfx942ScalarF32RecurrenceStepArtifactV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, check_authenticated_gfx942_scalar_f32_recurrence_step_v1,
    inspect_physical_machine_effect_worker_candidate_v1,
    verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1,
};
use std::{path::Path, sync::Mutex, time::Duration};

static WORKER_LOCK: Mutex<()> = Mutex::new(());

fn fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-machine-effect-worker-fixture"))
}

fn limits() -> AuthenticatedPhysicalMachineEffectLimitsV1 {
    AuthenticatedPhysicalMachineEffectLimitsV1::new(Duration::from_secs(30), 1024 * 1024, 16 * 1024)
        .unwrap()
}

fn worker() -> AuthenticatedPhysicalMachineEffectWorkerV1 {
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(fixture(), limits()).unwrap();
    AuthenticatedPhysicalMachineEffectWorkerV1::open(fixture(), candidate.policy(), limits())
        .unwrap()
}

fn execution(mode: u8) -> fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1 {
    let _guard = WORKER_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let payload_len = if mode == 34 { 20 } else { 16 };
    let mut payload = (0_u8..payload_len).collect::<Vec<_>>();
    payload[0] = mode;
    let entry = PhysicalMachineEffectEntryRequestV1::new(
        "scalar_gemm_v1",
        PhysicalMachineEffectBudgetV1::new(0, 0, 0, 1, 0),
    )
    .unwrap();
    worker().analyze(payload, vec![entry], limits()).unwrap()
}

fn positive() -> AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
    check_authenticated_gfx942_scalar_f32_recurrence_step_v1(execution(27), "scalar_gemm_v1")
        .unwrap()
}

#[test]
fn authenticated_recurrence_step_retains_exact_inert_artifact() {
    let checked = positive();
    let artifact = checked.artifact();
    assert_eq!(artifact.function_symbol(), "scalar_gemm_v1");
    assert_eq!(artifact.multiply_offset(), 4);
    assert_eq!(artifact.add_offset(), 8);
    assert_eq!(artifact.product_register(), 1);
    assert_eq!(artifact.accumulator_register(), 0);
    assert_eq!(artifact.result_register(), 0);
    assert_eq!(artifact.product_source_operand_index(), 0);
    assert_eq!(artifact.accumulator_source_operand_index(), 1);
    assert!(artifact.binds_authenticated_trace_and_exact_instruction_encodings());
    assert!(artifact.validates_separate_recurrence_step_dataflow_shape());
    assert!(artifact.excludes_fused_definitions_from_step_inputs());
    assert!(artifact.provides_executable_candidate_numeric_semantics());
    assert!(!artifact.establishes_machine_loop_recurrence());
    assert!(!artifact.establishes_gfx942_instruction_semantics());
    assert!(!artifact.establishes_compiler_refinement());
    assert!(!artifact.grants_worker_v3_refinement_authority());
    assert!(!artifact.grants_load_or_launch_authority());
    assert!(checked.authenticates_analyzer_execution());
    assert!(!checked.establishes_semantic_machine_refinement());
    assert!(!checked.grants_runtime_authority());

    let decoded =
        Gfx942ScalarF32RecurrenceStepArtifactV1::decode_canonical(artifact.canonical_bytes())
            .unwrap();
    assert_eq!(decoded, *artifact);
    assert_eq!(decoded.identity(), artifact.identity());
    assert_eq!(
        decoded.authenticated_execution_identity().0,
        &checked.authenticated_execution_identity().sha256()
    );
}

#[test]
fn persisted_artifact_requires_exact_authenticated_replay() {
    let checked = positive();
    let canonical = checked.artifact().canonical_bytes().to_vec();
    let execution = checked.into_authenticated_execution();
    let replayed = verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1(
        execution,
        "scalar_gemm_v1",
        &canonical,
    )
    .unwrap();
    assert_eq!(replayed.artifact().canonical_bytes(), canonical);

    let checked = positive();
    let mut mutated = checked.artifact().canonical_bytes().to_vec();
    let execution = checked.into_authenticated_execution();
    let final_byte = mutated.last_mut().unwrap();
    *final_byte ^= 1;
    let failure = verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1(
        execution,
        "scalar_gemm_v1",
        &mutated,
    )
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ArtifactMismatch
            | Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::Artifact(
                Gfx942ScalarF32RecurrenceStepArtifactErrorV1::InvalidField
            )
    ));
}

#[test]
fn opcode_dataflow_order_and_contraction_mutations_fail_named_obligations() {
    for (mode, expected) in [
        (28, "multiply-count"),
        (29, "product-dataflow"),
        (30, "fused-dataflow"),
        (31, "dominance"),
        (32, "operand-order"),
        (33, "accumulator-update"),
        (34, "fused-copy-dataflow"),
    ] {
        let failure = check_authenticated_gfx942_scalar_f32_recurrence_step_v1(
            execution(mode),
            "scalar_gemm_v1",
        )
        .unwrap_err();
        let matched = matches!(
            (expected, failure.error()),
            (
                "multiply-count",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongMultiplyCount { actual: 0 },
            ) | (
                "product-dataflow",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductDoesNotReachAdd,
            ) | (
                "fused-dataflow",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd { .. },
            ) | (
                "fused-copy-dataflow",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd {
                    offset: 0,
                },
            ) | (
                "operand-order",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductOperandPosition { actual: 1 },
            ) | (
                "accumulator-update",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ResultDoesNotUpdateAccumulator {
                    result: 7,
                    accumulator: 0,
                },
            ) | (
                "dominance",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::MultiplyDoesNotDominateAdd,
            )
        );
        assert!(matched, "mode {mode} did not fail {expected}: {failure:?}");
        let (execution, _) = failure.into_parts();
        assert!(execution.authenticates_analyzer_execution());
    }
}
