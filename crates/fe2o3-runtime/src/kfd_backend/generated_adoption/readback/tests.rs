use super::*;
use crate::authorized_execution::tests::source_projection;

fn fixture() -> (
    GeneratedHostRosterV1,
    Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
) {
    let (_, projection) = source_projection();
    let mut roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    // Descriptive copy-driver fixture, not compiler or Worker authority.
    for (slot, access) in roster.buffers.iter_mut().zip([
        Gfx942RuntimeBufferAccessV1::ReadOnly,
        Gfx942RuntimeBufferAccessV1::WriteOnly,
        Gfx942RuntimeBufferAccessV1::ReadWrite,
    ]) {
        slot.as_mut().unwrap().access = access;
    }
    let destinations = roster.buffers[..roster.count]
        .iter()
        .map(|slot| {
            let slot = slot.unwrap();
            (slot.access, vec![0xa5; slot.bytes as usize])
        })
        .collect();
    (roster, destinations)
}

#[test]
fn full_roster_copy_preserves_every_destination_owner_and_copies_readonly_bytes() {
    let (roster, mut destinations) = fixture();
    assert_eq!(destinations[0].0, Gfx942RuntimeBufferAccessV1::ReadOnly);
    let pointers: Vec<_> = destinations
        .iter()
        .map(|(_, bytes)| (bytes.as_ptr(), bytes.capacity()))
        .collect();
    let mut calls = Vec::new();
    read_roster_v1(&roster, &mut destinations, |ordinal, bytes| {
        calls.push(ordinal);
        bytes.fill(ordinal as u8);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(calls, (0..roster.count).collect::<Vec<_>>());
    for (ordinal, (_, bytes)) in destinations.iter().enumerate() {
        assert_eq!((bytes.as_ptr(), bytes.capacity()), pointers[ordinal]);
        assert!(bytes.iter().all(|byte| *byte == ordinal as u8));
    }
}

#[test]
fn malformed_full_roster_rejects_before_any_destination_is_written() {
    for fault in 0..11 {
        let (mut roster, mut destinations) = fixture();
        let last = roster.count - 1;
        match fault {
            0 => roster.count = 0,
            1 => roster.count = roster.buffers.len() + 1,
            2 => {
                destinations.pop();
            }
            3 => roster.buffers[last] = None,
            4 => roster.buffers[last].as_mut().unwrap().ordinal = 0,
            5 => roster.buffers[last].as_mut().unwrap().bytes += 1,
            6 => roster.buffers[last].as_mut().unwrap().bytes = 0,
            7 => destinations[last].0 = Gfx942RuntimeBufferAccessV1::ReadOnly,
            8 => {
                destinations[last].1.reserve_exact(1);
            }
            9 => roster.readback_bytes += 1,
            10 => roster.buffers[roster.count] = roster.buffers[0],
            _ => unreachable!(),
        }
        let before = destinations.clone();
        assert_eq!(
            read_roster_v1(&roster, &mut destinations, |_, _| -> Result<(), ()> {
                panic!("no copy before complete validation")
            }),
            Err(ReadbackErrorV1::Roster),
            "fault={fault}"
        );
        assert_eq!(destinations, before);
    }
}

#[test]
fn copy_error_or_unwind_preserves_exact_prefix_suffix_and_original_allocations() {
    for panic in [false, true] {
        for failed in 0..3 {
            let (roster, mut destinations) = fixture();
            assert_eq!(roster.count, 3);
            let pointers: Vec<_> = destinations
                .iter()
                .map(|(_, bytes)| (bytes.as_ptr(), bytes.capacity()))
                .collect();
            let mut calls = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| {
                read_roster_v1(&roster, &mut destinations, |ordinal, bytes| {
                    calls.push(ordinal);
                    bytes.fill(ordinal as u8);
                    if ordinal == failed {
                        if panic {
                            std::panic::panic_any(ordinal);
                        }
                        return Err(ordinal);
                    }
                    Ok(())
                })
            }));
            if panic {
                assert_eq!(*result.unwrap_err().downcast::<usize>().unwrap(), failed);
            } else {
                assert_eq!(result.unwrap(), Err(ReadbackErrorV1::Copy(failed)));
            }
            assert_eq!(calls, (0..=failed).collect::<Vec<_>>());
            for (ordinal, (_, bytes)) in destinations.iter().enumerate() {
                assert_eq!((bytes.as_ptr(), bytes.capacity()), pointers[ordinal]);
                assert!(bytes.iter().all(|byte| *byte
                    == if ordinal <= failed {
                        ordinal as u8
                    } else {
                        0xa5
                    }));
            }
        }
    }
}

#[test]
fn readback_plan_requires_complete_cardinality_and_every_member_extent() {
    let (mut backend, plan) = super::super::tests::shells();
    let (roster, _) = fixture();
    assert!(roster_matches_plan_v1(&plan, &roster));
    for fault in 0..5 {
        let mut changed = roster.clone();
        match fault {
            0 => changed.count -= 1,
            1 => changed.count += 1,
            2 => changed.buffers[2] = None,
            3 => changed.buffers[2].as_mut().unwrap().bytes += 1,
            4 => changed.buffers[2].as_mut().unwrap().ordinal = 0,
            _ => unreachable!(),
        }
        assert!(!roster_matches_plan_v1(&plan, &changed));
    }
    backend.dispose_generated_shells_v1(&plan);
}

#[test]
fn cohort3_readback_borrows_original_member_vectors_and_keeps_partial_failure_prefix() {
    for fail in 0..4 {
        let (roster, originals) = fixture();
        let mut originals: [Vec<DestinationV1>; 3] = originals
            .into_iter()
            .map(|value| vec![value])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let pointers = originals
            .each_ref()
            .map(|v| (v[0].1.as_ptr(), v[0].1.capacity()));
        let [a, b, c] = originals.each_mut();
        let mut calls = Vec::new();
        let result = read_destinations_v1(
            &roster,
            &mut DestinationsV1::Cohort3([a, b, c]),
            |index, bytes| {
                calls.push(index);
                bytes.fill(index as u8);
                if index == fail { Err(index) } else { Ok(()) }
            },
        );
        if fail < 3 {
            assert_eq!(result, Err(ReadbackErrorV1::Copy(fail)));
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(calls, (0..=fail.min(2)).collect::<Vec<_>>());
        for (index, original) in originals.iter().enumerate() {
            assert_eq!(
                (original[0].1.as_ptr(), original[0].1.capacity()),
                pointers[index]
            );
            assert!(
                original[0]
                    .1
                    .iter()
                    .all(|b| *b == if index <= fail { index as u8 } else { 0xa5 })
            );
        }
    }
}

#[test]
fn cohort3_readback_refuses_bad_last_member_before_writing_any_original() {
    for fault in 0..4 {
        let (mut roster, originals) = fixture();
        let mut originals: [Vec<DestinationV1>; 3] = originals
            .into_iter()
            .map(|value| vec![value])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        match fault {
            0 => {
                originals[2].clear();
            }
            1 => originals[2][0].1.reserve_exact(1),
            2 => roster.readback_bytes += 1,
            _ => roster.buffers[2].as_mut().unwrap().ordinal = 1,
        }
        let before = originals.clone();
        let [a, b, c] = originals.each_mut();
        assert_eq!(
            read_destinations_v1(
                &roster,
                &mut DestinationsV1::Cohort3([a, b, c]),
                |_, _| -> Result<(), ()> { panic!("complete roster before copy") }
            ),
            Err(ReadbackErrorV1::Roster)
        );
        assert_eq!(originals, before);
    }
}
