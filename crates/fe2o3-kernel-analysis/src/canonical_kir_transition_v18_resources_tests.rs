use super::super::{Literal, Resource, State};
use super::tests::{Inventory, LIMIT, Rows, all_layouts, execution_module, inspect, unit_module};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, Module, VerifiedCanonicalKernelIrModuleV12,
};
use std::mem::size_of;

fn scratch(a: &Inventory<'_>, b: &Inventory<'_>) -> usize {
    // Two function maps; five input-block maps; two output-block maps;
    // bidirectional operation maps; input parents and output anchors;
    // three input-edge maps; literals and two one-byte flag rosters.
    let words = a.functions().len()
        + b.functions().len()
        + 5 * a.blocks().len()
        + 2 * b.blocks().len()
        + a.operations().len()
        + b.operations().len()
        + a.definitions().len()
        + b.definitions().len()
        + 3 * a.edges().len();
    size_of::<State<'_, '_, '_, '_, VerifiedCanonicalKernelIrModuleV18>>()
        + words * size_of::<usize>()
        + a.definitions().len() * size_of::<Option<Literal>>()
        + a.blocks().len()
        + b.definitions().len()
}

#[test]
fn independent_empty_and_unit_work_preserve_the_legacy_schedule_plus_table_header() {
    // Legacy empty "x": entry + state = 2; metadata = 1+2+1;
    // fixed-point visit = 1. V18 adds the table header: 8.
    // Unit "f" adds 10 initialized cells, 7 signature work, 8 block install,
    // output parameter roster 1, reachability 3, phi block 1, connector 2,
    // coverage 1, terminator 1, ordered roster 1, selected-edge refresh 1: 44.
    for (original, required) in [(Module::new("x"), 8), (unit_module(), 44)] {
        inspect(
            original.clone(),
            original,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                let prefix = 17;
                let peak = floor
                    + size_of::<CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>()
                    + scratch(a, b);
                for allowance in [required, required - 1] {
                    let mut work = Work::new(prefix + allowance);
                    let mut budget = Budget::new(&mut work, peak);
                    budget.charge_work(prefix).unwrap();
                    budget.reserve_storage(floor).unwrap();
                    let result =
                        check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget);
                    if allowance == required {
                        let (checked, receipt) = result.unwrap();
                        assert_eq!(
                            receipt.retained_storage(),
                            size_of::<CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>()
                        );
                        drop(checked);
                        assert_eq!(budget.work(), prefix + required);
                    } else {
                        assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                        assert_eq!(budget.work(), prefix + required - 1);
                    }
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.peak_storage(), peak);
                    assert_eq!(budget.failed_storage(), None);
                    drop(budget);
                    assert_eq!(
                        work.failed_work(),
                        (allowance != required).then_some(prefix + required)
                    );
                }
            },
        );
    }
    assert_eq!(
        size_of::<CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>(),
        size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>()
    );
    assert_eq!(
        size_of::<State<'_, '_, '_, '_, VerifiedCanonicalKernelIrModuleV18>>(),
        size_of::<State<'_, '_, '_, '_, VerifiedCanonicalKernelIrModuleV12>>()
    );
}

#[test]
fn finite_table_work_is_independent_of_object_extent_and_preserves_denial_history() {
    // One header, ten row headers, two record fields, two union fields,
    // two direct variants and three niche variants. Pointer recursion is an ID,
    // and array object extent is fixed metadata: neither expands this roster.
    let rows = all_layouts();
    assert_eq!(rows.len(), 10);
    let required = 1 + 10 + 2 + 2 + 2 + 3;
    for allowance in [required, required - 1] {
        for prior_denial in [false, true] {
            let prefix = 19;
            let mut work = Work::new(prefix + allowance);
            let mut budget = Budget::new(&mut work, 23);
            budget.reserve_storage(23).unwrap();
            budget.charge_work(prefix).unwrap();
            let first_attempt = prefix + required + 100;
            if prior_denial {
                assert!(budget.charge_work(required + 100).is_err());
            }
            let result = same_table(&rows, &rows, &mut budget);
            if allowance == required {
                assert_eq!(result, Ok(true));
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            }
            assert_eq!(budget.work(), prefix + allowance);
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.peak_storage(), 23);
            drop(budget);
            assert_eq!(
                work.failed_work(),
                if prior_denial {
                    Some(first_attempt)
                } else if allowance < required {
                    Some(prefix + required)
                } else {
                    None
                }
            );
        }
    }
    let mut rows = rows;
    if let Kind::Array { length, stride, .. } = &mut rows[5].kind {
        *length = u64::MAX;
        *stride = u64::MAX;
    }
    // Private comparison, not admission of this intentionally impossible layout.
    let mut work = Work::new(required);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(same_table(&rows, &rows, &mut budget), Ok(true));
    assert_eq!(budget.work(), required);
}

#[test]
fn table_preflight_refuses_before_state_allocation_and_records_the_first_debit() {
    let original = execution_module(1);
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            let mut work = Work::new(0);
            let mut budget = Budget::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(
                check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget),
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            drop(budget);
            assert_eq!(work.failed_work(), Some(1));
        },
    );
}

#[test]
fn independently_counted_storage_scratch_has_exact_and_one_short_boundaries() {
    let original = execution_module(1);
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert_eq!(
                (
                    a.functions().len(),
                    a.blocks().len(),
                    a.operations().len(),
                    a.edges().len()
                ),
                (1, 1, 6, 0)
            );
            assert_eq!(a.definitions().len(), 8); // slice/index args, four nominal and two parts
            let peak = floor
                + size_of::<CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>()
                + scratch(a, b);
            for limit in [peak, peak - 1] {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, limit);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                let result =
                    check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget);
                if limit == peak {
                    assert!(result.is_ok());
                    assert_eq!(budget.peak_storage(), peak);
                    assert_eq!(budget.failed_storage(), None);
                } else {
                    assert!(
                        matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == peak && error.limit() == limit)
                    );
                    assert_eq!(budget.failed_storage(), Some(peak));
                    assert!(budget.peak_storage() < peak);
                }
                assert_eq!(budget.storage(), floor);
            }
        },
    );
}

#[test]
fn borrowed_view_and_control_index_receipts_never_release_owners_or_row_storage() {
    let original = execution_module(1);
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, floor + LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (checked, receipt) =
                check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let outer = budget.storage();
            let (index, index_receipt) =
                CheckedCanonicalKirControlIndexV18::derive_v18(&checked, &mut budget).unwrap();
            let retained = size_of::<CheckedCanonicalKirControlIndexV18<'_, '_, '_>>()
                + a.blocks().len() * size_of::<super::super::CanonicalKirBlockControlV1>()
                + a.edges().len() * size_of::<super::super::CanonicalKirEdgeControlV1>()
                + a.uses().len() * size_of::<Option<super::super::CanonicalKirOutputUseV1>>()
                + a.edge_arguments().len()
                    * size_of::<Option<fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1>>();
            assert_eq!(index_receipt.retained_storage(), retained);
            assert_eq!(budget.storage(), outer);
            budget.reserve_storage(retained).unwrap();
            drop(index);
            budget.release_storage(retained).unwrap();
            drop(checked);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
            let peak = budget.peak_storage();
            // Failed history is global, not a permission to bypass a fresh checker.
            assert!(budget.reserve_storage(LIMIT * 2).is_err());
            let denial = budget.failed_storage();
            let (checked, _) =
                check_canonical_kir_transition_v18(a, b, rows.candidate(), &mut budget).unwrap();
            drop(checked);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.failed_storage(), denial);
        },
    );
}
