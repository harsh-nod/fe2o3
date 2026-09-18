//! Scoped structural corruptions, with removed real owners kept in local custody.

use super::*;

fn owners(set: &mut Gfx942SdmaQueueSetV1) -> &mut Vec<Gfx942SdmaQueueOwnerV1> {
    let (Gfx942SdmaQueueSetV1::Generic(owners) | Gfx942SdmaQueueSetV1::Striped { owners, .. }) =
        set
    else {
        panic!("owner fixture profile")
    };
    owners
}

pub(crate) fn generic_pending_observation(
    set: &Gfx942SdmaQueueSetV1,
) -> impl std::fmt::Debug + PartialEq {
    let (Gfx942SdmaQueueSetV1::Generic(owners) | Gfx942SdmaQueueSetV1::Striped { owners, .. }) =
        set
    else {
        panic!("owner fixture profile")
    };
    let mapping = |value: &Gfx942XgmiMappedDeviceMemoryV1| {
        (
            value.lease().storage_identity(),
            value.gpu_ids().as_ptr() as usize,
            value.gpu_ids().to_vec(),
            value.mapped_prefix(),
            value.unmapped_prefix(),
            value.is_fully_mapped(),
        )
    };
    owners
        .iter()
        .map(|owner| {
            (
                owner
                    .records
                    .iter()
                    .map(|record| {
                        record.as_ref().map(|r| {
                            (
                                r.directional_persistent,
                                r.generation,
                                r.completion_value,
                                r.fence_header,
                                r.completion_observed,
                                r.source.cleanup_metadata(),
                                r.destination.cleanup_metadata(),
                                r.copy_bytes,
                                r.source_offset,
                                r.destination_offset,
                            )
                        })
                    })
                    .collect::<Vec<_>>(),
                owner
                    .xgmi_records
                    .iter()
                    .map(|record| {
                        record.as_ref().map(|r| {
                            (
                                r.generation,
                                r.completion_value,
                                mapping(&r.source),
                                mapping(&r.destination),
                                r.copy_bytes,
                            )
                        })
                    })
                    .collect::<Vec<_>>(),
                owner
                    .persistent_window_slots
                    .iter()
                    .map(|slot| {
                        slot.as_ref()
                            .map(|s| (s.anchor_slot, s.generation, s.completion_value))
                    })
                    .collect::<Vec<_>>(),
                owner
                    .persistent_window_records
                    .iter()
                    .map(|record| {
                        record.as_ref().map(|r| {
                            (
                                r.request.source.cleanup_metadata(),
                                r.request.destination.cleanup_metadata(),
                                r.request.source_offset,
                                r.request.destination_offset,
                                r.request.copy_bytes,
                                r.packet_count,
                            )
                        })
                    })
                    .collect::<Vec<_>>(),
                owner.uncertain_xgmi_ticket,
            )
        })
        .collect::<Vec<_>>()
}

pub(crate) fn with_generic_corruption(
    set: Gfx942SdmaQueueSetV1,
    case: u8,
    primary_id: u32,
    check: impl FnOnce(Gfx942SdmaQueueSetV1) -> Gfx942SdmaQueueSetV1,
) -> Gfx942SdmaQueueSetV1 {
    assert!(matches!(set, Gfx942SdmaQueueSetV1::Generic(_)));
    with_owner_corruption(set, 0, case, primary_id, check)
}

