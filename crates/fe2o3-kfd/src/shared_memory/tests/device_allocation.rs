//! CPU custody tests; the fake backend is not native execution evidence.
use super::*;
use crate::shared_memory::device_allocation::{
    AllocationLeaseV1 as Lease, DeviceAllocationCustodyV1 as Root,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AllocationSnapshot {
    lease: Option<(LeaseIdentity, bool)>,
    started: bool,
    failed: bool,
    native_started: bool,
    progress: crate::shared_memory::transitions::NativeTransitionProgressV1,
}

pub(in crate::shared_memory) fn allocation_snapshot(root: &Root) -> AllocationSnapshot {
    let lease = match &root.lease {
        Lease::None => None,
        Lease::Unmapped(lease) => Some((
            (
                lease.id,
                lease.generation,
                lease.device,
                lease.vm,
                lease.layout,
            ),
            false,
        )),
        Lease::Mapped(lease) => Some((
            (
                lease.id,
                lease.generation,
                lease.device,
                lease.vm,
                lease.layout,
            ),
            true,
        )),
    };
    AllocationSnapshot {
        lease,
        started: root.started,
        failed: root.failed,
        native_started: root.native_started,
        progress: root.progress,
    }
}

impl AllocationSnapshot {
    pub(crate) fn with_failure_for_test(&self) -> Self {
        let mut result = self.clone();
        result.failed = true;
        result
    }

    pub(crate) fn started(&self) -> bool {
        self.started
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed
    }
    pub(crate) fn native_started(&self) -> bool {
        self.native_started
    }
    pub(crate) fn progress(&self) -> (bool, Option<bool>, Option<u32>) {
        (
            self.progress.attempted,
            self.progress.returned_success,
            self.progress.returned_map_prefix,
        )
    }
    pub(crate) fn lease(
        &self,
    ) -> Option<(
        Gfx942DeviceMemoryIdentityV1,
        Gfx942DeviceMemoryLayoutV1,
        bool,
    )> {
        self.lease
            .map(|((id, generation, device, vm, layout), mapped)| {
                (
                    Gfx942DeviceMemoryIdentityV1 {
                        id,
                        generation,
                        device,
                        vm,
                    },
                    layout,
                    mapped,
                )
            })
    }
}

fn prepare(
    fixture: &mut Fixture,
    root: &mut Root,
    bytes: u64,
    alignment: u64,
) -> Result<(), MemorySessionError> {
    root.prepare_in_place(
        &mut fixture.memory.engine,
        fixture.memory.device.model_key(),
        fixture.memory.vm,
        bytes,
        alignment,
    )
}

fn counters(fixture: &Fixture) -> [usize; 5] {
    let (currentness, reserve, alloc, cpu, gpu, _) = fixture.calls();
    [currentness, reserve, alloc, cpu, gpu]
}

fn assert_prefix(fixture: &Fixture, before: [usize; 5], delta: [usize; 5]) {
    assert_eq!(
        counters(fixture),
        std::array::from_fn(|i| before[i] + delta[i])
    );
    let backend = &fixture.memory.engine.backend;
    assert_eq!(backend.map_cpu_inputs, []);
    assert_eq!(backend.last_unmapped_bytes, None);
    assert_eq!(backend.last_unmapped_readback_calls, 0);
    assert!(
        fixture
            .memory
            .engine
            .device_memory
            .iter()
            .all(|r| r.mapping.is_none())
    );
    assert!(backend.operations.iter().all(|op| *op == "map_gpu"));
}

fn assert_usage(fixture: &Fixture, bytes: u64, records: u64, quarantined: usize) {
    fixture.assert_usage(bytes, records, quarantined);
    if fixture.account.is_none() {
        assert_eq!(fixture.memory.usage(), None);
        assert!(fixture.memory.engine.device_backing_account.is_none());
        assert!(
            fixture
                .memory
                .engine
                .device_memory
                .iter()
                .all(|r| r.backing_charge.is_none())
        );
    }
}

fn assert_actual_lease(fixture: &Fixture, root: &Root, mapped: bool, bytes: u64) {
    let state = allocation_snapshot(root);
    let expected = (
        fixture.next_id,
        1,
        fixture.memory.device.model_key(),
        fixture.memory.vm,
        device_memory_layout(bytes, 4096, KfdAllocMemoryFlags::DEVICE_LOCAL).unwrap(),
    );
    assert_eq!(state.lease, Some((expected, mapped)));
    assert_eq!(
        snapshot(&fixture.memory.engine.device_memory[1]).identity,
        expected
    );
}

fn clear_faults(fixture: &mut Fixture) {
    let backend = &mut fixture.memory.engine.backend;
    backend.fail_operation = None;
    backend.panic_operation = None;
    backend.fail_currentness_at = None;
    backend.panic_currentness_at = None;
    backend.map_errno = false;
    backend.map_progress = 1;
    backend.alloc_oom = false;
    backend.corrupt_flags = false;
    backend.allocation_output_mutator = None;
    backend.fixed_va = None;
}

fn no_retry(fixture: &mut Fixture) {
    let engine = &fixture.memory.engine;
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let records: Vec<_> = engine.device_memory.iter().map(snapshot).collect();
    let calls = fixture.calls();
    let usage = fixture.memory.usage();
    let next_id = engine.next_device_memory_id;
    let initialized = engine
        .terminal_device_initialization
        .as_ref()
        .map(root_snapshot);
    let allocation = engine
        .terminal_device_initialization
        .allocation_as_ref()
        .map(allocation_snapshot);
    clear_faults(fixture);
    let mut retry = Root::new();
    assert!(matches!(
        prepare(fixture, &mut retry, 17, 4096),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert!(!retry.requires_retention());
    retry.retain_live_failure(&mut fixture.memory.engine);
    assert_eq!(fixture.calls(), calls);
    assert_eq!(fixture.memory.usage(), usage);
    let engine = &fixture.memory.engine;
    assert_eq!(engine.next_device_memory_id, next_id);
    assert_eq!(
        engine
            .device_memory
            .iter()
            .map(snapshot)
            .collect::<Vec<_>>(),
        records
    );
    assert_eq!(
        engine
            .terminal_device_initialization
            .as_ref()
            .map(root_snapshot),
        initialized
    );
    assert_eq!(
        engine
            .terminal_device_initialization
            .allocation_as_ref()
            .map(allocation_snapshot),
        allocation
    );
    fixture.assert_anchor();
}

fn retain_failure(fixture: &mut Fixture, mut root: Root) {
    let before = allocation_snapshot(&root);
    // Retention must not repair missing quarantine at the preparation boundary.
    no_retry(fixture);
    assert_eq!(allocation_snapshot(&root), before);
    assert!(root.completed().is_err());
    assert!(root.take_complete().is_err());
    let calls = fixture.calls();
    assert!(matches!(
        prepare(fixture, &mut root, 17, 4096),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(allocation_snapshot(&root), before);
    assert_eq!(fixture.calls(), calls);
    assert!(root.requires_retention());
    root.retain_live_failure(&mut fixture.memory.engine);
    let terminal = fixture
        .memory
        .engine
        .terminal_device_initialization
        .allocation_as_ref()
        .unwrap();
    assert_eq!(allocation_snapshot(terminal), before);
    assert!(terminal.completed().is_err());
    assert!(
        fixture
            .memory
            .engine
            .terminal_device_initialization
            .as_ref()
            .is_none()
    );
    no_retry(fixture);
}

#[test]
fn device_allocator_success_retains_exact_device_local_owner_until_single_extraction() {
    for configured in [false, true] {
        for bytes in [1, 17, 4097] {
            let mut fixture = Fixture::new(configured);
            let before = counters(&fixture);
            let mut root = Root::new();
            assert!(root.completed().is_err());
            prepare(&mut fixture, &mut root, bytes, 4096).unwrap();
            assert_prefix(&fixture, before, [4, 1, 1, 0, 1]);
            assert_actual_lease(&fixture, &root, true, bytes);
            assert!(root.started && root.native_started && !root.failed);
            assert_eq!(
                (
                    root.progress.attempted,
                    root.progress.returned_success,
                    root.progress.returned_map_prefix
                ),
                (true, Some(true), Some(1))
            );
            assert_eq!(
                fixture.memory.engine.backend.flags[1],
                KfdAllocMemoryFlags::DEVICE_LOCAL.bits()
            );
            let record = &fixture.memory.engine.device_memory[1];
            assert_eq!(record.phase, DeviceMemoryPhaseV1::Mapped);
            assert_eq!(
                fixture.memory.engine.backend.map_gpu_inputs[1],
                (record.handle.unwrap(), 0)
            );
            let state = allocation_snapshot(&root);
            assert!(matches!(
                prepare(&mut fixture, &mut root, 17, 4096),
                Err(MemorySessionError::InvalidDeviceMemoryAuthority)
            ));
            assert_eq!(allocation_snapshot(&root), state);
            assert_prefix(&fixture, before, [4, 1, 1, 0, 1]);
            let output = root.take_complete().unwrap();
            assert_eq!(
                (
                    output.id,
                    output.generation,
                    output.device,
                    output.vm,
                    output.layout
                ),
                state.lease.unwrap().0
            );
            assert!(root.take_complete().is_err());
            assert!(root.completed().is_err());
            assert!(matches!(root.lease, Lease::None));
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            assert_usage(&fixture, 4096 + output.layout.backing_bytes, 2, 0);
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_allocator_currentness_matrix_distinguishes_per_call_native_admission() {
    for configured in [false, true] {
        for panic in [false, true] {
            for ordinal in 1..=4 {
                let mut fixture = Fixture::new(configured);
                let before = counters(&fixture);
                assert!(fixture.memory.engine.device_backing_activity_started);
                let backend = &mut fixture.memory.engine.backend;
                if panic {
                    backend.panic_currentness_at = Some(before[0] + ordinal);
                } else {
                    backend.fail_currentness_at = Some(before[0] + ordinal);
                }
                let mut root = Root::new();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare(&mut fixture, &mut root, 4097, 4096)
                }));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("currentness"))
                    ));
                }
                assert_prefix(
                    &fixture,
                    before,
                    [
                        ordinal,
                        usize::from(ordinal > 1),
                        usize::from(ordinal > 1),
                        0,
                        usize::from(ordinal == 4),
                    ],
                );
                assert!(root.started && root.failed);
                assert_eq!(root.native_started, ordinal > 1);
                assert_eq!(
                    (
                        root.progress.attempted,
                        root.progress.returned_success,
                        root.progress.returned_map_prefix
                    ),
                    (
                        ordinal == 4,
                        (ordinal == 4).then_some(true),
                        (ordinal == 4).then_some(1)
                    )
                );
                if ordinal <= 2 {
                    assert!(matches!(root.lease, Lease::None));
                } else {
                    assert_actual_lease(&fixture, &root, false, 4097);
                }
                if ordinal == 1 {
                    assert_eq!(fixture.memory.engine.next_device_memory_id, fixture.next_id);
                    assert_eq!(fixture.memory.engine.device_memory.len(), 1);
                    root.retain_live_failure(&mut fixture.memory.engine);
                    assert!(
                        fixture
                            .memory
                            .engine
                            .terminal_device_initialization
                            .is_none()
                    );
                    assert_usage(&fixture, 4096, 1, 0);
                    fixture.assert_anchor();
                    if panic && !configured {
                        assert_eq!(
                            fixture.memory.engine.phase(),
                            SharedMemorySessionPhaseV1::Active
                        );
                        clear_faults(&mut fixture);
                        let mut retry = Root::new();
                        prepare(&mut fixture, &mut retry, 17, 4096).unwrap();
                        let _output = retry.take_complete().unwrap();
                        fixture.assert_anchor();
                    } else {
                        no_retry(&mut fixture);
                    }
                } else {
                    assert_eq!(
                        fixture.memory.engine.next_device_memory_id,
                        fixture.next_id + 1
                    );
                    assert_eq!(fixture.memory.engine.device_memory.len(), 2);
                    assert_eq!(
                        fixture.memory.engine.device_memory[1].phase,
                        if ordinal == 3 {
                            DeviceMemoryPhaseV1::Unmapped
                        } else {
                            DeviceMemoryPhaseV1::Ambiguous
                        }
                    );
                    assert_usage(&fixture, 12_288, 2, 0);
                    retain_failure(&mut fixture, root);
                }
            }
        }
    }
}

#[test]
fn device_allocator_native_error_and_panic_prefixes_retain_actual_owners() {
    for configured in [false, true] {
        for panic in [false, true] {
            for operation in ["reserve_va", "alloc", "map_gpu"] {
                let mut fixture = Fixture::new(configured);
                let before = counters(&fixture);
                if panic {
                    fixture.memory.engine.backend.panic_operation = Some(operation);
                } else {
                    fixture.memory.engine.backend.fail_operation = Some(operation);
                }
                let mut root = Root::new();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare(&mut fixture, &mut root, 4097, 4096)
                }));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(
                        matches!(result.unwrap(), Err(MemorySessionError::Injected(actual)) if actual == operation)
                    );
                }
                assert_prefix(
                    &fixture,
                    before,
                    [
                        if operation == "map_gpu" { 3 } else { 1 },
                        1,
                        usize::from(operation != "reserve_va"),
                        0,
                        usize::from(operation == "map_gpu"),
                    ],
                );
                assert!(root.started && root.failed && root.native_started);
                assert_eq!(
                    (
                        root.progress.attempted,
                        root.progress.returned_success,
                        root.progress.returned_map_prefix
                    ),
                    (
                        operation == "map_gpu",
                        (operation == "map_gpu" && !panic).then_some(false),
                        (operation == "map_gpu" && !panic).then_some(1)
                    )
                );
                if operation == "map_gpu" {
                    assert_actual_lease(&fixture, &root, false, 4097);
                } else {
                    assert!(matches!(root.lease, Lease::None));
                }
                if operation == "reserve_va" {
                    assert_eq!(fixture.memory.engine.next_device_memory_id, fixture.next_id);
                    assert_eq!(fixture.memory.engine.device_memory.len(), 1);
                    assert_eq!(fixture.memory.engine.retained_device_memory_bytes, 4096);
                    assert_usage(&fixture, 12_288, 2, 1);
                } else {
                    let engine = &fixture.memory.engine;
                    assert_eq!(engine.next_device_memory_id, fixture.next_id + 1);
                    assert_eq!(engine.device_memory.len(), 2);
                    let record = &engine.device_memory[1];
                    assert_eq!(record.phase, DeviceMemoryPhaseV1::Ambiguous);
                    assert!(record.reservation.is_some());
                    assert!(!record.free_attempted);
                    let output = engine.backend.last_allocation_output.unwrap();
                    assert_ne!(output.handle, 0);
                    assert_eq!(
                        record.handle,
                        (!(operation == "alloc" && panic)).then_some(output.handle)
                    );
                    assert_eq!(
                        record.mmap_offset,
                        if operation == "alloc" && panic {
                            0
                        } else {
                            output.mmap_offset
                        }
                    );
                    assert_usage(&fixture, 12_288, 2, 0);
                }
                retain_failure(&mut fixture, root);
            }
        }
    }
}

