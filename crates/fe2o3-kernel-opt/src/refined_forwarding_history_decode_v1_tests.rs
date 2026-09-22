use super::*;
use crate::materialize_refined_forwarding_history_v1 as materialize;

#[test]
fn history_final_graph_transfer_preserves_actual_allocation_and_admission_receipt() {
    for nonempty in [false, true] {
        let (wire, expected) = with_history(nonempty, |inputs, floor| {
            (
                encode(inputs, floor),
                inputs.output.canonical().canonical_bytes().to_vec(),
            )
        });
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        let (independent, expected_storage) =
            Owner::from_canonical_bytes_with_verification_budget_v12(&expected, &mut b).unwrap();
        drop(independent);
        let wire_storage = wire.storage().retained_storage();
        b.reserve_storage(37 + wire_storage).unwrap();
        assert!(b.reserve_storage(usize::MAX).is_err());
        let frame = read_refined_forwarding_history_v1(wire.canonical_bytes(), &mut b).unwrap();
        let frame_storage = frame.storage().retained_storage();
        b.reserve_storage(frame_storage).unwrap();
        let decoded = materialize(&frame, &mut b).unwrap();
        let history_storage = decoded.storage().retained_storage();
        b.reserve_storage(history_storage).unwrap();
        let checked = decoded.check_semantics(&mut b).unwrap();
        let checked_storage = checked.storage().retained_storage();
        b.reserve_storage(checked_storage).unwrap();
        let pointer = checked.output().canonical().canonical_bytes().as_ptr();
        drop(checked);
        b.release_storage(checked_storage).unwrap();
        let before = (b.work(), b.storage(), b.peak_storage(), b.failed_storage());
        let (output, storage) = decoded.into_final_graph();
        assert_eq!(
            (b.work(), b.storage(), b.peak_storage(), b.failed_storage()),
            before
        );
        assert_eq!(
            storage.retained_storage(),
            expected_storage.retained_storage()
        );
        b.release_storage(
            history_storage
                .checked_sub(storage.retained_storage())
                .unwrap(),
        )
        .unwrap();
        drop(frame);
        b.release_storage(frame_storage).unwrap();
        drop(wire);
        b.release_storage(wire_storage).unwrap();
        assert_eq!(output.canonical().canonical_bytes().as_ptr(), pointer);
        assert_eq!(output.canonical().canonical_bytes(), expected);
        assert_eq!(b.storage(), 37 + storage.retained_storage());
        drop(output);
        b.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(b.storage(), 37);
        assert_eq!(b.failed_storage(), Some(usize::MAX));
    }
}

#[test]
fn history_decoded_all_twelve_actual_owners_replay_full_semantics() {
    for nonempty in [false, true] {
        with_history(nonempty, |inputs, floor| {
            let wire = encode(inputs, floor);
            let wire_storage = wire.storage().retained_storage();
            let mut work = Work::new(WORK);
            let mut b = Budget::new(&mut work, STORAGE);
            b.reserve_storage(floor + wire_storage).unwrap();
            let inherited = b.storage();
            let ledger = b.work_ledger_identity_v1();
            b.charge_work(11).unwrap();
            let frame_storage;
            {
                let frame =
                    read_refined_forwarding_history_v1(wire.canonical_bytes(), &mut b).unwrap();
                frame_storage = frame.storage().retained_storage();
                b.reserve_storage(frame_storage).unwrap();
                let decoded = materialize(&frame, &mut b).unwrap();
                let retained = decoded.storage().retained_storage();
                b.reserve_storage(retained).unwrap();
                assert!(!decoded.grants_authority());
                assert!(!decoded.authenticates_execution());
                let roles = [
                    RefinedForwardingHistoryRoleV1::B,
                    RefinedForwardingHistoryRoleV1::C,
                    RefinedForwardingHistoryRoleV1::S,
                    RefinedForwardingHistoryRoleV1::O,
                    RefinedForwardingHistoryRoleV1::I,
                    RefinedForwardingHistoryRoleV1::J,
                    RefinedForwardingHistoryRoleV1::K,
                    RefinedForwardingHistoryRoleV1::P,
                    RefinedForwardingHistoryRoleV1::H,
                    RefinedForwardingHistoryRoleV1::L,
                    RefinedForwardingHistoryRoleV1::R,
                    RefinedForwardingHistoryRoleV1::F,
                ];
                for (i, role) in roles.into_iter().enumerate() {
                    assert_eq!(
                        decoded.graph(role).canonical().canonical_bytes(),
                        frame.fields[i]
                    );
                    for prior in &roles[..i] {
                        assert!(!std::ptr::eq(decoded.graph(role), decoded.graph(*prior)));
                    }
                }
                let receipt = decoded.check_semantics(&mut b).unwrap();
                let receipt_storage = receipt.storage().retained_storage();
                b.reserve_storage(receipt_storage).unwrap();
                assert!(std::ptr::eq(
                    receipt.output(),
                    decoded.graph(RefinedForwardingHistoryRoleV1::F)
                ));
                assert!(!receipt.authenticates_execution());
                assert!(!receipt.grants_authority());
                receipt.replay(frame.limits(), &mut b).unwrap();
                drop(receipt);
                b.release_storage(receipt_storage).unwrap();
                drop(decoded);
                b.release_storage(retained).unwrap();
            }
            b.release_storage(frame_storage).unwrap();
            assert_eq!(b.storage(), inherited);
            assert!(b.work_ledger_identity_v1() == ledger);
        });
    }
}

