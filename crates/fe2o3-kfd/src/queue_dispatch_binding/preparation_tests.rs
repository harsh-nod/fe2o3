//! Real planner/loader and production sequencer; native calls remain CPU fixtures.

use super::*;
use crate::shared_memory::{
    Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1, PreparationMemoryCallV1 as Call,
    PreparationMemoryFixtureV1 as Memory, PreparationMemoryObservationV1,
    PreparationNativeFaultV1 as NativeFault, SharedGttAllocationLayoutV1,
    SharedMemorySessionPhaseV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "template_binding_tests.rs"]
mod template_binding_tests;

#[path = "conditional_fill_tests.rs"]
mod conditional_fill_tests;

#[path = "target_profile_tests.rs"]
mod target_profile_tests;

pub(in crate::queue) use target_profile_tests::{
    synthetic_gfx950_image_v1, synthetic_gfx950_program_v1,
};

pub(in crate::queue) fn control_release_fixture_v1() -> (Memory, DispatchResourceOwnerV1) {
    let mut memory = Memory::new(true);
    let data = memory.roster();
    let mut custody = FixedDispatchPreparationCustodyV1::new([packet(2)], data);
    run(&mut custody, &mut memory).unwrap();
    (memory, custody.take_completed().unwrap())
}

/// Simulated process teardown for CPU fixture storage, never a recovery transition.
pub(in crate::queue) fn teardown_completed_restore_fixture_v1(
    owner: DispatchResourceOwnerV1,
    memory: &mut Memory,
) {
    use crate::queue::dispatch_binding::pristine_abort::PristineControlReleaseV1;
    use crate::shared_memory::ControlCleanupCustodyV1;

    assert!(owner.data.is_empty());
    assert!(matches!(
        owner.persistent_control,
        PersistentFixedDispatchControlStateV1::DataDetached(_)
    ));
    let mut cleanup = ControlCleanupCustodyV1::kernarg(owner.kernarg.into_token());
    memory.release_control(&mut cleanup).unwrap();
    assert!(cleanup.is_complete());
    for code in owner.code {
        let mut cleanup = ControlCleanupCustodyV1::code(code.into_token());
        memory.release_control(&mut cleanup).unwrap();
        assert!(cleanup.is_complete());
    }
}

pub(in crate::queue) type RecycledDataExpectationV1 = (
    Gfx942SdmaBufferStorageIdentityV1,
    Gfx942FixedDispatchDataLayoutV1,
    bool,
);

pub(in crate::queue) fn ordinary_recycled_in_memory_v1(
    memory: &mut Memory,
    queue: QueueKeyV1,
    bindings: usize,
) -> (DispatchResourceOwnerV1, u64, Vec<RecycledDataExpectationV1>) {
    ordinary_recycled_with_capacity_in_memory_v1(
        memory,
        queue,
        bindings,
        &Gfx942FixedDispatchCapacityV1::default(),
    )
}

pub(in crate::queue) fn ordinary_recycled_with_capacity_in_memory_v1(
    memory: &mut Memory,
    queue: QueueKeyV1,
    bindings: usize,
    capacity: &Gfx942FixedDispatchCapacityV1,
) -> (DispatchResourceOwnerV1, u64, Vec<RecycledDataExpectationV1>) {
    let (programs, packets, mut data) = match bindings {
        1 => (programs(), [packet(2)], memory.roster()),
        3 => {
            let (program, packets, mut data) = three_binding_inputs(memory);
            data.extend([memory.host(true), memory.device(false), memory.host(false)]);
            (vec![program], packets, data)
        }
        _ => panic!("ordinary fixture binding count"),
    };
    let DispatchDataStorageV1::Uninitialized(lease) = memory.device(false).storage else {
        unreachable!();
    };
    // Synthetic completed typing exercises representation recovery, not GPU initialization.
    data.push(Gfx942FixedDispatchDataV1::initialized_storage(lease));
    let expected = data
        .iter()
        .map(|data| {
            (
                data.sdma_storage_identity(),
                data.layout(),
                data.is_fully_initialized(),
            )
        })
        .collect();
    let mut custody = FixedDispatchPreparationCustodyV1::new(packets, data);
    let mut prepared = capacity.preallocate_fresh_v1::<1>().unwrap();
    prepare_public_fixed_dispatch_resources_with_capacity_in_place(
        memory,
        &programs,
        &mut custody,
        capacity,
        &mut prepared,
    )
    .unwrap();
    let mut owner = custody.take_completed().unwrap();
    assert!(matches!(
        owner.persistent_control,
        PersistentFixedDispatchControlStateV1::Ordinary
    ));
    let generation = owner.generation.next_generation;
    // These are real logical transitions with a fixture occurrence, not GPU completion evidence.
    let mut completion = test_completion_occurrence_v1(generation);
    completion.queue = queue;
    completion.dispatch_roster.queue = queue;
    completion.signal_mapping.allocation.vm = queue.vm;
    let epoch = owner
        .generation
        .reserve(queue, completion.dispatch_roster)
        .unwrap();
    owner.generation.mark_published(epoch, completion).unwrap();
    owner.generation.complete_epoch(epoch, completion).unwrap();
    owner.generation.recycle_epoch(epoch, completion).unwrap();
    (owner, generation, expected)
}

