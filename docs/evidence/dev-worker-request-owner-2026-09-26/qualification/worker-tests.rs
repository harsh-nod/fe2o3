use super::*;
use crate::{
    RuntimeAccessV1, RuntimeAllocationAdmissionProfileV1 as Profile,
    RuntimeAllocationDeviceAdmissionV1 as Entry, RuntimeAllocationRequestWitnessV1,
    RuntimeBackendAllocationOutcomeV1 as Outcome, RuntimeCapabilitiesV1,
    RuntimeRequestAllocationResultV1 as RequestResult, RuntimeResourceKindV1 as K,
};
use fe2o3_kfd::Gfx942ComposedBackingRootV1;
use std::{cell::RefCell, collections::HashSet, io::Cursor, rc::Rc};

#[derive(Default)]
struct State {
    calls: Vec<&'static str>,
    allocate: u8,
    release: u8,
    other: u8,
    next: u64,
    live: HashSet<u64>,
    panic: Option<Box<u64>>,
}
struct Backend {
    entries: Vec<Entry>,
    state: Rc<RefCell<State>>,
}
impl fmt::Debug for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("accounting fixture backend")
    }
}
fn error() -> io::Error {
    io::Error::other("scripted backend outcome")
}
impl Backend {
    fn other<T>(
        &self,
        name: &'static str,
        value: T,
    ) -> Result<T, RuntimeBackendFailureV1<io::Error>> {
        let mut state = self.state.borrow_mut();
        state.calls.push(name);
        match state.other {
            0 => Ok(value),
            1 => Err(RuntimeBackendFailureV1::Terminal(error())),
            _ => resume_unwind(state.panic.take().unwrap()),
        }
    }
}
macro_rules! forward {
    ($name:ident($($arg:ident: $ty:ty),*) -> $result:ty = $value:expr) => {
        fn $name(&mut self, $($arg: $ty),*) -> Result<$result, RuntimeBackendFailureV1<Self::Error>> {
            $(let _ = $arg;)* self.other(stringify!($name), $value)
        }
    };
}
impl RuntimeBackendV1 for Backend {
    type Error = io::Error;
    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        self.state.borrow_mut().calls.push("enumerate");
        Ok([19, 7]
            .map(|backend_device| BackendDeviceDescriptionV1 {
                backend_device,
                name: "accounting fixture".into(),
                target: "gfx942:xnack-".into(),
                global_memory_bytes: 0,
                capabilities: RuntimeCapabilitiesV1 {
                    device_memory: true,
                    host_visible_memory: true,
                    multi_device: true,
                    ..RuntimeCapabilitiesV1::default()
                },
            })
            .to_vec())
    }
    fn allocation_admission_profile_v1(
        &self,
    ) -> Result<Profile, RuntimeBackendFailureV1<Self::Error>> {
        Ok(Profile::Required(self.entries.clone()))
    }
    fn allocate_v1(
        &mut self,
        _: u64,
        _: RuntimeMemoryKindV1,
        _: u64,
        _: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        panic!("witness-free allocation reached backend")
    }
    fn allocate_with_request_v1(
        &mut self,
        device: u64,
        _: RuntimeMemoryKindV1,
        bytes: u64,
        _: u64,
        witness: RuntimeAllocationRequestWitnessV1<'_>,
    ) -> RequestResult<Self::Error> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.backend_device_v1() == device)
            .unwrap();
        assert!(witness.matches_v1(entry, bytes));
        let mut state = self.state.borrow_mut();
        state.calls.push("allocate");
        RequestResult::Outcome(match state.allocate {
            0 => {
                let id = state.next;
                state.next += 1;
                assert!(state.live.insert(id));
                Ok(Outcome::Allocated(id))
            }
            1 => Err(RuntimeBackendFailureV1::Rejected(error())),
            2 => Ok(Outcome::SettledNoOwner(error())),
            3 => Err(RuntimeBackendFailureV1::Quiescent(error())),
            4 => Err(RuntimeBackendFailureV1::Terminal(error())),
            5 => resume_unwind(state.panic.take().unwrap()),
            6 => Ok(Outcome::Allocated(0)),
            7 => Ok(Outcome::Allocated(41)),
            8 => return RequestResult::Unsupported,
            _ => unreachable!(),
        })
    }
    fn release_allocation_v1(
        &mut self,
        handle: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        let mut state = self.state.borrow_mut();
        state.calls.push("release");
        match state.release {
            0 => {
                assert!(state.live.remove(&handle));
                Ok(())
            }
            1 => Err(RuntimeBackendFailureV1::Rejected(error())),
            2 => Err(RuntimeBackendFailureV1::Quiescent(error())),
            3 => Err(RuntimeBackendFailureV1::Terminal(error())),
            _ => resume_unwind(state.panic.take().unwrap()),
        }
    }
    forward!(create_stream_v1(device: u64) -> u64 = 81);
    forward!(destroy_stream_v1(stream: u64) -> () = ());
    forward!(write_allocation_v1(allocation: u64, offset: u64, bytes: &[u8]) -> () = ());
    forward!(read_allocation_v1(allocation: u64, offset: u64, bytes: &mut [u8]) -> () = ());
    forward!(load_module_v1(device: u64, image: &[u8]) -> u64 = 82);
    forward!(unload_module_v1(module: u64) -> () = ());
    forward!(resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64 = 83);
    forward!(submit_v1(launch: BackendLaunchV1<'_>) -> u64 = 84);
    forward!(poll_v1(submission: u64) -> BackendPollV1 = BackendPollV1::Succeeded);
    forward!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1 = BackendPollV1::Succeeded);
    forward!(release_submission_v1(submission: u64) -> () = ());
    forward!(record_event_v1(stream: u64, submission: u64) -> u64 = 85);
    forward!(release_event_v1(event: u64) -> () = ());
    forward!(peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64 = 86);
}
impl RuntimeWorkerV1ImmediateProgressBackendV1 for Backend {}
impl RuntimeFlushBackendV1 for Backend {
    forward!(flush_stream_v1(stream: u64) -> () = ());
}
impl RuntimeAsyncCopyBackendV1 for Backend {
    forward!(copy_async_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64 = 87);
}
impl RuntimeCancellationBackendV1 for Backend {
    forward!(cancel_v1(submission: u64) -> BackendCancellationV1 = BackendCancellationV1::Cancelled);
    forward!(drain_v1(submission: u64, deadline: Instant) -> BackendPollV1 = BackendPollV1::Succeeded);
}
impl RuntimeAtomicBackendV1 for Backend {
    forward!(submit_atomic_v1(launch: BackendLaunchV1<'_>) -> u64 = 88);
}
impl RuntimeCollectiveBackendV1 for Backend {
    forward!(submit_collective_v1(launch: BackendLaunchV1<'_>) -> u64 = 89);
}

fn fixture() -> (
    Gfx942ComposedBackingRootV1,
    [Entry; 2],
    Backend,
    Rc<RefCell<State>>,
) {
    let root = Entry::qualification_root_v1();
    let entries = [
        Entry::qualification_entry_v1(&root, 19),
        Entry::qualification_entry_v1(&root, 7),
    ];
    let state = Rc::new(RefCell::new(State {
        next: 41,
        ..State::default()
    }));
    let backend = Backend {
        entries: entries.to_vec(),
        state: state.clone(),
    };
    (root, entries, backend, state)
}
fn allocate(device: u64, bytes: u64) -> Vec<u8> {
    encode_binary_request_v1(RuntimeWorkerOperationV1::Allocate {
        device,
        kind: RuntimeMemoryKindV1::DeviceLocal,
        byte_len: bytes,
        alignment: 8,
    })
    .unwrap()
}
fn release(handle: u64) -> Vec<u8> {
    encode_binary_request_v1(RuntimeWorkerOperationV1::ReleaseAllocation { allocation: handle })
        .unwrap()
}
fn frames(requests: &[Vec<u8>], close: bool) -> Vec<u8> {
    let mut input = Vec::new();
    for request in requests {
        write_frame_v1(&mut input, request).unwrap();
    }
    if close {
        write_frame_v1(&mut input, &[]).unwrap();
    }
    input
}
fn run_with<W: Write>(
    owner: &mut RuntimeWorkerRequestOwnerV1<Backend>,
    version: u8,
    input: Vec<u8>,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1> {
    match version {
        1 => serve_runtime_request_owner_v1(owner, Cursor::new(input), output),
        4 => serve_runtime_request_owner_v4(owner, Cursor::new(input), output),
        5 => serve_runtime_request_owner_v5(owner, Cursor::new(input), output),
        _ => unreachable!(),
    }
}
fn run(
    owner: &mut RuntimeWorkerRequestOwnerV1<Backend>,
    version: u8,
    requests: &[Vec<u8>],
) -> Vec<Vec<u8>> {
    let mut bytes = Vec::new();
    run_with(owner, version, frames(requests, true), &mut bytes).unwrap();
    let mut input = Cursor::new(bytes);
    let mut responses = Vec::new();
    while input.position() < input.get_ref().len() as u64 {
        responses.push(read_frame_v1(&mut input).unwrap());
    }
    responses
}
fn clear(entry: &Entry) {
    let usage = entry.account().usage_v1();
    assert_eq!(usage.used.get(K::RequestedAllocationBytes), 0);
    assert_eq!(usage.used.get(K::AllocationRecords), 0);
    assert_eq!(
        (
            usage.reserved_records,
            usage.retained_records,
            usage.quarantined_records
        ),
        (0, 0, 0)
    );
}

#[test]
fn qualification_worker_required_roster_rejection_returns_original_backend() {
    for mode in 0..6 {
        let (_root, entries, mut backend, state) = fixture();
        backend.entries = match mode {
            0 => vec![],
            1 => vec![entries[0].clone()],
            2 => vec![entries[0].clone(), entries[0].clone()],
            3 => vec![
                entries[0].clone(),
                Entry::qualification_v1(7, entries[0].model(), entries[0].account().clone()),
            ],
            4 => vec![entries[0].clone(), entries[1].clone(), entries[1].clone()],
            _ => {
                entries[1]
                    .account()
                    .reserve_v1(1)
                    .unwrap()
                    .retain()
                    .quarantine();
                backend.entries
            }
        };
        let failure = RuntimeWorkerRequestOwnerV1::open(backend).unwrap_err();
        let (backend, _) = failure.into_parts();
        assert!(Rc::ptr_eq(&backend.state, &state));
        assert_eq!(state.borrow().calls, ["enumerate"]);
        if mode != 5 {
            for entry in &entries {
                clear(entry);
            }
        }
    }
}

#[test]
fn qualification_worker_all_versions_use_context_witnesses_and_frozen_enumeration() {
    for version in [1, 4, 5] {
        let (root, entries, backend, state) = fixture();
        let baseline = root.usage_v1();
        let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
        let responses = run(
            &mut owner,
            version,
            &[
                vec![OP_ENUMERATE_DEVICES_V1],
                allocate(19, 16),
                allocate(7, 24),
            ],
        );
        assert_eq!(
            responses[0],
            match version {
                1 => RUNTIME_WORKER_HANDSHAKE_V1,
                4 => RUNTIME_WORKER_HANDSHAKE_V4,
                _ => RUNTIME_WORKER_HANDSHAKE_V5,
            }
        );
        assert_eq!(
            responses[2],
            encode_handle_response_v1::<()>(Ok(41)).unwrap()
        );
        assert_eq!(
            responses[3],
            encode_handle_response_v1::<()>(Ok(42)).unwrap()
        );
        assert_eq!(state.borrow().calls, ["enumerate", "allocate", "allocate"]);
        assert_eq!(
            entries[0]
                .account()
                .usage_v1()
                .used
                .get(K::RequestedAllocationBytes),
            16
        );
        assert_eq!(
            entries[1]
                .account()
                .usage_v1()
                .used
                .get(K::RequestedAllocationBytes),
            24
        );
        owner = *owner.try_into_backend().unwrap_err();
        run(&mut owner, version, &[release(41), release(42)]);
        assert_eq!(owner.retained_allocations_v1(), 0);
        for entry in &entries {
            clear(entry);
        }
        assert_eq!(root.usage_v1(), baseline);
        assert!(owner.try_into_backend().is_ok());
    }
}

#[test]
fn qualification_worker_allocation_outcomes_preserve_local_settlement_and_wire_tags() {
    for mode in [1, 2, 3, 4, 6, 8] {
        let (_root, entries, backend, state) = fixture();
        state.borrow_mut().allocate = mode;
        let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
        let responses = run(&mut owner, 5, &[allocate(19, 16)]);
        let expected = match mode {
            1 | 8 => RESPONSE_REJECTED_V1,
            2 | 3 => RESPONSE_QUIESCENT_V1,
            _ => RESPONSE_TERMINAL_V1,
        };
        assert_eq!(responses[1][0], expected);
        if matches!(mode, 1 | 2 | 8) {
            clear(&entries[0]);
            assert!(owner.try_into_backend().is_ok());
        } else {
            assert_eq!(entries[0].account().usage_v1().quarantined_records, 1);
            assert_eq!(owner.retained_allocations_v1(), usize::from(mode == 6));
            assert!(owner.try_into_backend().is_err());
        }
        clear(&entries[1]);
    }
}

#[test]
fn qualification_worker_release_retry_removes_credit_only_after_confirmed_disposal() {
    for mode in [1, 2, 3] {
        let (_root, entries, backend, state) = fixture();
        let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
        run(&mut owner, 5, &[allocate(19, 16), allocate(7, 24)]);
        state.borrow_mut().release = mode;
        let responses = run(&mut owner, 5, &[release(41)]);
        assert_eq!(
            responses[1][0],
            [
                0,
                RESPONSE_REJECTED_V1,
                RESPONSE_QUIESCENT_V1,
                RESPONSE_TERMINAL_V1
            ][mode as usize]
        );
        assert_eq!(owner.retained_allocations_v1(), 2);
        if mode < 3 {
            for entry in &entries {
                assert_eq!(entry.account().usage_v1().retained_records, 1);
            }
            state.borrow_mut().release = 0;
            run(&mut owner, 5, &[release(41), release(42)]);
            for entry in &entries {
                clear(entry);
            }
            assert!(owner.try_into_backend().is_ok());
        } else {
            for entry in &entries {
                assert_eq!(entry.account().usage_v1().quarantined_records, 1);
            }
            assert!(owner.is_terminal_v1());
        }
    }
}

struct FailOutput {
    remaining: usize,
    flushes: usize,
    fail_flush: usize,
}
impl Write for FailOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected output loss"));
        }
        let count = self.remaining.min(bytes.len());
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.flushes == self.fail_flush {
            Err(io::Error::other("injected flush loss"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn qualification_worker_response_loss_never_refunds_allocation_or_replays_backend() {
    for offset in [0, 2, 4, 7, 13, usize::MAX] {
        let (_root, entries, backend, state) = fixture();
        let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
        run(&mut owner, 5, &[allocate(7, 24)]);
        let output = FailOutput {
            remaining: if offset == usize::MAX {
                usize::MAX
            } else {
                4 + RUNTIME_WORKER_HANDSHAKE_V5.len() + offset
            },
            flushes: 0,
            fail_flush: if offset >= 13 { 2 } else { usize::MAX },
        };
        assert!(run_with(&mut owner, 5, frames(&[allocate(19, 16)], true), output).is_err());
        assert!(owner.is_terminal_v1());
        assert_eq!(owner.retained_allocations_v1(), 2);
        for (entry, bytes) in entries.iter().zip([16, 24]) {
            assert_eq!(entry.account().usage_v1().quarantined_records, 1);
            assert_eq!(
                entry
                    .account()
                    .usage_v1()
                    .used
                    .get(K::RequestedAllocationBytes),
                bytes
            );
            assert_eq!(entry.account().usage_v1().used.get(K::AllocationRecords), 1);
        }
        assert_eq!(state.borrow().live, HashSet::from([41, 42]));
        let calls = state.borrow().calls.clone();
        assert!(run_with(&mut owner, 5, frames(&[release(41)], true), Vec::new()).is_err());
        assert_eq!(state.borrow().calls, calls);
        assert!(!calls.contains(&"release"));
    }
}

#[test]
fn qualification_worker_lost_release_response_refunds_only_confirmed_disposal() {
    let (_root, entries, backend, state) = fixture();
    let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
    run(&mut owner, 5, &[allocate(19, 16), allocate(7, 24)]);
    let output = FailOutput {
        remaining: 4 + RUNTIME_WORKER_HANDSHAKE_V5.len(),
        flushes: 0,
        fail_flush: usize::MAX,
    };
    assert!(run_with(&mut owner, 5, frames(&[release(41)], true), output).is_err());
    clear(&entries[0]);
    assert_eq!(entries[1].account().usage_v1().quarantined_records, 1);
    assert_eq!(owner.retained_allocations_v1(), 1);
    assert_eq!(state.borrow().live, HashSet::from([42]));
    assert_eq!(
        state.borrow().calls,
        ["enumerate", "allocate", "allocate", "release"]
    );
}

#[test]
fn qualification_worker_framing_loss_seals_all_existing_request_custody() {
    for input in [
        vec![],
        vec![1, 0],
        vec![2, 0, 0, 0, OP_ALLOCATE_V1],
        frames(&[vec![255]], true),
        frames(&[vec![OP_ENUMERATE_DEVICES_V1, 1]], true),
    ] {
        let (root, entries, backend, state) = fixture();
        let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
        run(&mut owner, 5, &[allocate(19, 16), allocate(7, 24)]);
        assert!(run_with(&mut owner, 5, input, Vec::new()).is_err());
        assert!(owner.is_terminal_v1());
        assert_eq!(state.borrow().calls, ["enumerate", "allocate", "allocate"]);
        let observe = root.qualification_observer_v1();
        drop(owner);
        drop(entries);
        drop(root);
        assert_eq!(observe().unwrap().quarantined_records, 2);
    }
}

#[test]
fn qualification_worker_panic_and_direct_terminal_preserve_all_custody() {
    for stage in ["allocate", "release", "other", "terminal"] {
        let (_root, entries, backend, state) = fixture();
        let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
        run(&mut owner, 5, &[allocate(19, 16), allocate(7, 24)]);
        let payload = Box::new(827_u64);
        let pointer = core::ptr::from_ref(payload.as_ref());
        state.borrow_mut().panic = Some(payload);
        let request = match stage {
            "allocate" => {
                state.borrow_mut().allocate = 5;
                allocate(19, 8)
            }
            "release" => {
                state.borrow_mut().release = 4;
                release(41)
            }
            _ => {
                state.borrow_mut().other = if stage == "terminal" { 1 } else { 2 };
                encode_binary_request_v1(RuntimeWorkerOperationV1::Poll { submission: 99 }).unwrap()
            }
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_with(&mut owner, 5, frames(&[request], true), Vec::new())
        }));
        if stage == "terminal" {
            assert!(result.unwrap().is_ok());
        } else {
            assert_eq!(
                core::ptr::from_ref(result.unwrap_err().downcast::<u64>().unwrap().as_ref()),
                pointer
            );
        }
        assert!(owner.is_terminal_v1());
        assert_eq!(owner.retained_allocations_v1(), 2);
        assert_eq!(
            entries[0].account().usage_v1().quarantined_records,
            if stage == "allocate" { 2 } else { 1 }
        );
        assert_eq!(entries[1].account().usage_v1().quarantined_records, 1);
    }
}

#[test]
fn qualification_worker_duplicate_success_retains_both_context_records() {
    let (_root, entries, backend, state) = fixture();
    let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
    run(&mut owner, 5, &[allocate(19, 16)]);
    state.borrow_mut().allocate = 7;
    let responses = run(&mut owner, 5, &[allocate(7, 24)]);
    assert_eq!(responses[1][0], RESPONSE_TERMINAL_V1);
    assert_eq!(owner.retained_allocations_v1(), 2);
    for entry in &entries {
        assert_eq!(entry.account().usage_v1().quarantined_records, 1);
    }
    assert!(owner.try_into_backend().is_err());
}

#[test]
fn qualification_worker_capacity_and_unknown_requests_have_no_backend_effects() {
    let (_root, entries, backend, state) = fixture();
    let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
    let before = entries[0].account().usage_v1();
    let responses = run(
        &mut owner,
        5,
        &[
            allocate(99, 16),
            allocate(19, u64::MAX),
            allocate(19, 0),
            release(999),
        ],
    );
    assert!(
        responses[1..]
            .iter()
            .all(|response| response[0] == RESPONSE_REJECTED_V1)
    );
    assert_eq!(state.borrow().calls, ["enumerate"]);
    assert_eq!(entries[0].account().usage_v1(), before);
    // Other healthy users of the exact leaf do not block ownership transfer.
    let credit = entries[0].account().reserve_v1(8).unwrap().retain();
    assert!(owner.try_into_backend().is_ok());
    credit.release_after_rejection().unwrap();
}

fn references(handle: u64, position: usize) -> Vec<Vec<u8>> {
    let region = BackendMemoryRegionV1 {
        allocation: handle,
        access: RuntimeAccessV1::ReadWrite,
        byte_offset: 0,
        byte_len: 8,
    };
    let known = BackendMemoryRegionV1 {
        allocation: 41,
        ..region
    };
    let geometry = RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let bindings = [0, 1, 2].map(|index| BackendBindingV1 {
        region: if index == position { region } else { known },
        kernarg_byte_offset: (index * 8) as u32,
    });
    let (source, destination) = if position == 0 {
        (region, known)
    } else {
        (known, region)
    };
    let mut requests = vec![
        encode_binary_request_v1(RuntimeWorkerOperationV1::WriteAllocation {
            allocation: handle,
            byte_offset: 0,
            bytes: &[0; 8],
        })
        .unwrap(),
        encode_binary_request_v1(RuntimeWorkerOperationV1::ReadAllocation {
            allocation: handle,
            byte_offset: 0,
            byte_len: 8,
        })
        .unwrap(),
        encode_binary_request_v1(RuntimeWorkerOperationV1::PeerCopy {
            stream: 81,
            source,
            destination,
            dependencies: &[],
        })
        .unwrap(),
        encode_binary_request_v1(RuntimeWorkerOperationV1::Submit {
            stream: 81,
            kernel: 83,
            explicit_kernarg: &[0; 24],
            bindings: &bindings,
            dependencies: &[],
            geometry,
        })
        .unwrap(),
        RuntimeBinaryCodecV5
            .encode_async_copy_request_v4(81, source, destination, &[])
            .unwrap(),
    ];
    let atomic = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::CompareExchange,
        scope: RuntimeMemoryScopeV1::Device,
        order: RuntimeMemoryOrderV1::AcquireRelease,
        failure_order: Some(RuntimeMemoryOrderV1::Acquire),
        weak: true,
        geometry,
    };
    let collective = RuntimeCollectiveLaunchContractV1 {
        operation: RuntimeCollectiveOperationV1::ReduceSum,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::AcquireRelease,
        participants: 64,
        geometry,
    };
    for semantic in [
        BackendSemanticLaunchV1::Atomic(atomic),
        BackendSemanticLaunchV1::Collective(collective),
    ] {
        let launch = BackendLaunchV1 {
            stream: 81,
            kernel: 83,
            explicit_kernarg: &[0; 24],
            bindings: &bindings,
            dependencies: &[],
            geometry,
            semantic_launch: semantic,
        };
        requests.push(
            match semantic {
                BackendSemanticLaunchV1::Atomic(_) => {
                    RuntimeBinaryCodecV5.encode_atomic_submit_request_v5(launch)
                }
                BackendSemanticLaunchV1::Collective(_) => {
                    RuntimeBinaryCodecV5.encode_collective_submit_request_v5(launch)
                }
                _ => unreachable!(),
            }
            .unwrap(),
        );
    }
    requests
}

#[test]
fn qualification_worker_all_memory_references_require_owned_allocation_before_dispatch() {
    let (_root, entries, backend, state) = fixture();
    let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
    run(&mut owner, 5, &[allocate(19, 16)]);
    for position in 0..3 {
        let responses = run(&mut owner, 5, &references(999, position));
        assert_eq!(responses.len(), 8);
        assert!(
            responses[1..]
                .iter()
                .all(|response| response[0] == RESPONSE_REJECTED_V1)
        );
    }
    assert_eq!(state.borrow().calls, ["enumerate", "allocate"]);
    let responses = run(&mut owner, 5, &references(41, 1));
    assert!(
        responses[1..]
            .iter()
            .all(|response| response[0] == RESPONSE_OK_V1)
    );
    assert_eq!(state.borrow().calls.len(), 9);
    run(&mut owner, 5, &[release(41)]);
    let calls = state.borrow().calls.clone();
    let responses = run(&mut owner, 5, &references(41, 1));
    assert!(
        responses[1..]
            .iter()
            .all(|response| response[0] == RESPONSE_REJECTED_V1)
    );
    assert_eq!(state.borrow().calls, calls);
    for entry in &entries {
        clear(entry);
    }
    assert!(owner.try_into_backend().is_ok());
}
