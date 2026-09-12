//! CPU/shared-sequence tests; FakeBackend does not qualify Linux or GPU behavior.
use super::*;
use crate::shared_memory::device_initialization::{
    self as init, DeviceInitializationStageV1 as Stage,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeSnapshot {
    identity: (u64, u64, DeviceKeyV1, VmKeyV1, Gfx942DeviceMemoryLayoutV1),
    gpu_va: u64,
    mmap_offset: u64,
    reservation: Option<(u64, usize)>,
    handle: Option<u64>,
    free_attempted: bool,
    phase: DeviceMemoryPhaseV1,
    mapping: Option<(u64, Vec<u8>, usize, bool, bool, usize)>,
}

pub(super) fn snapshot(record: &DeviceMemoryRecord<FakeBackend>) -> NativeSnapshot {
    NativeSnapshot {
        identity: (
            record.id,
            record.generation,
            record.device,
            record.vm,
            record.layout,
        ),
        gpu_va: record.gpu_va,
        mmap_offset: record.mmap_offset,
        reservation: record.reservation,
        handle: record.handle,
        free_attempted: record.free_attempted,
        phase: record.phase,
        mapping: record.mapping.as_ref().map(|mapping| {
            (
                mapping.address,
                mapping.bytes.clone(),
                mapping.byte_offset,
                mapping.active,
                mapping.writable,
                mapping.readback_calls.get(),
            )
        }),
    }
}

type LeaseIdentity = (u64, u64, DeviceKeyV1, VmKeyV1, Gfx942DeviceMemoryLayoutV1);

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceSnapshot {
    Unvalidated(usize, Vec<u8>, Gfx942DeviceContentDescriptorV1),
    Validated(usize, Vec<u8>, u64, Gfx942DeviceContentDescriptorV1),
    Repeated(u8, Gfx942DeviceContentDescriptorV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RootSnapshot {
    source: SourceSnapshot,
    lease: Option<(LeaseIdentity, Option<Gfx942DeviceContentDescriptorV1>)>,
    stage: Stage,
    native_started: bool,
    input_admitted: bool,
    progress: crate::shared_memory::transitions::NativeTransitionProgressV1,
    failed: bool,
}

pub(super) fn root_snapshot(root: &init::DeviceInitializationCustodyV1) -> RootSnapshot {
    let source = match root.source.as_ref().unwrap() {
        init::InitializationSourceV1::Unvalidated(bytes, content) => {
            SourceSnapshot::Unvalidated(bytes.as_ptr() as usize, bytes.to_vec(), *content)
        }
        init::InitializationSourceV1::Validated(source) => SourceSnapshot::Validated(
            source.bytes().as_ptr() as usize,
            source.bytes().to_vec(),
            source.byte_len(),
            source.content(),
        ),
        init::InitializationSourceV1::Repeated(recipe) => {
            SourceSnapshot::Repeated(recipe.repeated_byte(), recipe.content())
        }
    };
    let lease = match &root.lease {
        init::InitializationLeaseV1::None => None,
        init::InitializationLeaseV1::Unmapped(lease) => Some((
            (
                lease.id,
                lease.generation,
                lease.device,
                lease.vm,
                lease.layout,
            ),
            None,
        )),
        init::InitializationLeaseV1::Complete(output) => {
            let lease = &output.lease;
            Some((
                (
                    lease.id,
                    lease.generation,
                    lease.device,
                    lease.vm,
                    lease.layout,
                ),
                Some(output.content()),
            ))
        }
    };
    RootSnapshot {
        source,
        lease,
        stage: root.stage,
        native_started: root.native_started,
        input_admitted: root.input_admitted,
        progress: root.progress,
        failed: root.failed,
    }
}

impl RootSnapshot {
    pub(crate) fn assert_source(
        &self,
        pointer: usize,
        bytes: &[u8],
        content: Gfx942DeviceContentDescriptorV1,
    ) {
        assert_eq!(
            matches!(self.source, SourceSnapshot::Validated(..)),
            self.stage != Stage::Source,
            "exact source validation variant"
        );
        if let SourceSnapshot::Validated(_, _, len, _) = &self.source {
            assert_eq!(*len, bytes.len() as u64);
            assert_eq!(*len, content.byte_len());
        }
        match &self.source {
            SourceSnapshot::Unvalidated(actual, data, descriptor)
            | SourceSnapshot::Validated(actual, data, _, descriptor) => {
                assert_eq!(*actual, pointer, "original insertion source allocation");
                assert_eq!(data, bytes);
                assert_eq!(*descriptor, content);
            }
            SourceSnapshot::Repeated(..) => {
                panic!("owned-byte insertion cannot replace its source")
            }
        }
        if self.stage == Stage::Complete {
            assert_eq!(
                self.lease.as_ref().and_then(|(_, content)| *content),
                Some(content),
                "Complete retains exact initialized content until extraction"
            );
        }
    }

    pub(crate) fn with_failure_for_test(&self) -> Self {
        let mut result = self.clone();
        result.failed = true;
        result
    }

    pub(crate) fn stage_name(&self) -> String {
        format!("{:?}", self.stage)
    }

    pub(crate) fn failed(&self) -> bool {
        self.failed
    }

    pub(crate) fn native_started(&self) -> bool {
        self.native_started
    }

    pub(crate) fn lease(
        &self,
    ) -> Option<(
        Gfx942DeviceMemoryIdentityV1,
        Gfx942DeviceMemoryLayoutV1,
        bool,
    )> {
        self.lease
            .map(|((id, generation, device, vm, layout), content)| {
                (
                    Gfx942DeviceMemoryIdentityV1 {
                        id,
                        generation,
                        device,
                        vm,
                    },
                    layout,
                    content.is_some(),
                )
            })
    }

    pub(crate) fn progress(&self) -> (bool, Option<bool>, Option<u32>) {
        (
            self.progress.attempted,
            self.progress.returned_success,
            self.progress.returned_map_prefix,
        )
    }
}

struct Input {
    bytes: Option<Box<[u8]>>,
    expected: Vec<u8>,
    pointer: Option<usize>,
    content: Gfx942DeviceContentDescriptorV1,
    recipe: Option<Gfx942RepeatedByteContentV1>,
}

impl Input {
    fn new(repeated: bool, len: usize) -> Self {
        let expected: Vec<_> = (0..len)
            .map(|i| if repeated { 0x5a } else { (i % 251) as u8 })
            .collect();
        let recipe = repeated.then(|| repeated_content(len as u64, 0x5a));
        let content = recipe.map_or_else(|| content(&expected), |recipe| recipe.content());
        let bytes = (!repeated).then(|| expected.clone().into_boxed_slice());
        let pointer = bytes.as_ref().map(|bytes| bytes.as_ptr() as usize);
        Self {
            bytes,
            expected,
            pointer,
            content,
            recipe,
        }
    }

    fn run(
        &mut self,
        fixture: &mut Fixture,
        alignment: u64,
    ) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
        let memory = &mut fixture.memory;
        match self.recipe {
            Some(recipe) => init::initialize_repeated_v1(
                &mut memory.engine,
                memory.device.model_key(),
                memory.vm,
                recipe,
                alignment,
            ),
            None => init::initialize_bytes_v1(
                &mut memory.engine,
                memory.device.model_key(),
                memory.vm,
                self.bytes.take().unwrap(),
                alignment,
                self.content,
            ),
        }
    }

    fn assert_source(&self, root: &init::DeviceInitializationCustodyV1) {
        match root.source.as_ref().unwrap() {
            init::InitializationSourceV1::Validated(source) => {
                assert!(self.recipe.is_none());
                assert_eq!(Some(source.bytes().as_ptr() as usize), self.pointer);
                assert_eq!(source.bytes(), self.expected);
                assert_eq!(source.byte_len(), self.expected.len() as u64);
                assert_eq!(source.content(), self.content);
            }
            init::InitializationSourceV1::Repeated(recipe) => {
                assert!(self.pointer.is_none());
                assert_eq!(recipe.content(), self.content);
                assert_eq!(recipe.repeated_byte(), 0x5a);
            }
            init::InitializationSourceV1::Unvalidated(..) => {
                panic!("expected authenticated source")
            }
        }
    }
}

struct Fixture {
    memory: BackingConstructorFixture,
    anchor: Gfx942DeviceMemoryDispatchAuthorityV1,
    anchor_native: NativeSnapshot,
    identity: model::DeviceIdentityStateV1,
    foundation: model::MemoryLifecycleStateV1,
    account: Option<usize>,
    next_id: u64,
    terminal_storage: (usize, usize),
}

impl Fixture {
    fn new(configured: bool) -> Self {
        let mut memory = BackingConstructorFixture::new(
            configured.then(|| Gfx942DeviceBackingBudgetV1::new(32_768, 8).unwrap()),
        );
        let anchor = memory.mapped_device();
        let anchor_native = snapshot(&memory.engine.device_memory[0]);
        let identity = memory.foundation.identity().clone();
        let foundation = memory.foundation.memory().clone();
        let account = memory
            .engine
            .device_backing_account
            .as_ref()
            .map(DeviceBackingAccountV1::domain_identity_for_test);
        let next_id = memory.engine.next_device_memory_id;
        let terminal_storage = memory
            .engine
            .terminal_device_initialization
            .storage_for_test();
        Self {
            memory,
            anchor,
            anchor_native,
            identity,
            foundation,
            account,
            next_id,
            terminal_storage,
        }
    }

    fn assert_anchor(&self) {
        let engine = &self.memory.engine;
        assert_eq!(
            engine.terminal_device_initialization.storage_for_test(),
            self.terminal_storage
        );
        assert_eq!(
            engine
                .device_backing_account
                .as_ref()
                .map(DeviceBackingAccountV1::domain_identity_for_test),
            self.account
        );
        assert_eq!(snapshot(&engine.device_memory[0]), self.anchor_native);
        assert_eq!(self.anchor.lease.id, self.anchor_native.identity.0);
        assert_eq!(self.anchor.lease.generation, self.anchor_native.identity.1);
        assert_eq!(self.anchor.lease.device, self.anchor_native.identity.2);
        assert_eq!(self.anchor.lease.vm, self.anchor_native.identity.3);
        assert_eq!(self.anchor.lease.layout, self.anchor_native.identity.4);
        assert_eq!(self.memory.foundation.identity(), &self.identity);
        assert_eq!(self.memory.foundation.memory(), &self.foundation);
        assert_eq!(
            self.memory.ownership.phase,
            QueueModelOwnershipPhaseV1::SessionOwned
        );
        assert_eq!(self.memory.ownership.next_live_loan_generation, 1);
        for record in &engine.device_memory {
            match (&engine.device_backing_account, &record.backing_charge) {
                (None, None) => {}
                (Some(account), Some(charge)) => assert!(charge.matches(
                    account,
                    engine.session_id,
                    record.device,
                    record.vm,
                    record.id,
                    record.generation,
                    record.layout,
                )),
                _ => panic!("original account/charge mismatch"),
            }
        }
        assert_eq!(engine.backend.unmap_gpu_calls, 0);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }

    fn assert_usage(&self, bytes: u64, records: u64, quarantined: usize) {
        if let Some(account) = &self.memory.engine.device_backing_account {
            assert_eq!(Some(account.domain_identity_for_test()), self.account);
            let usage = account.usage();
            assert_eq!(usage.used_backing_bytes, bytes);
            assert_eq!(usage.used_allocation_records, records);
            assert_eq!(usage.reserved_records, 0);
            assert_eq!(usage.quarantined_records, quarantined);
            assert_eq!(usage.retained_records, records as usize - quarantined);
            assert_eq!(self.memory.usage(), Some(usage));
        }
    }

    fn assert_terminal(&self, input: &Input, stage: Stage, has_lease: bool) {
        let engine = &self.memory.engine;
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        let root = engine.terminal_device_initialization.as_ref().unwrap();
        assert!(root.failed);
        assert!(root.native_started);
        assert!(!root.input_admitted);
        assert_eq!(root.stage, stage);
        input.assert_source(root);
        match &root.lease {
            init::InitializationLeaseV1::None => assert!(!has_lease),
            init::InitializationLeaseV1::Unmapped(lease) => {
                assert!(has_lease);
                let record = &engine.device_memory[1];
                assert_eq!(
                    (
                        lease.id,
                        lease.generation,
                        lease.device,
                        lease.vm,
                        lease.layout
                    ),
                    snapshot(record).identity
                );
                assert_eq!(
                    (
                        lease.id,
                        lease.generation,
                        lease.device,
                        lease.vm,
                        lease.layout
                    ),
                    (
                        self.next_id,
                        1,
                        self.memory.device.model_key(),
                        self.memory.vm,
                        device_memory_layout(
                            input.expected.len() as u64,
                            4096,
                            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC
                        )
                        .unwrap(),
                    )
                );
            }
            init::InitializationLeaseV1::Complete(_) => {
                panic!("failure returned completed authority")
            }
        }
        self.assert_anchor();
    }

    fn no_retry(&mut self, input: &Input) {
        assert_eq!(
            self.memory.engine.phase(),
            SharedMemorySessionPhaseV1::Quarantined
        );
        let records: Vec<_> = self
            .memory
            .engine
            .device_memory
            .iter()
            .map(snapshot)
            .collect();
        let calls = self.calls();
        let usage = self.memory.usage();
        let engine = &mut self.memory.engine;
        let terminal = engine
            .terminal_device_initialization
            .as_ref()
            .map(root_snapshot);
        engine.backend.fail_operation = None;
        engine.backend.panic_operation = None;
        engine.backend.fail_currentness_at = None;
        engine.backend.panic_currentness_at = None;
        engine.backend.map_errno = false;
        engine.backend.map_progress = 1;
        engine.backend.corrupt_readback = false;
        assert!(matches!(
            Input::new(false, 17).run(self, 4096),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
        assert_eq!(self.calls(), calls);
        assert_eq!(self.memory.usage(), usage);
        assert_eq!(
            self.memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect::<Vec<_>>(),
            records
        );
        assert_eq!(
            self.memory
                .engine
                .terminal_device_initialization
                .as_ref()
                .map(root_snapshot),
            terminal
        );
        if let Some(root) = self.memory.engine.terminal_device_initialization.as_ref() {
            input.assert_source(root);
        }
        self.assert_anchor();
    }

    fn calls(&self) -> (usize, usize, usize, usize, usize, Vec<&'static str>) {
        let backend = &self.memory.engine.backend;
        (
            backend.currentness_calls,
            backend.reserve_va_calls,
            backend.alloc_calls,
            backend.map_cpu_calls,
            backend.map_gpu_calls,
            backend.operations.clone(),
        )
    }
}

#[test]
fn device_initializer_complete_entry_preserves_exact_success_and_readback_policy() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for len in [1, 4096, 4097] {
                let mut fixture = Fixture::new(configured);
                let mut input = Input::new(repeated, len);
                fixture.memory.engine.backend.corrupt_readback = repeated;
                let id = fixture.memory.engine.next_device_memory_id;
                let currentness = fixture.memory.engine.backend.currentness_calls;
                let output = input.run(&mut fixture, 4096).unwrap();
                let engine = &fixture.memory.engine;
                let record = &engine.device_memory[1];
                assert_eq!(output.lease.id, id);
                assert_eq!(output.lease.generation, 1);
                assert_eq!(output.lease.device, fixture.memory.device.model_key());
                assert_eq!(output.lease.vm, fixture.memory.vm);
                assert_eq!(output.lease.layout, record.layout);
                assert_eq!(output.content(), input.content);
                assert_eq!(engine.backend.currentness_calls, currentness + 6);
                assert_eq!(record.phase, DeviceMemoryPhaseV1::Mapped);
                assert!(record.mapping.is_none());
                assert!(engine.terminal_device_initialization.is_none());
                assert_eq!(
                    engine.backend.flags[1],
                    KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
                );
                assert_eq!(
                    engine.backend.last_unmapped_readback_calls,
                    usize::from(!repeated)
                );
                let bytes = engine.backend.last_unmapped_bytes.as_ref().unwrap();
                assert_eq!(&bytes[..len], input.expected);
                assert!(bytes[len..].iter().all(|byte| *byte == 0));
                assert_eq!(
                    engine.backend.map_cpu_inputs,
                    [(
                        record.reservation.unwrap(),
                        record.mmap_offset,
                        record.layout.backing_bytes as usize
                    )]
                );
                assert_eq!(
                    engine.backend.map_gpu_inputs[1],
                    (record.handle.unwrap(), 0)
                );
                assert_eq!(
                    &engine.backend.operations[1..],
                    ["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"]
                );
                fixture.assert_usage(4096 + record.layout.backing_bytes, 2, 0);
                fixture.assert_anchor();
            }
        }
    }
}

#[test]
fn device_initializer_currentness_matrix_retains_only_admitted_owners() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for panic in [false, true] {
                for ordinal in 1..=6 {
                    let mut fixture = Fixture::new(configured);
                    let mut input = Input::new(repeated, 4097);
                    let backend = &mut fixture.memory.engine.backend;
                    let at = backend.currentness_calls + ordinal;
                    if panic {
                        backend.panic_currentness_at = Some(at);
                    } else {
                        backend.fail_currentness_at = Some(at);
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        input.run(&mut fixture, 4096)
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
                    if ordinal == 1 {
                        assert!(
                            fixture
                                .memory
                                .engine
                                .terminal_device_initialization
                                .is_none()
                        );
                        assert_eq!(fixture.memory.engine.device_memory.len(), 1);
                        fixture.assert_usage(4096, 1, 0);
                        fixture.assert_anchor();
                        if panic && !configured {
                            assert_eq!(
                                fixture.memory.engine.phase(),
                                SharedMemorySessionPhaseV1::Active
                            );
                            fixture.memory.engine.backend.panic_currentness_at = None;
                            let _initialized =
                                Input::new(repeated, 17).run(&mut fixture, 4096).unwrap();
                            fixture.assert_anchor();
                        } else {
                            fixture.no_retry(&input);
                        }
                        continue;
                    }
                    let stage = match ordinal {
                        2 => Stage::Allocate,
                        3 => Stage::CpuCurrentness,
                        4 => Stage::CpuClosingCurrentness,
                        _ => Stage::GpuMap,
                    };
                    fixture.assert_terminal(&input, stage, ordinal >= 3);
                    let engine = &fixture.memory.engine;
                    assert_eq!(engine.device_memory.len(), 2);
                    assert!(engine.device_memory[1].mapping.is_none());
                    assert_eq!(
                        engine.device_memory[1].phase,
                        if ordinal == 2 || ordinal == 6 {
                            DeviceMemoryPhaseV1::Ambiguous
                        } else {
                            DeviceMemoryPhaseV1::Unmapped
                        }
                    );
                    let progress = engine
                        .terminal_device_initialization
                        .as_ref()
                        .unwrap()
                        .progress;
                    assert_eq!(progress.attempted, ordinal == 6);
                    assert_eq!(progress.returned_success, (ordinal == 6).then_some(true));
                    assert_eq!(progress.returned_map_prefix, (ordinal == 6).then_some(1));
                    fixture.assert_usage(12_288, 2, 0);
                    fixture.no_retry(&input);
                }
            }
        }
    }
}

