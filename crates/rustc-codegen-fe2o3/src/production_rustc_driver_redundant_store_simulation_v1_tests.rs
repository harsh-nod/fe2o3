//! Finite typed byte oracles over exact admitted J bytes, not a proof.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, LaunchDomain, LaunchExtent, Type, VerifiedCanonicalKernelIrV12,
    WorkgroupSize,
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
    case: Case,
    identity: [u8; 32],
    canonical_bytes: u64,
    pub(super) scenarios: Vec<Scenario>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scenario {
    len: usize,
    input: u128,
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
fn vectors(integer: Integer) -> [u128; 10] {
    let high = 1_u128 << (integer.width() - 1);
    let mask = (1_u128 << integer.width()) - 1;
    let alternating = 0xaaaa_aaaa_aaaa_aaaa_u128 & mask;
    [
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
    ]
}
fn scenarios(case: Case) -> Vec<(usize, u128)> {
    match case {
        Case::Smoke {} => CASES
            .into_iter()
            .map(|(len, input)| (len, u128::from(input)))
            .collect(),
        Case::Matrix { integer, .. } => vectors(integer)
            .into_iter()
            .flat_map(|input| [0, 1, 63, 64, 65, 129].map(|len| (len, input)))
            .collect(),
    }
}
fn rust_element(integer: Integer, input: u128) -> Vec<u8> {
    match integer {
        Integer::I8 => (input as i8).to_le_bytes().to_vec(),
        Integer::U8 => (input as u8).to_le_bytes().to_vec(),
        Integer::I16 => (input as i16).to_le_bytes().to_vec(),
        Integer::U16 => (input as u16).to_le_bytes().to_vec(),
        Integer::I32 => (input as i32).to_le_bytes().to_vec(),
        Integer::U32 => (input as u32).to_le_bytes().to_vec(),
        Integer::I64 => (input as i64).to_le_bytes().to_vec(),
        Integer::U64 => (input as u64).to_le_bytes().to_vec(),
    }
}
fn bytes(case: Case, len: usize, input: u128) -> Vec<u8> {
    let width = case.integer().width() as usize / 8;
    let element = rust_element(case.integer(), input);
    let mut output = vec![0xa5; (len + 2 * GUARDS) * width];
    output[(len + GUARDS) * width..].fill(0x5a);
    for cell in output[GUARDS * width..(GUARDS + len) * width].chunks_exact_mut(width) {
        cell.copy_from_slice(&element);
    }
    output
}
fn backing(case: Case, len: usize, value: u128) -> Result<SharedBufferV1, String> {
    let data = bytes(case, len, value);
    let initialized = vec![true; data.len()];
    Ok(SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer: BufferArgumentV1::new(
            case.integer().scalar(),
            AccessMode::ReadWrite,
            case.integer().width() / 8,
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

pub(super) fn observe(output: &VerifiedCanonicalKernelIrV12, case: Case) -> Result<Report, String> {
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
        return Err("SIM output/scalar ABI".into());
    };
    if kernel.id.as_str() != case.root()
        || slice.address_space != AddressSpace::Global
        || slice.access != AccessMode::ReadWrite
        || slice.element.as_ref() != &Type::Scalar(case.integer().scalar())
        || value != &Type::Scalar(case.integer().scalar())
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
    let width = case.integer().width() as usize / 8;
    for (len, input) in self::scenarios(case) {
        let view = BufferViewArgumentV1::new(
            BufferBackingIdV1(0),
            case.integer().scalar(),
            AccessMode::ReadWrite,
            width as u32,
            GUARDS * width,
            len,
            TARGET,
        )
        .map_err(|e| format!("{e:?}"))?;
        let arguments = vec![
            SimulationArgumentV1::BufferView(view),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(case.integer().scalar(), input, TARGET)
                    .map_err(|e| format!("{e:?}"))?,
            ),
        ];
        let request = SimulationRequestV1::new(kernel.id.clone(), grid(len), [64, 1, 1], arguments)
            .with_shared_buffers(vec![backing(case, len, 0x37)?]);
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
            || first.shared_buffers() != [backing(case, len, input)?]
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
            checked_bytes: bytes(case, len, input).len(),
            replays: 2,
            conflicts_incomplete,
            races_incomplete,
        });
    }
    let report = Report {
        case,
        identity: *identity.digest(),
        canonical_bytes: identity.canonical_length(),
        scenarios,
    };
    validate(&report, *output.identity().digest(), case)?;
    Ok(report)
}
pub(super) fn validate(report: &Report, expected: [u8; 32], case: Case) -> Result<(), String> {
    let expected_scenarios = scenarios(case);
    if report.case != case
        || report.identity != expected
        || report.canonical_bytes == 0
        || report.scenarios.len() != expected_scenarios.len()
    {
        return Err("SIM report J identity/extent/roster mismatch".into());
    }
    for (row, (len, input)) in report.scenarios.iter().zip(expected_scenarios) {
        if row.len != len
            || row.input != input
            || row.grid != grid(len)
            || row.workgroup != [64, 1, 1]
            || row.invocations != grid(len)[0]
            || row.checked_bytes != bytes(case, len, input).len()
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
        let result = bytes(Case::Smoke {}, len, u128::from(input));
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
        case: Case::Smoke {},
        identity: [1; 32],
        canonical_bytes: 1,
        scenarios: CASES
            .into_iter()
            .map(|(len, input)| Scenario {
                len,
                input: u128::from(input),
                grid: grid(len),
                workgroup: [64, 1, 1],
                steps: 1,
                invocations: grid(len)[0],
                checked_bytes: bytes(Case::Smoke {}, len, u128::from(input)).len(),
                replays: 2,
                conflicts_incomplete: true,
                races_incomplete: true,
            })
            .collect(),
    };
    validate(&report, [1; 32], Case::Smoke {}).unwrap();
    assert!(validate(&report, [2; 32], Case::Smoke {}).is_err());
    for invalid in [[1, 1, 1], [128, 1, 1], [64, 2, 1]] {
        report.scenarios[0].grid = invalid;
        assert!(validate(&report, [1; 32], Case::Smoke {}).is_err());
    }
}

