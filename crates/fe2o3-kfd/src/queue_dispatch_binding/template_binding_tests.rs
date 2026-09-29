use super::*;
use crate::queue::completion::CompletionSignalArenaOwnerV1;
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};

fn fixture() -> (Memory, DispatchResourceOwnerV1, QueueKeyV1) {
    let mut memory = Memory::with_host_budget(true, 1 << 30, 1 << 20);
    let mut custody =
        FixedDispatchPreparationCustodyV1::new([packet(2), packet(0), packet(1)], memory.roster());
    run(&mut custody, &mut memory).unwrap();
    let mut queue = test_dispatch_queue_v1();
    queue.vm = memory.primary_vm();
    (memory, custody.take_completed().unwrap(), queue)
}

fn cleanup(owner: DispatchResourceOwnerV1, memory: &mut Memory) {
    let mut cleanup =
        ReturningControlCleanupCustodyV1::new(owner, ReturningControlModeV1::Ordinary);
    cleanup.release_ordinary_in_place(memory).unwrap();
    assert!(cleanup.is_complete());
    memory.primary_assert_all_released_v1();
}

#[derive(Debug, Eq, PartialEq)]
struct Resources {
    code: Vec<ResolvedCodeIdentityV1>,
    packets: Vec<PreparedDispatchPacketV1>,
    identities: Vec<SharedGttAllocationIdentityV1>,
    data: Vec<Gfx942SdmaBufferStorageIdentityV1>,
    premises: String,
    control: PersistentFixedDispatchControlStateV1,
    storage: [(usize, usize); 5],
    bytes: Vec<Vec<u8>>,
}

fn resources(owner: &DispatchResourceOwnerV1, memory: &Memory) -> Resources {
    Resources {
        code: owner.code_identity.clone(),
        packets: owner.packets.clone(),
        identities: owner.primary_fixture_identities_v1(),
        data: owner.data.iter().map(Memory::data_storage).collect(),
        premises: format!("{:?}", owner.data_premises),
        control: owner.persistent_control,
        storage: [
            (owner.code.as_ptr() as usize, owner.code.capacity()),
            (
                owner.code_identity.as_ptr() as usize,
                owner.code_identity.capacity(),
            ),
            (owner.packets.as_ptr() as usize, owner.packets.capacity()),
            (owner.data.as_ptr() as usize, owner.data.capacity()),
            (
                owner.data_premises.as_ptr() as usize,
                owner.data_premises.capacity(),
            ),
        ],
        bytes: owner
            .code
            .iter()
            .map(|code| code.facts().mapping())
            .chain([owner.kernarg.facts().mapping()])
            .map(|mapping| memory.mapped_bytes(mapping).to_vec())
            .collect(),
    }
}

fn rejected<const N: usize>(
    owner: &mut DispatchResourceOwnerV1,
    memory: &Memory,
    queue: QueueKeyV1,
    error: &str,
) {
    let generation = owner.source_failure_snapshot_v1();
    let retained = resources(owner, memory);
    let observation = memory.observation();
    assert_eq!(
        owner.bind_templates::<N>(queue).unwrap_err().to_string(),
        error
    );
    assert_eq!(owner.source_failure_snapshot_v1(), generation);
    assert_eq!(resources(owner, memory), retained);
    assert_eq!(memory.observation(), observation);
}

