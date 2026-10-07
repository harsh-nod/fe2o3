use super::*;
use crate::shared_memory::{
    CleanupStageV1 as Stage, ControlCleanupObservationV1, Gfx942DeviceMemoryDispatchFactsV1,
    PreparationMemoryFixtureV1 as Memory, SharedGttAllocationLayoutV1,
    SharedGttMappedResourceFactsV1,
};

type Root = ReturningControlCleanupCustodyV1;
type Mode = ReturningControlModeV1;

#[path = "detached_tests.rs"]
mod detached;

#[path = "persistent_tests.rs"]
mod persistent;

#[path = "ordinary_tests.rs"]
mod ordinary;

#[derive(Debug, Eq, PartialEq)]
struct HostFacts {
    gpu_va: u64,
    logical_bytes: usize,
    cpu_mapping_bytes: usize,
    gpu_va_bytes: u64,
    mapping: MemoryMappingKeyV1,
    publication: fe2o3_runtime_model::MemoryPublicationKeyV1,
}

fn host_facts(f: &SharedGttMappedResourceFactsV1) -> HostFacts {
    HostFacts {
        gpu_va: f.gpu_va(),
        logical_bytes: f.logical_bytes(),
        cpu_mapping_bytes: f.cpu_mapping_bytes(),
        gpu_va_bytes: f.gpu_va_bytes(),
        mapping: f.mapping(),
        publication: f.publication(),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Control {
    identity: SharedGttAllocationIdentityV1,
    layout: SharedGttAllocationLayoutV1,
    facts: HostFacts,
}

#[derive(Debug, Eq, PartialEq)]
enum DataFacts {
    Device(Gfx942DeviceMemoryDispatchFactsV1),
    Host(HostFacts),
}

#[derive(Debug, Eq, PartialEq)]
struct Data {
    identity: Gfx942SdmaBufferStorageIdentityV1,
    facts: DataFacts,
}

#[derive(Debug, Eq, PartialEq)]
struct PersistentData {
    identity: Gfx942SdmaBufferStorageIdentityV1,
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content: Option<Gfx942DeviceContentDescriptorV1>,
}

fn persistent_data(data: &[Gfx942FixedDispatchDataV1]) -> Vec<PersistentData> {
    data.iter()
        .map(|data| PersistentData {
            identity: data.sdma_storage_identity(),
            layout: data.layout(),
            initialized: data.is_fully_initialized(),
            content: data.initialized_content(),
        })
        .collect()
}

fn data(a: &DispatchDataAuthorityV1) -> Data {
    Data {
        identity: Memory::data_storage(a),
        facts: match a {
            DispatchDataAuthorityV1::Device(a) => DataFacts::Device(*a.facts()),
            DispatchDataAuthorityV1::HostVisible(a) => DataFacts::Host(host_facts(a.facts())),
        },
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Premise {
    value: RetainedDataPremiseV1,
    storage: [(usize, usize); 2],
}

fn premise(p: &RetainedDataPremiseV1) -> Premise {
    Premise {
        value: RetainedDataPremiseV1 {
            layout: p.layout,
            role_identity: p.role_identity,
            valid_bytes: p.valid_bytes,
            effect: p.effect,
            initialized_content: p.initialized_content,
            fully_initialized: p.fully_initialized,
            writable_ranges: p.writable_ranges.clone(),
            completed_snapshots: p.completed_snapshots.clone(),
        },
        storage: [
            (p.writable_ranges.as_ptr() as usize, p.writable_ranges.len()),
            (
                p.completed_snapshots.as_ptr() as usize,
                p.completed_snapshots.len(),
            ),
        ],
    }
}

fn storage<T>(v: &Vec<T>) -> (usize, usize) {
    (v.as_ptr() as usize, v.capacity())
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::queue) struct Snapshot {
    mode: Mode,
    kernarg: Option<Control>,
    code: Vec<Control>,
    code_pointer: usize,
    code_identity: Vec<ResolvedCodeIdentityV1>,
    packets: Vec<PreparedDispatchPacketV1>,
    data: Vec<Data>,
    premises: Vec<Premise>,
    generation: DispatchGenerationOwnerV1,
    slots_pointer: usize,
    persistent: PersistentFixedDispatchControlStateV1,
    active: Option<ControlCleanupObservationV1>,
    returned: Vec<(Data, Premise)>,
    returned_generation: Option<u64>,
    persistent_returned: Vec<PersistentData>,
    persistent_output: PersistentOutputStateV1,
    storage: [(usize, usize); 6],
    started: bool,
    complete: bool,
}

fn snapshot(r: &Root) -> Snapshot {
    Snapshot {
        mode: r.mode,
        kernarg: r.kernarg.as_ref().map(|a| Control {
            identity: Memory::kernarg_identity(a),
            layout: a.layout(),
            facts: host_facts(a.facts()),
        }),
        code: r
            .code
            .as_slice()
            .iter()
            .map(|a| Control {
                identity: Memory::code_identity(a),
                layout: a.layout(),
                facts: host_facts(a.facts()),
            })
            .collect(),
        code_pointer: r.code.as_slice().as_ptr() as usize,
        code_identity: r.code_identity.clone(),
        packets: r.packets.clone(),
        data: r.data.iter().map(data).collect(),
        premises: r.data_premises.iter().map(premise).collect(),
        generation: r.generation.clone(),
        slots_pointer: r.generation.slots.as_ptr() as usize,
        persistent: r.persistent_control,
        active: r
            .active_control
            .as_ref()
            .map(ControlCleanupCustodyV1::observation),
        returned: r
            .returned
            .iter()
            .map(|d| (data(&d.authority), premise(&d.premise)))
            .collect(),
        returned_generation: r.returned_generation,
        persistent_returned: persistent_data(&r.persistent_returned),
        persistent_output: r.persistent_output,
        storage: [
            storage(&r.code_identity),
            storage(&r.packets),
            storage(&r.data),
            storage(&r.data_premises),
            storage(&r.returned),
            storage(&r.persistent_returned),
        ],
        started: r.started,
        complete: r.complete,
    }
}

fn order(s: &Snapshot) -> Vec<SharedGttAllocationIdentityV1> {
    std::iter::once(s.kernarg.as_ref().unwrap().identity)
        .chain(s.code.iter().map(|a| a.identity))
        .collect()
}

fn assert_inputs(r: &Root, before: &Snapshot) {
    let after = snapshot(r);
    assert_eq!(after.mode, before.mode);
    assert_eq!(after.code_identity, before.code_identity);
    assert_eq!(after.packets, before.packets);
    assert_eq!(after.data, before.data);
    assert_eq!(after.premises, before.premises);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.slots_pointer, before.slots_pointer);
    assert_eq!(after.persistent, before.persistent);
    assert_eq!(after.storage[..4], before.storage[..4]);
    assert!(after.returned.is_empty());
    assert!(!after.complete);
}

fn owner_snapshot(owner: &DispatchResourceOwnerV1, mode: Mode) -> Snapshot {
    Snapshot {
        mode,
        kernarg: Some(Control {
            identity: Memory::kernarg_identity(&owner.kernarg),
            layout: owner.kernarg.layout(),
            facts: host_facts(owner.kernarg.facts()),
        }),
        code: owner
            .code
            .iter()
            .map(|a| Control {
                identity: Memory::code_identity(a),
                layout: a.layout(),
                facts: host_facts(a.facts()),
            })
            .collect(),
        code_pointer: owner.code.as_ptr() as usize,
        code_identity: owner.code_identity.clone(),
        packets: owner.packets.clone(),
        data: owner.data.iter().map(data).collect(),
        premises: owner.data_premises.iter().map(premise).collect(),
        generation: owner.generation.clone(),
        slots_pointer: owner.generation.slots.as_ptr() as usize,
        persistent: owner.persistent_control,
        active: None,
        returned: Vec::new(),
        returned_generation: None,
        persistent_returned: Vec::new(),
        persistent_output: PersistentOutputStateV1::Unprepared,
        storage: [
            storage(&owner.code_identity),
            storage(&owner.packets),
            storage(&owner.data),
            storage(&owner.data_premises),
            storage(&Vec::<ReturnedDispatchDataLeaseV1>::new()),
            storage(&Vec::<Gfx942FixedDispatchDataV1>::new()),
        ],
        started: false,
        complete: false,
    }
}

fn new_root(owner: DispatchResourceOwnerV1, mode: Mode) -> Root {
    let before = owner_snapshot(&owner, mode);
    let root = Root::new(owner, mode);
    assert_eq!(
        snapshot(&root),
        before,
        "constructor changed original owner"
    );
    root
}

impl Snapshot {
    pub(in crate::queue) fn ordinary_owner_v1(owner: &DispatchResourceOwnerV1) -> Self {
        owner_snapshot(owner, Mode::Ordinary)
    }
    pub(in crate::queue) fn assert_ordinary_control_prefix_v1(
        &self,
        root: &Root,
        completed: usize,
    ) {
        assert_eq!(self.mode, Mode::Ordinary);
        self.assert_detached_prefix_v1(root, completed, true, false);
    }

    pub(in crate::queue) fn assert_restored_v1(&self, mut after: Self, poisoned: bool) {
        assert_eq!(after.generation.poisoned, poisoned);
        after.generation.poisoned = self.generation.poisoned;
        assert_eq!(&after, self, "restore the entire original control owner");
    }

    pub(in crate::queue) fn owner_v1(owner: &DispatchResourceOwnerV1, generation: u64) -> Self {
        owner_snapshot(
            owner,
            Mode::DetachedPersistent {
                expected_generation: generation,
            },
        )
    }

    pub(in crate::queue) fn root_v1(root: &Root) -> Self {
        snapshot(root)
    }

    pub(in crate::queue) fn recycled_owner_v1(owner: &DispatchResourceOwnerV1) -> Self {
        owner_snapshot(owner, Mode::AfterRecycle)
    }

    pub(in crate::queue) fn persistent_cancel_owner_v1(owner: &DispatchResourceOwnerV1) -> Self {
        owner_snapshot(owner, Mode::PersistentBeforePublication)
    }

    pub(in crate::queue) fn assert_persistent_cancel_prefix_v1(
        &self,
        root: &Root,
        completed: usize,
        step: usize,
        panicked: bool,
        returned: &[Gfx942FixedDispatchDataV1],
    ) {
        let after = snapshot(root);
        assert_eq!(self.mode, Mode::PersistentBeforePublication);
        persistent::assert_control(
            root,
            self,
            completed,
            if step == 0 { "Mapped" } else { "Unmapped" },
        );
        assert_eq!(after.mode, self.mode);
        assert_eq!(after.code_identity, self.code_identity);
        assert_eq!(after.packets, self.packets);
        assert_eq!(after.generation, self.generation);
        assert_eq!(after.slots_pointer, self.slots_pointer);
        assert_eq!(after.persistent, self.persistent);
        assert_eq!(after.premises, self.premises);
        assert_eq!(after.storage[..5], self.storage[..5]);
        assert!(after.started && !after.complete);
        assert!(after.kernarg.is_none());
        assert_eq!(after.code, self.code[completed..]);
        assert_eq!(
            after.code_pointer,
            self.code_pointer + completed * size_of::<CodeAuthority>()
        );
        assert_eq!(
            after.active.as_ref().unwrap().identity,
            self.order_v1()[completed]
        );
        assert!(after.returned.is_empty() && after.persistent_returned.is_empty());
        assert_eq!(after.returned_generation, None);
        if panicked {
            assert!(root.persistent_returned.capacity() >= self.data.len());
            assert_eq!(after.data, self.data);
            assert!(returned.is_empty());
            assert_eq!(
                after.persistent_output,
                PersistentOutputStateV1::Prepared(
                    self.generation
                        .persistent_cancellation_generation()
                        .unwrap()
                )
            );
        } else {
            persistent::assert_output(returned, self);
            assert_eq!(root.persistent_returned.capacity(), 0);
            assert!(after.data.is_empty());
            assert_eq!(after.persistent_output, PersistentOutputStateV1::Taken);
            assert_eq!(returned.len(), self.data.len());
            for ((returned, data), premise) in returned.iter().zip(&self.data).zip(&self.premises) {
                assert_eq!(returned.sdma_storage_identity(), data.identity);
                assert_eq!(returned.layout(), premise.value.layout);
                assert_eq!(
                    returned.is_fully_initialized(),
                    premise.value.fully_initialized
                );
            }
        }
    }

    pub(in crate::queue) fn assert_recycled_prefix_v1(
        &self,
        root: &Root,
        completed: usize,
        active: bool,
        complete: bool,
    ) {
        let after = snapshot(root);
        let touched = completed + usize::from(active);
        let code_touched = touched.saturating_sub(1);
        assert_eq!(self.mode, Mode::AfterRecycle);
        assert_eq!(after.mode, self.mode);
        assert_eq!(after.code_identity, self.code_identity);
        assert_eq!(after.packets, self.packets);
        assert_eq!(after.generation, self.generation);
        assert_eq!(after.slots_pointer, self.slots_pointer);
        assert_eq!(after.persistent, self.persistent);
        assert_eq!(after.storage[..4], self.storage[..4]);
        assert_eq!(after.storage[5], self.storage[5]);
        assert_eq!(
            after.returned_generation,
            self.generation.recycled_generation
        );
        assert_eq!(after.persistent_returned, self.persistent_returned);
        assert_eq!(after.persistent_output, self.persistent_output);
        if complete {
            assert!(after.data.is_empty() && after.premises.is_empty());
            assert_eq!(after.returned.len(), self.data.len());
            for ((data, premise), (expected_data, expected_premise)) in after
                .returned
                .iter()
                .zip(self.data.iter().zip(&self.premises))
            {
                assert_eq!(data, expected_data);
                assert_eq!(premise, expected_premise);
            }
        } else {
            assert_eq!(after.data, self.data);
            assert_eq!(after.premises, self.premises);
            assert!(after.returned.is_empty());
        }
        assert!(after.started);
        assert_eq!(after.complete, complete);
        assert_eq!(
            after.kernarg.as_ref(),
            (touched == 0).then_some(self.kernarg.as_ref()).flatten()
        );
        assert_eq!(after.code, self.code[code_touched..]);
        assert_eq!(
            after.code_pointer,
            self.code_pointer + code_touched * size_of::<CodeAuthority>()
        );
        assert_eq!(
            after.active.as_ref().map(|a| a.identity),
            active.then(|| self.order_v1()[completed])
        );
    }

    pub(in crate::queue) fn order_v1(&self) -> Vec<SharedGttAllocationIdentityV1> {
        order(self)
    }

    pub(in crate::queue) fn active_v1(&self) -> Option<&ControlCleanupObservationV1> {
        self.active.as_ref()
    }

    pub(in crate::queue) fn assert_detached_prefix_v1(
        &self,
        root: &Root,
        completed: usize,
        active: bool,
        complete: bool,
    ) {
        let after = snapshot(root);
        let touched = completed + usize::from(active);
        let code_touched = touched.saturating_sub(1);
        assert_eq!(after.mode, self.mode);
        assert_eq!(after.code_identity, self.code_identity);
        assert_eq!(after.packets, self.packets);
        assert_eq!(after.data, self.data);
        assert_eq!(after.premises, self.premises);
        assert_eq!(after.generation, self.generation);
        assert_eq!(after.slots_pointer, self.slots_pointer);
        assert_eq!(after.persistent, self.persistent);
        assert_eq!(after.storage, self.storage);
        assert_eq!(after.returned, self.returned);
        assert_eq!(after.returned_generation, None);
        assert_eq!(after.persistent_returned, self.persistent_returned);
        assert_eq!(after.persistent_output, self.persistent_output);
        assert!(after.started);
        assert_eq!(after.complete, complete);
        assert_eq!(
            after.kernarg.as_ref(),
            (touched == 0).then_some(self.kernarg.as_ref()).flatten()
        );
        assert_eq!(after.code, self.code[code_touched..]);
        assert_eq!(
            after.code_pointer,
            self.code_pointer + code_touched * size_of::<CodeAuthority>()
        );
        assert_eq!(
            after.active.as_ref().map(|a| a.identity),
            active.then(|| self.order_v1()[completed])
        );
    }
}

struct NoEntry;
impl PristineControlReleaseV1 for NoEntry {
    fn release_control(
        &mut self,
        _: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        panic!("returning retry entered cleanup callback")
    }
}

fn reject_retry(r: &mut Root) {
    let before = snapshot(r);
    assert!(matches!(
        r.release_in_place(&mut NoEntry),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(snapshot(r), before);
    assert!(matches!(
        r.take_completed(),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(snapshot(r), before);
}

// Synthetic completion observations drive the real generation transitions.
fn publish(
    g: &mut DispatchGenerationOwnerV1,
) -> (DispatchEpochIdentityV1, CompletionBatchOccurrenceV1) {
    let next = g.next_generation;
    let epoch = g
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(next))
        .unwrap();
    let completion = test_completion_occurrence_v1(next);
    g.mark_published(epoch, completion).unwrap();
    (epoch, completion)
}

fn recycle(g: &mut DispatchGenerationOwnerV1) -> u64 {
    let (epoch, completion) = publish(g);
    g.complete_epoch(epoch, completion).unwrap();
    g.recycle_epoch(epoch, completion).unwrap();
    epoch.dispatch_generation
}

fn fixture(mode: Mode) -> (crate::shared_memory::PristineAbortMemoryFixtureV1, Root) {
    let (memory, mut owner) = pristine_abort::pristine_dispatch_fixture_v1(8);
    if matches!(mode, Mode::AfterRecycle | Mode::PersistentAfterRecycle) {
        assert_eq!(recycle(&mut owner.generation), 8);
    }
    (memory, new_root(owner, mode))
}

fn assert_interrupted(r: &mut Root, before: &Snapshot, index: usize, owner: &str) {
    assert_inputs(r, before);
    let after = snapshot(r);
    let active = after.active.as_ref().expect("failed control stays rooted");
    assert_eq!(active.identity, order(before)[index]);
    let original = if index == 0 {
        before.kernarg.as_ref().unwrap()
    } else {
        &before.code[index - 1]
    };
    assert_eq!(active.layout, original.layout);
    assert_eq!(
        active.profile_type,
        if index == 0 {
            std::any::TypeId::of::<KernargGttV1>()
        } else {
            std::any::TypeId::of::<ExecutableGttV1>()
        }
    );
    assert_eq!(
        active.state_type,
        match (index == 0, owner == "Mapped") {
            (true, true) => std::any::TypeId::of::<GttGpuAccessibleMutableV1>(),
            (false, true) => std::any::TypeId::of::<GttGpuAccessibleExecutableV1>(),
            (true, false) => std::any::TypeId::of::<crate::shared_memory::GttCpuWritableV1>(),
            (false, false) =>
                std::any::TypeId::of::<crate::shared_memory::GttExecutableImmutableV1>(),
        }
    );
    if matches!(before.mode, Mode::DetachedPersistent { .. }) {
        assert_eq!(r.returned.capacity(), 0);
        assert_eq!(r.returned_generation, None);
    } else {
        assert!(
            r.returned.capacity() >= before.data.len(),
            "return capacity precedes disposal"
        );
        assert_eq!(
            r.returned_generation,
            Some(match before.mode {
                Mode::AfterRecycle => before.generation.returned_generation().unwrap(),
                Mode::ReturningDestroy => before.generation.returning_destroy_generation().unwrap(),
                Mode::Ordinary
                | Mode::DetachedPersistent { .. }
                | Mode::PersistentBeforePublication
                | Mode::PersistentAfterRecycle => unreachable!(),
            })
        );
    }
    assert_eq!(active.owner, owner);
    assert_eq!(after.kernarg, None);
    assert_eq!(after.code, before.code[index..]);
    assert_eq!(
        after.code_pointer,
        before.code_pointer + index * core::mem::size_of::<CodeAuthority>()
    );
    assert!(after.started && !after.complete);
    reject_retry(r);
}

#[path = "tests/custody_tests.rs"]
mod custody_tests;
