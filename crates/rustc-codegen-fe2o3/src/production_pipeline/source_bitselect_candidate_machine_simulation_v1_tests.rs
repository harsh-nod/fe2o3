//! Whole-kernel simulation directly borrows the actual fresh V17 source owner.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, LaunchDomain, LaunchExtent, VerifiedCanonicalKernelIrModuleV17,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationKernelIrIdentityV1, SimulationLimitsV1, SimulationRaceAssessmentV1,
    SimulationRequestV1, SimulationTargetV1,
};
use serde::Serialize;

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
const BYTE_CAP: usize = 64 * 1024;
const MAX_RUNS: usize = 30;
const MAX_STEPS: u64 = 4_000_000;

#[derive(Default, Serialize)]
struct OracleLedger {
    runs: usize,
    steps: u64,
    prepaid_host_payload: usize,
}
impl OracleLedger {
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let total = self
            .prepaid_host_payload
            .checked_add(bytes)
            .ok_or("source-candidate machine oracle payload overflow")?;
        if total > 2 * 1024 * 1024 {
            return Err("source-candidate machine oracle host payload limit".into());
        }
        self.prepaid_host_payload = total;
        Ok(())
    }
    fn next(&mut self) -> Result<SimulationLimitsV1, String> {
        if self.runs >= MAX_RUNS || self.steps >= MAX_STEPS {
            return Err("source-candidate machine cumulative oracle limit".into());
        }
        self.runs += 1;
        let mut limits = limits();
        limits.max_steps = limits.max_steps.min(MAX_STEPS - self.steps);
        Ok(limits)
    }
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: BYTE_CAP,
        max_reachable_functions: 1,
        max_reachable_operations: 4096,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 512,
        max_steps: 250_000,
        max_call_depth: 1,
        max_ssa_values: 4096,
        max_allocations: 1024,
        max_allocation_bytes: 4096,
        max_total_bytes: 1024 * 1024,
        max_resident_bytes: 32 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 4096,
    }
}

fn backing(
    len: usize,
    value: u32,
    initialized: bool,
) -> Result<(SimulationArgumentV1, SharedBufferV1), String> {
    let mut bytes = vec![0u8; (len + 2) * 4];
    bytes[..4].copy_from_slice(&0xdead_beefu32.to_le_bytes());
    bytes[(len + 1) * 4..].copy_from_slice(&0x1357_2468u32.to_le_bytes());
    for element in bytes[4..(len + 1) * 4].chunks_exact_mut(4) {
        element.copy_from_slice(&value.to_le_bytes());
    }
    let mut initialized_bytes = vec![true; bytes.len()];
    initialized_bytes[4..(len + 1) * 4].fill(initialized);
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        bytes,
        initialized_bytes,
        TARGET,
    )
    .map_err(|e| e.to_string())?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        len,
        TARGET,
    )
    .map_err(|e| e.to_string())?;
    Ok((
        SimulationArgumentV1::BufferView(view),
        SharedBufferV1 {
            id: BufferBackingIdV1(0),
            buffer,
        },
    ))
}

