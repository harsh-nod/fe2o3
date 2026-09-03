#![cfg(all(
    feature = "hardware-qualification",
    target_os = "linux",
    target_arch = "x86_64"
))]
#![allow(unsafe_code)]

use std::convert::Infallible;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{
    DEFAULT_KFD_PATH, DeviceSelector, KfdAdapterError, KfdDebugExceptionInfoV1,
    KfdDebugQueueOperationStateV1, KfdDebugSessionErrorV1, KfdDebugSessionPlanV1,
    KfdDebuggerTelemetryEndpointV2, KfdLiveDebugSessionErrorV1, KfdLiveDebugSessionV1,
    KfdOpaqueCheckpointObservationV1, KfdOpaqueCheckpointSegmentKindV1, KfdStoppedAvailabilityV1,
    KfdStoppedContextSaveObservationV1, KfdStoppedQueueCapturePlanV1,
    KfdStoppedSnapshotOwnershipV1, KfdStoppedStateScopeV1, KfdStoppedUnavailableReasonV1,
    KfdTargetDebugArtifactIdentityV1, KfdTargetDebugSessionNonceV1, KfdTargetDebugSessionOutcomeV2,
    KfdTargetDebugTelemetryDigestV1, KfdTargetDebugTelemetryPayloadV2,
    KfdTargetDebugTelemetryProcessV1, KfdTargetRuntimeDebugTokenV1,
    MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1, OpenedKfd, create_kfd_target_debug_telemetry_channel_v2,
};
use fe2o3_kfd_uapi::{
    KfdDebugExceptionMaskV1, KfdDebugRuntimeStateV1, KfdDebugTrapExceptionCodeV1,
    KfdDebugTrapWaveLaunchModeV1,
};
use fe2o3_runtime::{
    AuthorizedRuntimeDebugTelemetrySessionV2, Gfx942RuntimeBufferAccessV1,
    Gfx942RuntimeDispatchBufferV1, Gfx942RuntimeDispatchInputsV1, PreparedGfx942RuntimeDispatchV1,
    WorkerV3Gfx942ExecutionAuthorityV1, execute_authorized_gfx942_runtime_debug_target_dispatch_v2,
    prepare_gfx942_runtime_dispatch_v1,
};
use sha2::{Digest, Sha256};

const HELPER_ENV: &str = "FE2O3_KFD_ACTIVE_CHECKPOINT_LIVE_HELPER";
const WAIT_BOUND: Duration = Duration::from_secs(45);
const KERNEL: &str = "active_checkpoint_liveness";
const ITERATIONS: u32 = 1_000_000_000;
const LANES: usize = 64;
const HSACO: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-active-checkpoint-v1/active-checkpoint.hsaco");
const SOURCE: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-active-checkpoint-v1/active-checkpoint.ll");
const POLICY: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-active-checkpoint-v1/policy-v1.txt");
const SOURCE_SHA256: [u8; 32] = [
    0xb5, 0x0f, 0xed, 0xd4, 0x59, 0x7e, 0xc5, 0x86, 0xd0, 0xd8, 0x0e, 0x2b, 0x51, 0xf1, 0x16, 0x11,
    0xe6, 0x22, 0xd5, 0x80, 0x26, 0x85, 0x8c, 0x01, 0x40, 0x7f, 0x27, 0xe3, 0x8e, 0x1f, 0x2b, 0x74,
];
const POLICY_SHA256: [u8; 32] = [
    0xef, 0xe4, 0x98, 0x37, 0x56, 0x73, 0x1c, 0xd0, 0x00, 0xa2, 0x7e, 0x2d, 0x9d, 0x1d, 0x8b, 0xc6,
    0x78, 0xc7, 0x2a, 0x06, 0xec, 0x1b, 0x4e, 0x61, 0xe4, 0xfe, 0xb0, 0xe0, 0xb2, 0x28, 0x2b, 0xb6,
];
const HSACO_SHA256: [u8; 32] = [
    0x3f, 0x65, 0xc3, 0x3a, 0x88, 0x6d, 0xd3, 0xf4, 0x36, 0x04, 0x10, 0x47, 0x64, 0x21, 0x03, 0x44,
    0xdf, 0xde, 0xb8, 0xd4, 0xf8, 0xda, 0xc0, 0xb0, 0xd7, 0x10, 0x98, 0xb9, 0xcc, 0xf9, 0x19, 0x50,
];

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn child(&mut self) -> &mut Child {
        self.0.as_mut().expect("child remains owned")
    }

    fn finish(mut self, deadline: Instant) {
        let mut child = self.0.take().expect("child remains owned");
        let Some(status) = wait_for_exit(&mut child, deadline) else {
            let _ = child.kill();
            let _ = wait_for_exit(&mut child, Instant::now() + Duration::from_secs(2));
            panic!("timed out waiting for active-checkpoint helper exit");
        };
        assert!(
            status.success(),
            "active-checkpoint helper failed: {status}"
        );
    }

    fn terminate(mut self) {
        let mut child = self.0.take().expect("child remains owned");
        let _ = child.kill();
        reap_bounded(&mut child, Instant::now() + Duration::from_secs(2));
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            reap_bounded(child, Instant::now() + Duration::from_secs(2));
        }
    }
}