pub(in crate::queue) fn single_persistent_control_fixture_v1() -> (Memory, DispatchResourceOwnerV1)
{
    let mut memory = Memory::new(true);
    let owner = single_persistent_control_in_memory_v1(
        &mut memory,
        super::super::tests::persistent_control_test_queue(41),
    );
    (memory, owner)
}

pub(in crate::queue) fn single_persistent_control_in_memory_v1(
    memory: &mut Memory,
    queue: QueueKeyV1,
) -> DispatchResourceOwnerV1 {
    let data = memory.device(true);
    let programs = programs();
    let packets = [packet(0)];
    let identity = persistent_fixed_dispatch_control_identity_v1(
        queue,
        &programs,
        &packets,
        data.layout(),
        true,
        Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap(),
        data.sdma_storage_identity(),
    )
    .unwrap();
    let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, vec![data]);
    prepare_persistent_fixed_dispatch_resources_v1(
        memory,
        &programs,
        &mut preparation,
        Some(7),
        identity,
    )
    .unwrap();
    preparation.take_completed().unwrap()
}

pub(in crate::queue) fn three_persistent_control_fixture_v1() -> (Memory, DispatchResourceOwnerV1) {
    let mut memory = Memory::new(true);
    let owner = three_persistent_control_in_memory_v1(
        &mut memory,
        super::super::tests::persistent_control_test_queue(42),
    );
    (memory, owner)
}

pub(in crate::queue) fn three_persistent_control_in_memory_v1(
    memory: &mut Memory,
    queue: QueueKeyV1,
) -> DispatchResourceOwnerV1 {
    let (program, packets, data) = three_binding_inputs(memory);
    let identity = three_binding_persistent_fixed_dispatch_control_identity_v1(
        queue,
        core::slice::from_ref(&program),
        &packets,
        core::array::from_fn(|i| data[i].layout()),
        [true; 3],
        core::array::from_fn(|i| Gfx942DeviceContentRoleV1::new([0x62; 32], i as u32).unwrap()),
        core::array::from_fn(|i| data[i].sdma_storage_identity()),
    )
    .unwrap();
    let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, data);
    prepare_three_binding_persistent_fixed_dispatch_resources_v1(
        memory,
        &[program],
        &mut preparation,
        Some(7),
        identity,
    )
    .unwrap();
    preparation.take_completed().unwrap()
}

fn three_binding_inputs(
    memory: &mut Memory,
) -> (
    ValidatedKernelEnvelope<'static>,
    [Gfx942FixedDispatchPacketV1; 1],
    Vec<Gfx942FixedDispatchDataV1>,
) {
    let (program, packets) = three_binding_recipe();
    let data = vec![
        memory.device(true),
        memory.device(true),
        memory.device(true),
    ];
    (program, packets, data)
}

fn three_binding_recipe() -> (
    ValidatedKernelEnvelope<'static>,
    [Gfx942FixedDispatchPacketV1; 1],
) {
    use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
    let image =
        include_bytes!("../../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    let program = validate(image, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .reconcile_dispatch_abi(
            [0x71; 32],
            &[
                KernelGlobalBufferAbiV1::new(0, "arg0.data", 0, 4, ArgumentAccess::ReadOnly),
                KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 4, ArgumentAccess::ReadOnly),
                KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 4, ArgumentAccess::WriteOnly),
            ],
        )
        .unwrap();
    let mut bytes = [0; 48];
    for offset in [8, 24, 40] {
        bytes[offset..offset + 8].copy_from_slice(&1024_u64.to_le_bytes());
    }
    let packets = [Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([1024, 1, 1], [256, 1, 1]).unwrap(),
        0,
        bytes.into(),
        vec![
            Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096),
            Gfx942DispatchBufferBindingV1::new(2, 1, 0, 4096),
            Gfx942DispatchBufferBindingV1::new(4, 2, 0, 4096),
        ]
        .into_boxed_slice(),
    )];
    (program, packets)
}

