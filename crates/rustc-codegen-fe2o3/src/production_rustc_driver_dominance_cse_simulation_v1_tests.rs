//! Finite typed Rust oracle over exact final-I bytes, never a proof or producer.
use super::*;
use fe2o3_kernel_ir::{AccessMode, ScalarType, VerifiedCanonicalKernelIrV12};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationKernelIrIdentityV1, SimulationLimitsV1, SimulationRaceAssessmentV1,
    SimulationRequestV1, SimulationTargetV1,
};
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();
const GUARDS: usize = 4;
const LENGTHS: [usize; 6] = [0, 1, 63, 64, 65, 129];
const CHOICES: [u32; 2] = [0, u32::MAX];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    case: Case,
    identity: [u8; 32],
    canonical_bytes: u64,
    scenarios: Vec<Scenario>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    root: String,
    len: usize,
    lhs: u64,
    rhs: u64,
    choose: u32,
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
fn vectors(integer: Integer) -> Vec<(u64, u64)> {
    let mask = (1u128 << integer.width()) - 1;
    let high = 1u128 << (integer.width() - 1);
    let alternating = 0xaaaa_aaaa_aaaa_aaaau128 & mask;
    let values = [
        0,
        1,
        2,
        high - 1,
        high,
        high + 1,
        mask - 1,
        mask,
        alternating,
        alternating ^ mask,
    ];
    values
        .into_iter()
        .enumerate()
        .map(|(index, a)| (a as u64, values[(index + 3) % values.len()] as u64))
        .collect()
}
fn oracle(integer: Integer, ordinal: usize, lhs: u64, rhs: u64) -> Vec<u8> {
    macro_rules! typed {
        ($ty:ty) => {{
            let (lhs, rhs) = (lhs as $ty, rhs as $ty);
            match ordinal {
                0 => lhs & rhs,
                1 => lhs | rhs,
                2 => lhs ^ rhs,
                _ => panic!("closed opcode"),
            }
            .to_le_bytes()
            .to_vec()
        }};
    }
    match integer {
        Integer::I8 => typed!(i8),
        Integer::U8 => typed!(u8),
        Integer::I16 => typed!(i16),
        Integer::U16 => typed!(u16),
        Integer::I32 => typed!(i32),
        Integer::U32 => typed!(u32),
        Integer::I64 => typed!(i64),
        Integer::U64 => typed!(u64),
    }
}
fn bytes(len: usize, value: &[u8]) -> Vec<u8> {
    let width = value.len();
    let mut result = vec![0xa5; (len + 2 * GUARDS) * width];
    result[(len + GUARDS) * width..].fill(0x5a);
    for cell in result[GUARDS * width..(len + GUARDS) * width].chunks_exact_mut(width) {
        cell.copy_from_slice(value);
    }
    result
}
fn backing(integer: Integer, len: usize, value: &[u8]) -> Result<SharedBufferV1, String> {
    let data = bytes(len, value);
    let width = integer.width() / 8;
    if value.len() != width as usize {
        return Err("typed oracle width".into());
    }
    Ok(SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer: BufferArgumentV1::new(
            integer.scalar(),
            AccessMode::ReadWrite,
            width,
            data.clone(),
            vec![true; data.len()],
            TARGET,
        )
        .map_err(|e| format!("{e:?}"))?,
    })
}
fn grid(len: usize) -> [u64; 3] {
    [(len.max(1) as u64).div_ceil(64) * 64, 1, 1]
}

