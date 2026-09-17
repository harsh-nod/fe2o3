use super::*;
use crate::{RuntimeGfx942GeneratedCompletionViewV1, RuntimeGfx942GeneratedSourceV1};

fn mixed_projection() -> (Vec<u8>, crate::PreparedGfx942PersistentDispatchV1) {
    source_projection_with_access(
        fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        [
            Gfx942RuntimeBufferAccessV1::ReadWrite,
            Gfx942RuntimeBufferAccessV1::ReadOnly,
            Gfx942RuntimeBufferAccessV1::WriteOnly,
        ],
    )
}

fn original_destinations(
    projection: &crate::PreparedGfx942PersistentDispatchV1,
) -> Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)> {
    projection
        .buffers()
        .iter()
        .enumerate()
        .map(|(index, buffer)| {
            (
                projection.buffer_access(index).unwrap(),
                buffer.bytes().to_vec(),
            )
        })
        .collect()
}

fn pointers(destinations: &[(Gfx942RuntimeBufferAccessV1, Vec<u8>)]) -> Vec<(*const u8, usize)> {
    destinations
        .iter()
        .map(|(_, bytes)| (bytes.as_ptr(), bytes.capacity()))
        .collect()
}

#[test]
fn generated_completion_view_keeps_same_authority_and_original_roster() {
    let (hsaco, projection) = mixed_projection();
    let authority = source_authority(&projection);
    let unrelated = source_authority(&projection);
    let source = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority);
    let roster = source.validate(7).unwrap();
    authority.checks.set(0);
    let mut destinations = original_destinations(&projection);
    let addresses = pointers(&destinations);
    let view = RuntimeGfx942GeneratedCompletionViewV1::new(source, &mut destinations);
    let (source, destinations) = view.into_parts();
    source
        .with_current_source_v1(7, &roster, || {
            unrelated.current.set(false);
            assert_eq!(destinations[1].0, Gfx942RuntimeBufferAccessV1::ReadOnly);
            destinations[0].1.fill(0xee);
            destinations[2].1.fill(0xff);
            source.validate_completed_readback_v1(destinations).unwrap();
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(authority.checks.get(), 3);
    assert_eq!(unrelated.checks.get(), 0);
    assert_eq!(pointers(destinations), addresses);
    assert_eq!(destinations[1].1, projection.buffers()[1].bytes());
}

#[test]
fn generated_completion_readback_validates_every_extent_access_and_read_only_byte() {
    let (hsaco, projection) = mixed_projection();
    let authority = source_authority(&projection);
    let source = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority);
    for mutation in 0..12 {
        let mut destinations = original_destinations(&projection);
        match mutation {
            0 => {
                destinations.pop();
            }
            1 => destinations.push((Gfx942RuntimeBufferAccessV1::ReadOnly, vec![1])),
            2..=4 => {
                destinations[mutation - 2].0 = match destinations[mutation - 2].0 {
                    Gfx942RuntimeBufferAccessV1::ReadOnly => Gfx942RuntimeBufferAccessV1::ReadWrite,
                    _ => Gfx942RuntimeBufferAccessV1::ReadOnly,
                }
            }
            5..=7 => {
                destinations[mutation - 5].1.pop();
            }
            8..=10 => destinations[mutation - 8].1.reserve_exact(1),
            _ => destinations[1].1[19] ^= 1,
        }
        assert!(
            source
                .validate_completed_readback_v1(&destinations)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn generated_completion_view_currentness_faults_retain_exact_copied_prefix() {
    for check in 1..=3 {
        for panic in [false, true] {
            let (hsaco, projection) = mixed_projection();
            let authority = source_authority(&projection);
            let source = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority);
            let roster = source.validate(7).unwrap();
            authority.checks.set(0);
            authority.fault.set(Some(if panic {
                CurrentnessFault::PanicAt(check)
            } else {
                CurrentnessFault::ErrorAt(check)
            }));
            let mut destinations = original_destinations(&projection);
            let addresses = pointers(&destinations);
            let called = Cell::new(0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (source, destinations) =
                    RuntimeGfx942GeneratedCompletionViewV1::new(source, &mut destinations)
                        .into_parts();
                source.with_current_source_v1(7, &roster, || {
                    called.set(called.get() + 1);
                    destinations[0].1[..7].fill(9);
                    Ok::<(), ()>(())
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(called.get(), usize::from(check == 3));
            assert_eq!(authority.checks.get(), check);
            assert_eq!(pointers(&destinations), addresses);
            assert_eq!(
                &destinations[0].1[..7],
                if check == 3 { &[9; 7] } else { &[0; 7] }
            );
            assert_eq!(&destinations[0].1[7..], &[0; 9]);
            assert_eq!(destinations[1].1, projection.buffers()[1].bytes());
            assert_eq!(destinations[2].1, projection.buffers()[2].bytes());
        }
    }
}

#[test]
fn generated_completion_view_foreign_source_rejects_before_callback() {
    let (hsaco, projection) = mixed_projection();
    let (_, foreign) = mixed_projection();
    assert_eq!(
        projection.dispatch_contract_sha256(),
        foreign.dispatch_contract_sha256()
    );
    let authority = source_authority(&projection);
    let source = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority);
    let roster = crate::generated_source::GeneratedHostRosterV1::from_projection(&foreign).unwrap();
    let mut destinations = original_destinations(&projection);
    let addresses = pointers(&destinations);
    let (source, destinations) =
        RuntimeGfx942GeneratedCompletionViewV1::new(source, &mut destinations).into_parts();
    assert!(matches!(
        source.with_current_source_v1(7, &roster, || -> Result<(), ()> {
            panic!("foreign source entered")
        }),
        Err(crate::RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster)
    ));
    assert_eq!(pointers(destinations), addresses);
    assert_eq!(authority.checks.get(), 1);
}