pub(in crate::queue) fn persistent_cancel_control_in_memory_v1(
    memory: &mut Memory,
    queue: QueueKeyV1,
    data: Vec<Gfx942FixedDispatchDataV1>,
    predecessor: Option<u64>,
) -> DispatchResourceOwnerV1 {
    if data.len() == 3 {
        let (program, packets) = three_binding_recipe();
        let identity = three_binding_persistent_fixed_dispatch_control_identity_v1(
            queue,
            core::slice::from_ref(&program),
            &packets,
            core::array::from_fn(|i| data[i].layout()),
            core::array::from_fn(|i| data[i].is_fully_initialized()),
            core::array::from_fn(|i| Gfx942DeviceContentRoleV1::new([0x62; 32], i as u32).unwrap()),
            core::array::from_fn(|i| data[i].sdma_storage_identity()),
        )
        .unwrap();
        let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, data);
        prepare_three_binding_persistent_fixed_dispatch_resources_v1(
            memory,
            &[program],
            &mut preparation,
            predecessor,
            identity,
        )
        .unwrap();
        return preparation.take_completed().unwrap();
    }
    assert_eq!(data.len(), 1);
    use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
    let program = validate(
        include_bytes!("../../../fe2o3-runtime/fixtures/trusted-gfx942-active-checkpoint-v1/active-checkpoint.hsaco"),
        AdmittedProfile::Gfx942XnackOffCov6,
    ).unwrap().bind_kernel("active_checkpoint_liveness").unwrap().reconcile_dispatch_abi(
        [0x61; 32], &[KernelGlobalBufferAbiV1::new(0, "output", 0, 4, ArgumentAccess::WriteOnly)],
    ).unwrap();
    let packets = [Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        vec![0; usize::try_from(program.selected_kernel().kernarg_segment_size()).unwrap()]
            .into_boxed_slice(),
        vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
    )];
    let identity = persistent_fixed_dispatch_control_identity_v1(
        queue,
        core::slice::from_ref(&program),
        &packets,
        data[0].layout(),
        data[0].is_fully_initialized(),
        Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap(),
        data[0].sdma_storage_identity(),
    )
    .unwrap();
    let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, data);
    prepare_persistent_fixed_dispatch_resources_v1(
        memory,
        &[program],
        &mut preparation,
        predecessor,
        identity,
    )
    .unwrap();
    preparation.take_completed().unwrap()
}

pub(in crate::queue) fn recycle_and_detach_persistent_fixture_v1(
    owner: &mut DispatchResourceOwnerV1,
) -> (u64, Vec<Gfx942FixedDispatchDataV1>) {
    let PersistentFixedDispatchControlStateV1::Attached(identity) = owner.persistent_control else {
        panic!("prepared persistent fixture must start attached");
    };
    let generation = owner.generation.next_generation;
    // Exercise real logical transitions; this occurrence is not a GPU observation.
    let mut completion = test_completion_occurrence_v1(generation);
    completion.queue = identity.queue;
    completion.dispatch_roster.queue = identity.queue;
    completion.signal_mapping.allocation.vm = identity.queue.vm;
    let epoch = owner
        .generation
        .reserve(identity.queue, completion.dispatch_roster)
        .unwrap();
    owner.generation.mark_published(epoch, completion).unwrap();
    owner.generation.complete_epoch(epoch, completion).unwrap();
    owner.generation.recycle_epoch(epoch, completion).unwrap();
    let (returned_generation, data) = owner
        .detach_persistent_replay_data_after_recycle_v1()
        .unwrap();
    assert_eq!(returned_generation, generation);
    assert_eq!(data.len(), identity.binding_count());
    (generation, data)
}

const IMAGE: &[u8] = include_bytes!(
    "../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
);

fn programs() -> Vec<ValidatedKernelEnvelope<'static>> {
    (1..=3)
        .map(|index| {
            super::super::tests::actual_persistent_control_test_program(IMAGE, [index; 32])
        })
        .collect()
}

fn packet(program_index: usize) -> Gfx942FixedDispatchPacketV1 {
    let mut bytes = [0; 16];
    bytes[8..].copy_from_slice(&1024_u64.to_le_bytes());
    Gfx942FixedDispatchPacketV1::new(
        program_index,
        AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
        0,
        bytes.into(),
        vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
    )
}

#[derive(Debug, Eq, PartialEq)]
struct Input {
    identity: Gfx942FixedDispatchStorageIdentityV1,
    storage: Gfx942SdmaBufferStorageIdentityV1,
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content: Option<Gfx942DeviceContentDescriptorV1>,
}

fn inputs(data: &[Gfx942FixedDispatchDataV1]) -> Vec<Input> {
    data.iter()
        .map(|d| Input {
            identity: d.storage_identity(),
            storage: d.sdma_storage_identity(),
            layout: d.layout(),
            initialized: d.is_fully_initialized(),
            content: d.initialized_content(),
        })
        .collect()
}

fn assert_inputs<const N: usize, P: PreparationPacketsV1<N>>(
    owner: &FixedDispatchPreparationCustodyV1<N, P>,
    expected: &[Input],
) {
    assert_inputs_with_completed(owner, expected, None, owner.completed.as_ref());
}

struct ExpectedDataEffectV1 {
    effect: Option<DeviceDataEffectV1>,
    writable_ranges: Vec<CompletedWritableRangeV1>,
}

fn ordinary_data_effect<const N: usize>(index: usize) -> ExpectedDataEffectV1 {
    ExpectedDataEffectV1 {
        effect: (index == 0).then_some(DeviceDataEffectV1::ReadWrite),
        writable_ranges: if index == 0 {
            vec![
                CompletedWritableRangeV1 {
                    offset: 0,
                    byte_len: 4096,
                };
                N
            ]
        } else {
            Vec::new()
        },
    }
}

