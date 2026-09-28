use super::layout_tests::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

#[test]
fn original_extension_transfer_moves_only_the_one_row_table_and_checks_its_closed_graph() {
    for hostile in [false, true] {
        let (declarations, ty) = original_cycle_types_v29(2, false);
        let owner = owner_with(declarations);
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(271).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let schema = layouts
            .select_original_closure(&owner, ty, &mut budget)
            .unwrap();
        assert_eq!(layouts.physical.borrow().original_extensions.len(), 4);
        let expected = retained_rows(&layouts);
        let pointer = layouts.physical.borrow().rows.as_ptr();
        let field_pointer = match &layouts.physical.borrow().rows[schema.0 as usize].kind {
            StorageLayoutKindV1::Record(fields) => fields.as_ptr(),
            _ => panic!("original source record"),
        };
        if hostile {
            let mut physical = layouts.physical.borrow_mut();
            let StorageLayoutKindV1::Record(fields) = &mut physical.rows[schema.0 as usize].kind
            else {
                panic!("original source record");
            };
            fields[0].layout = schema;
        }
        let mut candidate = Module::new("original-extension-transfer");
        let result = layouts.install_rows(&owner, &mut candidate, &mut budget);
        if hostile {
            assert!(result.is_err());
            assert!(candidate.storage_layouts.is_empty());
            assert_eq!(budget.storage(), 271);
        } else {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(candidate.storage_layouts.len(), 4);
            assert_eq!(candidate.storage_layouts.as_ptr(), pointer);
            assert!(matches!(&candidate.storage_layouts[schema.0 as usize].kind,
                StorageLayoutKindV1::Record(fields) if fields.as_ptr() == field_pointer));
            assert_eq!(budget.storage(), 271 + expected);
            drop(candidate);
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), 271);
        }
    }
}

fn retained_rows(table: &SourceStorageLayoutsV29<'_>) -> usize {
    let physical = table.physical.try_borrow().unwrap();
    let rows = physical.rows.capacity() * size_of::<StorageLayoutV1>();
    let boxes = physical
        .rows
        .iter()
        .map(|row| match &row.kind {
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                fields.len() * size_of::<StorageFieldV1>()
            }
            StorageLayoutKindV1::Variants { variants, .. } => {
                variants.len() * size_of::<StorageVariantV1>()
            }
            _ => 0,
        })
        .sum::<usize>();
    rows + boxes
}

#[test]
fn selected_rows_are_checked_at_consuming_install_without_transferring_failed_tables() {
    for case in 0..3 {
        let owner = owner();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(107).unwrap();
        let table = SourceStorageLayoutsV29::new_with_limits(
            &owner,
            &[DESCRIPTOR],
            fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                rows: 6,
                edges: 8,
                containment_depth: 2,
                object_bytes: 16,
            },
            &mut budget,
        )
        .unwrap();
        let element = table.row_for(&owner, WORD, &mut budget).unwrap();
        let schema = table
            .select_schema(
                &owner,
                DESCRIPTOR,
                SourceStorageSelectionV29::Slice {
                    element,
                    value_space: AddressSpace::Global,
                    access: AccessMode::ReadOnly,
                },
                &mut budget,
            )
            .unwrap();
        let expected = retained_rows(&table);
        let pointer = table.rows(&owner, &mut budget).unwrap().as_ptr();
        if case == 1 {
            // Mutate only after admitted owner, bounded original table and an
            // actual selected row have been constructed. No authority is added.
            let mut physical = table.physical.try_borrow_mut().unwrap();
            let StorageLayoutKindV1::Slice { data, .. } = physical.rows[schema.0 as usize].kind
            else {
                panic!("slice")
            };
            let StorageLayoutKindV1::Pointer(data) =
                &mut physical.rows[data.layout.0 as usize].kind
            else {
                panic!("data")
            };
            data.pointee = StorageLayoutIdV1(u32::MAX);
        } else if case == 2 {
            budget.charge_work(usize::MAX - budget.work()).unwrap();
        }
        let mut candidate = Module::new("selected-row-consuming-transfer");
        let result = table.install_rows(&owner, &mut candidate, &mut budget);
        if case == 0 {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(candidate.storage_layouts.as_ptr(), pointer);
            assert_eq!(candidate.storage_layouts.len(), 6);
            assert_eq!(budget.storage(), 107 + expected);
            drop(candidate);
            budget.release_storage(expected).unwrap();
        } else {
            assert!(result.is_err());
            assert!(candidate.storage_layouts.is_empty());
        }
        assert_eq!(budget.storage(), 107);
    }
}

