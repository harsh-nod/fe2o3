//! Actual preparation/binder control flow with synthetic native memory only.
#![cfg(test)]

use super::*;

fn members<'a, const N: usize>(
    memory: &mut Memory,
    bytes: &'a [u8],
) -> [Gfx942NativeFillCohortMemberV1<'a>; N] {
    core::array::from_fn(|index| {
        let count = 1 + index as u64 * 64;
        Gfx942NativeFillCohortMemberV1::new(
            program(bytes),
            input(count, (index as u32 + 1) * 64),
            data(memory, count as usize * 4),
        )
    })
}

fn prepare_cohort<const N: usize>(
    memory: &mut Memory,
    cohort: Gfx942NativeFillCohortV1<'_, N>,
) -> DispatchResourceOwnerV1 {
    let Gfx942NativeFillCohortV1 {
        programs,
        packets,
        data,
    } = cohort;
    let mut custody = FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data);
    custody
        .prepare_in_place(
            memory,
            &programs,
            DispatchGenerationOwnerV1::new(),
            PersistentFixedDispatchControlStateV1::Ordinary,
        )
        .unwrap();
    custody.take_completed().unwrap()
}

#[test]
fn cohort_three_original_outputs_have_distinct_packets_images_and_whole_batch_custody() {
    let captured = payload();
    let mut memory = Memory::new(true);
    let cohort =
        Gfx942NativeFillCohortV1::admit(members::<3>(&mut memory, captured.exact_payload_bytes()))
            .unwrap();
    assert_eq!(cohort.member_count(), 3);
    let originals: Vec<_> = cohort
        .data
        .iter()
        .map(Gfx942FixedDispatchDataV1::sdma_storage_identity)
        .collect();
    assert!(
        originals
            .iter()
            .enumerate()
            .all(|(index, id)| !originals[..index].contains(id))
    );
    let mut owner = prepare_cohort(&mut memory, cohort);
    assert_eq!(
        (owner.code.len(), owner.packets.len(), owner.data.len()),
        (3, 3, 3)
    );
    let images = memory.mapped_bytes(owner.kernarg.facts().mapping());
    assert_eq!(images.len(), 3 * 272);
    for index in 0..3 {
        let image = &images[index * 272..(index + 1) * 272];
        let count = 1 + index as u64 * 64;
        assert_eq!(u64::from_le_bytes(image[8..16].try_into().unwrap()), count);
        assert_eq!(
            Some(u64::from_le_bytes(image[..8].try_into().unwrap())),
            owner.data[index].checked_gpu_subrange(0, count * 4, 4)
        );
        assert_eq!(owner.packets[index].code_index, index);
    }
    let mut queue = test_dispatch_queue_v1();
    queue.vm = memory.primary_vm();
    let before = owner.source_failure_snapshot_v1();
    assert!(owner.bind_templates::<1>(queue).is_err());
    assert_eq!(owner.source_failure_snapshot_v1(), before);
    let (templates, epoch) = owner.bind_templates::<3>(queue).unwrap();
    assert_eq!(templates.len(), 3);
    assert!(owner.bind_templates::<3>(queue).is_err());
    owner.cancel_binding(epoch).unwrap();
    assert!(owner.bind_templates::<3>(queue).is_err());
    cleanup(owner, &mut memory);
    memory.primary_assert_all_released_v1();
}

fn rejected_count<const N: usize>() {
    let captured = payload();
    let mut memory = Memory::new(true);
    let mut identities = Vec::new();
    let original = members::<N>(&mut memory, captured.exact_payload_bytes()).map(|member| {
        let (program, packet, output) = member.into_parts();
        identities.push(output.sdma_storage_identity());
        Gfx942NativeFillCohortMemberV1::new(program, packet, output)
    });
    let before = memory.observation();
    let Err(failure) = Gfx942NativeFillCohortV1::admit(original) else {
        panic!("unsupported cohort size")
    };
    let (returned, _) = failure.into_parts();
    assert_eq!(returned.len(), N);
    assert_eq!(memory.observation(), before);
    // These are CPU fixture owners, retained through the refusal. Teardown here
    // is not a native cancellation/release observation.
    let mut returned_count = 0;
    for (member, identity) in returned.into_iter().zip(identities) {
        let (_, _, output) = member.into_parts();
        assert_eq!(output.sdma_storage_identity(), identity);
        returned_count += 1;
    }
    assert_eq!(returned_count, N);
}