#[test]
fn device_allocator_map_prefix_matrix_preserves_progress_and_error_precedence() {
    for configured in [false, true] {
        for prefix in [0, 1, 2] {
            for errno in [false, true] {
                let mut fixture = Fixture::new(configured);
                let before = counters(&fixture);
                fixture.memory.engine.backend.map_progress = prefix;
                fixture.memory.engine.backend.map_errno = errno;
                let mut root = Root::new();
                let result = prepare(&mut fixture, &mut root, 4097, 4096);
                assert_eq!(
                    (
                        root.progress.attempted,
                        root.progress.returned_success,
                        root.progress.returned_map_prefix
                    ),
                    (true, Some(!errno), Some(prefix))
                );
                let success = prefix == 1 && !errno;
                assert_prefix(&fixture, before, [if success { 4 } else { 3 }, 1, 1, 0, 1]);
                assert_actual_lease(&fixture, &root, success, 4097);
                assert_usage(&fixture, 12_288, 2, 0);
                if success {
                    result.unwrap();
                    assert_eq!(
                        fixture.memory.engine.device_memory[1].phase,
                        DeviceMemoryPhaseV1::Mapped
                    );
                    let _output = root.take_complete().unwrap();
                    fixture.assert_anchor();
                } else {
                    let expected = if prefix == 2 {
                        "device-memory MAP_MEMORY_TO_GPU cumulative n_success"
                    } else if errno {
                        "map_gpu"
                    } else {
                        "device-memory MAP_MEMORY_TO_GPU full prefix"
                    };
                    if errno && prefix != 2 {
                        assert!(
                            matches!(result, Err(MemorySessionError::Injected(actual)) if actual == expected)
                        );
                    } else {
                        assert!(
                            matches!(result, Err(MemorySessionError::KernelResultMalformed(actual)) if actual == expected)
                        );
                    }
                    assert_eq!(
                        fixture.memory.engine.device_memory[1].phase,
                        DeviceMemoryPhaseV1::Ambiguous
                    );
                    retain_failure(&mut fixture, root);
                }
            }
        }
    }
}