#[test]
fn device_initializer_native_error_and_panic_prefixes_preserve_original_source() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for panic in [false, true] {
                for (operation, stage) in [
                    ("reserve_va", Stage::Allocate),
                    ("alloc", Stage::Allocate),
                    ("map_cpu", Stage::CpuMap),
                    ("prepare_cpu_mapping", Stage::CpuPrepare),
                    ("unmap_cpu", Stage::CpuUnmap),
                    ("map_gpu", Stage::GpuMap),
                    ("with_bytes_mut", Stage::CpuWrite),
                    ("with_bytes", Stage::CpuVerify),
                ] {
                    if (operation.starts_with("with_bytes") && !panic)
                        || (operation == "with_bytes" && repeated)
                    {
                        continue;
                    }
                    let mut fixture = Fixture::new(configured);
                    let mut input = Input::new(repeated, 4097);
                    if panic {
                        fixture.memory.engine.backend.panic_operation = Some(operation);
                    } else {
                        fixture.memory.engine.backend.fail_operation = Some(operation);
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        input.run(&mut fixture, 4096)
                    }));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", operation))
                        );
                    } else {
                        assert!(
                            matches!(result.unwrap(), Err(MemorySessionError::Injected(observed)) if observed == operation)
                        );
                    }
                    let allocated = operation != "reserve_va" && operation != "alloc";
                    fixture.assert_terminal(&input, stage, allocated);
                    let engine = &fixture.memory.engine;
                    if operation == "reserve_va" {
                        assert_eq!(engine.device_memory.len(), 1);
                        assert_eq!(engine.retained_device_memory_bytes, 4096);
                        fixture.assert_usage(12_288, 2, 1);
                    } else {
                        let record = &engine.device_memory[1];
                        assert_eq!(record.handle.is_some(), !(operation == "alloc" && panic));
                        let mapped = matches!(
                            operation,
                            "prepare_cpu_mapping" | "unmap_cpu" | "with_bytes" | "with_bytes_mut"
                        );
                        assert_eq!(record.mapping.is_some(), mapped);
                        if let Some(mapping) = &record.mapping {
                            assert!(mapping.active);
                            assert_eq!(mapping.writable, operation != "prepare_cpu_mapping");
                            if operation == "unmap_cpu" || operation == "with_bytes" {
                                assert_eq!(&mapping.bytes[..4097], input.expected);
                            } else {
                                assert!(mapping.bytes.iter().all(|byte| *byte == 0));
                            }
                            assert!(mapping.bytes[4097..].iter().all(|byte| *byte == 0));
                        }
                        let progress = engine
                            .terminal_device_initialization
                            .as_ref()
                            .unwrap()
                            .progress;
                        assert_eq!(progress.attempted, operation == "map_gpu");
                        assert_eq!(
                            progress.returned_success,
                            (operation == "map_gpu" && !panic).then_some(false)
                        );
                        assert_eq!(
                            progress.returned_map_prefix,
                            (operation == "map_gpu" && !panic).then_some(1)
                        );
                        fixture.assert_usage(12_288, 2, 0);
                    }
                    fixture.no_retry(&input);
                }
            }
        }
    }
}

