use super::*;
use crate::{
    RuntimeBackendAllocationOutcomeV1, RuntimeResourceKindV1 as K, RuntimeValidationErrorV1,
};

const ALLOCATION_QUIESCENT_SERVER: &str = r#"
import struct
import sys

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer
version = sys.argv[1]

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def read_frame():
    return read_exact(struct.unpack('<I', read_exact(4))[0])

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

def blob(value):
    return struct.pack('<I', len(value)) + value

def expect(request):
    if read_frame() != request:
        sys.exit(71)

handshake = b'fe2o3-runtime-worker-' + version.encode()
if version != 'v1':
    handshake += b';extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1'
if version == 'v5':
    handshake += b',semantic-launch-v1'
write_frame(handshake)
allocation = bytes((4,)) + struct.pack('<QBQQ', 1, 1, 8, 8)
failure = bytes((2,)) + blob(b'backend quiescent')
expect(allocation)
write_frame(failure)
expect(bytes((1,)))
write_frame(bytes((0,)) + struct.pack('<IQ', 1, 1) + blob(b'allocation-device') + blob(b'gfx942') + struct.pack('<QH', 1024, 10))
if version != 'v1':
    expect(bytes((19,)) + struct.pack('<Q', 1))
    write_frame(bytes((0,)) + struct.pack('<IH', 2, 0))
expect(allocation)
write_frame(failure)
# No retry or allocation release may reach the worker after Context quarantine.
expect(bytes((2,)) + struct.pack('<Q', 1))
write_frame(bytes((0,)) + struct.pack('<Q', 41))
expect(bytes((3,)) + struct.pack('<Q', 41))
write_frame(bytes((0,)))
try:
    sys.exit(0 if not read_frame() else 72)
except EOFError:
    sys.exit(0)
"#;

fn command(version: &str) -> RuntimeWorkerCommandV1 {
    RuntimeWorkerCommandV1::new("python3")
        .argument("-u")
        .argument("-c")
        .argument(ALLOCATION_QUIESCENT_SERVER)
        .argument(version)
}

fn exercise<B: RuntimeBackendV1>(mut backend: B) {
    assert!(matches!(
        backend.allocate_with_outcome_v1(1, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    let mut context = RuntimeContextV1::open(backend).unwrap();
    let device = context.devices()[0].id();
    context
        .configure_allocation_admission_v1(device, 8, 1)
        .unwrap();
    assert!(matches!(
        context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeErrorV1::BackendQuiescent(_))
    ));
    assert!(!context.is_terminal());
    let usage = context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(usage.used.get(K::RequestedAllocationBytes), 8);
    assert_eq!(usage.used.get(K::AllocationRecords), 1);
    assert_eq!(
        (
            usage.reserved_records,
            usage.retained_records,
            usage.quarantined_records
        ),
        (0, 0, 1)
    );
    assert!(matches!(
        context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert!(
        context
            .configure_allocation_admission_v1(device, 16, 2)
            .is_err()
    );
    assert_eq!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap(),
        usage
    );
    let report = context.cleanup();
    assert!(!report.is_complete());
    assert_eq!(report.allocation_credit_records_v1(), 1);
    assert!(report.retained().is_empty());
    assert!(report.failures().is_empty());
    assert!(!report.is_terminal());
    let failure = match context.shutdown() {
        Ok(_) => panic!("quarantined credit cannot disappear through shutdown"),
        Err(failure) => failure,
    };
    assert_eq!(failure.report().allocation_credit_records_v1(), 1);
    assert!(failure.report().retained().is_empty());
    assert!(failure.report().failures().is_empty());
    assert!(!failure.report().is_terminal());
    let mut context = failure.into_context();
    let stream = context.create_stream(device).unwrap();
    context.destroy_stream(stream).unwrap();
    assert!(!context.is_terminal());
    assert_eq!(
        context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap(),
        usage
    );
    // Normal transport Drop owns termination/reaping, not successful Context shutdown.
    drop(context);
}

#[test]
fn v1_allocation_quiescence_keeps_context_credit_quarantined_over_child_process() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let backend = RuntimeWorkerBackendV1::spawn(
        &command("v1"),
        RuntimeBinaryCodecV1,
        Duration::from_secs(5),
        Duration::from_secs(5),
    )
    .unwrap();
    #[cfg(target_os = "linux")]
    let pid = backend.transport.child.id();
    exercise(backend);
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

#[test]
fn v4_allocation_quiescence_keeps_context_credit_quarantined_over_child_process() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let backend = RuntimeWorkerBackendV4::spawn(
        &command("v4"),
        RuntimeBinaryCodecV4,
        Duration::from_secs(5),
        Duration::from_secs(5),
    )
    .unwrap();
    #[cfg(target_os = "linux")]
    let pid = backend.inner.transport.child.id();
    exercise(backend);
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

#[test]
fn v5_allocation_quiescence_keeps_context_credit_quarantined_over_child_process() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let backend = RuntimeWorkerBackendV5::spawn(
        &command("v5"),
        RuntimeBinaryCodecV5,
        Duration::from_secs(5),
        Duration::from_secs(5),
    )
    .unwrap();
    #[cfg(target_os = "linux")]
    let pid = backend.inner.inner.transport.child.id();
    exercise(backend);
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

#[test]
fn allocation_dispatchers_erase_settlement_authority_to_legacy_quiescence() {
    let mut backend = ProtocolBackendV1 {
        settled_allocation: true,
        ..Default::default()
    };
    assert!(matches!(
        backend.allocate_with_outcome_v1(1, RuntimeMemoryKindV1::DeviceLocal, 8, 8),
        Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(_))
    ));
    backend.calls.clear();
    let request = RuntimeBinaryCodecV1
        .encode_request_v1(RuntimeWorkerOperationV1::Allocate {
            device: 1,
            kind: RuntimeMemoryKindV1::DeviceLocal,
            byte_len: 8,
            alignment: 8,
        })
        .unwrap();
    let message = b"backend quiescent";
    let mut expected = vec![2];
    expected.extend_from_slice(&(message.len() as u32).to_le_bytes());
    expected.extend_from_slice(message);
    for response in [
        dispatch_binary_request_v1(&mut backend, &request),
        dispatch_binary_request_v4(&mut backend, &request),
        dispatch_binary_request_v5(&mut backend, &request),
    ] {
        assert_eq!(response.unwrap(), expected);
    }
    assert_eq!(backend.calls, ["allocate_legacy_quiescent"; 3]);
}