#[test]
fn history_decoded_every_role_uses_actual_canonical_decoder() {
    with_history(false, |inputs, floor| {
        let wire = encode(inputs, floor);
        let frame = read(wire.canonical_bytes()).unwrap();
        for i in 0..12 {
            let mut graph = frame.fields[i].to_vec();
            graph[0] ^= 128;
            let mut fields = frame.fields;
            fields[i] = &graph;
            let bytes = raw(&fields);
            let mut w = Work::new(WORK);
            let mut b = Budget::new(&mut w, STORAGE);
            b.reserve_storage(floor + bytes.capacity()).unwrap();
            let framed = read_refined_forwarding_history_v1(&bytes, &mut b).unwrap();
            b.reserve_storage(framed.storage().retained_storage())
                .unwrap();
            let inherited = b.storage();
            assert!(
                matches!(materialize(&framed, &mut b), Err(Error::Admission { role, .. }) if role as usize == i)
            );
            assert_eq!(b.storage(), inherited);
        }
    });
}

#[test]
fn history_decoded_preserves_required_p7_decoder_and_j_k_identity_refusals() {
    with_history(false, |inputs, floor| {
        let wire = encode(inputs, floor);
        let frame = read(wire.canonical_bytes()).unwrap();
        for kind in 0..3 {
            let slot = if kind == 0 { 18 } else { 19 };
            let mut changed = frame.fields[slot].to_vec();
            changed[if kind == 0 { 0 } else { 44 + (kind - 1) * 40 }] ^= 128;
            let mut fields = frame.fields;
            fields[slot] = &changed;
            let bytes = raw(&fields);
            let mut w = Work::new(WORK);
            let mut b = Budget::new(&mut w, STORAGE);
            b.reserve_storage(floor + bytes.capacity()).unwrap();
            let framed = read_refined_forwarding_history_v1(&bytes, &mut b).unwrap();
            b.reserve_storage(framed.storage().retained_storage())
                .unwrap();
            let inherited = b.storage();
            let result = materialize(&framed, &mut b);
            assert!(match result {
                Err(Error::Policy7(_)) => kind == 0,
                Err(Error::TailIdentity) => kind != 0,
                _ => false,
            });
            assert_eq!(b.storage(), inherited);
        }
    });
}

#[test]
fn history_decoded_framed_foreign_graph_and_missing_origins_fail_semantic_replay() {
    with_history(true, |inputs, floor| {
        let wire = encode(inputs, floor);
        let frame = read(wire.canonical_bytes()).unwrap();
        let mut prep_work = Work::new(WORK);
        let mut prep = Budget::new(&mut prep_work, STORAGE);
        let (foreign, _) = Owner::from_module_ref_with_verification_budget_v12(
            &Module::new("foreign-final"),
            &mut prep,
        )
        .unwrap();
        for kind in 0..2 {
            let mut fields = frame.fields;
            if kind == 0 {
                fields[11] = foreign.canonical().canonical_bytes();
            } else {
                fields[25] = &[0; 4];
            }
            let bytes = raw(&fields);
            let mut w = Work::new(WORK);
            let mut b = Budget::new(&mut w, STORAGE);
            b.reserve_storage(floor + bytes.capacity()).unwrap();
            let framed = read_refined_forwarding_history_v1(&bytes, &mut b).unwrap();
            b.reserve_storage(framed.storage().retained_storage())
                .unwrap();
            let decoded = materialize(&framed, &mut b).unwrap();
            let storage = decoded.storage().retained_storage();
            b.reserve_storage(storage).unwrap();
            let inherited = b.storage();
            assert!(matches!(
                decoded.check_semantics(&mut b),
                Err(CanonicalRefinedForwardingHistoryErrorV1::Forwarding(_))
            ));
            assert_eq!(b.storage(), inherited);
            drop(decoded);
            b.release_storage(storage).unwrap();
        }
    });
}

struct Observation {
    work: usize,
    peak: usize,
    denied_work: Option<usize>,
    denied_storage: Option<usize>,
}
fn decode_run(
    frame: &InertRefinedForwardingHistoryRefV1<'_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<usize, Error>, Observation) {
    let mut work = Work::new(work_limit);
    let (result, used, peak, denied_storage) = {
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        b.charge_work(11).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = materialize(frame, &mut b).map(|o| o.storage().retained_storage());
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        (result, b.work(), b.peak_storage(), b.failed_storage())
    };
    (
        result,
        Observation {
            work: used,
            peak,
            denied_work: work.failed_work(),
            denied_storage,
        },
    )
}