pub(super) fn observe(owner: &VerifiedCanonicalKernelIrModuleV17) -> Result<Value, String> {
    let mut ledger = OracleLedger::default();
    let bytes = owner.canonical().canonical_bytes();
    if bytes.len() > BYTE_CAP {
        return Err("source-candidate machine oracle canonical byte limit".into());
    }
    ledger.charge(
        bytes
            .len()
            .checked_mul(2)
            .ok_or("source-candidate machine oracle byte overflow")?,
    )?;
    // Existing V17 API borrows this actual owner; it does not recreate source
    // custody from a diagnostic or project through an earlier wire version.
    let identity = SimulationKernelIrIdentityV1::from(*owner.identity());
    let module =
        AdmittedSimulationModuleV1::admit_v17(owner, limits()).map_err(|e| e.to_string())?;
    if module.identity() != &identity || module.grants_execution_authority() {
        return Err("source-candidate machine simulator identity or authority changed".into());
    }
    let [kernel] = module.module().kernels.as_slice() else {
        return Err("source-candidate machine oracle root roster".into());
    };
    let entry = module
        .module()
        .function(&kernel.entry)
        .ok_or("source-candidate machine oracle entry missing")?;
    let [Type::Slice(output), a, b, mask] = entry.signature.parameters.as_slice() else {
        return Err(
            "source-candidate machine oracle requires output slice and three scalar inputs".into(),
        );
    };
    if output.address_space != AddressSpace::Global
        || output.access != AccessMode::ReadWrite
        || output.element.as_ref() != &Type::Scalar(ScalarType::U32)
        || [a, b, mask]
            .iter()
            .any(|ty| **ty != Type::Scalar(ScalarType::U32))
        || !entry.signature.results.is_empty()
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || !matches!(
            kernel.domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic
            }
        )
    {
        return Err("source-candidate machine oracle source ABI or launch changed".into());
    }
    let before_runs = ledger.runs;
    let before_steps = ledger.steps;
    let mut incomplete = 0usize;
    for [a, b, mask] in [
        [0, u32::MAX, 0],
        [u32::MAX, 0, u32::MAX],
        [0xaaaa_aaaa, 0x5555_5555, 0x0f0f_0f0f],
        [0x8000_0000, 1, 0x8000_0000],
        [0x1234_5678, 0x8765_4321, 0x4444_4444],
    ] {
        for len in [0usize, 1, 65] {
            // Prepay fixed small request/clone/expected payload envelopes; model
            // internal allocation is separately controlled by SimulationLimits.
            ledger.charge(4096 + 16 * (len + 2) * 4)?;
            let (output, initial) = backing(len, 0x3141_5926, false)?;
            let expected = backing(len, (a & mask) | (b & !mask), true)?.1;
            let mut arguments = vec![output];
            arguments.extend(
                [a, b, mask].map(|value| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value))),
            );
            let grid = [((len.max(1) as u64).div_ceil(64) * 64), 1, 1];
            let request = SimulationRequestV1::new(kernel.id.clone(), grid, [64, 1, 1], arguments)
                .with_shared_buffers(vec![initial]);
            let before = request.clone();
            let execution = module
                .simulate(&request, TARGET, ledger.next()?)
                .map_err(|e| format!("source-candidate machine oracle execution: {e:?}"))?;
            ledger.steps = ledger
                .steps
                .checked_add(execution.steps_executed())
                .ok_or("source-candidate machine oracle step overflow")?;
            if execution.identity() != &identity
                || execution.arguments() != request.arguments
                || execution.invocations_executed() != grid[0]
                || execution.shared_buffers() != std::slice::from_ref(&expected)
            {
                return Err(
                    "source-candidate machine oracle output, initialization, canaries or identity mismatch"
                        .into(),
                );
            }
            match execution.conflict_assessment() {
                SimulationConflictAssessmentV1::NoConflictsObserved => {}
                SimulationConflictAssessmentV1::Incomplete {
                    conflicting_bytes: 0,
                    first: None,
                    ..
                } => incomplete += 1,
                other => {
                    return Err(format!(
                        "source-candidate machine observed conflict: {other:?}"
                    ));
                }
            }
            match execution.race_assessment() {
                SimulationRaceAssessmentV1::NoRacesObserved { .. } => {}
                SimulationRaceAssessmentV1::Incomplete {
                    racing_bytes: 0,
                    first: None,
                    ..
                } => incomplete += 1,
                other => return Err(format!("source-candidate machine observed race: {other:?}")),
            }
            let repeated = module
                .simulate(&request, TARGET, ledger.next()?)
                .map_err(|e| format!("source-candidate machine oracle replay: {e:?}"))?;
            ledger.steps = ledger
                .steps
                .checked_add(repeated.steps_executed())
                .ok_or("source-candidate machine oracle step overflow")?;
            if request != before || execution != repeated {
                return Err(
                    "source-candidate machine simulator mutated input or replay diverged".into(),
                );
            }
        }
    }
    Ok(
        json!({"scenarios":15,"runs":ledger.runs-before_runs,"steps":ledger.steps-before_steps,
        "output_and_canaries_checked":true,"immutable_inputs":true,"incomplete_assessments":incomplete,
        "oracle":"host_u32_masked_or_exact","simulator_version":"V17",
        "cumulative_step_limit":MAX_STEPS,"host_payload_limit":2*1024*1024,
        "prepaid_host_payload":ledger.prepaid_host_payload,
        "simulation_is_proof":false,"hardware_observed":false}),
    )
}

#[test]
fn source_candidate_machine_oracle_budget_is_cumulative_and_failed_charge_preserves_it() {
    let mut ledger = OracleLedger::default();
    ledger.charge(2 * 1024 * 1024).unwrap();
    assert!(ledger.charge(1).is_err());
    assert_eq!(ledger.prepaid_host_payload, 2 * 1024 * 1024);
    for _ in 0..MAX_RUNS {
        ledger.next().unwrap();
    }
    assert!(ledger.next().is_err());
    assert_eq!(ledger.runs, MAX_RUNS);
    let mut ledger = OracleLedger {
        steps: MAX_STEPS,
        ..Default::default()
    };
    assert!(ledger.next().is_err());
    assert_eq!(ledger.runs, 0);
}
