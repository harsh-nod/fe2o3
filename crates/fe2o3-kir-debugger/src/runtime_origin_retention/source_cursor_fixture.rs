//! Fresh-input acceptance helper, not a second source producer.
use super::*;
use crate::{DebugWaveWidthV1, DebuggerLimitsV1};
use fe2o3_kernel_ir::{
    AccessMode, ScalarType, VerifiedCanonicalKernelIrV11, VerifiedSimulationBundleV6,
};
use fe2o3_kir_sim::*;
use std::io::Read;

pub(super) const CAP: usize = 256 * 1024;
pub(super) const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

pub(super) fn input_parameters(path: &str, identity: &str) -> Result<[u8; 32], &'static str> {
    if !std::path::Path::new(path).is_absolute()
        || path.len() > 4096
        || path.chars().any(char::is_control)
        || identity.len() != 64
        || !identity
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("absolute bundle path and lowercase model identity required");
    }
    let mut expected = [0; 32];
    for (index, byte) in expected.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&identity[index * 2..index * 2 + 2], 16).map_err(|_| "identity")?;
    }
    Ok(expected)
}

pub(super) fn read(path: &str) -> Vec<u8> {
    assert!(
        std::fs::symlink_metadata(path)
            .expect("bundle stat")
            .is_file(),
        "regular nonsymlink bundle required"
    );
    let file = std::fs::File::open(path).expect("bundle open");
    let metadata = file.metadata().expect("opened bundle metadata");
    assert!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= CAP as u64,
        "bundle input cap"
    );
    let mut bytes = Vec::new();
    file.take((CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .expect("bounded bundle read");
    assert!(!bytes.is_empty() && bytes.len() <= CAP, "bundle bytes cap");
    bytes
}

pub(super) fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: CAP,
        max_reachable_functions: 8,
        max_reachable_operations: 256,
        max_invocations: 4,
        max_workgroups: 1,
        max_scheduled_slots: 64,
        max_steps: 4096,
        max_call_depth: 8,
        max_ssa_values: 256,
        max_allocations: 16,
        max_allocation_bytes: 4096,
        max_total_bytes: 16 * 1024,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 8192,
        max_memory_access_records: 256,
    }
}

pub(super) fn admit(bundle: &VerifiedSimulationBundleV6) -> AdmittedSimulationModuleV1 {
    bundle.revalidate().unwrap();
    assert_eq!(bundle.target(), "gfx942:xnack-");
    assert_eq!(bundle.kernel_count(), 1);
    let owner =
        VerifiedCanonicalKernelIrV11::from_canonical_bytes(bundle.canonical_kir_v11().to_vec())
            .unwrap();
    AdmittedSimulationModuleV1::admit_v11(owner, limits()).unwrap()
}

pub(super) fn request(rounds: u32) -> SimulationRequestV1 {
    let values = [
        0xdeadbeef, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xcafebabe,
    ]
    .map(ScalarBitsV1::u32);
    let buffer = BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &values, TARGET).unwrap();
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        4,
        TARGET,
    )
    .unwrap();
    SimulationRequestV1::new(
        "loop_helper",
        [4, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(view),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0xabcd1234)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0x0f0f55aa)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(rounds)),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: BufferBackingIdV1(0),
        buffer,
    }])
}

pub(super) fn expected(rounds: u32) -> u32 {
    // Deliberately independent of the source fixture's admitted modulo-four.
    let mut value = 0xabcd1234;
    for iteration in 0..(rounds & 3) {
        value = (value ^ (0x0f0f55aa ^ iteration)) & 0xffff;
    }
    value
}

pub(super) fn run(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    seeded: bool,
    enabled: bool,
) -> (SimulationExecutionV1, ObservedTranscript) {
    let mut collector = OriginCollector::new(
        DebuggerLimitsV1::new(4096, 131_072, 1024 * 1024).unwrap(),
        enabled.then(|| fixtures::origin_limits(4096)),
    );
    let schedule = if seeded {
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 71,
            max_decisions: 4096,
        }
    } else {
        SimulationScheduleRequestV1::RecordCanonical {
            max_decisions: 4096,
        }
    };
    let execution = module
        .simulate_debugged_scheduled_with_sink(
            request,
            TARGET,
            limits(),
            schedule,
            SimulationDebugCaptureLimitsV1::new(4, 32, 1, 24).unwrap(),
            &mut collector,
        )
        .expect("source execution");
    let transcript = collector.finish(fixtures::identity(module), DebugWaveWidthV1::Wave64, None);
    assert!(!transcript.transcript.records().is_empty());
    assert_eq!(
        transcript.transcript.completeness(),
        crate::DebugTranscriptCompletenessV1::Complete
    );
    (execution, transcript)
}

pub(super) fn check_outputs(
    execution: &SimulationExecutionV1,
    request: &SimulationRequestV1,
    word: u32,
) {
    assert_eq!(execution.arguments(), request.arguments);
    assert_eq!(execution.invocations_executed(), 4);
    assert_eq!(execution.workgroups_visited(), 1);
    assert_eq!(execution.scheduled_slots_visited(), 64);
    assert_eq!(execution.shared_buffers().len(), 1);
    let buffer = execution.shared_buffer(BufferBackingIdV1(0)).unwrap();
    let wanted: Vec<_> = [0xdeadbeef, word, word, word, word, 0xcafebabe]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(buffer.bytes(), wanted);
    assert_eq!(buffer.initialized(), [true; 24]);
}

#[test]
fn fresh_input_parser_does_not_accept_relative_paths_or_identity_substitution_shapes() {
    let id = "1".repeat(64);
    assert!(input_parameters("/tmp/fresh.fe2sim", &id).is_ok());
    for path in ["relative.fe2sim", "/tmp/a\nb"] {
        assert!(input_parameters(path, &id).is_err());
    }
    for bad in [String::new(), "abcd".into(), "A".repeat(64), "g".repeat(64)] {
        assert!(input_parameters("/tmp/fresh.fe2sim", &bad).is_err());
    }
    assert!(input_parameters(&format!("/{}", "a".repeat(4096)), &id).is_err());
}
