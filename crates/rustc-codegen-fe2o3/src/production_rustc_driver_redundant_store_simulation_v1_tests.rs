//! Finite independent u32 byte oracle over exact admitted J bytes, not a proof.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, LaunchDomain, LaunchExtent, ScalarType, Type,
    VerifiedCanonicalKernelIrV12, WorkgroupSize,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationKernelIrIdentityV1, SimulationLimitsV1, SimulationRaceAssessmentV1,
    SimulationRequestV1, SimulationTargetV1,
};
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
const GUARDS: usize = 4;
const CASES: [(usize, u32); 6] = [
    (0, 0),
    (1, 1),
    (63, u32::MAX),
    (64, 0x8000_0000),
    (65, 0x5555_aaaa),
    (129, 0xaaaa_5555),
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    identity: [u8; 32],
    canonical_bytes: u64,
    pub(super) scenarios: Vec<Scenario>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scenario {
    len: usize,
    input: u32,
    grid: [u64; 3],
    workgroup: [u32; 3],
    steps: u64,
    invocations: u64,
    checked_bytes: usize,
    replays: usize,
    conflicts_incomplete: bool,
    races_incomplete: bool,
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 4 * 1024 * 1024,
        max_reachable_functions: 64,
        max_reachable_operations: 32_768,
        max_invocations: 1_024,
        max_workgroups: 16,
        max_scheduled_slots: 4_096,
        max_steps: 4_000_000,
        max_call_depth: 16,
        max_ssa_values: 16_384,
        max_allocations: 16_384,
        max_allocation_bytes: 1024 * 1024,
        max_total_bytes: 16 * 1024 * 1024,
        max_resident_bytes: 128 * 1024 * 1024,
        max_events: 1,
        max_memory_access_records: 16_384,
    }
}
fn bytes(len: usize, input: u32) -> Vec<u8> {
    let mut output = vec![0xa5; (len + 2 * GUARDS) * 4];
    output[(len + GUARDS) * 4..].fill(0x5a);
    for cell in output[GUARDS * 4..(GUARDS + len) * 4].chunks_exact_mut(4) {
        cell.copy_from_slice(&input.to_le_bytes());
    }
    output
}
fn backing(len: usize, value: u32) -> Result<SharedBufferV1, String> {
    let data = bytes(len, value);
    let initialized = vec![true; data.len()];
    Ok(SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer: BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            data,
            initialized,
            TARGET,
        )
        .map_err(|e| format!("{e:?}"))?,
    })
}
fn grid(len: usize) -> [u64; 3] {
    [(len.max(1) as u64).div_ceil(64) * 64, 1, 1]
}

