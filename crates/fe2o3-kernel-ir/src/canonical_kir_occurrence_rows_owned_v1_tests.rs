use super::super::{
    CanonicalKirOccurrenceRowBytesStorageV1,
    encode_canonical_kir_occurrence_row_bytes_v1 as encode,
    read_canonical_kir_occurrence_row_bytes_v1 as read,
};
use super::*;
use crate::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 100_000_000;
const COUNTS: [u32; 9] = [1; 9];

// A neutral syntax-only subject with one row on every axis. Coordinates alias;
// it is deliberately not described as a real graph transition.
fn body() -> Vec<u8> {
    let mut bytes = vec![0; 232];
    word(&mut bytes, 20, 1); // Block's one segment.
    word(&mut bytes, 108, 1); // Definition's one descendant.
    bytes
}
fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn forged(bytes: &[u8], counts: [u32; 9]) -> CanonicalKirOccurrenceRowsRefV1<'_> {
    CanonicalKirOccurrenceRowsRefV1 {
        bytes,
        counts,
        storage: CanonicalKirOccurrenceRowBytesStorageV1(size_of::<
            CanonicalKirOccurrenceRowsRefV1<'_>,
        >()),
    }
}

#[derive(Clone, Copy)]
enum Fault {
    Error,
    Panic,
    Payload,
}
thread_local! {
    static FAULT: Cell<Option<(usize, Fault)>> = const { Cell::new(None) };
    static VISITS: Cell<usize> = const { Cell::new(0) };
}
struct FaultScope;
impl Drop for FaultScope {
    fn drop(&mut self) {
        FAULT.with(|slot| slot.set(None));
    }
}
fn fault(axis: usize, fault: Fault) -> FaultScope {
    FAULT.with(|slot| slot.set(Some((axis, fault))));
    VISITS.with(|visits| visits.set(0));
    FaultScope
}
struct PanickingPayload;
impl Drop for PanickingPayload {
    fn drop(&mut self) {
        panic!("owned-row payload destructor");
    }
}
pub(super) fn after_allocation(axis: usize) -> Result<()> {
    VISITS.with(|visits| visits.set(visits.get() + 1));
    let selected = FAULT.with(|slot| {
        if slot.get().is_some_and(|(selected, _)| selected == axis) {
            slot.take()
        } else {
            None
        }
    });
    match selected {
        None => Ok(()),
        Some((_, Fault::Error)) => Err(Resource::Allocation.into()),
        Some((_, Fault::Panic)) => panic!("owned-row allocation-boundary fault"),
        Some((_, Fault::Payload)) => std::panic::panic_any(PanickingPayload),
    }
}

#[test]
fn all_nine_owned_axes_round_trip_complete_body_and_exact_capacity_receipt() {
    let bytes = body();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(23).unwrap();
    let view = read(&bytes, COUNTS, &mut budget).unwrap();
    budget
        .reserve_storage(view.storage().retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (owner, storage) =
        materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    let sizes = [
        owner.rows.functions.capacity(),
        owner.rows.blocks.capacity(),
        owner.rows.segments.capacity(),
        owner.rows.operations.capacity(),
        owner.rows.definitions.capacity(),
        owner.rows.definition_outputs.capacity(),
        owner.rows.uses.capacity(),
        owner.rows.edges.capacity(),
        owner.rows.edge_arguments.capacity(),
    ];
    let expected = size_of::<InertOwnedCanonicalKirOccurrenceRowsV1>()
        + sizes
            .into_iter()
            .zip(SIZES)
            .map(|(count, size)| count * size)
            .sum::<usize>();
    assert_eq!(storage.retained_storage(), expected);
    assert_eq!(owner.storage(), storage);
    assert!(!owner.grants_authority());
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (encoded, encoded_storage) = encode(owner.candidate(), &mut budget).unwrap();
    assert_eq!(encoded.canonical_row_bytes(), bytes);
    assert_eq!(encoded.counts(), COUNTS);
    assert_eq!(budget.storage(), floor + storage.retained_storage());
    assert!(encoded_storage.retained_storage() > 232);
    drop(encoded);
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn independent_owned_rows_survive_wire_and_view_and_preserve_coordinate_aliases() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let owner = {
        let bytes = body();
        let view = read(&bytes, COUNTS, &mut budget).unwrap();
        let (owner, _) = materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget).unwrap();
        owner
    };
    budget
        .reserve_storage(owner.storage().retained_storage())
        .unwrap();
    let candidate = owner.candidate();
    assert!(std::ptr::eq(
        candidate.functions,
        owner.candidate().functions
    ));
    assert_eq!(candidate.functions[0].input, candidate.functions[0].output);
    assert_eq!(candidate.uses[0].input, candidate.uses[0].output);
    let (encoded, _) = encode(candidate, &mut budget).unwrap();
    assert_eq!(encoded.canonical_row_bytes(), body());
    assert!(!owner.grants_authority());
}