#[test]
fn real_prepared_templates_preserve_packet_program_association_and_cancel_without_refunding() {
    let (mut memory, mut owner, queue) = fixture();
    let retained = resources(&owner, &memory);
    let before_memory = memory.observation();
    let mut completion =
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue);
    for generation in 1..=3 {
        let expected = std::array::from_fn::<_, 3, _>(|i| {
            let packet = &retained.packets[i];
            let code = &retained.code[[2, 0, 1][i]];
            CompletionPacketTemplateV1::new(
                packet.geometry,
                packet.ordering,
                packet.private_segment_size,
                packet.group_segment_size,
                code.descriptor_address,
                packet.kernarg_address,
                packet.kernarg_alignment,
                CompletionDispatchGenerationBindingV1::new(
                    queue,
                    code.mapping,
                    packet.kernarg_mapping,
                    generation,
                ),
            )
        });
        let (templates, epoch) = owner.bind_templates::<3>(queue).unwrap();
        assert_eq!(&*templates, &expected);
        assert_eq!(epoch.dispatch_generation, generation);
        assert_eq!(epoch.slot_generation, generation);
        assert_eq!(epoch.slot_index, 0);
        let bound = completion.bind_boxed_batch(templates).unwrap();
        let events = completion
            .record_dependency_event_batch_for_bound_v1(101, generation, &bound)
            .unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(
            completion
                .release_dependency_event_batch_v1(events)
                .unwrap(),
            3
        );
        let (_, retention) = bound.into_parts();
        completion.cancel_bound(retention).unwrap();
        owner.cancel_binding(epoch).unwrap();
        assert_eq!(completion.state_snapshot_for_test(), (generation + 1, 8192));
        assert_eq!(owner.generation.next_generation, generation + 1);
        assert_eq!(owner.generation.slots[0].slot_generation, generation);
        assert_eq!(
            owner.generation.slots[0].phase,
            DispatchEpochPhaseV1::Vacant
        );
        assert!(owner.cancel_binding(epoch).is_err());
        assert_eq!(resources(&owner, &memory), retained);
        assert_eq!(memory.observation(), before_memory);
    }
    cleanup(owner, &mut memory);
}

#[test]
fn template_refusal_precedence_preserves_resources_and_all_epoch_fields() {
    let (mut memory, mut owner, queue) = fixture();
    owner.generation.poisoned = true;
    rejected::<0>(&mut owner, &memory, queue, "Poisoned");
    owner.generation.poisoned = false;
    rejected::<0>(&mut owner, &memory, queue, "ZeroPacketCount");
    rejected::<8193>(
        &mut owner,
        &memory,
        queue,
        "PacketCountExceedsMaximum { requested: 8193, maximum: 8192 }",
    );
    rejected::<2>(&mut owner, &memory, queue, "WrongQueueGeneration");
    let packets = owner.packets.clone();
    owner.packets[0].code_index = usize::MAX;
    owner.packets[1].ordering = AqlDispatchOrderingV1::Independent;
    owner.packets[2].ordering = AqlDispatchOrderingV1::Independent;
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 1, detail: \"multi-inflight recipe requires wait-for-prior ordering\" }",
    );
    owner.packets[1].ordering = AqlDispatchOrderingV1::WaitForPrior;
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 2, detail: \"multi-inflight recipe requires wait-for-prior ordering\" }",
    );
    owner.packets[2].ordering = AqlDispatchOrderingV1::WaitForPrior;
    let mut wrong_queue = queue;
    wrong_queue.generation.0 += 1;
    owner.generation.recipe_queue = Some(wrong_queue);
    rejected::<3>(&mut owner, &memory, queue, "WrongQueueGeneration");
    owner.generation.recipe_queue = None;
    owner.generation.next_generation = u64::MAX;
    rejected::<3>(&mut owner, &memory, queue, "GenerationExhausted");
    owner.generation.next_generation = 1;
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "InvalidCode(\"packet program index\")",
    );
    owner.packets.copy_from_slice(&packets);
    owner.packets[0].code_bound_kernarg_layout = true;
    owner.packets[0].kernarg_layout_identity = [0xff; 32];
    owner.packets[2].code_index = usize::MAX;
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 0, detail: \"prepared kernarg dispatch ABI identity\" }",
    );
    owner.packets.copy_from_slice(&packets);
    owner.packets[2].code_bound_kernarg_layout = true;
    owner.packets[2].kernarg_layout_identity = [0xff; 32];
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "InvalidKernarg { packet: 2, detail: \"prepared kernarg dispatch ABI identity\" }",
    );
    owner.packets[2].code_bound_kernarg_layout = false;
    let (_, id) = owner.bind_templates::<3>(queue).unwrap();
    assert_eq!(id.dispatch_generation, 1);
    owner.cancel_binding(id).unwrap();
    owner.packets.copy_from_slice(&packets);
    cleanup(owner, &mut memory);
}

