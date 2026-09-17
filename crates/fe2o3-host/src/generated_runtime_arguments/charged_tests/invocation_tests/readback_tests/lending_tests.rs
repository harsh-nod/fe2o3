use super::*;

fn destinations(
    storage: &GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
) -> Vec<(*const u8, usize)> {
    storage
        .readback
        .as_ref()
        .unwrap()
        .buffers()
        .iter()
        .map(|(_, bytes)| (bytes.as_ptr(), bytes.capacity()))
        .collect()
}

#[test]
fn readback_lending_checks_shape_before_read_only_contents() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, mut observer) = mixed(&budget);
    storage.install_readback(storage.prepare_readback().unwrap());
    let original = destinations(&storage);
    let usage = budget.usage();
    assert!(matches!(
        storage.validate_reserved_readback(),
        Err(Error::BindingMismatch)
    ));
    let (source, buffers) = storage.borrow_reserved_readback_v1().unwrap();
    assert_eq!(buffers.len(), 2);
    assert_eq!(buffers[1].0, Gfx942RuntimeBufferAccessV1::ReadOnly);
    assert!(
        buffers
            .iter()
            .all(|(_, bytes)| bytes.iter().all(|byte| *byte == 0))
    );
    for ((_, bytes), input) in buffers.iter_mut().zip(source.buffers()) {
        bytes.copy_from_slice(input.bytes());
    }
    buffers[0].1.fill(7);
    storage.validate_reserved_readback().unwrap();
    assert_eq!(destinations(&storage), original);
    assert_eq!(budget.usage(), usage);
    assert!(observer.try_take().unwrap().is_none());
    storage.readback.as_mut().unwrap().buffers_mut()[1].1[15] ^= 1;
    assert!(matches!(
        storage.validate_reserved_readback(),
        Err(Error::BindingMismatch)
    ));
    assert!(observer.try_take().unwrap().is_none());
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn readback_lending_rejects_complete_shape_and_gate_mutations_without_copy() {
    for mutation in 0..9 {
        let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
        let (mut storage, mut observer) = mixed(&budget);
        storage.install_readback(storage.prepare_readback().unwrap());
        match mutation {
            0 => {
                storage.readback.as_mut().unwrap().buffers_mut().pop();
            }
            1 => {
                storage.readback.as_mut().unwrap().buffers_mut()[1].1.pop();
            }
            2 => storage.readback.as_mut().unwrap().buffers_mut()[1]
                .1
                .reserve_exact(1),
            3 => {
                storage.readback.as_mut().unwrap().buffers_mut()[1].0 =
                    Gfx942RuntimeBufferAccessV1::ReadWrite
            }
            4 => storage.decoder.expectations[1].byte_len += 1,
            5 => storage.decoder.expectations[1].access = Gfx942RuntimeBufferAccessV1::ReadWrite,
            6 => storage.decoder.result_gate = None,
            7 => storage.decoder.expectations[0].custody = None,
            _ => {
                let other_budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
                let (other, _) = mixed(&other_budget);
                storage.decoder.result_gate = other.decoder.result_gate.clone();
            }
        }
        let original = destinations(&storage);
        let usage = budget.usage();
        let result = storage.borrow_reserved_readback_v1();
        assert!(result.is_err(), "mutation {mutation}");
        assert_eq!(destinations(&storage), original);
        assert!(
            storage
                .readback
                .as_ref()
                .unwrap()
                .buffers()
                .iter()
                .all(|(_, bytes)| bytes.iter().all(|byte| *byte == 0))
        );
        assert_eq!(budget.usage(), usage);
        if mutation != 7 {
            assert!(observer.try_take().unwrap().is_none());
        }
        drop(storage);
        assert_empty(&budget);
    }
}

#[test]
fn readback_lending_error_and_unwind_keep_original_owner_and_copied_prefix() {
    for panic in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
        let (mut storage, mut observer) = mixed(&budget);
        storage.install_readback(storage.prepare_readback().unwrap());
        let original = destinations(&storage);
        let usage = budget.usage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (_, buffers) = storage.borrow_reserved_readback_v1().unwrap();
            buffers[0].1[..7].fill(9);
            if panic {
                panic!("readback copy prefix");
            }
            Err::<(), _>("copy error")
        }));
        if panic {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"readback copy prefix")
            );
        } else {
            assert_eq!(result.unwrap(), Err("copy error"));
        }
        let buffers = storage.readback.as_ref().unwrap().buffers();
        assert_eq!(&buffers[0].1[..7], &[9; 7]);
        assert_eq!(&buffers[0].1[7..], &[0; 9]);
        assert_eq!(buffers[1].1, vec![0; 16]);
        assert_eq!(destinations(&storage), original);
        assert_eq!(budget.usage(), usage);
        assert!(observer.try_take().unwrap().is_none());
        drop(storage);
        assert_empty(&budget);
    }
}

#[test]
fn readback_lending_rejects_missing_owner_credit_and_committed_gate() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, mut observer) = mixed(&budget);
    assert!(matches!(
        storage.borrow_reserved_readback_v1(),
        Err(Error::BindingMismatch)
    ));
    storage.install_readback(storage.prepare_readback().unwrap());
    storage
        .readback
        .as_mut()
        .unwrap()
        .quarantine_credit_for_test();
    let usage = budget.usage();
    assert!(matches!(
        storage.borrow_reserved_readback_v1(),
        Err(Error::ResultCredit(ResourceCreditErrorV1::Invariant))
    ));
    assert_eq!(budget.usage(), usage);
    assert!(observer.try_take().unwrap().is_none());
    drop(storage);
    assert_eq!(budget.usage().quarantined_members, 1);

    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, observer) = mixed(&budget);
    storage.install_readback(storage.prepare_readback().unwrap());
    storage.decoder.result_gate.as_ref().unwrap().commit();
    assert!(matches!(
        storage.borrow_reserved_readback_v1(),
        Err(Error::BindingMismatch)
    ));
    drop(storage);
    drop(observer);
    assert_empty(&budget);
}