pub(crate) fn with_owner_corruption(
    mut set: Gfx942SdmaQueueSetV1,
    index: usize,
    case: u8,
    primary_id: u32,
    check: impl FnOnce(Gfx942SdmaQueueSetV1) -> Gfx942SdmaQueueSetV1,
) -> Gfx942SdmaQueueSetV1 {
    // Removed tokens, mappings and vectors stay owned until exact restoration.
    macro_rules! scoped {
        (@restore $field:ident, $original:expr) => {{
            let original = $original;
            set = check(set);
            owners(&mut set)[index].$field = original;
        }};
        ($field:ident, take) => {{
            scoped!(@restore $field, std::mem::take(&mut owners(&mut set)[index].$field));
        }};
        ($field:ident, Some($value:expr)) => {{
            scoped!(@restore $field, owners(&mut set)[index].$field.replace($value));
        }};
        ($field:ident, $replacement:expr) => {{
            scoped!(@restore $field, std::mem::replace(&mut owners(&mut set)[index].$field, $replacement));
        }};
    }
    match case {
        0 => {
            let original = std::mem::take(owners(&mut set));
            set = check(set);
            *owners(&mut set) = original;
        }
        1 => scoped!(engine_index, Some(2)),
        2 => scoped!(queue_id, primary_id),
        3 => scoped!(poisoned, true),
        4 => scoped!(destroyed, true),
        5 => {
            let mut foreign = owners(&mut set)[index].owner;
            foreign.generation.0 += 1;
            scoped!(owner, foreign);
        }
        6 => scoped!(records, take),
        7 => scoped!(xgmi_records, take),
        8 => scoped!(persistent_window_slots, take),
        9 => scoped!(persistent_window_records, take),
        10 => scoped!(ring, take),
        11 => scoped!(control, take),
        12 => scoped!(completions, take),
        13 => scoped!(doorbell, take),
        14 => {
            let owner = &mut owners(&mut set)[index];
            let ticket = Gfx942SdmaCopyTicketV1 {
                owner: owner.owner,
                queue_id: owner.queue_id,
                slot: 0,
                generation: 1,
            };
            scoped!(uncertain_xgmi_ticket, Some(ticket));
        }
        15 => {
            owners(&mut set)[index].persistent_window_slots[0] = Some(PersistentSdmaWindowSlotV1 {
                anchor_slot: 0,
                generation: 1,
                completion_value: 1,
            });
            set = check(set);
            let slot = owners(&mut set)[index].persistent_window_slots[0]
                .take()
                .unwrap();
            assert_eq!(
                (slot.anchor_slot, slot.generation, slot.completion_value),
                (0, 1, 1)
            );
        }
        16 | 18 => {
            let owner = &mut owners(&mut set)[index];
            // Metadata-only payloads exercise pending-presence rejection, not publication.
            let (source, destination) = persistent_sdma_buffers_for_test(owner.owner, 41);
            if case == 16 {
                owner.records[0] = Some(SdmaCopyRecordV1 {
                    directional_persistent: false,
                    generation: 1,
                    completion_value: 1,
                    fence_header: 0,
                    completion_observed: false,
                    source,
                    destination,
                    copy_bytes: 4096,
                    source_offset: 0,
                    destination_offset: 0,
                });
            } else {
                owner.persistent_window_records[0] = Some(PersistentSdmaWindowRecordV1 {
                    request: Gfx942SdmaCopyRequestV1::new(source, 0, destination, 0, 4096),
                    packet_count: 1,
                });
            }
            set = check(set);
            if case == 16 {
                assert!(owners(&mut set)[index].records[0].take().is_some());
            } else {
                assert!(
                    owners(&mut set)[index].persistent_window_records[0]
                        .take()
                        .is_some()
                );
            }
        }
        17 => {
            owners(&mut set)[index].xgmi_records[0] = Some(XgmiSdmaCopyRecordV1 {
                generation: 1,
                completion_value: 1,
                source: crate::shared_memory::xgmi_mapping_for_sdma_test(41),
                destination: crate::shared_memory::xgmi_mapping_for_sdma_test(42),
                copy_bytes: 4096,
            });
            set = check(set);
            assert!(owners(&mut set)[index].xgmi_records[0].take().is_some());
        }
        19 => {
            let duplicate = owners(&mut set)[0].queue_id;
            scoped!(queue_id, duplicate);
        }
        20 => scoped!(engine_index, take),
        21 => scoped!(engine_index, Some((index as u32 + 1) % 2)),
        _ => unreachable!(),
    }
    set
}

pub(crate) fn directional_as_malformed_generic(set: Gfx942SdmaQueueSetV1) -> Gfx942SdmaQueueSetV1 {
    let Gfx942SdmaQueueSetV1::Directional(owners) = set else {
        panic!("directional fixture profile")
    };
    Gfx942SdmaQueueSetV1::Generic(owners)
}