#[test]
fn template_epoch_capacity_precedes_late_template_errors_and_scaled_multi_packet_refusal() {
    let (mut memory, mut owner, queue) = fixture();
    let mut ids = Vec::new();
    for _ in 0..64 {
        ids.push(owner.bind_templates::<3>(queue).unwrap().1);
    }
    let original = owner.packets[2];
    owner.packets[2].code_index = usize::MAX;
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "DispatchEpochCapacity { maximum: 64 }",
    );
    owner.cancel_binding(ids[63]).unwrap();
    rejected::<3>(
        &mut owner,
        &memory,
        queue,
        "InvalidCode(\"packet program index\")",
    );
    owner.packets[2] = original;
    let (_, replacement) = owner.bind_templates::<3>(queue).unwrap();
    assert_eq!(
        (
            replacement.slot_index,
            replacement.slot_generation,
            replacement.dispatch_generation
        ),
        (63, 2, 65)
    );
    owner.cancel_binding(replacement).unwrap();
    for id in ids.into_iter().take(63) {
        owner.cancel_binding(id).unwrap();
    }
    // Raw profile substitution freezes the reserve rule, not constructor reachability.
    owner.generation.capacity_profile = FixedDispatchCapacityProfileV1::Qualification1024;
    rejected::<3>(&mut owner, &memory, queue, "StaleDispatchGeneration");
    owner.generation.capacity_profile = FixedDispatchCapacityProfileV1::Default64;
    cleanup(owner, &mut memory);
}

#[test]
fn template_shape_and_unused_code_vm_checks_precede_packet_validation() {
    let (mut memory, mut owner, queue) = fixture();
    let original = owner.packets.clone();
    owner.packets[0].code_index = usize::MAX;
    let code = owner.code_identity.pop().unwrap();
    rejected::<3>(&mut owner, &memory, queue, "WrongQueueGeneration");
    owner.code_identity.push(code);
    let premise = owner.data_premises.pop().unwrap();
    rejected::<3>(&mut owner, &memory, queue, "WrongQueueGeneration");
    owner.data_premises.push(premise);
    owner.packets.copy_from_slice(&original);
    owner.packets[2].code_index = 0;
    let identity = owner.code_identity[1];
    owner.code_identity[1].mapping.allocation.vm.id.0 += 1;
    rejected::<3>(&mut owner, &memory, queue, "WrongQueueGeneration");
    owner.code_identity[1] = identity;
    owner.packets.copy_from_slice(&original);
    cleanup(owner, &mut memory);
}

#[test]
fn maximum_template_binding_and_refusal_fit_a_two_mib_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let (mut memory, mut owner, queue) = fixture();
            rejected::<8192>(&mut owner, &memory, queue, "WrongQueueGeneration");
            rejected::<8193>(
                &mut owner,
                &memory,
                queue,
                "PacketCountExceedsMaximum { requested: 8193, maximum: 8192 }",
            );
            // Stress the retained-metadata binder, not maximum-size constructor
            // reachability: each row references the same genuine prepared packet.
            owner.packets = vec![owner.packets[0]; 8192];
            let retained = resources(&owner, &memory);
            let (templates, epoch) = owner.bind_templates::<8192>(queue).unwrap();
            assert_eq!(templates.len(), 8192);
            let mut completion =
                CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(queue);
            let bound = completion.bind_boxed_batch(templates).unwrap();
            let events = completion
                .record_dependency_event_batch_for_bound_v1(101, 1, &bound)
                .unwrap();
            assert_eq!(events.len(), 8192);
            assert_eq!(
                completion
                    .release_dependency_event_batch_v1(events)
                    .unwrap(),
                8192
            );
            let (_, retention) = bound.into_parts();
            completion.cancel_bound(retention).unwrap();
            owner.cancel_binding(epoch).unwrap();
            assert_eq!(completion.state_snapshot_for_test(), (2, 8192));
            assert_eq!(owner.generation.next_generation, 2);
            assert_eq!(resources(&owner, &memory), retained);
            cleanup(owner, &mut memory);
        })
        .unwrap()
        .join()
        .unwrap();
}