#[test]
fn device_allocator_malformed_allocations_never_fabricate_a_lease() {
    for configured in [false, true] {
        for fault in [
            "oom",
            "va",
            "size",
            "gpu",
            "flags",
            "handle",
            "offset",
            "alignment",
            "collision",
            "overlap",
        ] {
            let mut fixture = Fixture::new(configured);
            let before = counters(&fixture);
            let backend = &mut fixture.memory.engine.backend;
            match fault {
                "oom" => backend.alloc_oom = true,
                "va" => backend.allocation_output_mutator = Some(|args| args.va_addr += 4096),
                "size" => backend.allocation_output_mutator = Some(|args| args.size += 4096),
                "gpu" => backend.allocation_output_mutator = Some(|args| args.gpu_id += 1),
                "flags" => backend.corrupt_flags = true,
                "handle" => backend.allocation_output_mutator = Some(|args| args.handle = 0),
                "offset" => backend.allocation_output_mutator = Some(|args| args.mmap_offset = 0),
                "alignment" => {
                    backend.allocation_output_mutator = Some(|args| args.mmap_offset += 1)
                }
                "collision" => backend.allocation_output_mutator = Some(|args| args.handle = 1),
                "overlap" => backend.fixed_va = Some(fixture.anchor_native.gpu_va),
                _ => unreachable!(),
            }
            let mut root = Root::new();
            assert!(prepare(&mut fixture, &mut root, 4097, 4096).is_err());
            assert_prefix(
                &fixture,
                before,
                [1, 1, usize::from(fault != "overlap"), 0, 0],
            );
            assert!(matches!(root.lease, Lease::None));
            let engine = &fixture.memory.engine;
            assert_eq!(engine.next_device_memory_id, fixture.next_id + 1);
            assert_eq!(engine.device_memory.len(), 2);
            let record = &engine.device_memory[1];
            assert_eq!(record.phase, DeviceMemoryPhaseV1::Ambiguous);
            assert!(!record.free_attempted);
            assert!(record.reservation.is_some());
            if fault == "overlap" {
                assert!(record.handle.is_none());
                assert_eq!(record.mmap_offset, 0);
            } else {
                let output = engine.backend.last_allocation_output.unwrap();
                assert_eq!(record.handle, (output.handle != 0).then_some(output.handle));
                assert_eq!(record.mmap_offset, output.mmap_offset);
            }
            assert_usage(&fixture, 12_288, 2, 0);
            retain_failure(&mut fixture, root);
        }
    }
}