#[derive(Debug)]
struct PinnedActiveCheckpointAuthorityV1 {
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
}

// SAFETY: this ignored live test re-hashes the repository-owned artifact,
// source, and complete invocation below. The authority is private to this test
// binary and accepts no caller-selected kernel, object, ABI, geometry, or
// device. It is qualification evidence only, never production authority.
unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for PinnedActiveCheckpointAuthorityV1 {
    type CurrentnessError = Infallible;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    fn kernel_name(&self) -> &str {
        KERNEL
    }

    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract_sha256
    }

    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn random_nonce() -> KfdTargetDebugSessionNonceV1 {
    let mut bytes = [0_u8; 32];
    fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .expect("read one live-test session nonce");
    KfdTargetDebugSessionNonceV1::from_bytes(bytes).expect("urandom nonce is nonzero")
}

fn artifact_identity(bytes: &[u8]) -> KfdTargetDebugArtifactIdentityV1 {
    KfdTargetDebugArtifactIdentityV1::new(
        KfdTargetDebugTelemetryDigestV1::from_bytes(digest(bytes)).unwrap(),
        u64::try_from(bytes.len()).unwrap(),
    )
    .unwrap()
}

fn prepare_active_checkpoint_dispatch() -> PreparedGfx942RuntimeDispatchV1 {
    assert_eq!(digest(SOURCE), SOURCE_SHA256);
    assert_eq!(digest(POLICY), POLICY_SHA256);
    assert_eq!(digest(HSACO), HSACO_SHA256);
    prepare_gfx942_runtime_dispatch_v1(
        HSACO,
        KERNEL,
        Gfx942RuntimeDispatchInputsV1::new(
            vec![0_u8; 8],
            vec![
                Gfx942RuntimeDispatchBufferV1::new(
                    vec![0_u8; LANES * size_of::<u32>()],
                    Gfx942RuntimeBufferAccessV1::WriteOnly,
                )
                .unwrap(),
            ],
            vec![fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            AqlDispatchGeometryV1::new([LANES as u32, 1, 1], [LANES as u32, 1, 1]).unwrap(),
            0,
            60_000,
        ),
    )
    .unwrap()
}

fn active_checkpoint_target_helper_body() {
    let unique_id = fe2o3_kfd::topology::discover_default_topology()
        .unwrap()
        .topology()
        .gpu_nodes()
        .iter()
        .filter(|node| node.target().name() == "gfx942")
        .map(|node| node.unique_id())
        .filter(|unique_id| *unique_id != 0)
        .min()
        .expect("MI300X live validation requires one gfx942 GPU");
    let prepared = prepare_active_checkpoint_dispatch();
    let authority = PinnedActiveCheckpointAuthorityV1 {
        finalized_hsaco_sha256: prepared.identity().object_sha256(),
        finalized_hsaco_length: prepared.finalized_hsaco_length(),
        dispatch_contract_sha256: prepared.dispatch_contract_sha256(),
        device_unique_id: unique_id,
    };
    let device = OpenedKfd::open_default()
        .unwrap()
        .admit_uapi()
        .unwrap()
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))
        .unwrap();
    let endpoint = fe2o3_kfd::admit_inherited_kfd_target_debug_telemetry_v2()
        .unwrap()
        .expect("helper requires the inherited V2 telemetry endpoint");
    let executable = artifact_identity(&fs::read(std::env::current_exe().unwrap()).unwrap());
    let process_instance = KfdTargetDebugTelemetryProcessV1::capture(std::process::id())
        .unwrap()
        .correlation_identity_v2()
        .unwrap();
    let generation = endpoint.session_generation();
    let telemetry = AuthorizedRuntimeDebugTelemetrySessionV2::new(
        endpoint,
        process_instance,
        executable,
        generation,
    )
    .unwrap();
    let token = KfdTargetRuntimeDebugTokenV1::enable_current_process().unwrap();
    let result = execute_authorized_gfx942_runtime_debug_target_dispatch_v2(
        authority, token, device, prepared, telemetry,
    )
    .unwrap();
    let [output] = result.into_buffers().try_into().unwrap();
    for value in output.into_bytes().chunks_exact(size_of::<u32>()) {
        assert_eq!(u32::from_le_bytes(value.try_into().unwrap()), ITERATIONS);
    }
    stop_tracee_process_leader();
    wait_for_debugger_release();
}