#[test]
fn device_initializer_gpu_prefixes_and_readback_rejection_never_produce_output() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for prefix in [0, 1, 2] {
                for errno in [false, true] {
                    if prefix == 1 && !errno {
                        continue;
                    }
                    let mut fixture = Fixture::new(configured);
                    let mut input = Input::new(repeated, 4097);
                    fixture.memory.engine.backend.map_progress = prefix;
                    fixture.memory.engine.backend.map_errno = errno;
                    assert!(input.run(&mut fixture, 4096).is_err());
                    fixture.assert_terminal(&input, Stage::GpuMap, true);
                    let engine = &fixture.memory.engine;
                    let progress = engine
                        .terminal_device_initialization
                        .as_ref()
                        .unwrap()
                        .progress;
                    assert!(progress.attempted);
                    assert_eq!(progress.returned_success, Some(!errno));
                    assert_eq!(progress.returned_map_prefix, Some(prefix));
                    assert_eq!(
                        engine.device_memory[1].phase,
                        DeviceMemoryPhaseV1::Ambiguous
                    );
                    assert!(engine.device_memory[1].mapping.is_none());
                    fixture.assert_usage(12_288, 2, 0);
                    fixture.no_retry(&input);
                }
            }
        }
        let mut fixture = Fixture::new(configured);
        let mut input = Input::new(false, 4097);
        fixture.memory.engine.backend.corrupt_readback = true;
        assert!(matches!(
            input.run(&mut fixture, 4096),
            Err(MemorySessionError::DeviceContentMismatch)
        ));
        fixture.assert_terminal(&input, Stage::CpuVerify, true);
        assert_eq!(
            fixture.memory.engine.device_memory[1]
                .mapping
                .as_ref()
                .unwrap()
                .readback_calls
                .get(),
            1
        );
        fixture.no_retry(&input);
    }
}

