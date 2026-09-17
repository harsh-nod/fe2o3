#![cfg(target_os = "linux")]

use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    Gfx942ScalarF32RecurrenceStepAnalysisErrorV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, check_authenticated_gfx942_scalar_f32_recurrence_step_v1,
    inspect_physical_machine_effect_worker_candidate_v1,
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

#[test]
fn authenticated_fixture_cannot_replace_instruction_bytes_or_implicit_effects() {
    // The shared fixture labels arbitrary payload bytes as VOP instructions and reports no
    // implicit EXEC/MODE uses. Authenticating that process does not authenticate those claims.
    // Valid encodings, operand/effect mutations, dataflow and artifact round trips are exercised
    // by the private checker's unit tests without constructing authenticated execution custody.
    for mode in 27..=34 {
        let failure = check_authenticated_gfx942_scalar_f32_recurrence_step_v1(
            execution(mode),
            "scalar_gemm_v1",
        )
        .unwrap_err();
        let expected = match mode {
            28 => Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongMultiplyCount { actual: 0 },
            31 | 34 => Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: 8,
            },
            _ => Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: 4,
            },
        };
        assert_eq!(failure.error(), &expected, "mode {mode}");
        let (execution, _) = failure.into_parts();
        assert!(execution.authenticates_analyzer_execution());
    }
}
