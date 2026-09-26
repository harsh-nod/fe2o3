use super::*;
use std::{cell::Cell, rc::Rc};

#[test]
fn exact_and_one_short_work_storage_preserve_original_floor() {
    let bytes = encoded();
    for decoding in [false, true] {
        let run = |budget: &mut Budget<'_>| -> Result<(), E> {
            if decoding {
                with_decoded_native_conditional_source_packet_v2(&bytes, budget, |_, _| ())
            } else {
                fixture(|input| encode_native_conditional_source_packet_v2(input, budget).map(drop))
            }
        };
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        run(&mut budget).unwrap();
        let (cost, peak) = (budget.work(), budget.peak_storage());
        for (work_limit, storage_limit, denial) in
            [(cost, peak, 0), (cost - 1, peak, 1), (cost, peak - 1, 2)]
        {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let result = run(&mut budget);
            match denial {
                0 => {
                    result.unwrap();
                    assert_eq!((budget.work(), budget.peak_storage()), (cost, peak));
                }
                1 => {
                    assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                    assert!(budget.failed_work().is_some());
                }
                _ => {
                    assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
                    assert!(budget.failed_storage().is_some());
                }
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn success_error_and_unwind_preserve_callback_reservations_and_first_denials() {
    let bytes = encoded();
    for exit in 0..3 {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let denials = (budget.failed_work(), budget.failed_storage());
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_decoded_native_conditional_source_packet_v2(&bytes, &mut budget, |_, b| {
                // Component-only reservation owned by the callback, not codec.
                b.reserve_storage(7).unwrap();
                b.charge_work(13).unwrap();
                match exit {
                    0 => Ok(()),
                    1 => Err("consumer"),
                    _ => panic!("consumer unwind"),
                }
            })
        }));
        match exit {
            0 => result.unwrap().unwrap().unwrap(),
            1 => assert_eq!(result.unwrap().unwrap(), Err("consumer")),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), FLOOR + 7);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
        assert!(budget.work() > 30);
    }
}

struct Provisional(Rc<Cell<usize>>);
impl Drop for Provisional {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn foreign_account_or_damaged_floor_drops_provisional_result_without_refund() {
    let bytes = encoded();
    for foreign in [false, true] {
        for unwind in [false, true] {
            let drops = Rc::new(Cell::new(0));
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let account = budget.work_ledger_identity_v1();
            let mut after = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_decoded_native_conditional_source_packet_v2(&bytes, &mut budget, |_, b| {
                    let result = Provisional(drops.clone());
                    if foreign {
                        let floor = b.storage();
                        // A fixed two test meters are leaked to meet the callback's
                        // arbitrary work lifetime without introducing unsafe code.
                        *b = Budget::new(Box::leak(Box::new(Work::new(usize::MAX))), usize::MAX);
                        b.reserve_storage(floor).unwrap();
                        b.charge_work(11).unwrap();
                    } else {
                        b.release_storage(1).unwrap();
                    }
                    after = Some((b.work(), b.storage()));
                    if unwind {
                        panic!("postcheck damage");
                    }
                    result
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(E::Resource(Resource::Accounting))
                ));
            }
            assert_eq!(drops.get(), 1);
            assert_eq!(Some((budget.work(), budget.storage())), after);
            assert_eq!(budget.work_ledger_identity_v1() == account, !foreign);
        }
    }
}

#[test]
fn callback_owned_allocation_survives_codec_refund_until_caller_drops_it() {
    let bytes = encoded();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let output = with_decoded_native_conditional_source_packet_v2(&bytes, &mut budget, |_, b| {
        b.reserve_storage(7).unwrap();
        vec![5u8; 7]
    })
    .unwrap();
    assert_eq!(output, [5; 7]);
    assert_eq!(budget.storage(), FLOOR + 7);
    drop(output);
    budget.release_storage(7).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn metadata_capacity_is_prepaid_and_arithmetic_refuses_before_allocation() {
    let bytes = encoded();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    scoped(&mut budget, |s| {
        let packet = decode::decode(&bytes, s)?;
        let expected = size_of::<decode::Packet<'_>>()
            + size_of::<Vec<u8>>()
            + packet.order.capacity() * size_of::<u32>()
            + packet.order.len()
            + packet.roots.capacity() * size_of::<decode::Root<'_>>()
            + packet
                .roots
                .iter()
                .map(|root| {
                    let view = root.view();
                    view.staging_commitments.len() * size_of::<Staging>()
                        + view.effect_receipts.len() * size_of::<Signature>()
                })
                .sum::<usize>();
        assert_eq!(s.budget.storage(), expected);
        assert_eq!(*s.reserved, expected);
        drop(packet);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 0);
    assert!(matches!(
        scoped(&mut budget, |s| s.vector::<u128>(usize::MAX)),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        scoped(&mut budget, |s| s.vector::<()>(1)),
        Err(E::Resource(Resource::Allocation))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn outer_scope_rejects_and_drops_success_payload_after_failed_postcheck() {
    let drops = Rc::new(Cell::new(0));
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let result = scoped(&mut budget, |s| {
        s.reserve(11)?;
        s.budget.release_storage(1)?;
        Ok(Provisional(drops.clone()))
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR + 10);
}

#[test]
fn rejected_frames_preserve_prior_work_and_storage_denials() {
    let mut bytes = encoded();
    bytes[16] = 1;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    budget.charge_work(17).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    assert!(budget.charge_work(usize::MAX).is_err());
    let denials = (budget.failed_work(), budget.failed_storage());
    assert!(
        with_decoded_native_conditional_source_packet_v2(&bytes, &mut budget, |_, _| {
            panic!("malformed frame exposed")
        })
        .is_err()
    );
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work() > 17);
    assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
}