#[test]
fn device_initializer_preflight_rejects_before_native_effects_despite_prior_activity() {
    for configured in [false, true] {
        for invalid in ["empty", "length", "digest", "mutated", "alignment"] {
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            match invalid {
                "empty" => input.bytes = Some(Vec::new().into_boxed_slice()),
                "length" => input.bytes = Some(vec![1; 16].into_boxed_slice()),
                "digest" => input.content = content(&[1; 17]),
                "mutated" => input.bytes.as_mut().unwrap()[0] ^= 1,
                _ => {}
            }
            let calls = fixture.calls();
            assert!(
                input
                    .run(&mut fixture, if invalid == "alignment" { 3 } else { 4096 })
                    .is_err()
            );
            assert_eq!(fixture.calls(), calls);
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
            fixture.assert_usage(4096, 1, 0);
            fixture.assert_anchor();
            let _initialized = Input::new(false, 17).run(&mut fixture, 4096).unwrap();
        }
    }
}

#[test]
fn device_initializer_existing_lease_preflight_retains_genuine_native_input() {
    for configured in [false, true] {
        for invalid in ["length", "validated_length", "validated_content", "profile"] {
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            let mut source =
                validate_initialization_source(input.bytes.take().unwrap(), input.content).unwrap();
            if invalid == "validated_length" {
                source.byte_len += 1;
            }
            if invalid == "validated_content" {
                source.content = content(&[1; 18]);
            }
            let lease = fixture
                .memory
                .engine
                .allocate_device_memory_with_flags(
                    fixture.memory.device.model_key(),
                    fixture.memory.vm,
                    if invalid == "length" { 18 } else { 17 },
                    4096,
                    if invalid == "profile" {
                        KfdAllocMemoryFlags::DEVICE_LOCAL
                    } else {
                        KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC
                    },
                )
                .unwrap();
            let before = snapshot(&fixture.memory.engine.device_memory[1]);
            let calls = fixture.calls();
            let original_source = (source.byte_len(), source.content());
            assert!(matches!(
                fixture
                    .memory
                    .engine
                    .initialize_public_device_memory(lease, source),
                Err(MemorySessionError::DeviceContentMismatch)
            ));
            assert_eq!(fixture.calls(), calls);
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Quarantined
            );
            assert_eq!(snapshot(&fixture.memory.engine.device_memory[1]), before);
            let root = fixture
                .memory
                .engine
                .terminal_device_initialization
                .as_ref()
                .unwrap();
            assert!(root.input_admitted && !root.native_started && root.failed);
            let init::InitializationLeaseV1::Unmapped(lease) = &root.lease else {
                panic!("lost original lease");
            };
            assert_eq!(
                (
                    lease.id,
                    lease.generation,
                    lease.device,
                    lease.vm,
                    lease.layout
                ),
                before.identity
            );
            let Some(init::InitializationSourceV1::Validated(source)) = &root.source else {
                panic!("lost original source");
            };
            assert_eq!(source.bytes(), input.expected);
            assert_eq!(Some(source.bytes().as_ptr() as usize), input.pointer);
            assert_eq!((source.byte_len(), source.content()), original_source);
            fixture.assert_usage(8192, 2, 0);
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_initializer_in_place_core_keeps_external_custody_until_extraction() {
    for configured in [false, true] {
        for outcome in ["success", "error", "panic"] {
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            let mut root = init::DeviceInitializationCustodyV1::new(
                init::InitializationSourceV1::Unvalidated(
                    input.bytes.take().unwrap(),
                    input.content,
                ),
                init::InitializationLeaseV1::None,
            );
            if outcome == "error" {
                fixture.memory.engine.backend.fail_operation = Some("map_gpu");
            }
            if outcome == "panic" {
                fixture.memory.engine.backend.panic_operation = Some("map_gpu");
            }
            let request = init::AllocationRequestV1 {
                device: fixture.memory.device.model_key(),
                vm: fixture.memory.vm,
                alignment: 4096,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                root.prepare_in_place(&mut fixture.memory.engine, Some(request))
            }));
            if outcome == "panic" {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "map_gpu"))
                );
            } else {
                assert_eq!(result.unwrap().is_err(), outcome == "error");
            }
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            input.assert_source(&root);
            if outcome != "success" {
                assert_eq!(
                    fixture.memory.engine.phase(),
                    SharedMemorySessionPhaseV1::Quarantined
                );
                let before = root_snapshot(&root);
                assert!(root.take_complete().is_err());
                let calls = fixture.calls();
                assert!(
                    root.prepare_in_place(&mut fixture.memory.engine, Some(request))
                        .is_err()
                );
                assert_eq!(fixture.calls(), calls);
                assert!(root.take_complete().is_err());
                assert_eq!(root_snapshot(&root), before);
                assert!(matches!(
                    root.lease,
                    init::InitializationLeaseV1::Unmapped(_)
                ));
            } else {
                let output = root.take_complete().unwrap();
                assert_eq!(output.content(), input.content);
                assert!(root.take_complete().is_err());
            }
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_initializer_capacity_rejection_keeps_source_host_only_and_allows_later_retry() {
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
        let calls = fixture.calls();
        let usage = fixture.memory.usage();
        for repeated in [false, true] {
            assert!(Input::new(repeated, 17).run(&mut fixture, 4096).is_err());
            assert_eq!(fixture.calls(), calls);
            assert_eq!(fixture.memory.usage(), usage);
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
        }
        fixture
            .memory
            .engine
            .release_device_memory(extras.pop().unwrap())
            .unwrap();
        let _initialized = Input::new(false, 17).run(&mut fixture, 4096).unwrap();
        assert_eq!(
            snapshot(&fixture.memory.engine.device_memory[0]),
            fixture.anchor_native
        );
        assert_eq!(fixture.memory.engine.backend.free_calls, 1);
        assert_eq!(fixture.memory.engine.backend.release_va_calls, 1);
    }
}

