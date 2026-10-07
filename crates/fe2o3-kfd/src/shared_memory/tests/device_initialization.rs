//! CPU/shared-sequence tests; FakeBackend does not qualify Linux or GPU behavior.
use super::*;
use crate::shared_memory::device_initialization::{
    self as init, DeviceInitializationStageV1 as Stage,
};

#[path = "device_allocation.rs"]
pub(in crate::shared_memory) mod allocation_cases;

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
        Self::with_budget(configured.then(|| Gfx942DeviceBackingBudgetV1::new(32_768, 8).unwrap()))
    }

    fn with_budget(budget: Option<Gfx942DeviceBackingBudgetV1>) -> Self {
        let mut memory = BackingConstructorFixture::new(budget);
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

#[path = "device_initialization/initialization_tests.rs"]
mod initialization_tests;