#[test]
fn policy7_matrix_typed_oracles_keep_each_widths_high_bits_canaries_and_exact_launch() {
    for integer in Integer::ALL {
        let width = integer.width() as usize / 8;
        let values = vectors(integer);
        let mask = (1_u128 << integer.width()) - 1;
        assert_eq!(
            values
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            10
        );
        assert!(values.into_iter().all(|value| value <= mask));
        assert_eq!(rust_element(integer, mask), vec![0xff; width]);
        let mut high_bit = vec![0; width];
        high_bit[width - 1] = 0x80;
        assert_eq!(
            rust_element(integer, 1_u128 << (integer.width() - 1)),
            high_bit
        );
        for target in [Target::Gfx942, Target::Gfx950] {
            let case = Case::Matrix { integer, target };
            let cases = scenarios(case);
            assert_eq!(cases.len(), 60);
            for (len, input) in cases {
                let result = bytes(case, len, input);
                assert_eq!(result.len(), (len + 2 * GUARDS) * width);
                assert!(result[..GUARDS * width].iter().all(|b| *b == 0xa5));
                assert!(result[(len + GUARDS) * width..].iter().all(|b| *b == 0x5a));
                for cell in result[GUARDS * width..(len + GUARDS) * width].chunks_exact(width) {
                    assert_eq!(cell, rust_element(integer, input));
                }
            }
        }
    }
    assert_eq!(
        [0, 1, 63, 64, 65, 129].map(|len| grid(len)[0]),
        [64, 64, 64, 64, 128, 192]
    );
}

#[test]
fn policy7_matrix_reports_reject_wrong_case_type_target_roster_bits_and_launch() {
    let report = |case| Report {
        case,
        identity: [1; 32],
        canonical_bytes: 1,
        scenarios: scenarios(case)
            .into_iter()
            .map(|(len, input)| Scenario {
                len,
                input,
                grid: grid(len),
                workgroup: [64, 1, 1],
                steps: 1,
                invocations: grid(len)[0],
                checked_bytes: bytes(case, len, input).len(),
                replays: 2,
                conflicts_incomplete: true,
                races_incomplete: true,
            })
            .collect(),
    };
    for integer in Integer::ALL {
        for target in [Target::Gfx942, Target::Gfx950] {
            let case = Case::Matrix { integer, target };
            let valid = report(case);
            validate(&valid, [1; 32], case).unwrap();
            for other in [
                Case::Smoke {},
                Case::Matrix {
                    integer,
                    target: match target {
                        Target::Gfx942 => Target::Gfx950,
                        Target::Gfx950 => Target::Gfx942,
                    },
                },
                Case::Matrix {
                    integer: if integer == Integer::I8 {
                        Integer::U8
                    } else {
                        Integer::I8
                    },
                    target,
                },
            ] {
                assert!(validate(&valid, [1; 32], other).is_err());
            }
            for hostile in 0..10 {
                let mut changed = report(case);
                match hostile {
                    0 => {
                        changed.scenarios.pop();
                    }
                    1 => changed.scenarios.swap(0, 1),
                    2 => changed.scenarios[0].input = 1_u128 << integer.width(),
                    3 => changed.scenarios[0].grid = [1, 1, 1],
                    4 => changed.scenarios[0].grid = [128, 1, 1],
                    5 => changed.scenarios[0].workgroup = [32, 1, 1],
                    6 => changed.scenarios[0].checked_bytes -= 1,
                    7 => changed.scenarios[0].replays = 1,
                    8 => changed.scenarios[0].invocations = 0,
                    9 => changed.scenarios[0].steps = 0,
                    _ => unreachable!(),
                }
                assert!(
                    validate(&changed, [1; 32], case).is_err(),
                    "hostile {hostile}"
                );
            }
        }
    }
}