#[test]
fn device_allocator_invalid_layout_and_coordinates_reject_before_effects() {
    for configured in [false, true] {
        for invalid in [
            "zero",
            "overflow",
            "zero_alignment",
            "alignment",
            "large_alignment",
            "device",
            "vm",
        ] {
            let mut fixture = Fixture::new(configured);
            let before = counters(&fixture);
            let mut root = Root::new();
            let mut device = fixture.memory.device.model_key();
            let mut vm = fixture.memory.vm;
            let bytes = match invalid {
                "zero" => 0,
                "overflow" => u64::MAX,
                _ => 17,
            };
            let alignment = match invalid {
                "zero_alignment" => 0,
                "alignment" => 3,
                "large_alignment" => u64::MAX,
                _ => 4096,
            };
            if invalid == "device" {
                device.generation.0 += 1;
            }
            if invalid == "vm" {
                vm.device.generation.0 += 1;
            }
            let result =
                root.prepare_in_place(&mut fixture.memory.engine, device, vm, bytes, alignment);
            match invalid {
                "zero" | "overflow" => assert!(matches!(
                    result,
                    Err(MemorySessionError::InvalidDeviceMemorySize)
                )),
                "device" | "vm" => assert!(matches!(
                    result,
                    Err(MemorySessionError::InvalidDeviceMemoryAuthority)
                )),
                _ => assert!(matches!(
                    result,
                    Err(MemorySessionError::InvalidDeviceMemoryAlignment)
                )),
            }
            assert!(root.started && root.failed && !root.native_started);
            assert!(matches!(root.lease, Lease::None));
            assert!(!root.progress.attempted);
            root.retain_live_failure(&mut fixture.memory.engine);
            assert_prefix(&fixture, before, [0; 5]);
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            assert_eq!(fixture.memory.engine.next_device_memory_id, fixture.next_id);
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            assert_usage(&fixture, 4096, 1, 0);
            fixture.assert_anchor();
            let mut retry = Root::new();
            prepare(&mut fixture, &mut retry, 17, 4096).unwrap();
            let _output = retry.take_complete().unwrap();
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_allocator_capacity_rejection_and_real_release_allow_fresh_retry() {
    for configured in [false, true] {
        let mut fixture = Fixture::new(configured);
        let capacity = if configured {
            8
        } else {
            MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
        };
        let mut extras = Vec::new();
        for _ in 1..capacity {
            extras.push(
                fixture
                    .memory
                    .engine
                    .allocate_device_memory(
                        fixture.memory.device.model_key(),
                        fixture.memory.vm,
                        17,
                        4096,
                    )
                    .unwrap(),
            );
        }
        let before = counters(&fixture);
        let usage = fixture.memory.usage();
        let next_id = fixture.memory.engine.next_device_memory_id;
        let records: Vec<_> = fixture
            .memory
            .engine
            .device_memory
            .iter()
            .map(snapshot)
            .collect();
        let mut root = Root::new();
        let result = prepare(&mut fixture, &mut root, 17, 4096);
        if configured {
            assert!(matches!(
                result,
                Err(MemorySessionError::DeviceBackingCredits(_))
            ));
        } else {
            assert!(
                matches!(result, Err(MemorySessionError::DeviceMemoryAllocationCapacity { maximum }) if maximum == capacity)
            );
        }
        assert!(!root.requires_retention());
        assert!(matches!(root.lease, Lease::None));
        root.retain_live_failure(&mut fixture.memory.engine);
        assert_prefix(&fixture, before, [0; 5]);
        assert_eq!(fixture.memory.usage(), usage);
        assert_eq!(fixture.memory.engine.next_device_memory_id, next_id);
        assert_eq!(
            fixture
                .memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect::<Vec<_>>(),
            records
        );
        assert!(
            fixture
                .memory
                .engine
                .terminal_device_initialization
                .is_none()
        );
        assert_eq!(
            fixture.memory.engine.phase(),
            SharedMemorySessionPhaseV1::Active
        );
        fixture.assert_anchor();
        fixture
            .memory
            .engine
            .release_device_memory(extras.pop().unwrap())
            .unwrap();
        let mut retry = Root::new();
        prepare(&mut fixture, &mut retry, 17, 4096).unwrap();
        let _output = retry.take_complete().unwrap();
        assert_eq!(
            snapshot(&fixture.memory.engine.device_memory[0]),
            fixture.anchor_native
        );
        assert_eq!(fixture.memory.engine.backend.free_calls, 1);
        assert_eq!(fixture.memory.engine.backend.release_va_calls, 1);
        assert_eq!(fixture.memory.engine.backend.unmap_gpu_calls, 0);
    }
}

#[test]
fn device_allocator_complete_output_can_be_retained_without_reallocation_on_default_stack() {
    assert!(
        std::mem::size_of::<init::TerminalInitializationSlotV1>()
            <= 3 * std::mem::size_of::<usize>()
    );
    for configured in [false, true] {
        let mut fixture = Fixture::new(configured);
        assert!(fixture.terminal_storage.1 >= 1);
        let mut root = Root::new();
        prepare(&mut fixture, &mut root, 4097, 4096).unwrap();
        assert_actual_lease(&fixture, &root, true, 4097);
        let mut expected = allocation_snapshot(&root);
        expected.failed = true;
        root.retain_live_failure(&mut fixture.memory.engine);
        let terminal = fixture
            .memory
            .engine
            .terminal_device_initialization
            .allocation_as_ref()
            .unwrap();
        assert_eq!(allocation_snapshot(terminal), expected);
        assert!(terminal.completed().is_err());
        assert_eq!(
            fixture.memory.engine.device_memory[1].phase,
            DeviceMemoryPhaseV1::Mapped
        );
        assert_usage(&fixture, 12_288, 2, 0);
        no_retry(&mut fixture);
    }
}

#[test]
fn device_allocator_mixed_terminal_kinds_block_each_other_without_overwrite() {
    for configured in [false, true] {
        for initialized_first in [false, true] {
            let mut fixture = Fixture::new(configured);
            fixture.memory.engine.backend.fail_operation = Some("map_gpu");
            if initialized_first {
                assert!(Input::new(false, 4097).run(&mut fixture, 4096).is_err());
            } else {
                let mut root = Root::new();
                assert!(prepare(&mut fixture, &mut root, 4097, 4096).is_err());
                root.retain_live_failure(&mut fixture.memory.engine);
            }
            let engine = &fixture.memory.engine;
            let initialized = engine
                .terminal_device_initialization
                .as_ref()
                .map(root_snapshot);
            let allocation = engine
                .terminal_device_initialization
                .allocation_as_ref()
                .map(allocation_snapshot);
            assert_eq!(initialized.is_some(), initialized_first);
            assert_eq!(allocation.is_some(), !initialized_first);
            let records: Vec<_> = engine.device_memory.iter().map(snapshot).collect();
            let calls = fixture.calls();
            let usage = fixture.memory.usage();
            clear_faults(&mut fixture);
            // Isolate the mixed-kind slot guard from the earlier phase guard.
            fixture.memory.engine.phase = SharedMemorySessionPhaseV1::Active;
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_some()
            );
            assert!(
                !fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            if initialized_first {
                let mut root = Root::new();
                assert!(matches!(
                    prepare(&mut fixture, &mut root, 17, 4096),
                    Err(MemorySessionError::SharedSessionQuarantined)
                ));
                assert!(!root.requires_retention());
                root.retain_live_failure(&mut fixture.memory.engine);
            } else {
                assert!(matches!(
                    Input::new(false, 17).run(&mut fixture, 4096),
                    Err(MemorySessionError::SharedSessionQuarantined)
                ));
            }
            assert_eq!(fixture.calls(), calls);
            assert_eq!(fixture.memory.usage(), usage);
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .device_memory
                    .iter()
                    .map(snapshot)
                    .collect::<Vec<_>>(),
                records
            );
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .as_ref()
                    .map(root_snapshot),
                initialized
            );
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .allocation_as_ref()
                    .map(allocation_snapshot),
                allocation
            );
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if initialized_first {
                    fixture
                        .memory
                        .engine
                        .terminal_device_initialization
                        .retain_allocation(Root::new());
                } else {
                    fixture.memory.engine.terminal_device_initialization.retain(
                        init::DeviceInitializationCustodyV1::new(
                            init::InitializationSourceV1::Repeated(repeated_content(17, 0x33)),
                            init::InitializationLeaseV1::None,
                        ),
                    );
                }
            }));
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"occupied initialization custody")
            );
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .as_ref()
                    .map(root_snapshot),
                initialized
            );
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .allocation_as_ref()
                    .map(allocation_snapshot),
                allocation
            );
            assert_usage(&fixture, 12_288, 2, 0);
            no_retry(&mut fixture);
        }
    }
}

