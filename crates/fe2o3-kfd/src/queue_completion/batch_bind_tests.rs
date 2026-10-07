mod batch_bind_tests {
    use super::super::super::super::*;
    use super::*;
    use crate::topology::tests::{AllocationCounter, count_allocations_for_test};
    use fe2o3_aql::{AqlAddressObservationError, AqlPacketBatchPublicationTargetV1};

    #[derive(Debug, Eq, PartialEq)]
    enum Write {
        Body(u32, [u8; 64]),
        Header(u32, u16),
    }

    #[derive(Default)]
    struct Capture(Vec<Write>);

    impl AqlPacketBatchPublicationTargetV1 for Capture {
        type Error = core::convert::Infallible;

        fn write_unpublished(
            &mut self,
            index: u32,
            packet: &AqlKernelDispatchPacketV1,
        ) -> Result<(), Self::Error> {
            self.0
                .push(Write::Body(index, packet.encode_unpublished_le()));
            Ok(())
        }

        fn publish_release_header(&mut self, index: u32, header: u16) -> Result<(), Self::Error> {
            self.0.push(Write::Header(index, header));
            Ok(())
        }
    }

    fn distinct_templates() -> [CompletionPacketTemplateV1; 3] {
        let grids = [[64, 1, 1], [32, 4, 1], [16, 4, 2]];
        let workgroups = [[8, 1, 1], [16, 2, 1], [8, 2, 2]];
        std::array::from_fn(|i| {
            let mut value = template();
            value.geometry = AqlDispatchGeometryV1::new(grids[i], workgroups[i]).unwrap();
            value.ordering = if i == 1 {
                AqlDispatchOrderingV1::WaitForPrior
            } else {
                AqlDispatchOrderingV1::Independent
            };
            value.private_segment_size = [3, 5, 7][i];
            value.group_segment_size = [16, 32, 64][i];
            value.kernel_object = ObservedGpuAddressV1::new(0x400000 + i as u64 * 64).unwrap();
            value.kernarg_address = ObservedGpuAddressV1::new(0x500000 + i as u64 * 16).unwrap();
            value.generations.code = mapping(50 + i as u64 * 2);
            value.generations.kernarg = mapping(51 + i as u64 * 2);
            value.generations.dispatch_generation = 100 + i as u64;
            value
        })
    }

    fn expected_packet(index: usize) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[0..2].copy_from_slice(&1_u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&[1_u16, 2, 3][index].to_le_bytes());
        for (axis, value) in [[8_u16, 1, 1], [16, 2, 1], [8, 2, 2]][index]
            .iter()
            .enumerate()
        {
            bytes[4 + axis * 2..6 + axis * 2].copy_from_slice(&value.to_le_bytes());
        }
        for (axis, value) in [[64_u32, 1, 1], [32, 4, 1], [16, 4, 2]][index]
            .iter()
            .enumerate()
        {
            bytes[12 + axis * 4..16 + axis * 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[24..28].copy_from_slice(&[3_u32, 5, 7][index].to_le_bytes());
        bytes[28..32].copy_from_slice(&[16_u32, 32, 64][index].to_le_bytes());
        bytes[32..40].copy_from_slice(&[0x400000_u64, 0x400040, 0x400080][index].to_le_bytes());
        bytes[40..48].copy_from_slice(&[0x500000_u64, 0x500010, 0x500020][index].to_le_bytes());
        bytes[56..64].copy_from_slice(&[0x1000_u64, 0x1040, 0x80000][index].to_le_bytes());
        bytes
    }

    fn restrict_available(owner: &mut CompletionSignalArenaOwnerV1, selected: &[usize]) {
        for (index, slot) in owner.slots.iter_mut().enumerate() {
            if slot.phase == CompletionSlotPhaseV1::Available && !selected.contains(&index) {
                slot.phase = CompletionSlotPhaseV1::Bound { batch_id: u64::MAX };
            }
        }
    }

    fn restore_available(owner: &mut CompletionSignalArenaOwnerV1) {
        for slot in owner.slots.iter_mut() {
            if slot.phase == (CompletionSlotPhaseV1::Bound { batch_id: u64::MAX }) {
                slot.phase = CompletionSlotPhaseV1::Available;
            }
        }
    }

    #[test]
    fn sparse_batch_preserves_packet_roster_association_and_all_neighbor_custody() {
        let AliasedRoster {
            mut owner,
            neighbor,
            neighbor_event,
            batch,
            retained,
        } = aliased_roster();
        let selected = [63, 64, 8191];
        restrict_available(&mut owner, &selected);
        for (index, generation) in selected.into_iter().zip([7, 13, 29]) {
            owner.slots[index].generation = generation;
        }
        let mut expected = snapshot(&owner);
        for index in selected {
            expected.custody.slots[index].phase = CompletionSlotPhaseV1::Bound {
                batch_id: expected.custody.next_batch_id,
            };
        }
        expected.custody.next_batch_id += 1;
        let templates = distinct_templates();
        let (packets, retention) = owner.bind_batch(templates).unwrap().into_parts();
        assert_eq!(snapshot(&owner), expected);
        assert_eq!(retention.slots.map(|slot| slot.index), [63, 64, 8191]);
        assert_eq!(retention.slots.map(|slot| slot.generation), [7, 13, 29]);
        assert_eq!(
            *retention.dispatches,
            templates.map(|value| value.generations)
        );
        assert_eq!(retention.batch_id, expected.custody.next_batch_id - 1);
        assert_eq!(retention.queue, owner.queue);
        assert_eq!(retention.signal_mapping, owner.signal_mapping);
        assert_eq!(retention.last_packet_id, None);
        let mut capture = Capture::default();
        packets.publish_with(&mut capture).unwrap();
        assert_eq!(
            capture.0,
            [
                Write::Body(0, expected_packet(0)),
                Write::Body(1, expected_packet(1)),
                Write::Body(2, expected_packet(2)),
                Write::Header(0, 0x1402),
                Write::Header(1, 0x1502),
                Write::Header(2, 0x1402),
            ]
        );
        owner.cancel_bound_retaining(retention).unwrap();
        restore_available(&mut owner);
        owner
            .release_compute_dependency_reader_event_batch(retained)
            .unwrap();
        finish_aliases(&mut owner, neighbor, neighbor_event, batch);
    }

    #[test]
    fn late_batch_refusals_preserve_full_state_and_do_not_burn_slots_or_ids() {
        for fault in 0..7 {
            let mut owner = owner();
            let (neighbor_batch, neighbor) = published(&mut owner);
            let (neighbor, reader) = owner
                .retain_compute_dependency_reader(neighbor, SESSION, DEPENDENT_EPOCH)
                .unwrap();
            let mut templates = distinct_templates();
            let error = match fault {
                0 => {
                    templates[2].generations.queue.generation.0 += 1;
                    Gfx942CompletionErrorV1::WrongQueueGeneration
                }
                1 | 2 => {
                    let key = if fault == 1 {
                        &mut templates[2].generations.code
                    } else {
                        &mut templates[2].generations.kernarg
                    };
                    key.allocation.vm.id.0 += 1;
                    Gfx942CompletionErrorV1::WrongVmGeneration
                }
                3 => {
                    templates[2].generations.dispatch_generation = 0;
                    Gfx942CompletionErrorV1::WrongVmGeneration
                }
                4 => {
                    templates[2].kernarg_alignment = 3;
                    Gfx942CompletionErrorV1::PacketBinding(AqlDispatchPacketError::Kernarg(
                        AqlAddressObservationError::InvalidRequiredAlignment,
                    ))
                }
                5 => {
                    templates[2].kernel_object = ObservedGpuAddressV1::new(65).unwrap();
                    Gfx942CompletionErrorV1::PacketBinding(AqlDispatchPacketError::KernelObject(
                        AqlAddressObservationError::Misaligned,
                    ))
                }
                _ => {
                    templates[0].kernarg_alignment = 3;
                    templates[1].generations.queue.generation.0 += 1;
                    Gfx942CompletionErrorV1::PacketBinding(AqlDispatchPacketError::Kernarg(
                        AqlAddressObservationError::InvalidRequiredAlignment,
                    ))
                }
            };
            let before = snapshot(&owner);
            assert_eq!(owner.bind_batch(templates).unwrap_err(), error);
            assert_eq!(snapshot(&owner), before);
            let (_, retention) = owner.bind_batch(distinct_templates()).unwrap().into_parts();
            assert_eq!(retention.batch_id, before.custody.next_batch_id);
            assert_eq!(retention.slots.map(|slot| slot.index), [1, 2, 3]);
            owner.cancel_bound_retaining(retention).unwrap();
            owner.release_compute_dependency_reader(reader).unwrap();
            owner.release_compute_event(neighbor).unwrap();
            finish(&mut owner, neighbor_batch);
        }
    }

    #[test]
    fn batch_identity_exhaustion_precedes_selection_and_packet_preparation() {
        let mut owner = owner();
        owner.next_batch_id = u64::MAX - 1;
        let (_, retention) = owner.bind_batch(distinct_templates()).unwrap().into_parts();
        assert_eq!(retention.batch_id, u64::MAX - 1);
        assert_eq!(owner.next_batch_id, u64::MAX);
        owner.cancel_bound_retaining(retention).unwrap();
        restrict_available(&mut owner, &[]);
        let mut templates = distinct_templates();
        templates[0].kernarg_alignment = 3;
        let templates = CompletionPacketTemplatesV1::from_array(templates);
        let before = snapshot(&owner);
        let (result, calls) = count_allocations_for_test(|| owner.bind_fixed_batch(templates));
        assert_eq!(
            result.unwrap_err(),
            Gfx942CompletionErrorV1::BatchIdentityExhausted
        );
        assert_eq!(calls, 0);
        assert_eq!(snapshot(&owner), before);
        restore_available(&mut owner);
        owner.ensure_releasable().unwrap();
    }

    fn bind_counting_commit<const N: usize>(
        owner: &mut CompletionSignalArenaOwnerV1,
        templates: CompletionPacketTemplatesV1<N>,
        counter: &mut Option<AllocationCounter>,
    ) -> Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        completion_bind_batch_body!(@annotated completion_rust_expr, owner, templates, N,
            next_id, slots, prepared, index, dispatches, packets, bound, [], [], [], [], [], [],
            [*counter = Some(AllocationCounter::begin());])
    }

    fn check_commit<const N: usize>() {
        let mut owner = owner();
        let templates =
            CompletionPacketTemplatesV1::<N>::try_from_vec(vec![template(); N]).unwrap();
        let mut counter = None;
        let result = bind_counting_commit(&mut owner, templates, &mut counter);
        let allocations = counter.unwrap().finish();
        assert_eq!(allocations, 0, "commit at depth {N}");
        let (_, retention) = result.unwrap().into_parts();
        owner.cancel_bound_retaining(retention).unwrap();
        owner.ensure_releasable().unwrap();
    }

    #[test]
    fn batch_commit_is_allocation_free_through_return_at_all_reviewed_depths() {
        check_commit::<1>();
        check_commit::<3>();
        check_commit::<64>();
        check_commit::<8192>();
    }

    #[test]
    fn selection_preserves_generation_and_pin_metadata_without_new_admission_rules() {
        let mut owner = owner();
        restrict_available(&mut owner, &[63, 64, 8191]);
        for (index, generation) in [63, 64, 8191].into_iter().zip([0, u64::MAX, 7]) {
            owner.slots[index].generation = generation;
            owner.slots[index].event_pins = 9;
            owner.slots[index].native_reader_pins = 11;
        }
        let mut expected = snapshot(&owner);
        for index in [63, 64, 8191] {
            expected.custody.slots[index].phase = CompletionSlotPhaseV1::Bound {
                batch_id: expected.custody.next_batch_id,
            };
        }
        expected.custody.next_batch_id += 1;
        let (_, retention) = owner.bind_batch(distinct_templates()).unwrap().into_parts();
        assert_eq!(
            retention.slots.map(|slot| slot.generation),
            [0, u64::MAX, 7]
        );
        assert_eq!(snapshot(&owner), expected);
        for index in [63, 64, 8191] {
            owner.slots[index].event_pins = 0;
            owner.slots[index].native_reader_pins = 0;
        }
        owner.cancel_bound_retaining(retention).unwrap();
        restore_available(&mut owner);
        owner.ensure_releasable().unwrap();
    }

    #[test]
    fn insufficient_signals_precedes_invalid_templates_and_preserves_state() {
        let mut owner = owner();
        restrict_available(&mut owner, &[64]);
        let mut templates = distinct_templates();
        templates[0].generations.queue.generation.0 += 1;
        let before = snapshot(&owner);
        assert_eq!(
            owner.bind_batch(templates).unwrap_err(),
            Gfx942CompletionErrorV1::InsufficientSignals
        );
        assert_eq!(snapshot(&owner), before);
        restore_available(&mut owner);
        owner.ensure_releasable().unwrap();
    }

    #[test]
    fn arena_address_errors_are_ordered_and_do_not_partially_bind() {
        for (base, expected) in [
            (
                u64::MAX - 127,
                Gfx942CompletionErrorV1::InvalidArena("completion slot address"),
            ),
            (
                0,
                Gfx942CompletionErrorV1::InvalidArena("completion address"),
            ),
            (
                1,
                Gfx942CompletionErrorV1::PacketBinding(AqlDispatchPacketError::CompletionSignal(
                    AqlAddressObservationError::Misaligned,
                )),
            ),
        ] {
            let mut owner = owner();
            owner.gpu_base = base;
            let before = snapshot(&owner);
            assert_eq!(
                owner.bind_batch(distinct_templates()).unwrap_err(),
                expected
            );
            assert_eq!(snapshot(&owner), before);
            // A row's generation checks precede that same row's address checks.
            let mut templates = distinct_templates();
            templates[0].generations.queue.generation.0 += 1;
            assert_eq!(
                owner.bind_batch(templates).unwrap_err(),
                Gfx942CompletionErrorV1::WrongQueueGeneration
            );
            assert_eq!(snapshot(&owner), before);
        }
    }

    #[test]
    fn readiness_precedes_invalid_count_and_selection_stops_at_requested_count() {
        for phase in [
            CompletionOwnerPhaseV1::Poisoned,
            CompletionOwnerPhaseV1::ProbeActive,
        ] {
            let mut owner = owner();
            owner.phase = phase;
            let before = snapshot(&owner);
            assert_eq!(
                owner.bind_batch([]).unwrap_err(),
                Gfx942CompletionErrorV1::Poisoned
            );
            assert_eq!(snapshot(&owner), before);
        }
        let mut owner = owner();
        restrict_available(&mut owner, &[63, 64, 8190, 8191]);
        let (_, retention) = owner.bind_batch(distinct_templates()).unwrap().into_parts();
        assert_eq!(retention.slots.map(|slot| slot.index), [63, 64, 8190]);
        assert_eq!(owner.slots[8191].phase, CompletionSlotPhaseV1::Available);
        owner.cancel_bound_retaining(retention).unwrap();
        restore_available(&mut owner);
        owner.ensure_releasable().unwrap();
    }
}