fn assert_inputs_with_completed<const N: usize, P: PreparationPacketsV1<N>>(
    owner: &FixedDispatchPreparationCustodyV1<N, P>,
    expected: &[Input],
    expected_effects: Option<&[ExpectedDataEffectV1]>,
    completed: Option<&DispatchResourceOwnerV1>,
) {
    if let Some(effects) = expected_effects {
        assert_eq!(effects.len(), expected.len());
    }
    if let Some(completed) = completed {
        assert_eq!(completed.data.len(), expected.len());
        assert_eq!(completed.data_premises.len(), expected.len());
        for (index, ((authority, premise), input)) in completed
            .data
            .iter()
            .zip(&completed.data_premises)
            .zip(expected)
            .enumerate()
        {
            assert_eq!(Memory::data_storage(authority), input.storage);
            assert_eq!(premise.layout, input.layout);
            assert_eq!(premise.initialized_content, input.content);
            assert_eq!(premise.fully_initialized, input.initialized);
            assert_eq!(premise.valid_bytes, input.layout.requested_bytes());
            let ordinary = ordinary_data_effect::<N>(index);
            let effect = expected_effects.map_or(&ordinary, |effects| &effects[index]);
            assert_eq!(premise.effect, effect.effect);
            assert_eq!(premise.writable_ranges.as_ref(), effect.writable_ranges);
            assert!(premise.completed_snapshots.is_empty());
        }
    } else if let Some(retained) = &owner.retained_data {
        assert_eq!(retained.len(), expected.len());
        for (retained, input) in retained.iter().zip(expected) {
            assert_eq!(Memory::data_storage(&retained.authority), input.storage);
            assert_eq!(retained.layout, input.layout);
            assert_eq!(retained.initialized_content, input.content);
            assert_eq!(retained.fully_initialized, input.initialized);
        }
        let plan = owner.plan.as_ref().unwrap();
        assert_eq!(plan.data.len(), expected.len());
        for (index, plan) in plan.data.iter().enumerate() {
            assert_eq!(plan.layout, expected[index].layout);
            let ordinary = ordinary_data_effect::<N>(index);
            let effect = expected_effects.map_or(&ordinary, |effects| &effects[index]);
            assert_eq!(plan.effect, effect.effect);
            assert_eq!(plan.writable_ranges.as_ref(), effect.writable_ranges);
            assert!(plan.completed_snapshots.is_empty());
        }
    } else {
        assert_eq!(inputs(&owner.original_data), expected);
    }
}

fn assert_custody<const N: usize, P: PreparationPacketsV1<N>>(
    memory: &Memory,
    owner: &FixedDispatchPreparationCustodyV1<N, P>,
) {
    let mut ids = owner
        .code
        .iter()
        .map(Memory::code_identity)
        .collect::<Vec<_>>();
    let mut markers = Vec::new();
    match &owner.current_code {
        CodeStageV1::Cpu(t) => ids.push(t.storage_identity()),
        CodeStageV1::Immutable(t) => ids.push(t.storage_identity()),
        CodeStageV1::Mapped(t) => ids.push(t.storage_identity()),
        CodeStageV1::Retained(a) => ids.push(Memory::code_identity(a)),
        CodeStageV1::InSession(id) => markers.push(*id),
        CodeStageV1::Empty | CodeStageV1::Allocating => {}
    }
    match &owner.kernarg {
        KernargStageV1::Cpu(t) => ids.push(t.storage_identity()),
        KernargStageV1::Mapped(t) => ids.push(t.storage_identity()),
        KernargStageV1::Retained(a) => ids.push(Memory::kernarg_identity(a)),
        KernargStageV1::InSession(id) => markers.push(*id),
        KernargStageV1::Empty | KernargStageV1::Allocating => {}
    }
    if let Some(completed) = &owner.completed {
        ids.extend(completed.code.iter().map(Memory::code_identity));
        ids.push(Memory::kernarg_identity(&completed.kernarg));
    }
    let allocating = if matches!(owner.current_code, CodeStageV1::Allocating) {
        Some(crate::shared_memory::SharedGttProfileV1::Executable)
    } else if matches!(owner.kernarg, KernargStageV1::Allocating) {
        Some(crate::shared_memory::SharedGttProfileV1::Kernarg)
    } else {
        None
    };
    memory.assert_control_custody(&ids, &markers, allocating);
}

fn assert_backing(memory: &Memory, before: &PreparationMemoryObservationV1) {
    memory.assert_data_unchanged(before);
    let after = memory.observation();
    assert_eq!(after.host, before.host);
    assert_eq!(after.device, before.device);
    assert_eq!(
        &after.calls[5..],
        &before.calls[5..],
        "preparation must not clean up or refund"
    );
    let allocations = after.calls[2] - before.calls[2];
    assert!(allocations == after.controls || after.pending && allocations == after.controls + 1);
}