pub(super) fn observe(output: &VerifiedCanonicalKernelIrV12, case: Case) -> Result<Report, String> {
    if output.canonical_bytes().len() > limits().max_canonical_bytes {
        return Err("actual I exceeds bounded SIM input".into());
    }
    let copied =
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(output.canonical_bytes().to_vec())
            .map_err(|e| format!("{e:?}"))?;
    if copied.identity() != output.identity()
        || copied.canonical_bytes() != output.canonical_bytes()
    {
        return Err("SIM copied another I".into());
    }
    let identity = SimulationKernelIrIdentityV1::from(*copied.identity());
    let module =
        AdmittedSimulationModuleV1::admit_v12(copied, limits()).map_err(|e| format!("{e:?}"))?;
    if module.identity() != &identity || module.grants_execution_authority() {
        return Err("SIM I identity/authority".into());
    }
    graph::roster(module.module().kernels.iter().map(|k| k.id.as_str()))?;
    let width = (case.integer.width() / 8) as usize;
    let mut scenarios = Vec::new();
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        let (kernel, _, _) = graph::kernel(module.module(), case, root)?;
        for len in LENGTHS {
            for (lhs, rhs) in vectors(case.integer) {
                for choose in CHOICES {
                    let view = BufferViewArgumentV1::new(
                        BufferBackingIdV1(0),
                        case.integer.scalar(),
                        AccessMode::ReadWrite,
                        width as u32,
                        GUARDS * width,
                        len,
                        TARGET,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    let scalar = |ty, bits| {
                        ScalarBitsV1::new(ty, bits, TARGET)
                            .map(SimulationArgumentV1::Scalar)
                            .map_err(|e| format!("{e:?}"))
                    };
                    let arguments = vec![
                        SimulationArgumentV1::BufferView(view),
                        scalar(case.integer.scalar(), lhs as u128)?,
                        scalar(case.integer.scalar(), rhs as u128)?,
                        scalar(ScalarType::U32, choose as u128)?,
                    ];
                    let request = SimulationRequestV1::new(
                        kernel.id.clone(),
                        grid(len),
                        [64, 1, 1],
                        arguments,
                    )
                    .with_shared_buffers(vec![backing(
                        case.integer,
                        len,
                        &vec![0x37; width],
                    )?]);
                    let original = request.clone();
                    let first = module
                        .simulate(&request, TARGET, limits())
                        .map_err(|e| format!("{root}/{len}/{lhs:x}/{rhs:x}/{choose}: {e:?}"))?;
                    let second = module
                        .simulate(&request, TARGET, limits())
                        .map_err(|e| format!("replay {root}: {e:?}"))?;
                    let expected = oracle(case.integer, ordinal, lhs, rhs);
                    if first != second
                        || request != original
                        || first.identity() != &identity
                        || first.arguments() != request.arguments
                        || first.shared_buffers() != [backing(case.integer, len, &expected)?]
                        || first.invocations_executed() != grid(len)[0]
                    {
                        return Err("actual I differs from typed Rust/canaries/input immutability/twice replay".into());
                    }
                    let conflicts_incomplete = match first.conflict_assessment() {
                        SimulationConflictAssessmentV1::NoConflictsObserved => false,
                        SimulationConflictAssessmentV1::Incomplete {
                            conflicting_bytes: 0,
                            first: None,
                            ..
                        } => true,
                        other => return Err(format!("SIM conflict: {other:?}")),
                    };
                    let races_incomplete = match first.race_assessment() {
                        SimulationRaceAssessmentV1::NoRacesObserved { .. } => false,
                        SimulationRaceAssessmentV1::Incomplete {
                            racing_bytes: 0,
                            first: None,
                            ..
                        } => true,
                        other => return Err(format!("SIM race: {other:?}")),
                    };
                    scenarios.push(Scenario {
                        root: root.into(),
                        len,
                        lhs,
                        rhs,
                        choose,
                        grid: grid(len),
                        workgroup: [64, 1, 1],
                        steps: first.steps_executed(),
                        invocations: first.invocations_executed(),
                        checked_bytes: bytes(len, &expected).len(),
                        replays: 2,
                        conflicts_incomplete,
                        races_incomplete,
                    });
                }
            }
        }
    }
    let report = Report {
        case,
        identity: *identity.digest(),
        canonical_bytes: identity.canonical_length(),
        scenarios,
    };
    validate(
        &report,
        case,
        *output.identity().digest(),
        output.identity().canonical_length(),
    )?;
    Ok(report)
}
pub(super) fn validate(
    report: &Report,
    case: Case,
    identity: [u8; 32],
    canonical_bytes: u64,
) -> Result<(), String> {
    let count = ROOTS.len() * LENGTHS.len() * vectors(case.integer).len() * CHOICES.len();
    if report.case != case
        || report.identity != identity
        || canonical_bytes == 0
        || report.canonical_bytes != canonical_bytes
        || report.scenarios.len() != count
    {
        return Err("SIM exact I/case/scenario roster".into());
    }
    let mut actual = report.scenarios.iter();
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        for len in LENGTHS {
            for (lhs, rhs) in vectors(case.integer) {
                for choose in CHOICES {
                    let row = actual.next().ok_or("SIM missing scenario")?;
                    if row.root != root
                        || row.len != len
                        || row.lhs != lhs
                        || row.rhs != rhs
                        || row.choose != choose
                        || row.grid != grid(len)
                        || row.workgroup != [64, 1, 1]
                        || row.steps == 0
                        || row.invocations != grid(len)[0]
                        || row.replays != 2
                        || row.checked_bytes
                            != bytes(len, &oracle(case.integer, ordinal, lhs, rhs)).len()
                    {
                        return Err(
                            "SIM report lost exact typed oracle/launch/replay evidence".into()
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn dominance_typed_oracle_covers_each_width_sign_bit_and_canary() {
    for integer in Integer::ALL {
        let values = vectors(integer);
        assert_eq!(values.len(), 10);
        assert_eq!(
            values
                .iter()
                .map(|(a, _)| a)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            10
        );
        assert!(
            values
                .iter()
                .any(|(a, _)| *a == 1u64 << (integer.width() - 1))
        );
        for (a, b) in values {
            let width = integer.width() as usize / 8;
            for (op, bits) in [a & b, a | b, a ^ b].into_iter().enumerate() {
                assert_eq!(oracle(integer, op, a, b), bits.to_le_bytes()[..width]);
                let output = bytes(1, &oracle(integer, op, a, b));
                assert!(output[..GUARDS * width].iter().all(|b| *b == 0xa5));
                assert!(output[(GUARDS + 1) * width..].iter().all(|b| *b == 0x5a));
            }
        }
    }
    assert_eq!(LENGTHS.map(|len| grid(len)[0]), [64, 64, 64, 64, 128, 192]);
}

#[test]
fn dominance_sim_report_refuses_foreign_type_target_identity_or_launch() {
    let case = Case {
        integer: Integer::U32,
        target: Target::Gfx942,
    };
    let mut report = Report {
        case,
        identity: [7; 32],
        canonical_bytes: 123,
        scenarios: Vec::new(),
    };
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        for len in LENGTHS {
            for (lhs, rhs) in vectors(case.integer) {
                for choose in CHOICES {
                    report.scenarios.push(Scenario {
                        root: root.into(),
                        len,
                        lhs,
                        rhs,
                        choose,
                        grid: grid(len),
                        workgroup: [64, 1, 1],
                        steps: 1,
                        invocations: grid(len)[0],
                        checked_bytes: bytes(len, &oracle(case.integer, ordinal, lhs, rhs)).len(),
                        replays: 2,
                        conflicts_incomplete: true,
                        races_incomplete: true,
                    });
                }
            }
        }
    }
    validate(&report, case, [7; 32], 123).unwrap();
    assert!(validate(&report, case, [8; 32], 123).is_err());
    assert!(validate(&report, case, [7; 32], 0).is_err());
    assert!(validate(&report, case, [7; 32], 124).is_err());
    report.canonical_bytes = 122;
    assert!(validate(&report, case, [7; 32], 123).is_err());
    report.canonical_bytes = 123;
    assert!(
        validate(
            &report,
            Case {
                integer: Integer::I32,
                ..case
            },
            [7; 32],
            123
        )
        .is_err()
    );
    assert!(
        validate(
            &report,
            Case {
                target: Target::Gfx950,
                ..case
            },
            [7; 32],
            123
        )
        .is_err()
    );
    for grid in [[0, 1, 1], [1, 1, 1], [128, 1, 1], [64, 2, 1]] {
        report.scenarios[0].grid = grid;
        assert!(validate(&report, case, [7; 32], 123).is_err());
    }
    report.scenarios[0].grid = [64, 1, 1];
    report.scenarios.swap(0, 1);
    assert!(validate(&report, case, [7; 32], 123).is_err());
}
