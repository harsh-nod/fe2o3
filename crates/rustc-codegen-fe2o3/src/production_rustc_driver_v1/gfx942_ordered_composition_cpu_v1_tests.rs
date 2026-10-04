//! Independent CPU output oracle over the actual V17 composition owner.
use super::*;
use fe2o3_kernel_ir::{AccessMode, ScalarType, Type, VerifiedCanonicalKernelIrModuleV17};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationTargetV1,
};

const INPUTS: [[u32; 3]; 4] = [
    [0, 0, 0],
    [u32::MAX, u32::MAX, u32::MAX],
    [0, u32::MAX, 1],
    [0xa5a5_a5a5, 0x5a5a_5a5a, 0xf0f0_0f0f],
];
const LENGTHS: [usize; 8] = [0, 1, 13, 63, 64, 65, 128, 129];
const GRIDS: [u64; 2] = [64, 128];

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 256 * 1024,
        max_reachable_functions: 3,
        max_reachable_operations: 4096,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 500_000,
        max_call_depth: 2,
        max_ssa_values: 4096,
        max_allocations: 16,
        max_allocation_bytes: 65536,
        max_total_bytes: 65536,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 1_000_000,
        max_memory_access_records: 8192,
    }
}
/// Ordinary source mathematics, deliberately not descriptor/ISA evaluation.
fn oracle(feature: &str, [a, b, c]: [u32; 3]) -> u32 {
    let region = |x, y, z| (x ^ y) & z;
    let helper = |x, y, z| (region(x, y, z) ^ y) | z;
    match feature {
        "ordered-composition-root" => region(region(a, b, c) ^ c, b, a),
        "ordered-composition-helper" => helper(a, b, c),
        "ordered-composition-two-calls" => helper(helper(a, b, c) ^ c, b, a),
        "ordered-composition-root-helper" => helper(region(a, b, c) ^ c, b, a),
        "ordered-composition-const-monos" => b,
        "ordered-composition-scalar-helper" => region((a ^ b) & !c, b, c),
        "ordered-composition-wrapping" => region(a, b, c).wrapping_add(b).wrapping_sub(c),
        _ => panic!("CPU oracle is only defined for seven positive source fixtures"),
    }
}
pub(super) fn observe_cpu(
    owner: &VerifiedCanonicalKernelIrModuleV17,
    feature: &str,
    started: std::time::Instant,
) -> Value {
    // V17's existing simulator admission is a bounded inert re-encode domain,
    // NOT a reset or substitution of the authenticated source phase's ledger.
    let bounds = limits().validate().unwrap();
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner, bounds).unwrap();
    let target = SimulationTargetV1::amdgpu_64();
    let [kernel] = owner.module().kernels.as_slice() else {
        panic!("one actual kernel");
    };
    let function = owner
        .module()
        .functions
        .iter()
        .find(|f| f.id == kernel.entry)
        .unwrap();
    assert_eq!(function.signature.parameters.len(), 4);
    assert!(matches!(function.signature.parameters[0], Type::Slice(_)));
    assert!(
        function.signature.parameters[1..]
            .iter()
            .all(|ty| *ty == Type::Scalar(ScalarType::U32))
    );
    let before = digest(owner.canonical_bytes());
    let mut count = 0_usize;
    let mut total_steps = 0_u64;
    let mut evidence = Sha256::new();
    for inputs in INPUTS {
        for length in LENGTHS {
            for grid in GRIDS {
                timely(started.elapsed(), 300).unwrap();
                let backing = BufferBackingIdV1(7);
                let buffer = BufferArgumentV1::new(
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    vec![0x5a; (length + 4) * 4],
                    vec![false; (length + 4) * 4],
                    target,
                )
                .unwrap();
                let view = BufferViewArgumentV1::new(
                    backing,
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    8,
                    length,
                    target,
                )
                .unwrap();
                let mut arguments = vec![SimulationArgumentV1::BufferView(view)];
                arguments.extend(inputs.map(|n| {
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(ScalarType::U32, u128::from(n), target).unwrap(),
                    )
                }));
                let request = SimulationRequestV1::new(
                    kernel.id.clone(),
                    [grid, 1, 1],
                    [64, 1, 1],
                    arguments,
                )
                .with_shared_buffers(vec![SharedBufferV1 {
                    id: backing,
                    buffer,
                }]);
                let original = request.clone();
                let execution = admitted.simulate(&request, target, bounds).unwrap();
                assert_eq!(request, original);
                assert!(!execution.grants_execution_authority());
                assert_eq!(execution.identity().wire_version(), 17);
                assert_eq!(execution.identity().digest(), owner.identity().digest());
                let steps = execution.steps_executed();
                assert!(steps <= bounds.max_steps);
                total_steps = total_steps.checked_add(steps).unwrap();
                let expected = oracle(feature, inputs);
                let written = length.min(usize::try_from(grid).unwrap());
                let output = execution.shared_buffer(backing).unwrap();
                for element in 0..length + 4 {
                    let range = element * 4..(element + 1) * 4;
                    if (2..2 + written).contains(&element) {
                        assert_eq!(&output.bytes()[range.clone()], &expected.to_le_bytes());
                        assert!(output.initialized()[range].iter().all(|b| *b));
                    } else {
                        assert_eq!(&output.bytes()[range.clone()], &[0x5a; 4]);
                        assert!(output.initialized()[range].iter().all(|b| !*b));
                    }
                }
                for n in inputs {
                    evidence.update(n.to_le_bytes());
                }
                evidence.update((length as u64).to_le_bytes());
                evidence.update(grid.to_le_bytes());
                evidence.update(expected.to_le_bytes());
                evidence.update(output.bytes());
                for b in output.initialized() {
                    evidence.update([u8::from(*b)]);
                }
                count += 1;
                timely(started.elapsed(), 300).unwrap();
            }
        }
    }
    assert_eq!(count, 64);
    assert!(total_steps <= 64 * bounds.max_steps);
    assert_eq!(digest(owner.canonical_bytes()), before);
    json!({
        "cases":count, "grids":GRIDS, "output_lengths":LENGTHS, "inputs":INPUTS,
        "buffer_view_byte_offset":8, "front_canary_words":2, "back_canary_words":2,
        "untouched_bytes_and_initialization_checked":true, "request_unchanged":true,
        "steps_executed":total_steps, "output_observation_sha256":super::super::lower_hex_v1(&evidence.finalize()),
        "domain":"separate bounded CPU observation; not the source verification ledger",
        "max_canonical_bytes":bounds.max_canonical_bytes,
        "max_resident_bytes":bounds.max_resident_bytes, "max_steps_per_case":bounds.max_steps,
        "max_total_steps":64* bounds.max_steps,
        "resident_bound_is_predecode_allocator_or_process_rss_cap":false,
        "source_custody_from_bytes":false, "grants_execution_authority":false,
    })
}

#[test]
fn composition_oracle_covers_ordinary_transport_and_wrapping_overflow() {
    let input = [0, u32::MAX, 1];
    assert_eq!(oracle(FEATURES[0], input), 0);
    assert_eq!(oracle(FEATURES[1], input), u32::MAX);
    assert_eq!(oracle(FEATURES[2], input), u32::MAX);
    assert_eq!(oracle(FEATURES[3], input), u32::MAX);
    assert_eq!(oracle(FEATURES[4], input), u32::MAX);
    assert_eq!(oracle(FEATURES[5], input), 1);
    // (1 + MAX) wraps to 0; (0 - 1) wraps back to MAX.
    assert_eq!(oracle(FEATURES[6], input), u32::MAX);
    assert_eq!(INPUTS.len() * LENGTHS.len() * GRIDS.len(), 64);
}
#[test]
fn composition_cpu_limits_are_explicit_and_finite() {
    let bounds = limits().validate().unwrap();
    assert_eq!(bounds.max_reachable_functions, 3);
    assert_eq!(bounds.max_call_depth, 2);
    assert_eq!(bounds.max_invocations, 128);
    assert_eq!(bounds.max_total_bytes, 65536);
    assert_eq!(bounds.max_canonical_bytes, 256 * 1024);
}