#[test]
fn device_initializer_malformed_allocations_retain_source_without_fabricated_lease() {
    for configured in [false, true] {
        for repeated in [false, true] {
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
                let mut input = Input::new(repeated, 4097);
                let backend = &mut fixture.memory.engine.backend;
                match fault {
                    "oom" => backend.alloc_oom = true,
                    "va" => backend.allocation_output_mutator = Some(|args| args.va_addr += 4096),
                    "size" => backend.allocation_output_mutator = Some(|args| args.size += 4096),
                    "gpu" => backend.allocation_output_mutator = Some(|args| args.gpu_id += 1),
                    "flags" => backend.corrupt_flags = true,
                    "handle" => backend.allocation_output_mutator = Some(|args| args.handle = 0),
                    "offset" => {
                        backend.allocation_output_mutator = Some(|args| args.mmap_offset = 0)
                    }
                    "alignment" => {
                        backend.allocation_output_mutator = Some(|args| args.mmap_offset += 1)
                    }
                    "collision" => backend.allocation_output_mutator = Some(|args| args.handle = 1),
                    "overlap" => backend.fixed_va = Some(fixture.anchor_native.gpu_va),
                    _ => unreachable!(),
                }
                assert!(input.run(&mut fixture, 4096).is_err());
                fixture.assert_terminal(&input, Stage::Allocate, false);
                let engine = &fixture.memory.engine;
                let record = &engine.device_memory[1];
                assert_eq!(record.phase, DeviceMemoryPhaseV1::Ambiguous);
                assert!(record.mapping.is_none());
                if fault == "overlap" {
                    assert!(record.handle.is_none());
                    assert_eq!(record.mmap_offset, 0);
                } else {
                    let output = engine.backend.last_allocation_output.unwrap();
                    assert_eq!(record.handle, (output.handle != 0).then_some(output.handle));
                    assert_eq!(record.mmap_offset, output.mmap_offset);
                }
                assert_eq!(engine.backend.map_cpu_calls, 0);
                assert_eq!(engine.backend.map_gpu_calls, 1);
                fixture.assert_usage(12_288, 2, 0);
                fixture.no_retry(&input);
            }
        }
    }
}