#[test]
fn device_allocator_preflight_arithmetic_and_configured_domain_are_side_effect_free() {
    for configured in [false, true] {
        for fault in ["id", "bytes_overflow", "bytes_limit", "domain", "vm_id"] {
            if !configured && matches!(fault, "domain" | "vm_id") {
                continue;
            }
            let mut fixture = Fixture::new(configured);
            let before = counters(&fixture);
            let usage = fixture.memory.usage();
            let records: Vec<_> = fixture
                .memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect();
            let old_bytes = fixture.memory.engine.retained_device_memory_bytes;
            let mut device = fixture.memory.device.model_key();
            let mut vm = fixture.memory.vm;
            match fault {
                "id" => fixture.memory.engine.next_device_memory_id = u64::MAX,
                "bytes_overflow" => fixture.memory.engine.retained_device_memory_bytes = u64::MAX,
                "bytes_limit" => {
                    fixture.memory.engine.retained_device_memory_bytes =
                        MAX_GFX942_DEVICE_MEMORY_BYTES_V1
                }
                "domain" => {
                    device.generation.0 += 1;
                    vm.device = device;
                }
                "vm_id" => vm.id.0 += 1,
                _ => unreachable!(),
            }
            let expected_id = fixture.memory.engine.next_device_memory_id;
            let expected_bytes = fixture.memory.engine.retained_device_memory_bytes;
            let mut root = Root::new();
            let result = root.prepare_in_place(&mut fixture.memory.engine, device, vm, 17, 4096);
            match fault {
                "id" | "bytes_overflow" => {
                    assert!(matches!(result, Err(MemorySessionError::SizeOverflow)))
                }
                "bytes_limit" => assert!(
                    matches!(result, Err(MemorySessionError::DeviceMemoryByteCapacity { maximum_bytes }) if maximum_bytes == MAX_GFX942_DEVICE_MEMORY_BYTES_V1)
                ),
                _ => assert!(matches!(
                    result,
                    Err(MemorySessionError::InvalidDeviceMemoryAuthority)
                )),
            }
            assert!(root.started && root.failed && !root.native_started);
            assert!(matches!(root.lease, Lease::None));
            assert_eq!(root.progress, Default::default());
            root.retain_live_failure(&mut fixture.memory.engine);
            assert_prefix(&fixture, before, [0; 5]);
            assert_eq!(fixture.memory.usage(), usage);
            assert_eq!(fixture.memory.engine.next_device_memory_id, expected_id);
            assert_eq!(
                fixture.memory.engine.retained_device_memory_bytes,
                expected_bytes
            );
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .device_memory
                    .iter()
                    .map(snapshot)
                    .collect::<Vec<_>>(),
                records
            );
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            fixture.memory.engine.next_device_memory_id = fixture.next_id;
            fixture.memory.engine.retained_device_memory_bytes = old_bytes;
            fixture.assert_anchor();
            let mut retry = Root::new();
            prepare(&mut fixture, &mut retry, 17, 4096).unwrap();
            let _output = retry.take_complete().unwrap();
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_allocator_independent_credit_limits_release_only_after_actual_disposal() {
    for byte_limit in [false, true] {
        let budget = if byte_limit {
            Gfx942DeviceBackingBudgetV1::new(12_288, 8).unwrap()
        } else {
            Gfx942DeviceBackingBudgetV1::new(16_384, 2).unwrap()
        };
        let mut fixture = Fixture::with_budget(Some(budget));
        let extra = fixture
            .memory
            .engine
            .allocate_device_memory(
                fixture.memory.device.model_key(),
                fixture.memory.vm,
                17,
                4096,
            )
            .unwrap();
        let before = counters(&fixture);
        let usage = fixture.memory.usage();
        let records: Vec<_> = fixture
            .memory
            .engine
            .device_memory
            .iter()
            .map(snapshot)
            .collect();
        let next_id = fixture.memory.engine.next_device_memory_id;
        let bytes = if byte_limit { 4097 } else { 17 };
        let mut root = Root::new();
        assert!(matches!(
            prepare(&mut fixture, &mut root, bytes, 4096),
            Err(MemorySessionError::DeviceBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            ))
        ));
        assert!(root.started && root.failed && !root.native_started);
        assert!(matches!(root.lease, Lease::None));
        assert_eq!(root.progress, Default::default());
        root.retain_live_failure(&mut fixture.memory.engine);
        assert_prefix(&fixture, before, [0; 5]);
        assert_eq!(fixture.memory.usage(), usage);
        assert_eq!(
            fixture
                .memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect::<Vec<_>>(),
            records
        );
        assert_eq!(fixture.memory.engine.next_device_memory_id, next_id);
        assert_eq!(
            fixture.memory.engine.phase(),
            SharedMemorySessionPhaseV1::Active
        );
        assert!(
            fixture
                .memory
                .engine
                .terminal_device_initialization
                .is_none()
        );
        fixture.assert_anchor();
        fixture.memory.engine.release_device_memory(extra).unwrap();
        assert_usage(&fixture, 4096, 1, 0);
        let mut retry = Root::new();
        prepare(&mut fixture, &mut retry, bytes, 4096).unwrap();
        let output = retry.take_complete().unwrap();
        assert_usage(&fixture, 4096 + output.layout.backing_bytes, 2, 0);
        assert_eq!(
            snapshot(&fixture.memory.engine.device_memory[0]),
            fixture.anchor_native
        );
        assert_eq!(fixture.memory.engine.backend.free_calls, 1);
        assert_eq!(fixture.memory.engine.backend.release_va_calls, 1);
        assert_eq!(fixture.memory.engine.backend.unmap_gpu_calls, 0);
    }
}