pub(crate) struct PrimaryPreparationSnapshotV1 {
    data: Vec<Input>,
    data_vector: Option<(usize, usize)>,
    packets: Vec<PacketSnapshotV1>,
    expected_effects: Option<Vec<ExpectedDataEffectV1>>,
}

#[derive(Debug, Eq, PartialEq)]
struct PacketSnapshotV1 {
    program_index: usize,
    geometry: AqlDispatchGeometryV1,
    ordering: AqlDispatchOrderingV1,
    dynamic_group_segment_bytes: u32,
    kernarg_storage: usize,
    kernarg_bytes: Box<[u8]>,
    bindings_storage: usize,
    bindings: Box<[Gfx942DispatchBufferBindingV1]>,
}

impl PacketSnapshotV1 {
    fn capture(packet: &Gfx942FixedDispatchPacketV1) -> Self {
        Self {
            program_index: packet.program_index,
            geometry: packet.geometry,
            ordering: packet.ordering,
            dynamic_group_segment_bytes: packet.dynamic_group_segment_bytes,
            kernarg_storage: packet.kernarg_bytes.as_ptr() as usize,
            kernarg_bytes: packet.kernarg_bytes.clone(),
            bindings_storage: packet.buffers.as_ptr() as usize,
            bindings: packet.buffers.clone(),
        }
    }
}

#[derive(Default)]
pub(crate) struct PreparationOwnerRefsV1<'a> {
    pub(crate) shared: Vec<(SharedGttAllocationIdentityV1, SharedGttAllocationLayoutV1)>,
    pub(crate) device_leases: Vec<&'a Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>>,
    pub(crate) device_authorities: Vec<&'a Gfx942DeviceMemoryDispatchAuthorityV1>,
    pub(crate) in_session: Vec<SharedGttAllocationIdentityV1>,
}

impl<'a> PreparationOwnerRefsV1<'a> {
    pub(crate) fn data(&mut self, data: &'a [Gfx942FixedDispatchDataV1]) {
        for data in data {
            match data.storage_ref() {
                DispatchDataStorageRefV1::HostVisible(token) => {
                    self.shared.push((token.storage_identity(), token.layout()));
                }
                DispatchDataStorageRefV1::Device(lease) => self.device_leases.push(lease),
            }
        }
    }

    fn authority(&mut self, data: &'a DispatchDataAuthorityV1) {
        match data {
            DispatchDataAuthorityV1::HostVisible(authority) => self.shared.push((
                Memory::primary_token_identity(authority),
                authority.layout(),
            )),
            DispatchDataAuthorityV1::Device(authority) => self.device_authorities.push(authority),
        }
    }

    pub(in crate::queue) fn dispatch(&mut self, owner: &'a DispatchResourceOwnerV1) {
        self.shared.extend(
            owner
                .code
                .iter()
                .map(|a| (Memory::code_identity(a), a.layout())),
        );
        self.shared.push((
            Memory::kernarg_identity(&owner.kernarg),
            owner.kernarg.layout(),
        ));
        for data in &owner.data {
            self.authority(data);
        }
    }
}

impl PrimaryPreparationSnapshotV1 {
    pub(crate) fn packets(packets: &[Gfx942FixedDispatchPacketV1]) -> Self {
        Self {
            data: Vec::new(),
            data_vector: None,
            packets: packets.iter().map(PacketSnapshotV1::capture).collect(),
            expected_effects: None,
        }
    }

    pub(crate) fn with_expected_write_only_ranges_v1(mut self, ranges: &[(u64, u64)]) -> Self {
        assert_eq!(ranges.len(), self.data.len());
        self.expected_effects = Some(
            ranges
                .iter()
                .map(|&(offset, byte_len)| ExpectedDataEffectV1 {
                    effect: Some(DeviceDataEffectV1::WriteOnly),
                    writable_ranges: vec![CompletedWritableRangeV1 { offset, byte_len }],
                })
                .collect(),
        );
        self
    }

    pub(crate) fn capture_data(&mut self, data: &[Gfx942FixedDispatchDataV1]) {
        self.data = inputs(data);
    }

    pub(crate) fn capture_data_vector_v1(
        &mut self,
        data: &[Gfx942FixedDispatchDataV1],
        capacity: usize,
    ) {
        self.capture_data(data);
        self.data_vector = Some((data.as_ptr() as usize, capacity));
    }

    pub(crate) fn assert_data(&self, data: &[Gfx942FixedDispatchDataV1]) {
        assert_eq!(inputs(data), self.data);
    }
}