#[test]
fn separate_materializations_are_equal_claims_not_shared_vector_backing() {
    let bytes = body();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let view = read(&bytes, COUNTS, &mut budget).unwrap();
    budget
        .reserve_storage(view.storage().retained_storage())
        .unwrap();
    let (first, first_storage) =
        materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget).unwrap();
    budget
        .reserve_storage(first_storage.retained_storage())
        .unwrap();
    let (second, second_storage) =
        materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget).unwrap();
    assert_eq!(first.candidate().functions, second.candidate().functions);
    assert!(!std::ptr::eq(
        first.candidate().functions,
        second.candidate().functions
    ));
    let minimum =
        size_of::<InertOwnedCanonicalKirOccurrenceRowsV1>() + SIZES.into_iter().sum::<usize>();
    assert!(first_storage.retained_storage() >= minimum);
    assert!(second_storage.retained_storage() >= minimum);
    assert!(!first.grants_authority() && !second.grants_authority());
}

#[test]
fn empty_materialization_retains_real_owner_header_with_no_artificial_lifetime() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let owner = {
        let view = read(&[], [0; 9], &mut budget).unwrap();
        materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget)
            .unwrap()
            .0
    };
    assert!(owner.candidate().functions.is_empty());
    assert_eq!(
        owner.storage().retained_storage(),
        size_of::<InertOwnedCanonicalKirOccurrenceRowsV1>()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn hostile_public_syntax_and_private_forged_tags_padding_partitions_agree() {
    for (at, value) in [
        (32, 2),
        (36, 1),
        (60, 2),
        (76, 1),
        (80, 1),
        (84, 3),
        (92, 1),
        (96, 1),
        (112, 3),
        (120, 1),
        (124, 1),
        (132, 2),
        (136, 2),
        (156, 2),
        (20, 0),
        (108, 0),
    ] {
        let mut bytes = body();
        word(&mut bytes, at, value);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(17).unwrap();
        let public = match read(&bytes, COUNTS, &mut budget) {
            Err(error) => error,
            Ok(_) => panic!("hostile neutral view accepted"),
        };
        let private = match materialize_canonical_kir_occurrence_rows_v1(
            &forged(&bytes, COUNTS),
            &mut budget,
        ) {
            Err(error) => error,
            Ok(_) => panic!("forged hostile body materialized"),
        };
        assert_eq!(private, public, "offset {at}");
        assert!(matches!(
            private,
            Error::Malformed(_) | Error::RangePartition
        ));
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn forged_extent_cap_and_overflow_counts_refuse_before_any_vector_allocation() {
    let bytes = body();
    for end in 0..bytes.len() {
        VISITS.with(|visits| visits.set(0));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        assert!(matches!(
            materialize_canonical_kir_occurrence_rows_v1(
                &forged(&bytes[..end], COUNTS),
                &mut budget
            ),
            Err(Error::Malformed(_))
        ));
        assert_eq!(VISITS.with(Cell::get), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
    let mut extra = bytes;
    extra.push(0);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(
        materialize_canonical_kir_occurrence_rows_v1(&forged(&extra, COUNTS), &mut budget).is_err()
    );
    assert!(matches!(
        materialize_canonical_kir_occurrence_rows_v1(&forged(&[], [u32::MAX; 9]), &mut budget),
        Err(Error::Limit)
    ));
    assert_eq!(budget.peak_storage(), 0);
}

fn observe(
    view: &CanonicalKirOccurrenceRowsRefV1<'_>,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<usize>, usize, usize, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(19).unwrap();
    budget.charge_work(7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result =
        materialize_canonical_kir_occurrence_rows_v1(view, &mut budget).map(|(owner, storage)| {
            drop(owner);
            storage.retained_storage()
        });
    assert_eq!(budget.storage(), 19);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn exact_one_short_and_cumulative_work_storage_do_not_reset_the_input_floor() {
    let bytes = body();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let view = read(&bytes, COUNTS, &mut budget).unwrap();
    let baseline = observe(&view, WORK, STORAGE);
    assert!(baseline.0.is_ok());
    assert_eq!(observe(&view, baseline.1, baseline.2), baseline);
    assert!(matches!(
        observe(&view, baseline.1 - 1, baseline.2).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    let short = observe(&view, baseline.1, baseline.2 - 1);
    assert!(matches!(
        short.0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(short.3, Some(baseline.2));
    let mut work = Work::new(baseline.1 - 7);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    drop(materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget).unwrap());
    let used = budget.work();
    assert!(matches!(
        materialize_canonical_kir_occurrence_rows_v1(&view, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!((budget.work(), budget.storage()), (used, 19));
}

#[test]
fn all_requested_backing_is_denied_before_first_allocation_and_spare_capacity_is_charged() {
    let bytes = body();
    let requested =
        size_of::<InertOwnedCanonicalKirOccurrenceRowsV1>() + SIZES.into_iter().sum::<usize>();
    VISITS.with(|visits| visits.set(0));
    let result = observe(&forged(&bytes, COUNTS), WORK, 19 + requested - 1);
    assert!(matches!(
        result.0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(result.3, Some(19 + requested));
    assert_eq!(VISITS.with(Cell::get), 0);
    let backing = Vec::<super::super::FunctionRow>::with_capacity(7);
    let actual = backing.capacity() * size_of::<super::super::FunctionRow>();
    for shortage in [0, 1] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, 17 + actual - shortage);
        budget.reserve_storage(17).unwrap();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(size_of::<super::super::FunctionRow>())?;
            account_capacity::<super::super::FunctionRow>(1, backing.capacity(), budget)
        });
        assert_eq!(result.is_ok(), shortage == 0);
        assert_eq!(budget.storage(), 17);
        if shortage == 1 {
            assert_eq!(budget.failed_storage(), Some(17 + actual));
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(matches!(
        account_capacity::<super::super::FunctionRow>(2, 1, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        account_capacity::<super::super::FunctionRow>(usize::MAX, usize::MAX, &mut budget),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn each_actual_axis_boundary_error_panic_and_panicking_payload_preserves_floor() {
    let bytes = body();
    for axis in 0..9 {
        for mode in [Fault::Error, Fault::Panic, Fault::Payload] {
            let _fault = fault(axis, mode);
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(29).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                materialize_canonical_kir_occurrence_rows_v1(&forged(&bytes, COUNTS), &mut budget)
            }));
            assert_eq!(budget.storage(), 29);
            assert!(budget.work() > 0);
            assert_eq!(VISITS.with(Cell::get), axis + 1);
            match mode {
                Fault::Error => assert!(matches!(
                    result,
                    Ok(Err(Error::Resource(Resource::Allocation)))
                )),
                Fault::Panic => assert!(matches!(result, Ok(Err(Error::Panicked)))),
                Fault::Payload => assert!(result.is_err()),
            }
        }
    }
}

#[test]
fn rejected_real_owned_rows_drop_without_refunding_a_foreign_ledger_or_undercut() {
    let bytes = body();
    let mut own_work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut budget = Budget::new(&mut own_work, STORAGE);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE);
    budget.reserve_storage(17).unwrap();
    foreign.reserve_storage(71).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let retained = Cell::new(0);
    let result = scoped(&mut budget, |budget| {
        let (owner, storage) =
            materialize_canonical_kir_occurrence_rows_v1(&forged(&bytes, COUNTS), budget)?;
        retained.set(storage.retained_storage());
        budget.reserve_storage(storage.retained_storage())?;
        std::mem::swap(budget, &mut foreign);
        Ok(owner)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(
        (budget.storage(), foreign.storage()),
        (71, 17 + retained.get())
    );
    assert!(foreign.work_ledger_identity_v1() == ledger);
    std::mem::swap(&mut budget, &mut foreign);
    // The invalid scope did not refund the original ledger. The private test
    // now explicitly retires its inert credit before a distinct undercut case.
    budget.release_storage(retained.get()).unwrap();
    let result = scoped(&mut budget, |budget| {
        let (owner, storage) =
            materialize_canonical_kir_occurrence_rows_v1(&forged(&bytes, COUNTS), budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        drop(owner);
        budget.release_storage(storage.retained_storage() + 1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 16);
}