#[test]
fn cohort_bounds_refuse_without_native_entry_or_singleton_widening() {
    rejected_count::<0>();
    rejected_count::<1>();
    rejected_count::<17>();
    let captured = payload();
    let mut memory = Memory::new(true);
    let cohort =
        Gfx942NativeFillCohortV1::admit(members::<3>(&mut memory, captured.exact_payload_bytes()))
            .unwrap();
    let Gfx942NativeFillCohortV1 {
        programs,
        packets,
        data,
    } = cohort;
    let mut old = FixedDispatchPreparationCustodyV1::new(packets, data);
    let before = memory.observation();
    assert!(
        old.prepare_in_place(
            &mut memory,
            &programs,
            DispatchGenerationOwnerV1::new(),
            PersistentFixedDispatchControlStateV1::Ordinary
        )
        .is_err()
    );
    assert_eq!(memory.observation(), before);
    assert_custody(&memory, &old);
}

fn admitted_bound<const N: usize>() {
    let captured = payload();
    let mut memory = Memory::new(true);
    let cohort =
        Gfx942NativeFillCohortV1::admit(members::<N>(&mut memory, captured.exact_payload_bytes()))
            .unwrap();
    let mut owner = prepare_cohort(&mut memory, cohort);
    let mut queue = test_dispatch_queue_v1();
    queue.vm = memory.primary_vm();
    let (templates, epoch) = owner.bind_templates::<N>(queue).unwrap();
    assert_eq!(templates.len(), N);
    owner.cancel_binding(epoch).unwrap();
    cleanup(owner, &mut memory);
    memory.primary_assert_all_released_v1();
}

#[test]
fn cohort_exact_lower_and_upper_bounds_use_original_existing_batch_capacity() {
    admitted_bound::<2>();
    admitted_bound::<16>();
}

#[test]
fn cohort_aggregate_alias_and_missing_final_member_refuse_before_native_preparation() {
    let captured = payload();
    for mutation in 0..4 {
        let mut memory = Memory::new(true);
        let mut cohort = Gfx942NativeFillCohortV1::admit(members::<3>(
            &mut memory,
            captured.exact_payload_bytes(),
        ))
        .unwrap();
        match mutation {
            0 => cohort.packets[2].buffers[0].data_index = 0,
            1 => cohort.packets[2].program_index = 0,
            2 => cohort.packets[2].conditional_fill = false,
            3 => cohort.packets[2].kernarg_bytes[8] ^= 1,
            _ => unreachable!(),
        }
        let Gfx942NativeFillCohortV1 {
            programs,
            packets,
            data,
        } = cohort;
        let mut custody = FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data);
        let before = memory.observation();
        assert!(
            custody
                .prepare_in_place(
                    &mut memory,
                    &programs,
                    DispatchGenerationOwnerV1::new(),
                    PersistentFixedDispatchControlStateV1::Ordinary
                )
                .is_err()
        );
        assert_eq!(memory.observation(), before);
        assert_custody(&memory, &custody);
    }
}

#[test]
fn cohort_member_shape_refusals_return_all_original_data() {
    let captured = payload();
    for mutation in 0..8 {
        let mut memory = Memory::new(true);
        let mut original = members::<3>(&mut memory, captured.exact_payload_bytes());
        let [a, b, c] = original;
        let (p, mut packet, output) = b.into_parts();
        let identity = output.sdma_storage_identity();
        match mutation {
            0 => packet.conditional_fill = false,
            1 => packet.ordering = AqlDispatchOrderingV1::Independent,
            2 => packet.program_index = 1,
            3 => packet.buffers[0].data_index = 1,
            4 => packet.buffers[0].data_byte_offset = 4,
            5 => packet.dynamic_group_segment_bytes = 4,
            6 => packet.kernarg_bytes[8] ^= 1,
            7 => packet.buffers[0].byte_len -= 4,
            _ => unreachable!(),
        }
        original = [a, Gfx942NativeFillCohortMemberV1::new(p, packet, output), c];
        let before = memory.observation();
        let Err(failure) = Gfx942NativeFillCohortV1::admit(original) else {
            panic!("member mutation {mutation}")
        };
        let ([a, b, c], _) = failure.into_parts();
        assert_eq!(b.into_parts().2.sdma_storage_identity(), identity);
        assert_eq!(a.into_parts().2.layout().requested_bytes(), 4);
        assert_eq!(c.into_parts().2.layout().requested_bytes(), 129 * 4);
        assert_eq!(memory.observation(), before);
    }
}