impl<const N: usize, P: PreparationPacketsV1<N>> FixedDispatchPreparationCustodyV1<N, P> {
    pub(crate) fn primary_collect_owners_v1<'a>(&'a self, out: &mut PreparationOwnerRefsV1<'a>) {
        out.data(&self.original_data);
        if let Some(retained) = &self.retained_data {
            for data in retained.iter() {
                out.authority(&data.authority);
            }
        }
        for data in &self.data_authorities {
            out.authority(data);
        }
        out.shared.extend(
            self.code
                .iter()
                .map(|a| (Memory::code_identity(a), a.layout())),
        );
        match &self.current_code {
            CodeStageV1::Cpu(t) => out.shared.push((t.storage_identity(), t.layout())),
            CodeStageV1::Immutable(t) => out.shared.push((t.storage_identity(), t.layout())),
            CodeStageV1::Mapped(t) => out.shared.push((t.storage_identity(), t.layout())),
            CodeStageV1::Retained(a) => out.shared.push((Memory::code_identity(a), a.layout())),
            CodeStageV1::InSession(id) => out.in_session.push(*id),
            CodeStageV1::Empty | CodeStageV1::Allocating => {}
        }
        match &self.kernarg {
            KernargStageV1::Cpu(t) => out.shared.push((t.storage_identity(), t.layout())),
            KernargStageV1::Mapped(t) => out.shared.push((t.storage_identity(), t.layout())),
            KernargStageV1::Retained(a) => {
                out.shared.push((Memory::kernarg_identity(a), a.layout()))
            }
            KernargStageV1::InSession(id) => out.in_session.push(*id),
            KernargStageV1::Empty | KernargStageV1::Allocating => {}
        }
        if let Some(completed) = &self.completed {
            out.dispatch(completed);
        }
    }

    pub(crate) fn primary_snapshot_v1(&self) -> PrimaryPreparationSnapshotV1 {
        PrimaryPreparationSnapshotV1 {
            data: inputs(&self.original_data),
            data_vector: Some((
                self.original_data.as_ptr() as usize,
                self.original_data.capacity(),
            )),
            packets: self
                .packets
                .as_packets()
                .iter()
                .map(PacketSnapshotV1::capture)
                .collect(),
            expected_effects: None,
        }
    }

    pub(crate) fn primary_inject_stage_v1(&mut self, stage: PreparationStageV1, panic: bool) {
        assert_eq!(self.stage, PreparationStageV1::Fresh);
        self.fault = Some((stage, panic));
    }

    pub(crate) fn primary_assert_failed_stage_v1(&self, stage: PreparationStageV1) {
        assert_eq!(self.stage, stage);
        assert!(self.failed);
        assert!(self.completed().is_err());
        let prefix = match stage {
            PreparationStageV1::CodeAllocate(i)
            | PreparationStageV1::CodeMaterialize(i)
            | PreparationStageV1::CodeSeal(i)
            | PreparationStageV1::CodeMap(i)
            | PreparationStageV1::CodeRetain(i)
            | PreparationStageV1::CodeResolve(i) => i,
            PreparationStageV1::KernargAllocate
            | PreparationStageV1::KernargMaterialize
            | PreparationStageV1::KernargMap
            | PreparationStageV1::KernargRetain
            | PreparationStageV1::PacketResolve(_)
            | PreparationStageV1::Commit => self.program_identity.len(),
            _ => 0,
        };
        assert_eq!(self.code.len(), prefix);
        assert_eq!(self.code_identity.len(), prefix);
        if stage == PreparationStageV1::Complete {
            let completed = self.completed.as_ref().unwrap();
            assert_eq!(completed.code.len(), self.program_identity.len());
            assert_eq!(completed.code_identity.len(), self.program_identity.len());
            assert_eq!(completed.packets.len(), N);
        }
    }

    pub(in crate::queue) fn primary_assert_snapshot_v1(
        &self,
        memory: &Memory,
        expected: &PrimaryPreparationSnapshotV1,
        transferred: Option<&DispatchResourceOwnerV1>,
    ) {
        self.primary_assert_descriptors_v1(expected, transferred);
        if transferred.is_none() {
            assert_custody(memory, self);
        }
    }

    pub(in crate::queue) fn primary_assert_descriptors_v1(
        &self,
        expected: &PrimaryPreparationSnapshotV1,
        transferred: Option<&DispatchResourceOwnerV1>,
    ) {
        assert_eq!(
            self.packets
                .as_packets()
                .iter()
                .map(PacketSnapshotV1::capture)
                .collect::<Vec<_>>(),
            expected.packets,
            "exact original packet descriptors and backing"
        );
        if let Some(vector) = expected.data_vector {
            assert_eq!(
                (
                    self.original_data.as_ptr() as usize,
                    self.original_data.capacity()
                ),
                vector,
                "original data vector backing"
            );
        }
        assert_inputs_with_completed(
            self,
            &expected.data,
            expected.expected_effects.as_deref(),
            transferred.or(self.completed.as_ref()),
        );
        if transferred.is_some() {
            assert_eq!(self.stage, PreparationStageV1::Transferred);
            assert!(!self.failed && self.completed.is_none());
        }
    }

    pub(in crate::queue) fn assert_fresh_pristine_occurrence_for_test(&self, previous: u64) {
        let generation = self
            .generation
            .as_ref()
            .or_else(|| self.completed.as_ref().map(|owner| &owner.generation))
            .expect("retained pristine generation");
        assert_ne!(generation.recipe_occurrence, previous);
    }

    pub(in crate::queue) fn primary_assert_replacement_generation_v1(
        &self,
        next_generation: Option<u64>,
        transferred: Option<&DispatchResourceOwnerV1>,
    ) {
        let owners = self
            .generation
            .iter()
            .chain(self.completed.iter().map(|owner| &owner.generation))
            .chain(transferred.map(|owner| &owner.generation))
            .collect::<Vec<_>>();
        assert_eq!(owners.len(), usize::from(next_generation.is_some()));
        if let Some(next_generation) = next_generation {
            let owner = owners[0];
            assert_eq!(
                owner.next_generation, next_generation,
                "exact replacement generation"
            );
            assert_ne!(owner.recipe_occurrence, 0);
            assert_eq!(owner.recipe_queue, None);
            assert!(
                owner
                    .slots
                    .iter()
                    .all(|slot| *slot == DispatchEpochSlotV1::VACANT)
            );
            assert_eq!(owner.recycled_generation, None);
            assert!(!owner.poisoned);
        }
        assert_eq!(
            self.control,
            PersistentFixedDispatchControlStateV1::Ordinary
        );
        for owner in self.completed.iter().chain(transferred) {
            assert_eq!(
                owner.persistent_control,
                PersistentFixedDispatchControlStateV1::Ordinary
            );
        }
    }
}