#[test]
fn history_decoded_exact_measured_work_peak_and_first_reservation_refusal() {
    with_history(true, |inputs, floor| {
        let wire = encode(inputs, floor);
        let frame = read(wire.canonical_bytes()).unwrap();
        let floor =
            floor + wire.storage().retained_storage() + frame.storage().retained_storage() + 37;
        let (result, measured) = decode_run(&frame, floor, WORK, STORAGE);
        let retained = result.unwrap();
        assert!(measured.work > 11 && measured.peak > floor + retained);
        for _ in 0..2 {
            let (result, exact) = decode_run(&frame, floor, measured.work, measured.peak);
            assert_eq!(result.unwrap(), retained);
            assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
            assert_eq!((exact.denied_work, exact.denied_storage), (None, None));
        }
        let (result, denied) = decode_run(&frame, floor, measured.work - 1, measured.peak);
        match result {
            Err(Error::Resource(Resource::Work(e))) => {
                assert_eq!(e.limit(), measured.work - 1);
                assert_eq!(denied.denied_work, Some(e.actual()));
                assert_eq!(e.actual(), measured.work);
                assert_eq!(denied.work, measured.work - 1);
            }
            other => panic!("final debit: {other:?}"),
        }
        let (result, denied) = decode_run(&frame, floor, WORK, floor);
        match result {
            Err(Error::Resource(Resource::Storage(e))) => {
                assert_eq!(e.limit(), floor);
                assert!(e.actual() > floor);
                assert_eq!(denied.denied_storage, Some(e.actual()));
                assert_eq!(
                    (denied.work, denied.peak, denied.denied_work),
                    (11, floor, None)
                );
            }
            other => panic!("first reservation: {other:?}"),
        }
    });
}

#[test]
fn history_wire_reader_and_encoder_preserve_existing_ledger_and_first_denials() {
    with_history(true, |inputs, floor| {
        let wire = encode(inputs, floor);
        let floor = floor + wire.storage().retained_storage();
        let mut original = Work::new(11);
        let mut b = Budget::new(&mut original, STORAGE);
        b.reserve_storage(floor).unwrap();
        b.charge_work(11).unwrap();
        let ledger = b.work_ledger_identity_v1();
        assert!(matches!(
            encode_refined_forwarding_history_v1(inputs, &mut b),
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), 11);
        assert!(b.work_ledger_identity_v1() == ledger);
        let mut original = Work::new(WORK);
        let mut b = Budget::new(&mut original, floor);
        b.reserve_storage(floor).unwrap();
        assert!(
            matches!(read_refined_forwarding_history_v1(wire.canonical_bytes(), &mut b), Err(Error::Resource(Resource::Storage(e))) if e.limit() == floor && e.actual() > floor)
        );
        assert_eq!((b.work(), b.storage(), b.peak_storage()), (0, floor, floor));
        let mut original = Work::new(WORK);
        let mut b = Budget::new(&mut original, STORAGE + 1);
        assert!(matches!(
            read_refined_forwarding_history_v1(wire.canonical_bytes(), &mut b),
            Err(Error::Limit)
        ));
        assert_eq!((b.work(), b.storage(), b.peak_storage()), (0, 0, 0));
    });
}

#[test]
fn history_new_scope_unwind_drops_typed_backing_before_refund() {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(53).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let result: Result<(), Error> = resources::scoped(&mut b, |meter| {
        meter.reserve(std::mem::size_of::<Vec<Site>>())?;
        let (mut values, _) = meter.table::<Site>(3)?;
        meter.push(&mut values, site(0))?;
        meter.work(7)?;
        panic!("new typed-row scope unwind")
    });
    assert!(matches!(result, Err(Error::Panicked)));
    assert_eq!(b.storage(), 53);
    assert_eq!(b.work(), 12);
    assert!(b.peak_storage() > 53);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(b.failed_storage(), None);
}

#[test]
fn history_live_wire_and_frame_numeric_floors_are_necessary_not_authentication() {
    with_history(false, |inputs, floor| {
        let wire = encode(inputs, floor);
        let frame = read(wire.canonical_bytes()).unwrap();
        let mut w = Work::new(WORK);
        let mut b = Budget::new(&mut w, STORAGE);
        assert!(matches!(
            read_refined_forwarding_history_v1(wire.canonical_bytes(), &mut b),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!((b.work(), b.storage(), b.peak_storage()), (0, 0, 0));
        b.reserve_storage(wire.canonical_bytes().len()).unwrap();
        assert!(matches!(
            materialize(&frame, &mut b),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(b.work(), 0);
        assert_eq!(b.storage(), wire.canonical_bytes().len());
    });
}
