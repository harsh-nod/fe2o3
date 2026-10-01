//! Independent numerical and complete backing-buffer oracle for the repeat lane.
use super::*;
use fe2o3_kernel_ir::{AccessMode, ScalarType};
use fe2o3_kir_sim::*;

/// No descriptor evaluation, simulated result, publisher, or source text is
/// consulted to compute this independent expected kernel result.
fn expected(n: u8, change: bool, [a, b, c]: [u32; 3]) -> u32 {
    assert!(matches!(n, 1 | 2 | 15));
    if change {
        c
    } else {
        a.wrapping_add(b.wrapping_mul(u32::from(n)))
    }
}
fn check_output(
    bytes: &[u8],
    initialized: &[bool],
    length: usize,
    grid: u64,
    value: u32,
) -> Result<(), &'static str> {
    if !matches!(length, 0 | 13 | 64 | 129) || !matches!(grid, 64 | 128) {
        return Err("closed output profile");
    }
    let expected_len = (length + 4) * 4;
    if bytes.len() != expected_len || initialized.len() != expected_len {
        return Err("backing length");
    }
    let written = length.min(grid as usize);
    for element in 0..length + 4 {
        let range = element * 4..(element + 1) * 4;
        if (2..2 + written).contains(&element) {
            if bytes[range.clone()] != value.to_le_bytes() {
                return Err("written value");
            }
            if !initialized[range].iter().all(|v| *v) {
                return Err("written initialization");
            }
        } else {
            if bytes[range.clone()] != [0x5a; 4] {
                return Err("untouched canary");
            }
            if initialized[range].iter().any(|v| *v) {
                return Err("untouched initialization");
            }
        }
    }
    Ok(())
}
pub(super) fn observe(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17,
    n: u8,
    change: bool,
    started: std::time::Instant,
) -> Value {
    let limits = SimulationLimitsV1 {
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
    .validate()
    .unwrap();
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner, limits).unwrap();
    let machine = SimulationTargetV1::amdgpu_64();
    let mut cases = 0;
    let mut wrong_expectations_refused = 0;
    let mut hash = Sha256::new();
    for inputs in [
        [0_u32, 0, 0],
        [0, u32::MAX, 1],
        [u32::MAX, 1, 7],
        [19, 23, 42],
    ] {
        for length in [0_usize, 13, 64, 129] {
            for grid in [64_u64, 128] {
                timely(started.elapsed(), 300).unwrap();
                let backing = BufferBackingIdV1(7);
                let buffer = BufferArgumentV1::new(
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    vec![0x5a; (length + 4) * 4],
                    vec![false; (length + 4) * 4],
                    machine,
                )
                .unwrap();
                let view = BufferViewArgumentV1::new(
                    backing,
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    8,
                    length,
                    machine,
                )
                .unwrap();
                let request = SimulationRequestV1::new(
                    owner.module().kernels[0].id.clone(),
                    [grid, 1, 1],
                    [64, 1, 1],
                    vec![
                        SimulationArgumentV1::BufferView(view),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(inputs[0])),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(inputs[1])),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(inputs[2])),
                    ],
                )
                .with_shared_buffers(vec![SharedBufferV1 {
                    id: backing,
                    buffer,
                }]);
                let unchanged = request.clone();
                let execution = admitted.simulate(&request, machine, limits).unwrap();
                assert_eq!(request, unchanged);
                assert!(!execution.grants_execution_authority());
                assert_eq!(execution.identity().digest(), owner.identity().digest());
                assert_eq!(execution.identity().wire_version(), 17);
                let output = execution.shared_buffer(backing).unwrap();
                let value = expected(n, change, inputs);
                check_output(output.bytes(), output.initialized(), length, grid, value).unwrap();
                if length != 0 {
                    assert_eq!(
                        check_output(
                            output.bytes(),
                            output.initialized(),
                            length,
                            grid,
                            value.wrapping_add(1)
                        ),
                        Err("written value"),
                    );
                    wrong_expectations_refused += 1;
                }
                hash.update(output.bytes());
                for bit in output.initialized() {
                    hash.update([u8::from(*bit)]);
                }
                cases += 1;
                timely(started.elapsed(), 300).unwrap();
            }
        }
    }
    assert_eq!(cases, 32);
    assert_eq!(wrong_expectations_refused, 24);
    json!({
        "cases":cases,"wrong_expectations_refused":wrong_expectations_refused,
        "repeat_count":n,"edited_intent":change,"view_offset":8,
        "oracle":if change {"input_c"}else{"a.wrapping_add(b.wrapping_mul(N))"},
        "output_sha256":super::super::super::super::lower_hex_v1(&hash.finalize()),
        "canaries_and_initialization":true,
        "domain":"independent bounded CPU observer; original live source ledger retained",
        "native_or_physical_execution":false,
    })
}
#[test]
fn repeat_numerical_oracle_covers_wrapping_and_changing_intent() {
    assert_eq!(expected(1, false, [u32::MAX, 1, 7]), 0);
    assert_eq!(expected(2, false, [u32::MAX, 1, 7]), 1);
    assert_eq!(expected(15, false, [u32::MAX, 1, 7]), 14);
    assert_eq!(expected(15, false, [0, u32::MAX, 1]), u32::MAX - 14);
    for n in [1, 2, 15] {
        assert_eq!(expected(n, true, [19, 23, 41]), 41);
        assert_ne!(expected(n, false, [19, 23, 41]), 41);
    }
}
fn synthetic(value: u32) -> (Vec<u8>, Vec<bool>) {
    let mut bytes = vec![0x5a; 17 * 4];
    let mut init = vec![false; 17 * 4];
    for element in 2..15 {
        bytes[element * 4..(element + 1) * 4].copy_from_slice(&value.to_le_bytes());
        init[element * 4..(element + 1) * 4].fill(true);
    }
    (bytes, init)
}
#[test]
fn repeat_output_oracle_rejects_wrong_value_canary_initialization_and_shape() {
    let (bytes, init) = synthetic(65);
    assert_eq!(check_output(&bytes, &init, 13, 64, 65), Ok(()));
    assert_eq!(
        check_output(&bytes, &init, 13, 64, 66),
        Err("written value")
    );
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    assert_eq!(
        check_output(&corrupt, &init, 13, 64, 65),
        Err("untouched canary")
    );
    let mut corrupt = init.clone();
    corrupt[8] = false;
    assert_eq!(
        check_output(&bytes, &corrupt, 13, 64, 65),
        Err("written initialization")
    );
    let mut corrupt = init.clone();
    corrupt[0] = true;
    assert_eq!(
        check_output(&bytes, &corrupt, 13, 64, 65),
        Err("untouched initialization")
    );
    assert_eq!(
        check_output(&bytes[..bytes.len() - 1], &init, 13, 64, 65),
        Err("backing length")
    );
    assert_eq!(
        check_output(&bytes, &init, 12, 64, 65),
        Err("closed output profile")
    );
}
#[test]
fn repeat_output_oracle_checks_all_unlaunched_tail_and_empty_view_bytes() {
    let (mut bytes, mut init) = (vec![0x5a; 133 * 4], vec![false; 133 * 4]);
    for element in 2..66 {
        bytes[element * 4..(element + 1) * 4].copy_from_slice(&9_u32.to_le_bytes());
        init[element * 4..(element + 1) * 4].fill(true);
    }
    assert_eq!(check_output(&bytes, &init, 129, 64, 9), Ok(()));
    bytes[66 * 4] = 0;
    assert_eq!(
        check_output(&bytes, &init, 129, 64, 9),
        Err("untouched canary")
    );
    assert_eq!(check_output(&[0x5a; 16], &[false; 16], 0, 64, 99), Ok(()));
    let mut empty = [0x5a; 16];
    empty[8] = 0;
    assert_eq!(
        check_output(&empty, &[false; 16], 0, 64, 99),
        Err("untouched canary")
    );
}
