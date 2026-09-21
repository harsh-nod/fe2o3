use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::mem::size_of_val;

fn copy_run(work_limit: usize, storage_limit: usize) -> (R<usize>, usize, usize, Option<usize>) {
    let input = [0x37u8; 19];
    let sibling = vec![0x53u8; 37];
    let floor = size_of_val(&input) + size_of::<Vec<u8>>() + sibling.capacity();
    let mut work = Work::new(work_limit);
    work.charge_work(7).unwrap();
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = scoped(&mut budget, |b| {
        let value = copied(&input, b)?;
        assert_eq!(value, input);
        let capacity = value.capacity();
        assert_eq!(b.storage(), floor + size_of::<Vec<u8>>() + capacity);
        drop(value);
        Ok(capacity)
    });
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, vec![0x53u8; 37]);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn final_f_wire_copy_measures_actual_capacity_and_exact_work() {
    let (result, work, peak, failed) = copy_run(usize::MAX, MAX_STORAGE);
    let capacity = result.unwrap();
    assert!(capacity >= 19);
    assert_eq!(work, 7 + 1 + 19);
    assert_eq!(
        peak,
        19 + size_of::<Vec<u8>>() + 37 + size_of::<Vec<u8>>() + capacity
    );
    assert_eq!(failed, None);
    let exact = copy_run(work, peak);
    assert_eq!(exact.0.unwrap(), capacity);
    assert_eq!((exact.1, exact.2, exact.3), (work, peak, None));
}

#[test]
fn final_f_wire_copy_work_short_keeps_typed_denial_and_sibling() {
    let (ok, work, peak, _) = copy_run(usize::MAX, MAX_STORAGE);
    ok.unwrap();
    let short = copy_run(work - 1, peak);
    match short.0 {
        Err(E::Resource(Resource::Work(error))) => {
            assert_eq!(error.actual(), work);
            assert_eq!(error.limit(), work - 1);
        }
        other => panic!("exact copying Work refusal: {other:?}"),
    }
    assert_eq!((short.1, short.2, short.3), (8, peak, None));
}

#[test]
fn final_f_wire_copy_first_storage_denial_precedes_allocation_work() {
    let floor = 19 + size_of::<Vec<u8>>() + 37;
    let attempted = floor + size_of::<Vec<u8>>() + 19;
    let short = copy_run(usize::MAX, attempted - 1);
    match short.0 {
        Err(E::Resource(Resource::Storage(error))) => {
            assert_eq!(error.actual(), attempted);
            assert_eq!(error.limit(), attempted - 1);
        }
        other => panic!("exact copying Storage refusal: {other:?}"),
    }
    assert_eq!((short.1, short.2, short.3), (7, floor, Some(attempted)));
}

#[test]
fn final_f_wire_copy_zero_work_does_not_return_unpaid_backing() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    let result = scoped(&mut budget, |b| copied(&[0x17; 19], b));
    match result {
        Err(E::Resource(Resource::Work(error))) => {
            assert_eq!(error.actual(), 1);
            assert_eq!(error.limit(), 0);
        }
        other => panic!("preallocation Work refusal: {other:?}"),
    }
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 37, 37 + size_of::<Vec<u8>>() + 19)
    );
}

#[test]
fn final_f_wire_temporary_extent_overflow_never_reserves_or_charges() {
    let mut work = Work::new(100);
    work.charge_work(3).unwrap();
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        scoped(&mut budget, |b| vector::<u64>(usize::MAX, b)),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        scoped(&mut budget, |b| codec::<Association>(usize::MAX, b)),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (3, 37, 37, None)
    );
}

#[test]
fn final_f_wire_scope_restores_live_floor_after_panic() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    let result: R<()> = scoped(&mut budget, |b| {
        b.reserve_storage(91)?;
        b.charge_work(3)?;
        panic!("failed owned temporary");
    });
    assert!(matches!(result, Err(E::Panicked)));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (3, 37, 128)
    );
}

#[test]
fn final_f_wire_scope_refuses_and_does_not_refund_foreign_ledger() {
    let mut first = Work::new(100);
    let mut other = Work::new(100);
    let mut budget = Budget::new(&mut first, MAX_STORAGE);
    let mut foreign = Budget::new(&mut other, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    foreign.reserve_storage(53).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |b| {
        b.reserve_storage(11)?;
        std::mem::swap(b, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!((budget.storage(), foreign.storage()), (53, 48));
    std::mem::swap(&mut budget, &mut foreign);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.release_storage(11).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn final_f_wire_field_comparison_prepays_exact_visit_and_keeps_inputs() {
    let left = [1u8, 2, 3];
    let right = [1u8, 2, 4];
    let mut work = Work::new(6);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget
        .reserve_storage(size_of_val(&left) + size_of_val(&right))
        .unwrap();
    match same(&left, &right, &mut budget) {
        Err(E::Resource(Resource::Work(error))) => {
            assert_eq!((error.actual(), error.limit()), (7, 6));
        }
        other => panic!("comparison must charge before observing mismatch: {other:?}"),
    }
    assert_eq!(budget.work(), 0);
    assert_eq!(left, [1, 2, 3]);
    assert_eq!(right, [1, 2, 4]);
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    assert!(matches!(
        same(&left, &right, &mut budget),
        Err(E::Mismatch("exact live-derived output field"))
    ));
    assert_eq!(budget.work(), 7);
}

#[test]
fn final_f_wire_word_overflow_preserves_caller_buffer() {
    if let Some(value) = (u32::MAX as usize).checked_add(1) {
        let mut bytes = vec![0x53u8; 4];
        assert!(matches!(
            word(&mut bytes, value),
            Err(E::Resource(Resource::Arithmetic))
        ));
        assert_eq!(bytes, vec![0x53u8; 4]);
    }
}

#[test]
fn final_f_wire_field_roles_are_complete_and_canonical() {
    assert_eq!(FIELDS.len(), 14);
    for (index, field) in FIELDS.into_iter().enumerate() {
        assert_eq!(field as usize, index);
    }
}

#[test]
fn final_f_wire_byte_buffer_transfer_reserves_before_further_work() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    let inherited = 37;
    budget.reserve_storage(inherited).unwrap();
    let header = size_of::<PreparedRefinedForwardingWireV1>() - size_of::<Live>();
    assert!(header >= size_of::<Vec<u8>>() + size_of::<usize>());
    budget.reserve_storage(header).unwrap();
    let wire = scoped(&mut budget, |b| copied(&[0x53u8; 19], b)).unwrap();
    assert_eq!(budget.storage(), inherited + header);
    let retained = wire.capacity();
    budget.reserve_storage(retained).unwrap();
    same(&wire, &[0x53u8; 19], &mut budget).unwrap();
    assert_eq!(budget.storage(), inherited + header + retained);
    drop(wire);
    budget.release_storage(header + retained).unwrap();
    assert_eq!(budget.storage(), inherited);
}