#[test]
fn device_initializer_lower_entry_rejects_foreign_coordinates_and_account_before_effects() {
    for configured in [false, true] {
        for coordinate in ["id", "generation", "device", "vm", "layout", "account"] {
            if coordinate == "account" && !configured {
                continue;
            }
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            let source =
                validate_initialization_source(input.bytes.take().unwrap(), input.content).unwrap();
            let mut lease = fixture
                .memory
                .engine
                .allocate_device_memory_with_flags(
                    fixture.memory.device.model_key(),
                    fixture.memory.vm,
                    17,
                    4096,
                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap();
            let original_account = if coordinate == "account" {
                let engine = &mut fixture.memory.engine;
                let replacement = DeviceBackingAccountV1::new(
                    engine.session_id,
                    fixture.memory.device.model_key(),
                    fixture.memory.vm,
                    Gfx942DeviceBackingBudgetV1::new(32_768, 8).unwrap(),
                )
                .unwrap();
                engine.device_backing_account.replace(replacement)
            } else {
                None
            };
            match coordinate {
                "id" => lease.id += 1,
                "generation" => lease.generation += 1,
                "device" => lease.device.generation.0 += 1,
                "vm" => lease.vm.id.0 += 1,
                "layout" => lease.layout.requested_bytes += 1,
                _ => {}
            }
            let records: Vec<_> = fixture
                .memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect();
            let calls = fixture.calls();
            assert!(matches!(
                fixture
                    .memory
                    .engine
                    .initialize_public_device_memory(lease, source),
                Err(MemorySessionError::InvalidDeviceMemoryAuthority)
            ));
            assert_eq!(fixture.calls(), calls);
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
            if let Some(account) = original_account {
                fixture.memory.engine.device_backing_account = Some(account);
            }
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_initializer_terminal_storage_is_preallocated_and_cannot_overwrite_custody() {
    assert!(
        std::mem::size_of::<init::TerminalInitializationSlotV1>()
            <= 3 * std::mem::size_of::<usize>()
    );
    for configured in [false, true] {
        for repeated in [false, true] {
            let mut fixture = Fixture::new(configured);
            assert!(fixture.terminal_storage.1 >= 1);
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            let mut input = Input::new(repeated, 4097);
            fixture.memory.engine.backend.fail_operation = Some("map_cpu");
            assert!(matches!(
                input.run(&mut fixture, 4096),
                Err(MemorySessionError::Injected("map_cpu"))
            ));
            fixture.assert_terminal(&input, Stage::CpuMap, true);
            let original = root_snapshot(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .as_ref()
                    .unwrap(),
            );
            let replacement = init::DeviceInitializationCustodyV1::new(
                init::InitializationSourceV1::Repeated(repeated_content(17, 0x33)),
                init::InitializationLeaseV1::None,
            );
            let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .retain(replacement);
            }));
            assert_eq!(
                rejected.unwrap_err().downcast_ref::<&str>(),
                Some(&"occupied initialization custody")
            );
            assert_eq!(
                root_snapshot(
                    fixture
                        .memory
                        .engine
                        .terminal_device_initialization
                        .as_ref()
                        .unwrap()
                ),
                original
            );
            fixture.no_retry(&input);
        }
    }
}
