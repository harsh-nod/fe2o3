//! Pure state-machine controls only. u32 stands for I in tests; these values
//! are not compiler Instances, source-admission proof or actual-source evidence.
use super::*;

fn key() -> Key {
    Key {
        caller: SemanticFunctionIdV1::from_index(0),
        block: 3,
        callee: SemanticCallableIdV1::from_index(1),
    }
}
fn binding() -> CallBinding<u32> {
    CallBinding {
        instance: 19,
        packed: Gfx942CompleteBodyPackedV1::from_words(1, 1, [0x4100, 0, 0, 0], [8, 0, 0, 0])
            .unwrap(),
        registers: [32, 33, 34, 35, 36],
        argument_locals: [1, 2, 3, 4, 5],
        moved_arguments: 1,
    }
}
fn slot() -> OneUse<u32> {
    OneUse {
        key: key(),
        pending: Some(binding()),
    }
}

#[test]
fn one_use_slot_requires_successful_exact_consumption() {
    let mut slot = slot();
    assert!(slot.require_drained().is_err());
    assert_eq!(slot.take(key(), &binding()).unwrap(), binding());
    slot.require_drained().unwrap();
    assert!(slot.take(key(), &binding()).is_err());
}

#[test]
fn each_occurrence_coordinate_is_checked_before_consumption() {
    for changed in [
        Key {
            caller: SemanticFunctionIdV1::from_index(1),
            ..key()
        },
        Key { block: 4, ..key() },
        Key {
            callee: SemanticCallableIdV1::from_index(2),
            ..key()
        },
    ] {
        let mut slot = slot();
        assert!(slot.take(changed, &binding()).is_err());
        assert!(slot.require_drained().is_err());
        assert_eq!(slot.take(key(), &binding()).unwrap(), binding());
    }
}

#[test]
fn instance_words_roles_local_transport_and_move_mode_are_independent_bindings() {
    let original = binding();
    let mut changed = original;
    changed.instance += 1;
    let mut variants = vec![changed];
    changed = original;
    changed.packed =
        Gfx942CompleteBodyPackedV1::from_words(1, 1, [0x4101, 0, 0, 0], [8, 0, 0, 0]).unwrap();
    variants.push(changed);
    changed = original;
    changed.packed =
        Gfx942CompleteBodyPackedV1::from_words(1, 1, [0x4100, 0, 0, 0], [24, 0, 0, 0]).unwrap();
    variants.push(changed);
    changed = original;
    changed.registers.swap(0, 1);
    variants.push(changed);
    changed = original;
    changed.argument_locals.swap(1, 2);
    variants.push(changed);
    changed = original;
    changed.moved_arguments = 3;
    variants.push(changed);
    for changed in variants {
        let mut slot = slot();
        assert!(slot.take(key(), &changed).is_err());
        assert_eq!(slot.take(key(), &original).unwrap(), original);
    }
}

#[test]
fn work_bound_is_monotonic_exact_and_overflow_refuses_without_wrapping() {
    for limit in [0, MAX_WORK + 1, usize::MAX] {
        assert!(CompleteBodySourceWorkVNext::new(limit).is_err());
    }
    let mut work = CompleteBodySourceWorkVNext::new(100).unwrap();
    work.charge(37).unwrap();
    assert_eq!(work.used(), 37); // preserve a nonzero incoming floor
    assert!(work.charge(64).is_err());
    assert_eq!(work.used(), 37);
    assert!(work.charge(usize::MAX).is_err());
    assert_eq!(work.used(), 37);
    work.charge(63).unwrap();
    assert_eq!(work.used(), 100);
    assert!(work.charge(1).is_err());
    work.charge(0).unwrap();
}