#[test]
fn source_storage_rows_transfer_moves_the_actual_table_and_retains_exact_backing_credit() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(73).unwrap();
    let table =
        SourceStorageLayoutsV29::new(&owner, &[PAIR, DIRECT, NICHE, HUGE], &mut budget).unwrap();
    let expected = retained_rows(&table);
    let rows = table.rows(&owner, &mut budget).unwrap();
    let pointer = rows.as_ptr();
    let length = rows.len();
    let field_pointer = rows
        .iter()
        .find_map(|row| match &row.kind {
            StorageLayoutKindV1::Record(fields) if !fields.is_empty() => Some(fields.as_ptr()),
            _ => None,
        })
        .unwrap();
    drop(rows);
    budget.reserve_storage(91).unwrap();
    let mut candidate = Module::new("source-storage-transfer");
    assert_eq!(
        table
            .install_rows(&owner, &mut candidate, &mut budget)
            .unwrap(),
        expected
    );
    assert_eq!(budget.storage(), 73 + 91 + expected);
    assert_eq!(candidate.storage_layouts.as_ptr(), pointer);
    assert_eq!(candidate.storage_layouts.len(), length);
    assert!(candidate.storage_layouts.iter().any(|row| matches!(&row.kind, StorageLayoutKindV1::Record(fields) if fields.as_ptr() == field_pointer)));
    assert!(candidate.storage_layouts.iter().any(|row| matches!(row.kind, StorageLayoutKindV1::Array { length, .. } if length == u64::from(u32::MAX))));
    drop(candidate);
    budget.release_storage(expected).unwrap();
    assert_eq!(budget.storage(), 73 + 91);
}

#[test]
fn source_storage_empty_rows_transfer_is_valid_and_refunds_all_table_storage() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let table = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
    let mut candidate = Module::new("empty-source-storage-transfer");
    assert_eq!(
        table
            .install_rows(&owner, &mut candidate, &mut budget)
            .unwrap(),
        0
    );
    assert_eq!(budget.storage(), 0);
    assert!(candidate.storage_layouts.is_empty());
}

#[test]
fn source_storage_rows_transfer_source_and_destination_refusals_preserve_candidate() {
    let owner = owner();
    let other = super::layout_tests::owner();
    for wrong_source in [false, true] {
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(73).unwrap();
        let table = SourceStorageLayoutsV29::new(&owner, &[PAIR], &mut budget).unwrap();
        let mut candidate = Module::new("unchanged-source-storage-transfer");
        if !wrong_source {
            candidate.storage_layouts = Vec::with_capacity(1);
        }
        let pointer = candidate.storage_layouts.as_ptr();
        let capacity = candidate.storage_layouts.capacity();
        assert!(
            table
                .install_rows(
                    if wrong_source { &other } else { &owner },
                    &mut candidate,
                    &mut budget
                )
                .is_err()
        );
        assert_eq!(candidate.storage_layouts.as_ptr(), pointer);
        assert_eq!(candidate.storage_layouts.capacity(), capacity);
        assert!(candidate.storage_layouts.is_empty());
        assert_eq!(budget.storage(), 73);
    }
}

#[test]
fn source_storage_rows_transfer_denial_has_no_further_work_and_never_changes_candidate() {
    let owner = owner();
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        let table = SourceStorageLayoutsV29::new(&owner, &[PAIR], &mut budget).unwrap();
        // One original-source owner visit, then one prepaid constant-time visit
        // per physical row. Box lengths are read without walking their elements.
        let exact = 1 + table.rows(&owner, &mut budget).unwrap().len();
        let remaining = exact - usize::from(short);
        let expected = retained_rows(&table);
        budget
            .charge_work(usize::MAX - budget.work() - remaining)
            .unwrap();
        let before = budget.work();
        let mut candidate = Module::new("bounded-source-storage-transfer");
        let result = table.install_rows(&owner, &mut candidate, &mut budget);
        if short {
            assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
            assert_eq!(budget.work(), before + 1);
            assert_eq!(budget.storage(), 0);
            assert!(candidate.storage_layouts.is_empty());
        } else {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(budget.work(), before + exact);
            assert_eq!(budget.storage(), expected);
            drop(candidate);
            budget.release_storage(expected).unwrap();
        }
    }
}

#[test]
fn source_storage_rows_transfer_lost_or_foreign_custody_never_refunds() {
    let owner = owner();
    for foreign in [false, true] {
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        let table = SourceStorageLayoutsV29::new(&owner, &[PAIR], &mut budget).unwrap();
        let mut candidate = Module::new("lost-source-storage-transfer");
        if foreign {
            let mut foreign_work = super::layout_tests::work();
            let mut other = Budget::new(&mut foreign_work, 64 * 1024 * 1024);
            other.reserve_storage(budget.storage()).unwrap();
            let before = other.storage();
            assert!(
                table
                    .install_rows(&owner, &mut candidate, &mut other)
                    .is_err()
            );
            assert_eq!(other.storage(), before);
            assert_eq!(other.work(), 0);
        } else {
            budget.release_storage(1).unwrap();
            let before = budget.storage();
            assert!(
                table
                    .install_rows(&owner, &mut candidate, &mut budget)
                    .is_err()
            );
            assert_eq!(budget.storage(), before);
        }
        assert!(candidate.storage_layouts.is_empty());
    }
}