#[test]
fn active_checkpoint_target_helper() {
    if std::env::var_os(HELPER_ENV).is_some() {
        active_checkpoint_target_helper_body();
    }
}

#[test]
#[ignore = "requires MI300X, direct KFD debug-trap, and ptrace ownership"]
fn mi300x_captures_nonempty_same_queue_opaque_checkpoint() {
    let executable = std::env::current_exe().unwrap();
    let nonce = random_nonce();
    let (debugger_fd, target_fd) = create_kfd_target_debug_telemetry_channel_v2().unwrap();
    let target_raw = target_fd.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .args(["--exact", "active_checkpoint_target_helper", "--nocapture"])
        .env(HELPER_ENV, "1")
        .env(
            fe2o3_kfd::KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V2,
            target_raw.to_string(),
        )
        .env(
            fe2o3_kfd::KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V2,
            hex(nonce.as_bytes()),
        )
        .env(
            fe2o3_kfd::KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V2,
            std::process::id().to_string(),
        )
        .env_remove("RUST_MIN_STACK")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    // SAFETY: the post-fork hook changes flags on one retained socket and
    // performs a pointer-free ptrace syscall before exec. No allocation or
    // non-async-signal-safe Rust API is called in the child hook.
    unsafe {
        command.pre_exec(move || prepare_inherited_target(target_raw));
    }
    let child = command.spawn().unwrap();
    drop(target_fd);
    let mut child = ChildGuard(Some(child));
    wait_for_stop(child.child(), Instant::now() + WAIT_BOUND);

    let target = KfdTargetDebugTelemetryProcessV1::capture(child.child().id()).unwrap();
    let mut telemetry = KfdDebuggerTelemetryEndpointV2::admit(debugger_fd, nonce, target).unwrap();
    let runtime_mask =
        KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::ProcessRuntime);
    let queue_new = KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::QueueNew);
    let subscribed = KfdDebugExceptionMaskV1::new(runtime_mask.bits() | queue_new.bits()).unwrap();
    let plan =
        KfdDebugSessionPlanV1::new(child.child().id(), std::process::id(), subscribed, 64).unwrap();
    let mut session = match KfdLiveDebugSessionV1::attach(plan) {
        Ok(session) => session,
        Err(KfdLiveDebugSessionErrorV1::Kfd(KfdAdapterError::Open { path, source }))
            if path.as_path() == std::path::Path::new(DEFAULT_KFD_PATH)
                && source == rustix::io::Errno::NOENT =>
        {
            eprintln!("SKIP: {DEFAULT_KFD_PATH} is absent after ptrace/pidfd admission");
            detach_tracee(child.child());
            child.terminate();
            return;
        }
        Err(error) => panic!("live KFD debug attach failed: {error}"),
    };
    assert_eq!(
        session.runtime_observation().state(),
        KfdDebugRuntimeStateV1::Disabled
    );
    assert!(matches!(
        session.set_launch_mode(KfdDebugTrapWaveLaunchModeV1::Halt),
        Err(KfdLiveDebugSessionErrorV1::Session(
            KfdDebugSessionErrorV1::RuntimeNotEnabled
        ))
    ));

    continue_tracee(child.child());
    acknowledge_runtime_state(
        &mut session,
        KfdDebugRuntimeStateV1::Enabled,
        Instant::now() + WAIT_BOUND,
    );
    let declaration = receive_telemetry(&mut telemetry, Instant::now() + WAIT_BOUND);
    let KfdTargetDebugTelemetryPayloadV2::DispatchDeclared {
        process_instance,
        executable,
        artifact,
        dispatch,
        generation,
        grid,
        workgroup,
        ..
    } = declaration.payload()
    else {
        panic!("expected exact dispatch declaration: {declaration:?}");
    };
    assert_eq!(artifact.digest().as_bytes(), &HSACO_SHA256);
    assert_eq!(*grid, [LANES as u32, 1, 1]);
    assert_eq!(*workgroup, [LANES as u32, 1, 1]);

    let publication = receive_telemetry(&mut telemetry, Instant::now() + WAIT_BOUND);
    let KfdTargetDebugTelemetryPayloadV2::NativeDispatchPublished {
        process_instance: published_process,
        dispatch: published_dispatch,
        artifact: published_artifact,
        generation: published_generation,
        target_kfd_gpu_id_observation,
        target_kfd_queue_id_observation,
        target_aql_packet_id_observation,
        grid: published_grid,
        workgroup: published_workgroup,
        ..
    } = publication.payload()
    else {
        panic!("expected exact native publication: {publication:?}");
    };
    // This joins two target-emitted runtime records and the exact queue later
    // queried and suspended by KFD. It does not independently authenticate the
    // bytes physically loaded at the observed queue.
    assert_eq!(published_process, process_instance);
    assert_eq!(published_dispatch, dispatch);
    assert_eq!(published_artifact, &artifact.digest());
    assert_eq!(published_generation, generation);
    assert_eq!(published_grid, grid);
    assert_eq!(published_workgroup, workgroup);
    assert_eq!(*target_aql_packet_id_observation, 0);

    let queue = wait_for_queue(
        &mut session,
        *target_kfd_queue_id_observation,
        queue_new,
        Instant::now() + WAIT_BOUND,
    );
    assert_eq!(queue.gpu_id(), *target_kfd_gpu_id_observation);
    let suspension = session
        .suspend_queues(
            &[*target_kfd_queue_id_observation],
            KfdDebugExceptionMaskV1::NONE,
            u32::MAX,
        )
        .unwrap();
    assert!(matches!(
        suspension.as_slice(),
        [observation] if observation.state() == KfdDebugQueueOperationStateV1::Complete
    ));

    let stopped = session
        .capture_stopped_queue_v1(
            KfdStoppedQueueCapturePlanV1::with_checkpoint_byte_limit(
                *target_kfd_queue_id_observation,
                KfdStoppedStateScopeV1::new([0x4d; 32]).unwrap(),
                MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        stopped.ownership(),
        KfdStoppedSnapshotOwnershipV1::SessionRetainedSuspension
    );
    let layout = match stopped.context_save() {
        KfdStoppedContextSaveObservationV1::Available(layout) => layout,
        other => panic!("active gfx942 context-save capture unavailable: {other:?}"),
    };
    assert_eq!(layout.context_bytes_per_xcc(), 0x162_1000);
    assert_eq!(layout.headers().len(), 8);
    let published_range_bytes = layout.headers().iter().fold(0_u64, |total, header| {
        total + u64::from(header.control_stack().bytes()) + u64::from(header.wave_state().bytes())
    });
    let published_range_count = layout
        .headers()
        .iter()
        .flat_map(|header| [header.control_stack(), header.wave_state()])
        .filter(|range| !range.is_empty())
        .count();
    assert!(published_range_bytes > 0);
    assert!(published_range_count > 0);
    eprintln!(
        "active gfx942 public KFD ranges: bytes={published_range_bytes} segments={published_range_count}"
    );
    let checkpoint = match stopped.opaque_checkpoint() {
        KfdOpaqueCheckpointObservationV1::Complete(checkpoint) => checkpoint,
        other => panic!("active gfx942 opaque checkpoint unavailable: {other:?}"),
    };
    assert_eq!(checkpoint.captured_bytes(), published_range_bytes);
    assert_eq!(checkpoint.segments().len(), published_range_count);
    assert!(checkpoint.segments().iter().all(|segment| {
        matches!(
            segment.kind(),
            KfdOpaqueCheckpointSegmentKindV1::ControlStack
                | KfdOpaqueCheckpointSegmentKindV1::WaveState
        ) && !segment.range().is_empty()
            && segment.with_private_bytes(|bytes| bytes.len() == segment.range().bytes() as usize)
    }));
    assert_eq!(
        stopped.waves(),
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi
        )
    );
    assert_eq!(
        stopped.program_counter(),
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::ProgramCounterRequiresRegisterRecord
        )
    );

    let resumed = session
        .resume_queues(&[*target_kfd_queue_id_observation])
        .unwrap();
    assert!(matches!(
        resumed.as_slice(),
        [observation] if observation.state() == KfdDebugQueueOperationStateV1::Complete
    ));
    acknowledge_runtime_state(
        &mut session,
        KfdDebugRuntimeStateV1::Disabled,
        Instant::now() + WAIT_BOUND,
    );
    let terminal = receive_telemetry(&mut telemetry, Instant::now() + WAIT_BOUND);
    assert!(matches!(
        terminal.payload(),
        KfdTargetDebugTelemetryPayloadV2::SessionEnded {
            outcome: KfdTargetDebugSessionOutcomeV2::Completed
        }
    ));
    wait_for_stop(child.child(), Instant::now() + WAIT_BOUND);
    session.finish().unwrap();
    detach_tracee(child.child());
    release_helper(child.child());
    child.finish(Instant::now() + WAIT_BOUND);

    assert_ne!(stopped.logical_identity().as_bytes(), &[0; 32]);
    assert_ne!(checkpoint.content_identity().as_bytes(), &[0; 32]);
    assert_ne!(executable.digest().as_bytes(), &[0; 32]);
}