#[test]
fn cohort_native_owner_substitutions_and_member_reordering_do_not_bind() {
    let captured = payload();
    let mut memory = Memory::new(true);
    let first =
        Gfx942NativeFillCohortV1::admit(members::<3>(&mut memory, captured.exact_payload_bytes()))
            .unwrap();
    let second =
        Gfx942NativeFillCohortV1::admit(members::<3>(&mut memory, captured.exact_payload_bytes()))
            .unwrap();
    let mut left = prepare_cohort(&mut memory, first);
    let mut right = prepare_cohort(&mut memory, second);
    let mut queue = test_dispatch_queue_v1();
    queue.vm = memory.primary_vm();
    let before = left.source_failure_snapshot_v1();
    for index in 0..3 {
        core::mem::swap(&mut left.data[index], &mut right.data[index]);
        assert!(left.bind_templates::<3>(queue).is_err());
        core::mem::swap(&mut left.data[index], &mut right.data[index]);
        core::mem::swap(&mut left.code[index], &mut right.code[index]);
        assert!(left.bind_templates::<3>(queue).is_err());
        core::mem::swap(&mut left.code[index], &mut right.code[index]);
    }
    left.packets.swap(0, 2);
    assert!(left.bind_templates::<3>(queue).is_err());
    left.packets.swap(0, 2);
    core::mem::swap(&mut left.kernarg, &mut right.kernarg);
    assert!(left.bind_templates::<3>(queue).is_err());
    core::mem::swap(&mut left.kernarg, &mut right.kernarg);
    assert_eq!(left.source_failure_snapshot_v1(), before);
    cleanup(left, &mut memory);
    cleanup(right, &mut memory);
    memory.primary_assert_all_released_v1();
}

#[test]
fn cohort_partial_preparation_errors_and_panics_keep_every_original_owner() {
    let captured = payload();
    for stage in [
        PreparationStageV1::Capacity,
        PreparationStageV1::CodeAllocate(0),
        PreparationStageV1::CodeMap(1),
        PreparationStageV1::CodeResolve(2),
        PreparationStageV1::KernargMap,
        PreparationStageV1::PacketResolve(2),
        PreparationStageV1::Commit,
        PreparationStageV1::Complete,
    ] {
        for panic in [false, true] {
            let mut memory = Memory::new(true);
            let cohort = Gfx942NativeFillCohortV1::admit(members::<3>(
                &mut memory,
                captured.exact_payload_bytes(),
            ))
            .unwrap();
            let Gfx942NativeFillCohortV1 {
                programs,
                packets,
                data,
            } = cohort;
            let mut custody =
                FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data);
            custody.primary_inject_stage_v1(stage, panic);
            let result = catch_unwind(AssertUnwindSafe(|| {
                custody.prepare_in_place(
                    &mut memory,
                    &programs,
                    DispatchGenerationOwnerV1::new(),
                    PersistentFixedDispatchControlStateV1::Ordinary,
                )
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(custody.take_completed().is_err());
            assert_custody(&memory, &custody);
            let before = memory.observation();
            assert!(
                custody
                    .prepare_in_place(
                        &mut memory,
                        &programs,
                        DispatchGenerationOwnerV1::new(),
                        PersistentFixedDispatchControlStateV1::Ordinary
                    )
                    .is_err()
            );
            assert_eq!(memory.observation(), before);
        }
    }
}
