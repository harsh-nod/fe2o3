use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

const FLOOR: usize = 23;
const PREFIX: usize = 5;
type Key = (SemanticFunctionIdV1, SemanticFunctionIdV1);
type Row = (u32, u32, u32);

fn key(row: &Row) -> Key {
    (
        SemanticFunctionIdV1::from_index(row.0),
        SemanticFunctionIdV1::from_index(row.1),
    )
}

fn functions() -> BTreeMap<Key, usize> {
    [(1, 9, 0), (1, 1, 1), (9, 1, 2), (5, 5, 3)]
        .into_iter()
        .map(|row| (key(&row), row.2 as usize))
        .collect()
}

fn rows() -> Vec<Row> {
    let mut rows = Vec::with_capacity(32);
    rows.extend([
        (9, 1, 0),
        (1, 1, 1),
        (1, 9, 2),
        (9, 1, 3),
        (1, 9, 4),
        (1, 1, 5),
        (9, 1, 6),
        (9, 1, 7),
        (9, 1, 8),
    ]);
    rows
}

fn floor(rows: &Vec<Row>) -> usize {
    FLOOR + rows.capacity() * std::mem::size_of::<Row>()
}

fn baseline() -> (usize, usize) {
    let rows = rows();
    let floor = floor(&rows);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(PREFIX).unwrap();
    order_borrowed_correspondence_records_v1(rows, &functions(), key, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    (budget.work(), budget.peak_storage())
}

#[test]
fn empty_order_is_free_and_preserves_even_spare_capacity() {
    for capacity in [0, 8] {
        let rows: Vec<Row> = Vec::with_capacity(capacity);
        let pointer = rows.as_ptr();
        let capacity = rows.capacity();
        let floor = floor(&rows);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let ordered = order_borrowed_correspondence_records_v1(
            rows,
            &functions(),
            |_| panic!("empty input must not request a key"),
            &mut budget,
        )
        .unwrap();
        assert!(ordered.is_empty());
        assert_eq!(ordered.as_ptr(), pointer);
        assert_eq!(ordered.capacity(), capacity);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn ordering_is_stable_root_qualified_and_reuses_the_input_allocation() {
    let rows = rows();
    let pointer = rows.as_ptr();
    let capacity = rows.capacity();
    let floor = floor(&rows);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(PREFIX).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let calls = Cell::new(0);
    let ordered = order_borrowed_correspondence_records_v1(
        rows,
        &functions(),
        |row| {
            calls.set(calls.get() + 1);
            key(row)
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(calls.get(), 9);
    assert_eq!(
        ordered.iter().map(|row| row.2).collect::<Vec<_>>(),
        [2, 4, 1, 5, 0, 3, 6, 7, 8]
    );
    assert_eq!(ordered.as_ptr(), pointer);
    assert_eq!(ordered.capacity(), capacity);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let one_run = budget.work() - PREFIX;
    let peak = budget.peak_storage();
    let again =
        order_borrowed_correspondence_records_v1(ordered, &functions(), key, &mut budget).unwrap();
    assert_eq!(
        again.iter().map(|row| row.2).collect::<Vec<_>>(),
        [2, 4, 1, 5, 0, 3, 6, 7, 8]
    );
    assert_eq!(again.as_ptr(), pointer);
    assert_eq!(budget.work() - PREFIX, 2 * one_run);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), peak);
}

#[test]
fn resource_denials_restore_floor_at_every_work_boundary_and_storage_boundary() {
    let (exact_work, exact_storage) = baseline();
    for work_limit in PREFIX..=exact_work {
        let rows = rows();
        let floor = floor(&rows);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, exact_storage);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(PREFIX).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = order_borrowed_correspondence_records_v1(rows, &functions(), key, &mut budget);
        if work_limit == exact_work {
            result.unwrap();
            assert_eq!(budget.work(), exact_work);
            assert_eq!(budget.peak_storage(), exact_storage);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    for storage_limit in floor(&rows())..=exact_storage {
        let rows = rows();
        let floor = floor(&rows);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(PREFIX).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = order_borrowed_correspondence_records_v1(rows, &functions(), key, &mut budget);
        if storage_limit == exact_storage {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
            assert!(budget.failed_storage().unwrap() > storage_limit);
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn mismatches_and_callback_unwind_drop_records_and_restore_scratch() {
    struct Tracked<'a> {
        row: Row,
        dropped: &'a Cell<usize>,
    }
    impl Drop for Tracked<'_> {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    for failure in ["missing", "ordinal", "panic"] {
        let dropped = Cell::new(0);
        let rows = rows()
            .into_iter()
            .map(|row| Tracked {
                row,
                dropped: &dropped,
            })
            .collect::<Vec<_>>();
        let count = rows.len();
        let floor = FLOOR + rows.capacity() * std::mem::size_of::<Tracked<'_>>();
        let mut functions = functions();
        if failure == "missing" {
            functions.remove(&key(&(1, 9, 0)));
        } else if failure == "ordinal" {
            functions.insert(key(&(1, 9, 0)), functions.len());
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            order_borrowed_correspondence_records_v1(
                rows,
                &functions,
                |record| {
                    if failure == "panic" && record.row.2 == 2 {
                        std::panic::panic_any(73_u32);
                    }
                    key(&record.row)
                },
                &mut budget,
            )
        }));
        match result {
            Err(payload) => {
                assert_eq!(failure, "panic");
                assert_eq!(payload.downcast_ref::<u32>(), Some(&73));
            }
            Ok(result) => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            )),
        }
        assert_eq!(dropped.get(), count);
        assert_eq!(budget.storage(), floor);
        assert!(budget.peak_storage() > floor);
        assert!(budget.work() > 0);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}