fn receive_telemetry(
    endpoint: &mut KfdDebuggerTelemetryEndpointV2,
    deadline: Instant,
) -> fe2o3_kfd::KfdTargetDebugTelemetryRecordV2 {
    loop {
        if let Some(record) = endpoint.try_receive().unwrap() {
            return record;
        }
        assert!(Instant::now() < deadline, "timed out waiting for telemetry");
        thread::sleep(Duration::from_millis(2));
    }
}

fn wait_for_queue(
    session: &mut KfdLiveDebugSessionV1,
    queue_id: u32,
    clear: KfdDebugExceptionMaskV1,
    deadline: Instant,
) -> fe2o3_kfd::KfdDebugQueueObservationV1 {
    loop {
        let queues = session.queue_snapshot(clear).unwrap();
        if let Some(queue) = queues.iter().find(|queue| queue.queue_id() == queue_id) {
            assert_eq!(queues.len(), 1, "unexpected queue snapshot: {queues:?}");
            return *queue;
        }
        assert!(queues.is_empty(), "unexpected queue snapshot: {queues:?}");
        assert!(Instant::now() < deadline, "timed out waiting for queue");
        thread::sleep(Duration::from_millis(2));
    }
}

fn acknowledge_runtime_state(
    session: &mut KfdLiveDebugSessionV1,
    expected: KfdDebugRuntimeStateV1,
    deadline: Instant,
) {
    loop {
        let _ = session.drain_notifications(1_024).unwrap();
        if let Some(event) = session.query_event(KfdDebugExceptionMaskV1::NONE).unwrap()
            && event
                .exceptions()
                .contains(KfdDebugTrapExceptionCodeV1::ProcessRuntime)
        {
            let info = session
                .query_exception_info(0, KfdDebugTrapExceptionCodeV1::ProcessRuntime, true)
                .unwrap();
            assert!(matches!(
                info,
                KfdDebugExceptionInfoV1::Runtime(runtime) if runtime.state() == expected
            ));
            session.acknowledge_runtime_transition(event).unwrap();
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected:?}"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

fn prepare_inherited_target(target_fd: RawFd) -> std::io::Result<()> {
    // SAFETY: fcntl reads and updates flags for the exact inherited socket.
    let flags = unsafe { libc::fcntl(target_fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: target_fd remains open across this pre-exec hook.
    if unsafe { libc::fcntl(target_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: PTRACE_TRACEME takes no pointer operands and marks this child for
    // its existing parent before exec.
    if unsafe {
        libc::ptrace(
            libc::PTRACE_TRACEME,
            0,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn wait_for_stop(child: &mut Child, deadline: Instant) {
    loop {
        let mut status = 0;
        // SAFETY: status points to one live integer and the exact child PID is
        // retained. WNOHANG keeps cleanup bounded.
        let result = unsafe {
            libc::waitpid(
                child.id() as libc::pid_t,
                &mut status,
                libc::WNOHANG | libc::WUNTRACED,
            )
        };
        if result == child.id() as libc::pid_t {
            assert!(
                libc::WIFSTOPPED(status),
                "target exited before stop: {status}"
            );
            return;
        }
        assert!(
            result >= 0,
            "waitpid failed: {}",
            std::io::Error::last_os_error()
        );
        assert!(
            Instant::now() < deadline,
            "timed out waiting for target stop"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

fn continue_tracee(child: &Child) {
    // SAFETY: this process owns the ptrace-stopped child and passes no address.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_CONT,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    assert_eq!(
        result,
        0,
        "PTRACE_CONT failed: {}",
        std::io::Error::last_os_error()
    );
}

fn detach_tracee(child: &Child) {
    // SAFETY: this process owns the ptrace-stopped child. SIGCONT clears its
    // process-wide stop while detaching.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            libc::SIGCONT as usize as *mut libc::c_void,
        )
    };
    assert_eq!(
        result,
        0,
        "PTRACE_DETACH failed: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: the child PID remains owned. ESRCH means it exited after detach.
    if unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGCONT) } != 0 {
        let error = std::io::Error::last_os_error();
        assert_eq!(
            error.raw_os_error(),
            Some(libc::ESRCH),
            "SIGCONT failed: {error}"
        );
    }
}

fn stop_tracee_process_leader() {
    let pid = std::process::id() as libc::pid_t;
    // SAFETY: tgkill receives the current process and leader task IDs.
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_tgkill, pid, pid, libc::SIGSTOP) },
        0,
        "tgkill(SIGSTOP) failed: {}",
        std::io::Error::last_os_error()
    );
}

fn wait_for_debugger_release() {
    let mut byte = [0_u8; 1];
    std::io::stdin().read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'.']);
}

fn release_helper(child: &mut Child) {
    child
        .stdin
        .as_mut()
        .expect("helper stdin remains owned")
        .write_all(b".")
        .unwrap();
}

fn wait_for_exit(child: &mut Child, deadline: Instant) -> Option<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(error) => panic!("failed to poll target exit: {error}"),
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn reap_bounded(child: &mut Child, deadline: Instant) {
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => thread::sleep(Duration::from_millis(2)),
        }
    }
}