pub(super) fn observe(output: &VerifiedCanonicalKernelIrV12) -> Result<Report, String> {
    if output.canonical_bytes().len() > limits().max_canonical_bytes {
        return Err("actual J exceeds SIM cap".into());
    }
    // Decode only exact actual-J bytes for the simulator-owned copy. No graph
    // builder, optimizer rerun or other wire version substitutes for J.
    let copied =
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(output.canonical_bytes().to_vec())
            .map_err(|e| format!("{e:?}"))?;
    if copied.identity() != output.identity()
        || copied.canonical_bytes() != output.canonical_bytes()
    {
        return Err("SIM actual J byte/identity mismatch".into());
    }
    let identity = SimulationKernelIrIdentityV1::from(*copied.identity());
    let module =
        AdmittedSimulationModuleV1::admit_v12(copied, limits()).map_err(|e| format!("{e:?}"))?;
    if module.identity() != &identity || module.grants_execution_authority() {
        return Err("SIM changed J custody/authority".into());
    }
    let [kernel] = module.module().kernels.as_slice() else {
        return Err("SIM exact one J root".into());
    };
    let function = module
        .module()
        .function(&kernel.entry)
        .ok_or("SIM J entry")?;
    let [Type::Slice(slice), value] = function.signature.parameters.as_slice() else {
        return Err("SIM output/u32 ABI".into());
    };
    if kernel.id.as_str() != ROOT
        || slice.address_space != AddressSpace::Global
        || slice.access != AccessMode::ReadWrite
        || slice.element.as_ref() != &Type::Scalar(ScalarType::U32)
        || value != &Type::Scalar(ScalarType::U32)
        || !function.signature.results.is_empty()
        || !matches!(
            kernel.domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic
            }
        )
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
    {
        return Err("SIM actual J root/ABI/dynamic64 launch mismatch".into());
    }
    let mut scenarios = Vec::new();
    for (len, input) in CASES {
        let view = BufferViewArgumentV1::new(
            BufferBackingIdV1(0),
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            GUARDS * 4,
            len,
            TARGET,
        )
        .map_err(|e| format!("{e:?}"))?;
        let arguments = vec![
            SimulationArgumentV1::BufferView(view),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::U32, u128::from(input), TARGET)
                    .map_err(|e| format!("{e:?}"))?,
            ),
        ];
        let request = SimulationRequestV1::new(kernel.id.clone(), grid(len), [64, 1, 1], arguments)
            .with_shared_buffers(vec![backing(len, 0x37)?]);
        let original = request.clone();
        let first = module
            .simulate(&request, TARGET, limits())
            .map_err(|e| format!("len {len}, input {input:x}: {e:?}"))?;
        let second = module
            .simulate(&request, TARGET, limits())
            .map_err(|e| format!("replay len {len}: {e:?}"))?;
        if first != second
            || request != original
            || first.identity() != &identity
            || first.arguments() != request.arguments
            || first.shared_buffers() != [backing(len, input)?]
            || first.invocations_executed() != grid(len)[0]
        {
            return Err("SIM J differs from independent Rust oracle, canaries, inputs or deterministic replay".into());
        }
        let conflicts_incomplete = match first.conflict_assessment() {
            SimulationConflictAssessmentV1::NoConflictsObserved => false,
            SimulationConflictAssessmentV1::Incomplete {
                conflicting_bytes: 0,
                first: None,
                ..
            } => true,
            other => return Err(format!("SIM conflicting accesses: {other:?}")),
        };
        let races_incomplete = match first.race_assessment() {
            SimulationRaceAssessmentV1::NoRacesObserved { .. } => false,
            SimulationRaceAssessmentV1::Incomplete {
                racing_bytes: 0,
                first: None,
                ..
            } => true,
            other => return Err(format!("SIM racing accesses: {other:?}")),
        };
        scenarios.push(Scenario {
            len,
            input,
            grid: grid(len),
            workgroup: [64, 1, 1],
            steps: first.steps_executed(),
            invocations: first.invocations_executed(),
            checked_bytes: bytes(len, input).len(),
            replays: 2,
            conflicts_incomplete,
            races_incomplete,
        });
    }
    let report = Report {
        identity: *identity.digest(),
        canonical_bytes: identity.canonical_length(),
        scenarios,
    };
    validate(&report, *output.identity().digest())?;
    Ok(report)
}
pub(super) fn validate(report: &Report, expected: [u8; 32]) -> Result<(), String> {
    if report.identity != expected
        || report.canonical_bytes == 0
        || report.scenarios.len() != CASES.len()
    {
        return Err("SIM report J identity/extent/roster mismatch".into());
    }
    for (row, (len, input)) in report.scenarios.iter().zip(CASES) {
        if row.len != len
            || row.input != input
            || row.grid != grid(len)
            || row.workgroup != [64, 1, 1]
            || row.invocations != grid(len)[0]
            || row.checked_bytes != bytes(len, input).len()
            || row.steps == 0
            || row.replays != 2
        {
            return Err("SIM report lost exact launch/oracle/replay coverage".into());
        }
    }
    Ok(())
}

#[test]
fn policy7_smoke_u32_oracle_and_exact_launch_cover_boundary_and_high_bits() {
    for (len, input) in CASES {
        let result = bytes(len, input);
        assert!(result[..GUARDS * 4].iter().all(|b| *b == 0xa5));
        assert!(result[(len + GUARDS) * 4..].iter().all(|b| *b == 0x5a));
        for cell in result[GUARDS * 4..(len + GUARDS) * 4].chunks_exact(4) {
            assert_eq!(cell, input.to_le_bytes());
        }
    }
    assert_eq!(
        CASES.map(|(len, _)| grid(len)[0]),
        [64, 64, 64, 64, 128, 192]
    );
    let mut report = Report {
        identity: [1; 32],
        canonical_bytes: 1,
        scenarios: CASES
            .into_iter()
            .map(|(len, input)| Scenario {
                len,
                input,
                grid: grid(len),
                workgroup: [64, 1, 1],
                steps: 1,
                invocations: grid(len)[0],
                checked_bytes: bytes(len, input).len(),
                replays: 2,
                conflicts_incomplete: true,
                races_incomplete: true,
            })
            .collect(),
    };
    validate(&report, [1; 32]).unwrap();
    assert!(validate(&report, [2; 32]).is_err());
    for invalid in [[1, 1, 1], [128, 1, 1], [64, 2, 1]] {
        report.scenarios[0].grid = invalid;
        assert!(validate(&report, [1; 32]).is_err());
    }
}