fn run<const N: usize>(
    owner: &mut FixedDispatchPreparationCustodyV1<N>,
    memory: &mut Memory,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    prepare_public_fixed_dispatch_resources_in_place(memory, &programs(), owner)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindFault {
    None,
    Error,
    Panic,
}

fn check_bind_settlement(
    memory: &mut Memory,
    owner: &mut FixedDispatchPreparationCustodyV1<1>,
    prepare: impl FnOnce(
        &mut Memory,
        &mut FixedDispatchPreparationCustodyV1<1>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1>,
    closing: BindFault,
    validation: BindFault,
) {
    use crate::queue::live::model_loan::execute_live_model_custody_v1;
    use crate::queue::live::persistent_bind::settle_persistent_bind_preparation_v1;
    let operation_fault = owner.fault;
    let mut retakes = 0;
    let mut validations = 0;
    let mut poisoned = false;
    let result = settle_persistent_bind_preparation_v1(
        memory,
        owner,
        |memory, owner| {
            let (result, closing) = execute_live_model_custody_v1(
                memory,
                |_| Ok(()),
                |memory| prepare(memory, owner),
                |_, ()| {
                    retakes += 1;
                    match closing {
                        BindFault::None => Ok(()),
                        BindFault::Error => Err(Gfx942DispatchBindingErrorV1::Poisoned),
                        BindFault::Panic => std::panic::panic_any("bind retake"),
                    }
                },
                |_| poisoned = true,
            )?;
            closing?;
            result
        },
        |_, owner| {
            validations += 1;
            assert!(owner.completed().is_ok());
            match validation {
                BindFault::None => Ok(()),
                BindFault::Error => Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
                BindFault::Panic => std::panic::panic_any("bind validation"),
            }
        },
    );
    assert_eq!(retakes, 1);
    let operation_panics = operation_fault.is_some_and(|(_, panic)| panic);
    assert_eq!(poisoned, operation_panics || closing != BindFault::None);
    assert_eq!(
        validations,
        usize::from(operation_fault.is_none() && closing == BindFault::None)
    );
    if operation_panics {
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<(&str, PreparationStageV1)>(),
            Some(&("dispatch preparation", operation_fault.unwrap().0))
        );
    } else if closing == BindFault::Panic {
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"bind retake")
        );
    } else if validations != 0 && validation == BindFault::Panic {
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"bind validation")
        );
    } else {
        let result = result.unwrap();
        assert_eq!(
            result.is_err(),
            operation_fault.is_some()
                || closing != BindFault::None
                || validation != BindFault::None
        );
        if closing == BindFault::Error {
            assert!(matches!(
                result,
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
        }
    }
}

fn check_single_bind_settlement(
    fault: Option<(PreparationStageV1, bool)>,
    closing: BindFault,
    validation: BindFault,
) {
    let mut memory = Memory::new(true);
    let data = memory.device(true);
    let queue = super::super::tests::persistent_control_test_queue(41);
    let programs = programs();
    let packets = [packet(0)];
    let role = Gfx942DeviceContentRoleV1::new([0x61; 32], 0).unwrap();
    let identity = persistent_fixed_dispatch_control_identity_v1(
        queue,
        &programs,
        &packets,
        data.layout(),
        true,
        role,
        data.sdma_storage_identity(),
    )
    .unwrap();
    let expected = inputs(core::slice::from_ref(&data));
    let before = memory.observation();
    let mut owner = FixedDispatchPreparationCustodyV1::new(packets, vec![data]);
    owner.fault = fault;
    check_bind_settlement(
        &mut memory,
        &mut owner,
        |memory, owner| {
            prepare_persistent_fixed_dispatch_resources_v1(
                memory,
                &programs,
                owner,
                Some(7),
                identity,
            )
        },
        closing,
        validation,
    );
    assert_inputs(&owner, &expected);
    assert_custody(&memory, &owner);
    assert_backing(&memory, &before);
    if fault.is_some() {
        assert_eq!(owner.generation.as_ref().unwrap().next_generation, 8);
        return;
    }
    let completed = owner.completed.as_ref().unwrap();
    assert_eq!(completed.data_premises[0].role_identity, role.identity());
    assert_eq!(completed.generation.next_generation, 8);
    assert_eq!(completed.generation.recycled_generation, None);
    assert_eq!(
        completed.persistent_control,
        PersistentFixedDispatchControlStateV1::Attached(
            BoundedPersistentFixedDispatchControlIdentityV1::from_single(identity)
        )
    );
}

fn check_three_bind_settlement(
    fault: Option<(PreparationStageV1, bool)>,
    closing: BindFault,
    validation: BindFault,
) {
    use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
    let image =
        include_bytes!("../../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    let program = validate(image, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .reconcile_dispatch_abi(
            [0x71; 32],
            &[
                KernelGlobalBufferAbiV1::new(0, "arg0.data", 0, 4, ArgumentAccess::ReadOnly),
                KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 4, ArgumentAccess::ReadOnly),
                KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 4, ArgumentAccess::WriteOnly),
            ],
        )
        .unwrap();
    let mut bytes = [0; 48];
    for offset in [8, 24, 40] {
        bytes[offset..offset + 8].copy_from_slice(&1024_u64.to_le_bytes());
    }
    let packets = [Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new([1024, 1, 1], [256, 1, 1]).unwrap(),
        0,
        bytes.into(),
        vec![
            Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096),
            Gfx942DispatchBufferBindingV1::new(2, 1, 0, 4096),
            Gfx942DispatchBufferBindingV1::new(4, 2, 0, 4096),
        ]
        .into_boxed_slice(),
    )];
    let mut memory = Memory::new(true);
    let data = vec![
        memory.device(true),
        memory.device(true),
        memory.device(true),
    ];
    let expected = inputs(&data);
    let before = memory.observation();
    let roles = core::array::from_fn(|index| {
        Gfx942DeviceContentRoleV1::new([0x62; 32], index as u32).unwrap()
    });
    let identity = three_binding_persistent_fixed_dispatch_control_identity_v1(
        super::super::tests::persistent_control_test_queue(42),
        core::slice::from_ref(&program),
        &packets,
        core::array::from_fn(|index| data[index].layout()),
        [true, true, true],
        roles,
        core::array::from_fn(|index| data[index].sdma_storage_identity()),
    )
    .unwrap();
    let mut owner = FixedDispatchPreparationCustodyV1::new(packets, data);
    owner.fault = fault;
    check_bind_settlement(
        &mut memory,
        &mut owner,
        |memory, owner| {
            prepare_three_binding_persistent_fixed_dispatch_resources_v1(
                memory,
                &[program],
                owner,
                Some(7),
                identity,
            )
        },
        closing,
        validation,
    );
    assert_custody(&memory, &owner);
    assert_backing(&memory, &before);
    let effects = [
        DeviceDataEffectV1::ReadOnly,
        DeviceDataEffectV1::ReadOnly,
        DeviceDataEffectV1::WriteOnly,
    ];
    if let Some(completed) = &owner.completed {
        assert_eq!(completed.generation.next_generation, 8);
        assert_eq!(completed.data.len(), 3);
        for index in 0..3 {
            let premise = &completed.data_premises[index];
            assert_eq!(
                Memory::data_storage(&completed.data[index]),
                expected[index].storage
            );
            assert_eq!(premise.initialized_content, expected[index].content);
            assert_eq!(premise.fully_initialized, expected[index].initialized);
            assert_eq!(premise.role_identity, roles[index].identity());
            assert_eq!(premise.effect, Some(effects[index]));
        }
    } else {
        assert_eq!(owner.generation.as_ref().unwrap().next_generation, 8);
        let retained = owner.retained_data.as_ref().unwrap();
        assert_eq!(retained.len(), 3);
        for index in 0..3 {
            assert_eq!(
                Memory::data_storage(&retained[index].authority),
                expected[index].storage
            );
            assert_eq!(retained[index].initialized_content, expected[index].content);
            assert_eq!(
                owner.plan.as_ref().unwrap().data[index].effect,
                Some(effects[index])
            );
        }
    }
}

#[path = "preparation_tests/settlement_tests.rs"]
mod settlement_tests;

#[path = "preparation_tests/fault_fixture.rs"]
mod fault_fixture;
use fault_fixture::native_fault;
use fault_fixture::stage_fault;
